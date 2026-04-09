use std::str::FromStr;

use sea_orm::Condition;

use crate::{
    api::{FilterMapping, apply_filters, preclude::*, sea_orm_utils::paginate_query},
    entity::{event_logs, sea_orm_active_enums::EventType},
};

/// GET /api/admin/events/{event_id}/logs
#[get("")]
pub async fn get_event_logs(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    event_id: Path<Uuid>,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<event_logs::Model>> {
    let event_id = event_id.into_inner();
    let mut query_params = query_params.0;

    let mappings = [
        FilterMapping {
            key: "id",
            column: Box::new(|v| {
                Condition::all()
                    .add(event_logs::Column::Id.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())))
            }),
        },
        FilterMapping {
            key: "user_id",
            column: Box::new(|v| {
                Condition::all()
                    .add(event_logs::Column::UserId.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())))
            }),
        },
        FilterMapping {
            key: "team_id",
            column: Box::new(|v| {
                Condition::all()
                    .add(event_logs::Column::TeamId.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())))
            }),
        },
        FilterMapping {
            key: "type",
            column: Box::new(|v| {
                Condition::all().add(
                    event_logs::Column::Type
                        .eq(serde_json::from_str(v).unwrap_or(EventType::JeopardyPractice)),
                )
            }),
        },
        FilterMapping {
            key: "level",
            column: Box::new(|v| Condition::all().add(event_logs::Column::Level.eq(v.to_string()))),
        },
        FilterMapping {
            key: "action",
            column: Box::new(|v| Condition::all().add(event_logs::Column::Action.contains(v))),
        },
        FilterMapping {
            key: "details",
            column: Box::new(|v| Condition::all().add(event_logs::Column::Details.contains(v))),
        },
    ];

    let stmt = event_logs::Entity::find().filter(event_logs::Column::EventId.eq(event_id));
    let stmt = apply_filters(stmt, query_params.filter.clone(), &mappings);

    let (items, total_items) =
        if let (Some(limit), Some(page)) = (query_params.limit, query_params.page) {
            paginate_query(stmt, db.get_ref(), limit, page).await?
        } else {
            let items = stmt.all(db.get_ref()).await?;
            (items.clone(), items.len())
        };

    query_params.total = Some(total_items);

    UniResponse::ok_meta(items.into(), query_params.into()).into()
}
