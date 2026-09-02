CREATE TYPE user_role AS ENUM ('admin', 'user');
CREATE TYPE question_type AS ENUM (
    'single_choice',
    'fill_blank',
    'true_false',
    'short_answer'
);
CREATE TYPE question_status AS ENUM ('draft', 'published', 'archived');
CREATE TYPE attempt_status AS ENUM (
    'in_progress',
    'submitted',
    'needs_review',
    'graded'
);
CREATE TYPE grading_status AS ENUM ('pending', 'needs_review', 'graded');

CREATE TABLE users (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    display_name TEXT NOT NULL CHECK (btrim(display_name) <> ''),
    role user_role NOT NULL DEFAULT 'user',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE questions (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    status question_status NOT NULL DEFAULT 'draft',
    published_revision_id BIGINT,
    created_by BIGINT REFERENCES users (id) ON DELETE SET NULL,
    published_by BIGINT REFERENCES users (id) ON DELETE SET NULL,
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (status <> 'published' OR published_revision_id IS NOT NULL)
);

CREATE TABLE question_revisions (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    question_id BIGINT NOT NULL REFERENCES questions (id) ON DELETE CASCADE,
    version INTEGER NOT NULL CHECK (version > 0),
    question_type question_type NOT NULL,
    stem TEXT NOT NULL CHECK (btrim(stem) <> ''),
    explanation TEXT,
    score NUMERIC(8, 2) NOT NULL DEFAULT 1 CHECK (score >= 0),
    created_by BIGINT REFERENCES users (id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (question_id, version),
    UNIQUE (id, question_id)
);

ALTER TABLE questions
    ADD CONSTRAINT questions_published_revision_fk
    FOREIGN KEY (published_revision_id, id)
    REFERENCES question_revisions (id, question_id)
    DEFERRABLE INITIALLY DEFERRED;

CREATE TABLE question_options (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    revision_id BIGINT NOT NULL REFERENCES question_revisions (id) ON DELETE CASCADE,
    option_key TEXT NOT NULL CHECK (btrim(option_key) <> ''),
    option_text TEXT NOT NULL CHECK (btrim(option_text) <> ''),
    display_order INTEGER NOT NULL CHECK (display_order >= 0),
    UNIQUE (revision_id, option_key),
    UNIQUE (revision_id, display_order)
);

CREATE TABLE question_answers (
    revision_id BIGINT PRIMARY KEY REFERENCES question_revisions (id) ON DELETE CASCADE,
    answer_payload JSONB NOT NULL CHECK (jsonb_typeof(answer_payload) = 'object'),
    scoring_config JSONB NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(scoring_config) = 'object'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE attempts (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE RESTRICT,
    status attempt_status NOT NULL DEFAULT 'in_progress',
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    submitted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (status = 'in_progress' OR submitted_at IS NOT NULL)
);

CREATE TABLE attempt_answers (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    attempt_id BIGINT NOT NULL REFERENCES attempts (id) ON DELETE CASCADE,
    question_id BIGINT NOT NULL REFERENCES questions (id) ON DELETE RESTRICT,
    revision_id BIGINT NOT NULL REFERENCES question_revisions (id) ON DELETE RESTRICT,
    answer_payload JSONB NOT NULL,
    grading_status grading_status NOT NULL DEFAULT 'pending',
    score NUMERIC(8, 2) CHECK (score IS NULL OR score >= 0),
    reviewed_by BIGINT REFERENCES users (id) ON DELETE SET NULL,
    feedback TEXT,
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    graded_at TIMESTAMPTZ,
    UNIQUE (attempt_id, question_id)
);

CREATE TABLE audit_logs (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    actor_user_id BIGINT REFERENCES users (id) ON DELETE SET NULL,
    action TEXT NOT NULL CHECK (btrim(action) <> ''),
    entity_type TEXT NOT NULL CHECK (btrim(entity_type) <> ''),
    entity_id BIGINT,
    details JSONB NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(details) = 'object'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX questions_status_idx ON questions (status);
CREATE INDEX question_revisions_question_idx ON question_revisions (question_id, version DESC);
CREATE INDEX attempts_user_idx ON attempts (user_id, created_at DESC);
CREATE INDEX attempts_status_idx ON attempts (status);
CREATE INDEX attempt_answers_question_idx ON attempt_answers (question_id, submitted_at DESC);
CREATE INDEX attempt_answers_review_idx
    ON attempt_answers (grading_status, submitted_at DESC)
    WHERE grading_status <> 'graded';
CREATE INDEX audit_logs_entity_idx ON audit_logs (entity_type, entity_id, created_at DESC);

CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$;

CREATE TRIGGER users_set_updated_at
BEFORE UPDATE ON users
FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER questions_set_updated_at
BEFORE UPDATE ON questions
FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER question_answers_set_updated_at
BEFORE UPDATE ON question_answers
FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TRIGGER attempts_set_updated_at
BEFORE UPDATE ON attempts
FOR EACH ROW EXECUTE FUNCTION set_updated_at();
