use super::UniResponse;
use actix_web::{HttpResponse, ResponseError};
use sea_orm::DbErr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UniError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Internal error: {0}")]
    InternalError(String),
    #[error("Not Found: {0}")]
    NotFound(String),
    #[error("Authentication error")]
    AuthError,
    #[error("{0}")]
    CustomError(String),
    #[error("SQL Execution Error: {0}")]
    SQLError(String),
    #[error("Bad Request: {0}")]
    BadRequest(String),
    #[error("Not Enough Permission")]
    NotEnoughPermission,
}

impl UniError {
    pub fn code(&self) -> i32 {
        match self {
            UniError::DatabaseError(_) => 500,
            UniError::InternalError(_) => 500,
            UniError::NotFound(_) => 404,
            UniError::AuthError => 401,
            UniError::CustomError(_) => 400,
            UniError::SQLError(_) => 400,
            UniError::BadRequest(_) => 400,
            UniError::NotEnoughPermission => 403,
        }
    }

    pub fn to_response(&self) -> UniResponse<()> {
        UniResponse::err(self.code(), self.to_string())
    }
}

impl ResponseError for UniError {
    fn error_response(&self) -> HttpResponse {
        // HttpResponse::Ok().json(self.to_response())
        HttpResponse::build(self.status_code()).json(self.to_response())
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            UniError::DatabaseError(_) => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            UniError::InternalError(_) => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            UniError::NotFound(_) => actix_web::http::StatusCode::NOT_FOUND,
            UniError::AuthError => actix_web::http::StatusCode::UNAUTHORIZED,
            UniError::CustomError(_) => actix_web::http::StatusCode::BAD_REQUEST,
            UniError::SQLError(_) => actix_web::http::StatusCode::BAD_REQUEST,
            UniError::BadRequest(_) => actix_web::http::StatusCode::BAD_REQUEST,
            UniError::NotEnoughPermission => actix_web::http::StatusCode::FORBIDDEN,
        }
    }
}

//  for database error
impl From<DbErr> for UniError {
    fn from(value: DbErr) -> Self {
        UniError::DatabaseError(value.to_string())
    }
}

pub type UniResult<T> = Result<UniResponse<T>, UniError>;

impl<T> From<UniResponse<T>> for Result<UniResponse<T>, UniError> {
    fn from(resp: UniResponse<T>) -> Self {
        Ok(resp)
    }
}

impl<T> From<UniError> for Result<UniResponse<T>, UniError> {
    fn from(err_resp: UniError) -> Self {
        Err(err_resp)
    }
}
