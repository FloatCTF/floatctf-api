use sea_orm::{ColumnTrait, QueryFilter};

use super::super::preclude::*;
use crate::{
    auth::SuperAdminJwtGuard,
    entity::{
        event_announcements,
        prelude::{EventAnnouncements, Events, Users},
        users,
    },
};

#[derive(Debug, Serialize, Deserialize)]
pub struct AddEventAnnouncementRequest {
    pub title: String,
    pub content: String,
}

#[post("")]
pub async fn add_event_announcement(
    db: WebDb,
    id: Path<Uuid>,
    atr: Json<AddEventAnnouncementRequest>,
) -> UniResult<event_announcements::Model> {
    let atr = atr.into_inner();

    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let new_event_announcement = event_announcements::ActiveModel {
        event_id: Set(event.id),
        title: Set(atr.title),
        content: Set(atr.content),
        ..Default::default()
    };

    let event_announcement = new_event_announcement.insert(db.get_ref()).await?;

    UniResponse::ok(event_announcement.into()).into()
}

pub type PatchEventAnnouncementRequest = AddEventAnnouncementRequest;

#[patch("/{announcement_id}")]
pub async fn update_event_announcement(
    db: WebDb,
    path: Path<(Uuid, Uuid)>,
    atr: Json<PatchEventAnnouncementRequest>,
) -> UniResult<event_announcements::Model> {
    let (id, announcement_id) = path.into_inner();
    let atr = atr.into_inner();

    let mut event_announcement = EventAnnouncements::find_by_id(announcement_id)
        .filter(event_announcements::Column::EventId.eq(id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?
        .into_active_model();

    event_announcement.title = Set(atr.title);
    event_announcement.content = Set(atr.content);
    let event_announcement = event_announcement.update(db.get_ref()).await?;

    UniResponse::ok(event_announcement.into()).into()
}

#[delete("/{announcement_id}")]
pub async fn remove_event_announcement(db: WebDb, path: Path<(Uuid, Uuid)>) -> UniResult<u64> {
    let (id, announcement_id) = path.into_inner();

    let event_announcement = EventAnnouncements::find_by_id(announcement_id)
        .filter(event_announcements::Column::EventId.eq(id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let r = event_announcement.delete(db.get_ref()).await?;

    UniResponse::ok(r.rows_affected.into()).into()
}

#[get("/{announcement_id}")]
pub async fn get_event_announcement(
    db: WebDb,
    path: Path<(Uuid, Uuid)>,
) -> UniResult<event_announcements::Model> {
    let (id, announcement_id) = path.into_inner();

    let event_announcement = EventAnnouncements::find_by_id(announcement_id)
        .filter(event_announcements::Column::EventId.eq(id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    UniResponse::ok(event_announcement.into()).into()
}

#[get("")]
pub async fn list_event_announcements(
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<Vec<event_announcements::Model>> {
    let event_announcements = EventAnnouncements::find()
        .filter(event_announcements::Column::EventId.eq(*id))
        .all(db.get_ref())
        .await?;

    UniResponse::ok(event_announcements.into()).into()
}
