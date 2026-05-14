use crate::{
    api::prelude::*,
    entity::{event_teams, event_writeup, events},
    prelude::*,
    strategies::event,
};
use actix_multipart::form::{MultipartForm, tempfile::TempFile, text::Text};
use aws_sdk_s3::primitives::ByteStream;

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
    ctx: ReqCtx,
    sfr: Json<SubmitFlagRequest>,
) -> UniResult<()> {
    let user = user.into_inner();
    let mut sfr = sfr.into_inner();
    sfr.flag = sfr.flag.trim().to_string();

    // prepare event
    let event = match sfr.event_id {
        Some(event_id) => events::Entity::find_by_id(event_id)
            .one(ctx.db.get_ref())
            .await?
            .ok_or(UniError::NotFound("no event".into()))?
            .into(),
        None => None,
    };

    // 1st prepare event_ctx
    let event_ctx = event::EventContextBuilder::new()
        .db(ctx.db)
        .docker(ctx.docker)
        .user(user)
        .event(event)
        .build()
        .await
        .map_err(|e| UniError::CustomError(format!("build event context error: {}", e)))?;

    // 2st chose strategy
    let strategy = event::EventStrategyFactory::create(&event_ctx.event.r#type);

    // 3st call function
    strategy
        .submit(
            &event_ctx,
            event::SubmitFlagRequest {
                instance_id: sfr.instance_id,
                flag: sfr.flag.clone(),
            },
        )
        .await
        .map_err(|e| UniError::CustomError(format!("submit flag error: {}", e)))?;

    if let Some(_event_id) = sfr.event_id {
        ctx.log
            .add_event_log(
                &event_ctx.event,
                "INFO",
                "SUBMIT_FLAG",
                json!({"flag": sfr.flag, "instance_id": sfr.instance_id}),
                Some(event_ctx.user.id),
                event_ctx.team.as_ref().map(|t| t.id),
                Some(&ctx.req),
            )
            .await;
    } else {
        ctx.log
            .add_log(
                "INFO",
                "SUBMIT",
                "SUBMIT_FLAG",
                format!("提交 Flag: {}", sfr.flag).as_str(),
                json!({"instance_id": sfr.instance_id}),
                event_ctx.user.id.into(),
                None,
                Some(&ctx.req),
            )
            .await;
    }

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
    ctx: ReqCtx,
    MultipartForm(form): MultipartForm<WriteupForm>,
) -> UniResult<()> {
    let user = user.into_inner();

    let event_id = form.event_id.into_inner();

    // 写入文件
    let writeup_file = form.writeup_pdf;
    let team_id = form.team_id.map(|x| x.into_inner());
    let writeup_file_name = {
        if let Some(team_id) = team_id.clone() {
            let team = event_teams::Entity::find_by_id(team_id)
                .one(ctx.db.get_ref())
                .await?
                .ok_or(UniError::NotFound("no team".into()))?;
            format!("{}/{}/{}.pdf", event_id, team.id, team.name)
        } else {
            format!("{}/{}/{}.pdf", event_id, user.id, user.nickname)
        }
    };

    let s3_key = format!("writeups/{}", writeup_file_name);

    let body = ByteStream::from(
        tokio::fs::read(&writeup_file.file.path())
            .await
            .map_err(|e| UniError::InternalError(format!("Failed to read writeup file: {}", e)))?,
    );

    ctx.rustfs
        .put_object()
        .bucket("floatctf-private")
        .key(&s3_key)
        .body(body)
        .content_type("application/pdf")
        .send()
        .await
        .map_err(|e| UniError::InternalError(format!("Failed to upload writeup to S3: {}", e)))?;

    // 插入或更新数据库
    use sea_orm::sea_query::OnConflict;

    event_writeup::Entity::insert(event_writeup::ActiveModel {
        event_id: Set(event_id),
        user_id: Set(user.id),
        team_id: Set(team_id),
        file_url: Set(s3_key),
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
    .exec(ctx.db.get_ref())
    .await?;

    let event = events::Entity::find_by_id(event_id)
        .one(ctx.db.get_ref())
        .await?
        .ok_or(UniError::NotFound("event not found".to_string()))?;

    ctx.log
        .add_event_log(
            &event,
            "INFO",
            "SUBMIT_WRITEUP",
            json!({"team_id": team_id}),
            Some(user.id),
            team_id,
            Some(&ctx.req),
        )
        .await;

    UniResponse::ok_none().into()
}
