use std::str::FromStr;

use crate::{
    api::{FilterMapping, admin::dto::DeleteItemsRequest, preclude::*, sea_orm_utils::query_query},
    entity::users,
};
use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};
use sea_orm::Condition;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserRequest {
    username: String,
    password: String,
    nickname: String,
    email: String,
}

/// POST /api/admin/users
#[post("")]
pub async fn create_user(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    cur: Json<CreateUserRequest>,
) -> UniResult<users::Model> {
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

    let user = new_user.insert(db.get_ref()).await?;

    UniResponse::ok(user.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PathUserRequest {
    username: Option<String>,
    nickname: Option<String>,
    password: Option<String>,
    email: Option<String>,
}

/// PATCH /api/admin/users/{user_id}
#[patch("/{user_id}")]
pub async fn patch_user(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    pur: Json<PathUserRequest>,
    user_id: Path<Uuid>,
) -> UniResult<users::Model> {
    let pur = pur.into_inner();
    let user_id = user_id.into_inner();
    let user = users::Entity::find_by_id(user_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", user_id)))?;

    let mut m_user = user.into_active_model();

    pur.username.map(|u| {
        m_user.username = Set(u);
    });

    if let Some(p) = pur.password {
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

    pur.email.map(|e| {
        m_user.email = Set(e);
    });

    pur.nickname.map(|n| {
        m_user.nickname = Set(n);
    });
    m_user.updated_at = Set(Utc::now().into());

    let user = m_user.update(db.get_ref()).await?;

    UniResponse::ok(user.into()).into()
}

/// GET /api/admin/users
#[get("")]
pub async fn get_users(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<users::Model>> {
    let mut query_params = query_params.0;

    let mappings = [
        FilterMapping {
            key: "id",
            column: Box::new(|v| {
                Condition::all()
                    .add(users::Column::Id.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())))
            }),
        },
        FilterMapping {
            key: "username",
            column: Box::new(|v| Condition::all().add(users::Column::Username.contains(v))),
        },
        FilterMapping {
            key: "nickname",
            column: Box::new(|v| Condition::all().add(users::Column::Nickname.contains(v))),
        },
        FilterMapping {
            key: "email",
            column: Box::new(|v| Condition::all().add(users::Column::Email.contains(v))),
        },
    ];

    let (items, total_items) =
        query_query::<users::Entity>(db.get_ref(), &mappings, &query_params).await?;

    query_params.total = Some(total_items);

    UniResponse::ok_meta(items.into(), query_params.into()).into()
}

/// GET /api/admin/users/{id}
#[get("/{id}")]
pub async fn get_user(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    user_id: Path<Uuid>,
) -> UniResult<users::Model> {
    let user_id = user_id.into_inner();
    let model = users::Entity::find_by_id(user_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", user_id)))?;

    UniResponse::ok(model.into()).into()
}

/// DELETE /api/admin/users
#[delete("")]
pub async fn delete_user(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    dir: Json<DeleteItemsRequest>,
) -> UniResult<u64> {
    let dir = dir.into_inner();
    let deleted_count = users::Entity::delete_many()
        .filter(users::Column::Id.is_in(dir.id_list))
        .exec(db.get_ref())
        .await?
        .rows_affected;
    UniResponse::ok(deleted_count.into()).into()
}

#[actix_web::test]
pub async fn add_users() {
    dotenvy::dotenv().ok();
    let db = crate::db::init_db().await.unwrap();
    let users = [users::ActiveModel {
        username: Set("user2".to_string()),
        password: Set("user2".to_string()),
        email: Set("user2".to_string()),
        nickname: Set("user2".to_string()),
        ..Default::default()
    }];

    for user in users {
        user.insert(&db).await.unwrap();
    }
}
