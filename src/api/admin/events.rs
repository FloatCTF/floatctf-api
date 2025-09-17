use super::super::preclude::*;
use crate::api::service::{ScoreboardItem, TrendItem};
use crate::{
    api::service::{__get_scoreboard, __get_trend, calculate_next_dynamic_score},
    auth::SuperAdminJwtGuard,
    entity::{
        event_challenge_solves, event_challenges, event_teams, event_users, events,
        prelude::{
            Challenges, EventChallengeSolves, EventChallenges, EventTeams, EventUsers, Events,
            Users,
        },
        sea_orm_active_enums::EventType,
        users,
    },
};
use chrono::NaiveDateTime;
use sea_orm::{ColumnTrait, QueryFilter, QueryOrder, QuerySelect};
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateEventRequest {
    pub r#type: EventType,
    pub title: String,
    pub description: Option<String>,
    pub hidden: bool,
    pub start_time: NaiveDateTime,
    pub end_time: NaiveDateTime,
}

#[post("")]
pub async fn create_event(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    cer: Json<CreateEventRequest>,
) -> UniResult<events::Model> {
    let cer = cer.into_inner();

    let new_event = events::ActiveModel {
        r#type: Set(cer.r#type),
        title: Set(cer.title),
        description: Set(cer.description),
        start_time: Set(cer.start_time),
        hidden: Set(cer.hidden),
        end_time: Set(cer.end_time),
        ..Default::default()
    };

    let event = new_event.insert(db.get_ref()).await?;

    UniResponse::ok(event.into()).into()
}

