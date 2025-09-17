use super::super::preclude::*;
use crate::{
    auth::UserJwtGuard,
    db::WebDocker,
    entity::{
        challenges, event_instances, instances,
        prelude::{Challenges, EventInstances, Instances, Users},
        sea_orm_active_enums::InstanceStatus,
        users,
    },
};
use actix_web::{HttpMessage, HttpRequest, delete};
use anyhow::{Context, Result, anyhow};
use fcmc::ChallengeMeta;
use sea_orm::{ColumnTrait, JoinType, ModelTrait, QueryFilter};
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
    team_id: Option<Uuid>,
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
    let lir = lir.into_inner();

    let user = user.into_inner();

    // 题目是否可见
    // 每个人能启动的最大实例数为1
    let running_instances_count = Instances::find()
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .filter(instances::Column::UserId.eq(user.id))
        .count(db.get_ref())
        .await?;

    let max_instances_per_user = std::env::var("INSTANCE_MAX_PER_USER")
        .unwrap()
        .parse::<u64>()
        .unwrap();

    if running_instances_count >= max_instances_per_user {
        return UniError::CustomError(format!(
            "you can only launch {} instances at the same time",
            max_instances_per_user
        ))
        .into();
    }

    if lir.event_id.is_none() && lir.team_id.is_none() {
        jeopardy_single_practice_launch(db, docker, user, lir).await
    } else if lir.event_id.is_some() && lir.team_id.is_none() {
        jeopardy_event_single_launch(db, docker, user, lir).await
    } else if lir.event_id.is_some() && lir.team_id.is_some() {
        jeopardy_event_team_launch(db, docker, user, lir).await
    } else {
        UniError::InternalError("unimplemented!".into()).into()
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
    let identifier = user.id.to_string();
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
    let identifier = format!("{}_{}", event_id, user.id);

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
    unimplemented!()
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

    let node_ip = std::env::var("NODE_IP").unwrap();
    let http_prefix = std::env::var("HTTP_PREFIX").unwrap();

    let content = match cm.category.as_str() {
        "Web" => {
            let port = cm
                .create_and_start(docker, &identifier, &flag)
                .await
                .with_context(|| format!("failed to start Web instance for {}", challenge_id))?;
            format!("{}{}:{}", http_prefix, node_ip, port)
        }
        "Pwn" => {
            let port = cm
                .create_and_start(docker, &identifier, &flag)
                .await
                .with_context(|| format!("failed to start Pwn instance for {}", challenge_id))?;
            format!("nc {} {}", node_ip, port)
        }
        "Misc" => "".to_string(),
        "Crypto" => "".to_string(),
        "Reverse" => "".to_string(),
        _ => cm.description,
    };

    let delay: i64 = std::env::var("INSTANCE_DESTROY_DELAY")
        .context("missing env INSTANCE_DESTROY_DELAY")?
        .parse()
        .context("invalid INSTANCE_DESTROY_DELAY (must be i64)")?;

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
