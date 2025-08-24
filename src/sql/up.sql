CREATE EXTENSION "uuid-ossp";

CREATE TABLE IF NOT EXISTS "users" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    "username" TEXT NOT NULL UNIQUE,
    "password_hash" TEXT NOT NULL,
    "email" TEXT NOT NULL,
    "created_at" TIMESTAMP NOT NULL DEFAULT now(),
    "updated_at" TIMESTAMP NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS "super_admin" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    "username" TEXT NOT NULL UNIQUE,
    "password_hash" TEXT NOT NULL,
    "email" TEXT NOT NULL,
    "created_at" TIMESTAMP NOT NULL DEFAULT now(),
    "updated_at" TIMESTAMP NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS "challenges" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    "name" TEXT NOT NULL UNIQUE,
    "category" TEXT NOT NULL DEFAULT 'other',
    "description" TEXT NOT NULL DEFAULT 'no description',
    "attachment" TEXT NULL,
    "hidden" BOOLEAN NOT NULL DEFAULT FALSE,
    "toml_str" TEXT NOT NULL,
    "created_at" TIMESTAMP NOT NULL DEFAULT now(),
    "updated_at" TIMESTAMP NOT NULL DEFAULT now()
);

CREATE TYPE "instance_status" AS ENUM ('pending', 'running', 'completed', 'failed');

CREATE TABLE IF NOT EXISTS "instances" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    "status" "instance_status" NOT NULL DEFAULT 'pending',
    "ref" TEXT NOT NULL DEFAULT 'Training',
    "flag" TEXT NOT NULL,
    "content" TEXT,
    "challenge_id" UUID NOT NULL REFERENCES "challenges" (id) ON DELETE CASCADE,
    "user_id" UUID NOT NULL REFERENCES "users" (id) ON DELETE CASCADE,
    "created_at" TIMESTAMP NOT NULL DEFAULT now(),
    "updated_at" TIMESTAMP NOT NULL DEFAULT now()
);

CREATE TYPE "event_type" AS ENUM ('jeopardy_single', 'jeopardy_team', 'awd_team');

CREATE TYPE "event_status" AS ENUM ('pending', 'running', 'completed', 'failed');

CREATE TABLE IF NOT EXISTS "events" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    "type" "event_type" NOT NULL DEFAULT 'jeopardy_single',
    "title" TEXT NOT NULL,
    "description" TEXT,
    "hidden" BOOLEAN NOT NULL DEFAULT TRUE,
    "start_time" TIMESTAMP NOT NULL,
    "end_time" TIMESTAMP NOT NULL,
    "created_at" TIMESTAMP NOT NULL DEFAULT now(),
    "updated_at" TIMESTAMP NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS "event_challenges" (
    "event_id" UUID NOT NULL REFERENCES "events" ("id") ON DELETE CASCADE,
    "challenge_id" UUID NOT NULL REFERENCES "challenges" ("id") ON DELETE CASCADE,
    "hidden" BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY ("event_id", "challenge_id")
);

CREATE TABLE IF NOT EXISTS "event_instances" (
    "event_id" UUID NOT NULL REFERENCES "events" ("id") ON DELETE CASCADE,
    "instance_id" UUID NOT NULL REFERENCES "instances" ("id") ON DELETE CASCADE,
    PRIMARY KEY ("event_id", "instance_id")
);

CREATE TABLE IF NOT EXISTS "event_users" (
    "event_id" UUID NOT NULL REFERENCES "events" ("id") ON DELETE CASCADE,
    "user_id" UUID NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "joined_at" TIMESTAMP NOT NULL DEFAULT now(),
    PRIMARY KEY ("event_id", "user_id")
);

CREATE TABLE IF NOT EXISTS "event_teams" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    "event_id" UUID NOT NULL REFERENCES events ("id") ON DELETE CASCADE,
    "name" TEXT NOT NULL,
    "description" TEXT,
    "created_at" TIMESTAMP NOT NULL DEFAULT now(),
    "updated_at" TIMESTAMP NOT NULL DEFAULT now(),
    UNIQUE ("event_id", "name")
);

CREATE TYPE "event_team_member_role" AS ENUM ('captain', 'member');

CREATE TABLE IF NOT EXISTS "event_team_members" (
    "event_id" UUID NOT NULL REFERENCES "events" ("id") ON DELETE CASCADE,
    "team_id" UUID NOT NULL REFERENCES "event_teams" ("id") ON DELETE CASCADE,
    "user_id" UUID NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "role" "event_team_member_role" NOT NULL DEFAULT 'member',
    "joined_at" TIMESTAMP NOT NULL DEFAULT now(),
    PRIMARY KEY (
        "event_id",
        "team_id",
        "user_id"
    )
);

CREATE TABLE IF NOT EXISTS "challenge_solves" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    "event_id" UUID NULL REFERENCES "events" ("id") ON DELETE CASCADE,
    "challenge_id" UUID NOT NULL REFERENCES "challenges" ("id") ON DELETE CASCADE,
    "user_id" UUID NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "created_at" TIMESTAMP NOT NULL DEFAULT now()
);

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

-- for updated_at
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
   NEW."updated_at" = now();
   RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER "trg_users_updated_at"
BEFORE UPDATE ON "users"
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER "trg_super_admin_updated_at"
BEFORE UPDATE ON "super_admin"
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER "trg_challenges_updated_at"
BEFORE UPDATE ON "challenges"
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER "trg_instances_updated_at"
BEFORE UPDATE ON "instances"
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER "trg_events_updated_at"
BEFORE UPDATE ON "events"
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER "trg_event_teams_updated_at"
BEFORE UPDATE ON "event_teams"
FOR EACH ROW
EXECUTE FUNCTION update_updated_at_column();