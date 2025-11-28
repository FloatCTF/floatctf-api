use crate::{
    api::{admin::dto::DeleteItemsRequest, preclude::*},
    entity::{challenge_set_items, challenge_sets, challenges},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateChallengeSetRequest {
    pub name: String,
    pub description: Option<String>,
    pub challenge_id_list: Option<Vec<Uuid>>,
}
/// POST /api/admin/challenge_sets
#[post("")]
pub async fn create_challenge_set(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    csr: Json<CreateChallengeSetRequest>,
) -> UniResult<challenge_sets::Model> {
    let csr = csr.into_inner();
    let challenge_set = challenge_sets::ActiveModel {
        name: Set(csr.name),
        description: Set(csr.description),
        ..Default::default()
    };
    let challenge_set = challenge_set.insert(db.get_ref()).await?;

    if let Some(challenge_id_list) = csr.challenge_id_list {
        for challenge_id in challenge_id_list {
            challenge_set_items::ActiveModel {
                set_id: Set(challenge_set.id),
                challenge_id: Set(challenge_id),
                ..Default::default()
            }
            .insert(db.get_ref())
            .await?;
        }
    }

    UniResponse::ok_none().into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatchChallengeSetRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

/// PATCH /api/admin/challenge_sets/{challenge_set_id}
#[patch("/{challenge_set_id}")]
pub async fn patch_challenge_set(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    challenge_set_id: Path<Uuid>,
    psr: Json<PatchChallengeSetRequest>,
) -> UniResult<challenge_sets::Model> {
    let challenge_set_id = challenge_set_id.into_inner();
    let psr = psr.into_inner();
    let challenge_set = challenge_sets::Entity::find_by_id(challenge_set_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(
            "challenge set {} not found",
            challenge_set_id
        )))?;
    let mut m_challenge_set = challenge_set.into_active_model();

    psr.name.map(|name| m_challenge_set.name = Set(name));
    psr.description
        .map(|description| m_challenge_set.description = Set(Some(description)));

    let challenge_set = m_challenge_set.update(db.get_ref()).await?;
    UniResponse::ok(challenge_set.into()).into()
}

/// DELETE /api/admin/challenge_sets
#[delete("")]
pub async fn delete_challenge_set(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    dir: Json<DeleteItemsRequest>,
) -> UniResult<u64> {
    let dir = dir.into_inner();
    let deleted_count = challenge_sets::Entity::delete_many()
        .filter(challenge_sets::Column::Id.is_in(dir.id_list))
        .exec(db.get_ref())
        .await?
        .rows_affected;
    UniResponse::ok(deleted_count.into()).into()
}

/// GET /api/admin/challenge_sets
#[get("")]
pub async fn get_challenge_sets(
    _user: SuperAdminJwtGuard,
    db: WebDb,
) -> UniResult<Vec<challenge_sets::Model>> {
    let challenge_sets = challenge_sets::Entity::find().all(db.get_ref()).await?;
    UniResponse::ok(challenge_sets.into()).into()
}

/// GET /api/admin/challenge_sets/{challenge_set_id}
#[get("/{challenge_set_id}")]
pub async fn get_challenge_set(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    challenge_set_id: Path<Uuid>,
) -> UniResult<Vec<challenges::Model>> {
    let challenge_set_id = challenge_set_id.into_inner();
    let challenge_set_items = challenge_set_items::Entity::find()
        .filter(challenge_set_items::Column::SetId.eq(challenge_set_id))
        .all(db.get_ref())
        .await?;
    let challenge_ids: Vec<Uuid> = challenge_set_items
        .iter()
        .map(|item| item.challenge_id)
        .collect();
    let challenges = challenges::Entity::find()
        .filter(challenges::Column::Id.is_in(challenge_ids))
        .all(db.get_ref())
        .await?;
    UniResponse::ok(challenges.into()).into()
}

/// DELETE /api/admin/challenge_sets/{challenge_set_id}/challenges
#[delete("/{challenge_set_id}/challenges")]
pub async fn delete_challenge_from_set(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    challenge_set_id: Path<Uuid>,
    dir: Json<DeleteItemsRequest>,
) -> UniResult<u64> {
    let challenge_set_id = challenge_set_id.into_inner();
    let dir = dir.into_inner();

    let deleted_count = challenge_set_items::Entity::delete_many()
        .filter(challenge_set_items::Column::SetId.eq(challenge_set_id))
        .filter(challenge_set_items::Column::ChallengeId.is_in(dir.id_list))
        .exec(db.get_ref())
        .await?
        .rows_affected;
    UniResponse::ok(deleted_count.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddChallengeToSetRequest {
    pub challenge_id_list: Vec<Uuid>,
}
/// POST /api/admin/challenge_sets/{challenge_set_id}/challenges
#[post("/{challenge_set_id}/challenges")]
pub async fn add_challenge_to_set(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    challenge_set_id: Path<Uuid>,
    acr: Json<AddChallengeToSetRequest>,
) -> UniResult<()> {
    let challenge_set_id = challenge_set_id.into_inner();
    let acr = acr.into_inner();
    for challenge_id in acr.challenge_id_list {
        challenge_set_items::ActiveModel {
            set_id: Set(challenge_set_id),
            challenge_id: Set(challenge_id),
            ..Default::default()
        }
        .insert(db.get_ref())
        .await?;
    }
    UniResponse::ok_none().into()
}
