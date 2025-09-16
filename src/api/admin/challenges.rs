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
        let toml_str = import_challenge_zip(
            challenge_zip
                .file_name
                .ok_or(UniError::CustomError(format!(
                    "challenge zip file name is empty"
                )))?,
            challenge_zip.file,
        )
        .await
        .map_err(|e| UniError::CustomError(format!("import challenge zip error: {}", e)))?;

        will_insert_toml_strs.push(toml_str);
    }

    if let Some(challenge_list_zip) = form.challenge_list_zip {
        let toml_strs = import_challenge_list_zip(challenge_list_zip)
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
    dir_name: String,
    challenge_zip: NamedTempFile,
) -> anyhow::Result<String> {
    let mut archive = zip::ZipArchive::new(challenge_zip)?;

    let name = {
        if dir_name.contains(".zip") {
            dir_name
                .strip_suffix(".zip")
                .ok_or(UniError::CustomError(
                    "challenge zip file name is not end with .zip".to_string(),
                ))?
                .to_owned()
        } else {
            dir_name
        }
    };

    let output_path = std::env::var("CHALLENGES_DIR").expect("YOU must set CHALLENGES_DIR");
    let output_path = std::path::Path::new(&output_path).join(&name);

    // >>> 新增：如果目录已存在，先删再解压（确保覆盖）
    if output_path.exists() {
        std::fs::remove_dir_all(&output_path)?;
    }

    // 解压zip文件（原逻辑保持不变）
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;

        let file_path = std::path::Path::new(file.name());
        let out_path = output_path.join(file_path);

        if file.name().ends_with('/') || file.name().ends_with('\\') {
            std::fs::create_dir_all(&out_path)?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut outfile = std::fs::File::create(&out_path)?;
        std::io::copy(&mut file, &mut outfile)?;
    }

    let toml_str = std::fs::read_to_string(output_path.join("meta.toml"))?;
    Ok(toml_str)
}

pub async fn import_challenge_list_zip(
    challenge_list_zip: TempFile,
) -> anyhow::Result<Vec<String>> {
    let tmp_dir = tempfile::tempdir()?;
    let mut archive = zip::ZipArchive::new(challenge_list_zip.file)?;
    let mut will_insert_toml_strs = Vec::new();

    // 1. 解压外层 ZIP 文件到临时目录
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let out_path = tmp_dir.path().join(file.name());

        if file.name().ends_with('/') || file.name().ends_with('\\') {
            std::fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut outfile = std::fs::File::create(&out_path)?;
        std::io::copy(&mut file, &mut outfile)?;
    }

    // 3. 读取 CHALLENGES_DIR 环境变量路径
    let challenge_dir =
        std::env::var("CHALLENGES_DIR").expect("CHALLENGES_DIR env var must be set");
    let challenge_dir_path = std::path::Path::new(&challenge_dir);

    // 4. 遍历临时目录内所有文件，假设都是内层 ZIP 文件，解压它们
    for entry in std::fs::read_dir(tmp_dir.path())? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            // 内层 ZIP 文件名
            let file_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("invalid file name"))?;

            // 去掉 .zip 后缀作为解压目录名
            let dir_name = file_name
                .strip_suffix(".zip")
                .ok_or_else(|| anyhow::anyhow!("inner zip file does not end with .zip"))?;

            // 解压目标路径
            let out_path = challenge_dir_path.join(dir_name);

            // >>> 新增：如果同名目录已存在，先删除，确保覆盖解压
            if out_path.exists() {
                std::fs::remove_dir_all(&out_path)?;
            }

            // 创建目录
            std::fs::create_dir_all(&out_path)?;

            // 解压内层 ZIP
            let inner_file = std::fs::File::open(&path)?;
            let mut inner_archive = zip::ZipArchive::new(inner_file)?;

            for i in 0..inner_archive.len() {
                let mut file = inner_archive.by_index(i)?;
                let inner_file_path = std::path::Path::new(file.name());
                let inner_out_path = out_path.join(inner_file_path);

                if file.name().ends_with('/') || file.name().ends_with('\\') {
                    std::fs::create_dir_all(&inner_out_path)?;
                    continue;
                }
                if let Some(parent) = inner_out_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                let mut outfile = std::fs::File::create(&inner_out_path)?;
                std::io::copy(&mut file, &mut outfile)?;
            }

            // 读取 meta.toml
            let meta_path = out_path.join("meta.toml");
            let meta_str = std::fs::read_to_string(&meta_path)
                .map_err(|e| anyhow::anyhow!("read meta.toml failed: {}", e))?;
            will_insert_toml_strs.push(meta_str);
        }
    }

    Ok(will_insert_toml_strs)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChallengeCheckResult {
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
            let challenge_dir = std::path::Path::new(&challenge_dir).join(&challenge.name);
            challenge_dir.join(attachment).exists()
        });

        let cm = ChallengeMeta::from_toml_str(&challenge.toml_str)
            .map_err(|e| UniError::CustomError(format!("parse challenge meta error: {}", e)))?;

        let docker_image_ok = match &cm.docker {
            Some(d) => docker.inspect_image(&d.image_tag).await.is_ok(),
            None => true, // 非docker 题目 默认为true
        };

        challenge_check_results.push(ChallengeCheckResult {
            challenge_name: challenge.name,
            is_ok: attachment_ok && docker_image_ok,
            docker_image: docker_image_ok,
            attachment: attachment_ok,
        });
    }

    UniResponse::ok(challenge_check_results.into()).into()
}
