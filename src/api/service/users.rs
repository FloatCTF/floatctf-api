use argon2::{Argon2, PasswordHash, PasswordVerifier};
use sea_orm::{ColumnTrait, QueryFilter};

use super::super::preclude::*;
use crate::auth::{Role, gen_jwt_token};
use crate::entity::{prelude::Users, users};

#[derive(Debug, Deserialize, Serialize)]
pub struct UserLoginRequest {
    username: String,
    password: String,
}

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
                let parsed_hash = PasswordHash::new(&user.password_hash).map_err(|e| {
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
