use sea_orm::{ColumnTrait, QueryFilter};

use super::super::preclude::*;
use crate::{
    auth::UserJwtGuard,
    entity::{challenges, prelude::Challenges},
};

#[get("")]
pub async fn get_challenges(
    _user: UserJwtGuard,
    db: WebDb,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<challenges::Model>> {
    let mut query_params = query_params.0;

    let stmt = Challenges::find().filter(challenges::Column::Hidden.eq(false));

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

#[get("/{id}")]
pub async fn get_challenge(
    _user: UserJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<challenges::Model> {
    match Challenges::find_by_id(*id)
        .filter(challenges::Column::Hidden.eq(false))
        .one(db.get_ref())
        .await?
    {
        Some(model) => UniResponse::ok(model.into()).into(),
        None => UniError::NotFound(format!(" {} not exist", id)).into(),
    }
}

// #[get("/{id}")]
