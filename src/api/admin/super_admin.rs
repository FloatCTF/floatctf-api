use super::super::preclude::*;
use crate::{
    auth::SuperAdminJwtGuard,
    entity::{prelude::SuperAdmin, super_admin},
};
use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSuperAdminRequest {
    username: String,
    password: String,
    email: String,
}
#[post("")]
pub async fn create_super_admin(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    csr: Json<CreateSuperAdminRequest>,
) -> UniResult<super_admin::Model> {
    let csr = csr.into_inner();

    let hashed_password = {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(csr.password.as_bytes(), &salt)
            .map_err(|e| UniError::CustomError(format!("{}", e.to_string())))?
            .to_string();

        password_hash
    };

    let new_super_admin = super_admin::ActiveModel {
        username: Set(csr.username),
        password_hash: Set(hashed_password),
        email: Set(csr.email),
        ..Default::default()
    };

    let super_admin = new_super_admin.insert(db.get_ref()).await?;

    UniResponse::ok(super_admin.into()).into()
}

type UpdateSuperAdminRequest = CreateSuperAdminRequest;
#[post("/{id}")]
pub async fn update_super_admin(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    usr: Json<UpdateSuperAdminRequest>,
    id: Path<Uuid>,
) -> UniResult<super_admin::Model> {
    let usr = usr.into_inner();
    let hashed_password = {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(usr.password.as_bytes(), &salt)
            .map_err(|e| UniError::CustomError(format!("{}", e.to_string())))?
            .to_string();

        password_hash
    };

    let super_admin = SuperAdmin::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or_else(|| UniError::NotFound(format!("{} not exist", id)))?;

    let mut m_super_admin = super_admin.into_active_model();

    m_super_admin.username = Set(usr.username);
    m_super_admin.password_hash = Set(hashed_password);
    m_super_admin.email = Set(usr.email);

    let super_admin = m_super_admin.update(db.get_ref()).await?;

    UniResponse::ok(super_admin.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatchSuperAdminRequest {
    username: Option<String>,
    password: Option<String>,
    email: Option<String>,
}
#[post("/{id}")]
pub async fn patch_super_admin(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    psr: Json<PatchSuperAdminRequest>,
    id: Path<Uuid>,
) -> UniResult<super_admin::Model> {
    let psr = psr.into_inner();

    let super_admin = SuperAdmin::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or_else(|| UniError::NotFound(format!("{} not exist", id)))?;

    let mut m_super_admin = super_admin.into_active_model();

    psr.username.map(|u| {
        m_super_admin.username = Set(u);
    });

    if let Some(p) = psr.password {
        let hashed_password = {
            let salt = SaltString::generate(&mut OsRng);
            let argon2 = Argon2::default();

            let password_hash = argon2
                .hash_password(p.as_bytes(), &salt)
                .map_err(|e| UniError::CustomError(format!("{}", e.to_string())))?
                .to_string();

            password_hash
        };
        m_super_admin.password_hash = Set(hashed_password);
    }

    psr.email.map(|e| {
        m_super_admin.email = Set(e);
    });

    let super_admin = m_super_admin.update(db.get_ref()).await?;

    UniResponse::ok(super_admin.into()).into()
}

#[get("")]
pub async fn get_super_admins(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<super_admin::Model>> {
    let mut query_params = query_params.0;

    let stmt = SuperAdmin::find();

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
pub async fn get_super_admin(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<super_admin::Model> {
    let model = SuperAdmin::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or_else(|| UniError::NotFound(format!("{} not exist", id)))?;

    UniResponse::ok(model.into()).into()
}

#[delete("/{id}")]
pub async fn delete_super_admin(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<u64> {
    let super_admin = SuperAdmin::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or_else(|| UniError::NotFound(format!("{} not exist", id)))?;

    let r = super_admin.delete(db.get_ref()).await?;

    UniResponse::ok(r.rows_affected.into()).into()
}

#[actix_web::test]
pub async fn add_admin() {
    dotenvy::dotenv().ok();
    let db = crate::db::init_db().await.unwrap();
    let password = "admin";
    let username = "admin";
    let email = "admin@admin.com";

    let salt = SaltString::generate(&mut OsRng);
    let hashed_password = {
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(password.as_bytes(), &salt).unwrap();

        password_hash
    };

    let new_super_admin = super_admin::ActiveModel {
        username: Set(username.into()),
        password_hash: Set(hashed_password.to_string()),
        email: Set(email.into()),
        ..Default::default()
    };

    let _super_admin = new_super_admin.insert(&db).await.unwrap();
}
