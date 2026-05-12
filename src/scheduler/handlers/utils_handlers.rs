use async_trait::async_trait;
use tracing::info;

use crate::{
    db::{WebDb, WebRustfs},
    entity::scheduled_tasks,
    scheduler::TaskHandler,
};

pub struct CleanUnusedRustFSFilesHandler {
    pub db: WebDb,
    pub rustfs: WebRustfs,
}

#[async_trait]
impl TaskHandler for CleanUnusedRustFSFilesHandler {
    fn trigger_type(&self) -> &'static str {
        "cron"
    }
    fn task_key(&self) -> &'static str {
        "CLEAN_RUSTFS"
    }
    async fn run(&self, task: scheduled_tasks::Model) -> anyhow::Result<()> {
        info!("{} CleanRunningInstancesHandler", self.task_key());
        // check images mainly
        info!("{} task is running : {:?}", self.task_key(), &task);
        Ok(())
    }
}
