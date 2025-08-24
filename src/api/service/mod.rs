mod challenges;
mod events;
mod instances;
mod submit;
mod super_admin;
mod users;
use crate::middleware;
use actix_web::web::{ServiceConfig, scope};
use sea_orm::EntityTrait;
use sea_orm::entity::prelude::Uuid;

pub fn config(cfg: &mut ServiceConfig) {
    cfg.service(super_admin::super_admin_login);

    cfg.service(scope("/users").service(users::user_login));

    cfg.service(
        scope("/submit")
            .wrap(middleware::UserGuardMiddleWare)
            .service(submit::submit_flag)
            .service(submit::submit_writeup),
    );

    cfg.service(
        scope("/challenges")
            .wrap(middleware::UserGuardMiddleWare)
            .service(challenges::get_challenges)
            .service(challenges::get_challenge),
    );

    cfg.service(
        scope("/instances")
            .wrap(middleware::UserGuardMiddleWare)
            .service(instances::get_instances)
            .service(instances::get_instance)
            .service(instances::launch_instance)
            .service(instances::destroy_instance),
    );

    cfg.service(
        scope("/events")
            .wrap(middleware::UserGuardMiddleWare)
            .service(events::get_events)
            .service(events::get_event_challenges),
    );
}

pub async fn get_user(
    db: &crate::db::WebDb,
    request: &actix_web::HttpRequest,
) -> Result<crate::entity::users::Model, super::UniError> {
    let user_id = actix_web::HttpMessage::extensions(request)
        .get::<Uuid>()
        .ok_or_else(|| super::UniError::InternalError("can't parse the Uuid from jwt".to_string()))?
        .to_owned();

    let user = crate::entity::prelude::Users::find_by_id(user_id)
        .one(db.get_ref())
        .await?
        .ok_or_else(|| super::UniError::NotFound("user not found".to_string()))?;

    Ok(user)
}
