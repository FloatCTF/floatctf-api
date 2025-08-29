use super::super::preclude::*;
use crate::{
    auth::UserJwtGuard,
    db::WebDocker,
    entity::{
        challenges, instances,
        prelude::{Challenges, Instances},
        sea_orm_active_enums::InstanceStatus,
        users,
    },
};
use actix_web::{HttpMessage, HttpRequest, delete};
use cm::ChallengeMeta;
use sea_orm::entity::prelude::Uuid;
use sea_orm::{ColumnTrait, ModelTrait, QueryFilter};

#[get("")]
pub async fn get_instances(
    _user: UserJwtGuard,
    db: WebDb,
    query_params: Query<QueryParams>,
    request: HttpRequest,
) -> UniResult<Vec<instances::Model>> {
    let mut query_params = query_params.0;

    let user_id = request
        .extensions()
        .get::<Uuid>()
        .ok_or_else(|| UniError::InternalError("can't parse the Uuid from jwt".to_string()))?
        .to_owned();
    let stmt = Instances::find()
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .filter(instances::Column::UserId.eq(user_id));

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
    if running_instances_count != 0 {
        return UniError::CustomError("each user's max instances to 1".to_string()).into();
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
) -> UniResult<()> {
    let user = user.into_inner();

    __destroy_instance(db, docker, *id, user).await
}

pub async fn __destroy_instance(
    db: WebDb,
    docker: WebDocker,
    id: Uuid,
    user: users::Model,
) -> UniResult<()> {
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

        cm.stop_and_remove(docker.get_ref(), &user.id.to_string())
            .await
            .map_err(|e| UniError::InternalError(format!("destroy the instance: {}", e)))?;

        let mut m_instance = instance.into_active_model();
        m_instance.status = Set(InstanceStatus::Completed);
        m_instance.updated_at = Set(Utc::now().naive_utc());
        m_instance.update(db.get_ref()).await?;
    }

    UniResponse::ok_none().into()
}

pub async fn jeopardy_single_practice_launch(
    db: WebDb,
    docker: WebDocker,
    user: users::Model,
    lir: LaunchInstanceRequest,
) -> UniResult<instances::Model> {
    // 是否已经开启，开启了返回已经存在的
    let running_instance = Instances::find()
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .filter(instances::Column::ChallengeId.eq(lir.challenge_id))
        .filter(instances::Column::UserId.eq(user.id))
        .one(db.get_ref())
        .await?;

    if running_instance.is_some() {
        return UniResponse::ok(running_instance).into();
    }

    let challenge = Challenges::find_by_id(lir.challenge_id)
        .filter(challenges::Column::Hidden.eq(false))
        .one(db.get_ref())
        .await?
        .ok_or_else(|| UniError::NotFound(format!("has no id : {} challenge", lir.challenge_id)))?;

    // launch the instance
    let cm = ChallengeMeta::from_toml_str(&challenge.toml_str)
        .map_err(|e| UniError::CustomError(e.to_string()))?;

    let flag = if cm.flag.value.is_empty() {
        gen_flag()
    } else {
        cm.flag.value.clone()
    };

    let content = {
        if cm.category == "Web" {
            let port = cm
                .create_and_start(docker.get_ref(), &user.id.to_string(), &flag)
                .await
                .map_err(|e| {
                    UniError::InternalError(format!("failed to start instance: {}", e.to_string()))
                })?;
            // TODO add attachment here
            format!("http://127.0.0.1:{}", port)
        } else if cm.category == "Pwn" {
            let port = cm
                .create_and_start(docker.get_ref(), &user.id.to_string(), &flag)
                .await
                .map_err(|e| {
                    UniError::InternalError(format!("failed to start instance: {}", e.to_string()))
                })?;

            format!("nc 127.0.0.1 {}", port)
        } else {
            cm.description
        }
    };

    let new_instance = instances::ActiveModel {
        status: Set(InstanceStatus::Running),
        flag: Set(flag),
        content: Set(content.into()),
        challenge_id: Set(challenge.id),
        user_id: Set(user.id),
        ..Default::default()
    };

    let mut res_instance = new_instance.insert(db.get_ref()).await?;
    res_instance.flag.clear();

    UniResponse::ok(res_instance.into()).into()
}

pub async fn jeopardy_event_single_launch(
    db: WebDb,
    docker: WebDocker,
    user: users::Model,
    lir: LaunchInstanceRequest,
) -> UniResult<instances::Model> {
    unimplemented!()
}

pub async fn jeopardy_event_team_launch(
    db: WebDb,
    docker: WebDocker,
    user: users::Model,
    lir: LaunchInstanceRequest,
) -> UniResult<instances::Model> {
    unimplemented!()
}
pub fn gen_flag() -> String {
    let unique_flag = Uuid::new_v4();
    format!("flag{{{}}}", unique_flag)
}
