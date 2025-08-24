use actix_web::FromRequest;
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use sea_orm::{EntityTrait, entity::prelude::Uuid};
use serde::{Deserialize, Serialize};

use crate::{
    db::WebDb,
    entity::{prelude::SuperAdmin, prelude::Users, super_admin, users},
};
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Role {
    User,
    SuperAdmin,
}

/// Our claims struct, it needs to derive `Serialize` and/or `Deserialize`
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthClaims {
    pub sub: Uuid,
    pub role: Role,
    pub exp: usize,
}

pub fn gen_jwt_token(id: Uuid, role: Role) -> Result<String, jsonwebtoken::errors::Error> {
    let secret = std::env::var("SECRET").expect("SECRET must be set in .env file!");
    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(8))
        .expect("valid timestamp")
        .timestamp() as usize;
    let claims = AuthClaims {
        sub: id,
        role,
        exp: expiration,
    };

    encode(
        &Header::new(Algorithm::HS512),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
}

pub fn validate_jwt(token: String) -> Result<AuthClaims, jsonwebtoken::errors::Error> {
    let secret = std::env::var("SECRET").expect("SECRET must be set in .env file!");
    let token_data = decode::<AuthClaims>(
        &token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::new(Algorithm::HS512),
    )?;

    Ok(token_data.claims)
}

pub struct UserJwtGuard(users::Model);
impl UserJwtGuard {
    pub fn into_inner(self) -> users::Model {
        self.0
    }
}
pub struct SuperAdminJwtGuard(super_admin::Model);
impl SuperAdminJwtGuard {
    pub fn into_inner(self) -> super_admin::Model {
        self.0
    }
}
impl FromRequest for UserJwtGuard {
    type Error = actix_web::Error;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(
        req: &actix_web::HttpRequest,
        _payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        // Clone or convert everything we need from `req` here
        let db = req.app_data::<WebDb>().cloned().unwrap();
        let auth_header = req.headers().get("Authorization").map(|h| h.clone()); // Clone header to make it owned

        Box::pin(async move {
            if let Some(auth_header) = auth_header {
                let token = auth_header.to_str().unwrap_or("").to_string(); // owned String
                if token.starts_with("Bearer ") {
                    let jwt = token.trim_start_matches("Bearer ").trim().to_string();
                    if let Ok(claims) = validate_jwt(jwt) {
                        if let Ok(Some(user)) =
                            Users::find_by_id(claims.sub).one(db.get_ref()).await
                        {
                            return Ok(UserJwtGuard(user));
                        }
                    }
                }
            }

            Err(actix_web::error::ErrorUnauthorized(
                "Invalid or missing token, or contact the admin",
            ))
        })
    }
}

impl FromRequest for SuperAdminJwtGuard {
    type Error = actix_web::Error;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(
        req: &actix_web::HttpRequest,
        _payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        // Clone or convert everything we need from `req` here
        let db = req.app_data::<WebDb>().cloned().unwrap();
        let auth_header = req.headers().get("Authorization").map(|h| h.clone()); // Clone header to make it owned

        Box::pin(async move {
            if let Some(auth_header) = auth_header {
                let token = auth_header.to_str().unwrap_or("").to_string(); // owned String
                if token.starts_with("Bearer ") {
                    let jwt = token.trim_start_matches("Bearer ").trim().to_string();
                    if let Ok(claims) = validate_jwt(jwt) {
                        if let Ok(Some(super_admin)) =
                            SuperAdmin::find_by_id(claims.sub).one(db.get_ref()).await
                        {
                            return Ok(SuperAdminJwtGuard(super_admin));
                        }
                    }
                }
            }

            Err(actix_web::error::ErrorUnauthorized(
                "Invalid or missing token, or contact the admin",
            ))
        })
    }
}
