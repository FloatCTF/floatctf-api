use crate::{
    api::prelude::*,
    entity::{
        challenges, event_challenge_solves, event_challenges, event_instances, event_users,
        instances, sea_orm_active_enums::InstanceStatus, users,
    },
    strategies::event::{common, trait_def::*},
};

pub struct JeopardySingleStrategy;

#[async_trait]
impl EventStrategy for JeopardySingleStrategy {
    async fn submit(&self, ctx: &EventContext, sfr: SubmitFlagRequest) -> Result<()> {
        self.should_user_joined(ctx).await?;
        self.should_ongoing(ctx)?;

        let db = ctx.db.get_ref();
        // guard
        let instance_id = sfr.instance_id.ok_or(anyhow!("no instance_id"))?;

        // get instance & challenge
        let instance = instances::Entity::find_by_id(instance_id)
            .filter(instances::Column::Status.eq(InstanceStatus::Running))
            .one(db)
            .await?
            .ok_or(anyhow!("no instance"))?;

        let challenge =
            challenges::Entity::find_by_id(instance.challenge_id.ok_or(UniError::NotFound(
                format!("instance has no challeng_id: {}", instance.id.to_string()),
            ))?)
            .one(db)
            .await?
            .ok_or(anyhow!("no challenge"))?;

        if sfr.flag != instance.flag {
            return Err(anyhow!("wrong flag"));
        }

        // check solved?
        if let Some(_old_challenge_solve) = event_challenge_solves::Entity::find()
            .filter(event_challenge_solves::Column::EventId.eq(ctx.event.id))
            .filter(event_challenge_solves::Column::ChallengeId.eq(challenge.id))
            .filter(event_challenge_solves::Column::UserId.eq(ctx.user.id))
            .one(db)
            .await?
        {
            return Ok(());
        }

        let event_challenge = event_challenges::Entity::find_by_id((ctx.event.id, challenge.id))
            .one(db)
            .await?
            .ok_or(anyhow!("no event_challenge"))?;

        let solved_count = event_challenge_solves::Entity::find()
            .filter(
                event_challenge_solves::Column::EventId
                    .eq(ctx.event.id)
                    .and(event_challenge_solves::Column::ChallengeId.eq(challenge.id)),
            )
            .count(db)
            .await?;

        //  更新分数
        let current_points = common::calculate_next_dynamic_score(
            ctx.db.get_ref(),
            event_challenge.points,
            solved_count,
        )
        .await
        .map_err(|e| anyhow!("calculate_next_dynamic_score error: {}", e))?;

        let event_user = event_users::Entity::find_by_id((ctx.event.id, ctx.user.id))
            .one(db)
            .await?
            .ok_or(anyhow!("no event_user"))?;

        if event_user.banned {
            // banned!
            return Err(anyhow!("you are banned"));
        }

        // update points
        let new_points = event_user.points + current_points;
        let mut m_event_user = event_user.into_active_model();
        m_event_user.points = Set(new_points);
        m_event_user.update(db).await?;

        event_challenge_solves::ActiveModel {
            event_id: Set(ctx.event.id),
            challenge_id: Set(challenge.id),
            user_id: Set(ctx.user.id),
            bonus_points: Set(current_points),
            ..Default::default()
        }
        .insert(db)
        .await?;

        // destroy instance
        common::destroy_instance(&ctx.db, &ctx.docker, instance_id, &ctx.user).await?;

        Ok(())
    }

    async fn get_instance_by_challenge_id(
        &self,
        ctx: &EventContext,
        challenge_id: Uuid,
    ) -> Result<instances::Model> {
        self.should_user_joined(ctx).await?;
        self.should_ongoing_or_ended(ctx)?;

        let (_event_instance, instance) = event_instances::Entity::find()
            .filter(
                event_instances::Column::EventId
                    .eq(ctx.event.id)
                    .and(event_instances::Column::UserId.eq(ctx.user.id)),
            )
            .find_also_related(instances::Entity)
            .filter(
                instances::Column::Status
                    .eq(InstanceStatus::Running)
                    .and(instances::Column::Ref.eq("JeopardySingle"))
                    .and(instances::Column::ChallengeId.eq(challenge_id)),
            )
            .one(ctx.db.get_ref())
            .await?
            .ok_or(anyhow!("no instance"))?;

        instance.ok_or(anyhow!("no instance"))
    }

