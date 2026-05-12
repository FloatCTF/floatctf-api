mod api;
mod auth;
mod config;
mod db;
mod entity;
mod log;
mod prelude;
mod scheduler;
mod strategies;

use actix_cors::Cors;
use actix_web::middleware::Logger;
use actix_web::{App, HttpServer, web};
use dotenvy::dotenv;
use std::env;
use std::sync::Arc;
use tracing::{error, info};
use tracing_actix_web::TracingLogger;
use tracing_appender::rolling;
use tracing_subscriber::{EnvFilter, fmt::writer::MakeWriterExt};

use crate::log::LogService;
use crate::scheduler::TaskScheduler;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // for env var
    dotenv().ok();

    // work_dir
    let work_dir = env::var("WORK_DIR").unwrap_or("./".to_string());
    env::set_current_dir(&work_dir).unwrap();

    // 日志层
    let log_dir = env::var("LOG_DIR").unwrap_or("./logs".to_string());
    let file_appender = rolling::daily(log_dir, "log");
    let (file_writer, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("default=info".parse().unwrap()),
        )
        .with_writer(std::io::stdout.and(file_writer))
        .with_timer(tracing_subscriber::fmt::time::ChronoLocal::rfc_3339())
        .init();

    let version = env!("CARGO_PKG_VERSION");
    //
    info!("Current working dir = {}, version = {}", work_dir, version);

    // for database
    let db: db::WebDb = match db::init_db().await {
        Ok(db) => web::Data::new(db),
        Err(e) => {
            error!("init db failed: {}", e);
            panic!("init db failed: {}", e);
        }
    };

    // for docker
    let docker: db::WebDocker = match db::init_docker().await {
        Ok(docker) => web::Data::new(docker),
        Err(e) => {
            error!("init docker failed: {}", e);
            panic!("init docker failed: {}", e);
        }
    };

    // for rustfs
    let rustfs: db::WebRustfs = match db::init_rustfs().await {
        Ok(rustfs) => web::Data::new(rustfs),
        Err(e) => {
            error!("init rustfs failed: {}", e);
            panic!("init rustfs failed: {}", e);
        }
    };
    // for settings
    config::init_settings(&db).await;

    // log service
    let log_service = LogService::new(db.clone());

    // task scheduler
    let mut task_scheduler = TaskScheduler::new(
        db.clone(),
        docker.clone(),
        rustfs.clone(),
        log_service.clone(),
    );

    task_scheduler
        .init_startup_handlers()
        .await
        .expect("init startup handlers failed!");
    let task_scheduler_arc = Arc::new(task_scheduler);
    task_scheduler_arc
        .init_and_recover()
        .await
        .expect("init task scheduler failed!");

    let sc_clone = task_scheduler_arc.clone();
    actix_web::rt::spawn(async move {
        sc_clone.start_polling().await;
    });

    // for server
    let ip = env::var("SERVER_LISTEN_IP").unwrap_or("127.0.0.1".to_string());
    let port = env::var("SERVER_LISTEN_PORT")
        .unwrap_or("8080".to_string())
        .parse::<u16>()
        .unwrap();

    // for server
    HttpServer::new(move || {
        let cors = Cors::default()
            .allowed_origin("http://localhost:3000")
            .allowed_origin("http://127.0.0.1")
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
            .app_data(rustfs.clone())
            .app_data(web::Data::new(log_service.clone()))
            .app_data(web::Data::new(task_scheduler_arc.clone()))
            .service(
                web::scope("/api")
                    .configure(api::service_config)
                    .service(web::scope("/admin").configure(api::admin_config)),
            )
        // 将公共 API 放在另一个作用域，不受认证中间件影响
    })
    .bind((ip, port))?
    .run()
    .await
}
