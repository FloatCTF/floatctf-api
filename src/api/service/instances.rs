use crate::{
    api::preclude::*,
    entity::{events, instances, sea_orm_active_enums::InstanceStatus},
    strategies::event,
};
use actix_web::HttpRequest; // TODO : for log
use anyhow::{Context, anyhow};

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
        .filter(instances::Column::Ref.eq("JeopardyPractice"))
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

    let event = match lir.event_id {
        Some(event_id) => events::Entity::find_by_id(event_id)
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound("no event".into()))?
            .into(),
        None => None,
    };

    let ctx = event::EventContextBuilder::new()
        .db(db.clone())
        .docker(docker.clone())
        .user(user.clone())
        .event(event)
        .build()
        .await
        .map_err(|e| UniError::CustomError(format!("build event context error: {}", e)))?;

    let strategy = event::EventStrategyFactory::create(&ctx.event.r#type);

    let instance = strategy
        .launch_instance(&ctx, lir.challenge_id)
        .await
        .map_err(|e| UniError::CustomError(format!("when launch instance:{}", e)))?;

    UniResponse::ok(instance.into()).into()
}

/// DELETE /api/instances/{instance_id}
#[delete("/{instance_id}")]
pub async fn destroy_instance(
    user: UserJwtGuard,
    db: WebDb,
    docker: WebDocker,
    instance_id: Path<Uuid>,
) -> UniResult<()> {
    let user = user.into_inner();
    let instance_id = instance_id.into_inner();
    event::common::destroy_instance(&db, &docker, instance_id, &user)
        .await
        .map_err(|e| UniError::CustomError(format!("destroy_instance:{}", e)))?;

    UniResponse::ok_none().into()
}
