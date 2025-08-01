use actix_web::web;
use anyhow::Result;
use sea_orm::DbConn;

pub async fn init_db() -> Result<DbConn> {
    let database_url = std::env::var("DATABASE_URL")?;
    let db = sea_orm::Database::connect(&database_url).await?;
    Ok(db)
}

pub type WebDb = web::Data<DbConn>;
