use crate::{
    db::{WebDb, WebDocker},
    entity::{
        events, instances, scheduled_tasks,
        sea_orm_active_enums::{EventType, InstanceStatus},
        users,
    },
    scheduler::TaskHandler,
    strategies::event,
};
use async_trait::async_trait;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter,
};
use tracing::{error, info};
use uuid::Uuid;

pub struct CleanRunningInstancesHandler {
    pub db: WebDb,
    pub docker: WebDocker,
}

#[async_trait]
impl TaskHandler for CleanRunningInstancesHandler {
    fn trigger_type(&self) -> &'static str {
        "startup"
    }
    fn task_key(&self) -> &'static str {
        "CLEAN_INSTANCES"
    }
    async fn run(&self, task: scheduled_tasks::Model) -> anyhow::Result<()> {
        info!("[CLEAN_INSTANCES] CleanRunningInstancesHandler");

        info!("[CLEAN_INSTANCES] task is running : {:?}", &task);

        let instances_users = instances::Entity::find()
            .filter(instances::Column::Status.eq(InstanceStatus::Running))
            .find_also_related(users::Entity)
            .all(self.db.get_ref())
            .await?;

        for (instance, user) in instances_users
            .into_iter()
            .filter_map(|(i, u)| u.map(|user| (i, user)))
        {
            match event::common::destroy_instance(&self.db, &self.docker, instance.id, &user).await
            {
                Ok(_) => {
                    info!("[CLEAN_INSTANCES] Killed instance {}", instance.id);
                }
                Err(e) => {
                    let mut m_instance = instance.into_active_model();
                    m_instance.status = Set(InstanceStatus::Failed);
                    let instance = m_instance.update(self.db.get_ref()).await?;
                    error!("[CLEAN_INSTANCES] {}: But {} was killed!", e, instance.id);
                }
            }
        }

        Ok(())
    }
}

pub struct CheckPraticeEventHandler {
    pub db: WebDb,
}
#[async_trait]
impl TaskHandler for CheckPraticeEventHandler {
    fn trigger_type(&self) -> &'static str {
        "startup"
    }
    fn task_key(&self) -> &'static str {
        "CHECK_PRATICE_EVENT"
    }

    async fn run(&self, task: scheduled_tasks::Model) -> anyhow::Result<()> {
        let pratice_event = events::Entity::find_by_id(Uuid::nil())
            .one(self.db.get_ref())
            .await?;

        if pratice_event.is_some() {
            info!("[CHECK_PRATICE_EVENT] PraticeEvent already exists");
            return Ok(());
        }

        let pratice_event = events::ActiveModel {
            id: Set(Uuid::nil()),
            r#type: Set(EventType::JeopardyPractice),
            title: Set("PraticeEvent".into()),
            description: Set(Some("Practice Event".into())),
            hidden: Set(true),
            start_time: Set(Utc::now().into()),
            end_time: Set((Utc::now() + chrono::Duration::days(36500)).into()),
            rules: Set("".into()),
            allow_join: Set(true),
            flag_prefix: Set(None), // use config settings
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
            ..Default::default()
        };

        let pratice_event = pratice_event.insert(self.db.get_ref()).await?;

        info!(
            "[CHECK_PRATICE_EVENT] Inserting pratice_event: {:?}",
            pratice_event
        );
        Ok(())
    }
}
