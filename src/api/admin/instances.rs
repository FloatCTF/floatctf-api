use super::super::preclude::*;
use crate::{
    auth::SuperAdminJwtGuard,
    entity::{instances, prelude::Instances},
};

#[get("")]
pub async fn get_instances(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<instances::Model>> {
    let mut query_params = query_params.0;

    let stmt = Instances::find();

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
pub async fn get_instance(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<instances::Model> {
    let model = Instances::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    UniResponse::ok(model.into()).into()
}
