use std::io::Read;

use base64::Engine;

use super::super::preclude::*;
use crate::{
    auth::SuperAdminJwtGuard,
    db::WebDocker,
    entity::{challenges, prelude::Challenges},
};
use actix_multipart::form::{MultipartForm, tempfile::TempFile, text::Text};
use fcmc::ChallengeMeta;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, sea_query::OnConflict,
};
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

#[post("")]
pub async fn create_challenge(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    ccr: Json<CreateChallengeRequest>,
) -> UniResult<challenges::Model> {
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

    let challenge = new_challenge.insert(db.get_ref()).await?;

    UniResponse::ok(challenge.into()).into()
}

type UpdateChallengeRequest = CreateChallengeRequest;
#[put("/{id}")]
pub async fn update_challenge(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    ucr: Json<UpdateChallengeRequest>,
    id: Path<Uuid>,
) -> UniResult<challenges::Model> {
    let ucr = ucr.into_inner();

    let challenge = Challenges::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let mut m_challenge = challenge.into_active_model();

    m_challenge.name = Set(ucr.name);
    m_challenge.category = Set(ucr.category);
    m_challenge.description = Set(ucr.description);
    m_challenge.attachment = Set(ucr.attachment);
    m_challenge.toml_str = Set(ucr.toml_str);
    m_challenge.hidden = Set(ucr.hidden);
    m_challenge.updated_at = Set(Utc::now().naive_utc());

    let challenge = m_challenge.update(db.get_ref()).await?;

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
#[patch("/{id}")]
pub async fn patch_challenge(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    pcr: Json<PatchChallengeRequest>,
    id: Path<Uuid>,
) -> UniResult<challenges::Model> {
    let pcr = pcr.into_inner();

    let challenge = Challenges::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

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
    m_challenge.updated_at = Set(Utc::now().naive_utc());

    let challenge = m_challenge.update(db.get_ref()).await?;

    UniResponse::ok(challenge.into()).into()
}

#[get("")]
pub async fn get_challenges(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<challenges::Model>> {
    let mut query_params = query_params.0;

    let stmt = Challenges::find();

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
pub async fn get_challenge(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<challenges::Model> {
    let model = Challenges::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    UniResponse::ok(model.into()).into()
}

#[delete("/{id}")]
pub async fn delete_challenge(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<u64> {
    let challenge = Challenges::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let del_challenge_path =
        std::env::var("CHALLENGES_DIR").expect("CHALLENGES_DIR env var must be set");
    let del_challenge_path = std::path::Path::new(&del_challenge_path).join(&challenge.safe_name);
    if del_challenge_path.exists() {
        std::fs::remove_dir_all(&del_challenge_path)
            .map_err(|e| UniError::CustomError(format!("delete challenge dir error: {}", e)))?;
    }

    let r = challenge.delete(db.get_ref()).await?;

    UniResponse::ok(r.rows_affected.into()).into()
}

#[derive(Debug, MultipartForm)]
struct UploadForm {
    #[multipart(limit = "1024MB")]
    challenge_zip: Option<TempFile>,
    #[multipart(limit = "10240MB")]
    challenge_list_zip: Option<TempFile>,
    toml_str_b64: Option<Text<String>>,
}

#[post("/import")]
pub async fn web_import_challenge(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    MultipartForm(form): MultipartForm<UploadForm>,
) -> UniResult<Vec<challenges::Model>> {
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
        let toml_str = import_challenge_zip(challenge_zip.file)
            .await
            .map_err(|e| UniError::CustomError(format!("import challenge zip error: {}", e)))?;

        will_insert_toml_strs.push(toml_str);
    }

    if let Some(challenge_list_zip) = form.challenge_list_zip {
        let toml_strs = import_challenge_list_zip(challenge_list_zip.file)
            .await
            .map_err(|e| {
                UniError::CustomError(format!("import challenge list zip error: {}", e))
            })?;

        will_insert_toml_strs.extend(toml_strs);
    }

    for toml_str in will_insert_toml_strs {
        let challenge = import_challenge(db.get_ref(), toml_str)
            .await
            .map_err(|e| UniError::CustomError(format!("import challenge error: {}", e)))?;

        inserted_challenges.push(challenge);
    }

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
    challenge_zip: tempfile::NamedTempFile,
) -> anyhow::Result<String> {
    let output_root = std::env::var("CHALLENGES_DIR").expect("YOU must set CHALLENGES_DIR");
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
    challenge_list_zip: tempfile::NamedTempFile,
) -> anyhow::Result<Vec<String>> {
    let mut archive = zip::ZipArchive::new(challenge_list_zip)?;
    let file_names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();
    let mut will_insert_toml_strs = Vec::new();
    for zip_name in file_names {
        let mut file = archive.by_name(&zip_name)?;
        let mut temp_file = NamedTempFile::new()?;
        std::io::copy(&mut file, &mut temp_file)?;
        let meta_toml = import_challenge_zip(temp_file).await?;
        will_insert_toml_strs.push(meta_toml);
    }
    Ok(will_insert_toml_strs)
}

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
    db: WebDb,
    docker: WebDocker,
    ccr: Json<ChallengeCheckRequest>,
) -> UniResult<Vec<ChallengeCheckResult>> {
    let ccr = ccr.into_inner();
    let mut challenge_check_results = Vec::new();
    // check docker image
    // check challenge attachment
    let challenge_dir =
        std::env::var("CHALLENGES_DIR").expect("CHALLENGES_DIR env var must be set");

    let challenges = {
        if ccr.challenge_id_list.is_some() {
            Challenges::find()
                .filter(challenges::Column::Id.is_in(ccr.challenge_id_list.unwrap()))
                .all(db.get_ref())
                .await?
        } else {
            Challenges::find().all(db.get_ref()).await?
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
            Some(d) => docker.inspect_image(&d.image_tag).await.is_ok(),
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
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

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
    _user: SuperAdminJwtGuard,
    db: WebDb,
    docker: WebDocker,
    bcr: Json<BuildChallengeRequest>,
) -> UniResult<Vec<BuildChallengeResult>> {
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
            .one(db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(" {} not exist", challenge_id)))?;

        let cm = ChallengeMeta::from_toml_str(&challenge.toml_str)
            .map_err(|e| UniError::CustomError(format!("parse challenge meta error: {}", e)))?;

        if cm.docker.is_none() {
            continue;
        }

        let challenges_dir =
            std::env::var("CHALLENGES_DIR").expect("CHALLENGES_DIR env var must be set");
        let context_path = std::path::Path::new(&challenges_dir)
            .join(&challenge.safe_name)
            .join("src");

        let build_result = cm.build_image(&docker, &context_path).await;

        res.push(BuildChallengeResult {
            challenge_name: challenge.name,
            is_ok: build_result.is_ok(),
            message: build_result.map_or_else(|e| e.to_string(), |_| "ok".to_string()),
        });
    }

    UniResponse::ok(res.into()).into()
}
