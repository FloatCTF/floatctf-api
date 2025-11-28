use std::os::unix::fs::PermissionsExt;

use actix_multipart::form::{MultipartForm, tempfile::TempFile, text::Text};

use crate::{
    api::{admin::dto::DeleteItemsRequest, preclude::*},
    entity::weapons,
};

/// GET /api/admin/weapons
#[get("")]
pub async fn get_weapons(_user: SuperAdminJwtGuard, db: WebDb) -> UniResult<Vec<weapons::Model>> {
    let weapons = weapons::Entity::find().all(db.get_ref()).await?;
    UniResponse::ok(weapons.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateWeaponRequest {
    pub name: String,
    pub category: String,
    pub description: Option<String>,
    pub has_file: bool,
    pub file_url: String,
    pub download_count: i64,
}

/// POST /api/admin/weapons
#[post("")]
pub async fn create_weapon(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    cwr: Json<CreateWeaponRequest>,
) -> UniResult<weapons::Model> {
    let cwr = cwr.into_inner();

    let weapon = weapons::ActiveModel {
        name: Set(cwr.name),
        category: Set(cwr.category),
        description: Set(cwr.description),
        has_file: Set(cwr.has_file),
        file_url: Set(cwr.file_url),
        download_count: Set(cwr.download_count.into()),
        ..Default::default()
    };
    let weapon = weapon.insert(db.get_ref()).await?;
    UniResponse::ok(weapon.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatchWeaponRequest {
    pub name: Option<String>,
    pub category: Option<String>,
    pub description: Option<String>,
    pub has_file: Option<bool>,
    pub file_url: Option<String>,
    pub download_count: Option<i64>,
}

/// PATCH /api/admin/weapons/{weapon_id}
#[patch("/{weapon_id}")]
pub async fn patch_weapon(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    weapon_id: Path<Uuid>,
    pwr: Json<PatchWeaponRequest>,
) -> UniResult<weapons::Model> {
    let weapon_id = weapon_id.into_inner();
    let pwr = pwr.into_inner();

    let mut weapon = weapons::Entity::find_by_id(weapon_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(
            "Weapon {} not exist",
            weapon_id
        )))?
        .into_active_model();

    if let Some(name) = pwr.name {
        weapon.name = Set(name);
    }
    if let Some(category) = pwr.category {
        weapon.category = Set(category);
    }
    if let Some(description) = pwr.description {
        weapon.description = Set(description.into());
    }
    if let Some(has_file) = pwr.has_file {
        weapon.has_file = Set(has_file);
    }
    if let Some(file_url) = pwr.file_url {
        weapon.file_url = Set(file_url);
    }
    if let Some(download_count) = pwr.download_count {
        weapon.download_count = Set(download_count.into());
    }

    let weapon = weapon.update(db.get_ref()).await?;
    UniResponse::ok(weapon.into()).into()
}

#[delete("")]
pub async fn delete_weapon(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    dir: Json<DeleteItemsRequest>,
) -> UniResult<u64> {
    let dir = dir.into_inner();

    let deleted_count = weapons::Entity::delete_many()
        .filter(weapons::Column::Id.is_in(dir.id_list))
        .exec(db.get_ref())
        .await?
        .rows_affected;

    UniResponse::ok(deleted_count.into()).into()
}

#[derive(Debug, MultipartForm)]
pub struct WeaponForm {
    #[multipart(limit = "10240MB")]
    weapon: TempFile,
}
// POST /api/admin/weapons/{weapon_id}/upload
#[post("/{weapon_id}/upload")]
pub async fn upload_weapon(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    weapon_id: Path<Uuid>,
    MultipartForm(form): MultipartForm<WeaponForm>,
) -> UniResult<()> {
    let weapons_dir = get_setting(&db, "WEAPONS_DIR")
        .await
        .map_err(|e| UniError::CustomError(format!("get weapons dir error: {}", e)))?;
    // if not exists, create it
    if !std::fs::metadata(&weapons_dir).is_ok() {
        std::fs::create_dir_all(&weapons_dir).unwrap();
    }

    let weapon_id = weapon_id.into_inner();
    let weapon_file = form.weapon;
    let weapon_path = format!(
        "{}/{}",
        weapons_dir,
        weapon_file.file_name.unwrap_or(weapon_id.to_string())
    );
    std::fs::copy(weapon_file.file.path(), &weapon_path)
        .map_err(|e| UniError::InternalError(format!("Failed to copy file: {}", e)))?;
    std::fs::set_permissions(&weapon_path, std::fs::Permissions::from_mode(0o644))
        .map_err(|e| UniError::InternalError(format!("Failed to set permissions: {}", e)))?;

    let mut weapon = weapons::Entity::find_by_id(weapon_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(
            "Weapon {} not exist",
            weapon_id
        )))?
        .into_active_model();
    weapon.has_file = Set(true);
    weapon.file_url = Set(weapon_path);
    weapon.update(db.get_ref()).await?;

    UniResponse::ok_none().into()
}
