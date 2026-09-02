ALTER TABLE question_revisions
ADD COLUMN blank_count SMALLINT NOT NULL DEFAULT 0
CHECK (blank_count >= 0);
