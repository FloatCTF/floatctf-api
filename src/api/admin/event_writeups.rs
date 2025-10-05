use crate::{api::preclude::*, entity::event_writeup};

/// GET /api/admin/events/{event_id}/writeups
#[get("")]
pub async fn get_all_event_writeups(
    db: WebDb,
    _guard: SuperAdminJwtGuard,
    event_id: Path<Uuid>,
) -> UniResult<Vec<event_writeup::Model>> {
    let event_id = event_id.into_inner();

    let event_writeups = event_writeup::Entity::find()
        .filter(event_writeup::Column::EventId.eq(event_id))
        .all(db.get_ref())
        .await?;

    UniResponse::ok(event_writeups.into()).into()
}
