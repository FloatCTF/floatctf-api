use std::result;

use sea_orm::{ColumnTrait, QueryFilter};

use super::super::preclude::*;
use crate::{
    auth::UserJwtGuard,
    entity::{
        challenges, event_challenges, events,
        prelude::{Challenges, EventChallenges, Events},
    },
};

#[get("")]
pub async fn get_events(_user: UserJwtGuard, db: WebDb) -> UniResult<Vec<events::Model>> {
    let events = Events::find()
        .filter(events::Column::Hidden.eq(false))
        .all(db.get_ref())
        .await?;

    UniResponse::ok(events.into()).into()
}

#[get("/{event_id}/challenges")]
pub async fn get_event_challenges(
    _user: UserJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<Vec<challenges::Model>> {
    let event = Events::find_by_id(*id)
        .filter(events::Column::Hidden.eq(false))
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound("event not found".to_string()))?;

    let event_challenges = event
        .find_related(EventChallenges)
        .filter(event_challenges::Column::Hidden.eq(false))
        .find_also_related(Challenges)
        .all(db.get_ref())
        .await?;

    let result = event_challenges
        .into_iter()
        .filter_map(|(_, challenge)| challenge)
        .collect::<Vec<_>>();

    UniResponse::ok(result.into()).into()
}
