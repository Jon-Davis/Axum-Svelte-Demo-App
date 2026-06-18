CREATE TABLE api_keys (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    name         TEXT        NOT NULL,
    key_hash     TEXT        NOT NULL UNIQUE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at   TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ
);

-- To create a key (run from psql or any SQL client):
--   SELECT encode(gen_random_bytes(32), 'hex') AS raw_token;
--   INSERT INTO api_keys (name, key_hash)
--     VALUES ('my-service', encode(sha256('svrt_<raw_token_here>'::bytea), 'hex'));
-- The raw token is what you give to the service (prefix it with 'svrt_').
-- The hash is what is stored — the plaintext token is never persisted.
