use std::process::Stdio;

use actix_web::{HttpRequest, HttpResponse, web};
use actix_ws::Message;
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::api::prelude::*;
use crate::auth::validate_jwt;
use crate::entity::super_admin;

/// GET /api/admin/terminal/ws
#[get("/ws")]
pub async fn terminal_ws(
    req: HttpRequest,
    stream: web::Payload,
    query: web::Query<std::collections::HashMap<String, String>>,
) -> Result<HttpResponse, actix_web::Error> {
    // Validate JWT from query param
    let token = query.get("token").cloned().unwrap_or_default();
    let db = req.app_data::<WebDb>().cloned().expect("WebDb not found");

    let claims = validate_jwt(token)
        .map_err(|_| actix_web::error::ErrorUnauthorized("Invalid or missing token"))?;

    let _admin = super_admin::Entity::find_by_id(claims.sub)
        .one(db.get_ref())
        .await
        .map_err(|_| actix_web::error::ErrorInternalServerError("DB error"))?
        .ok_or_else(|| actix_web::error::ErrorUnauthorized("Not a super admin"))?;

    // Upgrade to WebSocket
    let (res, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    // Spawn bash inside script for PTY support
    // macOS: script -q /dev/null bash --login
    // Linux: script -q -c "bash --login" /dev/null
    let mut cmd = if cfg!(target_os = "macos") {
        let mut c = Command::new("script");
        c.arg("-q").arg("/dev/null").arg("bash").arg("--login");
        c
    } else {
        let mut c = Command::new("script");
        c.arg("-q").arg("-c").arg("bash --login").arg("/dev/null");
        c
    };

    // Set TERM so programs know how to render
    cmd.env("TERM", "xterm-256color");
    // Disable pagination
    cmd.env("PAGER", "cat");
    cmd.env("MANPAGER", "cat");
    // Merge stderr into stdout for simplicity
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.stdin(Stdio::piped());
    cmd.kill_on_drop(true);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = session
                .text(format!("Failed to spawn shell: {e}\r\n"))
                .await;
            let _ = session.close(None).await;
            return Ok(res);
        }
    };

    let mut child_stdin = child.stdin.take().unwrap();
    let child_stdout = child.stdout.take().unwrap();
    let child_stderr = child.stderr.take().unwrap();

    // Read stdout and forward to WebSocket
    let mut session_out = session.clone();
    let stdout_handle = actix_web::rt::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut reader = tokio::io::BufReader::new(child_stdout);
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let data = Vec::from(&buf[..n]);
                    if session_out.binary(data).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Read stderr and forward to WebSocket
    let mut session_err = session.clone();
    let stderr_handle = actix_web::rt::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut reader = tokio::io::BufReader::new(child_stderr);
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let data = Vec::from(&buf[..n]);
                    if session_err.binary(data).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Clone session for the msg processing task (needed for pong)
    let mut session_msg = session.clone();

    // Process incoming WebSocket messages
    actix_web::rt::spawn(async move {
        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                Message::Text(text) => {
                    // Check if this is a resize message
                    if let Ok(ctrl) = serde_json::from_str::<TerminalControl>(&text) {
                        if ctrl.r#type == "resize" {
                            // Tell bash about the new terminal size via stty
                            let cmd = format!(
                                "stty rows {} cols {} 2>/dev/null\n",
                                ctrl.rows.unwrap_or(24),
                                ctrl.cols.unwrap_or(80)
                            );
                            let _ = child_stdin.write_all(cmd.as_bytes()).await;
                            continue;
                        }
                    }
                    // Regular text input - forward to bash stdin
                    let _ = child_stdin.write_all(text.as_bytes()).await;
                }
                Message::Binary(bin) => {
                    let _ = child_stdin.write_all(&bin).await;
                }
                Message::Ping(bytes) => {
                    let _ = session_msg.pong(&bytes).await;
                }
                Message::Close(_) => {
                    break;
                }
                _ => {}
            }
        }

        // WebSocket closed, terminate the child process
        child.kill().await.ok();
    });

    // Cleanup: when stdout or stderr readers end, close the session
    actix_web::rt::spawn(async move {
        let _ = tokio::join!(stdout_handle, stderr_handle);
        let _ = session.close(None).await;
    });

    Ok(res)
}

#[derive(Debug, Deserialize)]
struct TerminalControl {
    #[serde(rename = "type")]
    r#type: String,
    #[serde(default)]
    cols: Option<u16>,
    #[serde(default)]
    rows: Option<u16>,
}
