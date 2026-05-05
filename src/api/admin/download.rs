use std::{env, time::Duration};

use crate::{
    api::{UniError, UniResponse, UniResult},
    auth::SuperAdminJwtGuard,
    prelude::ReqCtx,
};
use actix_web::{get, web::Query};
use aws_sdk_s3::presigning::PresigningConfig;
use serde::Deserialize;
use serde_json::json;
use tracing::info;

#[derive(Deserialize)]
struct DownloadParams {
    key: String,
}

/// GET /api/download?key=event_writeup/123.pdf
#[get("/download")]
pub async fn download(
    ctx: ReqCtx,
    super_admin: SuperAdminJwtGuard,
    params: Query<DownloadParams>,
) -> UniResult<String> {
    let super_admin = super_admin.into_inner();

    let presigned = ctx
        .rustfs
        .get_object()
        .bucket("floatctf-private")
        .key(&params.key)
        .presigned(
            PresigningConfig::expires_in(Duration::from_secs(90))
                .map_err(|e| UniError::CustomError(e.to_string()))?,
        )
        .await
        .map_err(|e| UniError::CustomError(e.to_string()))?;

    let message = format!(
        "[ADMIN] {} downloading {}",
        super_admin.username, params.key
    );

    info!(message);

    ctx.log
        .add_log(
            "INFO",
            "FILES",
            "DOWNLOAD",
            &message,
            json!([]),
            None,
            Some(super_admin.id),
            Some(&ctx.req),
        )
        .await;

    // right!
    let rustfs_endpoint_url = env::var("RUSTFS_ENDPOINT_URL").unwrap();
    let final_uri = presigned.uri().replace(
        &format!("{}/floatctf-private", rustfs_endpoint_url),
        "/private",
    );
    UniResponse::ok(final_uri.into()).into()
}
