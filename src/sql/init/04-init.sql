INSERT INTO
    "super_admin" (
        "id",
        "username",
        "password_hash",
        "email",
        "created_at",
        "updated_at"
    )
VALUES (
        'e9c27136-f30c-4619-8377-756b1148192d',
        'sysadmin',
        '$argon2id$v=19$m=19456,t=2,p=1$3THt36/y60+8SreEtA+T5A$xp4mvnbi0niUfEux7u24ZdTnv4t5QnH8ZhA/uF+GDe8',
        'sysadmin@system.com',
        '2025-09-29 13:04:49.689893',
        '2025-09-29 13:04:49.689893'
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