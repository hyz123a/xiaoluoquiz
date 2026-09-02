use std::{env, sync::Arc, time::SystemTime};

use sqlx::postgres::PgPoolOptions;
use xiaoluoquiz::{
    application::QuestionManagementService,
    domain::{
        AdminQuestionInput, CorrectAnswer, QuestionImportBatch, QuestionOption, QuestionStatus,
        QuestionType,
    },
    server::{PgQuestionStore, QuestionStore},
};

#[tokio::test]
#[ignore = "requires a migrated and seeded PostgreSQL database"]
async fn postgres_store_reads_published_questions_and_answers() {
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must point to a migrated demo database for this test");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("PostgreSQL should be reachable");
    let store = PgQuestionStore::new(pool);

    let questions = store
        .list_published(None)
        .await
        .expect("published questions should be readable");
    let single_choice = questions
        .iter()
        .find(|question| matches!(question.question_type, QuestionType::SingleChoice))
        .expect("seed data should include a single-choice question");

    assert!(questions.len() >= 4);
    assert_eq!(single_choice.options.len(), 2);

    let scored = store
        .get_for_scoring(single_choice.id)
        .await
        .expect("published answer should be readable")
        .expect("the published question should exist");
    assert!(matches!(
        scored.correct_answer,
        xiaoluoquiz::domain::CorrectAnswer::SingleChoice { .. }
    ));
}

#[tokio::test]
#[ignore = "requires a migrated and seeded PostgreSQL database"]
async fn postgres_store_imports_new_questions_as_published() {
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must point to a migrated demo database for this test");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("PostgreSQL should be reachable");
    let store = Arc::new(PgQuestionStore::new(pool.clone()));
    let management = QuestionManagementService::new(store.clone());
    let suffix = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after the Unix epoch")
        .as_nanos();
    let stem = format!("批量导入直接发布测试 {suffix}");

    let report = management
        .import_add_only(QuestionImportBatch {
            items: vec![AdminQuestionInput {
                question_bank_id: 1,
                question_type: QuestionType::SingleChoice,
                stem: stem.clone(),
                blank_count: 0,
                options: vec![
                    QuestionOption {
                        key: "A".to_owned(),
                        text: "正确选项".to_owned(),
                    },
                    QuestionOption {
                        key: "B".to_owned(),
                        text: "错误选项".to_owned(),
                    },
                ],
                explanation: None,
                correct_answer: CorrectAnswer::SingleChoice {
                    option_key: "A".to_owned(),
                },
            }],
        })
        .await
        .expect("question import should succeed");

    assert_eq!(report.inserted, 1);
    assert_eq!(report.skipped, 0);
    assert_eq!(report.errors, 0);
    let question_id = report.items[0]
        .question_id
        .expect("inserted question should have an id");
    let admin_question = store
        .get_admin(question_id)
        .await
        .expect("imported question should be readable")
        .expect("imported question should exist");
    assert_eq!(admin_question.stem, stem);
    assert_eq!(admin_question.status, QuestionStatus::Published);
    assert!(
        store
            .get_published(question_id)
            .await
            .expect("published question should be readable")
            .is_some()
    );

    sqlx::query("DELETE FROM questions WHERE id = $1")
        .bind(question_id)
        .execute(&pool)
        .await
        .expect("temporary imported question should be removable");
}

#[tokio::test]
#[ignore = "requires a migrated and seeded PostgreSQL database"]
async fn postgres_store_auto_submits_an_expired_attempt() {
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use xiaoluoquiz::{
        application::{ExamError, ExamService, PaperManagementService},
        domain::{
            AnswerPayload, AttemptStatus, CandidateInfo, CreatePaperInput, PaperMode,
            PaperQuestionInput, QuestionType, ResultVisibility,
        },
        server::PgQuestionStore,
    };

    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must point to a migrated demo database for this test");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await
        .expect("PostgreSQL should be reachable");
    let question_store = Arc::new(PgQuestionStore::new(pool.clone()));
    let paper_store = Arc::new(xiaoluoquiz::server::PgPaperStore::new(pool.clone()));
    let admin_id = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM users WHERE role = 'admin'::user_role ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .expect("seed data should include an admin user");
    let question = question_store
        .list_published(None)
        .await
        .expect("published questions should be readable")
        .into_iter()
        .find(|question| question.question_type == QuestionType::SingleChoice)
        .expect("seed data should include a single-choice question");
    let management = PaperManagementService::new(question_store, paper_store.clone());
    let unique_title = format!(
        "expired attempt test {}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos()
    );
    let paper = management
        .create_draft(
            admin_id,
            CreatePaperInput {
                title: unique_title,
                description: None,
                audience: None,
                mode: PaperMode::Exam,
                open_at: None,
                close_at: None,
                duration_seconds: Some(3_600),
                max_attempts: 1,
                allow_resume: true,
                auto_save: true,
                auto_submit: true,
                candidate_fields: Vec::new(),
                result_visibility: ResultVisibility::AfterSubmit,
                allow_preview: false,
                questions: vec![PaperQuestionInput {
                    question_id: question.id,
                    score: Some(2.0),
                }],
            },
        )
        .await
        .expect("paper draft should be created");
    let paper = management
        .publish(admin_id, paper.id)
        .await
        .expect("paper should be published");
    let exams = ExamService::new(paper_store);
    let attempt = exams
        .start(admin_id, paper.id, CandidateInfo::default())
        .await
        .expect("attempt should start");
    sqlx::query(
        "UPDATE attempts SET deadline_at = clock_timestamp() - interval '1 second' WHERE id = $1",
    )
    .bind(attempt.id)
    .execute(&pool)
    .await
    .expect("attempt deadline should be adjustable for the test");

    let reloaded = exams
        .get_attempt(admin_id, attempt.id)
        .await
        .expect("reading an expired attempt should finalize it");
    assert_eq!(reloaded.status, AttemptStatus::Graded);
    assert_eq!(reloaded.total_score, Some(0.0));
    let result = exams
        .result(admin_id, attempt.id)
        .await
        .expect("the finalized result should be available");
    assert_eq!(result.status, AttemptStatus::Graded);
    let save_after_deadline = exams
        .save_answer(
            admin_id,
            attempt.id,
            question.id,
            AnswerPayload::SingleChoice {
                option_key: "B".to_owned(),
            },
        )
        .await;
    assert!(matches!(save_after_deadline, Err(ExamError::AttemptClosed)));

    sqlx::query("DELETE FROM attempts WHERE id = $1")
        .bind(attempt.id)
        .execute(&pool)
        .await
        .expect("test attempt should be removed");
    sqlx::query("DELETE FROM papers WHERE id = $1")
        .bind(paper.id)
        .execute(&pool)
        .await
        .expect("test paper should be removed");
}