    async fn get_instances(&self, ctx: &EventContext) -> Result<Vec<EventInstanceResult>> {
        self.should_user_joined(ctx).await?;
        self.should_ongoing_or_ended(ctx)?;

        let db = ctx.db.get_ref();

        // 查 instance 并关联 challenge 和 user
        let data = instances::Entity::find()
            .filter(instances::Column::Status.eq(InstanceStatus::Running))
            .filter(instances::Column::UserId.eq(ctx.user.id))
            .filter(instances::Column::Ref.eq("JeopardySingle"))
            .find_also_related(challenges::Entity) // instance -> challenge
            .find_also_related(users::Entity) // instance -> user
            .all(db)
            .await?;

        // 把结果组装成 EventInstance
        let instances: Vec<EventInstanceResult> = data
            .into_iter()
            .map(|(instance, challenge_opt, user_opt)| EventInstanceResult {
                instance,
                challenge_name: challenge_opt.map(|c| c.name).unwrap_or_default(),
                nickname: user_opt.map(|u| u.nickname).unwrap_or_default(),
            })
            .collect();

        Ok(instances)
    }

    async fn launch_instance(
        &self,
        ctx: &EventContext,
        challenge_id: Uuid,
    ) -> Result<instances::Model> {
        self.should_user_joined(ctx).await?;
        self.should_ongoing(ctx)?;

        let event_id = ctx.event.id;
        let db = ctx.db.get_ref();

        let running_instances_count = event_instances::Entity::find()
            .filter(
                event_instances::Column::EventId
                    .eq(event_id)
                    .and(event_instances::Column::UserId.eq(ctx.user.id)),
            )
            .join(
                JoinType::InnerJoin,
                event_instances::Relation::Instances.def(),
            )
            .filter(
                instances::Column::Status
                    .eq(InstanceStatus::Running)
                    .and(instances::Column::Ref.eq("JeopardySingle")),
            )
            .count(db)
            .await?;

        let max_instances_per_user = 2 as u64;

        if running_instances_count >= max_instances_per_user {
            return Err(anyhow!(
                "you can only launch {} instances at the same time in JeopardySingle mode",
                max_instances_per_user
            ));
        }

        // 检查是否已有运行实例
        if let Some((_, Some(instance))) = event_instances::Entity::find()
            .filter(
                event_instances::Column::EventId
                    .eq(event_id)
                    .and(event_instances::Column::UserId.eq(ctx.user.id)),
            )
            .find_also_related(instances::Entity)
            .filter(
                instances::Column::Status
                    .eq(InstanceStatus::Running)
                    .and(instances::Column::ChallengeId.eq(challenge_id)),
            )
            .one(db)
            .await?
        {
            return Ok(instance);
        }

        // 调用公共启动逻辑
        let identifier = {
            let event_id_prefix = common::get_uuid_prefix(&event_id);
            let user_id_prefix = common::get_uuid_prefix(&ctx.user.id);
            let challenge_id_prefix = common::get_uuid_prefix(&challenge_id);
            format!(
                "JS-{}-{}-{}",
                event_id_prefix, user_id_prefix, challenge_id_prefix
            )
        };

        let res_instance = common::launch_instance(
            &ctx.db,
            &ctx.docker,
            challenge_id,
            identifier,
            ctx.user.id.clone(),
            "JeopardySingle".into(),
            ctx.event.flag_prefix.clone(),
        )
        .await
        .map_err(|e| anyhow!(e))?;

        // 写入 event_instances 记录
        let new_event_instance = event_instances::ActiveModel {
            event_id: Set(event_id),
            user_id: Set(ctx.user.id),
            instance_id: Set(res_instance.id),
            team_id: Set(None),
            ..Default::default()
        };
        new_event_instance.insert(db).await?;

        Ok(res_instance)
    }
}
