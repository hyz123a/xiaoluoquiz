CREATE TYPE account_status AS ENUM ('active', 'disabled');

ALTER TYPE user_role ADD VALUE IF NOT EXISTS 'student';

ALTER TABLE users
    ADD COLUMN username TEXT;

UPDATE users
SET username = 'legacy-' || id::text
WHERE username IS NULL;

ALTER TABLE users
    ALTER COLUMN username SET NOT NULL;

CREATE UNIQUE INDEX users_username_idx ON users (username);

ALTER TABLE users
    ADD COLUMN password_hash TEXT,
    ADD COLUMN must_change_password BOOLEAN NOT NULL DEFAULT true,
    ADD COLUMN status account_status NOT NULL DEFAULT 'active',
    ADD COLUMN student_number TEXT,
    ADD COLUMN class_name TEXT,
    ADD COLUMN last_login_at TIMESTAMPTZ;

CREATE TABLE user_sessions (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX user_sessions_user_idx ON user_sessions (user_id, expires_at DESC);
CREATE INDEX user_sessions_active_idx
    ON user_sessions (token_hash, expires_at)
    WHERE revoked_at IS NULL;
