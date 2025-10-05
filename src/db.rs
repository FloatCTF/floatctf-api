use actix_web::web;
use anyhow::Result;
use bollard::Docker;

use sea_orm::DbConn;
use tracing::info;

pub async fn init_db() -> Result<DbConn> {
    let database_url = std::env::var("DATABASE_URL")?;
    let db = sea_orm::Database::connect(&database_url).await?;
    db.ping().await?;
    info!("Database connected OK");
    Ok(db)
}

pub type WebDb = web::Data<DbConn>;

pub async fn init_docker() -> Result<Docker> {
    let docker = Docker::connect_with_defaults()?;
    let s = docker.ping().await?;
    info!("Docker connected {}", s);
    Ok(docker)
    // Ok(Docker::connect_with_defaults()?)
}

pub type WebDocker = web::Data<Docker>;
