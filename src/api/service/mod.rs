mod challenge_solves;

mod challenge_sets;
mod challenge_writeups;
mod challenges;
mod events;
mod instances;
mod submit;
mod super_admin;
mod users;
use actix_web::web::{ServiceConfig, scope};
pub use events::{__get_scoreboard, __get_trend, ScoreboardItem, TrendItem};
use sea_orm::entity::prelude::Uuid;
use sea_orm::{DbConn, EntityTrait};
use std::env;

use crate::config::get_setting;

pub fn config(cfg: &mut ServiceConfig) {
    cfg.service(super_admin::super_admin_login);

    cfg.service(
        scope("/users")
            .service(users::user_login)
            .service(users::create_user)
            .service(users::get_me)
            .service(users::patch_me),
    );

    cfg.service(
        scope("/submit")
            .service(submit::submit_flag)
            .service(submit::submit_writeup),
    );

    cfg.service(
        scope("/writeups")
            .service(challenge_writeups::get_writeup)
            .service(challenge_writeups::get_writeups),
    );

    cfg.service(
        scope("/challenges")
            .service(challenges::get_challenges)
            .service(challenges::get_challenge)
            .service(challenges::get_challenge_instance)
            .service(challenge_writeups::create_challenge_writeup)
            .service(challenge_writeups::get_challenge_writeup)
            .service(challenge_writeups::get_challenge_writeups),
    );
    cfg.service(
        scope("/challenge_sets")
            .service(challenge_sets::get_challenge_sets)
            .service(challenge_sets::get_challenge_set),
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
            .service(events::get_announcements)
            .service(events::get_trend)
            .service(events::get_submit_wp_status)
            .service(events::join_event)
            .service(events::leave_event)
            .service(events::create_team)
            .service(events::join_team)
            .service(events::quit_team),
    );
}

pub async fn calculate_next_dynamic_score(
    db: &DbConn,
    base_points: f64,
    solves: u64,
) -> anyhow::Result<f64> {
    if solves <= 0 {
        return Ok(base_points);
    }

    let decay = get_setting(db, "EVENT_SCORE_DECAY").await?.parse::<f64>()?;

    let event_score_min_percent = get_setting(db, "EVENT_SCORE_MIN_PERCENT")
        .await?
        .parse::<f64>()?;

    let min_points = base_points * event_score_min_percent;

    let current_points =
        min_points + (base_points - min_points) * ((decay / (decay + (solves) as f64)).sqrt());
    Ok(current_points.max(min_points))
}
