CREATE EXTENSION "uuid-ossp";

CREATE TYPE "setting_value_type" AS ENUM ('string', 'integer', 'boolean','float');

CREATE TABLE IF NOT EXISTS "settings" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    "key" TEXT NOT NULL UNIQUE,
    "value" TEXT NOT NULL,
    "type" "setting_value_type" NOT NULL DEFAULT 'string',
    "description" TEXT NOT NULL,
    "protected" BOOLEAN NOT NULL DEFAULT TRUE,
    "updated_at" TIMESTAMP NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS "users" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    "username" TEXT NOT NULL UNIQUE,
    "nickname" TEXT NOT NULL UNIQUE,
    "password" TEXT NOT NULL,
    "email" TEXT NOT NULL,
    "created_at" TIMESTAMP NOT NULL DEFAULT now(),
    "updated_at" TIMESTAMP NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS "super_admin" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    "username" TEXT NOT NULL UNIQUE,
    "password" TEXT NOT NULL,
    "email" TEXT NOT NULL,
    "created_at" TIMESTAMP NOT NULL DEFAULT now(),
    "updated_at" TIMESTAMP NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS "challenges" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    "name" TEXT NOT NULL UNIQUE,
    -- ALTER TABLE challenges ADD COLUMN safe_name TEXT; 允许'
    "safe_name" TEXT NOT NULL UNIQUE,
    "category" TEXT NOT NULL DEFAULT 'other',
    "description" TEXT NOT NULL DEFAULT 'no description',
    "attachment" TEXT NULL,
    "hidden" BOOLEAN NOT NULL DEFAULT TRUE,
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
    "identifier" TEXT NOT NULL,
    "created_at" TIMESTAMP NOT NULL DEFAULT now(),
    "updated_at" TIMESTAMP NOT NULL DEFAULT now(),
    "destroy_at" TIMESTAMP NOT NULL
);

CREATE TYPE "event_type" AS ENUM ('jeopardy_single', 'jeopardy_team', 'awd_team');

CREATE TABLE IF NOT EXISTS "events" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    "type" "event_type" NOT NULL DEFAULT 'jeopardy_single',
    "title" TEXT NOT NULL,
    "description" TEXT,
    "hidden" BOOLEAN NOT NULL DEFAULT TRUE,
    "start_time" TIMESTAMP NOT NULL,
    "rules" TEXT NOT NULL DEFAULT 'do not cheat',
    "allow_join" BOOLEAN NOT NULL DEFAULT FALSE,
    "flag_prefix" TEXT NULL DEFAULT 'flag',
    "end_time" TIMESTAMP NOT NULL,
    "created_at" TIMESTAMP NOT NULL DEFAULT now(),
    "updated_at" TIMESTAMP NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS "event_challenges" (
    "event_id" UUID NOT NULL REFERENCES "events" ("id") ON DELETE CASCADE,
    "challenge_id" UUID NOT NULL REFERENCES "challenges" ("id") ON DELETE CASCADE,
    "points" DOUBLE PRECISION NOT NULL DEFAULT 100,
    "hidden" BOOLEAN NOT NULL DEFAULT TRUE,
    PRIMARY KEY ("event_id", "challenge_id")
);

CREATE TABLE IF NOT EXISTS "event_users" (
    "event_id" UUID NOT NULL REFERENCES "events" ("id") ON DELETE CASCADE,
    "user_id" UUID NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "points" DOUBLE PRECISION NOT NULL DEFAULT 0,
    "banned" BOOLEAN NOT NULL DEFAULT false,
    "joined_at" TIMESTAMP NOT NULL DEFAULT now(),
    PRIMARY KEY ("event_id", "user_id")
);

CREATE TABLE IF NOT EXISTS "event_teams" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    "event_id" UUID NOT NULL REFERENCES events ("id") ON DELETE CASCADE,
    "name" TEXT NOT NULL,
    "description" TEXT,
    "points" DOUBLE PRECISION NOT NULL DEFAULT 0,
    "created_at" TIMESTAMP NOT NULL DEFAULT now(),
    "updated_at" TIMESTAMP NOT NULL DEFAULT now(),
    "banned" BOOLEAN NOT NULL DEFAULT false,
    UNIQUE ("event_id", "name")
);

CREATE TABLE IF NOT EXISTS "event_instances" (
    "event_id" UUID NOT NULL REFERENCES "events" ("id") ON DELETE CASCADE,
    "instance_id" UUID NOT NULL REFERENCES "instances" ("id") ON DELETE CASCADE,
    "challenge_id" UUID NOT NULL REFERENCES "challenges" ("id") ON DELETE CASCADE,
    "user_id" UUID NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "team_id" UUID NULL REFERENCES "event_teams" ("id") ON DELETE CASCADE,
    PRIMARY KEY ("event_id", "instance_id")
);

CREATE TABLE IF NOT EXISTS "event_challenge_solves" (
    "event_id" UUID NOT NULL REFERENCES "events" ("id") ON DELETE CASCADE,
    "challenge_id" UUID NOT NULL REFERENCES "challenges" ("id") ON DELETE CASCADE,
    "user_id" UUID NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "team_id" UUID NULL REFERENCES "event_teams" ("id") ON DELETE CASCADE,
    "obtained_points" DOUBLE PRECISION NOT NULL DEFAULT 0,
    "bonus_points" DOUBLE PRECISION NOT NULL DEFAULT 0,
    "created_at" TIMESTAMP NOT NULL DEFAULT now(),
    PRIMARY KEY (
        "event_id",
        "challenge_id",
        "user_id"
    )
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

CREATE TABLE IF NOT EXISTS "challenge_writeup" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    "challenge_id" UUID NOT NULL REFERENCES "challenges" ("id") ON DELETE CASCADE,
    "user_id" UUID NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "content" TEXT NOT NULL,
    "created_at" TIMESTAMP NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS "event_announcements" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    "event_id" UUID NOT NULL REFERENCES "events" ("id") ON DELETE CASCADE,
    "title" TEXT NOT NULL,
    "content" TEXT NOT NULL,
    "created_at" TIMESTAMP NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS "event_writeup" (
    "event_id" UUID NOT NULL REFERENCES "events" ("id") ON DELETE CASCADE,
    "user_id" UUID NOT NULL REFERENCES "users" ("id") ON DELETE CASCADE,
    "team_id" UUID NULL REFERENCES "event_teams" ("id") ON DELETE CASCADE,
    "file_url" TEXT NOT NULL,
    "created_at" TIMESTAMP NOT NULL DEFAULT now(),
    CONSTRAINT event_writeup_pkey PRIMARY KEY ("event_id", "user_id")
);

CREATE TABLE IF NOT EXISTS "challenge_sets" (
    "id" UUID PRIMARY KEY DEFAULT uuid_generate_v4 (),
    "name" TEXT NOT NULL,
    "description" TEXT,
    "created_at" TIMESTAMP NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS "challenge_set_items" (
    "set_id" UUID NOT NULL REFERENCES "challenge_sets" (id) ON DELETE CASCADE,
    "challenge_id" UUID NOT NULL REFERENCES "challenges" (id) ON DELETE CASCADE,
    PRIMARY KEY ("set_id", "challenge_id")
);
-- user_logs,training_logs, event_logs, system_logs, admin_logs