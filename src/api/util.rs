use crate::{config::get_setting, db::WebDb, entity::kv_store};
use anyhow::{Result, anyhow};
use chrono::{Duration, Utc};
use lettre::message::header;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DbConn, EntityTrait};
use serde_json::Value;

pub async fn send_email(
    db: &WebDb,
    to_list: &[&str],
    cc_list: Option<&[&str]>,
    subject: &str,
    body: &str,
) -> Result<()> {
    let smtp_uri = get_setting(db.get_ref(), "SMTP_URI")
        .await
        .map_err(|e| anyhow!("Failed to get SMTP_URI: {}", e))?;
    let parts: Vec<&str> = smtp_uri.split(':').collect();
    if parts.len() != 3 {
        return Err(anyhow!(
            "Invalid SMTP_URI format, expected server:user:pass"
        ));
    }
    let smtp_server = parts[0];
    let smtp_user = parts[1];
    let smtp_pass = parts[2];

    let mut builder = Message::builder().from(smtp_user.parse()?);

    if to_list.is_empty() {
        return Err(anyhow!("Recipient list cannot be empty"));
    }

    for recipient in to_list {
        builder = builder.to(recipient.parse()?);
    }

    if let Some(cc) = cc_list {
        for recipient in cc {
            builder = builder.cc(recipient.parse()?);
        }
    }

    let email = builder
        .subject(subject)
        .header(header::ContentType::TEXT_HTML)
        .body(body.to_string())?;

    let creds = Credentials::new(smtp_user.to_string(), smtp_pass.to_string());
    let mailer = SmtpTransport::relay(smtp_server)?
        .credentials(creds)
        .build();

    mailer
        .send(&email)
        .map_err(|e| anyhow!("Failed to send email: {}", e))?;

    Ok(())
}

pub fn none_if_empty(s: Option<String>) -> Option<String> {
    match s {
        Some(x) if x.trim().is_empty() => None,
        other => other,
    }
}

// ── KV store helpers ────────────────────────────────────────────────────────

/// 读取 key，过期自动删除并返回 None。
pub async fn kv_get(db: &DbConn, key: &str) -> Result<Option<Value>> {
    let entry = kv_store::Entity::find_by_id(key).one(db).await?;
    match entry {
        Some(e)
            if e.expires_at
                .is_some_and(|exp| exp.timestamp() <= Utc::now().timestamp()) =>
        {
            kv_store::Entity::delete_by_id(key).exec(db).await?;
            Ok(None)
        }
        Some(e) => Ok(Some(e.value)),
        None => Ok(None),
    }
}

/// 写入 key，ttl_secs 为 None 表示永不过期。
pub async fn kv_set(db: &DbConn, key: &str, value: Value, ttl_secs: Option<i64>) -> Result<()> {
    let expires_at =
        ttl_secs
            .map(|s| Utc::now() + Duration::seconds(s))
            .map(|dt: chrono::DateTime<Utc>| {
                let fixed: chrono::DateTime<chrono::FixedOffset> = dt.into();
                fixed
            });

    let existing = kv_store::Entity::find_by_id(key).one(db).await?;

    if let Some(model) = existing {
        let mut active: kv_store::ActiveModel = model.into();
        active.value = Set(value);
        active.expires_at = Set(expires_at);
        active.updated_at = Set(Utc::now().into());
        active.update(db).await?;
    } else {
        let now: chrono::DateTime<chrono::FixedOffset> = Utc::now().into();
        kv_store::ActiveModel {
            key: Set(key.to_string()),
            value: Set(value),
            expires_at: Set(expires_at),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(db)
        .await?;
    }

    Ok(())
}

/// 删除 key。
pub async fn kv_del(db: &DbConn, key: &str) -> Result<()> {
    kv_store::Entity::delete_by_id(key).exec(db).await?;
    Ok(())
}
