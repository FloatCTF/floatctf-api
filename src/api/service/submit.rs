use actix_multipart::form::{MultipartForm, tempfile::TempFile, text::Text};

use actix_web::HttpRequest;
use sea_orm::{ColumnTrait, QueryFilter};

use super::super::preclude::*;
use crate::{
    api::service::{calculate_next_dynamic_score, instances::__destroy_instance},
    auth::UserJwtGuard,
    db::WebDocker,
    entity::{
        challenge_solves, challenges, event_challenge_solves, event_users, instances,
        prelude::{Challenges, EventChallengeSolves, EventChallenges, EventUsers, Instances},
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
    user: UserJwtGuard,
    db: WebDb,
    docker: WebDocker,
    sfr: Json<SubmitFlagRequest>,
) -> UniResult<()> {
    // TODO: Implement submit_flag
    let user = user.into_inner();
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
) -> UniResult<()> {
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
    };

    UniResponse::ok_none().into()
}

pub async fn jeopardy_event_single_submit_handler(
    db: WebDb,
    docker: WebDocker,
    sfr: SubmitFlagRequest,
    user: users::Model,
) -> UniResult<()> {
    // challenge_solves
    // challenge_id, user_id, created_at
    // event_challenge_solves event_id, challenge_id, user_id, created_at
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
    //  add points add solves

    let old_challenge_solve = event_challenge_solves::Entity::find()
        .filter(event_challenge_solves::Column::EventId.eq(sfr.event_id))
        .filter(event_challenge_solves::Column::ChallengeId.eq(challenge.id))
        .filter(event_challenge_solves::Column::UserId.eq(user.id))
        .one(db.get_ref())
        .await?;

    match old_challenge_solve {
        Some(challenge_solve) => challenge_solve,
        None => {
            let event_challenge =
                EventChallenges::find_by_id((sfr.event_id.unwrap(), challenge.id))
                    .one(db.get_ref())
                    .await?
                    .ok_or(UniError::NotFound("no event_challenge".into()))?;

            let solved_count = EventChallengeSolves::find()
                .filter(event_challenge_solves::Column::EventId.eq(sfr.event_id.unwrap()))
                .filter(event_challenge_solves::Column::ChallengeId.eq(challenge.id))
                .count(db.get_ref())
                .await?;

            //  更新分数
            let current_points = calculate_next_dynamic_score(event_challenge.points, solved_count);
            let event_user = EventUsers::find_by_id((sfr.event_id.unwrap(), user.id))
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound("no event_user".into()))?;
            let new_points = event_user.points + current_points;

            let mut event_user = event_user.into_active_model();
            event_user.points = Set(new_points);
            event_user.update(db.get_ref()).await?;

            event_challenge_solves::ActiveModel {
                event_id: Set(sfr.event_id.unwrap()),
                challenge_id: Set(challenge.id),
                user_id: Set(user.id),
                bonus_points: Set(current_points),
                ..Default::default()
            }
            .insert(db.get_ref())
            .await?
        }
    };

    UniResponse::ok_none().into()
}

pub async fn jeopardy_event_team_submit_handler(
    db: WebDb,
    docker: WebDocker,
    sfr: SubmitFlagRequest,
    user: users::Model,
) -> UniResult<()> {
    // challenge_solves
    // challenge_id, user_id, created_at
    // event_challenge_solves event_id, challenge_id, user_id, created_at
    unimplemented!()
}
#[derive(Debug, MultipartForm)]
struct WriteupForm {
    #[multipart(limit = "1024MB")]
    writeup_docx: Option<TempFile>,
    event_id: Text<Uuid>,
    team_id: Option<Text<Uuid>>,
}

// now just for the event
#[post("writeup")]
pub async fn submit_writeup(
    user: UserJwtGuard,
    db: WebDb,
    MultipartForm(form): MultipartForm<WriteupForm>,
) -> UniResult<()> {
    // TODO: Implement submit_writeup
    unimplemented!()
}
