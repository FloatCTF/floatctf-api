use crate::{
    api::preclude::*,
    auth::{Role, UserJwtGuard, gen_jwt_token},
    entity::{prelude::Users, users},
};

use argon2::{
    Argon2, PasswordHash, PasswordVerifier,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};
use lettre::{
    message::{Message, header::ContentType},
    transport::smtp::{SmtpTransport, authentication::Credentials},
};

#[derive(Debug, Deserialize, Serialize)]
pub struct UserLoginRequest {
    username: String,
    password: String,
}

/// POST /api/users/session
#[post("/session")]
pub async fn user_login(db: WebDb, ulr: Json<UserLoginRequest>) -> UniResult<String> {
    let ulr = ulr.into_inner();

    match Users::find()
        .filter(users::Column::Username.eq(ulr.username))
        .one(db.get_ref())
        .await?
    {
        Some(user) => {
            let verified = {
                let parsed_hash = PasswordHash::new(&user.password).map_err(|e| {
                    UniError::InternalError(format!("Failed to new the PasswordHash: {e}"))
                })?;
                Argon2::default()
                    .verify_password(ulr.password.as_bytes(), &parsed_hash)
                    .is_ok()
            };

            if verified {
                let jwt = gen_jwt_token(user.id, Role::User, None)
                    .map_err(|e| UniError::CustomError(e.to_string()))?;

                UniResponse::ok(jwt.into()).into()
            } else {
                UniError::AuthError.into()
            }
        }
        None => UniError::AuthError.into(),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserRequest {
    username: String,
    nickname: String,
    password: String,
    email: String,
}

/// POST /api/users
#[post("")]
pub async fn create_user(db: WebDb, cur: Json<CreateUserRequest>) -> UniResult<String> {
    let cur = cur.into_inner();

    let hashed_password = {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(cur.password.as_bytes(), &salt)
            .map_err(|e| UniError::CustomError(format!("{}", e.to_string())))?
            .to_string();

        password_hash
    };

    let new_user = users::ActiveModel {
        username: Set(cur.username),
        password: Set(hashed_password),
        email: Set(cur.email),
        nickname: Set(cur.nickname),
        ..Default::default()
    };

    let _user = new_user.insert(db.get_ref()).await?;

    UniResponse::ok(
        "User created successfully, please login "
            .to_string()
            .into(),
    )
    .into()
}

/// GET /api/users/me
#[get("/me")]
pub async fn get_me(user: UserJwtGuard) -> UniResult<users::Model> {
    let mut user = user.into_inner();
    user.password = "".to_string();
    UniResponse::ok(user.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatchMeRequest {
    pub nickname: Option<String>,
    pub email: Option<String>,
    pub password: Option<String>,
}

/// PATCH /api/users/me
#[patch("/me")]
pub async fn patch_me(user: UserJwtGuard, db: WebDb, pmr: Json<PatchMeRequest>) -> UniResult<()> {
    let pmr = pmr.into_inner();
    let user = user.into_inner();

    let mut m_user = user.into_active_model();
    pmr.nickname.map(|n| {
        m_user.nickname = Set(n);
    });
    pmr.email.map(|e| {
        m_user.email = Set(e);
    });

    if let Some(p) = pmr.password {
        let hashed_password = {
            let salt = SaltString::generate(&mut OsRng);
            let argon2 = Argon2::default();

            let password_hash = argon2
                .hash_password(p.as_bytes(), &salt)
                .map_err(|e| UniError::CustomError(format!("{}", e.to_string())))?
                .to_string();

            password_hash
        };

        m_user.password = Set(hashed_password);
    }

    m_user.update(db.get_ref()).await?;

    UniResponse::ok_none().into()
}

// // reset email or username

// #[derive(Debug, Serialize, Deserialize)]
// pub struct ResetPasswordRequest {
//     pub email: Option<String>,
//     pub username: Option<String>,
// }

// // reset password
// #[post("/reset_password")]

// pub async fn send_email(db: WebDb, rpr: Json<ResetPasswordRequest>) -> UniResult<()> {
//     let mut rpr = rpr.into_inner();
//     // check user exist
//     if rpr.email.is_none() && rpr.username.is_none() {
//         return UniError::CustomError("Email or username is required".to_string()).into();
//     }

//     let user = if let Some(email) = rpr.email {
//         Users::find()
//             .filter(users::Column::Email.eq(email))
//             .one(db.get_ref())
//             .await?
//     } else if let Some(username) = rpr.username {
//         Users::find()
//             .filter(users::Column::Username.eq(username))
//             .one(db.get_ref())
//             .await?
//     } else {
//         return UniError::CustomError("Email or username is required".to_string()).into();
//     };

//     if user.is_none() {
//         return UniError::CustomError("User not found".to_string()).into();
//     }

//     let user = user.unwrap();

//     let main_url = get_setting(db.get_ref(), "MAIN_URL")
//         .await
//         .map_err(|e| UniError::CustomError(format!("Failed to get MAIN_URL: {}", e)))?;
//     let smtp_uri = get_setting(db.get_ref(), "SMTP_URI")
//         .await
//         .map_err(|e| UniError::CustomError(format!("Failed to get SMTP_URI: {}", e)))?;

//     // 解析：username:host:password
//     let parts: Vec<&str> = smtp_uri.split(':').collect();
//     if parts.len() != 3 {
//         return UniError::InternalError("SMTP_URI 格式错误，必须为 user:host:pass".to_string())
//             .into();
//     }

//     let smtp_host = parts[0];
//     let smtp_user = parts[1];
//     let smtp_pass = parts[2];

//     // generate token
//     let token = gen_jwt_token(user.id, Role::User, Some(10))
//         .map_err(|e| UniError::CustomError(e.to_string()))?;

//     // generate reset link
//     let reset_link = format!("https://example.com/reset_password?token={}", token);

//     // send email
//     let to = user.email;

//     // 你自己的 ENV 或配置

//     // HTML 邮件内容
//     let html_body = format!(
//         r#"
//         <p>您好，</p>
//         <p>请点击下方按钮重置密码（10 分钟内有效）</p>
//         <p><a href="{0}" style="color:#4a90e2;font-weight:bold;">点击这里重置密码</a></p>
//         <p>如果不是您发起的重置请求，请忽略此邮件。</p>
//         "#,
//         reset_link
//     );

//     let email = Message::builder()
//         .from(smtp_user.parse()?) // ⚠ 必须与 SMTP 登录邮箱一致
//         .to(to.parse()?)
//         .subject("重置密码")
//         .header(ContentType::TEXT_HTML)
//         .body(html_body)?;

//     let creds = Credentials::new(smtp_user.to_string(), smtp_pass.to_string());

//     let mailer = SmtpTransport::relay(smtp_host)?.credentials(creds).build();

//     mailer.send(&email)?;

//     UniResponse::ok_none().into()
// }
