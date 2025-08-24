mod api;
mod auth;
mod db;
mod entity;
mod middleware;

use actix_cors::Cors;
use actix_web::{App, HttpServer, http, web};
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

    // for docker
    let docker: db::WebDocker =
        web::Data::new(db::init_docker().await.expect("no docker installed!"));

    // for server
    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://localhost:3000")
            .allow_any_header()
            .allow_any_method()
            .supports_credentials()
            .max_age(3600);

        App::new()
            .wrap(cors)
            .app_data(db.clone())
            .app_data(docker.clone())
            .service(
                web::scope("/api/admin")
                    .wrap(middleware::SuperAdminGuardMiddleware)
                    .configure(api::admin_config),
            )
            // 将公共 API 放在另一个作用域，不受认证中间件影响
            .service(web::scope("/api").configure(api::service_config))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
