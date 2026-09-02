CREATE TABLE classes (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name TEXT NOT NULL CHECK (btrim(name) <> ''),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (name)
);

INSERT INTO classes (name)
SELECT DISTINCT btrim(class_name)
FROM users
WHERE class_name IS NOT NULL AND btrim(class_name) <> ''
ON CONFLICT (name) DO NOTHING;

ALTER TABLE users
    ADD COLUMN class_id BIGINT REFERENCES classes (id) ON DELETE SET NULL;

UPDATE users AS users
SET class_id = classes.id
FROM classes
WHERE users.class_id IS NULL
  AND users.class_name IS NOT NULL
  AND btrim(users.class_name) = classes.name;

CREATE INDEX users_class_id_idx ON users (class_id);
CREATE INDEX classes_name_idx ON classes (name);