type UpdateEventRequest = CreateEventRequest;
#[put("/{id}")]
pub async fn update_event(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    uer: Json<UpdateEventRequest>,
    id: Path<Uuid>,
) -> UniResult<events::Model> {
    let uer = uer.into_inner();

    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let mut m_event = event.into_active_model();

    m_event.r#type = Set(uer.r#type);
    m_event.title = Set(uer.title);
    m_event.description = Set(uer.description);
    m_event.start_time = Set(uer.start_time);
    m_event.end_time = Set(uer.end_time);

    let event = m_event.update(db.get_ref()).await?;

    UniResponse::ok(event.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatchEventRequest {
    pub r#type: Option<EventType>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub hidden: Option<bool>,
    pub start_time: Option<NaiveDateTime>,
    pub end_time: Option<NaiveDateTime>,
}
#[patch("/{id}")]
pub async fn patch_event(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    per: Json<PatchEventRequest>,
    id: Path<Uuid>,
) -> UniResult<events::Model> {
    let per = per.into_inner();
    dbg!(&per);
    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;
    let mut m_event = event.into_active_model();

    per.r#type.map(|t| m_event.r#type = Set(t));

    per.title.map(|t| {
        m_event.title = Set(t);
    });

    per.description.map(|d| {
        m_event.description = Set(d.into());
    });

    per.start_time.map(|s| {
        m_event.start_time = Set(s);
    });

    per.end_time.map(|e| {
        m_event.end_time = Set(e);
    });

    per.hidden.map(|h| {
        m_event.hidden = Set(h);
    });
    let event = m_event.update(db.get_ref()).await?;

    UniResponse::ok(event.into()).into()
}
#[get("")]
pub async fn get_events(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<events::Model>> {
    let mut query_params = query_params.0;

    let stmt = Events::find();

    if let (Some(limit), Some(page)) = (query_params.limit, query_params.page) {
        let paginator = stmt.paginate(db.get_ref(), limit);
        let items = paginator.fetch_page(page.saturating_sub(1)).await?;
        query_params.total = Some(paginator.num_items().await? as usize);

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    } else {
        let items = stmt.all(db.get_ref()).await?;
        query_params.total = Some(items.len());

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    }
}

#[get("/{id}")]
pub async fn get_event(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<events::Model> {
    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    UniResponse::ok(event.into()).into()
}

#[delete("/{id}")]
pub async fn delete_event(_user: SuperAdminJwtGuard, db: WebDb, id: Path<Uuid>) -> UniResult<u64> {
    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let r = event.delete(db.get_ref()).await?;

    UniResponse::ok(r.rows_affected.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataEventChallenge {
    pub name: String,
    pub category: String,
    pub points: f64,
    pub solved_count: u64,
    pub solved_percent: f64,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct DataEventChallengeSolve {
    pub user_nickname: String,
    pub challenge_name: String,
    pub challenge_category: String,
    pub created_at: NaiveDateTime,
    pub bonus_points: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataPresent {
    pub event: events::Model,
    pub user_count: u64,
    pub team_count: u64,
    pub solved_recent_15: Vec<DataEventChallengeSolve>, // 谁 什么题 什么时间 多少分
    pub event_challenges: Vec<DataEventChallenge>,
    pub scoreboard: Vec<ScoreboardItem>,
    pub trend: Vec<TrendItem>,
}
// 每道题 小方形卡片 名称 分类， 分数， 解题人数，解题百分比

#[get("/{event_id}/data")]
pub async fn get_data(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    event_id: Path<Uuid>,
) -> UniResult<DataPresent> {
    let event = Events::find_by_id(*event_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", event_id)))?;

    let user_count = EventUsers::find()
        .filter(event_users::Column::EventId.eq(*event_id))
        .count(db.get_ref())
        .await?;

    let team_count = {
        if event.r#type == EventType::JeopardyTeam {
            EventTeams::find()
                .filter(event_teams::Column::EventId.eq(*event_id))
                .count(db.get_ref())
                .await?
        } else {
            0
        }
    };

    let solved_recent_15 = EventChallengeSolves::find()
        .filter(event_challenge_solves::Column::EventId.eq(*event_id))
        .order_by_desc(event_challenge_solves::Column::CreatedAt)
        .limit(15)
        .find_also_related(Users)
        .find_also_related(Challenges)
        .all(db.get_ref())
        .await?
        .into_iter()
        .map(|(solve, user, challenge)| DataEventChallengeSolve {
            user_nickname: user.map(|u| u.nickname).unwrap_or_default(),
            challenge_name: challenge.clone().map(|c| c.name).unwrap_or_default(),
            challenge_category: challenge.map(|c| c.category).unwrap_or_default(),
            created_at: solve.created_at,
            bonus_points: solve.bonus_points,
        })
        .collect::<Vec<_>>();
    // for all event's challenges
    let event_challenges = EventChallenges::find()
        .filter(event_challenges::Column::EventId.eq(*event_id))
        .find_also_related(Challenges)
        .all(db.get_ref())
        .await?;

    let mut data_event_challenges = Vec::new();
    for (event_challenge, challenge) in event_challenges {
        let solved_count = EventChallengeSolves::find()
            .filter(event_challenge_solves::Column::EventId.eq(*event_id))
            .filter(event_challenge_solves::Column::ChallengeId.eq(event_challenge.challenge_id))
            .count(db.get_ref())
            .await?;

        let solved_percent = {
            if event.r#type == EventType::JeopardyTeam {
                solved_count as f64 / team_count as f64
            } else {
                solved_count as f64 / user_count as f64
            }
        };

        let dec = DataEventChallenge {
            name: challenge.clone().map(|c| c.name).unwrap_or_default(),
            category: challenge.map(|c| c.category).unwrap_or_default(),
            points: calculate_next_dynamic_score(event_challenge.points, solved_count),
            solved_count,
            solved_percent,
        };

        data_event_challenges.push(dec);
    }
    data_event_challenges.sort_by(|a, b| b.solved_count.cmp(&a.solved_count));

    let scoreboard = __get_scoreboard(db.clone(), *event_id)
        .await
        .map_err(|e| UniError::CustomError(format!("{}", e)))?;
    let trend_items = __get_trend(db, *event_id)
        .await
        .map_err(|e| UniError::CustomError(format!("{}", e)))?;

    let data_present = DataPresent {
        event: event,
        user_count,
        team_count,
        solved_recent_15,
        event_challenges: data_event_challenges,
        scoreboard,
        trend: trend_items,
    };

    UniResponse::ok(data_present.into()).into()
}
