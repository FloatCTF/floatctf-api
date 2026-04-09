use crate::db::{WebDb, WebDocker};
use crate::entity::scheduled_tasks;
use crate::scheduler::handlers;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::prelude::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, ConnectionTrait, EntityTrait, IntoActiveModel,
    QueryFilter, Statement,
};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

#[async_trait]
pub trait TaskHandler: Send + Sync {
    fn task_key(&self) -> &'static str;
    fn trigger_type(&self) -> &'static str;
    async fn run(&self, task: scheduled_tasks::Model) -> anyhow::Result<()>;
}

pub struct TaskScheduler {
    db: WebDb,
    docker: WebDocker,
    handlers: HashMap<String, Arc<dyn TaskHandler>>,
}

impl TaskScheduler {
    pub fn new(db: WebDb, docker: WebDocker) -> Self {
        Self {
            db,
            docker,
            handlers: HashMap::new(),
        }
    }

    pub async fn register_handler(&mut self, key: &str, handler: Arc<dyn TaskHandler>) {
        self.handlers.insert(key.to_string(), handler);
    }

    pub async fn start_polling(self: Arc<Self>) {
        let mut interval = actix_web::rt::time::interval(Duration::from_secs(5));

        loop {
            interval.tick().await;
            if let Err(e) = self.fetch_and_run().await {
                error!("[Scheduler] 执行任务时出错: {}", e);
            }
        }
    }

    async fn fetch_and_run(&self) -> Result<()> {
        // 这里的 SQL 变化：NOW() + INTERVAL '5 seconds'
        // 提前把未来 5 秒内要执行的任务全部锁住并取出来
        // 只取出is_enabled=true的任务
        let sql = r#"
                UPDATE scheduled_tasks
                SET status = 'running', updated_at = NOW()
                WHERE id IN (
                    SELECT id FROM scheduled_tasks
                    WHERE status = 'pending' AND enabled = true
                      AND execute_at <= NOW() + INTERVAL '5 seconds'
                    ORDER BY execute_at ASC
                    FOR UPDATE SKIP LOCKED LIMIT 20
                )
                RETURNING *;
            "#;

        let tasks = scheduled_tasks::Entity::find()
            .from_raw_sql(Statement::from_string(
                self.db.get_ref().get_database_backend(),
                sql,
            ))
            .all(self.db.get_ref()) // 这里的返回值会自动推导为 Vec<scheduled_tasks::Model>
            .await?;

        for task in tasks {
            let engine = Arc::new(self.clone_logic()); // 模拟克隆引用
            actix_web::rt::spawn(async move {
                engine.dispatch_with_precision(task).await;
            });
        }

        Ok(())
    }

    async fn dispatch_with_precision(&self, task: scheduled_tasks::Model) {
        if !task.enabled {
            warn!("[{}] task is disabled : {:?}", task.task_key, task);
            return;
        }

        let task_key = task.task_key.clone();

        // --- 精准睡眠阶段 ---
        if let Some(execute_at) = task.execute_at {
            let now = Utc::now();

            // 将 execute_at (FixedOffset) 转为 Utc 进行计算
            let target_time = execute_at.with_timezone(&Utc);

            if target_time > now {
                let duration_to_wait = target_time - now;
                let std_duration = duration_to_wait.to_std().unwrap_or(Duration::from_secs(0));

                info!(
                    "[Precision] 任务 {} 提前命中，等待 {:?} 后准时执行",
                    task_key, std_duration
                );
                actix_web::rt::time::sleep(std_duration).await;
            }
        }

        // --- 立即执行阶段 ---
        info!("[Execute] 时间已到，精准触发: {}", task_key);
        let result = if let Some(handler) = self.handlers.get(&task_key) {
            handler.run(task.clone()).await
        } else {
            Err(anyhow!("未注册处理器: {}", task_key))
        };

        self.mark_done(task, result).await;
    }

