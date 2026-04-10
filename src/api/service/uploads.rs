use std::os::unix::fs::PermissionsExt;

use actix_multipart::form::{MultipartForm, tempfile::TempFile};

use crate::{api::prelude::*, prelude::*};
use chrono::Utc;
use uuid::Uuid;

fn gen_image_path(image_dir: &str, original_name: Option<&str>) -> String {
    let ext = original_name
        .and_then(|name| name.rsplit('.').next())
        .unwrap_or("png");

    let timestamp = Utc::now().format("%Y%m%d%H%M%S%3f");

    let uuid_str = Uuid::new_v4().to_string(); // 32位随机字符

    format!("{}/{}_{}.{}", image_dir, timestamp, &uuid_str[0..6], ext)
}

#[derive(Debug, MultipartForm)]
pub struct ImageForm {
    #[multipart(limit = "50MB")]
    image_file: TempFile,
}

// POST /api/uploads/image
#[post("/image")]
pub async fn upload_image(
    _user: UserJwtGuard,
    ctx: ReqCtx,
    MultipartForm(form): MultipartForm<ImageForm>,
) -> UniResult<String> {
    let image_dir = get_setting(ctx.db.get_ref(), "IMAGE_DIR")
        .await
        .map_err(|e| UniError::CustomError(format!("{}", e.to_string())))?;

    let image_file = form.image_file;
    let image_name = image_file.file_name.unwrap_or("image.png".to_string());
    let image_path = gen_image_path(&image_dir, Some(&image_name));
    // copy 会覆盖旧文件
    std::fs::copy(image_file.file.path(), &image_path)
        .map_err(|e| UniError::InternalError(format!("Failed to copy image file: {}", e)))?;
    std::fs::set_permissions(&image_path, std::fs::Permissions::from_mode(0o644))
        .map_err(|e| UniError::InternalError(format!("Failed to set permissions: {}", e)))?;

    UniResponse::ok(image_path.into()).into()
}
