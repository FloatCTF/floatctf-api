use base64::write;
use sea_orm::{ColumnTrait, QueryFilter};

use super::super::preclude::*;
use crate::{
    auth::UserJwtGuard,
    entity::{
        challenge_writeup, challenges, instances,
        prelude::{Challenges, Instances},
        sea_orm_active_enums::InstanceStatus,
        users::{self, Entity},
    },
};

#[get("/{id}/my_writeup")]
pub async fn get_challenge_writeup(
    user: UserJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<challenge_writeup::Model> {
    let user = user.into_inner();

    let writeup = challenge_writeup::Entity::find()
        .filter(challenge_writeup::Column::ChallengeId.eq(*id))
        .filter(challenge_writeup::Column::UserId.eq(user.id))
        .one(db.get_ref())
        .await?;

    match writeup {
        Some(writeup) => UniResponse::ok(writeup.into()).into(),
        None => UniError::NotFound(format!("Writeup for challenge {} not found", id)).into(),
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct CreateChallengeWriteup {
    pub content: String,
}

#[post("/{id}/my_writeup")]
pub async fn create_challenge_writeup(
    user: UserJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
    ccw: Json<CreateChallengeWriteup>,
) -> UniResult<challenge_writeup::Model> {
    let user = user.into_inner();
    let ccw = ccw.into_inner();

    // 查找是否存在
    let existing = challenge_writeup::Entity::find()
        .filter(challenge_writeup::Column::ChallengeId.eq(*id))
        .filter(challenge_writeup::Column::UserId.eq(user.id))
        .one(db.get_ref())
        .await?;

    let wp = match existing {
        Some(wp) => {
            let mut active = wp.into_active_model();
            active.content = Set(ccw.content);
            active.created_at = Set(chrono::Utc::now().naive_utc());
            active.update(db.get_ref()).await?
        }
        None => {
            let active = challenge_writeup::ActiveModel {
                challenge_id: Set(*id),
                user_id: Set(user.id),
                content: Set(ccw.content),
                ..Default::default()
            };
            active.insert(db.get_ref()).await?
        }
    };

    Ok(UniResponse::ok(wp.into()).into())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChallengeWriteupResult {
    pub nickname: String,
    pub email: String,
    pub challenge: challenges::Model,
    pub writeup: challenge_writeup::Model,
}
#[get("/{id}/writeups")]
pub async fn get_challenge_writeups(
    _user: UserJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<Vec<ChallengeWriteupResult>> {
    let writeups = challenge_writeup::Entity::find()
        .filter(challenge_writeup::Column::ChallengeId.eq(*id))
        .find_also_related(challenges::Entity)
        .all(db.get_ref())
        .await?;

    let mut results = Vec::new();

    for (writeup, challenge) in writeups {
        let user = users::Entity::find_by_id(writeup.user_id)
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(
                "User {} not found",
                writeup.user_id
            )))?;

        let result = ChallengeWriteupResult {
            nickname: user.nickname,
            email: user.email,
            challenge: challenge.ok_or(UniError::NotFound(format!(
                "Challenge {} not found",
                writeup.challenge_id
            )))?,
            writeup,
        };

        results.push(result);
    }

    UniResponse::ok(results.into()).into()
}

// get writeup by id
// for /writeups/{id}
#[get("/{id}")]
pub async fn get_writeup(
    _user: UserJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<ChallengeWriteupResult> {
    let writeup = challenge_writeup::Entity::find_by_id(*id)
        .find_also_related(challenges::Entity)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!("Writeup {} not found", id)))?;

    let (writeup, challenge) = writeup;

    let user = users::Entity::find_by_id(writeup.user_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(
            "User {} not found",
            writeup.user_id
        )))?;

    let result = ChallengeWriteupResult {
        nickname: user.nickname,
        email: user.email,
        challenge: challenge.ok_or(UniError::NotFound(format!(
            "Challenge {} not found",
            writeup.challenge_id
        )))?,
        writeup,
    };

    UniResponse::ok(result.into()).into()
}

#[get("")]
pub async fn get_writeups(
    _user: UserJwtGuard,
    db: WebDb,
) -> UniResult<Vec<ChallengeWriteupResult>> {
    let writeups = challenge_writeup::Entity::find()
        .find_also_related(challenges::Entity)
        .all(db.get_ref())
        .await?;

    let mut results = Vec::new();

    for (writeup, challenge) in writeups {
        let user = users::Entity::find_by_id(writeup.user_id)
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(
                "User {} not found",
                writeup.user_id
            )))?;

        let result = ChallengeWriteupResult {
            nickname: user.nickname,
            email: user.email,
            challenge: challenge.ok_or(UniError::NotFound(format!(
                "Challenge {} not found",
                writeup.challenge_id
            )))?,
            writeup,
        };

        results.push(result);
    }

    UniResponse::ok(results.into()).into()
}
