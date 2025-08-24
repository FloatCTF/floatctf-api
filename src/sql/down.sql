DROP EXTENSION IF EXISTS "uuid-ossp" CASCADE;

DROP FUNCTION IF EXISTS update_updated_at_column () CASCADE;
-- 删除触发器
DROP TRIGGER IF EXISTS "trg_users_updated_at" ON "users";

DROP TRIGGER IF EXISTS "trg_super_admin_updated_at" ON "super_admin";

DROP TRIGGER IF EXISTS "trg_challenges_updated_at" ON "challenges";

DROP TRIGGER IF EXISTS "trg_instances_updated_at" ON "instances";

DROP TRIGGER IF EXISTS "trg_events_updated_at" ON "events";

DROP TRIGGER IF EXISTS "trg_event_teams_updated_at" ON "event_teams";

-- 删除索引
DROP INDEX IF EXISTS "idx_users_username";

DROP INDEX IF EXISTS "idx_users_email";

DROP INDEX IF EXISTS "idx_instances_status";

DROP INDEX IF EXISTS "idx_instances_challenge_id";

DROP INDEX IF EXISTS "idx_instances_user_id";

DROP INDEX IF EXISTS "idx_events_type";

DROP INDEX IF EXISTS "idx_events_start_time";

DROP INDEX IF EXISTS "idx_events_end_time";

DROP INDEX IF EXISTS "idx_event_users_user_id";

DROP INDEX IF EXISTS "idx_event_users_event_id";

DROP INDEX IF EXISTS "idx_event_instances_event_id";

DROP INDEX IF EXISTS "idx_event_instances_instance_id";

DROP INDEX IF EXISTS "idx_event_teams_event_id";

DROP INDEX IF EXISTS "idx_event_team_members_team_id";

DROP INDEX IF EXISTS "idx_event_team_members_user_id";

-- 删除表
DROP TABLE IF EXISTS "event_team_members" CASCADE;

DROP TABLE IF EXISTS "event_teams" CASCADE;

DROP TABLE IF EXISTS "event_users" CASCADE;

DROP TABLE IF EXISTS "event_instances" CASCADE;

DROP TABLE IF EXISTS "event_challenges" CASCADE;

DROP TABLE IF EXISTS "events" CASCADE;

DROP TABLE IF EXISTS "instances" CASCADE;

DROP TABLE IF EXISTS "challenges" CASCADE;

DROP TABLE IF EXISTS "super_admin" CASCADE;

DROP TABLE IF EXISTS "users" CASCADE;

DROP TABLE IF EXISTS "challenge_solves" CASCADE;
-- 删除类型
DROP TYPE IF EXISTS "event_team_member_role";

DROP TYPE IF EXISTS "event_status";

DROP TYPE IF EXISTS "event_type";

DROP TYPE IF EXISTS "instance_status";