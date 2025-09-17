use chrono::NaiveDateTime;
use sea_orm::{ColumnTrait, QueryFilter, QueryOrder};
use std::collections::BTreeSet;
use std::{
    collections::{HashMap, HashSet},
    result,
};

use super::super::preclude::*;
use crate::entity::prelude::EventWriteup;
use crate::entity::{event_announcements, event_writeup};
use crate::{
    api::service::calculate_next_dynamic_score,
    auth::UserJwtGuard,
    entity::{
        challenges, event_challenge_solves, event_challenges, event_team_members, event_teams,
        event_users, events, instances,
        prelude::{
            Challenges, EventAnnouncements, EventChallengeSolves, EventChallenges,
            EventTeamMembers, EventTeams, EventUsers, Events, Instances, Users,
        },
        sea_orm_active_enums::{EventTeamMemberRole, EventType, InstanceStatus},
        users,
    },
};

#[derive(Debug, Serialize, Deserialize)]
pub struct EventInfo {
    event: events::Model,
    joined: bool,
}

#[get("")]
pub async fn get_events(user: UserJwtGuard, db: WebDb) -> UniResult<Vec<EventInfo>> {
    let user = user.into_inner();

    let events_with_users = Events::find()
        .filter(events::Column::Hidden.eq(false))
        .find_with_related(EventUsers)
        .all(db.get_ref())
        .await?;

    let result = events_with_users
        .into_iter()
        .map(|(event, users)| {
            let joined = users.iter().any(|u| u.user_id == user.id);
            EventInfo { event, joined }
        })
        .collect::<Vec<_>>();

    UniResponse::ok(result.into()).into()
}
#[get("/{event_id}")]
pub async fn get_event(user: UserJwtGuard, db: WebDb, id: Path<Uuid>) -> UniResult<EventInfo> {
    let user = user.into_inner();

    let event = Events::find_by_id(*id)
        .filter(events::Column::Hidden.eq(false))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("event not found".to_string()))?;

    let joined = EventUsers::find_by_id((*id, user.id))
        .one(db.get_ref())
        .await?
        .is_some();

    UniResponse::ok(EventInfo { event, joined }.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventChallengeResult {
    pub challenge: challenges::Model,
    pub current_points: f64,
    pub solved_count: u64,
    pub solved: bool,
    pub solved_no: u64,
}

#[get("/{event_id}/challenges")]
pub async fn get_event_challenges(
    user: UserJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<Vec<EventChallengeResult>> {
    let user = user.into_inner();
    let _user = user.clone();

    // team 化
    let event = Events::find_by_id(*id)
        .filter(events::Column::Hidden.eq(false))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("event not found".to_string()))?;

    let now = Utc::now().naive_utc(); // 当前 UTC 时间
    if now < event.start_time {
        return Err(UniError::CustomError(
            "Event has not started yet".to_string(),
        ));
    }
    let joined = EventUsers::find_by_id((*id, user.id))
        .one(db.get_ref())
        .await?
        .is_some();
    if !joined {
        return Err(UniError::CustomError("not joined".to_string()));
    }

    let stmt = event
        .find_related(EventChallenges)
        .filter(event_challenges::Column::Hidden.eq(false))
        .find_also_related(Challenges); // join 关联挑战表

    let c_ec = stmt.all(db.get_ref()).await?;

    let mut result = Vec::new();
    for (event_challenge, challenge) in c_ec {
        if let Some(c) = challenge {
            let solved_count = EventChallengeSolves::find()
                .filter(event_challenge_solves::Column::EventId.eq(*id))
                .filter(event_challenge_solves::Column::ChallengeId.eq(c.id))
                .count(db.get_ref())
                .await?;

            // 查用户是否解出 & 解题记录
            let user_solve = EventChallengeSolves::find_by_id((*id, c.id, user.id))
                .one(db.get_ref())
                .await?;

            let mut solved_no = 0;
            let solved = user_solve.is_some();

            if let Some(us) = user_solve {
                // 统计比用户早的提交数量
                let before_count = EventChallengeSolves::find()
                    .filter(event_challenge_solves::Column::EventId.eq(*id))
                    .filter(event_challenge_solves::Column::ChallengeId.eq(c.id))
                    .filter(event_challenge_solves::Column::CreatedAt.lt(us.created_at))
                    .count(db.get_ref())
                    .await?;

                solved_no = before_count + 1;
            }

            let current_points = calculate_next_dynamic_score(event_challenge.points, solved_count);
            result.push(EventChallengeResult {
                challenge: c,
                current_points,
                solved_count,
                solved,
                solved_no,
            });
        }
    }

    UniResponse::ok(result.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventInstance {
    pub instance: instances::Model,
    pub challenge_name: String,
    pub user_nickname: String,
}
#[get("/{event_id}/instances")]
pub async fn get_event_instances(
    user: UserJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<Vec<EventInstance>> {
    let user = user.into_inner();

    let event = Events::find_by_id(*id)
        .filter(events::Column::Hidden.eq(false))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("event not found".to_string()))?;

    let now = Utc::now().naive_utc(); // 当前 UTC 时间
    if now < event.start_time {
        return Err(UniError::CustomError(
            "Event has not started yet".to_string(),
        ));
    }

    match event.r#type {
        EventType::JeopardySingle => {
            // 👇 查 instance 并关联 challenge 和 user
            let data = Instances::find()
                .filter(instances::Column::Status.eq(InstanceStatus::Running))
                .filter(instances::Column::UserId.eq(user.id))
                .filter(instances::Column::Ref.eq("JeopardySingle"))
                .find_also_related(Challenges) // instance -> challenge
                .find_also_related(Users) // instance -> user
                .all(db.get_ref())
                .await?;

            // 👇 把结果组装成 EventInstance
            let instances: Vec<EventInstance> = data
                .into_iter()
                .map(|(instance, challenge_opt, user_opt)| EventInstance {
                    instance,
                    challenge_name: challenge_opt.map(|c| c.name).unwrap_or_default(),
                    user_nickname: user_opt.map(|u| u.nickname).unwrap_or_default(),
                })
                .collect();

            UniResponse::ok(instances.into()).into()
        }
        _ => Err(UniError::CustomError(
            "event type not supported".to_string(),
        )),
    }
}

#[get("/{event_id}/challenges/{challenge_id}/instance")]
pub async fn get_event_challenge_instance(
    user: UserJwtGuard,
    db: WebDb,
    id: Path<(Uuid, Uuid)>,
) -> UniResult<instances::Model> {
    let user = user.into_inner();
    let (event_id, challenge_id) = id.into_inner();

    let event = Events::find_by_id(event_id)
        .filter(events::Column::Hidden.eq(false))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("event not found".to_string()))?;

    match event.r#type {
        EventType::JeopardySingle => {
            let instance = Instances::find()
                .filter(instances::Column::ChallengeId.eq(challenge_id))
                .filter(instances::Column::Status.eq(InstanceStatus::Running))
                .filter(instances::Column::UserId.eq(user.id))
                .filter(instances::Column::Ref.eq("JeopardySingle"))
                .one(db.get_ref())
                .await?;
            dbg!(&instance);
            UniResponse::ok(instance).into()
        }
        _ => Err(UniError::CustomError(
            "event type not supported".to_string(),
        )),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserTeam {
    pub name: String,
}

#[post("/{event_id}/team")]
pub async fn create_team(
    user: UserJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
    cut: Json<CreateUserTeam>,
) -> UniResult<event_teams::Model> {
    let user = user.into_inner();
    let cut = cut.into_inner();
    let event_id = *id;
    let event = Events::find_by_id(event_id)
        .filter(events::Column::Hidden.eq(false))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("event not found".to_string()))?;

    let now = Utc::now().naive_utc(); // 当前 UTC 时间
    if now >= event.start_time {
        return Err(UniError::CustomError(
            "Event has already started".to_string(),
        ));
    }
    // 判断是否已经加入了团队
    let event_user = EventUsers::find_by_id((event_id, user.id))
        .one(db.get_ref())
        .await?;

    if event_user.is_some() {
        return Err(UniError::CustomError("already joined team".to_string()));
    }

    let team = event_teams::ActiveModel {
        name: Set(cut.name),
        event_id: Set(event_id),
        ..Default::default()
    }
    .insert(db.get_ref())
    .await?;

    let new_event_user = event_users::ActiveModel {
        event_id: Set(event_id),
        user_id: Set(user.id),
        ..Default::default()
    };
    new_event_user.insert(db.get_ref()).await?;

    let new_event_team_member = event_team_members::ActiveModel {
        event_id: Set(event_id),
        user_id: Set(user.id),
        team_id: Set(team.id),
        role: Set(EventTeamMemberRole::Captain),
        ..Default::default()
    };
    new_event_team_member.insert(db.get_ref()).await?;

    UniResponse::ok(team.into()).into()
}

#[post("/{event_id}/join")]
pub async fn join_event(
    user: UserJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<event_users::Model> {
    let user = user.into_inner();
    let event = Events::find_by_id(*id)
        .filter(events::Column::Hidden.eq(false))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("event not found".to_string()))?;

    let now = Utc::now().naive_utc(); // 当前 UTC 时间
    if now >= event.start_time {
        return Err(UniError::CustomError(
            "Event has already started".to_string(),
        ));
    }

    let new_event_user = event_users::ActiveModel {
        event_id: Set(*id),
        user_id: Set(user.id),
        ..Default::default()
    };
    //  event_status 只有在未开始时 可加入 退出

    let user = new_event_user.insert(db.get_ref()).await?;

    UniResponse::ok(user.into()).into()
}

#[delete("/{event_id}/leave")]
pub async fn leave_event(user: UserJwtGuard, db: WebDb, id: Path<Uuid>) -> UniResult<u64> {
    let user = user.into_inner();
    let event = Events::find_by_id(*id)
        .filter(events::Column::Hidden.eq(false))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("event not found".to_string()))?;

    let now = Utc::now().naive_utc(); // 当前 UTC 时间
    if now >= event.start_time {
        return Err(UniError::CustomError(
            "Event has already started".to_string(),
        ));
    }
    let event_user = EventUsers::find_by_id((*id, user.id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("event user not found".to_string()))?;

    let event_user = event_user.delete(db.get_ref()).await?.rows_affected;

    UniResponse::ok(event_user.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChallengeScoreboard {
    pub name: String,
    pub solved: bool,
    pub solved_no: u64,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ScoreboardItem {
    pub no: u64,
    pub name: String,
    pub score: f64,
    pub solved_count: u64,
    pub challenges: Vec<ChallengeScoreboard>,
}
pub async fn __get_scoreboard(db: WebDb, event_id: Uuid) -> anyhow::Result<Vec<ScoreboardItem>> {
    let event = Events::find_by_id(event_id)
        .filter(events::Column::Hidden.eq(false))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("event not found".to_string()))?;

    match event.r#type {
        EventType::JeopardySingle => {
            // 1. 获取 event_challenges
            let event_challenges = EventChallenges::find()
                .filter(event_challenges::Column::EventId.eq(event_id))
                .filter(event_challenges::Column::Hidden.eq(false))
                .all(db.get_ref())
                .await?;

            // 提前拿到 challenge_ids
            let challenge_ids: Vec<Uuid> =
                event_challenges.iter().map(|ec| ec.challenge_id).collect();

            // 2. 获取所有 challenges
            let challenges = Challenges::find()
                .filter(challenges::Column::Id.is_in(challenge_ids.clone()))
                .all(db.get_ref())
                .await?;
            let challenge_map: HashMap<Uuid, challenges::Model> =
                challenges.into_iter().map(|c| (c.id, c)).collect();

            // 3. 获取所有 event_users
            // banned
            let event_users = EventUsers::find()
                .filter(event_users::Column::EventId.eq(event_id))
                .filter(event_users::Column::Banned.eq(false))
                .order_by_desc(event_users::Column::Points)
                .all(db.get_ref())
                .await?;
            let user_ids: Vec<Uuid> = event_users.iter().map(|eu| eu.user_id).collect();

            // 4. 获取所有 users
            let users = Users::find()
                .filter(users::Column::Id.is_in(user_ids.clone()))
                .all(db.get_ref())
                .await?;
            let user_map: HashMap<Uuid, users::Model> =
                users.into_iter().map(|u| (u.id, u)).collect();

            // 5. 获取所有 solves（按 challenge_id + created_at 排序）
            let solves = EventChallengeSolves::find()
                .filter(event_challenge_solves::Column::EventId.eq(event_id))
                .order_by_asc(event_challenge_solves::Column::ChallengeId)
                .order_by_asc(event_challenge_solves::Column::CreatedAt)
                .all(db.get_ref())
                .await?;

            // 这些结构：
            // user_solved 用来判断某用户是否解出某题
            // total_solved_per_chal 记录每题总解出人数
            // solve_order 为 (user_id, challenge_id) 记录该用户解出该题的“名次”（从 1 开始）
            let mut user_solved: HashSet<(Uuid, Uuid)> = HashSet::new();
            let mut total_solved_per_chal: HashMap<Uuid, u64> = HashMap::new();
            let mut solve_order: HashMap<(Uuid, Uuid), u64> = HashMap::new();

            for s in solves {
                user_solved.insert((s.user_id, s.challenge_id));
                let entry = total_solved_per_chal.entry(s.challenge_id).or_insert(0);
                *entry += 1;
                // 仅在首次遇到该用户对这道题的解时记录名次（防重）
                solve_order
                    .entry((s.user_id, s.challenge_id))
                    .or_insert(*entry);
            }

            // 6. 拼装 scoreboard
            let mut scoreboard = Vec::new();
            for (no, event_user) in event_users.iter().enumerate() {
                let user = user_map
                    .get(&event_user.user_id)
                    .ok_or(UniError::NotFound("user not found".to_string()))?;

                let mut challenges = Vec::new();

                for ec in event_challenges.iter() {
                    let solved = user_solved.contains(&(event_user.user_id, ec.challenge_id));
                    // 每题总解出人数（如果你也想展示的话）
                    let _total_for_chal = total_solved_per_chal
                        .get(&ec.challenge_id)
                        .cloned()
                        .unwrap_or(0);
                    // 该用户对这题的解题名次（第几个解出）
                    let order_for_user = solve_order
                        .get(&(event_user.user_id, ec.challenge_id))
                        .cloned()
                        .unwrap_or(0);

                    let challenge = challenge_map
                        .get(&ec.challenge_id)
                        .ok_or(UniError::NotFound("challenge not found".to_string()))?;

                    challenges.push(ChallengeScoreboard {
                        name: challenge.name.clone(),
                        solved,
                        solved_no: order_for_user, // ← 现在是“第几个解出”
                    });
                }

                let solved_count = challenges.iter().filter(|c| c.solved).count() as u64;

                scoreboard.push(ScoreboardItem {
                    no: no as u64 + 1,
                    name: user.nickname.clone(),
                    score: event_user.points,
                    solved_count,
                    challenges,
                });
            }

            Ok(scoreboard)
        }
        _ => Err(UniError::CustomError("event type not supported".to_string()).into()),
    }
}

#[get("/{event_id}/scoreboard")]
pub async fn get_scoreboard(
    _user: UserJwtGuard,
    db: WebDb,
    event_id: Path<Uuid>,
) -> UniResult<Vec<ScoreboardItem>> {
    let scoreboard = __get_scoreboard(db, *event_id)
        .await
        .map_err(|e| UniError::CustomError(format!("{}", e)))?;

    UniResponse::ok(scoreboard.into()).into()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TrendPoint {
    pub name: String,
    pub score: f64, // total score
    pub time: NaiveDateTime,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TrendItem {
    pub name: String,
    pub points: Vec<TrendPoint>,
}
pub async fn __get_trend(db: WebDb, event_id: Uuid) -> anyhow::Result<Vec<TrendItem>> {
    let solves = event_challenge_solves::Entity::find()
        .filter(event_challenge_solves::Column::EventId.eq(event_id))
        .order_by_asc(event_challenge_solves::Column::CreatedAt)
        .all(db.get_ref())
        .await?;

    // --- 预取用户 ---
    let user_ids: Vec<Uuid> = solves.iter().map(|s| s.user_id).collect();
    let users_map: HashMap<Uuid, users::Model> = users::Entity::find()
        .filter(users::Column::Id.is_in(user_ids.clone()))
        .all(db.get_ref())
        .await?
        .into_iter()
        .map(|u| (u.id, u))
        .collect();

    // --- 预取题目 ---
    let challenge_ids: Vec<Uuid> = solves.iter().map(|s| s.challenge_id).collect();
    let challenges_map: HashMap<Uuid, challenges::Model> = challenges::Entity::find()
        .filter(challenges::Column::Id.is_in(challenge_ids))
        .all(db.get_ref())
        .await?
        .into_iter()
        .map(|c| (c.id, c))
        .collect();

    // --- 按 user_id 分组 ---
    let mut user_solves_map: HashMap<Uuid, Vec<event_challenge_solves::Model>> = HashMap::new();
    for solve in solves {
        user_solves_map
            .entry(solve.user_id)
            .or_default()
            .push(solve);
    }

    // --- 收集所有时间点 ---
    let mut all_times = BTreeSet::new(); // 按升序排序
    for solves in user_solves_map.values() {
        for s in solves {
            all_times.insert(s.created_at);
        }
    }

    // --- 为每个用户生成趋势点 ---
    let mut user_scores: HashMap<Uuid, f64> = HashMap::new();
    let mut trend_items_map: HashMap<Uuid, Vec<TrendPoint>> = HashMap::new();

    for &time in &all_times {
        for (&user_id, solves) in &user_solves_map {
            let score = user_scores.entry(user_id).or_insert(0.0);

            // 当前时间点有 solve 就累加
            for solve in solves.iter().filter(|s| s.created_at == time) {
                *score += solve.bonus_points;
            }

            let name = solves
                .iter()
                .find(|s| s.created_at == time)
                .and_then(|s| challenges_map.get(&s.challenge_id))
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "".to_string());

            trend_items_map
                .entry(user_id)
                .or_default()
                .push(TrendPoint {
                    name,
                    score: *score,
                    time,
                });
        }
    }

    // --- 转成 Vec<TrendItem> ---
    let trend_items: Vec<TrendItem> = user_scores
        .keys()
        .map(|user_id| TrendItem {
            name: users_map.get(user_id).unwrap().nickname.clone(),
            points: trend_items_map.get(user_id).unwrap().clone(),
        })
        .collect();

    Ok(trend_items)
}

#[get("/{event_id}/trend")]
pub async fn get_trend(
    _user: UserJwtGuard,
    db: WebDb,
    event_id: Path<Uuid>,
) -> UniResult<Vec<TrendItem>> {
    let trend_items = __get_trend(db, *event_id)
        .await
        .map_err(|e| UniError::CustomError(format!("{}", e)))?;

    UniResponse::ok(trend_items.into()).into()
}

#[get("/{event_id}/announcements")]
pub async fn get_announcements(
    _user: UserJwtGuard,
    db: WebDb,
    event_id: Path<Uuid>,
) -> UniResult<Vec<event_announcements::Model>> {
    let announcements = EventAnnouncements::find()
        .filter(event_announcements::Column::EventId.eq(*event_id))
        .order_by_desc(event_announcements::Column::CreatedAt)
        .all(db.get_ref())
        .await?;

    UniResponse::ok(announcements.into()).into()
}

#[get("/{event_id}/submit_wp_status")]

pub async fn get_submit_wp_status(
    _user: UserJwtGuard,
    db: WebDb,
    event_id: Path<Uuid>,
) -> UniResult<NaiveDateTime> {
    let wp = EventWriteup::find()
        .filter(event_writeup::Column::EventId.eq(*event_id))
        .order_by_desc(event_writeup::Column::CreatedAt)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("no wp".into()))?;

    UniResponse::ok(wp.created_at.into()).into()
}
