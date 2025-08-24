use sea_orm::{ColumnTrait, QueryFilter};

use super::super::preclude::*;
use crate::entity::{
    event_team_members, event_teams, event_users,
    prelude::{EventTeams, Events, Users},
    users,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct AddTeamRequest {
    pub name: String,
    pub description: Option<String>,
}

#[post("")]
pub async fn add_team(
    db: WebDb,
    id: Path<Uuid>,
    atr: Json<AddTeamRequest>,
) -> UniResult<event_teams::Model> {
    let atr = atr.into_inner();

    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let new_event_team = event_teams::ActiveModel {
        event_id: Set(event.id),
        name: Set(atr.name),
        description: Set(atr.description),
        ..Default::default()
    };

    let event_team = new_event_team.insert(db.get_ref()).await?;

    UniResponse::ok(event_team.into()).into()
}

#[delete("/{team_id}")]
pub async fn remove_team(db: WebDb, path: Path<(Uuid, Uuid)>) -> UniResult<u64> {
    let (id, team_id) = path.into_inner();

    let event_team = EventTeams::find_by_id(team_id)
        .filter(event_teams::Column::EventId.eq(id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let r = event_team.delete(db.get_ref()).await?;

    UniResponse::ok(r.rows_affected.into()).into()
}

#[get("")]
pub async fn get_teams(
    db: WebDb,
    id: Path<Uuid>,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<event_teams::Model>> {
    let mut query_params = query_params.0;

    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let stmt = event.find_related(EventTeams);

    if let (Some(limit), Some(page)) = (query_params.limit, query_params.page) {
        let paginator = stmt.paginate(db.get_ref(), limit);
        let items = paginator.fetch_page(page.saturating_sub(1)).await?;
        query_params.total = Some(paginator.num_items().await? as usize);

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    } else {
        let items = stmt.all(db.get_ref()).await?;
        query_params.total = Some(items.len());

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetTeamMembersResult {
    pub team: event_teams::Model,
    pub members: Vec<users::Model>,
}

#[get("/{team_id}")]
pub async fn get_team_members(
    db: WebDb,
    path: Path<(Uuid, Uuid)>,
    query_params: Query<QueryParams>,
) -> UniResult<GetTeamMembersResult> {
    let mut query_params = query_params.0;
    let (id, team_id) = path.into_inner();

    let event_team = EventTeams::find_by_id(team_id)
        .filter(event_teams::Column::EventId.eq(id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", team_id)))?;

    let stmt = event_team
        .find_related(event_team_members::Entity)
        .find_also_related(Users);

    if let (Some(limit), Some(page)) = (query_params.limit, query_params.page) {
        let paginator = stmt.paginate(db.get_ref(), limit);
        let items: Vec<(event_team_members::Model, Option<users::Model>)> =
            paginator.fetch_page(page.saturating_sub(1)).await?;
        query_params.total = Some(paginator.num_items().await? as usize);
        let items: Vec<users::Model> = items
            .into_iter()
            .filter_map(|(_team_member, user_opt)| user_opt)
            .collect();
        let result = GetTeamMembersResult {
            team: event_team,
            members: items,
        };
        UniResponse::ok_meta(result.into(), query_params.into()).into()
    } else {
        let items = stmt.all(db.get_ref()).await?;
        let items: Vec<users::Model> = items
            .into_iter()
            .filter_map(|(_team_member, user_opt)| user_opt)
            .collect();
        query_params.total = Some(items.len());
        let result = GetTeamMembersResult {
            team: event_team,
            members: items,
        };
        UniResponse::ok_meta(result.into(), query_params.into()).into()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddUserToTeamRequest {
    pub user_id: Uuid,
}
#[post("/{team_id}/users")]
pub async fn add_user_to_team(
    db: WebDb,
    path: Path<(Uuid, Uuid)>,
    utt: Json<AddUserToTeamRequest>,
) -> UniResult<event_team_members::Model> {
    let (id, team_id) = path.into_inner();
    let user_id = utt.into_inner().user_id;

    let event_team = EventTeams::find_by_id(team_id)
        .filter(event_teams::Column::EventId.eq(id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", team_id)))?;

    let user = Users::find_by_id(user_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", user_id)))?;

    let new_event_user = event_users::ActiveModel {
        event_id: Set(event_team.event_id),
        user_id: Set(user.id),
        ..Default::default()
    };

    let _event_user = new_event_user.insert(db.get_ref()).await?;

    let new_team_user = event_team_members::ActiveModel {
        event_id: Set(event_team.event_id),
        team_id: Set(event_team.id),
        user_id: Set(user.id),
        ..Default::default()
    };

    let team_user = new_team_user.insert(db.get_ref()).await?;

    UniResponse::ok(team_user.into()).into()
}

#[delete("/{team_id}/users/{user_id}")]
pub async fn remove_user_from_team(db: WebDb, path: Path<(Uuid, Uuid, Uuid)>) -> UniResult<u64> {
    let (id, team_id, user_id) = path.into_inner();

    let event_team_member = event_team_members::Entity::find()
        .filter(event_team_members::Column::EventId.eq(id))
        .filter(event_team_members::Column::TeamId.eq(team_id))
        .filter(event_team_members::Column::UserId.eq(user_id))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", user_id)))?;

    let r = event_team_member.delete(db.get_ref()).await?;

    UniResponse::ok(r.rows_affected.into()).into()
}
