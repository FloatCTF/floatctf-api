use crate::{
    api::{admin::dto::DeleteItemsRequest, preclude::*},
    entity::{event_announcements, events},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct AddEventAnnouncementRequest {
    pub title: String,
    pub content: String,
}

/// POST /api/admin/events/{event_id}/announcements
#[post("")]
pub async fn add_event_announcement(
    db: WebDb,
    event_id: Path<Uuid>,
    atr: Json<AddEventAnnouncementRequest>,
) -> UniResult<event_announcements::Model> {
    let atr = atr.into_inner();
    let event_id = event_id.into_inner();

    let event = events::Entity::find_by_id(event_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", event_id)))?;

    let new_event_announcement = event_announcements::ActiveModel {
        event_id: Set(event.id),
        title: Set(atr.title),
        content: Set(atr.content),
        ..Default::default()
    };

    let event_announcement = new_event_announcement.insert(db.get_ref()).await?;

    UniResponse::ok(event_announcement.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatchEventAnnouncementRequest {
    pub title: Option<String>,
    pub content: Option<String>,
}
/// PATCH /api/admin/events/{event_id}/announcements/{announcement_id}
#[patch("/{announcement_id}")]
pub async fn patch_event_announcement(
    db: WebDb,
    path: Path<(Uuid, Uuid)>,
    atr: Json<PatchEventAnnouncementRequest>,
) -> UniResult<event_announcements::Model> {
    let (event_id, announcement_id) = path.into_inner();
    let atr = atr.into_inner();

    let mut event_announcement = event_announcements::Entity::find_by_id(announcement_id)
        .filter(event_announcements::Column::EventId.eq(event_id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", event_id)))?
        .into_active_model();
    atr.title.map(|title| event_announcement.title = Set(title));
    atr.content
        .map(|content| event_announcement.content = Set(content));

    let event_announcement = event_announcement.update(db.get_ref()).await?;

    UniResponse::ok(event_announcement.into()).into()
}

/// DELETE /api/admin/events/{event_id}/announcements
#[delete("")]
pub async fn remove_event_announcement(
    db: WebDb,
    path: Path<Uuid>,
    dir: Json<DeleteItemsRequest>,
) -> UniResult<u64> {
    let event_id = path.into_inner();
    let dir = dir.into_inner();

    let deleted_count = event_announcements::Entity::delete_many()
        .filter(event_announcements::Column::EventId.eq(event_id))
        .filter(event_announcements::Column::Id.is_in(dir.id_list))
        .exec(db.get_ref())
        .await?;

    UniResponse::ok(deleted_count.rows_affected.into()).into()
}

/// GET /api/admin/events/{event_id}/announcements/{announcement_id}
#[get("/{announcement_id}")]
pub async fn get_event_announcement(
    db: WebDb,
    path: Path<(Uuid, Uuid)>,
) -> UniResult<event_announcements::Model> {
    let (event_id, announcement_id) = path.into_inner();

    let event_announcement = event_announcements::Entity::find_by_id(announcement_id)
        .filter(event_announcements::Column::EventId.eq(event_id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", event_id)))?;

    UniResponse::ok(event_announcement.into()).into()
}

/// GET /api/admin/events/{event_id}/announcements
#[get("")]
pub async fn list_event_announcements(
    db: WebDb,
    event_id: Path<Uuid>,
) -> UniResult<Vec<event_announcements::Model>> {
    let event_id = event_id.into_inner();
    let event_announcements = event_announcements::Entity::find()
        .filter(event_announcements::Column::EventId.eq(event_id))
        .all(db.get_ref())
        .await?;

    UniResponse::ok(event_announcements.into()).into()
}
