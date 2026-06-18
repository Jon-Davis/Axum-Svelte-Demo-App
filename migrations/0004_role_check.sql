-- Backstop the application-level role allowlist with a DB constraint, so no code
-- path (or manual SQL) can ever persist a role the app doesn't recognise.
ALTER TABLE users    ADD CONSTRAINT users_role_check    CHECK (role IN ('user', 'admin'));
ALTER TABLE sessions ADD CONSTRAINT sessions_role_check CHECK (role IN ('user', 'admin'));
ALTER TABLE api_keys ADD CONSTRAINT api_keys_role_check CHECK (role IN ('user', 'admin'));
