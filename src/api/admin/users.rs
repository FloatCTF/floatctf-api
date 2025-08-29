use super::super::preclude::*;
use crate::{
    auth::SuperAdminJwtGuard,
    entity::{prelude::Users, users},
};
use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserRequest {
    username: String,
    password: String,
    email: String,
}
#[post("")]
pub async fn create_user(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    cur: Json<CreateUserRequest>,
) -> UniResult<users::Model> {
    let cur = cur.into_inner();

    let hashed_password = {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(cur.password.as_bytes(), &salt)
            .map_err(|e| UniError::CustomError(format!("{}", e.to_string())))?
            .to_string();

        password_hash
    };

    let new_user = users::ActiveModel {
        username: Set(cur.username),
        password_hash: Set(hashed_password),
        email: Set(cur.email),
        ..Default::default()
    };

    let user = new_user.insert(db.get_ref()).await?;

    UniResponse::ok(user.into()).into()
}

type UpdateUserRequest = CreateUserRequest;
#[put("/{id}")]
pub async fn update_user(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    uur: Json<UpdateUserRequest>,
    id: Path<Uuid>,
) -> UniResult<users::Model> {
    let uur = uur.into_inner();

    let hashed_password = {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(uur.password.as_bytes(), &salt)
            .map_err(|e| UniError::CustomError(format!("{}", e.to_string())))?
            .to_string();

        password_hash
    };

    let user = Users::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let mut m_user = user.into_active_model();

    m_user.username = Set(uur.username);
    m_user.password_hash = Set(hashed_password);
    m_user.email = Set(uur.email);
    m_user.updated_at = Set(Utc::now().naive_utc());

    let user = m_user.update(db.get_ref()).await?;

    UniResponse::ok(user.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PathUserRequest {
    username: Option<String>,
    password: Option<String>,
    email: Option<String>,
}
#[patch("/{id}")]
pub async fn patch_user(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    pur: Json<PathUserRequest>,
    id: Path<Uuid>,
) -> UniResult<users::Model> {
    let pur = pur.into_inner();
    let user = Users::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let mut m_user = user.into_active_model();

    pur.username.map(|u| {
        m_user.username = Set(u);
    });

    if let Some(p) = pur.password {
        let hashed_password = {
            let salt = SaltString::generate(&mut OsRng);
            let argon2 = Argon2::default();

            let password_hash = argon2
                .hash_password(p.as_bytes(), &salt)
                .map_err(|e| UniError::CustomError(format!("{}", e.to_string())))?
                .to_string();

            password_hash
        };

        m_user.password_hash = Set(hashed_password);
    }

    pur.email.map(|e| {
        m_user.email = Set(e);
    });

    m_user.updated_at = Set(Utc::now().naive_utc());

    let user = m_user.update(db.get_ref()).await?;

    UniResponse::ok(user.into()).into()
}

#[get("")]
pub async fn get_users(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<users::Model>> {
    let mut query_params = query_params.0;

    let stmt = Users::find();

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
pub async fn get_user(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<users::Model> {
    let model = Users::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    UniResponse::ok(model.into()).into()
}

#[delete("/{id}")]
pub async fn delete_user(_user: SuperAdminJwtGuard, db: WebDb, id: Path<Uuid>) -> UniResult<u64> {
    let user = Users::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let r = user.delete(db.get_ref()).await?;

    UniResponse::ok(r.rows_affected.into()).into()
}
