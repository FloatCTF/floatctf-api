use sea_orm::{Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryTrait, Select};
use sea_orm::{DbErr, QuerySelect};
pub struct FilterMapping {
    pub key: &'static str,
    pub column: Box<dyn Fn(&str) -> Condition>,
}

pub fn apply_filters<E: EntityTrait>(
    mut stmt: Select<E>,
    filter: Option<String>,
    mappings: &[FilterMapping],
) -> Select<E> {
    if let Some(f) = filter {
        let tokens: Vec<&str> = f.split_whitespace().collect();
        let mut i = 0;
        let mut or_conditions: Vec<Condition> = vec![];
        let mut and_conditions: Vec<Condition> = vec![];

        while i < tokens.len() {
            let token = tokens[i];

            match token {
                "&" => {
                    i += 1;
                    continue;
                }
                "|" => {
                    if !and_conditions.is_empty() {
                        // fold 累加多个 Condition
                        let combined = and_conditions
                            .drain(..)
                            .fold(Condition::all(), |c, cond| c.add(cond));
                        or_conditions.push(combined);
                    }
                    i += 1;
                }
                _ => {
                    if let Some(pos) = token.find(':') {
                        let key = &token[..pos];
                        let mut value = token[pos + 1..].to_string();

                        // 拼接后续 token，直到遇到逻辑符号
                        i += 1;
                        while i < tokens.len() && tokens[i] != "&" && tokens[i] != "|" {
                            value.push(' ');
                            value.push_str(tokens[i]);
                            i += 1;
                        }

                        if let Some(m) = mappings.iter().find(|m| m.key == key) {
                            and_conditions.push((m.column)(&value));
                        }
                    } else {
                        i += 1;
                    }
                }
            }
        }

        // 剩余 AND 条件加入 OR 条件
        if !and_conditions.is_empty() {
            let combined = and_conditions
                .drain(..)
                .fold(Condition::all(), |c, cond| c.add(cond));
            or_conditions.push(combined);
        }

        // 最终 OR 条件累加到 stmt
        if !or_conditions.is_empty() {
            let combined = or_conditions
                .into_iter()
                .fold(Condition::any(), |c, cond| c.add(cond));
            stmt = stmt.filter(combined);
        }
    }

    stmt
}
use sea_orm::PaginatorTrait;

use crate::api::QueryParams;

pub async fn paginate_query<E: EntityTrait>(
    stmt: Select<E>,
    db: &DatabaseConnection,
    limit: u64,
    page: u64,
) -> Result<(Vec<E::Model>, usize), DbErr>
where
    E: EntityTrait,
    E::Model: Send + Sync,
{
    if limit == 0 || page == 0 {
        return Ok((vec![], 0)); // 不分页或参数无效，直接返回空
    }

    let total_items = stmt.clone().count(db).await? as usize;

    // 计算分页索引
    let total_pages = (total_items + limit as usize - 1) / limit as usize;
    let page_index = page
        .saturating_sub(1)
        .min(total_pages.saturating_sub(1) as u64);

    // 分页查询
    let items = stmt.paginate(db, limit).fetch_page(page_index).await?;

    Ok((items, total_items))
}

pub async fn query_query<E>(
    db: &DatabaseConnection,
    mappings: &[FilterMapping],
    query_params: &QueryParams,
) -> Result<(Vec<E::Model>, usize), DbErr>
where
    E: EntityTrait,
    E::Model: Send + Sync,
{
    let stmt = apply_filters(E::find(), query_params.filter.clone(), &mappings);

    let (items, total_items) =
        if let (Some(limit), Some(page)) = (query_params.limit, query_params.page) {
            let d = paginate_query(stmt, db, limit, page).await?;
            d
        } else {
            let items = stmt.all(db).await?;
            (items.clone(), items.len())
        };

    Ok((items, total_items))
}
