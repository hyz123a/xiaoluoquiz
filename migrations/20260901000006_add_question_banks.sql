CREATE TABLE question_banks (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name TEXT NOT NULL CHECK (btrim(name) <> ''),
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (name)
);

INSERT INTO question_banks (name, description)
VALUES
    ('人工智能导论', '人工智能导论课程题目'),
    ('测试题库', '用于本地演示和系统测试的题目');

ALTER TABLE questions
    ADD COLUMN question_bank_id BIGINT;

UPDATE questions
SET question_bank_id = (
    SELECT id FROM question_banks WHERE name = '测试题库'
)
WHERE question_bank_id IS NULL;

ALTER TABLE questions
    ALTER COLUMN question_bank_id SET NOT NULL,
    ADD CONSTRAINT questions_question_bank_fk
        FOREIGN KEY (question_bank_id) REFERENCES question_banks (id) ON DELETE RESTRICT;

CREATE INDEX questions_question_bank_idx
    ON questions (question_bank_id, status);

CREATE TRIGGER question_banks_set_updated_at
BEFORE UPDATE ON question_banks
FOR EACH ROW EXECUTE FUNCTION set_updated_at();

ALTER TABLE question_revisions
    DROP COLUMN score;
