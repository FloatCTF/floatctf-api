use std::str::FromStr;

use sea_orm::Condition;

use crate::{
    api::{FilterMapping, apply_filters, prelude::*, sea_orm_utils::paginate_query},
    entity::{challenge_solves, users},
    prelude::*,
};

use chrono::{DateTime, FixedOffset};
use std::collections::HashMap;

/// GET /api/challenge_solves
#[get("")]
pub async fn get_solves(
    user: UserJwtGuard,
    ctx: ReqCtx,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<challenge_solves::Model>> {
    let user = user.into_inner();
    let mut query_params = query_params.0;

    let mappings = [
        FilterMapping {
            key: "id",
            column: Box::new(|v| {
                Condition::all()
                    .add(challenge_solves::Column::Id.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())))
            }),
        },
        FilterMapping {
            key: "challenge_id",
            column: Box::new(|v| {
                Condition::all().add(
                    challenge_solves::Column::ChallengeId
                        .eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())),
                )
            }),
        },
        FilterMapping {
            key: "event_id",
            column: Box::new(|v| {
                if v == "null" || v.is_empty() {
                    Condition::all().add(challenge_solves::Column::EventId.is_null())
                } else {
                    Condition::all().add(
                        challenge_solves::Column::EventId
                            .eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())),
                    )
                }
            }),
        },
    ];

    let stmt =
        challenge_solves::Entity::find().filter(challenge_solves::Column::UserId.eq(user.id));
    let stmt = apply_filters(stmt, query_params.filter.clone(), &mappings);
    let stmt = stmt.order_by_desc(challenge_solves::Column::CreatedAt);

    let (items, total_items) =
        if let (Some(limit), Some(page)) = (query_params.limit, query_params.page) {
            paginate_query(stmt, ctx.db.get_ref(), limit, page).await?
        } else {
            let items = stmt.all(ctx.db.get_ref()).await?;
            (items.clone(), items.len())
        };

    query_params.total = Some(total_items);

    UniResponse::ok_meta(items.into(), query_params.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TopUser {
    no: usize,
    nickname: String,
    solved_count: u64,
    solved_last_at: DateTime<FixedOffset>,
}

/// GET /api/challenge_solves/top15users
#[get("/top15users")]
pub async fn get_top_15_users(_user: UserJwtGuard, ctx: ReqCtx) -> UniResult<Vec<TopUser>> {
    let solves = challenge_solves::Entity::find()
        .filter(challenge_solves::Column::EventId.is_null())
        .all(ctx.db.get_ref())
        .await?;

    // 2. 在内存里统计
    let mut stats: HashMap<Uuid, (u64, DateTime<FixedOffset>)> = HashMap::new();

    for s in solves {
        stats
            .entry(s.user_id)
            .and_modify(|(cnt, last)| {
                *cnt += 1;
                if s.created_at > *last {
                    *last = s.created_at;
                }
            })
            .or_insert((1, s.created_at));
    }

    // 3. 查昵称
    let mut result = Vec::new();
    for (uid, (count, last)) in stats {
        if let Some(user) = users::Entity::find_by_id(uid).one(ctx.db.get_ref()).await? {
            result.push((user.nickname, count, last));
        }
    }

    // 4. 排序 + 取前 15
    result.sort_by(|a, b| b.1.cmp(&a.1));
    result.truncate(15);

    // 5. 加上排名号 no
    let result: Vec<TopUser> = result
        .into_iter()
        .enumerate()
        .map(|(idx, (nickname, count, last))| TopUser {
            no: idx + 1, // 👈 排名
            nickname,
            solved_count: count,
            solved_last_at: last,
        })
        .collect();
    UniResponse::ok(result.into()).into()
}