    async fn mark_done(&self, task: scheduled_tasks::Model, res: Result<()>) {
        let mut active_item: scheduled_tasks::ActiveModel = task.clone().into_active_model();
        let now = Utc::now();

        // 更新最后运行时间
        active_item.updated_at = ActiveValue::Set(now.into());
        active_item.last_run_at = ActiveValue::Set(Some(now.into()));

        if let Err(e) = res {
            active_item.status = ActiveValue::Set("failed".to_string());
            active_item.error_msg = ActiveValue::Set(Some(e.to_string()));
            error!("❌ 任务执行出错: {}", e);
        } else {
            // ✨ 核心逻辑：区分触发类型
            match task.trigger_type.as_str() {
                "cron" => {
                    if let Some(cron_expr) = &task.cron_expr {
                        match cron::Schedule::from_str(cron_expr) {
                            Ok(schedule) => {
                                let next_tick = schedule.upcoming(Utc).next();
                                if let Some(next_time) = next_tick {
                                    // ✨ 增加：检查是否超过了结束时间 (end_time/expires_at)
                                    let is_expired = if let Some(end_time) = task.expires_at {
                                        next_time > end_time.with_timezone(&Utc)
                                    } else {
                                        false
                                    };

                                    if is_expired {
                                        active_item.status =
                                            ActiveValue::Set("completed".to_string());
                                        info!(
                                            "[Cron] 任务 {} 已到期 (expires_at)，停止循环",
                                            task.task_key
                                        );
                                    } else {
                                        active_item.execute_at =
                                            ActiveValue::Set(Some(next_time.into()));
                                        active_item.status =
                                            ActiveValue::Set("pending".to_string());
                                        info!(
                                            "[Cron] 任务 {} 已重置，下次执行: {:?}",
                                            task.task_key, next_time
                                        );
                                    }
                                } else {
                                    active_item.status = ActiveValue::Set("completed".to_string());
                                }
                            }
                            Err(e) => {
                                // 如果 Cron 表达式写错了，不能让它死循环，设为 failed
                                active_item.status = ActiveValue::Set("failed".to_string());
                                active_item.error_msg =
                                    ActiveValue::Set(Some(format!("Cron 解析失败: {}", e)));
                            }
                        }
                    }
                }
                "startup" => {
                    // startup 任务执行完后，通常设为 completed
                    // 这样在本次运行期间不会再被扫描，直到下次重启被 init_and_recover 重置
                    active_item.status = ActiveValue::Set("completed".to_string());
                }
                _ => {
                    // once 类型执行完直接结束
                    active_item.status = ActiveValue::Set("completed".to_string());
                }
            }
        }

        if let Err(e) = active_item.update(self.db.get_ref()).await {
            error!("❌ 数据库状态更新失败: {}", e);
        }
    }

    pub async fn init_and_recover(&self) -> Result<()> {
        info!("[Scheduler] 正在恢复未完成的任务");
        scheduled_tasks::Entity::update_many()
            .col_expr(scheduled_tasks::Column::Status, Expr::value("pending"))
            .filter(scheduled_tasks::Column::Status.eq("running"))
            .exec(self.db.get_ref())
            .await?;
        info!("[Scheduler] 恢复完成");

        info!("[Scheduler] 正在执行Starup任务");
        let startup_tasks = scheduled_tasks::Entity::find()
            .filter(scheduled_tasks::Column::TriggerType.eq("startup"))
            .all(self.db.get_ref())
            .await?;
        for task in startup_tasks {
            let scheduler_arc = Arc::new(self.clone_logic());
            actix_web::rt::spawn(async move {
                scheduler_arc.dispatch_with_precision(task.clone()).await;
                scheduler_arc.mark_done(task, Ok(())).await;
            });
        }
        info!("[Scheduler] Starup任务 执行完成");
        self.fetch_and_run().await?;

        Ok(())
    }

    fn clone_logic(&self) -> Self {
        Self {
            db: self.db.clone(),
            docker: self.docker.clone(),
            handlers: self.handlers.clone(),
        }
    }

    pub async fn init_startup_handlers(&mut self) -> Result<()> {
        let startup_tasks: Vec<(&str, &str, Arc<dyn TaskHandler>)> = vec![
            (
                "00000000-0000-0000-0000-000000000000",
                "检查练习event",
                Arc::new(handlers::CheckPraticeEventHandler {
                    db: self.db.clone(),
                }),
            ),
            (
                "00000000-0000-0000-0000-000000000001",
                "实例清理",
                Arc::new(handlers::CleanRunningInstancesHandler {
                    db: self.db.clone(),
                    docker: self.docker.clone(),
                }),
            ), // (
               //     "另一个-UUID",
               //     "Flag刷新",
               //     Arc::new(handlers::FlagRefreshHandler {}),
               // ),
        ];

        for (id_str, name, handler) in startup_tasks {
            let id = Uuid::parse_str(id_str).map_err(|_| anyhow!("无效的 UUID: {}", id_str))?;

            // ✨ 修正 2：直接使用已经包装好的 Arc
            let key = handler.task_key();
            self.register_handler(key, handler.clone()).await;

            let exists = scheduled_tasks::Entity::find_by_id(id)
                .one(self.db.get_ref())
                .await?;

            if exists.is_none() {
                warn!("[Init] 数据库中未发现基础任务 '{}'，正在初始化...", name);

                let startup_model = scheduled_tasks::ActiveModel {
                    id: ActiveValue::Set(id),
                    task_name: ActiveValue::Set(name.to_string()),
                    task_key: ActiveValue::Set(key.to_string()),
                    trigger_type: ActiveValue::Set(handler.trigger_type().to_string()),
                    status: ActiveValue::Set("pending".to_string()),
                    created_at: ActiveValue::Set(Utc::now().into()),
                    updated_at: ActiveValue::Set(Utc::now().into()),
                    ..Default::default()
                };

                startup_model.insert(self.db.get_ref()).await?;
                info!("[Init] 任务 '{}' 成功录入数据库", name);
            }
        }
        Ok(())
    }
}
