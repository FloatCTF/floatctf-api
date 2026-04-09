use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use serde_json::Value;
use tracing::error;
use uuid::Uuid;

use crate::{
    db::WebDb,
    entity::{event_logs, events, logs},
};

#[derive(Clone, Debug)]
pub struct LogService {
    pub db: WebDb,
}

impl LogService {
    pub fn new(db: WebDb) -> Self {
        Self { db }
    }

    pub async fn add_log(
        &self,
        level: &str,
        category: &str,
        action: &str,
        message: &str,
        details: Value,
        user_id: Option<Uuid>,
        superadmin_id: Option<Uuid>,
        request: Option<&actix_web::HttpRequest>,
    ) {
        let ip_address = request
            .map(|req| {
                req.connection_info()
                    .realip_remote_addr()
                    .map(|ip| ip.to_string())
            })
            .flatten();

        let log = logs::ActiveModel {
            level: Set(level.to_string()),
            category: Set(category.to_string()),
            action: Set(action.to_string()),
            message: Set(message.to_string()),
            details: Set(details),
            ip_address: Set(ip_address),
            user_id: Set(user_id),
            superadmin_id: Set(superadmin_id),
            ..Default::default()
        };
        let db = self.db.get_ref().clone();
        actix_web::rt::spawn(async move {
            if let Err(e) = log.insert(&db).await {
                error!("Failed to insert log: {}", e);
            }
        });
    }
    pub async fn add_event_log(
        &self,
        event: &events::Model,
        level: &str,
        action: &str,
        details: Value,
        user_id: Option<Uuid>,
        team_id: Option<Uuid>,
        request: Option<&actix_web::HttpRequest>,
    ) {
        let ip_address = request
            .map(|req| {
                req.connection_info()
                    .realip_remote_addr()
                    .map(|ip| ip.to_string())
            })
            .flatten();

        let log = event_logs::ActiveModel {
            event_id: Set(event.id),
            user_id: Set(user_id),
            team_id: Set(team_id),
            r#type: Set(event.r#type.clone()),
            level: Set(level.to_string()),
            action: Set(action.to_string()),
            details: Set(details),
            ip_address: Set(ip_address),
            ..Default::default()
        };

        let db = self.db.get_ref().clone();
        actix_web::rt::spawn(async move {
            if let Err(e) = log.insert(&db).await {
                error!("Failed to insert log: {}", e);
            }
        });
    }
}
