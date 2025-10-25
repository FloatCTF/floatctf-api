use crate::{api::preclude::*, entity::weapons};

#[get("")]
pub async fn get_weapons(_user: UserJwtGuard, db: WebDb) -> UniResult<Vec<weapons::Model>> {
    let mut weapons = weapons::Entity::find().all(db.get_ref()).await?;
    for weapon in &mut weapons {
        let weapon_file = std::path::Path::new(&weapon.file_url);
        if !weapon_file.exists() {
            let mut m_weapon = weapon.clone().into_active_model();
            m_weapon.has_file = Set(false);
            m_weapon.update(db.get_ref()).await?;
            weapon.has_file = false; // 更新内存中的值
        }
    }
    UniResponse::ok(weapons.into()).into()
}
