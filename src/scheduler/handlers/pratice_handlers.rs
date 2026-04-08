use crate::{
    db::{WebDb, WebDocker},
    entity::{instances, scheduled_tasks, sea_orm_active_enums::InstanceStatus, users},
    scheduler::TaskHandler,
    strategies::event,
};
use async_trait::async_trait;

use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter,
};
use tracing::{error, info};

pub struct CleanRunningInstancesHandler {
    pub db: WebDb,
    pub docker: WebDocker,
}

#[async_trait]
impl TaskHandler for CleanRunningInstancesHandler {
    fn task_key(&self) -> &'static str {
        "CLEAN_INSTANCES"
    }
    fn trigger_type(&self) -> &'static str {
        "startup"
    }
    async fn run(&self, task: scheduled_tasks::Model) -> anyhow::Result<()> {
        info!("CleanRunningInstancesHandler");
        info!("task is running : {:?}", &task);

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
                    info!("Killed instance {}", instance.id);
                }
                Err(e) => {
                    let mut m_instance = instance.into_active_model();
                    m_instance.status = Set(InstanceStatus::Failed);
                    let instance = m_instance.update(self.db.get_ref()).await?;
                    error!("{}: But {} was killed!", e, instance.id);
                }
            }
        }

        Ok(())
    }
}
