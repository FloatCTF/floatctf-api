mod api;
mod auth;
mod db;
mod entity;

use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::{App, HttpServer, web};
use dotenvy::dotenv;
use tracing_actix_web::TracingLogger;
use tracing_appender::rolling;
use tracing_subscriber::{EnvFilter, fmt::writer::MakeWriterExt};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // for env var
    dotenv().ok();

    // 日志层
    let file_appender = rolling::daily("logs", "log");
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("default=info".parse().unwrap()),
        )
        .with_writer(std::io::stdout.and(file_writer))
        .with_timer(tracing_subscriber::fmt::time::ChronoLocal::rfc_3339())
        .init();

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
            .wrap(Logger::default())
            .wrap(TracingLogger::default())
            .wrap(cors)
            .app_data(db.clone())
            .app_data(docker.clone())
            .service(
                web::scope("/api")
                    .configure(api::service_config)
                    .service(web::scope("/admin").configure(api::admin_config)),
            )
        // 将公共 API 放在另一个作用域，不受认证中间件影响
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
