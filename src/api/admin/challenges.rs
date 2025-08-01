use super::super::preclude::*;
use crate::entity::{challenges, prelude::Challenges};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateChallengeRequest {
    pub name: String,
    pub category: String,
    pub description: String,
    pub attachment: Option<String>,
    pub toml_str: String,
}

#[post("")]
pub async fn create_challenge(db: WebDb, ccr: Json<CreateChallengeRequest>) -> UniResult<()> {
    let ccr = ccr.into_inner();

    let new_challenge = challenges::ActiveModel {
        name: Set(ccr.name),
        category: Set(ccr.category),
        description: Set(ccr.description),
        attachment: Set(ccr.attachment),
        toml_str: Set(ccr.toml_str),
        ..Default::default()
    };

    let _challenge = new_challenge.insert(db.get_ref()).await?;

    UniResponse::ok_none().into()
}

type UpdateChallengeRequest = CreateChallengeRequest;
#[put("/{id}")]
pub async fn update_challenge(
    db: WebDb,
    ucr: Json<UpdateChallengeRequest>,
    challenge_id: Path<i32>,
) -> UniResult<()> {
    let ucr = ucr.into_inner();

    match Challenges::find_by_id(*challenge_id)
        .one(db.get_ref())
        .await?
    {
        Some(challenge) => {
            let mut m_challenge = challenge.into_active_model();

            m_challenge.name = Set(ucr.name);
            m_challenge.category = Set(ucr.category);
            m_challenge.description = Set(ucr.description);
            m_challenge.attachment = Set(ucr.attachment);
            m_challenge.toml_str = Set(ucr.toml_str);
            m_challenge.updated_at = Set(Utc::now().naive_utc());

            let _challenge = m_challenge.update(db.get_ref()).await?;

            UniResponse::ok_none().into()
        }
        None => UniError::NotFound(format!("challenge_id {} not exist", challenge_id)).into(),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatchChallengeRequest {
    pub name: Option<String>,
    pub category: Option<String>,
    pub description: Option<String>,
    pub attachment: Option<String>,
    pub toml_str: Option<String>,
}
#[patch("/{id}")]
pub async fn patch_challenge(
    db: WebDb,
    pcr: Json<PatchChallengeRequest>,
    challenge_id: Path<i32>,
) -> UniResult<()> {
    let pcr = pcr.into_inner();
    match Challenges::find_by_id(*challenge_id)
        .one(db.get_ref())
        .await?
    {
        Some(challenge) => {
            let mut m_challenge = challenge.into_active_model();

            pcr.name.map(|n| {
                m_challenge.name = Set(n);
            });

            pcr.category.map(|c| {
                m_challenge.category = Set(c);
            });

            pcr.description.map(|d| {
                m_challenge.description = Set(d);
            });

            pcr.attachment.map(|a| {
                m_challenge.attachment = Set(a.into());
            });

            pcr.toml_str.map(|t| m_challenge.toml_str = Set(t));
            m_challenge.updated_at = Set(Utc::now().naive_utc());

            let _challenge = m_challenge.update(db.get_ref()).await?;

            UniResponse::ok_none().into()
        }
        None => UniError::NotFound(format!("challenge_id {} not exist", challenge_id)).into(),
    }
}

#[get("")]
pub async fn get_challenge(
    db: WebDb,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<challenges::Model>> {
    let mut query_params = query_params.0;

    let stmt = Challenges::find();

    if let (Some(limit), Some(page)) = (query_params.limit, query_params.page) {
        let paginator = stmt.paginate(db.get_ref(), limit);
        let items = paginator.fetch_page(page - 1).await?;
        query_params.total = Some(items.len());

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    } else {
        let items = stmt.all(db.get_ref()).await?;
        query_params.total = Some(items.len());

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    }
}

#[get("/{id}")]
pub async fn get_challenges(db: WebDb, id: Path<i32>) -> UniResult<challenges::Model> {
    match Challenges::find_by_id(*id).one(db.get_ref()).await? {
        Some(model) => UniResponse::ok(model.into()).into(),
        None => UniError::NotFound(format!(" {} not exist", id)).into(),
    }
}
