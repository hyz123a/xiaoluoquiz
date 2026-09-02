BEGIN;

DO $$
DECLARE
    admin_id BIGINT;
    test_bank_id BIGINT;
    question_id BIGINT;
    revision_id BIGINT;
    existing_question_id BIGINT;
BEGIN
    SELECT id
    INTO admin_id
    FROM users
    WHERE display_name = '演示管理员' AND role = 'admin'
    ORDER BY id
    LIMIT 1;

    IF admin_id IS NULL THEN
        INSERT INTO users (username, display_name, role)
        VALUES ('demo-admin', '演示管理员', 'admin'::user_role)
        RETURNING id INTO admin_id;
    END IF;

    SELECT id
    INTO test_bank_id
    FROM question_banks
    WHERE name = '测试题库'
    LIMIT 1;

    IF test_bank_id IS NULL THEN
        RAISE EXCEPTION '测试题库不存在，请先运行数据库迁移';
    END IF;

    SELECT q.id
    INTO existing_question_id
    FROM questions AS q
    JOIN question_revisions AS r ON r.question_id = q.id
    WHERE r.stem = 'Rust 的包管理工具是什么？'
    ORDER BY q.id
    LIMIT 1;

    IF existing_question_id IS NULL THEN
        INSERT INTO questions (question_bank_id, status, created_by)
        VALUES (test_bank_id, 'draft'::question_status, admin_id)
        RETURNING id INTO question_id;

        INSERT INTO question_revisions (
            question_id,
            version,
            question_type,
            stem,
            explanation,
            blank_count,
            created_by
        )
        VALUES (
            question_id,
            1,
            'single_choice'::question_type,
            'Rust 的包管理工具是什么？',
            'Cargo 负责 Rust 项目的依赖管理、构建和发布。',
            0,
            admin_id
        )
        RETURNING id INTO revision_id;

        INSERT INTO question_options (revision_id, option_key, option_text, display_order)
        VALUES
            (revision_id, 'A', 'npm', 0),
            (revision_id, 'B', 'Cargo', 1);

        INSERT INTO question_answers (revision_id, answer_payload)
        VALUES (
            revision_id,
            '{"type":"single_choice","option_key":"B"}'::jsonb
        );

        UPDATE questions
        SET status = 'published'::question_status,
            published_revision_id = revision_id,
            published_by = admin_id,
            published_at = now()
        WHERE id = question_id;
    END IF;

    SELECT q.id
    INTO existing_question_id
    FROM questions AS q
    JOIN question_revisions AS r ON r.question_id = q.id
    WHERE r.stem = 'Rust 的异步运行时通常使用 ___。'
    ORDER BY q.id
    LIMIT 1;

    IF existing_question_id IS NULL THEN
        INSERT INTO questions (question_bank_id, status, created_by)
        VALUES (test_bank_id, 'draft'::question_status, admin_id)
        RETURNING id INTO question_id;

        INSERT INTO question_revisions (
            question_id,
            version,
            question_type,
            stem,
            explanation,
            blank_count,
            created_by
        )
        VALUES (
            question_id,
            1,
            'fill_blank'::question_type,
            'Rust 的异步运行时通常使用 ___。',
            'Tokio 是 Rust 生态中常用的异步运行时。',
            1,
            admin_id
        )
        RETURNING id INTO revision_id;

        INSERT INTO question_answers (revision_id, answer_payload)
        VALUES (
            revision_id,
            '{"type":"fill_blank","accepted":[["tokio","Tokio"]]}'::jsonb
        );

        UPDATE questions
        SET status = 'published'::question_status,
            published_revision_id = revision_id,
            published_by = admin_id,
            published_at = now()
        WHERE id = question_id;
    END IF;

    SELECT q.id
    INTO existing_question_id
    FROM questions AS q
    JOIN question_revisions AS r ON r.question_id = q.id
    WHERE r.stem = 'PostgreSQL 支持 JSONB 类型。'
    ORDER BY q.id
    LIMIT 1;

    IF existing_question_id IS NULL THEN
        INSERT INTO questions (question_bank_id, status, created_by)
        VALUES (test_bank_id, 'draft'::question_status, admin_id)
        RETURNING id INTO question_id;

        INSERT INTO question_revisions (
            question_id,
            version,
            question_type,
            stem,
            explanation,
            blank_count,
            created_by
        )
        VALUES (
            question_id,
            1,
            'true_false'::question_type,
            'PostgreSQL 支持 JSONB 类型。',
            'JSONB 是 PostgreSQL 提供的二进制 JSON 数据类型，支持索引和结构化查询。',
            0,
            admin_id
        )
        RETURNING id INTO revision_id;

        INSERT INTO question_answers (revision_id, answer_payload)
        VALUES (
            revision_id,
            '{"type":"true_false","value":true}'::jsonb
        );

        UPDATE questions
        SET status = 'published'::question_status,
            published_revision_id = revision_id,
            published_by = admin_id,
            published_at = now()
        WHERE id = question_id;
    END IF;

    SELECT q.id
    INTO existing_question_id
    FROM questions AS q
    JOIN question_revisions AS r ON r.question_id = q.id
    WHERE r.stem = '请用一句话说明 SQLx 的作用。'
    ORDER BY q.id
    LIMIT 1;

    IF existing_question_id IS NULL THEN
        INSERT INTO questions (question_bank_id, status, created_by)
        VALUES (test_bank_id, 'draft'::question_status, admin_id)
        RETURNING id INTO question_id;

        INSERT INTO question_revisions (
            question_id,
            version,
            question_type,
            stem,
            explanation,
            blank_count,
            created_by
        )
        VALUES (
            question_id,
            1,
            'short_answer'::question_type,
            '请用一句话说明 SQLx 的作用。',
            'SQLx 为 Rust 提供异步、编译期可检查的数据库访问能力。',
            0,
            admin_id
        )
        RETURNING id INTO revision_id;

        INSERT INTO question_answers (revision_id, answer_payload)
        VALUES (
            revision_id,
            '{"type":"short_answer","reference":"SQLx 为 Rust 提供异步数据库访问能力。","rubric":"说明 Rust 应用如何访问关系型数据库即可。"}'::jsonb
        );

        UPDATE questions
        SET status = 'published'::question_status,
            published_revision_id = revision_id,
            published_by = admin_id,
            published_at = now()
        WHERE id = question_id;
    END IF;

    SELECT q.id
    INTO existing_question_id
    FROM questions AS q
    JOIN question_revisions AS r ON r.question_id = q.id
    WHERE r.stem = '以下哪些是 Rust 开发工具？'
    ORDER BY q.id
    LIMIT 1;

    IF existing_question_id IS NULL THEN
        INSERT INTO questions (question_bank_id, status, created_by)
        VALUES (test_bank_id, 'draft'::question_status, admin_id)
        RETURNING id INTO question_id;

        INSERT INTO question_revisions (
            question_id,
            version,
            question_type,
            stem,
            explanation,
            blank_count,
            created_by
        )
        VALUES (
            question_id,
            1,
            'multiple_choice'::question_type,
            '以下哪些是 Rust 开发工具？',
            'Cargo、rustc 和 Clippy 是 Rust 开发工具。',
            0,
            admin_id
        )
        RETURNING id INTO revision_id;

        INSERT INTO question_options (revision_id, option_key, option_text, display_order)
        VALUES
            (revision_id, 'A', 'Cargo', 0),
            (revision_id, 'B', 'npm', 1),
            (revision_id, 'C', 'rustc', 2),
            (revision_id, 'D', 'pip', 3),
            (revision_id, 'E', 'Clippy', 4);

        INSERT INTO question_answers (revision_id, answer_payload)
        VALUES (
            revision_id,
            '{"type":"multiple_choice","option_keys":["A","C","E"]}'::jsonb
        );

        UPDATE questions
        SET status = 'published'::question_status,
            published_revision_id = revision_id,
            published_by = admin_id,
            published_at = now()
        WHERE id = question_id;
    END IF;
END;
$$;

COMMIT;
