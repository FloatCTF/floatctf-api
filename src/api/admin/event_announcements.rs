use std::str::FromStr;

use sea_orm::Condition;

use crate::{
    api::{
        FilterMapping, admin::dto::DeleteItemsRequest, apply_filters, prelude::*,
        sea_orm_utils::paginate_query,
    },
    entity::{event_announcements, events},
    prelude::*,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct AddEventAnnouncementRequest {
    pub title: String,
    pub content: String,
}

/// POST /api/admin/events/{event_id}/announcements
#[post("")]
pub async fn add_event_announcement(
    ctx: ReqCtx,
    event_id: Path<Uuid>,
    atr: Json<AddEventAnnouncementRequest>,
) -> UniResult<event_announcements::Model> {
    let atr = atr.into_inner();
    let event_id = event_id.into_inner();

    let event = events::Entity::find_by_id(event_id)
        .one(ctx.db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", event_id)))?;

    let new_event_announcement = event_announcements::ActiveModel {
        event_id: Set(event.id),
        title: Set(atr.title),
        content: Set(atr.content),
        ..Default::default()
    };

    let event_announcement = new_event_announcement.insert(ctx.db.get_ref()).await?;

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
    ctx: ReqCtx,
    path: Path<(Uuid, Uuid)>,
    atr: Json<PatchEventAnnouncementRequest>,
) -> UniResult<event_announcements::Model> {
    let (event_id, announcement_id) = path.into_inner();
    let atr = atr.into_inner();

    let mut event_announcement = event_announcements::Entity::find_by_id(announcement_id)
        .filter(event_announcements::Column::EventId.eq(event_id))
        .one(ctx.db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", event_id)))?
        .into_active_model();
    atr.title.map(|title| event_announcement.title = Set(title));
    atr.content
        .map(|content| event_announcement.content = Set(content));

    let event_announcement = event_announcement.update(ctx.db.get_ref()).await?;

    UniResponse::ok(event_announcement.into()).into()
}

/// DELETE /api/admin/events/{event_id}/announcements
#[delete("")]
pub async fn remove_event_announcement(
    ctx: ReqCtx,
    path: Path<Uuid>,
    dir: Json<DeleteItemsRequest>,
) -> UniResult<u64> {
    let event_id = path.into_inner();
    let dir = dir.into_inner();

    let deleted_count = event_announcements::Entity::delete_many()
        .filter(event_announcements::Column::EventId.eq(event_id))
        .filter(event_announcements::Column::Id.is_in(dir.id_list))
        .exec(ctx.db.get_ref())
        .await?;

    UniResponse::ok(deleted_count.rows_affected.into()).into()
}

/// GET /api/admin/events/{event_id}/announcements/{announcement_id}
#[get("/{announcement_id}")]
pub async fn get_event_announcement(
    ctx: ReqCtx,
    path: Path<(Uuid, Uuid)>,
) -> UniResult<event_announcements::Model> {
    let (event_id, announcement_id) = path.into_inner();

    let event_announcement = event_announcements::Entity::find_by_id(announcement_id)
        .filter(event_announcements::Column::EventId.eq(event_id))
        .one(ctx.db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", event_id)))?;

    UniResponse::ok(event_announcement.into()).into()
}

/// GET /api/admin/events/{event_id}/announcements
#[get("")]
pub async fn list_event_announcements(
    _user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    event_id: Path<Uuid>,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<event_announcements::Model>> {
    let event_id = event_id.into_inner();
    let mut query_params = query_params.0;

    let mappings = [
        FilterMapping {
            key: "id",
            column: Box::new(|v| {
                Condition::all().add(
                    event_announcements::Column::Id.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())),
                )
            }),
        },
        FilterMapping {
            key: "title",
            column: Box::new(|v| {
                Condition::all().add(event_announcements::Column::Title.contains(v))
            }),
        },
        FilterMapping {
            key: "content",
            column: Box::new(|v| {
                Condition::all().add(event_announcements::Column::Content.contains(v))
            }),
        },
    ];

    let stmt = event_announcements::Entity::find()
        .filter(event_announcements::Column::EventId.eq(event_id));
    let stmt = apply_filters(stmt, query_params.filter.clone(), &mappings);

    let (items, total_items) =
        if let (Some(limit), Some(page)) = (query_params.limit, query_params.page) {
            paginate_query(stmt, ctx.db.get_ref(), limit, page).await?
        } else {
            let items = stmt.all(ctx.db.get_ref()).await?;
            (items.clone(), items.len())
        };

    query_params.total = Some(total_items);

    UniResponse::ok_meta(items.into(), query_params.into()).into()
}
