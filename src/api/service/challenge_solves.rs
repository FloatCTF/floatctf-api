use crate::{
    api::prelude::*,
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

    let stmt =
        challenge_solves::Entity::find().filter(challenge_solves::Column::UserId.eq(user.id));

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
