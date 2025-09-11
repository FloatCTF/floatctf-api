use super::super::preclude::*;
use crate::{
    auth::UserJwtGuard,
    db::WebDocker,
    entity::{
        challenge_solves, challenges, instances,
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
pub async fn get_solves(
    user: UserJwtGuard,
    db: WebDb,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<challenge_solves::Model>> {
    let user = user.into_inner();
    let mut query_params = query_params.0;

    let stmt =
        challenge_solves::Entity::find().filter(challenge_solves::Column::UserId.eq(user.id));

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
