use super::super::preclude::*;
use crate::{
    auth::UserJwtGuard,
    config::get_setting,
    db::WebDocker,
    entity::{
        challenges, event_instances, event_team_members, event_teams,
        events::Entity,
        instances,
        prelude::{
            Challenges, EventInstances, EventTeamMembers, EventTeams, Events, Instances, Users,
        },
        sea_orm_active_enums::{EventType, InstanceStatus},
        users,
    },
};
use actix_web::{HttpMessage, HttpRequest, delete};
use anyhow::{Context, Result, anyhow};
use fcmc::ChallengeMeta;
use sea_orm::{ColumnTrait, JoinType, ModelTrait, QueryFilter, RelationTrait};
use sea_orm::{QuerySelect, entity::prelude::Uuid};

#[get("")]
pub async fn get_instances(
    user: UserJwtGuard,
    db: WebDb,
    query_params: Query<QueryParams>,
    request: HttpRequest,
) -> UniResult<Vec<instances::Model>> {
    // challenge no hidden
    let user = user.into_inner();
    let mut query_params = query_params.0;

    let stmt = Instances::find()
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

#[get("/{id}")]
pub async fn get_instance(
    _user: UserJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
    request: HttpRequest,
) -> UniResult<instances::Model> {
    let user_id = request
        .extensions()
        .get::<Uuid>()
        .ok_or_else(|| UniError::InternalError("can't parse the Uuid from jwt".to_string()))?
        .to_owned();

    let mut model = Instances::find_by_id(*id)
        .filter(instances::Column::UserId.eq(user_id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    model.flag.clear();

    UniResponse::ok(model.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LaunchInstanceRequest {
    event_id: Option<Uuid>,
    challenge_id: Uuid,
    // for team
}

#[post("/launch")]
pub async fn launch_instance(
    user: UserJwtGuard,
    db: WebDb,
    docker: WebDocker,
    lir: Json<LaunchInstanceRequest>,
    request: HttpRequest,
) -> UniResult<instances::Model> {
    let user = user.into_inner();
    let lir = lir.into_inner();

    // practice

    match lir.event_id {
        Some(event_id) => {
            let event = Events::find_by_id(event_id)
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound("no event".into()))?;

            //  guard for end
            let now = Utc::now().naive_utc();
            if now >= event.end_time {
                return Err(UniError::CustomError("Event has already ended".to_string()));
            }

            match event.r#type {
                EventType::JeopardySingle => {
                    return jeopardy_event_single_launch(db, docker, user, lir).await;
                }
                EventType::JeopardyTeam => {
                    return jeopardy_event_team_launch(db, docker, user, lir).await;
                }
                _ => return UniError::InternalError("unimplemented!".into()).into(),
            }
        }
        None => {
            return jeopardy_single_practice_launch(db, docker, user, lir).await;
        }
    }
}

#[delete("/{id}")]
pub async fn destroy_instance(
    user: UserJwtGuard,
    db: WebDb,
    docker: WebDocker,
    id: Path<Uuid>,
    request: HttpRequest,
) -> UniResult<u64> {
    let user = user.into_inner();
    __destroy_instance(db, docker, *id, user).await
}

pub async fn __destroy_instance(
    db: WebDb,
    docker: WebDocker,
    id: Uuid,
    user: users::Model,
) -> UniResult<u64> {
    let running_instance = Instances::find_by_id(id)
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .filter(instances::Column::UserId.eq(user.id))
        .one(db.get_ref())
        .await?;

    if let Some(instance) = running_instance {
        let challenge = instance
            .find_related(Challenges)
            .one(db.get_ref())
            .await?
            .ok_or_else(|| UniError::NotFound("challenge not found?".to_string()))?;

        let cm = ChallengeMeta::from_toml_str(&challenge.toml_str)
            .map_err(|e| UniError::InternalError(format!("destroy the instance: {}", e)))?;

        //  no docker
        if cm.docker.is_some() {
            cm.stop_and_remove(docker.get_ref(), &instance.identifier)
                .await
                .map_err(|e| UniError::InternalError(format!("destroy the instance: {}", e)))?;
        }

        let mut m_instance = instance.into_active_model();
        m_instance.status = Set(InstanceStatus::Completed);
        m_instance.updated_at = Set(Utc::now().naive_utc());
        m_instance.update(db.get_ref()).await?;
    }

    UniResponse::ok(1.into()).into()
}

pub async fn jeopardy_single_practice_launch(
    db: WebDb,
    docker: WebDocker,
    user: users::Model,
    lir: LaunchInstanceRequest,
) -> UniResult<instances::Model> {
    let running_instances_count = Instances::find()
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
    if let Some(running_instance) = Instances::find()
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .filter(instances::Column::ChallengeId.eq(lir.challenge_id))
        .filter(instances::Column::UserId.eq(user.id))
        .one(db.get_ref())
        .await?
    {
        return UniResponse::ok(running_instance.into()).into();
    }

    // 调用公共函数启动实例
    let identifier = format!("{}_{}", user.id, lir.challenge_id);
    let res_instance = launch_instance_common(
        &db,
        &docker,
        lir.challenge_id,
        identifier,
        user.id,
        "Training".into(),
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
) -> UniResult<instances::Model> {
    let event_id = lir.event_id.unwrap();
    let challenge_id = lir.challenge_id;

    let running_instances_count = EventInstances::find()
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
    if let Some((_, Some(instance))) = EventInstances::find()
        .filter(event_instances::Column::EventId.eq(event_id))
        .filter(event_instances::Column::ChallengeId.eq(challenge_id))
        .filter(event_instances::Column::UserId.eq(user.id))
        .find_also_related(Instances)
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .one(db.get_ref())
        .await?
    {
        return UniResponse::ok(instance.into()).into();
    }

    // 调用公共启动逻辑
    let identifier = format!("{}_{}_{}", event_id, user.id, challenge_id);

    let res_instance = launch_instance_common(
        &db,
        &docker,
        challenge_id,
        identifier,
        user.id,
        "JeopardySingle".into(),
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
) -> UniResult<instances::Model> {
    let event_id = lir.event_id.unwrap();
    let challenge_id = lir.challenge_id;

    let (team_id, team_member_count) = {
        let team_member = EventTeamMembers::find()
            .filter(event_team_members::Column::EventId.eq(event_id))
            .filter(event_team_members::Column::UserId.eq(user.id))
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound("you are not in any team".into()))?;

        let team_member_count = EventTeamMembers::find()
            .filter(event_team_members::Column::TeamId.eq(team_member.team_id))
            .count(db.get_ref())
            .await?;

        (team_member.team_id, team_member_count)
    };

    // team_members * 2
    let running_instances_count = EventInstances::find()
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

    let running_instance = EventInstances::find()
        .filter(event_instances::Column::EventId.eq(event_id))
        .filter(event_instances::Column::ChallengeId.eq(challenge_id))
        .filter(event_instances::Column::TeamId.eq(team_id))
        .find_also_related(Instances)
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .one(db.get_ref())
        .await?;

    if let Some((_, Some(instance))) = running_instance {
        return UniResponse::ok(instance.into()).into();
    }

    let identifier = format!("{}_{}_{}", event_id, team_id, challenge_id);

    let res_instance = launch_instance_common(
        &db,
        &docker,
        challenge_id,
        identifier,
        user.id,
        "JeopardyTeam".into(),
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
) -> anyhow::Result<instances::Model> {
    // challenge 查询 & meta 解析
    let challenge = Challenges::find_by_id(challenge_id)
        .one(db.get_ref())
        .await?
        .ok_or_else(|| anyhow!("no such challenge: {}", challenge_id))?;

    let cm = ChallengeMeta::from_toml_str(&challenge.toml_str)
        .context("failed to parse challenge toml_str")?;

    let flag = if cm.flag.value.is_empty() {
        gen_flag()
    } else {
        cm.flag.value.clone()
    };

    let node_ip = get_setting(&db, "NODE_IP").await?;
    let http_prefix = get_setting(&db, "HTTP_PREFIX").await?;

    //  这里的逻辑是 如果是web 就返回url, 如果是pwn 就返回nc, 如果是misc 就返回description

    //  是根据 有无docker 来判断 而不仅仅是类型, 比如AI题目我可能暂时放到 Misc里
    let content = match cm.category.as_str() {
        "Web" => {
            let port = cm
                .create_and_start(docker, &identifier, &flag)
                .await
                .map_err(|e| UniError::InternalError(format!("{}", e)))?;

            let url = format!("{}{}:{}", http_prefix, node_ip, port);
            format!(
                "<a href=\"{url}\" target=\"_blank\" rel=\"noopener noreferrer\" download >{url}</a>",
            )
        }
        "Pwn" => {
            let port = cm
                .create_and_start(docker, &identifier, &flag)
                .await
                .with_context(|| format!("failed to start Pwn instance for {}", challenge_id))?;
            format!("nc {} {}", node_ip, port)
        }
        // "Misc" => "".to_string(),
        // "Crypto" => "".to_string(),
        // "Reverse" => "".to_string(),
        _ => {
            if cm.docker.is_some() {
                let port = cm
                    .create_and_start(docker, &identifier, &flag)
                    .await
                    .with_context(|| format!("failed to start instance for {}", challenge_id))?;

                format!("try nc or http<br />{} {}", node_ip, port)
            } else {
                cm.description
            }
        }
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
    let d_user = Users::find_by_id(user_id).one(db.get_ref()).await?.unwrap();

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
pub fn gen_flag() -> String {
    let unique_flag = Uuid::new_v4();
    format!("flag{{{}}}", unique_flag)
}
