use crate::{
    api::preclude::*,
    entity::{challenges, instances, sea_orm_active_enums::InstanceStatus},
};

/// GET /api/challenges
#[get("")]
pub async fn get_challenges(
    _user: UserJwtGuard,
    db: WebDb,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<challenges::Model>> {
    let mut query_params = query_params.0;

    let stmt = challenges::Entity::find().filter(challenges::Column::Hidden.eq(false));

    if let (Some(limit), Some(page)) = (query_params.limit, query_params.page) {
        let paginator = stmt.paginate(db.get_ref(), limit);
        let items = paginator.fetch_page(page.saturating_sub(1)).await?;
        query_params.total = Some(paginator.num_items().await? as usize);

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    } else {
        let items = stmt.all(db.get_ref()).await?;
        query_params.total = Some(items.len());

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    }
}

/// GET /api/challenges/{challenge_id}
#[get("/{challenge_id}")]
pub async fn get_challenge(
    _user: UserJwtGuard,
    db: WebDb,
    challenge_id: Path<Uuid>,
) -> UniResult<challenges::Model> {
    let challenge_id = challenge_id.into_inner();
    match challenges::Entity::find_by_id(challenge_id)
        .filter(challenges::Column::Hidden.eq(false))
        .one(db.get_ref())
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
    db: WebDb,
    challenge_id: Path<Uuid>,
) -> UniResult<instances::Model> {
    let user = user.into_inner();
    let challenge_id = challenge_id.into_inner();

    let instance = instances::Entity::find()
        .filter(instances::Column::ChallengeId.eq(challenge_id))
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .filter(instances::Column::UserId.eq(user.id))
        .filter(instances::Column::Ref.eq("JeopardyPractice"))
        .one(db.get_ref())
        .await?;

    UniResponse::ok(instance).into()
}
