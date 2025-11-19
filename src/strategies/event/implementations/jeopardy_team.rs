use crate::{
    api::preclude::*,
    entity::{
        sea_orm_active_enums::InstanceStatus,
        {
            challenges, event_challenge_solves, event_challenges, event_instances,
            event_team_members, event_teams, instances,
        },
    },
    strategies::event::{EventStrategy, common, trait_def::*},
};

pub struct JeopardyTeamStrategy;

#[async_trait]
impl EventStrategy for JeopardyTeamStrategy {
    async fn submit(&self, ctx: &EventContext, sfr: SubmitFlagRequest) -> Result<()> {
        self.should_user_joined(ctx).await?;
        self.should_ongoing(ctx)?;

        let db = ctx.db.get_ref();
        // guard
        let instance_id = sfr.instance_id.ok_or(anyhow!("no instance_id"))?;

        // get team_members
        let team_member = event_team_members::Entity::find()
            .filter(
                event_team_members::Column::EventId
                    .eq(ctx.event.id)
                    .and(event_team_members::Column::UserId.eq(ctx.user.id)),
            )
            .one(db)
            .await?
            .ok_or_else(|| anyhow!("you are not in any team"))?;

        //  get challenge & instance
        let instance = instances::Entity::find_by_id(instance_id)
            .filter(instances::Column::Status.eq(InstanceStatus::Running))
            .one(db)
            .await?
            .ok_or(anyhow!("no instance"))?;

        let challenge = challenges::Entity::find_by_id(instance.challenge_id)
            .one(db)
            .await?
            .ok_or(anyhow!("no challenge"))?;

        // check flag
        if sfr.flag != instance.flag {
            return Err(anyhow!("flag is not correct"));
        }

        //  check solved?
        if let Some(_old_challenge_solve) = event_challenge_solves::Entity::find()
            .filter(
                event_challenge_solves::Column::EventId
                    .eq(ctx.event.id)
                    .and(event_challenge_solves::Column::ChallengeId.eq(challenge.id))
                    .and(event_challenge_solves::Column::TeamId.eq(team_member.team_id)),
            )
            .one(db)
            .await?
        {
            return Ok(()); // Already solved, nothing to do
        }

        //  add points
        let event_challenge = event_challenges::Entity::find_by_id((ctx.event.id, challenge.id))
            .one(db)
            .await?
            .ok_or_else(|| anyhow!("no event_challenge"))?;

        let solved_count = event_challenge_solves::Entity::find()
            .filter(
                event_challenge_solves::Column::EventId
                    .eq(ctx.event.id)
                    .and(event_challenge_solves::Column::ChallengeId.eq(challenge.id)),
            )
            .count(db)
            .await?;

        let current_points =
            common::calculate_next_dynamic_score(db, event_challenge.points, solved_count)
                .await
                .map_err(|e| anyhow!("calculate_next_dynamic_score error: {}", e))?;

        let event_team = event_teams::Entity::find_by_id(team_member.team_id)
            .one(db)
            .await?
            .ok_or_else(|| anyhow!("no event_team"))?;
        // banned?
        if event_team.banned {
            return Err(anyhow!("you are banned"));
        }

        // update points
        let new_points = event_team.points + current_points;
        let mut m_event_team = event_team.into_active_model();
        m_event_team.points = Set(new_points);
        m_event_team.update(db).await?;

        // solved success!
        event_challenge_solves::ActiveModel {
            event_id: Set(ctx.event.id),
            challenge_id: Set(challenge.id),
            team_id: Set(Some(team_member.team_id)),
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

        let team_member = event_team_members::Entity::find()
            .filter(
                event_team_members::Column::EventId
                    .eq(ctx.event.id)
                    .and(event_team_members::Column::UserId.eq(ctx.user.id)),
            )
            .one(ctx.db.get_ref())
            .await?
            .ok_or(anyhow!("you are not in any team"))?;

        let (_event_instance, instance) = event_instances::Entity::find()
            .filter(
                event_instances::Column::EventId
                    .eq(ctx.event.id)
                    .and(event_instances::Column::ChallengeId.eq(challenge_id))
                    .and(event_instances::Column::TeamId.eq(team_member.team_id)),
            )
            .find_also_related(instances::Entity)
            .filter(
                instances::Column::Status
                    .eq(InstanceStatus::Running)
                    .and(instances::Column::Ref.eq("JeopardyTeam")),
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

        let team_member = event_team_members::Entity::find()
            .filter(event_team_members::Column::EventId.eq(ctx.event.id))
            .filter(event_team_members::Column::UserId.eq(ctx.user.id))
            .one(db)
            .await?
            .ok_or(UniError::NotFound("you are not in any team".into()))?;

        let data = event_instances::Entity::find()
            .filter(event_instances::Column::EventId.eq(ctx.event.id))
            .filter(event_instances::Column::TeamId.eq(team_member.team_id))
            .find_also_related(instances::Entity)
            .filter(instances::Column::Status.eq(InstanceStatus::Running))
            .filter(instances::Column::Ref.eq("JeopardyTeam"))
            .find_also_related(challenges::Entity)
            .all(db)
            .await?;

        // 👇 把结果组装成 EventInstance
        let instances: Vec<EventInstanceResult> = data
            .into_iter()
            .map(
                |(_event_instance, instance, challenge)| EventInstanceResult {
                    instance: instance.unwrap(),
                    challenge_name: challenge.map(|c| c.name).unwrap_or_default(),
                    nickname: "team_".to_string(), // 团队赛没有用户昵称 TODO: 这里应该是团队名称
                },
            )
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

        let db = ctx.db.get_ref();
        let user = ctx.user.clone();
        let event_id = ctx.event.id;

        let (team_id, team_member_count) = {
            let team_member = event_team_members::Entity::find()
                .filter(
                    event_team_members::Column::EventId
                        .eq(event_id)
                        .and(event_team_members::Column::UserId.eq(user.id)),
                )
                .one(db)
                .await?
                .ok_or(UniError::NotFound("you are not in any team".into()))?;

            let team_member_count = event_team_members::Entity::find()
                .filter(event_team_members::Column::TeamId.eq(team_member.team_id))
                .count(db)
                .await?;

            (team_member.team_id, team_member_count)
        };

        // team_members * 2
        let running_instances_count = event_instances::Entity::find()
            .filter(
                event_instances::Column::EventId
                    .eq(event_id)
                    .and(event_instances::Column::UserId.eq(user.id))
                    .and(event_instances::Column::TeamId.eq(team_id)),
            )
            .join(
                JoinType::InnerJoin,
                event_instances::Relation::Instances.def(),
            )
            .filter(
                instances::Column::Status
                    .eq(InstanceStatus::Running)
                    .and(instances::Column::Ref.eq("JeopardyTeam")),
            )
            .count(db)
            .await?;

        let max_instances_per_user = team_member_count * 2;

        if running_instances_count >= max_instances_per_user {
            return Err(anyhow!(
                "you can only launch {} instances at the same time in JeopardyTeam mode",
                max_instances_per_user
            ));
        }

        let running_instance = event_instances::Entity::find()
            .filter(
                event_instances::Column::EventId
                    .eq(event_id)
                    .and(event_instances::Column::ChallengeId.eq(challenge_id))
                    .and(event_instances::Column::TeamId.eq(team_id)),
            )
            .find_also_related(instances::Entity)
            .filter(instances::Column::Status.eq(InstanceStatus::Running))
            .one(db)
            .await?;

        if let Some((_, Some(instance))) = running_instance {
            return Ok(instance);
        }

        let identifier = {
            let event_id_prefix = common::get_uuid_prefix(&event_id);
            let team_id_prefix = common::get_uuid_prefix(&team_id);
            let challenge_id_prefix = common::get_uuid_prefix(&challenge_id);
            format!(
                "JT-{}-{}-{}",
                event_id_prefix, team_id_prefix, challenge_id_prefix
            )
        };

        let res_instance = common::launch_instance(
            &ctx.db,
            &ctx.docker,
            challenge_id,
            identifier,
            user.id,
            "JeopardyTeam".into(),
            ctx.event.flag_prefix.clone(),
        )
        .await
        .map_err(|e| anyhow!(e))?;

        let new_event_instance = event_instances::ActiveModel {
            event_id: Set(event_id),
            challenge_id: Set(challenge_id),
            user_id: Set(user.id),
            instance_id: Set(res_instance.id),
            team_id: Set(Some(team_id)),
            ..Default::default()
        };
        new_event_instance.insert(db).await?;

        Ok(res_instance)
    }
}
