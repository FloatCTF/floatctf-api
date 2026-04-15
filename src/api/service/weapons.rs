use crate::{
    api::{FilterMapping, prelude::*, sea_orm_utils::query_query},
    entity::weapons,
    prelude::*,
};
use sea_orm::Condition;

/// GET /api/weapons
#[get("")]
pub async fn get_weapons(
    _user: UserJwtGuard,
    ctx: ReqCtx,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<weapons::Model>> {
    let mut query_params = query_params.0;

    let mappings = [
        FilterMapping {
            key: "name",
            column: Box::new(|v| Condition::all().add(weapons::Column::Name.contains(v))),
        },
        FilterMapping {
            key: "category",
            column: Box::new(|v| Condition::all().add(weapons::Column::Category.contains(v))),
        },
        FilterMapping {
            key: "description",
            column: Box::new(|v| Condition::all().add(weapons::Column::Description.contains(v))),
        },
        FilterMapping {
            key: "has_file",
            column: Box::new(|v| {
                Condition::all()
                    .add(weapons::Column::HasFile.eq(v.parse::<bool>().unwrap_or(false)))
            }),
        },
    ];

    let (mut items, total_items) = query_query::<weapons::Entity>(
        ctx.db.get_ref(),
        &mappings,
        &query_params,
        Some(Box::new(|stmt| {
            stmt.order_by_desc(weapons::Column::UpdatedAt)
        })),
    )
    .await?;

    for weapon in &mut items {
        let weapon_file = std::path::Path::new(&weapon.file_url);
        if !weapon_file.exists() {
            let mut m_weapon = weapon.clone().into_active_model();
            m_weapon.has_file = Set(false);
            m_weapon.update(ctx.db.get_ref()).await?;
            weapon.has_file = false;
        }
    }

    query_params.total = Some(total_items);

    UniResponse::ok_meta(items.into(), query_params.into()).into()
}
