use actix_web::{
    delete, get, patch, post,
    web::{Json, Path},
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, IntoActiveModel, ModelTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    api::{UniError, UniResponse, UniResult},
    auth::SuperAdminJwtGuard,
    db::WebDb,
    entity::{sea_orm_active_enums::SettingValueType, settings},
};

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
#[patch("/{id}")]
pub async fn patch_setting(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
    psr: Json<PatchSettingRequest>,
) -> UniResult<settings::Model> {
    let id = id.into_inner();
    let psr = psr.into_inner();
    let setting = settings::Entity::find_by_id(id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

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

#[delete("/{id}")]
pub async fn delete_setting(_user: SuperAdminJwtGuard, db: WebDb, id: Path<Uuid>) -> UniResult<()> {
    let id = id.into_inner();
    let setting = settings::Entity::find_by_id(id).one(db.get_ref()).await?;
    if let Some(setting) = setting {
        if setting.protected {
            return Err(UniError::CustomError(format!(
                "protected setting can not be deleted: {}",
                setting.key
            )));
        }
        setting.delete(db.get_ref()).await?;
    }
    UniResponse::ok_none().into()
}
