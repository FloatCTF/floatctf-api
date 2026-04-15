mod announcements;
mod challenge_solves;

mod challenge_sets;
mod challenge_writeups;
mod challenges;
mod events;
mod instances;
mod submit;
mod super_admin;
mod uploads;
mod users;
mod weapons;
use crate::config::get_setting;
use actix_web::web::{ServiceConfig, scope};
pub use events::{__get_scoreboard, __get_trend, ScoreboardItem, TrendItem};

use sea_orm::DbConn;

pub fn config(cfg: &mut ServiceConfig) {
    // POST /api/admin/session
    cfg.service(super_admin::super_admin_login);
    // GET /api/weapons
    cfg.service(scope("/weapons").service(weapons::get_weapons));

    // GET /api/announcements
    cfg.service(scope("/announcements").service(announcements::get_announcements));

    // POST /api/uploads/image
    cfg.service(scope("/uploads").service(uploads::upload_image));
    cfg.service(
        scope("/users")
            // POST /api/users/session
            .service(users::user_login)
            // POST /api/users
            .service(users::create_user)
            // GET /api/users/me
            .service(users::get_me)
            // PATCH /api/users/me
            .service(users::patch_me)
            // POST /api/users/reset_password
            .service(users::send_reset_email)
            // POST /api/users/reset?token=token
            .service(users::reset_password),
    );

    cfg.service(
        scope("/submit")
            // POST /api/submit/flag
            .service(submit::submit_flag)
            // POST /api/submit/writeup
            .service(submit::submit_writeup),
    );

    cfg.service(
        scope("/writeups")
            // GET /api/writeups/{writeup_id}
            .service(challenge_writeups::get_writeup)
            // GET /api/writeups
            .service(challenge_writeups::get_writeups),
    );

    cfg.service(
        scope("/challenges")
            // GET /api/challenges
            .service(challenges::get_challenges)
            // GET /api/challenges/{challenge_id}
            .service(challenges::get_challenge)
            // GET /api/challenges/{challenge_id}/instance
            .service(challenges::get_challenge_instance)
            // POST /api/challenges/{challenge_id}/my_writeup
            .service(challenge_writeups::create_challenge_writeup)
            // GET /api/challenges/{challenge_id}/my_writeup
            .service(challenge_writeups::get_challenge_writeup)
            // GET /api/challenges/{challenge_id}/writeups
            .service(challenge_writeups::get_challenge_writeups),
    );
    cfg.service(
        scope("/challenge_sets")
            // GET /api/challenge_sets
            .service(challenge_sets::get_challenge_sets)
            // GET /api/challenge_sets/{challenge_set_id}
            .service(challenge_sets::get_challenge_set),
    );

    cfg.service(
        scope("/instances")
            // GET /api/instances
            .service(instances::get_instances)
            // GET /api/instances/{instance_id}
            .service(instances::get_instance)
            // POST /api/instances/launch
            .service(instances::launch_instance)
            // DELETE /api/instances/{instance_id}
            .service(instances::destroy_instance),
    );
    cfg.service(
        scope("/solves")
            // GET /api/challenge_solves
            .service(challenge_solves::get_solves)
            // GET /api/challenge_solves/top15users
            .service(challenge_solves::get_top_15_users),
    );
    cfg.service(
        scope("/events")
            // GET /api/events
            .service(events::get_events)
            // GET /api/events/{event_id}/challenges
            .service(events::get_event_challenges)
            // GET /api/events/{event_id}
            .service(events::get_event)
            // GET /api/events/{event_id}/instances
            .service(events::get_event_instances)
            // GET /api/events/{event_id}/challenges/{challenge_id}/instance
            .service(events::get_event_challenge_instance)
            // GET /api/events/{event_id}/scoreboard
            .service(events::get_scoreboard)
            // GET /api/events/{event_id}/announcements
            .service(events::get_announcements)
            // GET /api/events/{event_id}/trend
            .service(events::get_trend)
            // GET /api/events/{event_id}/submit_wp_status
            .service(events::get_submit_wp_status)
            // POST /api/events/{event_id}/join
            .service(events::join_event)
            // POST /api/events/{event_id}/leave
            .service(events::leave_event)
            // POST /api/events/{event_id}/teams
            .service(events::create_team)
            // POST /api/events/{event_id}/teams/{team_id}/join
            .service(events::join_team)
            // POST /api/events/{event_id}/teams/{team_id}/quit
            .service(events::quit_team),
    );
}

/// 计算下一次动态得分。
///
/// 动态得分通常用于 CTF 或竞赛类题目的积分衰减计算。
/// 分值会随着解题人数的增加而衰减，但不会低于预设的最小百分比。
///
/// # 参数
///
/// - `base_points`: 题目的基础分值（初始最高分）。
/// - `solves`: 当前解出题目的总人数。
///
/// # 返回
///
/// 返回根据当前解题人数计算后的动态分值（不会低于最小分值）。
///
/// # 公式
///
/// 动态得分的计算公式为：
///
/// ```text
/// min_points + (base_points - min_points) * sqrt(decay / (decay + solves))
/// ```
///
/// 其中：
/// - `min_points = base_points * event_score_min_percent`
/// - `decay` 和 `event_score_min_percent` 由系统设置提供。
///
///
/// # 示例
///
/// ```rust,ignore
/// let score = calculate_next_dynamic_score(&db, 500.0, 10).await?;
/// println!("当前分数: {}", score);
/// ```
///
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
