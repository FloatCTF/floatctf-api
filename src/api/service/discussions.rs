use std::{collections::HashMap, str::FromStr};

use sea_orm::Condition;

use crate::{
    api::{FilterMapping, apply_filters, prelude::*, sea_orm_utils::paginate_query},
    entity::{discussion_comments, discussion_likes, discussions, users},
    prelude::*,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscussionWithAuthor {
    #[serde(flatten)]
    pub discussion: discussions::Model,
    pub author_nickname: String,
    pub author_avatar: Option<String>,
    pub is_liked: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommentWithAuthor {
    #[serde(flatten)]
    pub comment: discussion_comments::Model,
    pub author_nickname: String,
    pub author_avatar: Option<String>,
}

/// GET /api/discussions
#[get("")]
pub async fn get_discussions(
    _user: UserJwtGuard,
    ctx: ReqCtx,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<DiscussionWithAuthor>> {
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

    // Fetch author info for all discussions (if any)
    let authors: Vec<users::Model> = if !items.is_empty() {
        let author_ids: Vec<Uuid> = items.iter().map(|d| d.author_id).collect();
        users::Entity::find()
            .filter(users::Column::Id.is_in(author_ids))
            .all(ctx.db.get_ref())
            .await?
    } else {
        Vec::new()
    };
    let author_map: HashMap<Uuid, &users::Model> = authors.iter().map(|u| (u.id, u)).collect();

    let result: Vec<DiscussionWithAuthor> = items
        .into_iter()
        .map(|d| {
            let author = author_map.get(&d.author_id);
            DiscussionWithAuthor {
                author_nickname: author
                    .map_or_else(|| d.author_id.to_string(), |u| u.nickname.clone()),
                author_avatar: author.and_then(|u| u.avatar.clone()),
                is_liked: false,
                discussion: d,
            }
        })
        .collect();

    query_params.total = Some(total_items);

    UniResponse::ok_meta(result.into(), query_params.into()).into()
}

/// GET /api/discussions/{discussion_id}
#[get("/{discussion_id}")]
pub async fn get_discussion(
    user: UserJwtGuard,
    ctx: ReqCtx,
    path: Path<Uuid>,
) -> UniResult<DiscussionWithAuthor> {
    let discussion_id = path.into_inner();
    let user = user.into_inner();

    let discussion = discussions::Entity::find_by_id(discussion_id)
        .one(ctx.db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(
            "Discussion {} not exist",
            discussion_id
        )))?;

    // Fetch author info
    let author = users::Entity::find_by_id(discussion.author_id)
        .one(ctx.db.get_ref())
        .await?;

    let author_nickname = author
        .as_ref()
        .map_or_else(|| discussion.author_id.to_string(), |u| u.nickname.clone());
    let author_avatar = author.and_then(|u| u.avatar.clone());

    // Check if current user has liked this discussion
    let existing_like = discussion_likes::Entity::find()
        .filter(discussion_likes::Column::DiscussionId.eq(discussion_id))
        .filter(discussion_likes::Column::UserId.eq(user.id))
        .one(ctx.db.get_ref())
        .await?;
    let is_liked = existing_like.is_some();

    // Increment view count — only if viewer is not the author
    let is_author = discussion.author_id == user.id;
    let current_views = discussion.view_count;
    let mut m = discussion.into_active_model();
    if !is_author {
        m.view_count = Set(current_views + 1);
    }
    let updated = m.update(ctx.db.get_ref()).await?;

    let result = DiscussionWithAuthor {
        author_nickname,
        author_avatar,
        is_liked,
        discussion: updated,
    };

    UniResponse::ok(result.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateDiscussionRequest {
    pub title: String,
    pub content: String,
}

/// POST /api/discussions
#[post("")]
pub async fn create_discussion(
    user: UserJwtGuard,
    ctx: ReqCtx,
    cdr: Json<CreateDiscussionRequest>,
) -> UniResult<discussions::Model> {
    let cdr = cdr.into_inner();
    let user = user.into_inner();

    let discussion = discussions::ActiveModel {
        title: Set(cdr.title),
        content: Set(cdr.content),
        author_id: Set(user.id),
        view_count: Set(0),
        like_count: Set(0),
        comment_count: Set(0),
        ..Default::default()
    };
    let discussion = discussion.insert(ctx.db.get_ref()).await?;
    UniResponse::ok(discussion.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatchDiscussionRequest {
    pub title: Option<String>,
    pub content: Option<String>,
}

/// PATCH /api/discussions/{discussion_id}
#[patch("/{discussion_id}")]
pub async fn patch_discussion(
    user: UserJwtGuard,
    ctx: ReqCtx,
    path: Path<Uuid>,
    pdr: Json<PatchDiscussionRequest>,
) -> UniResult<discussions::Model> {
    let discussion_id = path.into_inner();
    let pdr = pdr.into_inner();
    let user = user.into_inner();

    let discussion = discussions::Entity::find_by_id(discussion_id)
        .one(ctx.db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(
            "Discussion {} not exist",
            discussion_id
        )))?;

    // Check if user is the author
    if discussion.author_id != user.id {
        return UniError::NotEnoughPermission.into();
    }

    let mut m = discussion.into_active_model();
    if let Some(title) = pdr.title {
        m.title = Set(title);
    }
    if let Some(content) = pdr.content {
        m.content = Set(content);
    }
    let discussion = m.update(ctx.db.get_ref()).await?;
    UniResponse::ok(discussion.into()).into()
}

/// DELETE /api/discussions/{discussion_id}
#[delete("/{discussion_id}")]
pub async fn delete_discussion(user: UserJwtGuard, ctx: ReqCtx, path: Path<Uuid>) -> UniResult<()> {
    let discussion_id = path.into_inner();
    let user = user.into_inner();

    let discussion = discussions::Entity::find_by_id(discussion_id)
        .one(ctx.db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(
            "Discussion {} not exist",
            discussion_id
        )))?;

    // Check if user is the author
    if discussion.author_id != user.id {
        return UniError::NotEnoughPermission.into();
    }

    discussion.delete(ctx.db.get_ref()).await?;
    UniResponse::ok_none().into()
}

/// POST /api/discussions/{discussion_id}/like
#[post("/{discussion_id}/like")]
pub async fn like_discussion(user: UserJwtGuard, ctx: ReqCtx, path: Path<Uuid>) -> UniResult<()> {
    let discussion_id = path.into_inner();
    let user = user.into_inner();

    // Check if discussion exists
    let discussion = discussions::Entity::find_by_id(discussion_id)
        .one(ctx.db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(
            "Discussion {} not exist",
            discussion_id
        )))?;

    // Check if already liked
    let existing = discussion_likes::Entity::find()
        .filter(discussion_likes::Column::DiscussionId.eq(discussion_id))
        .filter(discussion_likes::Column::UserId.eq(user.id))
        .one(ctx.db.get_ref())
        .await?;

    if existing.is_some() {
        return UniError::BadRequest("Already liked".to_string()).into();
    }

    // Create like
    let like = discussion_likes::ActiveModel {
        discussion_id: Set(discussion_id),
        user_id: Set(user.id),
        ..Default::default()
    };
    like.insert(ctx.db.get_ref()).await?;

    // Update like count
    let new_like_count = discussion.like_count + 1;
    let mut m = discussion.into_active_model();
    m.like_count = Set(new_like_count);
    m.update(ctx.db.get_ref()).await?;

    UniResponse::ok_none().into()
}

/// DELETE /api/discussions/{discussion_id}/like
#[delete("/{discussion_id}/like")]
pub async fn unlike_discussion(user: UserJwtGuard, ctx: ReqCtx, path: Path<Uuid>) -> UniResult<()> {
    let discussion_id = path.into_inner();
    let user = user.into_inner();

    let discussion = discussions::Entity::find_by_id(discussion_id)
        .one(ctx.db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(
            "Discussion {} not exist",
            discussion_id
        )))?;

    // Find and delete like
    let like = discussion_likes::Entity::find()
        .filter(discussion_likes::Column::DiscussionId.eq(discussion_id))
        .filter(discussion_likes::Column::UserId.eq(user.id))
        .one(ctx.db.get_ref())
        .await?;

    if let Some(like) = like {
        like.delete(ctx.db.get_ref()).await?;

        // Update like count
        let new_like_count = (discussion.like_count - 1).max(0);
        let mut m = discussion.into_active_model();
        m.like_count = Set(new_like_count);
        m.update(ctx.db.get_ref()).await?;
    }

    UniResponse::ok_none().into()
}

/// GET /api/discussions/{discussion_id}/comments
#[get("/{discussion_id}/comments")]
pub async fn get_discussion_comments(
    _user: UserJwtGuard,
    ctx: ReqCtx,
    path: Path<Uuid>,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<CommentWithAuthor>> {
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

    // Fetch author info
    let authors: Vec<users::Model> = if !items.is_empty() {
        let author_ids: Vec<Uuid> = items.iter().map(|c| c.author_id).collect();
        users::Entity::find()
            .filter(users::Column::Id.is_in(author_ids))
            .all(ctx.db.get_ref())
            .await?
    } else {
        Vec::new()
    };
    let author_map: HashMap<Uuid, &users::Model> = authors.iter().map(|u| (u.id, u)).collect();

    let results: Vec<CommentWithAuthor> = items
        .into_iter()
        .map(|c| {
            let author = author_map.get(&c.author_id);
            CommentWithAuthor {
                author_nickname: author
                    .map_or_else(|| c.author_id.to_string(), |u| u.nickname.clone()),
                author_avatar: author.and_then(|u| u.avatar.clone()),
                comment: c,
            }
        })
        .collect();

    query_params.total = Some(total_items);

    UniResponse::ok_meta(results.into(), query_params.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCommentRequest {
    pub content: String,
    pub parent_id: Option<Uuid>,
}

/// POST /api/discussions/{discussion_id}/comments
#[post("/{discussion_id}/comments")]
pub async fn create_comment(
    user: UserJwtGuard,
    ctx: ReqCtx,
    path: Path<Uuid>,
    ccr: Json<CreateCommentRequest>,
) -> UniResult<discussion_comments::Model> {
    let discussion_id = path.into_inner();
    let ccr = ccr.into_inner();
    let user = user.into_inner();

    // Check if discussion exists
    let discussion = discussions::Entity::find_by_id(discussion_id)
        .one(ctx.db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(
            "Discussion {} not exist",
            discussion_id
        )))?;

    // If parent_id is provided, check it exists and belongs to same discussion
    if let Some(parent_id) = ccr.parent_id {
        let parent = discussion_comments::Entity::find_by_id(parent_id)
            .one(ctx.db.get_ref())
            .await?
            .ok_or(UniError::NotFound(format!(
                "Parent comment {} not exist",
                parent_id
            )))?;
        if parent.discussion_id != discussion_id {
            return UniError::BadRequest(
                "Parent comment does not belong to this discussion".to_string(),
            )
            .into();
        }
    }

    let comment = discussion_comments::ActiveModel {
        discussion_id: Set(discussion_id),
        author_id: Set(user.id),
        content: Set(ccr.content),
        parent_id: Set(ccr.parent_id),
        ..Default::default()
    };
    let comment = comment.insert(ctx.db.get_ref()).await?;

    // Update comment count
    let new_comment_count = discussion.comment_count + 1;
    let mut m = discussion.into_active_model();
    m.comment_count = Set(new_comment_count);
    m.update(ctx.db.get_ref()).await?;

    UniResponse::ok(comment.into()).into()
}

/// PATCH /api/discussions/{discussion_id}/comments/{comment_id}
#[patch("/{discussion_id}/comments/{comment_id}")]
pub async fn patch_comment(
    user: UserJwtGuard,
    ctx: ReqCtx,
    path: Path<(Uuid, Uuid)>,
    pcr: Json<PatchCommentRequest>,
) -> UniResult<discussion_comments::Model> {
    let (discussion_id, comment_id) = path.into_inner();
    let pcr = pcr.into_inner();
    let user = user.into_inner();

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

    if comment.author_id != user.id {
        return UniError::NotEnoughPermission.into();
    }

    let mut m = comment.into_active_model();
    if let Some(content) = pcr.content {
        m.content = Set(content);
    }
    let comment = m.update(ctx.db.get_ref()).await?;
    UniResponse::ok(comment.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatchCommentRequest {
    pub content: Option<String>,
}

/// DELETE /api/discussions/{discussion_id}/comments/{comment_id}
#[delete("/{discussion_id}/comments/{comment_id}")]
pub async fn delete_comment(
    user: UserJwtGuard,
    ctx: ReqCtx,
    path: Path<(Uuid, Uuid)>,
) -> UniResult<()> {
    let (discussion_id, comment_id) = path.into_inner();
    let user = user.into_inner();

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

    if comment.author_id != user.id {
        return UniError::NotEnoughPermission.into();
    }

    comment.delete(ctx.db.get_ref()).await?;

    // Update comment count
    let discussion = discussions::Entity::find_by_id(discussion_id)
        .one(ctx.db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(
            "Discussion {} not exist",
            discussion_id
        )))?;

    let new_comment_count = (discussion.comment_count - 1).max(0);
    let mut m = discussion.into_active_model();
    m.comment_count = Set(new_comment_count);
    m.update(ctx.db.get_ref()).await?;

    UniResponse::ok_none().into()
}
