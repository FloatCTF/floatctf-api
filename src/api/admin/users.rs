use super::super::preclude::*;
use crate::entity::{prelude::Users, users};
use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserRequest {
    username: String,
    password: String,
    email: String,
}
#[post("")]
pub async fn create_user(db: WebDb, cur: Json<CreateUserRequest>) -> UniResult<()> {
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
        uuid: Set(Uuid::new_v4().to_string()),
        username: Set(cur.username),
        password_hash: Set(hashed_password),
        email: Set(cur.email),
        ..Default::default()
    };

    let _user = new_user.insert(db.get_ref()).await?;

    UniResponse::ok_none().into()
}

type UpdateUserRequest = CreateUserRequest;
#[put("/{id}")]
pub async fn update_user(
    db: WebDb,
    uur: Json<UpdateUserRequest>,
    user_id: Path<i32>,
) -> UniResult<()> {
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

    match Users::find_by_id(*user_id).one(db.get_ref()).await? {
        Some(user) => {
            let mut m_user = user.into_active_model();

            m_user.username = Set(uur.username);
            m_user.password_hash = Set(hashed_password);
            m_user.email = Set(uur.email);
            m_user.updated_at = Set(Utc::now().naive_utc());

            let _user = m_user.update(db.get_ref()).await?;

            UniResponse::ok_none().into()
        }
        None => UniError::NotFound(format!("user_id {} not exist", user_id)).into(),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PathUserRequest {
    username: Option<String>,
    password: Option<String>,
    email: Option<String>,
}
#[patch("/{id}")]
pub async fn patch_user(
    db: WebDb,
    pur: Json<PathUserRequest>,
    user_id: Path<i32>,
) -> UniResult<()> {
    let pur = pur.into_inner();

    match Users::find_by_id(*user_id).one(db.get_ref()).await? {
        Some(user) => {
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

            let _user = m_user.update(db.get_ref()).await?;
            UniResponse::ok_none().into()
        }
        None => UniError::NotFound(format!("user_id {} not exist", user_id)).into(),
    }
}

#[get("")]
pub async fn get_users(
    db: WebDb,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<users::Model>> {
    let mut query_params = query_params.0;

    let stmt = Users::find();

    if let (Some(limit), Some(page)) = (query_params.limit, query_params.page) {
        let paginator = stmt.paginate(db.get_ref(), limit);
        let items = paginator.fetch_page(page - 1).await?;
        query_params.total = Some(items.len());

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    } else {
        let items = stmt.all(db.get_ref()).await?;
        query_params.total = Some(items.len());

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    }
}

#[get("/{id}")]
pub async fn get_user(db: WebDb, id: Path<i32>) -> UniResult<users::Model> {
    match Users::find_by_id(*id).one(db.get_ref()).await? {
        Some(model) => UniResponse::ok(model.into()).into(),
        None => UniError::NotFound(format!(" {} not exist", id)).into(),
    }
}
