use crate::{
    api::preclude::*,
    entity::{event_teams, event_writeup, events},
    strategies::event,
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

    // prepare event
    let event = match sfr.event_id {
        Some(event_id) => events::Entity::find_by_id(event_id)
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound("no event".into()))?
            .into(),
        None => None,
    };

    // 1st prepare ctx
    let ctx = event::EventContextBuilder::new()
        .db(db)
        .docker(docker)
        .user(user)
        .event(event)
        .build()
        .map_err(|e| UniError::CustomError(format!("build event context error: {}", e)))?;

    // 2st chose strategy
    let strategy = event::EventStrategyFactory::create(&ctx.event.r#type);

    // 3st call function
    strategy
        .submit(
            &ctx,
            event::SubmitFlagRequest {
                instance_id: sfr.instance_id,
                flag: sfr.flag,
            },
        )
        .await
        .map_err(|e| UniError::CustomError(format!("submit flag error: {}", e)))?;

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
