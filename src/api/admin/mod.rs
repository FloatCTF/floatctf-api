mod challenges;
mod event_challenges;
mod event_teams;
mod event_users;
mod events;
mod instances;
mod super_admin;
mod users;
mod utils;
use actix_web::web::{ServiceConfig, scope};

pub fn config(cfg: &mut ServiceConfig) {
    cfg.service(
        scope("/users")
            .service(users::create_user)
            .service(users::delete_user)
            .service(users::update_user)
            .service(users::patch_user)
            .service(users::get_users)
            .service(users::get_user),
    );

    cfg.service(
        scope("/challenges")
            .service(challenges::check_challenges)
            .service(challenges::web_import_challenge)
            // 优先级高于 /challenges/{challenge_id}
            .service(challenges::create_challenge)
            .service(challenges::delete_challenge)
            .service(challenges::update_challenge)
            .service(challenges::patch_challenge)
            .service(challenges::get_challenges)
            .service(challenges::get_challenge), // web import challenge
    );

    cfg.service(
        scope("/super_admin")
            .service(super_admin::create_super_admin)
            .service(super_admin::delete_super_admin)
            .service(super_admin::update_super_admin)
            .service(super_admin::patch_super_admin)
            .service(super_admin::get_super_admins)
            .service(super_admin::get_super_admin),
    );

    cfg.service(
        scope("/instances")
            .service(instances::get_instances)
            .service(instances::get_instance),
    );

    cfg.service(
        scope("/events")
            .service(events::create_event)
            .service(events::delete_event)
            .service(events::update_event)
            .service(events::patch_event)
            .service(events::get_events)
            .service(events::get_event)
            .service(
                scope("/{event_id}/users")
                    .service(event_users::add_user)
                    .service(event_users::remove_user)
                    .service(event_users::get_users),
            )
            .service(
                scope("/{event_id}/teams")
                    .service(event_teams::add_team)
                    .service(event_teams::remove_team)
                    .service(event_teams::get_teams)
                    .service(event_teams::get_team_members)
                    .service(event_teams::add_user_to_team)
                    .service(event_teams::remove_user_from_team),
            )
            .service(
                scope("/{event_id}/challenges")
                    .service(event_challenges::add_challenge)
                    .service(event_challenges::remove_challenge)
                    .service(event_challenges::get_challenges)
                    .service(event_challenges::hidden_challenges)
                    .service(event_challenges::open_challenges),
            ),
    );
}
