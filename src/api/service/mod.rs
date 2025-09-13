mod challenge_solves;
mod challenges;
mod events;
mod instances;
mod submit;
mod super_admin;
mod users;
use std::env;

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
            .service(challenges::get_challenge)
            .service(challenges::get_challenge_instance),
    );

    cfg.service(
        scope("/instances")
            .service(instances::get_instances)
            .service(instances::get_instance)
            .service(instances::launch_instance)
            .service(instances::destroy_instance),
    );
    cfg.service(
        scope("/solves")
            .service(challenge_solves::get_solves)
            .service(challenge_solves::get_top_15_users),
    );
    cfg.service(
        scope("/events")
            .service(events::get_events)
            .service(events::get_event_challenges)
            .service(events::get_event)
            .service(events::get_event_instances)
            .service(events::get_event_challenge_instance)
            .service(events::get_scoreboard)
            .service(events::get_trend)
            .service(events::join_event)
            .service(events::leave_event),
    );
}

pub fn calculate_next_dynamic_score(base_points: f64, solves: u64) -> f64 {
    if solves <= 0 {
        return base_points;
    }
    let decay: f64 = env::var("EVENT_SCORE_DECAY")
        .expect("EVENT_SCORE_DECAY must be set in .env file")
        .parse()
        .expect("需要一个数字来设置衰减");
    let min_points = base_points / 6.0;
    let current_points =
        min_points + (base_points - min_points) * ((decay / (decay + (solves) as f64)).sqrt());
    current_points.max(min_points)
}
