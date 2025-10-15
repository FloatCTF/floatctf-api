use crate::{
    api::{preclude::*, service::events::EventStatus},
    entity::{
        challenges, event_instances, event_team_members, events, instances,
        sea_orm_active_enums::{EventType, InstanceStatus},
        users,
    },
};
use actix_web::HttpRequest; // TODO : for log
use anyhow::{Context, anyhow};
use fcmc::ChallengeMeta;

/// GET /api/instances
#[get("")]
pub async fn get_instances(
    user: UserJwtGuard,
    db: WebDb,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<instances::Model>> {
    // challenge no hidden
    let user = user.into_inner();
    let mut query_params = query_params.0;

    let stmt = instances::Entity::find()
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .filter(instances::Column::Ref.eq("Training"))
        .filter(instances::Column::UserId.eq(user.id));

    if let (Some(limit), Some(page)) = (query_params.limit, query_params.page) {
        let paginator = stmt.paginate(db.get_ref(), limit);
        let mut items = paginator.fetch_page(page.saturating_sub(1)).await?;
        query_params.total = Some(paginator.num_items().await? as usize);

        for item in &mut items {
            item.flag.clear();
        }

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    } else {
        let mut items = stmt.all(db.get_ref()).await?;

        query_params.total = Some(items.len());

        for item in &mut items {
            item.flag.clear();
        }

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    }
}

/// GET /api/instances/{instance_id}
#[get("/{instance_id}")]
pub async fn get_instance(
    user: UserJwtGuard,
    db: WebDb,
    instance_id: Path<Uuid>,
) -> UniResult<instances::Model> {
    let instance_id = instance_id.into_inner();
    let user = user.into_inner();

    let mut model = instances::Entity::find_by_id(instance_id)
        .filter(instances::Column::UserId.eq(user.id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", instance_id)))?;

    model.flag.clear();

    UniResponse::ok(model.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LaunchInstanceRequest {
    event_id: Option<Uuid>,
    challenge_id: Uuid,
    // for team
}

/// POST /api/instances/launch
#[post("/launch")]
pub async fn launch_instance(
    user: UserJwtGuard,
    db: WebDb,
    docker: WebDocker,
    lir: Json<LaunchInstanceRequest>,
) -> UniResult<instances::Model> {
    let user = user.into_inner();
    let lir = lir.into_inner();

    // practice

    match lir.event_id {
        Some(event_id) => {
            let event = events::Entity::find_by_id(event_id)
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound("no event".into()))?;

            match EventStatus::check(&db, &event_id).await? {
                EventStatus::Ended | EventStatus::NotStarted => {
                    return Err(UniError::CustomError("Event is no ongoing".to_string()));
                }
                EventStatus::Ongoing => {}
            }

            match event.r#type {
                EventType::JeopardySingle => {
                    return jeopardy_event_single_launch(db, docker, user, lir, event).await;
                }
                EventType::JeopardyTeam => {
                    return jeopardy_event_team_launch(db, docker, user, lir, event).await;
                }
                _ => return UniError::InternalError("unimplemented!".into()).into(),
            }
        }
        None => {
            return jeopardy_single_practice_launch(db, docker, user, lir).await;
        }
    }
}

/// DELETE /api/instances/{instance_id}
#[delete("/{instance_id}")]
pub async fn destroy_instance(
    user: UserJwtGuard,
    db: WebDb,
    docker: WebDocker,
    instance_id: Path<Uuid>,
) -> UniResult<u64> {
    let user = user.into_inner();
    let instance_id = instance_id.into_inner();
    __destroy_instance(db, docker, instance_id, user).await
}

pub async fn __destroy_instance(
    db: WebDb,
    docker: WebDocker,
    id: Uuid,
    user: users::Model,
) -> UniResult<u64> {
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
            .ok_or_else(|| UniError::NotFound("challenge not found?".to_string()))?;

        let cm = ChallengeMeta::from_toml_str(&challenge.toml_str)
            .map_err(|e| UniError::InternalError(format!("destroy the instance: {}", e)))?;

        let instance_identifier = instance.identifier.clone();
        let mut m_instance = instance.into_active_model();
        m_instance.status = Set(InstanceStatus::Completed);
        m_instance.updated_at = Set(Utc::now().naive_utc());
        m_instance.update(db.get_ref()).await?;

        //  no docker
        if cm.docker.is_some() {
            cm.stop_and_remove(docker.get_ref(), &instance_identifier)
                .await
                .map_err(|e| UniError::InternalError(format!("destroy the instance: {}", e)))?;
        }
    }

    UniResponse::ok(1.into()).into()
}

pub async fn jeopardy_single_practice_launch(
    db: WebDb,
    docker: WebDocker,
    user: users::Model,
    lir: LaunchInstanceRequest,
) -> UniResult<instances::Model> {
    let running_instances_count = instances::Entity::find()
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .filter(instances::Column::UserId.eq(user.id))
        .filter(instances::Column::Ref.eq("Training"))
        .count(db.get_ref())
        .await?;

    let max_instances_per_user = 1 as u64;

    if running_instances_count >= max_instances_per_user {
        return UniError::CustomError(format!(
            "you can only launch {} instances at the same time in practice mode",
            max_instances_per_user
        ))
        .into();
    }

    // 是否已经有运行中的实例
    if let Some(running_instance) = instances::Entity::find()
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .filter(instances::Column::ChallengeId.eq(lir.challenge_id))
        .filter(instances::Column::UserId.eq(user.id))
        .one(db.get_ref())
        .await?
    {
        return UniResponse::ok(running_instance.into()).into();
    }

    // 调用公共函数启动实例
    let identifier = {
        let user_id_prefix = get_uuid_prefix(&user.id);
        let challenge_id_prefix = get_uuid_prefix(&lir.challenge_id);
        format!("P-{}-{}", user_id_prefix, challenge_id_prefix)
    };

    let res_instance = launch_instance_common(
        &db,
        &docker,
        lir.challenge_id,
        identifier,
        user.id,
        "Training".into(),
        None,
    )
    .await
    .map_err(|e| UniError::InternalError(e.to_string()))?;

    UniResponse::ok(res_instance.into()).into()
}

pub async fn jeopardy_event_single_launch(
    db: WebDb,
    docker: WebDocker,
    user: users::Model,
    lir: LaunchInstanceRequest,
    event: events::Model,
) -> UniResult<instances::Model> {
    let event_id = lir.event_id.unwrap();
    let challenge_id = lir.challenge_id;

    let running_instances_count = event_instances::Entity::find()
        .filter(event_instances::Column::EventId.eq(event_id))
        .filter(event_instances::Column::UserId.eq(user.id))
        .join(
            JoinType::InnerJoin,
            event_instances::Relation::Instances.def(),
        )
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .filter(instances::Column::Ref.eq("JeopardySingle"))
        .count(db.get_ref())
        .await?;

    let max_instances_per_user = 2 as u64;

    if running_instances_count >= max_instances_per_user {
        return UniError::CustomError(format!(
            "you can only launch {} instances at the same time in JeopardySingle mode",
            max_instances_per_user
        ))
        .into();
    }

    // 检查是否已有运行实例
    if let Some((_, Some(instance))) = event_instances::Entity::find()
        .filter(event_instances::Column::EventId.eq(event_id))
        .filter(event_instances::Column::ChallengeId.eq(challenge_id))
        .filter(event_instances::Column::UserId.eq(user.id))
        .find_also_related(instances::Entity)
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .one(db.get_ref())
        .await?
    {
        return UniResponse::ok(instance.into()).into();
    }

    // 调用公共启动逻辑
    let identifier = {
        let event_id_preifx = get_uuid_prefix(&event_id);
        let user_id_prefix = get_uuid_prefix(&user.id);
        let challenge_id_prefix = get_uuid_prefix(&challenge_id);
        format!(
            "JS-{}-{}-{}",
            event_id_preifx, user_id_prefix, challenge_id_prefix
        )
    };

    let res_instance = launch_instance_common(
        &db,
        &docker,
        challenge_id,
        identifier,
        user.id,
        "JeopardySingle".into(),
        event.flag_prefix,
    )
    .await
    .map_err(|e| UniError::InternalError(e.to_string()))?;

    // 写入 event_instances 记录
    let new_event_instance = event_instances::ActiveModel {
        event_id: Set(event_id),
        challenge_id: Set(challenge_id),
        user_id: Set(user.id),
        instance_id: Set(res_instance.id),
        team_id: Set(None),
        ..Default::default()
    };
    new_event_instance.insert(db.get_ref()).await?;

    UniResponse::ok(res_instance.into()).into()
}

pub async fn jeopardy_event_team_launch(
    db: WebDb,
    docker: WebDocker,
    user: users::Model,
    lir: LaunchInstanceRequest,
    event: events::Model,
) -> UniResult<instances::Model> {
    let event_id = lir.event_id.unwrap();
    let challenge_id = lir.challenge_id;

    let (team_id, team_member_count) = {
        let team_member = event_team_members::Entity::find()
            .filter(event_team_members::Column::EventId.eq(event_id))
            .filter(event_team_members::Column::UserId.eq(user.id))
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound("you are not in any team".into()))?;

        let team_member_count = event_team_members::Entity::find()
            .filter(event_team_members::Column::TeamId.eq(team_member.team_id))
            .count(db.get_ref())
            .await?;

        (team_member.team_id, team_member_count)
    };

    // team_members * 2
    let running_instances_count = event_instances::Entity::find()
        .filter(event_instances::Column::EventId.eq(event_id))
        .filter(event_instances::Column::UserId.eq(user.id))
        .filter(event_instances::Column::TeamId.eq(team_id))
        .join(
            JoinType::InnerJoin,
            event_instances::Relation::Instances.def(),
        )
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .filter(instances::Column::Ref.eq("JeopardyTeam"))
        .count(db.get_ref())
        .await?;

    let max_instances_per_user = team_member_count * 2;

    if running_instances_count >= max_instances_per_user {
        return UniError::CustomError(format!(
            "you can only launch {} instances at the same time in JeopardyTeam mode",
            max_instances_per_user
        ))
        .into();
    }

    let running_instance = event_instances::Entity::find()
        .filter(event_instances::Column::EventId.eq(event_id))
        .filter(event_instances::Column::ChallengeId.eq(challenge_id))
        .filter(event_instances::Column::TeamId.eq(team_id))
        .find_also_related(instances::Entity)
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .one(db.get_ref())
        .await?;

    if let Some((_, Some(instance))) = running_instance {
        return UniResponse::ok(instance.into()).into();
    }

    let identifier = {
        let event_id_preifx = get_uuid_prefix(&event_id);
        let team_id_prefix = get_uuid_prefix(&team_id);
        let challenge_id_prefix = get_uuid_prefix(&challenge_id);
        format!(
            "JT-{}-{}-{}",
            event_id_preifx, team_id_prefix, challenge_id_prefix
        )
    };

    let res_instance = launch_instance_common(
        &db,
        &docker,
        challenge_id,
        identifier,
        user.id,
        "JeopardyTeam".into(),
        event.flag_prefix,
    )
    .await
    .map_err(|e| UniError::InternalError(e.to_string()))?;

    let new_event_instance = event_instances::ActiveModel {
        event_id: Set(event_id),
        challenge_id: Set(challenge_id),
        user_id: Set(user.id),
        instance_id: Set(res_instance.id),
        team_id: Set(Some(team_id)),
        ..Default::default()
    };
    new_event_instance.insert(db.get_ref()).await?;

    UniResponse::ok(res_instance.into()).into()
}

async fn launch_instance_common(
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
        .ok_or_else(|| anyhow!("no such challenge: {}", challenge_id))?;

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
        challenge_id: Set(challenge.id),
        user_id: Set(user_id),
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
                if let Err(e) = __destroy_instance(d_db, d_docker, d_id, d_user).await {
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

pub fn get_uuid_prefix(uuid: &Uuid) -> String {
    let uuid_str = uuid.to_string();
    uuid_str.split('-').next().unwrap_or("").to_string()
}
