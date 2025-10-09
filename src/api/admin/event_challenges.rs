use crate::{
    api::preclude::*,
    entity::{challenges, event_challenges, events},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct AddChallengeRequest {
    pub challenge_id: Option<Uuid>,
    pub challenge_id_list: Option<Vec<Uuid>>,
}

/// POST /api/admin/events/{event_id}/challenges
#[post("")]
pub async fn add_challenge(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    event_id: Path<Uuid>,
    acr: Json<AddChallengeRequest>,
) -> UniResult<Vec<event_challenges::Model>> {
    let acr = acr.into_inner();
    let event_id = event_id.into_inner();

    let event = events::Entity::find_by_id(event_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!("event {} not exist", event_id)))?;

    let mut event_challenges_list = Vec::new();

    // 把单个 id 和多个 id 合并成一个 Vec
    let challenge_ids: Vec<Uuid> = acr
        .challenge_id
        .into_iter()
        .chain(acr.challenge_id_list.unwrap_or_default())
        .collect();

    for challenge_id in challenge_ids {
        // 先检查 challenge 是否存在
        let challenge = challenges::Entity::find_by_id(challenge_id)
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
            let points = {
                match toml::from_str::<toml::Value>(&challenge.toml_str) {
                    // 只有 添加到 event_challenges 才会有 points
                    // 所以这里的 points 是从 challenge.toml_str 中解析出来的
                    Ok(value) => value
                        .get("points")
                        .and_then(|v| v.as_float())
                        .unwrap_or(0.0) as f64,
                    Err(_err) => {
                        println!("Error parsing TOML: {}", _err);
                        100 as f64
                    }
                }
            };
            let new_event_challenge = event_challenges::ActiveModel {
                event_id: Set(event.id),
                challenge_id: Set(challenge.id),
                points: Set(points),
                ..Default::default()
            };

            let inserted = new_event_challenge.insert(db.get_ref()).await?;
            event_challenges_list.push(inserted);
        }
    }

    UniResponse::ok(event_challenges_list.into()).into()
}

pub type DeleteChallengeRequest = AddChallengeRequest;
/// DELETE /api/admin/events/{event_id}/challenges
#[delete("")]
pub async fn remove_challenge(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    event_id: Path<Uuid>,
    dcr: Json<DeleteChallengeRequest>,
) -> UniResult<u64> {
    let dcr = dcr.into_inner();
    let event_id = event_id.into_inner();
    let event = events::Entity::find_by_id(event_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", event_id)))?;

    let mut rows_affected = 0;

    if let Some(challenge_id) = dcr.challenge_id {
        let challenge = challenges::Entity::find_by_id(challenge_id)
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

        let event_challenge = event_challenges::Entity::find()
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
            let challenge = challenges::Entity::find_by_id(challenge_id)
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

            let event_challenge = event_challenges::Entity::find()
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

/// GET /api/admin/events/{event_id}/challenges
#[get("")]
pub async fn get_challenges(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    event_id: Path<Uuid>,
) -> UniResult<Vec<EventChallengeResult>> {
    let event_id = event_id.into_inner();

    let event = events::Entity::find_by_id(event_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", event_id)))?;

    let stmt = event
        .find_related(event_challenges::Entity)
        .find_also_related(challenges::Entity);
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

/// POST /api/admin/events/{event_id}/challenges/hidden
#[post("/hidden")]
pub async fn hidden_challenges(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    event_id: Path<Uuid>,
    hcr: Json<HiddenChallengeRequest>,
) -> UniResult<Vec<event_challenges::Model>> {
    let hcr = hcr.into_inner();
    let event_id = event_id.into_inner();
    let event = events::Entity::find_by_id(event_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", event_id)))?;

    let mut event_challenges_list = Vec::new();

    if let Some(challenge_id) = hcr.challenge_id {
        let challenge = challenges::Entity::find_by_id(challenge_id)
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

        let event_challenge = event_challenges::Entity::find()
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
            let challenge = challenges::Entity::find_by_id(challenge_id)
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

            let event_challenge = event_challenges::Entity::find()
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
/// POST /api/admin/events/{event_id}/challenges/open
#[post("/open")]
pub async fn open_challenges(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    event_id: Path<Uuid>,
    ocr: Json<OpenChallengeRequest>,
) -> UniResult<Vec<event_challenges::Model>> {
    let ocr = ocr.into_inner();
    let event_id = event_id.into_inner();

    let event = events::Entity::find_by_id(event_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", event_id)))?;

    let mut event_challenges_list = Vec::new();

    if let Some(challenge_id) = ocr.challenge_id {
        let challenge = challenges::Entity::find_by_id(challenge_id)
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

        let event_challenge = event_challenges::Entity::find()
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
            let challenge = challenges::Entity::find_by_id(challenge_id)
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

            let event_challenge = event_challenges::Entity::find()
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
