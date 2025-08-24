use actix_web::web;
use anyhow::Result;
use bollard::{API_DEFAULT_VERSION, Docker};
use sea_orm::DbConn;

pub async fn init_db() -> Result<DbConn> {
    let database_url = std::env::var("DATABASE_URL")?;
    let db = sea_orm::Database::connect(&database_url).await?;
    Ok(db)
}

pub type WebDb = web::Data<DbConn>;

pub async fn init_docker() -> Result<Docker> {
    Ok(Docker::connect_with_http(
        "http://172.31.35.172:2375",
        4,
        API_DEFAULT_VERSION,
    )?)
    // Ok(Docker::connect_with_defaults()?)
}

pub type WebDocker = web::Data<Docker>;
