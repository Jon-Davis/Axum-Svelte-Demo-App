CREATE TABLE sessions (
    id          TEXT PRIMARY KEY,
    user_sub    TEXT NOT NULL,
    email       TEXT NOT NULL,
    username    TEXT,
    expires_at  TIMESTAMPTZ NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX sessions_expires_at_idx ON sessions (expires_at);
