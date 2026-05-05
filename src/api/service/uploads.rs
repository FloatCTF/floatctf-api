use actix_multipart::form::{MultipartForm, tempfile::TempFile};
use aws_sdk_s3::primitives::ByteStream;

use crate::{api::prelude::*, prelude::*};
use chrono::Utc;
use uuid::Uuid;

fn gen_image_path(original_name: Option<&str>) -> String {
    let ext = original_name
        .and_then(|name| name.rsplit('.').next())
        .unwrap_or("png");

    let timestamp = Utc::now().format("%Y%m%d%H%M%S%3f");

    let uuid_str = Uuid::new_v4().to_string(); // 32位随机字符

    format!("images/{}_{}.{}", timestamp, &uuid_str[0..6], ext)
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
    let image_file = form.image_file;
    let image_name = image_file.file_name.unwrap_or("image.png".to_string());
    let image_path = gen_image_path(Some(&image_name));

    let body = ByteStream::from(
        tokio::fs::read(&image_file.file.path())
            .await
            .map_err(|e| UniError::InternalError(format!("Failed to read image file: {}", e)))?,
    );

    ctx.rustfs
        .put_object()
        .bucket("floatctf-public")
        .key(&image_path)
        .body(body)
        .send()
        .await
        .map_err(|e| UniError::InternalError(format!("Failed to upload image to S3: {}", e)))?;

    let render_path = format!("/public/{}", image_path);
    UniResponse::ok(render_path.into()).into()
}
