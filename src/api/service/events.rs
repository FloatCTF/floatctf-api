use crate::{
    api::{preclude::*, service::calculate_next_dynamic_score},
    entity::{
        challenges, event_announcements, event_challenge_solves, event_challenges, event_instances,
        event_team_members, event_teams, event_users, event_writeup, events, instances,
        sea_orm_active_enums::{EventTeamMemberRole, EventType, InstanceStatus},
        users,
    },
};
use chrono::NaiveDateTime;
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Debug, Serialize, Deserialize)]
pub struct EventTeamMemberResult {
    pub member_name: String,
    pub member: event_team_members::Model,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct EventTeamResult {
    pub team: event_teams::Model,
    pub members: Vec<EventTeamMemberResult>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct EventInfo {
    event: events::Model,
    team_result: Option<EventTeamResult>,
    joined: bool,
}

/// GET /api/events
#[get("")]
pub async fn get_events(user: UserJwtGuard, db: WebDb) -> UniResult<Vec<EventInfo>> {
    let user = user.into_inner();

    let events_with_users = events::Entity::find()
        .filter(events::Column::Hidden.eq(false))
        .find_with_related(event_users::Entity)
        .all(db.get_ref())
        .await?;

    let mut result = Vec::new();

    for (event, users) in events_with_users {
        let joined = users.iter().any(|u| u.user_id == user.id);

        result.push(EventInfo {
            event,
            joined,
            team_result: None,
        });
    }

    UniResponse::ok(result.into()).into()
}

/// GET /api/events/{event_id}
#[get("/{event_id}")]
pub async fn get_event(user: UserJwtGuard, db: WebDb, id: Path<Uuid>) -> UniResult<EventInfo> {
    let user = user.into_inner();

    let event = events::Entity::find_by_id(*id)
        .filter(events::Column::Hidden.eq(false))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("event not found".to_string()))?;

    let joined = event_users::Entity::find_by_id((*id, user.id))
        .one(db.get_ref())
        .await?
        .is_some();

    let event_member = event_team_members::Entity::find()
        .filter(event_team_members::Column::EventId.eq(*id))
        .filter(event_team_members::Column::UserId.eq(user.id))
        .find_also_related(event_teams::Entity)
        .one(db.get_ref())
        .await?;

    let team = event_member.map(|(_, team)| team).flatten();
    match team {
        Some(team) => {
            let members = event_team_members::Entity::find()
                .filter(event_team_members::Column::EventId.eq(*id))
                .filter(event_team_members::Column::TeamId.eq(team.id))
                .find_also_related(users::Entity)
                .all(db.get_ref())
                .await?;
            let members = members
                .into_iter()
                .map(|(member, user)| EventTeamMemberResult {
                    member_name: user.map(|u| u.nickname).unwrap_or_default(),
                    member,
                })
                .collect();
            let team = EventTeamResult { team, members };
            return UniResponse::ok(
                EventInfo {
                    event,
                    joined,
                    team_result: Some(team),
                }
                .into(),
            )
            .into();
        }
        None => UniResponse::ok(
            EventInfo {
                event,
                joined,
                team_result: None,
            }
            .into(),
        )
        .into(),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventChallengeResult {
    pub challenge: challenges::Model,
    pub current_points: f64,
    pub solved_count: u64,
    pub solved: bool,
    pub solved_no: u64,
}

/// GET /api/events/{event_id}/challenges
#[get("/{event_id}/challenges")]
pub async fn get_event_challenges(
    user: UserJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<Vec<EventChallengeResult>> {
    let user = user.into_inner();
    let _user = user.clone();

    // team 化
    let event = events::Entity::find_by_id(*id)
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
    let joined = event_users::Entity::find_by_id((*id, user.id))
        .one(db.get_ref())
        .await?
        .is_some();
    if !joined {
        return Err(UniError::CustomError("not joined".to_string()));
    }

    let stmt = event
        .find_related(event_challenges::Entity)
        .filter(event_challenges::Column::Hidden.eq(false))
        .find_also_related(challenges::Entity); // join 关联挑战表

    let c_ec = stmt.all(db.get_ref()).await?;

    let mut result = Vec::new();
    for (event_challenge, challenge) in c_ec {
        if let Some(c) = challenge {
            let solved_count = event_challenge_solves::Entity::find()
                .filter(event_challenge_solves::Column::EventId.eq(*id))
                .filter(event_challenge_solves::Column::ChallengeId.eq(c.id))
                .count(db.get_ref())
                .await?;

            let (solved, solved_no) = {
                match event.r#type {
                    EventType::JeopardySingle => {
                        let user_solve =
                            event_challenge_solves::Entity::find_by_id((*id, c.id, user.id))
                                .one(db.get_ref())
                                .await?;

                        let mut solved_no = 0;
                        let solved = user_solve.is_some();

                        if let Some(us) = user_solve {
                            // 统计比用户早的提交数量
                            let before_count = event_challenge_solves::Entity::find()
                                .filter(event_challenge_solves::Column::EventId.eq(*id))
                                .filter(event_challenge_solves::Column::ChallengeId.eq(c.id))
                                .filter(event_challenge_solves::Column::CreatedAt.lt(us.created_at))
                                .count(db.get_ref())
                                .await?;

                            solved_no = before_count + 1;
                        }
                        (solved, solved_no)
                    }
                    EventType::JeopardyTeam => {
                        let team_member = event_team_members::Entity::find()
                            .filter(event_team_members::Column::EventId.eq(*id))
                            .filter(event_team_members::Column::UserId.eq(user.id))
                            .one(db.get_ref())
                            .await?
                            .ok_or(UniError::NotFound("you are not in any team".into()))?;

                        let team_solve = event_challenge_solves::Entity::find()
                            .filter(event_challenge_solves::Column::EventId.eq(*id))
                            .filter(event_challenge_solves::Column::ChallengeId.eq(c.id))
                            .filter(event_challenge_solves::Column::TeamId.eq(team_member.team_id))
                            .one(db.get_ref())
                            .await?;

                        let mut solved_no = 0;
                        let solved = team_solve.is_some();

                        if let Some(ts) = team_solve {
                            // 统计比用户早的提交数量
                            let before_count = event_challenge_solves::Entity::find()
                                .filter(event_challenge_solves::Column::EventId.eq(*id))
                                .filter(event_challenge_solves::Column::ChallengeId.eq(c.id))
                                .filter(event_challenge_solves::Column::CreatedAt.lt(ts.created_at))
                                .count(db.get_ref())
                                .await?;

                            solved_no = before_count + 1;
                        }

                        (solved, solved_no)
                    }
                    _ => {
                        return UniError::CustomError("event type not supported".to_string())
                            .into();
                    }
                }
            };
            // 查用户是否解出 & 解题记录

            let current_points =
                calculate_next_dynamic_score(&db, event_challenge.points, solved_count)
                    .await
                    .map_err(|e| {
                        UniError::CustomError(format!("calculate_next_dynamic_score error: {}", e))
                    })?;
            result.push(EventChallengeResult {
                challenge: c,
                current_points,
                solved_count,
                solved,
                solved_no,
            });
        }
    }
    result.sort_by(|a, b| b.challenge.category.cmp(&a.challenge.category));

    UniResponse::ok(result.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventInstance {
    pub instance: instances::Model,
    pub challenge_name: String,
    pub user_nickname: String,
}
/// GET /api/events/{event_id}/instances
#[get("/{event_id}/instances")]
pub async fn get_event_instances(
    user: UserJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<Vec<EventInstance>> {
    let user = user.into_inner();

    let event = events::Entity::find_by_id(*id)
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
            let data = instances::Entity::find()
                .filter(instances::Column::Status.eq(InstanceStatus::Running))
                .filter(instances::Column::UserId.eq(user.id))
                .filter(instances::Column::Ref.eq("JeopardySingle"))
                .find_also_related(challenges::Entity) // instance -> challenge
                .find_also_related(users::Entity) // instance -> user
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
        EventType::JeopardyTeam => {
            let team_member = event_team_members::Entity::find()
                .filter(event_team_members::Column::EventId.eq(*id))
                .filter(event_team_members::Column::UserId.eq(user.id))
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound("you are not in any team".into()))?;

            let data = event_instances::Entity::find()
                .filter(event_instances::Column::EventId.eq(*id))
                .filter(event_instances::Column::TeamId.eq(team_member.team_id))
                .find_also_related(instances::Entity)
                .filter(instances::Column::Status.eq(InstanceStatus::Running))
                .filter(instances::Column::Ref.eq("JeopardyTeam"))
                .find_also_related(challenges::Entity)
                .all(db.get_ref())
                .await?;

            // 👇 把结果组装成 EventInstance
            let instances: Vec<EventInstance> = data
                .into_iter()
                .map(|(_event_instance, instance, challenge)| EventInstance {
                    instance: instance.unwrap(),
                    challenge_name: challenge.map(|c| c.name).unwrap_or_default(),
                    user_nickname: "".to_string(), // 团队赛没有用户昵称 TODO: 这里应该是团队名称
                })
                .collect();
            UniResponse::ok(instances.into()).into()
        }
        _ => Err(UniError::CustomError(
            "event type not supported".to_string(),
        )),
    }
}
/// GET /api/events/{event_id}/challenges/{challenge_id}/instance
#[get("/{event_id}/challenges/{challenge_id}/instance")]
pub async fn get_event_challenge_instance(
    user: UserJwtGuard,
    db: WebDb,
    id: Path<(Uuid, Uuid)>,
) -> UniResult<instances::Model> {
    let user = user.into_inner();
    let (event_id, challenge_id) = id.into_inner();

    let event = events::Entity::find_by_id(event_id)
        .filter(events::Column::Hidden.eq(false))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("event not found".to_string()))?;

    match event.r#type {
        EventType::JeopardySingle => {
            let (_event_instance, instance) = event_instances::Entity::find()
                .filter(event_instances::Column::EventId.eq(event_id))
                .filter(event_instances::Column::ChallengeId.eq(challenge_id))
                .filter(event_instances::Column::UserId.eq(user.id))
                .find_also_related(instances::Entity)
                .filter(instances::Column::Status.eq(InstanceStatus::Running))
                .filter(instances::Column::Ref.eq("JeopardySingle"))
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound("no instance".into()))?;
            UniResponse::ok(instance).into()
        }
        EventType::JeopardyTeam => {
            let team_member = event_team_members::Entity::find()
                .filter(event_team_members::Column::EventId.eq(event_id))
                .filter(event_team_members::Column::UserId.eq(user.id))
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound("you are not in any team".into()))?;

            let (_event_instance, instance) = event_instances::Entity::find()
                .filter(event_instances::Column::EventId.eq(event_id))
                .filter(event_instances::Column::ChallengeId.eq(challenge_id))
                .filter(event_instances::Column::TeamId.eq(team_member.team_id))
                .find_also_related(instances::Entity)
                .filter(instances::Column::Status.eq(InstanceStatus::Running))
                .filter(instances::Column::Ref.eq("JeopardyTeam"))
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound("no instance".into()))?;

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
/// POST /api/events/{event_id}/team
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
    let event = events::Entity::find_by_id(event_id)
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
    let event_user = event_users::Entity::find_by_id((event_id, user.id))
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
/// DELETE /api/events/{event_id}/team/{team_id}
#[delete("/{event_id}/team/{team_id}")]
pub async fn quit_team(user: UserJwtGuard, db: WebDb, id: Path<(Uuid, Uuid)>) -> UniResult<()> {
    let user = user.into_inner();
    let (event_id, team_id) = id.into_inner();
    let team_member = event_team_members::Entity::find_by_id((event_id, team_id, user.id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("You are not of the team".to_string()))?;

    if team_member.role == EventTeamMemberRole::Captain {
        let team = event_teams::Entity::find_by_id(team_id)
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound("team not found".to_string()))?;
        team.delete(db.get_ref()).await?;
    } else {
        team_member.delete(db.get_ref()).await?;
    }

    let event_user = event_users::Entity::find_by_id((event_id, user.id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("You are not of the event".to_string()))?;
    event_user.delete(db.get_ref()).await?;

    UniResponse::ok_none().into()
}

/// POST /api/events/{event_id}/team/{team_id}/join
#[post("/{event_id}/team/{team_id}/join")]
pub async fn join_team(user: UserJwtGuard, db: WebDb, id: Path<(Uuid, Uuid)>) -> UniResult<()> {
    let user = user.into_inner();
    let (event_id, team_id) = id.into_inner();
    let team_member = event_team_members::Entity::find_by_id((event_id, team_id, user.id))
        .one(db.get_ref())
        .await?;

    if team_member.is_some() {
        return Err(UniError::CustomError("already joined team".to_string()));
    }
    let event_team = event_teams::Entity::find_by_id(team_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("team not found".to_string()))?;

    let new_event_team_member = event_team_members::ActiveModel {
        event_id: Set(event_id),
        user_id: Set(user.id),
        team_id: Set(event_team.id),
        role: Set(EventTeamMemberRole::Member),
        ..Default::default()
    };
    new_event_team_member.insert(db.get_ref()).await?;

    let new_event_user = event_users::ActiveModel {
        event_id: Set(event_id),
        user_id: Set(user.id),
        ..Default::default()
    };
    new_event_user.insert(db.get_ref()).await?;

    UniResponse::ok_none().into()
}

/// POST /api/events/{event_id}/team/{team_id}/leave
#[post("/{event_id}/team/{team_id}/leave")]
pub async fn leave_team(user: UserJwtGuard, db: WebDb, id: Path<(Uuid, Uuid)>) -> UniResult<()> {
    let user = user.into_inner();
    let (event_id, team_id) = id.into_inner();
    let team_member = event_team_members::Entity::find_by_id((event_id, team_id, user.id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("You are not of the team".to_string()))?;

    if team_member.role == EventTeamMemberRole::Captain {
        return Err(UniError::CustomError(
            "Captain can't leave team".to_string(),
        ));
    }

    team_member.delete(db.get_ref()).await?;

    UniResponse::ok_none().into()
}

/// POST /api/events/{event_id}/join
#[post("/{event_id}/join")]
pub async fn join_event(
    user: UserJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<event_users::Model> {
    let user = user.into_inner();
    let event = events::Entity::find_by_id(*id)
        .filter(events::Column::Hidden.eq(false))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("event not found".to_string()))?;

    if event.allow_join == false {
        return Err(UniError::CustomError("event not allow join".to_string()));
    }

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

/// DELETE /api/events/{event_id}/leave
#[delete("/{event_id}/leave")]
pub async fn leave_event(user: UserJwtGuard, db: WebDb, id: Path<Uuid>) -> UniResult<u64> {
    let user = user.into_inner();
    let event = events::Entity::find_by_id(*id)
        .filter(events::Column::Hidden.eq(false))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("event not found".to_string()))?;
    if event.allow_join == false {
        return Err(UniError::CustomError("event not allow leave".to_string()));
    }

    let now = Utc::now().naive_utc(); // 当前 UTC 时间
    if now >= event.start_time {
        return Err(UniError::CustomError(
            "Event has already started".to_string(),
        ));
    }
    let event_user = event_users::Entity::find_by_id((*id, user.id))
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
    let event = events::Entity::find_by_id(event_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("event not found".to_string()))?;

    match event.r#type {
        EventType::JeopardySingle => {
            // 1. 获取 event_challenges
            let event_challenges = event_challenges::Entity::find()
                .filter(event_challenges::Column::EventId.eq(event_id))
                .filter(event_challenges::Column::Hidden.eq(false))
                .all(db.get_ref())
                .await?;

            // 提前拿到 challenge_ids
            let challenge_ids: Vec<Uuid> =
                event_challenges.iter().map(|ec| ec.challenge_id).collect();

            // 2. 获取所有 challenges
            let challenges = challenges::Entity::find()
                .filter(challenges::Column::Id.is_in(challenge_ids.clone()))
                .all(db.get_ref())
                .await?;
            let challenge_map: HashMap<Uuid, challenges::Model> =
                challenges.into_iter().map(|c| (c.id, c)).collect();

            // 3. 获取所有 event_users
            // banned
            let event_users = event_users::Entity::find()
                .filter(event_users::Column::EventId.eq(event_id))
                .filter(event_users::Column::Banned.eq(false))
                .order_by_desc(event_users::Column::Points)
                .all(db.get_ref())
                .await?;
            let user_ids: Vec<Uuid> = event_users.iter().map(|eu| eu.user_id).collect();

            // 4. 获取所有 users
            let users = users::Entity::find()
                .filter(users::Column::Id.is_in(user_ids.clone()))
                .all(db.get_ref())
                .await?;

            let user_map: HashMap<Uuid, users::Model> =
                users.into_iter().map(|u| (u.id, u)).collect();

            // 5. 获取所有 solves（按 challenge_id + created_at 排序）
            let solves = event_challenge_solves::Entity::find()
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
        EventType::JeopardyTeam => {
            // 1. 获取 event_challenges
            let event_challenges = event_challenges::Entity::find()
                .filter(event_challenges::Column::EventId.eq(event_id))
                .filter(event_challenges::Column::Hidden.eq(false))
                .all(db.get_ref())
                .await?;

            // 提前拿到 challenge_ids
            let challenge_ids: Vec<Uuid> =
                event_challenges.iter().map(|ec| ec.challenge_id).collect();

            // 2. 获取所有 challenges
            let challenges = challenges::Entity::find()
                .filter(challenges::Column::Id.is_in(challenge_ids.clone()))
                .all(db.get_ref())
                .await?;
            let challenge_map: HashMap<Uuid, challenges::Model> =
                challenges.into_iter().map(|c| (c.id, c)).collect();

            let event_teams = event_teams::Entity::find()
                .filter(event_teams::Column::EventId.eq(event_id))
                .all(db.get_ref())
                .await?;

            let solves = event_challenge_solves::Entity::find()
                .filter(event_challenge_solves::Column::EventId.eq(event_id))
                .order_by_asc(event_challenge_solves::Column::ChallengeId)
                .order_by_asc(event_challenge_solves::Column::CreatedAt)
                .all(db.get_ref())
                .await?;

            let mut team_solved: HashSet<(Uuid, Uuid)> = HashSet::new();
            let mut total_solved_per_chal: HashMap<Uuid, u64> = HashMap::new();
            let mut solve_order: HashMap<(Uuid, Uuid), u64> = HashMap::new();

            for s in solves {
                team_solved.insert((s.team_id.unwrap(), s.challenge_id));
                let entry = total_solved_per_chal.entry(s.challenge_id).or_insert(0);
                *entry += 1;
                // 仅在首次遇到该用户对这道题的解时记录名次（防重）
                solve_order
                    .entry((s.team_id.unwrap(), s.challenge_id))
                    .or_insert(*entry);
            }

            let mut scoreboard = Vec::new();
            for (no, event_team) in event_teams.iter().enumerate() {
                let mut challenges = Vec::new();

                for ec in event_challenges.iter() {
                    let solved = team_solved.contains(&(event_team.id, ec.challenge_id));
                    // 每题总解出人数（如果你也想展示的话）
                    let _total_for_chal = total_solved_per_chal
                        .get(&ec.challenge_id)
                        .cloned()
                        .unwrap_or(0);
                    // 该用户对这题的解题名次（第几个解出）
                    let order_for_user = solve_order
                        .get(&(event_team.id, ec.challenge_id))
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
                    name: event_team.name.clone(),
                    score: event_team.points,
                    solved_count,
                    challenges,
                });
            }

            Ok(scoreboard)
        }

        _ => Err(UniError::CustomError("event type not supported".to_string()).into()),
    }
}

/// GET /api/events/{event_id}/scoreboard
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
    let event = events::Entity::find_by_id(event_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("event not found".to_string()))?;
    // --- 获取所有 solves ---
    let solves = event_challenge_solves::Entity::find()
        .filter(event_challenge_solves::Column::EventId.eq(event_id))
        .order_by_asc(event_challenge_solves::Column::CreatedAt)
        .all(db.get_ref())
        .await?;
    // --- 预取题目 ---
    let challenge_ids: Vec<Uuid> = solves.iter().map(|s| s.challenge_id).collect();
    let challenges_map: HashMap<Uuid, challenges::Model> = challenges::Entity::find()
        .filter(challenges::Column::Id.is_in(challenge_ids))
        .all(db.get_ref())
        .await?
        .into_iter()
        .map(|c| (c.id, c))
        .collect();

    match event.r#type {
        EventType::JeopardySingle => {
            // --- 预取用户 ---
            let user_ids: Vec<Uuid> = solves.iter().map(|s| s.user_id).collect();
            let users_map: HashMap<Uuid, users::Model> = users::Entity::find()
                .filter(users::Column::Id.is_in(user_ids.clone()))
                .all(db.get_ref())
                .await?
                .into_iter()
                .map(|u| (u.id, u))
                .collect();
            // --- 按 user_id 分组 ---
            let mut user_solves_map: HashMap<Uuid, Vec<event_challenge_solves::Model>> =
                HashMap::new();
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
        EventType::JeopardyTeam => {
            // TODO: 团队赛趋势图
            let team_ids = solves
                .iter()
                .map(|s| s.team_id.unwrap())
                .collect::<Vec<Uuid>>();
            let teams_map: HashMap<Uuid, event_teams::Model> = event_teams::Entity::find()
                .filter(event_teams::Column::Id.is_in(team_ids))
                .all(db.get_ref())
                .await?
                .into_iter()
                .map(|t| (t.id, t))
                .collect();
            let mut team_solves_map: HashMap<Uuid, Vec<event_challenge_solves::Model>> =
                HashMap::new();
            for solve in solves {
                team_solves_map
                    .entry(solve.team_id.unwrap())
                    .or_default()
                    .push(solve);
            }

            let mut all_times = BTreeSet::new(); // 按升序排序
            for solves in team_solves_map.values() {
                for s in solves {
                    all_times.insert(s.created_at);
                }
            }

            let mut team_scores: HashMap<Uuid, f64> = HashMap::new();
            let mut trend_items_map: HashMap<Uuid, Vec<TrendPoint>> = HashMap::new();

            for &time in &all_times {
                for (&team_id, solves) in &team_solves_map {
                    let score = team_scores.entry(team_id).or_insert(0.0);

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
                        .entry(team_id)
                        .or_default()
                        .push(TrendPoint {
                            name,
                            score: *score,
                            time,
                        });
                }
            }

            let trend_items: Vec<TrendItem> = team_scores
                .keys()
                .map(|team_id| TrendItem {
                    name: teams_map.get(team_id).unwrap().name.clone(),
                    points: trend_items_map.get(team_id).unwrap().clone(),
                })
                .collect();
            Ok(trend_items)
        }

        _ => Err(UniError::CustomError("event type not supported".to_string()).into()),
    }
}

/// GET /api/events/{event_id}/trend
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

/// GET /api/events/{event_id}/announcements
#[get("/{event_id}/announcements")]
pub async fn get_announcements(
    _user: UserJwtGuard,
    db: WebDb,
    event_id: Path<Uuid>,
) -> UniResult<Vec<event_announcements::Model>> {
    let announcements = event_announcements::Entity::find()
        .filter(event_announcements::Column::EventId.eq(*event_id))
        .order_by_desc(event_announcements::Column::CreatedAt)
        .all(db.get_ref())
        .await?;

    UniResponse::ok(announcements.into()).into()
}

/// GET /api/events/{event_id}/submit_wp_status
#[get("/{event_id}/submit_wp_status")]

pub async fn get_submit_wp_status(
    _user: UserJwtGuard,
    db: WebDb,
    event_id: Path<Uuid>,
) -> UniResult<NaiveDateTime> {
    let wp = event_writeup::Entity::find()
        .filter(event_writeup::Column::EventId.eq(*event_id))
        .order_by_desc(event_writeup::Column::CreatedAt)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("no wp".into()))?;

    UniResponse::ok(wp.created_at.into()).into()
}
