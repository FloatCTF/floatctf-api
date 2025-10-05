use crate::{
    api::preclude::*,
    auth::{Role, UserJwtGuard, gen_jwt_token},
    entity::{prelude::Users, users},
};

use argon2::{
    Argon2, PasswordHash, PasswordVerifier,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};

#[derive(Debug, Deserialize, Serialize)]
pub struct UserLoginRequest {
    username: String,
    password: String,
}

/// POST /api/users/session
#[post("/session")]
pub async fn user_login(db: WebDb, ulr: Json<UserLoginRequest>) -> UniResult<String> {
    let ulr = ulr.into_inner();

    match Users::find()
        .filter(users::Column::Username.eq(ulr.username))
        .one(db.get_ref())
        .await?
    {
        Some(user) => {
            let verified = {
                let parsed_hash = PasswordHash::new(&user.password).map_err(|e| {
                    UniError::InternalError(format!("Failed to new the PasswordHash: {e}"))
                })?;
                Argon2::default()
                    .verify_password(ulr.password.as_bytes(), &parsed_hash)
                    .is_ok()
            };

            if verified {
                let jwt = gen_jwt_token(user.id, Role::User)
                    .map_err(|e| UniError::CustomError(e.to_string()))?;

                UniResponse::ok(jwt.into()).into()
            } else {
                UniError::AuthError.into()
            }
        }
        None => UniError::AuthError.into(),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserRequest {
    username: String,
    nickname: String,
    password: String,
    email: String,
}

/// POST /api/users
#[post("")]
pub async fn create_user(db: WebDb, cur: Json<CreateUserRequest>) -> UniResult<String> {
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
        password: Set(hashed_password),
        email: Set(cur.email),
        nickname: Set(cur.nickname),
        ..Default::default()
    };

    let _user = new_user.insert(db.get_ref()).await?;

    UniResponse::ok(
        "User created successfully, please login "
            .to_string()
            .into(),
    )
    .into()
}

/// GET /api/users/me
#[get("/me")]
pub async fn get_me(user: UserJwtGuard) -> UniResult<users::Model> {
    let mut user = user.into_inner();
    user.password = "".to_string();
    UniResponse::ok(user.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatchMeRequest {
    pub nickname: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
}

/// PATCH /api/users/me
#[patch("/me")]
pub async fn patch_me(user: UserJwtGuard, db: WebDb, pmr: Json<PatchMeRequest>) -> UniResult<()> {
    let pmr = pmr.into_inner();
    let user = user.into_inner();

    let mut m_user = user.into_active_model();
    pmr.nickname.map(|n| {
        m_user.nickname = Set(n);
    });
    pmr.email.map(|e| {
        m_user.email = Set(e);
    });
    if let Some(p) = pmr.password {
        let hashed_password = {
            let salt = SaltString::generate(&mut OsRng);
            let argon2 = Argon2::default();

            let password_hash = argon2
                .hash_password(p.as_bytes(), &salt)
                .map_err(|e| UniError::CustomError(format!("{}", e.to_string())))?
                .to_string();

            password_hash
        };

        m_user.password = Set(hashed_password);
    }

    m_user.update(db.get_ref()).await?;

    UniResponse::ok_none().into()
}
