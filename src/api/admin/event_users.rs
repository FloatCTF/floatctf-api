use futures_util::future::join_all;
use sea_orm::{ColumnTrait, QueryFilter};

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
    pub user_id_list: Option<Vec<Uuid>>,
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

    if aur.user_id_list.is_some() {
        let user_id_list = aur.user_id_list.unwrap();
        for user_id in user_id_list {
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

    UniError::CustomError("user_id or user_id_list is required".to_string()).into()
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

#[derive(Debug, Serialize, Deserialize)]
pub struct EventUserResult {
    pub user: users::Model,
    pub event_user: event_users::Model,
}

#[get("")]
pub async fn get_users(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<Vec<EventUserResult>> {
    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let event_users = EventUsers::find()
        .filter(event_users::Column::EventId.eq(event.id))
        .all(db.get_ref())
        .await?;

    let mut users = Vec::new();
    for event_user in event_users {
        let user = Users::find_by_id(event_user.user_id)
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(
                " {} not exist",
                event_user.user_id
            )))?;

        users.push(EventUserResult { user, event_user });
    }

    UniResponse::ok(users.into()).into()
}

#[post("/{user_id}/banned")]

pub async fn banned_user(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    path: Path<(Uuid, Uuid)>,
) -> UniResult<()> {
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

    let mut event_user: event_users::ActiveModel = event_user.into();
    event_user.banned = Set(true);
    event_user.update(db.get_ref()).await?;

    UniResponse::ok_none().into()
}

#[post("/{user_id}/unbanned")]
pub async fn unbanned_user(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    path: Path<(Uuid, Uuid)>,
) -> UniResult<()> {
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

    let mut event_user: event_users::ActiveModel = event_user.into();
    event_user.banned = Set(false);
    event_user.update(db.get_ref()).await?;

    UniResponse::ok_none().into()
}
