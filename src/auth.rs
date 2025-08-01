use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum Role {
    User,
    SuperAdmin,
}

/// Our claims struct, it needs to derive `Serialize` and/or `Deserialize`
#[derive(Debug, Serialize, Deserialize)]
pub struct AuthClaims {
    pub sub: i32,
    pub role: Role,
    pub exp: usize,
}

pub fn gen_jwt_token(id: i32, role: Role) -> Result<String, jsonwebtoken::errors::Error> {
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
