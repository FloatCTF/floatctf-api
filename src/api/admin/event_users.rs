use crate::{
    api::{admin::dto::DeleteItemsRequest, preclude::*},
    entity::{event_users, events, users},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct AddUserRequest {
    pub user_id: Option<Uuid>,
    pub user_id_list: Option<Vec<Uuid>>,
}

/// POST /api/admin/events/{event_id}/users
#[post("")]
pub async fn add_user(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    event_id: Path<Uuid>,
    aur: Json<AddUserRequest>,
) -> UniResult<()> {
    let aur = aur.into_inner();
    let event_id = event_id.into_inner();

    let event = events::Entity::find_by_id(event_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", event_id)))?;

    if aur.user_id_list.is_some() {
        let user_id_list = aur.user_id_list.unwrap();
        for user_id in user_id_list {
            let user = users::Entity::find_by_id(user_id)
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
        let user = users::Entity::find_by_id(user_id)
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

/// DELETE /api/admin/events/{event_id}/users
#[delete("")]
pub async fn remove_user(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    path: Path<Uuid>,
    dir: Json<DeleteItemsRequest>,
) -> UniResult<u64> {
    let event_id = path.into_inner();
    let dir = dir.into_inner();

    let event = events::Entity::find_by_id(event_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", event_id)))?;

    let deleted_count = event_users::Entity::delete_many()
        .filter(event_users::Column::EventId.eq(event.id))
        .filter(event_users::Column::UserId.is_in(dir.id_list))
        .exec(db.get_ref())
        .await?
        .rows_affected;

    UniResponse::ok(deleted_count.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventUserResult {
    pub user: users::Model,
    pub event_user: event_users::Model,
}

/// GET /api/admin/events/{event_id}/users
#[get("")]
pub async fn get_users(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    event_id: Path<Uuid>,
) -> UniResult<Vec<EventUserResult>> {
    let event_id = event_id.into_inner();

    let event = events::Entity::find_by_id(event_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", event_id)))?;

    let event_users = event_users::Entity::find()
        .filter(event_users::Column::EventId.eq(event.id))
        .all(db.get_ref())
        .await?;

    let mut users = Vec::new();
    for event_user in event_users {
        let user = users::Entity::find_by_id(event_user.user_id)
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

/// POST /api/admin/events/{event_id}/users/{user_id}/banned
#[post("/{user_id}/banned")]

pub async fn banned_user(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    path: Path<(Uuid, Uuid)>,
) -> UniResult<()> {
    let (event_id, user_id) = path.into_inner();

    let event = events::Entity::find_by_id(event_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", event_id)))?;

    let user = users::Entity::find_by_id(user_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", user_id)))?;

    let event_user = event_users::Entity::find_by_id((event.id, user.id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(
            " {} not exist in {}",
            user_id, event_id
        )))?;

    let mut event_user: event_users::ActiveModel = event_user.into();
    event_user.banned = Set(true);
    event_user.update(db.get_ref()).await?;

    UniResponse::ok_none().into()
}

/// POST /api/admin/events/{event_id}/users/{user_id}/unbanned
#[post("/{user_id}/unbanned")]
pub async fn unbanned_user(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    path: Path<(Uuid, Uuid)>,
) -> UniResult<()> {
    let (event_id, user_id) = path.into_inner();

    let event = events::Entity::find_by_id(event_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", event_id)))?;

    let user = users::Entity::find_by_id(user_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", user_id)))?;

    let event_user = event_users::Entity::find_by_id((event.id, user.id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(
            " {} not exist in {}",
            user_id, event_id
        )))?;

    let mut event_user: event_users::ActiveModel = event_user.into();
    event_user.banned = Set(false);
    event_user.update(db.get_ref()).await?;

    UniResponse::ok_none().into()
}
