use crate::{
    api::{preclude::*, service::__destroy_instance},
    entity::{instances, sea_orm_active_enums::InstanceStatus, users},
};

/// GET /api/admin/instances
#[get("")]
pub async fn get_instances(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<instances::Model>> {
    let mut query_params = query_params.0;

    let stmt = instances::Entity::find();

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
        __destroy_instance(db.clone(), docker.clone(), instance.id, user).await?;
        tracing::info!("Killed instance {}", instance.id);
    }

    Ok(())
}
