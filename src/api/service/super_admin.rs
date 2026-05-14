use crate::{
    api::prelude::*,
    auth::{Role, gen_jwt_token},
    entity::{prelude::SuperAdmin, super_admin},
    prelude::*,
};
use argon2::{Argon2, PasswordHash, PasswordVerifier};

#[derive(Debug, Deserialize, Serialize)]
pub struct SuperAdminLoginRequest {
    username: String,
    password: String,
}

/// POST /api/admin/session
#[post("/admin/session")]
pub async fn super_admin_login(ctx: ReqCtx, slr: Json<SuperAdminLoginRequest>) -> UniResult<String> {
    let slr = slr.into_inner();

    match SuperAdmin::find()
        .filter(super_admin::Column::Username.eq(slr.username))
        .one(ctx.db.get_ref())
        .await?
    {
        Some(super_admin) => {
            let verified = {
                let parsed_hash = PasswordHash::new(&super_admin.password).map_err(|e| {
                    UniError::InternalError(format!("Failed to new the PasswordHash: {e}"))
                })?;
                Argon2::default()
                    .verify_password(slr.password.as_bytes(), &parsed_hash)
                    .is_ok()
            };

            if verified {
                ctx.log
                    .add_log(
                        "INFO",
                        "AUTH",
                        "LOGIN",
                        format!("管理员 {} 登陆成功", super_admin.username).as_str(),
                        json!([]),
                        None,
                        super_admin.id.into(),
                        Some(&ctx.req),
                    )
                    .await;
                let jwt = gen_jwt_token(super_admin.id, Role::SuperAdmin, None)
                    .map_err(|e| UniError::CustomError(e.to_string()))?;

                UniResponse::ok(jwt.into()).into()
            } else {
                ctx.log
                    .add_log(
                        "ERROR",
                        "AUTH",
                        "LOGIN",
                        format!("管理员 {} 登陆失败", super_admin.username).as_str(),
                        json!([]),
                        None,
                        super_admin.id.into(),
                        Some(&ctx.req),
                    )
                    .await;
                UniError::AuthError.into()
            }
        }
        None => UniError::AuthError.into(),
    }
}
