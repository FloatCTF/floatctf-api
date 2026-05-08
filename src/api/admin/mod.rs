mod announcements;
mod challenge_sets;
mod challenges;
mod database;
mod docker;
mod download;
mod dto;
mod event_announcements;
mod event_challenges;
mod event_logs;
mod event_teams;
mod event_users;
mod event_writeups;
mod events;
mod instances;
mod logs;
mod scheduled_tasks;
mod settings;
mod super_admin;
mod system;
mod terminal;
mod users;
mod weapons;
use actix_web::web::{ServiceConfig, scope};

pub fn config(cfg: &mut ServiceConfig) {
    // GET /api/admin/download?key=asd/asd
    cfg.service(download::download);

    cfg.service(
        scope("system")
            // GET /api/admin/system/monitor
            .service(system::get_sys_info)
            // GET /api/admin/system/changelog
            .service(system::get_changelog),
    );

    // POST /api/admin/database/exec_sql
    cfg.service(scope("/database").service(database::exec_sql));

    cfg.service(
        scope("/announcements")
            // GET /api/admin/announcements
            .service(announcements::get_announcements)
            // POST /api/admin/announcements
            .service(announcements::create_announcement)
            // DELETE /api/admin/announcements
            .service(announcements::delete_announcement)
            // PATCH /api/admin/announcements/{announcement_id}
            .service(announcements::patch_announcement),
    );

    cfg.service(
        scope("/settings")
            // GET /api/admin/settings
            .service(settings::get_settings)
            // POST /api/admin/settings
            .service(settings::create_setting)
            // DELETE /api/admin/settings
            .service(settings::delete_setting)
            // PATCH /api/admin/settings/{setting_id}
            .service(settings::patch_setting),
    );

    cfg.service(
        scope("weapons")
            // POST /api/admin/weapons
            .service(weapons::create_weapon)
            // DELETE /api/admin/weapons
            .service(weapons::delete_weapon)
            // PATCH /api/admin/weapons/{weapon_id}
            .service(weapons::patch_weapon)
            // GET /api/admin/weapons
            .service(weapons::get_weapons)
            // POST /api/admin/weapons/{weapon_id}/upload
            .service(weapons::upload_weapon),
    );

    cfg.service(
        scope("/users")
            // POST /api/admin/users
            .service(users::create_user)
            // DELETE /api/admin/users
            .service(users::delete_user)
            // PATCH /api/admin/users/{user_id}
            .service(users::patch_user)
            // GET /api/admin/users
            .service(users::get_users)
            // GET /api/admin/users/{user_id}
            .service(users::get_user),
    );

    cfg.service(
        scope("/challenges")
            // POST /api/admin/challenges/check
            .service(challenges::check_challenges)
            // POST /api/admin/challenges/import
            .service(challenges::web_import_challenge)
            // POST /api/admin/challenges/{challenge_id}/build
            .service(challenges::build_challenge)
            // POST /api/admin/challenges
            .service(challenges::create_challenge) // 优先级高于 /challenges/{challenge_id}
            // DELETE /api/admin/challenges
            .service(challenges::delete_challenge)
            // PATCH /api/admin/challenges/{challenge_id}
            .service(challenges::patch_challenge)
            // GET /api/admin/challenges
            .service(challenges::get_challenges)
            // GET /api/admin/challenges/{challenge_id}
            .service(challenges::get_challenge),
    );

    cfg.service(
        scope("/challenge_sets")
            // POST /api/admin/challenge_sets
            .service(challenge_sets::create_challenge_set)
            // DELETE /api/admin/challenge_sets
            .service(challenge_sets::delete_challenge_set)
            // GET /api/admin/challenge_sets
            .service(challenge_sets::get_challenge_sets)
            // GET /api/admin/challenge_sets/{challenge_set_id}
            .service(challenge_sets::get_challenge_set)
            // DELETE /api/admin/challenge_sets/{challenge_set_id}/challenges
            .service(challenge_sets::delete_challenge_from_set)
            // POST /api/admin/challenge_sets/{challenge_set_id}/challenges
            .service(challenge_sets::add_challenge_to_set)
            // PATCH /api/admin/challenge_sets/{challenge_set_id}
            .service(challenge_sets::patch_challenge_set),
    );

    cfg.service(
        scope("/super_admin")
            // POST /api/admin/super_admin
            .service(super_admin::create_super_admin)
            // DELETE /api/admin/super_admin
            .service(super_admin::delete_super_admin)
            // PATCH /api/admin/super_admin/{super_admin_id}
            .service(super_admin::patch_super_admin)
            // GET /api/admin/super_admin
            .service(super_admin::get_super_admins)
            // GET /api/admin/super_admin/{super_admin_id}
            .service(super_admin::get_super_admin),
    );

    cfg.service(
        scope("/instances")
            // GET /api/admin/instances
            .service(instances::get_instances)
            // GET /api/admin/instances/{instance_id}
            .service(instances::get_instance),
    );

    cfg.service(
        scope("/events")
            // POST /api/admin/events
            .service(events::create_event)
            // DELETE /api/admin/events
            .service(events::delete_event)
            // PATCH /api/admin/events/{event_id}
            .service(events::patch_event)
            // GET /api/admin/events
            .service(events::get_events)
            // GET /api/admin/events/{event_id}
            .service(events::get_event)
            // GET /api/admin/events/{event_id}/data
            .service(events::get_data)
            // GET /api/admin/events/{event_id}/report
            .service(events::get_report)
            .service(
                scope("/{event_id}/users")
                    // POST /api/admin/events/{event_id}/users
                    .service(event_users::add_user)
                    // DELETE /api/admin/events/{event_id}/users
                    .service(event_users::remove_user)
                    // POST /api/admin/events/{event_id}/users/{user_id}/banned
                    .service(event_users::banned_user)
                    // POST /api/admin/events/{event_id}/users/{user_id}/unbanned
                    .service(event_users::unbanned_user)
                    // GET /api/admin/events/{event_id}/users
                    .service(event_users::get_users),
            )
            .service(
                scope("/{event_id}/teams")
                    // POST /api/admin/events/{event_id}/teams
                    .service(event_teams::add_team)
                    // DELETE /api/admin/events/{event_id}/teams
                    .service(event_teams::remove_team)
                    // GET /api/admin/events/{event_id}/teams
                    .service(event_teams::get_teams)
                    // GET /api/admin/events/{event_id}/teams/{team_id}/users
                    .service(event_teams::get_team_members)
                    // POST /api/admin/events/{event_id}/teams/{team_id}/users
                    .service(event_teams::add_user_to_team)
                    // DELETE /api/admin/events/{event_id}/teams/{team_id}/users
                    .service(event_teams::remove_user_from_team)
                    // POST /api/admin/events/{event_id}/teams/{team_id}/banned
                    .service(event_teams::ban_team)
                    // POST /api/admin/events/{event_id}/teams/{team_id}/unbanned
                    .service(event_teams::unbanned_team),
            )
            .service(
                scope("/{event_id}/challenges")
                    // POST /api/admin/events/{event_id}/challenges
                    .service(event_challenges::add_challenge)
                    // DELETE /api/admin/events/{event_id}/challenges
                    .service(event_challenges::remove_challenge)
                    // GET /api/admin/events/{event_id}/challenges
                    .service(event_challenges::get_challenges)
                    // POST /api/admin/events/{event_id}/challenges/hidden
                    .service(event_challenges::hidden_challenges)
                    // POST /api/admin/events/{event_id}/challenges/open
                    .service(event_challenges::open_challenges),
            )
            .service(
                scope("/{event_id}/announcements")
                    // POST /api/admin/events/{event_id}/announcements
                    .service(event_announcements::add_event_announcement)
                    // PATCH /api/admin/events/{event_id}/announcements/{announcement_id}
                    .service(event_announcements::patch_event_announcement)
                    // DELETE /api/admin/events/{event_id}/announcements
                    .service(event_announcements::remove_event_announcement)
                    // GET /api/admin/events/{event_id}/announcements/{announcement_id}
                    .service(event_announcements::get_event_announcement)
                    // GET /api/admin/events/{event_id}/announcements
                    .service(event_announcements::list_event_announcements),
            )
            // GET /api/admin/events/{event_id}/writeups
            .service(scope("/{event_id}/writeups").service(event_writeups::get_all_event_writeups))
            // GET /api/admin/events/{event_id}/logs
            .service(scope("/{event_id}/logs").service(event_logs::get_event_logs)),
    );

    cfg.service(
        scope("/scheduled_tasks")
            // POST /api/admin/scheduled_tasks
            .service(scheduled_tasks::create_scheduled_task)
            // DELETE /api/admin/scheduled_tasks
            .service(scheduled_tasks::delete_scheduled_task)
            // PATCH /api/admin/scheduled_tasks/{task_id}
            .service(scheduled_tasks::patch_scheduled_task)
            // GET /api/admin/scheduled_tasks
            .service(scheduled_tasks::get_scheduled_tasks)
            // GET /api/admin/scheduled_tasks/{task_id}
            .service(scheduled_tasks::get_scheduled_task),
    );

    cfg.service(
        scope("/logs")
            // GET /api/admin/logs
            .service(logs::get_logs)
            // GET /api/admin/logs/{log_id}
            .service(logs::get_log),
    );

    cfg.service(
        scope("/docker")
            // GET /api/admin/docker/containers
            .service(docker::get_containers)
            // POST /api/admin/docker/containers/{container_id}/stop
            .service(docker::stop_container)
            // POST /api/admin/docker/containers/{container_id}/start
            .service(docker::start_container)
            // DELETE /api/admin/docker/containers/{container_id}
            .service(docker::delete_container)
            // GET /api/admin/docker/images
            .service(docker::get_images)
            // DELETE /api/admin/docker/images/{image_id}
            .service(docker::delete_image)
            // GET /api/admin/docker/networks
            .service(docker::get_networks)
            // POST /api/admin/docker/networks
            .service(docker::create_network)
            // DELETE /api/admin/docker/networks/{network_id}
            .service(docker::delete_network),
    );

    // GET /api/admin/terminal/ws (WebSocket)
    cfg.service(scope("/terminal").service(terminal::terminal_ws));
}
