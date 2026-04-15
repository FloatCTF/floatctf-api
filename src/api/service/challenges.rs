use std::str::FromStr;

use sea_orm::Condition;

use crate::{
    api::{FilterMapping, apply_filters, prelude::*, sea_orm_utils::paginate_query},
    entity::{challenges, instances, sea_orm_active_enums::InstanceStatus},
    prelude::*,
};

/// GET /api/challenges
#[get("")]
pub async fn get_challenges(
    _user: UserJwtGuard,
    ctx: ReqCtx,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<challenges::Model>> {
    let mut query_params = query_params.0;

    let mappings = [
        FilterMapping {
            key: "id",
            column: Box::new(|v| {
                Condition::all()
                    .add(challenges::Column::Id.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())))
            }),
        },
        FilterMapping {
            key: "name",
            column: Box::new(|v| Condition::all().add(challenges::Column::Name.contains(v))),
        },
        FilterMapping {
            key: "category",
            column: Box::new(|v| Condition::all().add(challenges::Column::Category.contains(v))),
        },
        FilterMapping {
            key: "description",
            column: Box::new(|v| Condition::all().add(challenges::Column::Description.contains(v))),
        },
    ];

    let stmt = challenges::Entity::find().filter(challenges::Column::Hidden.eq(false));
    let stmt = apply_filters(stmt, query_params.filter.clone(), &mappings);
    let stmt = stmt.order_by_desc(challenges::Column::UpdatedAt);

    let (items, total_items) =
        if let (Some(limit), Some(page)) = (query_params.limit, query_params.page) {
            paginate_query(stmt, ctx.db.get_ref(), limit, page).await?
        } else {
            let items = stmt.all(ctx.db.get_ref()).await?;
            (items.clone(), items.len())
        };

    query_params.total = Some(total_items);

    UniResponse::ok_meta(items.into(), query_params.into()).into()
}

/// GET /api/challenges/{challenge_id}
#[get("/{challenge_id}")]
pub async fn get_challenge(
    _user: UserJwtGuard,
    ctx: ReqCtx,
    challenge_id: Path<Uuid>,
) -> UniResult<challenges::Model> {
    let challenge_id = challenge_id.into_inner();
    match challenges::Entity::find_by_id(challenge_id)
        .filter(challenges::Column::Hidden.eq(false))
        .one(ctx.db.get_ref())
        .await?
    {
        Some(model) => UniResponse::ok(model.into()).into(),
        None => UniError::NotFound(format!(" {} not exist", challenge_id)).into(),
    }
}

/// GET /api/challenges/{challenge_id}/instance
#[get("/{challenge_id}/instance")]
pub async fn get_challenge_instance(
    user: UserJwtGuard,
    ctx: ReqCtx,
    challenge_id: Path<Uuid>,
) -> UniResult<instances::Model> {
    let user = user.into_inner();
    let challenge_id = challenge_id.into_inner();

    let instance = instances::Entity::find()
        .filter(instances::Column::ChallengeId.eq(challenge_id))
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .filter(instances::Column::UserId.eq(user.id))
        .filter(instances::Column::Ref.eq("JeopardyPractice"))
        .one(ctx.db.get_ref())
        .await?;

    UniResponse::ok(instance).into()
}
