use std::str::FromStr;

use sea_orm::{Condition, sea_query::ValueType};

use crate::{
    api::{FilterMapping, preclude::*, sea_orm_utils::query_query},
    entity::{instances, sea_orm_active_enums::InstanceStatus, users},
    strategies::event,
};

/// GET /api/admin/instances
#[get("")]
pub async fn get_instances(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<instances::Model>> {
    let mut query_params = query_params.0;
    // const filterKeys = ["id", "status", "ref", "flag", "challenge_id", "user_id"];

    let mappings = [
        FilterMapping {
            key: "id",
            column: Box::new(|v| {
                Condition::all()
                    .add(instances::Column::Id.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())))
            }),
        },
        FilterMapping {
            key: "status",
            column: Box::new(|v| {
                Condition::all().add(
                    instances::Column::Status
                        .eq(serde_json::from_str(v).unwrap_or(InstanceStatus::Running)),
                )
            }),
        },
        FilterMapping {
            key: "ref",
            column: Box::new(|v| Condition::all().add(instances::Column::Ref.contains(v))),
        },
        FilterMapping {
            key: "flag",
            column: Box::new(|v| Condition::all().add(instances::Column::Flag.contains(v))),
        },
        FilterMapping {
            key: "challenge_id",
            column: Box::new(|v| {
                Condition::all().add(
                    instances::Column::ChallengeId.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())),
                )
            }),
        },
        FilterMapping {
            key: "user_id",
            column: Box::new(|v| {
                Condition::all()
                    .add(instances::Column::UserId.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())))
            }),
        },
    ];
    let (items, total_items) =
        query_query::<instances::Entity>(db.get_ref(), &mappings, &query_params).await?;

    query_params.total = Some(total_items);

    UniResponse::ok_meta(items.into(), query_params.into()).into()
}

/// GET /api/admin/instances/{instance_id}
#[get("/{instance_id}")]
pub async fn get_instance(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    instance_id: Path<Uuid>,
) -> UniResult<instances::Model> {
    let instance_id = instance_id.into_inner();
    let model = instances::Entity::find_by_id(instance_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", instance_id)))?;

    UniResponse::ok(model.into()).into()
}

pub async fn kill_running_instances(db: WebDb, docker: WebDocker) -> anyhow::Result<()> {
    let instances_users = instances::Entity::find()
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .find_also_related(users::Entity)
        .all(db.get_ref())
        .await?;

    for (instance, user) in instances_users
        .into_iter()
        .filter_map(|(i, u)| u.map(|user| (i, user)))
    {
        match event::common::destroy_instance(&db, &docker, instance.id, &user).await {
            Ok(_) => {
                tracing::info!("Killed instance {}", instance.id);
            }
            Err(e) => {
                let mut m_instance = instance.into_active_model();
                m_instance.status = Set(InstanceStatus::Failed);
                let instance = m_instance.update(db.get_ref()).await?;
                tracing::error!("{}: But {} was killed!", e, instance.id);
            }
        }
    }

    Ok(())
}
