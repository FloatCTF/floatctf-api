use super::super::preclude::*;
use crate::{
    auth::SuperAdminJwtGuard,
    entity::{events, prelude::Events, sea_orm_active_enums::EventType},
};
use chrono::NaiveDateTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateEventRequest {
    pub r#type: EventType,
    pub title: String,
    pub description: Option<String>,
    pub hidden: bool,
    pub start_time: NaiveDateTime,
    pub end_time: NaiveDateTime,
}

#[post("")]
pub async fn create_event(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    cer: Json<CreateEventRequest>,
) -> UniResult<events::Model> {
    let cer = cer.into_inner();

    let new_event = events::ActiveModel {
        r#type: Set(cer.r#type),
        title: Set(cer.title),
        description: Set(cer.description),
        start_time: Set(cer.start_time),
        hidden: Set(cer.hidden),
        end_time: Set(cer.end_time),
        ..Default::default()
    };

    let event = new_event.insert(db.get_ref()).await?;

    UniResponse::ok(event.into()).into()
}

type UpdateEventRequest = CreateEventRequest;
#[put("/{id}")]
pub async fn update_event(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    uer: Json<UpdateEventRequest>,
    id: Path<Uuid>,
) -> UniResult<events::Model> {
    let uer = uer.into_inner();

    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let mut m_event = event.into_active_model();

    m_event.r#type = Set(uer.r#type);
    m_event.title = Set(uer.title);
    m_event.description = Set(uer.description);
    m_event.start_time = Set(uer.start_time);
    m_event.end_time = Set(uer.end_time);

    let event = m_event.update(db.get_ref()).await?;

    UniResponse::ok(event.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatchEventRequest {
    pub r#type: Option<EventType>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub hidden: Option<bool>,
    pub start_time: Option<NaiveDateTime>,
    pub end_time: Option<NaiveDateTime>,
}
#[patch("/{id}")]
pub async fn patch_event(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    per: Json<PatchEventRequest>,
    id: Path<Uuid>,
) -> UniResult<events::Model> {
    let per = per.into_inner();
    dbg!(&per);
    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;
    let mut m_event = event.into_active_model();

    per.r#type.map(|t| m_event.r#type = Set(t));

    per.title.map(|t| {
        m_event.title = Set(t);
    });

    per.description.map(|d| {
        m_event.description = Set(d.into());
    });

    per.start_time.map(|s| {
        m_event.start_time = Set(s);
    });

    per.end_time.map(|e| {
        m_event.end_time = Set(e);
    });

    per.hidden.map(|h| {
        m_event.hidden = Set(h);
    });
    let event = m_event.update(db.get_ref()).await?;

    UniResponse::ok(event.into()).into()
}
#[get("")]
pub async fn get_events(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<events::Model>> {
    let mut query_params = query_params.0;

    let stmt = Events::find();

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
pub async fn get_event(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<events::Model> {
    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    UniResponse::ok(event.into()).into()
}

#[delete("/{id}")]
pub async fn delete_event(_user: SuperAdminJwtGuard, db: WebDb, id: Path<Uuid>) -> UniResult<u64> {
    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let r = event.delete(db.get_ref()).await?;

    UniResponse::ok(r.rows_affected.into()).into()
}
