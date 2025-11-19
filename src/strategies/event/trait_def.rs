use anyhow::Context;
pub use anyhow::{Result, anyhow};
pub use async_trait::async_trait;
use chrono::Utc;

use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::{WebDb, WebDocker},
    entity::{event_teams, event_users, events, instances, users},
    strategies::event::common,
};
pub enum EventStatus {
    NotStarted,
    Ongoing,
    Ended,
}
// 单次请求的ctx对吗
#[derive(Debug)]
pub struct EventContext {
    pub db: WebDb,
    pub docker: WebDocker,
    pub event: events::Model,
    pub user: users::Model,
    pub team: Option<event_teams::Model>,
}

#[derive(Debug)]
pub struct EventContextBuilder {
    db: Option<WebDb>,
    docker: Option<WebDocker>,
    event: Option<events::Model>,
    user: Option<users::Model>,
    team: Option<event_teams::Model>,
}
impl EventContextBuilder {
    /// 新建 builder
    pub fn new() -> Self {
        Self {
            db: None,
            docker: None,
            event: None,
            user: None,
            team: None,
        }
    }

    pub fn db(mut self, db: WebDb) -> Self {
        self.db = Some(db);
        self
    }

    pub fn docker(mut self, docker: WebDocker) -> Self {
        self.docker = Some(docker);
        self
    }

    pub fn event(mut self, event: Option<events::Model>) -> Self {
        self.event = event;
        self
    }

    pub fn user(mut self, user: users::Model) -> Self {
        self.user = Some(user);
        self
    }

    pub fn team(mut self, team: event_teams::Model) -> Self {
        self.team = Some(team);
        self
    }

    /// 构造 EventContext
    pub fn build(self) -> Result<EventContext> {
        Ok(EventContext {
            db: self.db.context("db is required")?,
            docker: self.docker.context("docker is required")?,
            user: self.user.context("user is required")?,
            event: self.event.unwrap_or(common::virtual_practice_event()),
            team: self.team,
        })
    }
}

// 专门用于函数，路由处自己定义再传入
#[derive(Debug, Deserialize, Serialize)]
pub struct SubmitFlagRequest {
    // single
    pub instance_id: Option<Uuid>,
    // value
    pub flag: String,
}
// return type
#[derive(Debug, Serialize, Deserialize)]
pub struct EventInstanceResult {
    pub instance: instances::Model,
    pub challenge_name: String,
    pub nickname: String,
}

// 额外默认方法实现
pub trait EventStrategyExt: EventStrategy {
    fn get_event_status(&self, ctx: &EventContext) -> EventStatus {
        let now = Utc::now().naive_utc();
        if now < ctx.event.start_time {
            EventStatus::NotStarted
        } else if now > ctx.event.end_time {
            EventStatus::Ended
        } else {
            EventStatus::Ongoing
        }
    }

    fn should_not_started(&self, ctx: &EventContext) -> Result<()> {
        match self.get_event_status(ctx) {
            EventStatus::NotStarted => Ok(()),
            EventStatus::Ongoing => Err(anyhow!("Event is ongoing")),
            EventStatus::Ended => Err(anyhow!("Event is ended")),
        }
    }

    fn should_ongoing(&self, ctx: &EventContext) -> Result<()> {
        match self.get_event_status(ctx) {
            EventStatus::NotStarted => Err(anyhow!("Event is not started")),
            EventStatus::Ongoing => Ok(()),
            EventStatus::Ended => Err(anyhow!("Event is ended")),
        }
    }

    fn should_ongoing_or_ended(&self, ctx: &EventContext) -> Result<()> {
        match self.get_event_status(ctx) {
            EventStatus::NotStarted => Err(anyhow!("Event is not started")),
            EventStatus::Ongoing | EventStatus::Ended => Ok(()),
        }
    }

    async fn should_user_joined(&self, ctx: &EventContext) -> Result<()> {
        event_users::Entity::find_by_id((ctx.event.id, ctx.user.id))
            .one(ctx.db.get_ref())
            .await?
            .ok_or_else(|| anyhow!("User not joined the event!"))?;
        Ok(())
    }
}

// 专注业务逻辑
#[async_trait]
pub trait EventStrategy: Send + Sync {
    async fn submit(&self, ctx: &EventContext, sfr: SubmitFlagRequest) -> Result<()>;
    async fn get_instance_by_challenge_id(
        &self,
        ctx: &EventContext,
        challenge_id: Uuid,
    ) -> Result<instances::Model>;
    async fn get_instances(&self, ctx: &EventContext) -> Result<Vec<EventInstanceResult>>;
    async fn launch_instance(
        &self,
        ctx: &EventContext,
        challenge_id: Uuid,
    ) -> Result<instances::Model>;
}

// 让所有 EventStrategy 都自动实现扩展 trait
impl<T: EventStrategy + ?Sized> EventStrategyExt for T {}
