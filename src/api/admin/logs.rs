use std::str::FromStr;

use sea_orm::Condition;

use crate::{
    api::{FilterMapping, prelude::*, sea_orm_utils::query_query},
    entity::logs,
};

/// GET /api/admin/logs
#[get("")]
pub async fn get_logs(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<logs::Model>> {
    let mut query_params = query_params.0;

    let mappings = [
        FilterMapping {
            key: "id",
            column: Box::new(|v| {
                Condition::all().add(logs::Column::Id.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())))
            }),
        },
        FilterMapping {
            key: "user_id",
            column: Box::new(|v| {
                Condition::all()
                    .add(logs::Column::UserId.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())))
            }),
        },
        FilterMapping {
            key: "superadmin_id",
            column: Box::new(|v| {
                Condition::all()
                    .add(logs::Column::SuperadminId.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())))
            }),
        },
        FilterMapping {
            key: "ip_address",
            column: Box::new(|v| Condition::all().add(logs::Column::IpAddress.contains(v))),
        },
        FilterMapping {
            key: "category",
            column: Box::new(|v| Condition::all().add(logs::Column::Category.eq(v.to_string()))),
        },
        FilterMapping {
            key: "action",
            column: Box::new(|v| Condition::all().add(logs::Column::Action.contains(v))),
        },
        FilterMapping {
            key: "level",
            column: Box::new(|v| Condition::all().add(logs::Column::Level.eq(v.to_string()))),
        },
    ];
    let (items, total_items) =
        query_query::<logs::Entity>(db.get_ref(), &mappings, &query_params).await?;

    query_params.total = Some(total_items);

    UniResponse::ok_meta(items.into(), query_params.into()).into()
}

/// GET /api/admin/logs/{log_id}
#[get("/{log_id}")]
pub async fn get_log(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    log_id: Path<Uuid>,
) -> UniResult<logs::Model> {
    let log_id = log_id.into_inner();
    let model = logs::Entity::find_by_id(log_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", log_id)))?;

    UniResponse::ok(model.into()).into()
}
