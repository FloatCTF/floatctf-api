ALTER TABLE users ADD COLUMN avatar TEXT DEFAULT NULL;
CREATE TABLE IF NOT EXISTS "kv_store" (
    "key"         TEXT PRIMARY KEY,
    "value"       JSONB NOT NULL,
    "expires_at"  TIMESTAMPTZ,
    "created_at"  TIMESTAMPTZ NOT NULL DEFAULT now(),
    "updated_at"  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS "idx_kv_store_expires_at"
    ON "kv_store" ("expires_at") WHERE "expires_at" IS NOT NULL;
