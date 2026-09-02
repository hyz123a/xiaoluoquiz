CREATE TYPE paper_mode AS ENUM ('exam', 'practice');
CREATE TYPE paper_status AS ENUM ('draft', 'published', 'archived');
CREATE TYPE result_visibility AS ENUM ('after_submit', 'after_grading', 'admin_release');

CREATE TABLE papers (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    title TEXT NOT NULL CHECK (btrim(title) <> ''),
    description TEXT,
    audience TEXT,
    mode paper_mode NOT NULL DEFAULT 'exam',
    status paper_status NOT NULL DEFAULT 'draft',
    open_at TIMESTAMPTZ,
    close_at TIMESTAMPTZ,
    duration_seconds INTEGER CHECK (duration_seconds IS NULL OR duration_seconds > 0),
    max_attempts INTEGER NOT NULL DEFAULT 1 CHECK (max_attempts > 0),
    allow_resume BOOLEAN NOT NULL DEFAULT true,
    auto_save BOOLEAN NOT NULL DEFAULT true,
    auto_submit BOOLEAN NOT NULL DEFAULT true,
    candidate_fields JSONB NOT NULL DEFAULT '[]'::jsonb
        CHECK (jsonb_typeof(candidate_fields) = 'array'),
    result_visibility result_visibility NOT NULL DEFAULT 'after_submit',
    allow_preview BOOLEAN NOT NULL DEFAULT false,
    created_by BIGINT REFERENCES users (id) ON DELETE SET NULL,
    published_by BIGINT REFERENCES users (id) ON DELETE SET NULL,
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (close_at IS NULL OR open_at IS NULL OR close_at > open_at)
);

CREATE TABLE paper_questions (
    paper_id BIGINT NOT NULL REFERENCES papers (id) ON DELETE CASCADE,
    question_id BIGINT NOT NULL REFERENCES questions (id) ON DELETE RESTRICT,
    revision_id BIGINT NOT NULL,
    display_order INTEGER NOT NULL CHECK (display_order >= 0),
    score NUMERIC(8, 2) NOT NULL CHECK (score > 0),
    PRIMARY KEY (paper_id, question_id),
    UNIQUE (paper_id, display_order),
    CONSTRAINT paper_questions_revision_fk
        FOREIGN KEY (revision_id, question_id)
        REFERENCES question_revisions (id, question_id)
        ON DELETE RESTRICT
);

ALTER TABLE attempts
    ADD COLUMN paper_id BIGINT REFERENCES papers (id) ON DELETE RESTRICT,
    ADD COLUMN deadline_at TIMESTAMPTZ,
    ADD COLUMN candidate_info JSONB NOT NULL DEFAULT '{}'::jsonb
        CHECK (jsonb_typeof(candidate_info) = 'object'),
    ADD COLUMN max_score NUMERIC(8, 2) NOT NULL DEFAULT 0 CHECK (max_score >= 0),
    ADD COLUMN score NUMERIC(8, 2) CHECK (score IS NULL OR score >= 0);

CREATE INDEX papers_status_idx ON papers (status);
CREATE INDEX papers_window_idx ON papers (status, open_at, close_at);
CREATE INDEX paper_questions_paper_order_idx ON paper_questions (paper_id, display_order);
CREATE INDEX attempts_user_paper_idx ON attempts (user_id, paper_id, created_at DESC);
CREATE INDEX attempts_paper_status_idx ON attempts (paper_id, status, user_id);

CREATE TRIGGER papers_set_updated_at
BEFORE UPDATE ON papers
FOR EACH ROW EXECUTE FUNCTION set_updated_at();
