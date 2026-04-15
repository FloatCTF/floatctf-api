use crate::{
    api::prelude::*,
    entity::{challenge_set_items, challenge_sets, challenges},
    prelude::*,
};

/// GET /api/challenge_sets
#[get("")]
pub async fn get_challenge_sets(
    _user: UserJwtGuard,
    ctx: ReqCtx,
) -> UniResult<Vec<challenge_sets::Model>> {
    let challenge_sets = challenge_sets::Entity::find()
        .order_by_desc(challenge_sets::Column::CreatedAt)
        .all(ctx.db.get_ref())
        .await?;
    UniResponse::ok(challenge_sets.into()).into()
}

/// GET /api/challenge_sets/{challenge_set_id}
#[get("/{challenge_set_id}")]
pub async fn get_challenge_set(
    _user: UserJwtGuard,
    ctx: ReqCtx,
    challenge_set_id: Path<Uuid>,
) -> UniResult<Vec<challenges::Model>> {
    let challenge_set_id = challenge_set_id.into_inner();
    let challenges = challenges::Entity::find()
        // 只查在该 set 里的 challenge
        .join_rev(
            sea_orm::JoinType::InnerJoin,
            challenge_set_items::Entity::belongs_to(challenges::Entity)
                .from(challenge_set_items::Column::ChallengeId)
                .to(challenges::Column::Id)
                .into(),
        )
        .filter(challenge_set_items::Column::SetId.eq(challenge_set_id))
        // 只查未隐藏的
        .filter(challenges::Column::Hidden.eq(false))
        .all(ctx.db.get_ref())
        .await?;

    UniResponse::ok(challenges.into()).into()
}
