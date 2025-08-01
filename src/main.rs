mod api;
mod auth;
mod db;
mod entity;
mod middleware;

use actix_web::{App, HttpServer, web};
use dotenvy::dotenv;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // for env var
    dotenv().ok();

    // for database
    let db: db::WebDb = web::Data::new(
        db::init_db()
            .await
            .expect("DATABASE_URL must be set in .env file!"),
    );

    HttpServer::new(move || {
        App::new().app_data(db.clone()).service(
            web::scope("/api").configure(api::service_config).service(
                web::scope("/admin")
                    .wrap(middleware::SuperAdminGuardMiddleware)
                    .configure(api::admin_config),
            ),
        )
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
