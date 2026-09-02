CREATE TABLE practice_records (
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    question_id BIGINT NOT NULL REFERENCES questions (id) ON DELETE RESTRICT,
    revision_id BIGINT NOT NULL,
    answer_payload JSONB NOT NULL
        CHECK (jsonb_typeof(answer_payload) = 'object'),
    is_correct BOOLEAN NOT NULL,
    first_answered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, question_id),
    CONSTRAINT practice_records_revision_fk
        FOREIGN KEY (revision_id, question_id)
        REFERENCES question_revisions (id, question_id)
        ON DELETE RESTRICT
);

CREATE INDEX practice_records_user_idx
    ON practice_records (user_id, first_answered_at DESC);
