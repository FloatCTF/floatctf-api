use crate::{
    api::{FilterMapping, admin::dto::DeleteItemsRequest, prelude::*, sea_orm_utils::query_query},
    auth::SuperAdminJwtGuard,
    config::get_setting,
    db::WebDocker,
    entity::{challenges, prelude::Challenges},
    prelude::*,
};
use actix_multipart::form::{MultipartForm, tempfile::TempFile, text::Text};
use base64::Engine;
use fcmc::ChallengeMeta;

use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, Set,
    sea_query::OnConflict,
};
use std::{io::Read, str::FromStr};
use tempfile::NamedTempFile;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateChallengeRequest {
    pub name: String,
    pub category: String,
    pub description: String,
    pub hidden: bool,
    pub attachment: Option<String>,
    pub toml_str: String,
}
// POST /api/admin/challenges
#[post("")]
pub async fn create_challenge(
    user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    ccr: Json<CreateChallengeRequest>,
) -> UniResult<challenges::Model> {
    let user = user.into_inner();
    let ccr = ccr.into_inner();

    let new_challenge = challenges::ActiveModel {
        name: Set(ccr.name),
        category: Set(ccr.category),
        description: Set(ccr.description),
        attachment: Set(ccr.attachment),
        toml_str: Set(ccr.toml_str),
        hidden: Set(ccr.hidden),
        ..Default::default()
    };

    let challenge = new_challenge.insert(ctx.db.get_ref()).await?;

    ctx.log
        .add_log(
            "INFO",
            "CHALLENGES",
            "CREATE",
            format!("{} 创建题目: {}", user.username, challenge.name).as_str(),
            json!({"name": challenge.name, "category": challenge.category}),
            None,
            user.id.into(),
            Some(&ctx.req),
        )
        .await;

    UniResponse::ok(challenge.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatchChallengeRequest {
    pub name: Option<String>,
    pub category: Option<String>,
    pub description: Option<String>,
    pub attachment: Option<String>,
    pub hidden: Option<bool>,
    pub toml_str: Option<String>,
}
/// PATCH /api/admin/challenges/{challenge_id}
#[patch("/{challenge_id}")]
pub async fn patch_challenge(
    user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    pcr: Json<PatchChallengeRequest>,
    challenge_id: Path<Uuid>,
) -> UniResult<challenges::Model> {
    let user = user.into_inner();
    let pcr = pcr.into_inner();
    let challenge_id = challenge_id.into_inner();
    let challenge = Challenges::find_by_id(challenge_id)
        .one(ctx.db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

    let mut m_challenge = challenge.into_active_model();

    pcr.name.map(|n| {
        m_challenge.name = Set(n);
    });

    pcr.category.map(|c| {
        m_challenge.category = Set(c);
    });

    pcr.description.map(|d| {
        m_challenge.description = Set(d);
    });

    pcr.attachment.map(|a| {
        m_challenge.attachment = Set(a.into());
    });

    pcr.hidden.map(|h| {
        m_challenge.hidden = Set(h);
    });

    pcr.toml_str.map(|t| m_challenge.toml_str = Set(t));
    m_challenge.updated_at = Set(Utc::now().into());

    let challenge = m_challenge.update(ctx.db.get_ref()).await?;

    ctx.log
        .add_log(
            "INFO",
            "CHALLENGES",
            "UPDATE",
            format!("{} 更新题目: {}", user.username, challenge.name).as_str(),
            json!({"challenge_id": challenge.id}),
            None,
            user.id.into(),
            Some(&ctx.req),
        )
        .await;

    UniResponse::ok(challenge.into()).into()
}

/// GET /api/admin/challenges
#[get("")]
pub async fn get_challenges(
    _user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<challenges::Model>> {
    let mut query_params = query_params.0;

    let mappings = [
        FilterMapping {
            key: "id",
            column: Box::new(|v| {
                Condition::all()
                    .add(challenges::Column::Id.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())))
            }),
        },
        FilterMapping {
            key: "name",
            column: Box::new(|v| Condition::all().add(challenges::Column::Name.contains(v))),
        },
        FilterMapping {
            key: "category",
            column: Box::new(|v| Condition::all().add(challenges::Column::Category.contains(v))),
        },
        FilterMapping {
            key: "hidden",
            column: Box::new(|v| {
                Condition::all()
                    .add(challenges::Column::Hidden.eq(v.parse::<bool>().unwrap_or(true)))
            }),
        },
        FilterMapping {
            key: "description",
            column: Box::new(|v| Condition::all().add(challenges::Column::Description.contains(v))),
        },
    ];
    let (items, total_items) = query_query::<challenges::Entity>(
        ctx.db.get_ref(),
        &mappings,
        &query_params,
        Some(Box::new(|stmt| {
            stmt.order_by_desc(challenges::Column::UpdatedAt)
        })),
    )
    .await?;

    query_params.total = Some(total_items);

    UniResponse::ok_meta(items.into(), query_params.into()).into()
}

/// GET /api/admin/challenges/{challenge_id}
#[get("/{challenge_id}")]
pub async fn get_challenge(
    _user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    id: Path<Uuid>,
) -> UniResult<challenges::Model> {
    let model = Challenges::find_by_id(*id)
        .one(ctx.db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    UniResponse::ok(model.into()).into()
}

/// DELETE /api/admin/challenges
#[delete("")]
pub async fn delete_challenge(
    user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    dir: Json<DeleteItemsRequest>,
) -> UniResult<u64> {
    let user = user.into_inner();
    let dir = dir.into_inner();

    let mut deleted_count = 0;
    let challenges_path = get_setting(&ctx.db, "CHALLENGES_DIR")
        .await
        .map_err(|e| UniError::CustomError(format!("get setting error: {}", e)))?;

    for challenge_id in dir.id_list {
        let challenge = Challenges::find_by_id(challenge_id)
            .one(ctx.db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

        let del_challenge_path = std::path::Path::new(&challenges_path).join(&challenge.safe_name);
        if del_challenge_path.exists() {
            std::fs::remove_dir_all(&del_challenge_path)
                .map_err(|e| UniError::CustomError(format!("delete challenge dir error: {}", e)))?;
        }
        let r = challenge.delete(ctx.db.get_ref()).await?;
        deleted_count += r.rows_affected;
    }

    ctx.log
        .add_log(
            "INFO",
            "CHALLENGES",
            "DELETE",
            format!("{} 删除 {} 道题目", user.username, deleted_count).as_str(),
            json!({"deleted_count": deleted_count}),
            None,
            user.id.into(),
            Some(&ctx.req),
        )
        .await;

    UniResponse::ok(deleted_count.into()).into()
}

#[derive(Debug, MultipartForm)]
struct UploadForm {
    #[multipart(limit = "1024MB")]
    challenge_zip: Option<TempFile>,
    #[multipart(limit = "10240MB")]
    challenge_list_zip: Option<TempFile>,
    toml_str_b64: Option<Text<String>>,
}
/// POST /api/admin/challenges/import
#[post("/import")]
pub async fn web_import_challenge(
    user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    MultipartForm(form): MultipartForm<UploadForm>,
) -> UniResult<Vec<challenges::Model>> {
    let user = user.into_inner();
    let mut will_insert_toml_strs = Vec::new();
    let mut inserted_challenges = Vec::new();

    if let Some(s) = form.toml_str_b64 {
        let toml_str = base64::prelude::BASE64_STANDARD
            .decode(s.0)
            .map_err(|e| UniError::CustomError(format!("base64 decode error: {}", e)))?;

        let toml_str = String::from_utf8(toml_str)
            .map_err(|e| UniError::CustomError(format!("utf8 decode error: {}", e)))?;

        will_insert_toml_strs.push(toml_str);
    }

    if let Some(challenge_zip) = form.challenge_zip {
        let toml_str = import_challenge_zip(&ctx.db, challenge_zip.file)
            .await
            .map_err(|e| UniError::CustomError(format!("import challenge zip error: {}", e)))?;

        will_insert_toml_strs.push(toml_str);
    }

    if let Some(challenge_list_zip) = form.challenge_list_zip {
        let toml_strs = import_challenge_list_zip(&ctx.db, challenge_list_zip.file)
            .await
            .map_err(|e| {
                UniError::CustomError(format!("import challenge list zip error: {}", e))
            })?;

        will_insert_toml_strs.extend(toml_strs);
    }

    for toml_str in will_insert_toml_strs {
        let challenge = import_challenge(ctx.db.get_ref(), toml_str)
            .await
            .map_err(|e| UniError::CustomError(format!("import challenge error: {}", e)))?;

        inserted_challenges.push(challenge);
    }

    ctx.log
        .add_log(
            "INFO",
            "CHALLENGES",
            "IMPORT",
            format!(
                "{} 导入 {} 道题目",
                user.username,
                inserted_challenges.len()
            )
            .as_str(),
            json!({"count": inserted_challenges.len()}),
            None,
            user.id.into(),
            Some(&ctx.req),
        )
        .await;

    UniResponse::ok(inserted_challenges.into()).into()
}

pub async fn import_challenge(
    db: &DatabaseConnection,
    challenge_toml_str: String,
) -> anyhow::Result<challenges::Model> {
    let c = ChallengeMeta::from_toml_str(&challenge_toml_str)?;

    let new_challenge = challenges::ActiveModel {
        name: Set(c.name.clone()),
        category: Set(c.category),
        description: Set(c.description),
        attachment: Set(c.attachment),
        safe_name: Set(generate_safe_name(&c.name)),
        toml_str: Set(challenge_toml_str),
        ..Default::default()
    };

    // 关键：按 name 唯一键 UPSERT（存在则覆盖更新）
    challenges::Entity::insert(new_challenge)
        .on_conflict(
            OnConflict::column(challenges::Column::Name)
                .update_columns([
                    challenges::Column::Category,
                    challenges::Column::Description,
                    challenges::Column::Attachment,
                    challenges::Column::TomlStr,
                    challenges::Column::SafeName,
                ])
                .to_owned(),
        )
        .exec(db)
        .await?;

    // 返回最新记录（无论是插入还是更新）
    let model = challenges::Entity::find()
        .filter(challenges::Column::Name.eq(c.name))
        .one(db)
        .await?
        .ok_or_else(|| anyhow::anyhow!("challenge not found after upsert"))?;

    Ok(model)
}

pub async fn import_challenge_zip(
    db: &DatabaseConnection,
    challenge_zip: tempfile::NamedTempFile,
) -> anyhow::Result<String> {
    let output_root = get_setting(&db, "CHALLENGES_DIR")
        .await
        .map_err(|e| UniError::CustomError(format!("get setting error: {}", e)))?;
    let mut archive = zip::ZipArchive::new(challenge_zip)?;

    let meta_toml = {
        let mut meta_toml_file = archive.by_name("meta.toml")?;
        let mut meta_toml_content = String::new();
        meta_toml_file.read_to_string(&mut meta_toml_content)?;
        meta_toml_content
    };

    let cm = ChallengeMeta::from_toml_str(&meta_toml)?;
    let safe_name = generate_safe_name(&cm.name);
    let output_dir = std::path::Path::new(&output_root).join(safe_name);

    if output_dir.exists() {
        // 覆盖题目
        std::fs::remove_dir_all(&output_dir)?;
    }

    std::fs::create_dir_all(&output_dir)?;

    archive.extract(&output_dir)?;

    Ok(meta_toml)
}

pub async fn import_challenge_list_zip(
    db: &DatabaseConnection,
    challenge_list_zip: tempfile::NamedTempFile,
) -> anyhow::Result<Vec<String>> {
    let mut archive = zip::ZipArchive::new(challenge_list_zip)?;
    let file_names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();
    let mut will_insert_toml_strs = Vec::new();
    for zip_name in file_names {
        let mut file = archive.by_name(&zip_name)?;
        let mut temp_file = NamedTempFile::new()?;
        std::io::copy(&mut file, &mut temp_file)?;
        let meta_toml = import_challenge_zip(db, temp_file).await?;
        will_insert_toml_strs.push(meta_toml);
    }
    Ok(will_insert_toml_strs)
}

/// POST /api/admin/challenges/check
#[derive(Debug, Serialize, Deserialize)]
pub struct ChallengeCheckResult {
    pub id: Uuid,
    pub challenge_name: String,
    pub is_ok: bool,
    pub docker_image: bool,
    pub attachment: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChallengeCheckRequest {
    pub challenge_id_list: Option<Vec<Uuid>>,
}

#[post("/check")]
pub async fn check_challenges(
    _user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    ccr: Json<ChallengeCheckRequest>,
) -> UniResult<Vec<ChallengeCheckResult>> {
    let ccr = ccr.into_inner();
    let mut challenge_check_results = Vec::new();
    // check docker image
    // check challenge attachment
    let challenge_dir = get_setting(&ctx.db, "CHALLENGES_DIR")
        .await
        .map_err(|e| UniError::CustomError(format!("get setting error: {}", e)))?;

    let challenges = {
        if ccr.challenge_id_list.is_some() {
            Challenges::find()
                .filter(challenges::Column::Id.is_in(ccr.challenge_id_list.unwrap()))
                .all(ctx.db.get_ref())
                .await?
        } else {
            Challenges::find().all(ctx.db.get_ref()).await?
        }
    };

    for challenge in challenges {
        let attachment_ok = challenge.attachment.as_ref().map_or(true, |attachment| {
            let challenge_dir = std::path::Path::new(&challenge_dir).join(&challenge.safe_name);
            challenge_dir.join(attachment).exists()
        });

        let cm = ChallengeMeta::from_toml_str(&challenge.toml_str)
            .map_err(|e| UniError::CustomError(format!("parse challenge meta error: {}", e)))?;

        let docker_image_ok = match &cm.docker {
            Some(d) => ctx.docker.inspect_image(&d.image_tag).await.is_ok(),
            None => true, // 非docker 题目 默认为true
        };

        challenge_check_results.push(ChallengeCheckResult {
            id: challenge.id,
            challenge_name: challenge.name,
            is_ok: attachment_ok && docker_image_ok,
            docker_image: docker_image_ok,
            attachment: attachment_ok,
        });
    }

    UniResponse::ok(challenge_check_results.into()).into()
}
pub fn generate_safe_name(original: &str) -> String {
    original
        .chars()
        .map(|c| {
            if c.is_ascii() {
                // 保留 ASCII 字母、数字、空格、点号、下划线、连字符
                if c.is_ascii_alphanumeric() || c == ' ' || c == '.' || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            } else {
                // 中文、日文、韩文、emoji 等非 ASCII 字符不处理
                c
            }
        })
        .collect()
}

/// POST /api/admin/challenges/build
#[derive(Debug, Serialize, Deserialize)]
pub struct BuildChallengeRequest {
    pub challenge_id: Option<Uuid>,
    pub challenge_id_list: Option<Vec<Uuid>>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct BuildChallengeResult {
    pub challenge_name: String,
    pub is_ok: bool,
    pub message: String,
}

#[post("/build")]
pub async fn build_challenge(
    user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    bcr: Json<BuildChallengeRequest>,
) -> UniResult<Vec<BuildChallengeResult>> {
    let user = user.into_inner();
    let bcr = bcr.into_inner();
    let mut res = Vec::new();
    let mut challenge_id_list = Vec::new();
    bcr.challenge_id.map(|c| {
        challenge_id_list.push(c);
    });
    bcr.challenge_id_list.map(|c| {
        challenge_id_list.extend(c);
    });

    for challenge_id in challenge_id_list {
        let challenge = Challenges::find_by_id(challenge_id)
            .one(ctx.db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

        let cm = ChallengeMeta::from_toml_str(&challenge.toml_str)
            .map_err(|e| UniError::CustomError(format!("parse challenge meta error: {}", e)))?;

        if cm.docker.is_none() {
            continue;
        }

        let challenges_dir = get_setting(&ctx.db, "CHALLENGES_DIR")
            .await
            .map_err(|e| UniError::CustomError(format!("get setting error: {}", e)))?;

        let context_path = std::path::Path::new(&challenges_dir)
            .join(&challenge.safe_name)
            .join("src");

        let build_result = cm.build_image(&ctx.docker, &context_path).await;
        let is_ok = build_result.is_ok();
        let message = build_result.map_or_else(|e| e.to_string(), |_| "ok".to_string());

        res.push(BuildChallengeResult {
            challenge_name: challenge.name.clone(),
            is_ok,
            message,
        });

        ctx.log
            .add_log(
                "INFO",
                "CHALLENGES",
                "BUILD",
                format!("{} 构建题目镜像: {}", user.username, challenge.name).as_str(),
                json!({"challenge_name": challenge.name, "success": is_ok}),
                None,
                user.id.into(),
                Some(&ctx.req),
            )
            .await;
    }

    UniResponse::ok(res.into()).into()
}
