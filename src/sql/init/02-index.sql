-- users 表索引
CREATE UNIQUE INDEX IF NOT EXISTS "idx_users_username" ON "users" ("username");

CREATE UNIQUE INDEX IF NOT EXISTS "idx_users_email" ON "users" ("email");

-- instances 表索引
CREATE INDEX IF NOT EXISTS "idx_instances_status" ON "instances" ("status");

CREATE INDEX IF NOT EXISTS "idx_instances_challenge_id" ON "instances" ("challenge_id");

CREATE INDEX IF NOT EXISTS "idx_instances_user_id" ON "instances" ("user_id");

-- events 表索引
CREATE INDEX IF NOT EXISTS "idx_events_type" ON "events" ("type");

CREATE INDEX IF NOT EXISTS "idx_events_start_time" ON "events" ("start_time");

CREATE INDEX IF NOT EXISTS "idx_events_end_time" ON "events" ("end_time");

-- event_users 表索引
CREATE INDEX IF NOT EXISTS "idx_event_users_user_id" ON "event_users" ("user_id");

CREATE INDEX IF NOT EXISTS "idx_event_users_event_id" ON "event_users" ("event_id");

-- event_instances 表索引
CREATE INDEX IF NOT EXISTS "idx_event_instances_event_id" ON "event_instances" ("event_id");

CREATE INDEX IF NOT EXISTS "idx_event_instances_instance_id" ON "event_instances" ("instance_id");

-- event_teams 表索引
CREATE INDEX IF NOT EXISTS "idx_event_teams_event_id" ON "event_teams" ("event_id");

-- event_team_members 表索引
CREATE INDEX IF NOT EXISTS "idx_event_team_members_team_id" ON "event_team_members" ("team_id");

CREATE INDEX IF NOT EXISTS "idx_event_team_members_user_id" ON "event_team_members" ("user_id");

CREATE INDEX "idx_event_user_challenge" ON "event_instances" (
    "event_id",
    "user_id",
    "challenge_id"
);

CREATE INDEX "idx_event_team_challenge" ON "event_instances" (
    "event_id",
    "team_id",
    "challenge_id"
);

-- 2. 调度器专用的极速轮询索引 (极其重要)
CREATE INDEX idx_scheduled_tasks_poll
ON "scheduled_tasks" ("status", "execute_at")
WHERE "status" = 'pending';

-- 3. 业务组关联索引
CREATE INDEX idx_scheduled_tasks_group ON "scheduled_tasks" ("group_id");
