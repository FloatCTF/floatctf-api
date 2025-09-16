use sea_orm::{ColumnTrait, QueryFilter, sea_query::OnConflict};

use super::super::preclude::*;
use crate::{
    auth::SuperAdminJwtGuard,
    entity::{
        challenges, event_challenges,
        prelude::{Challenges, EventChallenges, Events},
    },
};

#[derive(Debug, Serialize, Deserialize)]
pub struct AddChallengeRequest {
    pub challenge_id: Option<Uuid>,
    pub challenge_id_list: Option<Vec<Uuid>>,
}

#[post("")]
pub async fn add_challenge(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
    acr: Json<AddChallengeRequest>,
) -> UniResult<Vec<event_challenges::Model>> {
    let acr = acr.into_inner();

    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!("event {} not exist", id)))?;

    let mut event_challenges_list = Vec::new();

    // 把单个 id 和多个 id 合并成一个 Vec
    let challenge_ids: Vec<Uuid> = acr
        .challenge_id
        .into_iter()
        .chain(acr.challenge_id_list.unwrap_or_default())
        .collect();

    for challenge_id in challenge_ids {
        // 先检查 challenge 是否存在
        let challenge = Challenges::find_by_id(challenge_id)
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(
                "challenge {} not exist",
                challenge_id
            )))?;

        // 查询是否已存在 event_challenge
        if let Some(existing) = event_challenges::Entity::find()
            .filter(event_challenges::Column::EventId.eq(event.id))
            .filter(event_challenges::Column::ChallengeId.eq(challenge.id))
            .one(db.get_ref())
            .await?
        {
            // 已存在，直接放进结果
            event_challenges_list.push(existing);
        } else {
            // 不存在，执行插入
            let new_event_challenge = event_challenges::ActiveModel {
                event_id: Set(event.id),
                challenge_id: Set(challenge.id),
                ..Default::default()
            };

            let inserted = new_event_challenge.insert(db.get_ref()).await?;
            event_challenges_list.push(inserted);
        }
    }

    UniResponse::ok(event_challenges_list.into()).into()
}

pub type DeleteChallengeRequest = AddChallengeRequest;
#[delete("")]
pub async fn remove_challenge(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
    dcr: Json<DeleteChallengeRequest>,
) -> UniResult<u64> {
    let dcr = dcr.into_inner();

    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let mut rows_affected = 0;

    if let Some(challenge_id) = dcr.challenge_id {
        let challenge = Challenges::find_by_id(challenge_id)
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

        let event_challenge = EventChallenges::find()
            .filter(event_challenges::Column::EventId.eq(event.id))
            .filter(event_challenges::Column::ChallengeId.eq(challenge.id))
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

        let r = event_challenge.delete(db.get_ref()).await?;
        rows_affected += r.rows_affected;
    }

    if let Some(challenge_id_list) = dcr.challenge_id_list {
        for challenge_id in challenge_id_list {
            let challenge = Challenges::find_by_id(challenge_id)
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

            let event_challenge = EventChallenges::find()
                .filter(event_challenges::Column::EventId.eq(event.id))
                .filter(event_challenges::Column::ChallengeId.eq(challenge.id))
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

            let r = event_challenge.delete(db.get_ref()).await?;
            rows_affected += r.rows_affected;
        }
    }

    UniResponse::ok(rows_affected.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventChallengeResult {
    pub event_challenge: event_challenges::Model,
    pub challenge: challenges::Model,
}

#[get("")]
pub async fn get_challenges(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<Vec<EventChallengeResult>> {
    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let stmt = event
        .find_related(EventChallenges)
        .find_also_related(Challenges);
    // .filter(challenges::Column::Hidden.eq(false)); 并不需要， 比赛题目可以是隐藏的 再说 这是 admin api

    let event_challenges = stmt.all(db.get_ref()).await?;

    let result = event_challenges
        .into_iter()
        .filter_map(|(event_challenge, challenge)| {
            if let Some(challenge) = challenge {
                Some(EventChallengeResult {
                    event_challenge,
                    challenge,
                })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    UniResponse::ok(result.into()).into()
}

pub type HiddenChallengeRequest = AddChallengeRequest;
#[post("/hidden")]
pub async fn hidden_challenges(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
    hcr: Json<HiddenChallengeRequest>,
) -> UniResult<Vec<event_challenges::Model>> {
    let hcr = hcr.into_inner();

    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let mut event_challenges_list = Vec::new();

    if let Some(challenge_id) = hcr.challenge_id {
        let challenge = Challenges::find_by_id(challenge_id)
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

        let event_challenge = EventChallenges::find()
            .filter(event_challenges::Column::EventId.eq(event.id))
            .filter(event_challenges::Column::ChallengeId.eq(challenge.id))
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

        let mut event_challenge: event_challenges::ActiveModel = event_challenge.into();
        event_challenge.hidden = Set(true);

        let event_challenge = event_challenge.update(db.get_ref()).await?;
        event_challenges_list.push(event_challenge);
    }

    if let Some(challenge_id_list) = hcr.challenge_id_list {
        for challenge_id in challenge_id_list {
            let challenge = Challenges::find_by_id(challenge_id)
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

            let event_challenge = EventChallenges::find()
                .filter(event_challenges::Column::EventId.eq(event.id))
                .filter(event_challenges::Column::ChallengeId.eq(challenge.id))
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

            let mut event_challenge: event_challenges::ActiveModel = event_challenge.into();
            event_challenge.hidden = Set(true);

            let event_challenge = event_challenge.update(db.get_ref()).await?;
            event_challenges_list.push(event_challenge);
        }
    }

    UniResponse::ok(event_challenges_list.into()).into()
}

pub type OpenChallengeRequest = AddChallengeRequest;
#[post("/open")]
pub async fn open_challenges(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
    ocr: Json<OpenChallengeRequest>,
) -> UniResult<Vec<event_challenges::Model>> {
    let ocr = ocr.into_inner();

    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let mut event_challenges_list = Vec::new();

    if let Some(challenge_id) = ocr.challenge_id {
        let challenge = Challenges::find_by_id(challenge_id)
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

        let event_challenge = EventChallenges::find()
            .filter(event_challenges::Column::EventId.eq(event.id))
            .filter(event_challenges::Column::ChallengeId.eq(challenge.id))
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

        let mut event_challenge: event_challenges::ActiveModel = event_challenge.into();
        event_challenge.hidden = Set(false);

        let event_challenge = event_challenge.update(db.get_ref()).await?;
        event_challenges_list.push(event_challenge);
    }

    if let Some(challenge_id_list) = ocr.challenge_id_list {
        for challenge_id in challenge_id_list {
            let challenge = Challenges::find_by_id(challenge_id)
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

            let event_challenge = EventChallenges::find()
                .filter(event_challenges::Column::EventId.eq(event.id))
                .filter(event_challenges::Column::ChallengeId.eq(challenge.id))
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

            let mut event_challenge: event_challenges::ActiveModel = event_challenge.into();
            event_challenge.hidden = Set(false);

            let event_challenge = event_challenge.update(db.get_ref()).await?;
            event_challenges_list.push(event_challenge);
        }
    }

    UniResponse::ok(event_challenges_list.into()).into()
}
