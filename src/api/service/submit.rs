use actix_web::HttpRequest;
use sea_orm::{ColumnTrait, QueryFilter};

use super::super::preclude::*;
use crate::{
    api::service::{get_user, instances::__destroy_instance},
    db::WebDocker,
    entity::{
        challenge_solves, challenges, instances,
        prelude::{Challenges, Instances},
        sea_orm_active_enums::InstanceStatus,
        users,
    },
};

#[derive(Debug, Deserialize, Serialize)]
pub struct SubmitFlagRequest {
    // team
    pub event_id: Option<Uuid>,
    pub team_id: Option<Uuid>,
    // single
    pub instance_id: Option<Uuid>,
    // value
    pub flag: String,
}

#[post("/flag")]
pub async fn submit_flag(
    db: WebDb,
    docker: WebDocker,
    sfr: Json<SubmitFlagRequest>,
    request: HttpRequest,
) -> UniResult<challenge_solves::Model> {
    // TODO: Implement submit_flag
    let user = get_user(&db, &request).await?;
    let sfr = sfr.into_inner();

    // practice
    // event_single
    // event_team

    if sfr.event_id.is_none() && sfr.team_id.is_none() {
        jeopardy_single_practice_handler(db, docker, sfr, user).await
    } else if sfr.event_id.is_some() && sfr.team_id.is_none() {
        jeopardy_event_single_submit_handler(db, docker, sfr, user).await
    } else if sfr.event_id.is_some() && sfr.team_id.is_some() {
        jeopardy_event_team_submit_handler(db, docker, sfr, user).await
    } else {
        UniError::InternalError("unimplemented!".into()).into()
    }
}

pub async fn jeopardy_single_practice_handler(
    db: WebDb,
    docker: WebDocker,
    sfr: SubmitFlagRequest,
    user: users::Model,
) -> UniResult<challenge_solves::Model> {
    let instance_id = sfr
        .instance_id
        .ok_or(UniError::NotFound("no instance_id".into()))?;

    let instance = Instances::find_by_id(instance_id)
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .filter(instances::Column::UserId.eq(user.id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("no instance".into()))?;

    let challenge = Challenges::find_by_id(instance.challenge_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("no challenge".into()))?;

    if sfr.flag != instance.flag {
        return UniError::CustomError("flag is not correct".into()).into();
    }

    __destroy_instance(db.clone(), docker, instance_id, user.clone()).await?;

    let challenge_solve = {
        let old_challenge_solve = challenge_solves::Entity::find()
            .filter(challenge_solves::Column::ChallengeId.eq(challenge.id))
            .filter(challenge_solves::Column::UserId.eq(user.id))
            .one(db.get_ref())
            .await?;

        match old_challenge_solve {
            Some(challenge_solve) => challenge_solve,
            None => {
                challenge_solves::ActiveModel {
                    event_id: Set(sfr.event_id),
                    challenge_id: Set(challenge.id),
                    user_id: Set(user.id),
                    ..Default::default()
                }
                .insert(db.get_ref())
                .await?
            }
        }
    };

    UniResponse::ok(challenge_solve.into()).into()
}

pub async fn jeopardy_event_single_submit_handler(
    db: WebDb,
    docker: WebDocker,
    sfr: SubmitFlagRequest,
    user: users::Model,
) -> UniResult<challenge_solves::Model> {
    // challenge_solves
    // challenge_id, user_id, created_at
    // event_challenge_solves event_id, challenge_id, user_id, created_at
    unimplemented!()
}

pub async fn jeopardy_event_team_submit_handler(
    db: WebDb,
    docker: WebDocker,
    sfr: SubmitFlagRequest,
    user: users::Model,
) -> UniResult<challenge_solves::Model> {
    // challenge_solves
    // challenge_id, user_id, created_at
    // event_challenge_solves event_id, challenge_id, user_id, created_at
    unimplemented!()
}

// now just for the event
#[post("writeup")]
pub async fn submit_writeup(db: WebDb) -> UniResult<()> {
    // TODO: Implement submit_writeup
    unimplemented!()
}
