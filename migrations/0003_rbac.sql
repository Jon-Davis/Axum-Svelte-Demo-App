-- Persistent user records; upserted on every OIDC login so email stays current.
-- Role defaults to 'user'. To bootstrap the first admin after their first login:
--   UPDATE users SET role = 'admin' WHERE email = 'admin@example.com';
-- The change takes effect on their next login.
CREATE TABLE users (
    sub        TEXT        PRIMARY KEY,
    email      TEXT        NOT NULL,
    role       TEXT        NOT NULL DEFAULT 'user',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Sessions carry the role at login time so handlers never need a users join.
ALTER TABLE sessions ADD COLUMN role TEXT NOT NULL DEFAULT 'user';

-- API keys carry their own role, set when the key is created.
ALTER TABLE api_keys ADD COLUMN role TEXT NOT NULL DEFAULT 'user';
