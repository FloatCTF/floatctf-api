mod challenges;
mod events;
mod instances;
mod submit;
mod super_admin;
mod users;

use actix_web::web::{ServiceConfig, scope};
use sea_orm::EntityTrait;
use sea_orm::entity::prelude::Uuid;

pub fn config(cfg: &mut ServiceConfig) {
    cfg.service(super_admin::super_admin_login);

    cfg.service(scope("/users").service(users::user_login));

    cfg.service(
        scope("/submit")
            .service(submit::submit_flag)
            .service(submit::submit_writeup),
    );

    cfg.service(
        scope("/challenges")
            .service(challenges::get_challenges)
            .service(challenges::get_challenge),
    );

    cfg.service(
        scope("/instances")
            .service(instances::get_instances)
            .service(instances::get_instance)
            .service(instances::launch_instance)
            .service(instances::destroy_instance),
    );

    cfg.service(
        scope("/events")
            .service(events::get_events)
            .service(events::get_event_challenges),
    );
}
