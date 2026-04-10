use crate::{
    api::prelude::*,
    auth::{Role, UserJwtGuard, gen_jwt_token, validate_jwt},
    entity::{prelude::Users, users},
    prelude::*,
};

use argon2::{
    Argon2, PasswordHash, PasswordVerifier,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};

#[derive(Debug, Deserialize, Serialize)]
pub struct UserLoginRequest {
    username: String,
    password: String,
}

/// POST /api/users/session
#[post("/session")]
pub async fn user_login(ctx: ReqCtx, ulr: Json<UserLoginRequest>) -> UniResult<String> {
    let ulr = ulr.into_inner();

    match Users::find()
        .filter(users::Column::Username.eq(ulr.username))
        .one(ctx.db.get_ref())
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
                ctx.log
                    .add_log(
                        "INFO",
                        "AUTH",
                        "LOGIN",
                        format!("{} 登陆成功", user.username).as_str(),
                        json!([]),
                        user.id.into(),
                        None,
                        Some(&ctx.req),
                    )
                    .await;
                let jwt = gen_jwt_token(user.id, Role::User, None)
                    .map_err(|e| UniError::CustomError(e.to_string()))?;

                UniResponse::ok(jwt.into()).into()
            } else {
                ctx.log
                    .add_log(
                        "ERROR",
                        "AUTH",
                        "LOGIN",
                        format!("{} 登陆失败", user.username).as_str(),
                        json!([]),
                        user.id.into(),
                        None,
                        Some(&ctx.req),
                    )
                    .await;
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
pub async fn create_user(ctx: ReqCtx, cur: Json<CreateUserRequest>) -> UniResult<String> {
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

    let user = new_user.insert(ctx.db.get_ref()).await?;
    ctx.log
        .add_log(
            "INFO",
            "AUTH",
            "REGISTER",
            format!("{} 注册成功", user.username).as_str(),
            json!({}),
            user.id.into(),
            None,
            Some(&ctx.req),
        )
        .await;
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
pub async fn patch_me(user: UserJwtGuard, ctx: ReqCtx, pmr: Json<PatchMeRequest>) -> UniResult<()> {
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

    m_user.update(ctx.db.get_ref()).await?;

    UniResponse::ok_none().into()
}

// reset email or username

#[derive(Debug, Serialize, Deserialize)]
pub struct ResetPasswordRequest {
    pub email: Option<String>,
    pub username: Option<String>,
}

// reset password
#[post("/reset_password")]

pub async fn send_reset_email(ctx: ReqCtx, rpr: Json<ResetPasswordRequest>) -> UniResult<()> {
    let rpr = rpr.into_inner();

    if rpr.email.is_none() && rpr.username.is_none() {
        return UniError::CustomError("Email or username is required".to_string()).into();
    }
    let email = none_if_empty(rpr.email);
    let username = none_if_empty(rpr.username);

    let user = match (email, username) {
        (Some(email), _) => {
            Users::find()
                .filter(users::Column::Email.eq(email))
                .one(ctx.db.get_ref())
                .await?
        }
        (_, Some(username)) => {
            Users::find()
                .filter(users::Column::Username.eq(username))
                .one(ctx.db.get_ref())
                .await?
        }
        _ => return UniError::CustomError("Email or username required".into()).into(),
    };

    let user = user.ok_or_else(|| UniError::CustomError("User not found".to_string()))?;

    let main_url = get_setting(ctx.db.get_ref(), "MAIN_URL")
        .await
        .map_err(|e| UniError::CustomError(format!("Failed to get MAIN_URL: {}", e)))?;
    //
    // generate token
    let token = gen_jwt_token(user.id, Role::ResetAccount, Some(10))
        .map_err(|e| UniError::CustomError(e.to_string()))?;

    // generate reset link
    let reset_link = format!("{}/reset_password?token={}", main_url, token);

    // send email
    let to = user.email;

    // 你自己的 ENV 或配置

    // HTML 邮件内容
    let html_body = format!(
        r#"
        <p>您好，</p>
        <p>请点击下方按钮重置密码（10 分钟内有效）</p>
        <p><a href="{0}" style="color:#4a90e2;font-weight:bold;">点击这里重置密码</a></p>
        <p>如果不是您发起的重置请求，请忽略此邮件。</p>
        "#,
        reset_link
    );
    send_email(&ctx.db, &[&to], None, "重置密码", &html_body)
        .await
        .map_err(|e| UniError::CustomError(format!("Failed to send email: {}", e)))?;

    UniResponse::ok_none().into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResetPasswordOption {
    pub password: String,
    pub confirmed_password: String,
}
#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: String,
}
#[post("/reset")]
pub async fn reset_password(
    ctx: ReqCtx,
    token: Query<TokenQuery>,
    rpo: Json<ResetPasswordOption>,
) -> UniResult<()> {
    let rpo = rpo.into_inner();
    if rpo.password != rpo.confirmed_password {
        return UniError::CustomError("Passwords do not match".to_string()).into();
    }

    let token = token.token.clone();
    let claim = validate_jwt(token).map_err(|e| UniError::CustomError(e.to_string()))?;
    if claim.role != Role::ResetAccount {
        return UniError::CustomError("Invalid token".to_string()).into();
    }

    let user_id = claim.sub;
    let user = Users::find_by_id(user_id)
        .one(ctx.db.get_ref())
        .await?
        .ok_or_else(|| UniError::CustomError("User not found".to_string()))?;

    let mut m_user = user.into_active_model();
    let hashed_password = {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(rpo.password.as_bytes(), &salt)
            .map_err(|e| UniError::CustomError(format!("{}", e.to_string())))?
            .to_string();

        password_hash
    };

    m_user.password = Set(hashed_password);

    m_user.update(ctx.db.get_ref()).await?;

    UniResponse::ok_none().into()
}

