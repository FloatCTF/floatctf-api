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
}

impl UniError {
    pub fn code(&self) -> i32 {
        match self {
            UniError::DatabaseError(_) => 500,
            UniError::InternalError(_) => 500,
            UniError::NotFound(_) => 404,
            UniError::AuthError => 401,
            UniError::CustomError(_) => 500,
            UniError::SQLError(_) => 400,
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
