use crate::{
    api::prelude::*,
    entity::{challenge_solves, challenges, instances, sea_orm_active_enums::InstanceStatus},
    strategies::event::{common, trait_def::*},
};

pub struct JeopardyPractice;

#[async_trait]
impl EventStrategy for JeopardyPractice {
    async fn submit(&self, ctx: &EventContext, sfr: SubmitFlagRequest) -> Result<()> {
        let db = ctx.db.get_ref();

        let instance_id = sfr.instance_id.ok_or(anyhow!("no instance_id"))?;

        let instance = instances::Entity::find_by_id(instance_id)
            .filter(instances::Column::Status.eq(InstanceStatus::Running))
            .one(db)
            .await?
            .ok_or(anyhow!("no instance"))?;

        let challenge = challenges::Entity::find_by_id(instance.challenge_id.ok_or(anyhow!(
            "instance has no challenge_id: {}",
            instance.id.to_string()
        ))?)
        .one(db)
        .await?
        .ok_or(anyhow!("no challenge"))?;

        if sfr.flag != instance.flag {
            return Err(anyhow!("flag is not correct"));
        }

        let old_challenge_solve = challenge_solves::Entity::find()
            .filter(
                challenge_solves::Column::ChallengeId
                    .eq(challenge.id)
                    .and(challenge_solves::Column::UserId.eq(ctx.user.id)),
            )
            .one(db)
            .await?;

        match old_challenge_solve {
            Some(challenge_solve) => challenge_solve,
            None => {
                challenge_solves::ActiveModel {
                    event_id: Set(None), // no event_id for practice
                    challenge_id: Set(challenge.id),
                    user_id: Set(ctx.user.id),
                    ..Default::default()
                }
                .insert(db)
                .await?
            }
        };

        common::destroy_instance(&ctx.db, &ctx.docker, instance_id, &ctx.user).await?;

        Ok(())
    }

    async fn get_instance_by_challenge_id(
        &self,
        _ctx: &EventContext,
        _challenge_id: Uuid,
    ) -> Result<instances::Model> {
        Err(anyhow!("no need to implement"))
    }

    async fn get_instances(&self, _ctx: &EventContext) -> Result<Vec<EventInstanceResult>> {
        Err(anyhow!("no need to implement"))
    }

    async fn launch_instance(
        &self,
        ctx: &EventContext,
        challenge_id: Uuid,
    ) -> Result<instances::Model> {
        let db = ctx.db.get_ref();
        let user = ctx.user.clone();

        let running_instances_count = instances::Entity::find()
            .filter(
                instances::Column::Status
                    .eq(InstanceStatus::Running)
                    .and(instances::Column::UserId.eq(user.id))
                    .and(instances::Column::Ref.eq("JeopardyPractice")),
            )
            .count(db)
            .await?;

        let max_instances_per_user = 1 as u64;

        if running_instances_count >= max_instances_per_user {
            return Err(anyhow!(
                "you can only launch {} instances at the same time in practice mode",
                max_instances_per_user
            ));
        }

        // 是否已经有运行中的实例
        if let Some(running_instance) = instances::Entity::find()
            .filter(
                instances::Column::Status
                    .eq(InstanceStatus::Running)
                    .and(instances::Column::ChallengeId.eq(challenge_id))
                    .and(instances::Column::UserId.eq(user.id)),
            )
            .one(db)
            .await?
        {
            return Ok(running_instance);
        }

        // 调用公共函数启动实例
        let identifier = {
            let user_id_prefix = common::get_uuid_prefix(&user.id);
            let challenge_id_prefix = common::get_uuid_prefix(&challenge_id);
            format!("JP-{}-{}", user_id_prefix, challenge_id_prefix)
        };

        let res_instance = common::launch_instance(
            &ctx.db,
            &ctx.docker,
            challenge_id,
            identifier,
            user.id,
            "JeopardyPractice".into(),
            None,
        )
        .await
        .map_err(|e| anyhow!(e.to_string()))?;

        Ok(res_instance)
    }
}
