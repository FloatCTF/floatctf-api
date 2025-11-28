use crate::{
    api::{admin::dto::DeleteItemsRequest, preclude::*},
    entity::{sea_orm_active_enums::SettingValueType, settings},
};

/// GET /api/admin/settings
#[get("")]
pub async fn get_settings(_user: SuperAdminJwtGuard, db: WebDb) -> UniResult<Vec<settings::Model>> {
    let settings = settings::Entity::find().all(db.get_ref()).await?;
    UniResponse::ok(settings.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSettingRequest {
    pub key: String,
    pub value: String,
    pub description: String,
    pub protected: bool,
    pub r#type: SettingValueType,
}

/// POST /api/admin/settings
#[post("")]
pub async fn create_setting(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    csr: Json<CreateSettingRequest>,
) -> UniResult<settings::Model> {
    let csr = csr.into_inner();

    let setting = settings::ActiveModel {
        key: Set(csr.key),
        value: Set(csr.value),
        description: Set(csr.description),
        r#type: Set(csr.r#type),
        protected: Set(csr.protected),
        ..Default::default()
    };
    let setting = setting.insert(db.get_ref()).await?;
    UniResponse::ok(setting.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatchSettingRequest {
    pub key: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
    pub protected: Option<bool>,
    pub r#type: Option<SettingValueType>,
}

/// PATCH /api/admin/settings/{setting_id}
#[patch("/{setting_id}")]
pub async fn patch_setting(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    setting_id: Path<Uuid>,
    psr: Json<PatchSettingRequest>,
) -> UniResult<settings::Model> {
    let setting_id = setting_id.into_inner();
    let psr = psr.into_inner();
    let setting = settings::Entity::find_by_id(setting_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", setting_id)))?;

    let mut m_setting = setting.into_active_model();

    psr.key.map(|k| {
        m_setting.key = Set(k);
    });
    psr.value.map(|v| {
        m_setting.value = Set(v);
    });
    psr.description.map(|d| {
        m_setting.description = Set(d);
    });
    psr.r#type.map(|t| {
        m_setting.r#type = Set(t);
    });
    psr.protected.map(|p| {
        m_setting.protected = Set(p);
    });
    let setting = m_setting.update(db.get_ref()).await?;
    UniResponse::ok(setting.into()).into()
}

/// DELETE /api/admin/settings
#[delete("")]
pub async fn delete_setting(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    dir: Json<DeleteItemsRequest>,
) -> UniResult<u64> {
    let dir = dir.into_inner();
    let mut deleted_count = 0;
    for setting_id in dir.id_list {
        let setting = settings::Entity::find_by_id(setting_id)
            .one(db.get_ref())
            .await?;
        if let Some(setting) = setting {
            if setting.protected {
                return Err(UniError::CustomError(format!(
                    "protected setting can not be deleted: {}",
                    setting.key
                )));
            }
            let r = setting.delete(db.get_ref()).await?;
            deleted_count += r.rows_affected;
        }
    }
    UniResponse::ok(deleted_count.into()).into()
}
