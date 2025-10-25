use crate::{
    api::{
        preclude::*,
        service::{
            calculate_next_dynamic_score, events::EventStatus, instances::__destroy_instance,
        },
    },
    entity::{
        challenge_solves, challenges, event_challenge_solves, event_challenges, event_team_members,
        event_teams, event_users, event_writeup, events, instances,
        sea_orm_active_enums::{EventType, InstanceStatus},
        users,
    },
};
use actix_multipart::form::{MultipartForm, tempfile::TempFile, text::Text};
use std::{fs, os::unix::fs::PermissionsExt};

#[derive(Debug, Deserialize, Serialize)]
pub struct SubmitFlagRequest {
    pub event_id: Option<Uuid>,
    // single
    pub instance_id: Option<Uuid>,
    // value
    pub flag: String,
}

/// POST /api/submit/flag
#[post("/flag")]
pub async fn submit_flag(
    user: UserJwtGuard,
    db: WebDb,
    docker: WebDocker,
    sfr: Json<SubmitFlagRequest>,
) -> UniResult<()> {
    let user = user.into_inner();
    let mut sfr = sfr.into_inner();
    sfr.flag = sfr.flag.trim().to_string();

    match sfr.event_id {
        Some(event_id) => {
            let event = events::Entity::find_by_id(event_id)
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound("no event".into()))?;

            match EventStatus::check(&db, &event_id).await? {
                EventStatus::Ended | EventStatus::NotStarted => {
                    return Err(UniError::CustomError("Event is no ongoing".to_string()));
                }
                EventStatus::Ongoing => {}
            }

            match event.r#type {
                EventType::JeopardySingle => {
                    return jeopardy_event_single_submit_handler(db, docker, sfr, user).await;
                }
                EventType::JeopardyTeam => {
                    return jeopardy_event_team_submit_handler(db, docker, sfr, user).await;
                }
                _ => return UniError::InternalError("unimplemented!".into()).into(),
            }
        }
        None => jeopardy_single_practice_handler(db, docker, sfr, user).await,
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

    let instance = instances::Entity::find_by_id(instance_id)
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("no instance".into()))?;

    let challenge = challenges::Entity::find_by_id(instance.challenge_id)
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

    let instance = instances::Entity::find_by_id(instance_id)
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("no instance".into()))?;

    let challenge = challenges::Entity::find_by_id(instance.challenge_id)
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
                event_challenges::Entity::find_by_id((sfr.event_id.unwrap(), challenge.id))
                    .one(db.get_ref())
                    .await?
                    .ok_or(UniError::NotFound("no event_challenge".into()))?;

            let solved_count = event_challenge_solves::Entity::find()
                .filter(event_challenge_solves::Column::EventId.eq(sfr.event_id.unwrap()))
                .filter(event_challenge_solves::Column::ChallengeId.eq(challenge.id))
                .count(db.get_ref())
                .await?;

            //  更新分数
            let current_points =
                calculate_next_dynamic_score(&db, event_challenge.points, solved_count)
                    .await
                    .map_err(|e| {
                        UniError::CustomError(format!("calculate_next_dynamic_score error: {}", e))
                    })?;
            let event_user = event_users::Entity::find_by_id((sfr.event_id.unwrap(), user.id))
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound("no event_user".into()))?;

            if event_user.banned {
                // banned!
                return UniError::CustomError("you are banned".into()).into();
            }

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
    let team_member = event_team_members::Entity::find()
        .filter(event_team_members::Column::EventId.eq(sfr.event_id.unwrap()))
        .filter(event_team_members::Column::UserId.eq(user.id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("you are not in any team".into()))?;

    let instance_id = sfr
        .instance_id
        .ok_or(UniError::NotFound("no instance_id".into()))?;

    let instance = instances::Entity::find_by_id(instance_id)
        .filter(instances::Column::Status.eq(InstanceStatus::Running))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("no instance".into()))?;

    let challenge = challenges::Entity::find_by_id(instance.challenge_id)
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
        .filter(event_challenge_solves::Column::TeamId.eq(team_member.team_id))
        .one(db.get_ref())
        .await?;

    match old_challenge_solve {
        Some(challenge_solve) => challenge_solve,
        None => {
            let event_challenge =
                event_challenges::Entity::find_by_id((sfr.event_id.unwrap(), challenge.id))
                    .one(db.get_ref())
                    .await?
                    .ok_or(UniError::NotFound("no event_challenge".into()))?;

            let solved_count = event_challenge_solves::Entity::find()
                .filter(event_challenge_solves::Column::EventId.eq(sfr.event_id.unwrap()))
                .filter(event_challenge_solves::Column::ChallengeId.eq(challenge.id))
                .count(db.get_ref())
                .await?;

            //  更新分数
            let current_points =
                calculate_next_dynamic_score(&db, event_challenge.points, solved_count)
                    .await
                    .map_err(|e| {
                        UniError::CustomError(format!("calculate_next_dynamic_score error: {}", e))
                    })?;
            let event_team = event_teams::Entity::find_by_id(team_member.team_id)
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound("no event_team".into()))?;

            if event_team.banned {
                // banned!
                return UniError::CustomError("you are banned".into()).into();
            }

            let new_points = event_team.points + current_points;

            let mut event_team = event_team.into_active_model();
            event_team.points = Set(new_points);
            event_team.update(db.get_ref()).await?;

            event_challenge_solves::ActiveModel {
                event_id: Set(sfr.event_id.unwrap()),
                challenge_id: Set(challenge.id),
                team_id: Set(Some(team_member.team_id)),
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
#[derive(Debug, MultipartForm)]
pub struct WriteupForm {
    #[multipart(limit = "1024MB")]
    writeup_pdf: TempFile,
    event_id: Text<Uuid>,
    team_id: Option<Text<Uuid>>,
}

// now just for the event
/// POST /api/submit/writeup
#[post("writeup")]
pub async fn submit_writeup(
    user: UserJwtGuard,
    db: WebDb,
    MultipartForm(form): MultipartForm<WriteupForm>,
) -> UniResult<()> {
    let upload_dir = get_setting(&db, "UPLOAD_DIR")
        .await
        .map_err(|e| UniError::CustomError(format!("get upload dir error: {}", e)))?;
    // if not exists, create it
    if !fs::metadata(&upload_dir).is_ok() {
        fs::create_dir_all(&upload_dir).unwrap();
    }
    let user = user.into_inner();

    let event_id = form.event_id.into_inner();

    // 写入文件
    let writeup_file = form.writeup_pdf;
    let team_id = form.team_id.map(|x| x.into_inner());
    let writeup_file_name = {
        if let Some(team_id) = team_id.clone() {
            let team = event_teams::Entity::find_by_id(team_id)
                .one(db.get_ref())
                .await?
                .ok_or(UniError::NotFound("no team".into()))?;
            format!("{}_{}_{}.pdf", event_id, team.name, user.nickname)
        } else {
            format!("{}_{}.pdf", event_id, user.nickname)
        }
    };

    let writeup_file_path = format!("{}/{}", upload_dir, writeup_file_name);
    let writeup_file_path = std::path::Path::new(&writeup_file_path);

    // copy 会覆盖旧文件
    std::fs::copy(writeup_file.file.path(), &writeup_file_path)
        .map_err(|e| UniError::InternalError(format!("Failed to copy writeup file: {}", e)))?;
    std::fs::set_permissions(&writeup_file_path, std::fs::Permissions::from_mode(0o644))
        .map_err(|e| UniError::InternalError(format!("Failed to set permissions: {}", e)))?;

    // 插入或更新数据库
    use sea_orm::sea_query::OnConflict;

    event_writeup::Entity::insert(event_writeup::ActiveModel {
        event_id: Set(event_id),
        user_id: Set(user.id),
        team_id: Set(team_id),
        file_url: Set(writeup_file_path.to_str().unwrap().to_string()),
        ..Default::default()
    })
    .on_conflict(
        OnConflict::columns([
            event_writeup::Column::EventId,
            event_writeup::Column::UserId,
        ])
        .update_columns([
            event_writeup::Column::FileUrl,
            event_writeup::Column::TeamId,
        ])
        .to_owned(),
    )
    .exec(db.get_ref())
    .await?;

    UniResponse::ok_none().into()
}
