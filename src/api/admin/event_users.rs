use futures_util::future::join_all;

use super::super::preclude::*;
use crate::{
    auth::SuperAdminJwtGuard,
    entity::{
        event_users,
        prelude::{EventUsers, Events, Users},
        users,
    },
};

#[derive(Debug, Serialize, Deserialize)]
pub struct AddUserRequest {
    pub user_id: Option<Uuid>,
    pub user_ids: Option<Vec<Uuid>>,
}

#[post("")]
pub async fn add_user(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
    aur: Json<AddUserRequest>,
) -> UniResult<()> {
    let aur = aur.into_inner();

    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    if aur.user_ids.is_some() {
        let user_ids = aur.user_ids.unwrap();
        for user_id in user_ids {
            let user = Users::find_by_id(user_id)
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound(format!(" {} not exist", user_id)))?;

            let new_event_user = event_users::ActiveModel {
                event_id: Set(event.id),
                user_id: Set(user.id),
                ..Default::default()
            };

            new_event_user.insert(db.get_ref()).await?;
        }

        return UniResponse::ok_none().into();
    }
    if aur.user_id.is_some() {
        let user_id = aur.user_id.unwrap();
        let user = Users::find_by_id(user_id)
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(" {} not exist", user_id)))?;

        let new_event_user = event_users::ActiveModel {
            event_id: Set(event.id),
            user_id: Set(user.id),
            ..Default::default()
        };

        new_event_user.insert(db.get_ref()).await?;

        return UniResponse::ok_none().into();
    }

    UniError::CustomError("user_id or user_ids is required".to_string()).into()
}

#[delete("/{user_id}")]
pub async fn remove_user(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    path: Path<(Uuid, Uuid)>,
) -> UniResult<u64> {
    let (id, user_id) = path.into_inner();

    let event = Events::find_by_id(id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let user = Users::find_by_id(user_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", user_id)))?;

    let event_user = EventUsers::find_by_id((event.id, user.id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(
            " {} not exist in {}",
            user_id, id
        )))?;

    let r = event_user.delete(db.get_ref()).await?;

    UniResponse::ok(r.rows_affected.into()).into()
}

#[get("")]
pub async fn get_users(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<users::Model>> {
    let mut query_params = query_params.0;

    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let stmt = event.find_related(EventUsers);

    if let (Some(limit), Some(page)) = (query_params.limit, query_params.page) {
        let paginator = stmt.paginate(db.get_ref(), limit);
        let items: Vec<event_users::Model> = paginator.fetch_page(page.saturating_sub(1)).await?;

        // 构造所有 Future
        let futures_vec = items
            .iter()
            .map(|eu| Users::find_by_id(eu.user_id).one(db.get_ref()));

        // 等待所有完成，结果是 Vec<Option<users::Model>>
        let results = join_all(futures_vec).await;

        // 过滤 None，收集 Model
        let items: Vec<users::Model> = results
            .into_iter()
            .filter_map(|x| x.ok().flatten())
            .collect();

        query_params.total = Some(paginator.num_items().await? as usize);

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    } else {
        let items: Vec<event_users::Model> = stmt.all(db.get_ref()).await?;
        // 构造所有 Future
        let futures_vec = items
            .iter()
            .map(|eu| Users::find_by_id(eu.user_id).one(db.get_ref()));

        // 等待所有完成，结果是 Vec<Option<users::Model>>
        let results = join_all(futures_vec).await;

        // 过滤 None，收集 Model
        let items: Vec<users::Model> = results
            .into_iter()
            .filter_map(|x| x.ok().flatten())
            .collect();

        query_params.total = Some(items.len());

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    }

    // UniResponse::ok(users.into()).into()
}
