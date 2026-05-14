use std::str::FromStr;

use sea_orm::Condition;

use crate::{
    api::{FilterMapping, apply_filters, prelude::*, sea_orm_utils::paginate_query},
    entity::{events, instances, sea_orm_active_enums::InstanceStatus},
    prelude::*,
    strategies::event,
};

/// GET /api/instances
#[get("")]
pub async fn get_instances(
    user: UserJwtGuard,
    ctx: ReqCtx,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<instances::Model>> {
    let user = user.into_inner();
    let mut query_params = query_params.0;

    let mappings = [
        FilterMapping {
            key: "id",
            column: Box::new(|v| {
                Condition::all()
                    .add(instances::Column::Id.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())))
            }),
        },
        FilterMapping {
            key: "status",
            column: Box::new(|v| {
                Condition::all().add(
                    instances::Column::Status
                        .eq(serde_json::from_str(v).unwrap_or(InstanceStatus::Running)),
                )
            }),
        },
        FilterMapping {
            key: "ref",
            column: Box::new(|v| Condition::all().add(instances::Column::Ref.contains(v))),
        },
        FilterMapping {
            key: "challenge_id",
            column: Box::new(|v| {
                Condition::all().add(
                    instances::Column::ChallengeId.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())),
                )
            }),
        },
        FilterMapping {
            key: "gamebox_id",
            column: Box::new(|v| {
                Condition::all()
                    .add(instances::Column::GameboxId.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())))
            }),
        },
    ];

    let stmt = instances::Entity::find()
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .filter(instances::Column::Ref.eq("JeopardyPractice"))
        .filter(instances::Column::UserId.eq(user.id));
    let stmt = apply_filters(stmt, query_params.filter.clone(), &mappings);
    let stmt = stmt.order_by_desc(instances::Column::UpdatedAt);

    let (mut items, total_items) =
        if let (Some(limit), Some(page)) = (query_params.limit, query_params.page) {
            paginate_query(stmt, ctx.db.get_ref(), limit, page).await?
        } else {
            let items = stmt.all(ctx.db.get_ref()).await?;
            (items.clone(), items.len())
        };

    for item in &mut items {
        item.flag.clear();
    }

    query_params.total = Some(total_items);

    UniResponse::ok_meta(items.into(), query_params.into()).into()
}

/// GET /api/instances/{instance_id}
#[get("/{instance_id}")]
pub async fn get_instance(
    user: UserJwtGuard,
    ctx: ReqCtx,
    instance_id: Path<Uuid>,
) -> UniResult<instances::Model> {
    let instance_id = instance_id.into_inner();
    let user = user.into_inner();

    let mut model = instances::Entity::find_by_id(instance_id)
        .filter(instances::Column::UserId.eq(user.id))
        .one(ctx.db.get_ref())
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
    ctx: ReqCtx,
    lir: Json<LaunchInstanceRequest>,
) -> UniResult<instances::Model> {
    let user = user.into_inner();
    let lir = lir.into_inner();

    let event = match lir.event_id {
        Some(event_id) => events::Entity::find_by_id(event_id)
            .one(ctx.db.get_ref())
            .await?
            .ok_or(UniError::NotFound("no event".into()))?
            .into(),
        None => None,
    };

    let event_ctx = event::EventContextBuilder::new()
        .db(ctx.db.clone())
        .docker(ctx.docker.clone())
        .user(user.clone())
        .event(event)
        .build()
        .await
        .map_err(|e| UniError::CustomError(format!("build event context error: {}", e)))?;

    let strategy = event::EventStrategyFactory::create(&event_ctx.event.r#type);

    let instance = strategy
        .launch_instance(&event_ctx, lir.challenge_id)
        .await
        .map_err(|e| UniError::CustomError(format!("when launch instance:{}", e)))?;

    ctx.log
        .add_log(
            "INFO",
            "INSTANCE",
            "LAUNCH",
            format!("启动题目 {} 的实例", lir.challenge_id).as_str(),
            json!({"event_id": lir.event_id}),
            user.id.into(),
            None,
            Some(&ctx.req),
        )
        .await;

    UniResponse::ok(instance.into()).into()
}

/// DELETE /api/instances/{instance_id}
#[delete("/{instance_id}")]
pub async fn destroy_instance(
    user: UserJwtGuard,
    ctx: ReqCtx,
    instance_id: Path<Uuid>,
) -> UniResult<()> {
    let user = user.into_inner();
    let instance_id = instance_id.into_inner();
    event::common::destroy_instance(&ctx.db, &ctx.docker, instance_id, &user)
        .await
        .map_err(|e| UniError::CustomError(format!("destroy_instance:{}", e)))?;

    ctx.log
        .add_log(
            "INFO",
            "INSTANCE",
            "DESTROY",
            format!("销毁实例 {}", instance_id).as_str(),
            json!({}),
            user.id.into(),
            None,
            Some(&ctx.req),
        )
        .await;

    UniResponse::ok_none().into()
}
