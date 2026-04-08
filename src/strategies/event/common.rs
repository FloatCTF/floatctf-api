pub use crate::api::preclude::*;

pub use anyhow::{Context, Result, anyhow};

use chrono::Utc;
use fcmc::ChallengeMeta;
use sea_orm::{DbConn, TryIntoModel};

use crate::{
    db::{WebDb, WebDocker},
    entity::{
        challenges, event_teams, events, instances,
        sea_orm_active_enums::{EventType, InstanceStatus},
        users,
    },
};

pub async fn launch_instance(
    db: &WebDb,
    docker: &WebDocker,
    challenge_id: Uuid,
    identifier: String,
    user_id: Uuid,
    r#ref: String,
    flag_prefix: Option<String>,
) -> anyhow::Result<instances::Model> {
    // challenge 查询 & meta 解析
    let challenge = challenges::Entity::find_by_id(challenge_id)
        .one(db.get_ref())
        .await?
        .ok_or(anyhow!("no such challenge: {}", challenge_id))?;

    let cm = ChallengeMeta::from_toml_str(&challenge.toml_str)
        .context("failed to parse challenge toml_str")?;

    let flag = if cm.flag.value.is_empty() {
        gen_flag(&db, flag_prefix).await
    } else {
        cm.flag.value.clone()
    };

    let node_ip = get_setting(&db, "NODE_IP").await?;
    let http_prefix = get_setting(&db, "HTTP_PREFIX").await?;

    //  这里的逻辑是 如果是web 就返回url, 如果是pwn 就返回nc, 如果是misc 就返回description

    //  是根据 有无docker 来判断 而不仅仅是类型, 比如AI题目我可能暂时放到 Misc里
    let content = match &cm.docker {
        Some(d) => match d.is_nc {
            Some(true) => {
                let port = cm
                    .create_and_start(docker, &identifier, &flag)
                    .await
                    .map_err(|e| anyhow!("{}", e))?;
                format!("nc {} {}", node_ip, port)
            }
            _ => {
                let port = cm
                    .create_and_start(docker, &identifier, &flag)
                    .await
                    .map_err(|e| anyhow!("{}", e))?;
                let url = format!("{}{}:{}", http_prefix, node_ip, port);
                format!(
                    "<a href=\"{url}\" target=\"_blank\" rel=\"noopener noreferrer\" download >{url}</a>",
                )
            }
        },
        None => "".into(),
    };

    let delay = get_setting(&db, "INSTANCE_DESTROY_DELAY")
        .await?
        .parse::<i64>()?;

    let destroy_at = Utc::now().naive_utc() + chrono::Duration::minutes(delay);
    let new_instance = instances::ActiveModel {
        status: Set(InstanceStatus::Running),
        flag: Set(flag),
        content: Set(content.into()),
        user_id: Set(user_id),
        challenge_id: Set(challenge_id.into()),
        r#ref: Set(r#ref),
        destroy_at: Set(destroy_at.clone()),
        identifier: Set(identifier),
        ..Default::default()
    };

    let mut res = new_instance.insert(db.get_ref()).await?;
    res.flag.clear();

    // 添加自动销毁
    let d_db = db.clone();
    let d_docker = docker.clone();
    let d_id = res.id;
    let d_user = users::Entity::find_by_id(user_id)
        .one(db.get_ref())
        .await?
        .unwrap();

    actix_web::rt::spawn(async move {
        let now = Utc::now().naive_utc();
        let delay = (destroy_at - now).to_std();
        match delay {
            Ok(d) => {
                actix_web::rt::time::sleep(d).await;
                if let Err(e) = destroy_instance(&d_db, &d_docker, d_id, &d_user).await {
                    tracing::error!("[@destroy_auto]{}", e)
                }
            }
            Err(e) => {
                tracing::error!("[@destroy_auto]{}", e)
            }
        }
    });

    Ok(res)
}

pub async fn destroy_instance(
    db: &WebDb,
    docker: &WebDocker,
    id: Uuid,
    user: &users::Model,
) -> Result<()> {
    let running_instance = instances::Entity::find_by_id(id)
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .filter(instances::Column::UserId.eq(user.id))
        .one(db.get_ref())
        .await?;

    if let Some(instance) = running_instance {
        let challenge = instance
            .find_related(challenges::Entity)
            .one(db.get_ref())
            .await?
            .ok_or(anyhow!("challenge not found"))?;

        let cm = ChallengeMeta::from_toml_str(&challenge.toml_str)
            .map_err(|e| anyhow!("destroy the instance: {}", e))?;

        let instance_identifier = instance.identifier.clone();
        let mut m_instance = instance.into_active_model();
        m_instance.status = Set(InstanceStatus::Completed);
        m_instance.updated_at = Set(Utc::now().naive_utc());
        m_instance.update(db.get_ref()).await?;

        //  no docker
        if cm.docker.is_some() {
            fcmc::stop_and_remove(docker.get_ref(), &instance_identifier)
                .await
                .map_err(|e| anyhow!("destroy the instance: {}", e))?;
        }
    }

    Ok(())
}

pub async fn calculate_next_dynamic_score(
    db: &DbConn,
    base_points: f64,
    solves: u64,
) -> anyhow::Result<f64> {
    if solves <= 0 {
        return Ok(base_points);
    }

    let decay = get_setting(db, "EVENT_SCORE_DECAY").await?.parse::<f64>()?;

    let event_score_min_percent = get_setting(db, "EVENT_SCORE_MIN_PERCENT")
        .await?
        .parse::<f64>()?;

    let min_points = base_points * event_score_min_percent;

    let current_points =
        min_points + (base_points - min_points) * ((decay / (decay + (solves) as f64)).sqrt());
    Ok(current_points.max(min_points))
}

pub fn get_uuid_prefix(uuid: &Uuid) -> String {
    let uuid_str = uuid.to_string();
    uuid_str.split('-').next().unwrap_or("").to_string()
}

pub async fn gen_flag(db: &WebDb, flag_prefix: Option<String>) -> String {
    let unique_value = Uuid::new_v4();

    let prefix = match flag_prefix {
        Some(prefix) => prefix,
        None => get_setting(db, "FLAG_PREFIX")
            .await
            .unwrap_or("flag".into()),
    };

    format!("{}{{{}}}", prefix, unique_value)
}

pub fn virtual_practice_event() -> events::Model {
    let new_event = events::ActiveModel {
        id: Set(Uuid::nil()),
        r#type: Set(EventType::JeopardyPractice),
        title: Set("Virtual Practice Event".into()),
        description: Set(Some("Virtual Practice Event".into())),
        hidden: Set(true),
        start_time: Set(Utc::now().naive_utc()),
        end_time: Set(Utc::now().naive_utc() + chrono::Duration::days(365)),
        rules: Set("".into()),
        allow_join: Set(true),
        flag_prefix: Set(None), // use config settings
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    };
    let e = new_event
        .try_into_model()
        .expect("?????????????????? impossible !!!!!!!!!!!!!!!!!!!!");
    e
}
