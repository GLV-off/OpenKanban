-- Baseline identity tables for iteration 0.
-- Timestamps are RFC 3339 UTC strings; identifiers are UUIDv7 strings (see doc/domain_ru.md).

CREATE TABLE users (
    id            TEXT    PRIMARY KEY NOT NULL,
    email         TEXT    NOT NULL COLLATE NOCASE,
    password_hash TEXT    NOT NULL,
    display_name  TEXT    NOT NULL,
    email_verified INTEGER NOT NULL DEFAULT 0 CHECK (email_verified IN (0, 1)),
    status        TEXT    NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
    created_at    TEXT    NOT NULL,
    updated_at    TEXT    NOT NULL
);

CREATE UNIQUE INDEX users_email_unique ON users (email);

CREATE TABLE sessions (
    id          TEXT PRIMARY KEY NOT NULL,
    user_id     TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash  TEXT NOT NULL UNIQUE,
    issued_at   TEXT NOT NULL,
    expires_at  TEXT NOT NULL,
    revoked_at  TEXT,
    replaced_by TEXT REFERENCES sessions (id),
    user_agent  TEXT
);

CREATE INDEX sessions_user_id_idx ON sessions (user_id);
