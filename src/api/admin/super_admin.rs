use crate::{
    api::{admin::dto::DeleteItemsRequest, prelude::*},
    entity::super_admin,
    prelude::*,
};
use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSuperAdminRequest {
    username: String,
    password: String,
    email: String,
}
/// POST /api/admin/super_admin
#[post("")]
pub async fn create_super_admin(
    _user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    csr: Json<CreateSuperAdminRequest>,
) -> UniResult<super_admin::Model> {
    let csr = csr.into_inner();

    let hashed_password = {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(csr.password.as_bytes(), &salt)
            .map_err(|e| UniError::CustomError(format!("{}", e.to_string())))?
            .to_string();

        password_hash
    };

    let new_super_admin = super_admin::ActiveModel {
        username: Set(csr.username),
        password: Set(hashed_password),
        email: Set(csr.email),
        ..Default::default()
    };

    let super_admin = new_super_admin.insert(ctx.db.get_ref()).await?;

    UniResponse::ok(super_admin.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatchSuperAdminRequest {
    username: Option<String>,
    password: Option<String>,
    email: Option<String>,
}

/// POST /api/admin/super_admin/{super_user_id}
#[post("/{super_user_id}")]
pub async fn patch_super_admin(
    _user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    psr: Json<PatchSuperAdminRequest>,
    super_user_id: Path<Uuid>,
) -> UniResult<super_admin::Model> {
    let psr = psr.into_inner();
    let super_user_id = super_user_id.into_inner();

    let super_admin = super_admin::Entity::find_by_id(super_user_id)
        .one(ctx.db.get_ref())
        .await?
        .ok_or_else(|| UniError::NotFound(format!("{} not exist", super_user_id)))?;

    let mut m_super_admin = super_admin.into_active_model();

    psr.username.map(|u| {
        m_super_admin.username = Set(u);
    });

    if let Some(p) = psr.password {
        let hashed_password = {
            let salt = SaltString::generate(&mut OsRng);
            let argon2 = Argon2::default();

            let password_hash = argon2
                .hash_password(p.as_bytes(), &salt)
                .map_err(|e| UniError::CustomError(format!("{}", e.to_string())))?
                .to_string();

            password_hash
        };
        m_super_admin.password = Set(hashed_password);
    }

    psr.email.map(|e| {
        m_super_admin.email = Set(e);
    });

    let super_admin = m_super_admin.update(ctx.db.get_ref()).await?;

    UniResponse::ok(super_admin.into()).into()
}

/// GET /api/admin/super_admin
#[get("")]
pub async fn get_super_admins(
    _user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<super_admin::Model>> {
    let mut query_params = query_params.0;

    let stmt = super_admin::Entity::find().order_by_desc(super_admin::Column::UpdatedAt);

    if let (Some(limit), Some(page)) = (query_params.limit, query_params.page) {
        let paginator = stmt.paginate(ctx.db.get_ref(), limit);
        let items = paginator.fetch_page(page.saturating_sub(1)).await?;
        query_params.total = Some(paginator.num_items().await? as usize);

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    } else {
        let items = stmt.all(ctx.db.get_ref()).await?;
        query_params.total = Some(items.len());

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    }
}

/// GET /api/admin/super_admin/{super_user_id}
#[get("/{super_user_id}")]
pub async fn get_super_admin(
    _user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    super_user_id: Path<Uuid>,
) -> UniResult<super_admin::Model> {
    let super_user_id = super_user_id.into_inner();
    let model = super_admin::Entity::find_by_id(super_user_id)
        .one(ctx.db.get_ref())
        .await?
        .ok_or_else(|| UniError::NotFound(format!("{} not exist", super_user_id)))?;

    UniResponse::ok(model.into()).into()
}

/// DELETE /api/admin/super_admin
#[delete("")]
pub async fn delete_super_admin(
    _user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    dir: Json<DeleteItemsRequest>,
) -> UniResult<u64> {
    let dir = dir.into_inner();

    let r = super_admin::Entity::delete_many()
        .filter(super_admin::Column::Id.is_in(dir.id_list))
        .exec(ctx.db.get_ref())
        .await?;

    UniResponse::ok(r.rows_affected.into()).into()
}

#[actix_web::test]
pub async fn add_admin() {
    dotenvy::dotenv().ok();
    let db = crate::db::init_db().await.unwrap();
    let username = "sysadmin";
    let password = "FloatCTF@2025";
    let email = "sysadmin@system.com";

    let salt = SaltString::generate(&mut OsRng);
    let hashed_password = {
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(password.as_bytes(), &salt).unwrap();

        password_hash
    };

    let new_super_admin = super_admin::ActiveModel {
        username: Set(username.into()),
        password: Set(hashed_password.to_string()),
        email: Set(email.into()),
        ..Default::default()
    };

    let _super_admin = new_super_admin.insert(&db).await.unwrap();
}
