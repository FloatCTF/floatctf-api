mod challenges;
mod super_admin;
mod users;
use actix_web::web::{ServiceConfig, scope};

pub fn config(cfg: &mut ServiceConfig) {
    cfg.service(
        scope("/users")
            .service(users::create_user)
            .service(users::update_user)
            .service(users::patch_user)
            .service(users::get_users)
            .service(users::get_user),
    );

    cfg.service(
        scope("/challenges")
            .service(challenges::create_challenge)
            .service(challenges::update_challenge)
            .service(challenges::patch_challenge)
            .service(challenges::get_challenges)
            .service(challenges::get_challenge),
    );
}
