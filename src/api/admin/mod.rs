mod challenge_sets;
mod challenges;
mod event_announcements;
mod event_challenges;
mod event_teams;
mod event_users;
mod event_writeups;
mod events;
mod instances;
mod settings;
mod super_admin;
mod system;
mod users;
use actix_web::web::{ServiceConfig, scope};

pub fn config(cfg: &mut ServiceConfig) {
    cfg.service(system::get_sys_info);
    cfg.service(
        scope("/settings")
            .service(settings::get_settings)
            .service(settings::create_setting)
            .service(settings::delete_setting)
            .service(settings::patch_setting),
    );

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
            .service(challenges::build_challenge)
            // 优先级高于 /challenges/{challenge_id}
            .service(challenges::create_challenge)
            .service(challenges::delete_challenge)
            .service(challenges::update_challenge)
            .service(challenges::patch_challenge)
            .service(challenges::get_challenges)
            .service(challenges::get_challenge), // web import challenge
    );

    cfg.service(
        scope("/challenge_sets")
            .service(challenge_sets::create_challenge_set)
            .service(challenge_sets::delete_challenge_set)
            .service(challenge_sets::get_challenge_sets)
            .service(challenge_sets::get_challenge_set)
            .service(challenge_sets::delete_challenge_from_set)
            .service(challenge_sets::add_challenge_to_set)
            .service(challenge_sets::patch_challenge_set),
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
            .service(events::get_data)
            .service(events::get_report)
            .service(
                scope("/{event_id}/users")
                    .service(event_users::add_user)
                    .service(event_users::remove_user)
                    .service(event_users::banned_user)
                    .service(event_users::unbanned_user)
                    .service(event_users::get_users),
            )
            .service(
                scope("/{event_id}/teams")
                    .service(event_teams::add_team)
                    .service(event_teams::remove_team)
                    .service(event_teams::get_teams)
                    .service(event_teams::get_team_members)
                    .service(event_teams::add_user_to_team)
                    .service(event_teams::remove_user_from_team)
                    .service(event_teams::ban_team)
                    .service(event_teams::unbanned_team),
            )
            .service(
                scope("/{event_id}/challenges")
                    .service(event_challenges::add_challenge)
                    .service(event_challenges::remove_challenge)
                    .service(event_challenges::get_challenges)
                    .service(event_challenges::hidden_challenges)
                    .service(event_challenges::open_challenges),
            )
            .service(
                scope("/{event_id}/announcements")
                    .service(event_announcements::add_event_announcement)
                    .service(event_announcements::update_event_announcement)
                    .service(event_announcements::remove_event_announcement)
                    .service(event_announcements::get_event_announcement)
                    .service(event_announcements::list_event_announcements),
            )
            .service(scope("/{event_id}/writeups").service(event_writeups::get_all_event_writeups)),
    );
}
