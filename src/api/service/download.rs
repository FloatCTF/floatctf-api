use crate::{
    auth::{SuperAdminJwtGuard, UserJwtGuard},
    db::WebRustfs,
    prelude::ReqCtx,
};
use actix_web::{Responder, get, web::Query};
use anyhow::Result;
use aws_sdk_s3::presigning::PresigningConfig;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DownloadQuery {
    /// ?type=event_writeup&event_id=123
    EventWriteup { event_id: i64 },
}

pub async fn generate_presigned_download_url(
    rustfs: WebRustfs,
    bucket: &str,
    key: &str,
    ttl_secs: u64,
) -> Result<String> {
    let presigned = rustfs
        .get_object()
        .bucket(bucket)
        .key(key)
        .presigned(PresigningConfig::expires_in(Duration::from_secs(ttl_secs))?)
        .await?;

    Ok(presigned.uri().to_string())
}

// download?type=event_writeup&event_id=123

pub async fn download(ctx: ReqCtx, user: UserJwtGuard, Query(q): Query<DownloadQuery>) {

    // user
    // no
}

// // src/services/download.rs

// pub async fn unified_download_handler(
//     Extension(user): Extension<User>,
//     Path(path): Path<String>,
// ) -> impl IntoResponse {

//     // --- 第一层：管理员特权 ---
//     if user.is_admin {
//         // 管理员可以下载 private 桶下的任何路径
//         return sign_and_redirect(&path).await;
//     }

//     // --- 第二层：普通用户分类鉴权 ---
//     let segments: Vec<&str> = path.split('/').collect();

//     let can_download = match segments.as_slice() {
//         // 匹配：users/101/file.png
//         ["users", uid_str, ..] => {
//             uid_str.parse::<i32>().ok() == Some(user.id)
//         },

//         // 匹配：events/1/10/101/file.zip
//         ["events", _eid, tid_str, _uid, ..] => {
//             tid_str.parse::<i32>().ok() == Some(user.team_id)
//         },

//         // 其他路径（如 system/）普通用户一律拒绝
//         _ => false,
//     };

//     if can_download {
//         sign_and_redirect(&path).await
//     } else {
//         StatusCode::FORBIDDEN.into_response()
//     }
// }
