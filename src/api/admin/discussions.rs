use std::str::FromStr;

use sea_orm::Condition;

use crate::{
    api::{
        FilterMapping, admin::dto::DeleteItemsRequest, apply_filters, prelude::*,
        sea_orm_utils::paginate_query,
    },
    entity::{discussion_comments, discussions},
    prelude::*,
};

/// GET /api/admin/discussions
#[get("")]
pub async fn get_discussions(
    _user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<discussions::Model>> {
    let mut query_params = query_params.0;

    let mappings = [
        FilterMapping {
            key: "id",
            column: Box::new(|v| {
                Condition::all()
                    .add(discussions::Column::Id.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())))
            }),
        },
        FilterMapping {
            key: "title",
            column: Box::new(|v| Condition::all().add(discussions::Column::Title.contains(v))),
        },
        FilterMapping {
            key: "author_id",
            column: Box::new(|v| {
                Condition::all().add(
                    discussions::Column::AuthorId.eq(Uuid::from_str(&v).unwrap_or(Uuid::nil())),
                )
            }),
        },
    ];

    let stmt = discussions::Entity::find();
    let stmt = apply_filters(stmt, query_params.filter.clone(), &mappings);
    let stmt = stmt.order_by_desc(discussions::Column::UpdatedAt);

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

/// GET /api/admin/discussions/{discussion_id}
#[get("/{discussion_id}")]
pub async fn get_discussion(
    _user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    path: Path<Uuid>,
) -> UniResult<discussions::Model> {
    let discussion_id = path.into_inner();

    let discussion = discussions::Entity::find_by_id(discussion_id)
        .one(ctx.db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(
            "Discussion {} not exist",
            discussion_id
        )))?;

    UniResponse::ok(discussion.into()).into()
}

/// DELETE /api/admin/discussions
#[delete("")]
pub async fn delete_discussions(
    _user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    dir: Json<DeleteItemsRequest>,
) -> UniResult<u64> {
    let dir = dir.into_inner();
    let mut deleted_count = 0;
    for discussion_id in dir.id_list {
        let discussion = discussions::Entity::find_by_id(discussion_id)
            .one(ctx.db.get_ref())
            .await?;
        if let Some(discussion) = discussion {
            let r = discussion.delete(ctx.db.get_ref()).await?;
            deleted_count += r.rows_affected;
        }
    }
    UniResponse::ok(deleted_count.into()).into()
}

/// GET /api/admin/discussions/{discussion_id}/comments
#[get("/{discussion_id}/comments")]
pub async fn get_discussion_comments(
    _user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    path: Path<Uuid>,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<discussion_comments::Model>> {
    let discussion_id = path.into_inner();
    let mut query_params = query_params.0;

    let stmt = discussion_comments::Entity::find()
        .filter(discussion_comments::Column::DiscussionId.eq(discussion_id));
    let stmt = stmt.order_by_desc(discussion_comments::Column::CreatedAt);

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

/// DELETE /api/admin/discussions/{discussion_id}/comments/{comment_id}
#[delete("/{discussion_id}/comments/{comment_id}")]
pub async fn delete_comment(
    _user: SuperAdminJwtGuard,
    ctx: ReqCtx,
    path: Path<(Uuid, Uuid)>,
) -> UniResult<()> {
    let (discussion_id, comment_id) = path.into_inner();

    let comment = discussion_comments::Entity::find_by_id(comment_id)
        .one(ctx.db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(
            "Comment {} not exist",
            comment_id
        )))?;

    if comment.discussion_id != discussion_id {
        return UniError::BadRequest("Comment does not belong to this discussion".to_string())
            .into();
    }

    comment.delete(ctx.db.get_ref()).await?;
    UniResponse::ok_none().into()
}
