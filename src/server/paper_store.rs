use async_trait::async_trait;
use sqlx::{FromRow, PgPool, Postgres, Transaction};

use crate::application::{
    GradedAnswer, PaperQuestionSnapshot, PaperStore, PaperStoreError, StoredAttempt,
    StoredAttemptQuestion,
};
use crate::domain::{
    AdminPaper, AnswerPayload, CandidateInfo, CreatePaperInput, GradingStatus, PaperQuestion,
    PaperRuntimeStatus, PublishedPaper, QuestionOption,
};

#[derive(Clone)]
pub struct PgPaperStore {
    pool: PgPool,
}

impl PgPaperStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(Debug, FromRow)]
struct AdminPaperRow {
    id: i64,
    status: String,
    title: String,
    description: Option<String>,
    audience: Option<String>,
    mode: String,
    open_at: Option<String>,
    close_at: Option<String>,
    duration_seconds: Option<i32>,
    max_attempts: i32,
    allow_resume: bool,
    auto_save: bool,
    auto_submit: bool,
    candidate_fields: serde_json::Value,
    result_visibility: String,
    allow_preview: bool,
    question_id: Option<i64>,
    revision_id: Option<i64>,
    question_bank_id: Option<i64>,
    question_bank_name: Option<String>,
    position: Option<i32>,
    paper_score: Option<f64>,
    question_type: Option<String>,
    stem: Option<String>,
    blank_count: Option<i16>,
    option_key: Option<String>,
    option_text: Option<String>,
}

#[derive(Debug, FromRow)]
struct PublishedPaperRow {
    id: i64,
    title: String,
    description: Option<String>,
    audience: Option<String>,
    mode: String,
    open_at: Option<String>,
    close_at: Option<String>,
    duration_seconds: Option<i32>,
    max_attempts: i32,
    allow_resume: bool,
    auto_save: bool,
    auto_submit: bool,
    candidate_fields: serde_json::Value,
    result_visibility: String,
    allow_preview: bool,
    question_count: i64,
    total_score: f64,
    current_attempt_id: Option<i64>,
    current_attempt_status: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct AttemptRow {
    id: i64,
    user_id: i64,
    paper_id: i64,
    title: String,
    status: String,
    started_at: String,
    deadline_at: Option<String>,
    auto_submit: bool,
    deadline_reached: bool,
    submitted_at: Option<String>,
    candidate_info: serde_json::Value,
    max_score: f64,
    total_score: Option<f64>,
    result_visibility: String,
    question_id: i64,
    revision_id: i64,
    position: i32,
    paper_score: f64,
    question_type: String,
    stem: String,
    blank_count: i16,
    explanation: Option<String>,
    option_key: Option<String>,
    option_text: Option<String>,
    correct_answer_payload: serde_json::Value,
    submitted_answer: Option<serde_json::Value>,
    grading_status: String,
    awarded_score: Option<f64>,
    feedback: Option<String>,
}

const ADMIN_PAPER_SELECT_SQL: &str = r#"
SELECT
    p.id,
    p.status::text AS status,
    p.title,
    p.description,
    p.audience,
    p.mode::text AS mode,
    to_char(p.open_at AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM-DD"T"HH24:MI:SS.MS"+08:00"') AS open_at,
    to_char(p.close_at AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM-DD"T"HH24:MI:SS.MS"+08:00"') AS close_at,
    p.duration_seconds,
    p.max_attempts,
    p.allow_resume,
    p.auto_save,
    p.auto_submit,
    p.candidate_fields,
    p.result_visibility::text AS result_visibility,
    p.allow_preview,
    pq.question_id,
    pq.revision_id,
    q.question_bank_id,
    b.name AS question_bank_name,
    pq.display_order AS position,
    pq.score::double precision AS paper_score,
    r.question_type::text AS question_type,
    r.stem,
    r.blank_count,
    o.option_key,
    o.option_text
FROM papers AS p
LEFT JOIN paper_questions AS pq ON pq.paper_id = p.id
LEFT JOIN questions AS q ON q.id = pq.question_id
LEFT JOIN question_banks AS b ON b.id = q.question_bank_id
LEFT JOIN question_revisions AS r
    ON r.id = pq.revision_id AND r.question_id = pq.question_id
LEFT JOIN question_options AS o ON o.revision_id = r.id
"#;

const PUBLISHED_PAPER_SELECT_SQL: &str = r#"
SELECT
    p.id,
    p.title,
    p.description,
    p.audience,
    p.mode::text AS mode,
    to_char(p.open_at AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM-DD"T"HH24:MI:SS.MS"+08:00"') AS open_at,
    to_char(p.close_at AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM-DD"T"HH24:MI:SS.MS"+08:00"') AS close_at,
    p.duration_seconds,
    p.max_attempts,
    p.allow_resume,
    p.auto_save,
    p.auto_submit,
    p.candidate_fields,
    p.result_visibility::text AS result_visibility,
    p.allow_preview,
    count(pq.question_id)::bigint AS question_count,
    COALESCE(sum(pq.score), 0)::double precision AS total_score,
    current_attempt.id AS current_attempt_id,
    current_attempt.status::text AS current_attempt_status
FROM papers AS p
LEFT JOIN paper_questions AS pq ON pq.paper_id = p.id
LEFT JOIN LATERAL (
    SELECT a.id, a.status
    FROM attempts AS a
    WHERE a.paper_id = p.id
      AND a.user_id = $1
      AND a.status = 'in_progress'::attempt_status
    ORDER BY a.id DESC
    LIMIT 1
) AS current_attempt ON true
"#;

const ATTEMPT_SELECT_SQL: &str = r#"
SELECT
    a.id,
    a.user_id,
    a.paper_id,
    p.title,
    a.status::text AS status,
    to_char(a.started_at AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM-DD"T"HH24:MI:SS.MS"+08:00"') AS started_at,
    to_char(a.deadline_at AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM-DD"T"HH24:MI:SS.MS"+08:00"') AS deadline_at,
    p.auto_submit,
    a.deadline_at IS NOT NULL AND a.deadline_at <= clock_timestamp() AS deadline_reached,
    to_char(a.submitted_at AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM-DD"T"HH24:MI:SS.MS"+08:00"') AS submitted_at,
    a.candidate_info,
    a.max_score::double precision AS max_score,
    a.score::double precision AS total_score,
    p.result_visibility::text AS result_visibility,
    pq.question_id,
    pq.revision_id,
    pq.display_order AS position,
    pq.score::double precision AS paper_score,
    r.question_type::text AS question_type,
    r.stem,
    r.blank_count,
    r.explanation,
    o.option_key,
    o.option_text,
    ca.answer_payload AS correct_answer_payload,
    aa.answer_payload AS submitted_answer,
    COALESCE(aa.grading_status::text, 'pending') AS grading_status,
    aa.score::double precision AS awarded_score,
    aa.feedback
FROM attempts AS a
JOIN papers AS p ON p.id = a.paper_id
JOIN paper_questions AS pq ON pq.paper_id = a.paper_id
JOIN question_revisions AS r
    ON r.id = pq.revision_id AND r.question_id = pq.question_id
JOIN question_answers AS ca ON ca.revision_id = pq.revision_id
LEFT JOIN question_options AS o ON o.revision_id = r.id
LEFT JOIN attempt_answers AS aa
    ON aa.attempt_id = a.id AND aa.question_id = pq.question_id
"#;

const ATTEMPT_ORDER_SQL: &str = " ORDER BY a.id, pq.display_order, o.display_order NULLS LAST";

const ADMIN_ATTEMPT_ORDER_SQL: &str =
    " ORDER BY a.id DESC, pq.display_order, o.display_order NULLS LAST";

fn parse_enum<T>(value: &str, label: &str) -> Result<T, PaperStoreError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value.parse().map_err(|error| {
        PaperStoreError::Store(crate::application::StoreError::InvalidData(format!(
            "invalid {label}: {error}"
        )))
    })
}

fn parse_json<T: serde::de::DeserializeOwned>(
    value: serde_json::Value,
    label: &str,
) -> Result<T, PaperStoreError> {
    serde_json::from_value(value).map_err(|error| {
        PaperStoreError::Store(crate::application::StoreError::InvalidData(format!(
            "invalid {label}: {error}"
        )))
    })
}

fn parse_u16(value: i32, label: &str) -> Result<u16, PaperStoreError> {
    u16::try_from(value).map_err(|_| {
        PaperStoreError::Store(crate::application::StoreError::InvalidData(format!(
            "{label} is out of range"
        )))
    })
}

fn fold_admin_papers(rows: Vec<AdminPaperRow>) -> Result<Vec<AdminPaper>, PaperStoreError> {
    let mut papers = Vec::new();
    for row in rows {
        let Some(existing) = papers.last_mut() else {
            papers.push(admin_paper_from_row(&row)?);
            continue;
        };
        if existing.id != row.id {
            papers.push(admin_paper_from_row(&row)?);
            continue;
        }
        append_admin_item(existing, &row)?;
    }
    Ok(papers)
}

fn admin_paper_from_row(row: &AdminPaperRow) -> Result<AdminPaper, PaperStoreError> {
    let mut paper = AdminPaper {
        id: row.id,
        status: parse_enum(&row.status, "paper status")?,
        title: row.title.clone(),
        description: row.description.clone(),
        audience: row.audience.clone(),
        mode: parse_enum(&row.mode, "paper mode")?,
        open_at: row.open_at.clone(),
        close_at: row.close_at.clone(),
        duration_seconds: row.duration_seconds.map(i64::from),
        max_attempts: parse_u16(row.max_attempts, "max attempts")?,
        allow_resume: row.allow_resume,
        auto_save: row.auto_save,
        auto_submit: row.auto_submit,
        candidate_fields: parse_json(row.candidate_fields.clone(), "candidate fields")?,
        result_visibility: parse_enum(&row.result_visibility, "result visibility")?,
        allow_preview: row.allow_preview,
        items: Vec::new(),
        total_score: 0.0,
    };
    append_admin_item(&mut paper, row)?;
    Ok(paper)
}

fn append_admin_item(paper: &mut AdminPaper, row: &AdminPaperRow) -> Result<(), PaperStoreError> {
    let Some(question_id) = row.question_id else {
        return Ok(());
    };
    let Some(revision_id) = row.revision_id else {
        return Err(PaperStoreError::Store(
            crate::application::StoreError::InvalidData(format!(
                "paper {} has an incomplete question reference",
                paper.id
            )),
        ));
    };
    let position = parse_u16(row.position.unwrap_or_default(), "paper question position")?;
    let score = row.paper_score.ok_or_else(|| {
        PaperStoreError::Store(crate::application::StoreError::InvalidData(
            "paper question score is missing".to_owned(),
        ))
    })?;
    let question_type = parse_enum(
        row.question_type.as_deref().unwrap_or_default(),
        "question type",
    )?;
    let stem = row.stem.clone().ok_or_else(|| {
        PaperStoreError::Store(crate::application::StoreError::InvalidData(
            "paper question stem is missing".to_owned(),
        ))
    })?;
    let blank_count = parse_u16(
        i32::from(row.blank_count.unwrap_or_default()),
        "blank count",
    )?;

    if let Some(item) = paper
        .items
        .iter_mut()
        .find(|item| item.question_id == question_id)
    {
        if let (Some(key), Some(text)) = (&row.option_key, &row.option_text) {
            item.question.options.push(QuestionOption {
                key: key.clone(),
                text: text.clone(),
            });
        }
        return Ok(());
    }

    let question_bank_id = row.question_bank_id.ok_or_else(|| {
        PaperStoreError::Store(crate::application::StoreError::InvalidData(
            "paper question bank is missing".to_owned(),
        ))
    })?;
    let question_bank_name = row.question_bank_name.clone().ok_or_else(|| {
        PaperStoreError::Store(crate::application::StoreError::InvalidData(
            "paper question bank name is missing".to_owned(),
        ))
    })?;
    let mut question = crate::domain::PublicQuestion {
        id: question_id,
        revision_id,
        question_bank_id,
        question_bank_name,
        question_type,
        stem,
        blank_count,
        options: Vec::new(),
    };
    if let (Some(key), Some(text)) = (&row.option_key, &row.option_text) {
        question.options.push(QuestionOption {
            key: key.clone(),
            text: text.clone(),
        });
    }
    paper.total_score += score;
    paper.items.push(PaperQuestion {
        question_id,
        revision_id,
        position,
        score,
        question,
    });
    Ok(())
}

fn fold_attempt(rows: Vec<AttemptRow>) -> Result<Option<StoredAttempt>, PaperStoreError> {
    let Some(first) = rows.first().cloned() else {
        return Ok(None);
    };
    let mut questions = Vec::new();
    for row in rows {
        if let Some(question) = questions
            .iter_mut()
            .find(|question: &&mut StoredAttemptQuestion| question.question_id == row.question_id)
        {
            if let (Some(key), Some(text)) = (row.option_key, row.option_text) {
                question.options.push(QuestionOption { key, text });
            }
            continue;
        }
        let answer = row
            .submitted_answer
            .map(|value| parse_json::<AnswerPayload>(value, "submitted answer"))
            .transpose()?;
        let question = StoredAttemptQuestion {
            question_id: row.question_id,
            revision_id: row.revision_id,
            position: parse_u16(row.position, "attempt question position")?,
            score: row.paper_score,
            question_type: parse_enum(&row.question_type, "question type")?,
            stem: row.stem.clone(),
            blank_count: parse_u16(i32::from(row.blank_count), "blank count")?,
            options: row
                .option_key
                .zip(row.option_text)
                .map(|(key, text)| vec![QuestionOption { key, text }])
                .unwrap_or_default(),
            correct_answer: parse_json(row.correct_answer_payload, "correct answer")?,
            explanation: row.explanation.clone(),
            answer,
            grading_status: parse_enum(&row.grading_status, "grading status")?,
            awarded_score: row.awarded_score,
            feedback: row.feedback.clone(),
        };
        questions.push(question);
    }

    Ok(Some(StoredAttempt {
        id: first.id,
        user_id: first.user_id,
        paper_id: first.paper_id,
        title: first.title.clone(),
        status: parse_enum(&first.status, "attempt status")?,
        started_at: first.started_at.clone(),
        deadline_at: first.deadline_at.clone(),
        auto_submit: first.auto_submit,
        deadline_reached: first.deadline_reached,
        submitted_at: first.submitted_at.clone(),
        candidate_info: parse_json(first.candidate_info.clone(), "candidate information")?,
        max_score: first.max_score,
        total_score: first.total_score,
        result_visibility: parse_enum(&first.result_visibility, "result visibility")?,
        questions,
    }))
}

fn fold_attempts(rows: Vec<AttemptRow>) -> Result<Vec<StoredAttempt>, PaperStoreError> {
    let mut attempts = Vec::new();
    let mut current_id = None;
    let mut current_rows = Vec::new();

    for row in rows {
        if current_id.is_some_and(|id| id != row.id) {
            if let Some(attempt) = fold_attempt(std::mem::take(&mut current_rows))? {
                attempts.push(attempt);
            }
        }
        current_id = Some(row.id);
        current_rows.push(row);
    }
    if let Some(attempt) = fold_attempt(current_rows)? {
        attempts.push(attempt);
    }
    Ok(attempts)
}

async fn fetch_admin_attempts(pool: &PgPool) -> Result<Vec<StoredAttempt>, PaperStoreError> {
    let sql = format!(
        "{ATTEMPT_SELECT_SQL} WHERE a.status <> 'in_progress'::attempt_status{ADMIN_ATTEMPT_ORDER_SQL}"
    );
    let rows = sqlx::query_as::<_, AttemptRow>(&sql)
        .fetch_all(pool)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
    fold_attempts(rows)
}

async fn fetch_admin_attempt(
    pool: &PgPool,
    attempt_id: i64,
) -> Result<Option<StoredAttempt>, PaperStoreError> {
    let sql = format!("{ATTEMPT_SELECT_SQL} WHERE a.id = $1{ATTEMPT_ORDER_SQL}");
    let rows = sqlx::query_as::<_, AttemptRow>(&sql)
        .bind(attempt_id)
        .fetch_all(pool)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
    fold_attempt(rows)
}

fn published_from_row(row: PublishedPaperRow) -> Result<PublishedPaper, PaperStoreError> {
    Ok(PublishedPaper {
        id: row.id,
        title: row.title,
        description: row.description,
        audience: row.audience,
        mode: parse_enum(&row.mode, "paper mode")?,
        runtime_status: PaperRuntimeStatus::Open,
        open_at: row.open_at,
        close_at: row.close_at,
        duration_seconds: row.duration_seconds.map(i64::from),
        max_attempts: parse_u16(row.max_attempts, "max attempts")?,
        allow_resume: row.allow_resume,
        auto_save: row.auto_save,
        auto_submit: row.auto_submit,
        candidate_fields: parse_json(row.candidate_fields, "candidate fields")?,
        result_visibility: parse_enum(&row.result_visibility, "result visibility")?,
        allow_preview: row.allow_preview,
        question_count: u16::try_from(row.question_count).map_err(|_| {
            PaperStoreError::Store(crate::application::StoreError::InvalidData(
                "question count is out of range".to_owned(),
            ))
        })?,
        total_score: row.total_score,
        current_attempt_id: row.current_attempt_id,
        current_attempt_status: row
            .current_attempt_status
            .map(|status| parse_enum(&status, "attempt status"))
            .transpose()?,
    })
}

async fn fetch_attempt(
    pool: &PgPool,
    user_id: i64,
    attempt_id: i64,
) -> Result<Option<StoredAttempt>, PaperStoreError> {
    let sql = format!("{ATTEMPT_SELECT_SQL} WHERE a.id = $1 AND a.user_id = $2{ATTEMPT_ORDER_SQL}");
    let rows = sqlx::query_as::<_, AttemptRow>(&sql)
        .bind(attempt_id)
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
    fold_attempt(rows)
}

async fn insert_audit(
    transaction: &mut Transaction<'_, Postgres>,
    actor_user_id: i64,
    action: &str,
    paper_id: i64,
) -> Result<(), PaperStoreError> {
    sqlx::query(
        r#"
INSERT INTO audit_logs (actor_user_id, action, entity_type, entity_id, details)
VALUES ($1, $2, 'paper', $3, '{}'::jsonb)
"#,
    )
    .bind(actor_user_id)
    .bind(action)
    .bind(paper_id)
    .execute(&mut **transaction)
    .await
    .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
    Ok(())
}

#[async_trait]
impl PaperStore for PgPaperStore {
    async fn list_admin_papers(&self) -> Result<Vec<AdminPaper>, PaperStoreError> {
        let sql = format!(
            "{ADMIN_PAPER_SELECT_SQL} ORDER BY p.id, pq.display_order NULLS LAST, o.display_order NULLS LAST"
        );
        let rows = sqlx::query_as::<_, AdminPaperRow>(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        fold_admin_papers(rows)
    }

    async fn get_admin_paper(&self, id: i64) -> Result<Option<AdminPaper>, PaperStoreError> {
        let sql = format!(
            "{ADMIN_PAPER_SELECT_SQL} WHERE p.id = $1 ORDER BY p.id, pq.display_order NULLS LAST, o.display_order NULLS LAST"
        );
        let rows = sqlx::query_as::<_, AdminPaperRow>(&sql)
            .bind(id)
            .fetch_all(&self.pool)
            .await
            .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        Ok(fold_admin_papers(rows)?.into_iter().next())
    }

    async fn create_paper_draft(
        &self,
        actor_user_id: i64,
        input: &CreatePaperInput,
        items: &[PaperQuestionSnapshot],
    ) -> Result<AdminPaper, PaperStoreError> {
        let mut transaction =
            self.pool.begin().await.map_err(|error| {
                PaperStoreError::Store(crate::application::StoreError::from(error))
            })?;
        let candidate_fields = serde_json::to_value(&input.candidate_fields).map_err(|error| {
            PaperStoreError::Store(crate::application::StoreError::InvalidData(
                error.to_string(),
            ))
        })?;
        let paper_id = sqlx::query_scalar::<_, i64>(
            r#"
INSERT INTO papers (
    title, description, audience, mode, status, open_at, close_at,
    duration_seconds, max_attempts, allow_resume, auto_save, auto_submit,
    candidate_fields, result_visibility, allow_preview, created_by
)
VALUES (
    $1, $2, $3, $4::paper_mode, 'draft'::paper_status, $5::timestamptz, $6::timestamptz,
    $7, $8, $9, $10, $11, $12, $13::result_visibility, $14, $15
)
RETURNING id
"#,
        )
        .bind(&input.title)
        .bind(&input.description)
        .bind(&input.audience)
        .bind(input.mode.to_string())
        .bind(&input.open_at)
        .bind(&input.close_at)
        .bind(input.duration_seconds)
        .bind(i32::from(input.max_attempts))
        .bind(input.allow_resume)
        .bind(input.auto_save)
        .bind(input.auto_submit)
        .bind(candidate_fields)
        .bind(input.result_visibility.to_string())
        .bind(input.allow_preview)
        .bind(actor_user_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;

        for item in items {
            sqlx::query(
                r#"
INSERT INTO paper_questions (paper_id, question_id, revision_id, display_order, score)
VALUES ($1, $2, $3, $4, $5::double precision)
"#,
            )
            .bind(paper_id)
            .bind(item.question_id)
            .bind(item.revision_id)
            .bind(i32::from(item.position))
            .bind(item.score)
            .execute(&mut *transaction)
            .await
            .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        }
        insert_audit(&mut transaction, actor_user_id, "paper_created", paper_id).await?;
        transaction
            .commit()
            .await
            .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        self.get_admin_paper(paper_id)
            .await?
            .ok_or(PaperStoreError::PaperNotFound)
    }

    async fn publish_paper(
        &self,
        actor_user_id: i64,
        id: i64,
    ) -> Result<Option<AdminPaper>, PaperStoreError> {
        let mut transaction =
            self.pool.begin().await.map_err(|error| {
                PaperStoreError::Store(crate::application::StoreError::from(error))
            })?;
        let result = sqlx::query(
            r#"
UPDATE papers
SET status = 'published'::paper_status,
    published_by = $2,
    published_at = now()
WHERE id = $1 AND status = 'draft'::paper_status
"#,
        )
        .bind(id)
        .bind(actor_user_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        if result.rows_affected() == 0 {
            transaction.rollback().await.map_err(|error| {
                PaperStoreError::Store(crate::application::StoreError::from(error))
            })?;
            return Ok(None);
        }
        insert_audit(&mut transaction, actor_user_id, "paper_published", id).await?;
        transaction
            .commit()
            .await
            .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        self.get_admin_paper(id).await
    }

    async fn archive_paper(
        &self,
        actor_user_id: i64,
        id: i64,
    ) -> Result<Option<AdminPaper>, PaperStoreError> {
        let mut transaction =
            self.pool.begin().await.map_err(|error| {
                PaperStoreError::Store(crate::application::StoreError::from(error))
            })?;
        let result = sqlx::query(
            "UPDATE papers SET status = 'archived'::paper_status WHERE id = $1 AND status <> 'archived'::paper_status",
        )
        .bind(id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        if result.rows_affected() == 0 {
            transaction.rollback().await.map_err(|error| {
                PaperStoreError::Store(crate::application::StoreError::from(error))
            })?;
            return self.get_admin_paper(id).await;
        }
        insert_audit(&mut transaction, actor_user_id, "paper_archived", id).await?;
        transaction
            .commit()
            .await
            .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        self.get_admin_paper(id).await
    }

    async fn list_published_papers(
        &self,
        user_id: i64,
    ) -> Result<Vec<PublishedPaper>, PaperStoreError> {
        let sql = format!(
            "{PUBLISHED_PAPER_SELECT_SQL} WHERE p.status = 'published'::paper_status \
      AND (p.open_at IS NULL OR p.open_at <= now()) \
      AND (p.close_at IS NULL OR p.close_at > now()) \
GROUP BY p.id, current_attempt.id, current_attempt.status"
        );
        let rows = sqlx::query_as::<_, PublishedPaperRow>(&sql)
            .bind(user_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        rows.into_iter().map(published_from_row).collect()
    }

    async fn get_published_paper(
        &self,
        user_id: i64,
        id: i64,
    ) -> Result<Option<PublishedPaper>, PaperStoreError> {
        let sql = format!(
            "{PUBLISHED_PAPER_SELECT_SQL} WHERE p.id = $2 \
      AND p.status = 'published'::paper_status \
      AND (p.open_at IS NULL OR p.open_at <= now()) \
      AND (p.close_at IS NULL OR p.close_at > now()) \
GROUP BY p.id, current_attempt.id, current_attempt.status"
        );
        let row = sqlx::query_as::<_, PublishedPaperRow>(&sql)
            .bind(user_id)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        row.map(published_from_row).transpose()
    }

    async fn create_attempt(
        &self,
        user_id: i64,
        paper_id: i64,
        candidate_info: CandidateInfo,
    ) -> Result<StoredAttempt, PaperStoreError> {
        let mut transaction =
            self.pool.begin().await.map_err(|error| {
                PaperStoreError::Store(crate::application::StoreError::from(error))
            })?;
        let paper = sqlx::query_as::<_, (bool, bool, i32, i32)>(
            r#"
SELECT
    (open_at IS NULL OR open_at <= now()) AND (close_at IS NULL OR close_at > now()),
    allow_resume,
    COALESCE(duration_seconds, 0),
    max_attempts
FROM papers
WHERE id = $1 AND status = 'published'::paper_status
FOR UPDATE
"#,
        )
        .bind(paper_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?
        .ok_or(PaperStoreError::PaperNotFound)?;
        if !paper.0 {
            return Err(PaperStoreError::PaperUnavailable);
        }

        if paper.1 {
            let existing = sqlx::query_scalar::<_, i64>(
                "SELECT id FROM attempts WHERE user_id = $1 AND paper_id = $2 AND status = 'in_progress'::attempt_status ORDER BY id DESC LIMIT 1",
            )
            .bind(user_id)
            .bind(paper_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|error| {
                PaperStoreError::Store(crate::application::StoreError::from(error))
            })?;
            if let Some(attempt_id) = existing {
                transaction.commit().await.map_err(|error| {
                    PaperStoreError::Store(crate::application::StoreError::from(error))
                })?;
                return fetch_attempt(&self.pool, user_id, attempt_id)
                    .await?
                    .ok_or(PaperStoreError::AttemptNotFound);
            }
        }

        let count = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attempts WHERE user_id = $1 AND paper_id = $2",
        )
        .bind(user_id)
        .bind(paper_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        if count >= i64::from(paper.3) {
            return Err(PaperStoreError::MaxAttemptsReached);
        }

        let candidate_info = serde_json::to_value(candidate_info).map_err(|error| {
            PaperStoreError::Store(crate::application::StoreError::InvalidData(
                error.to_string(),
            ))
        })?;
        let attempt_id = sqlx::query_scalar::<_, i64>(
            r#"
INSERT INTO attempts (
    user_id, paper_id, status, deadline_at, candidate_info, max_score
)
SELECT
    $1,
    p.id,
    'in_progress'::attempt_status,
    CASE
        WHEN p.duration_seconds IS NULL THEN p.close_at
        WHEN p.close_at IS NULL THEN now() + (p.duration_seconds::double precision * interval '1 second')
        ELSE LEAST(
            now() + (p.duration_seconds::double precision * interval '1 second'),
            p.close_at
        )
    END,
    $3,
    COALESCE((SELECT sum(score) FROM paper_questions WHERE paper_id = p.id), 0)::double precision
FROM papers AS p
WHERE p.id = $2
RETURNING id
"#,
        )
        .bind(user_id)
        .bind(paper_id)
        .bind(candidate_info)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        transaction
            .commit()
            .await
            .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        fetch_attempt(&self.pool, user_id, attempt_id)
            .await?
            .ok_or(PaperStoreError::AttemptNotFound)
    }

    async fn get_attempt(
        &self,
        user_id: i64,
        attempt_id: i64,
    ) -> Result<Option<StoredAttempt>, PaperStoreError> {
        fetch_attempt(&self.pool, user_id, attempt_id).await
    }

    async fn list_admin_attempts(&self) -> Result<Vec<StoredAttempt>, PaperStoreError> {
        fetch_admin_attempts(&self.pool).await
    }

    async fn get_admin_attempt(
        &self,
        attempt_id: i64,
    ) -> Result<Option<StoredAttempt>, PaperStoreError> {
        fetch_admin_attempt(&self.pool, attempt_id).await
    }

    async fn grade_answer(
        &self,
        actor_user_id: i64,
        attempt_id: i64,
        question_id: i64,
        score: f64,
        feedback: Option<String>,
    ) -> Result<StoredAttempt, PaperStoreError> {
        let mut transaction =
            self.pool.begin().await.map_err(|error| {
                PaperStoreError::Store(crate::application::StoreError::from(error))
            })?;
        let status = sqlx::query_scalar::<_, String>(
            "SELECT status::text FROM attempts WHERE id = $1 FOR UPDATE",
        )
        .bind(attempt_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?
        .ok_or(PaperStoreError::AttemptNotFound)?;
        if status == "in_progress" {
            return Err(PaperStoreError::AttemptNotSubmitted);
        }

        let answer_exists = sqlx::query_scalar::<_, bool>(
            r#"
SELECT aa.answer_payload IS NOT NULL
FROM attempts AS a
JOIN paper_questions AS pq ON pq.paper_id = a.paper_id AND pq.question_id = $2
LEFT JOIN attempt_answers AS aa
    ON aa.attempt_id = a.id AND aa.question_id = pq.question_id
WHERE a.id = $1
"#,
        )
        .bind(attempt_id)
        .bind(question_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?
        .ok_or(PaperStoreError::QuestionNotInAttempt)?;
        if !answer_exists {
            return Err(PaperStoreError::AnswerNotSaved);
        }

        let updated = sqlx::query(
            r#"
UPDATE attempt_answers
SET grading_status = 'graded'::grading_status,
    score = $3::double precision,
    reviewed_by = $4,
    feedback = $5,
    graded_at = now()
WHERE attempt_id = $1 AND question_id = $2
"#,
        )
        .bind(attempt_id)
        .bind(question_id)
        .bind(score)
        .bind(actor_user_id)
        .bind(feedback)
        .execute(&mut *transaction)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        if updated.rows_affected() == 0 {
            return Err(PaperStoreError::AnswerNotSaved);
        }

        sqlx::query(
            r#"
UPDATE attempts
SET status = CASE
        WHEN EXISTS (
            SELECT 1
            FROM attempt_answers
            WHERE attempt_id = $1 AND grading_status <> 'graded'::grading_status
        ) THEN 'needs_review'::attempt_status
        ELSE 'graded'::attempt_status
    END,
    score = (
        SELECT COALESCE(sum(score), 0)::double precision
        FROM attempt_answers
        WHERE attempt_id = $1
    )
WHERE id = $1
"#,
        )
        .bind(attempt_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;

        sqlx::query(
            r#"
INSERT INTO audit_logs (actor_user_id, action, entity_type, entity_id, details)
VALUES ($1, 'attempt_answer_graded', 'attempt', $2, jsonb_build_object('question_id', $3, 'score', $4))
"#,
        )
        .bind(actor_user_id)
        .bind(attempt_id)
        .bind(question_id)
        .bind(score)
        .execute(&mut *transaction)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;

        transaction
            .commit()
            .await
            .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        fetch_admin_attempt(&self.pool, attempt_id)
            .await?
            .ok_or(PaperStoreError::AttemptNotFound)
    }

    async fn save_answer(
        &self,
        user_id: i64,
        attempt_id: i64,
        question_id: i64,
        answer: AnswerPayload,
    ) -> Result<StoredAttempt, PaperStoreError> {
        let mut transaction =
            self.pool.begin().await.map_err(|error| {
                PaperStoreError::Store(crate::application::StoreError::from(error))
            })?;
        let state = sqlx::query_as::<_, (String, bool)>(
            r#"
SELECT
    status::text,
    deadline_at IS NOT NULL AND deadline_at <= clock_timestamp()
FROM attempts
WHERE id = $1 AND user_id = $2
FOR UPDATE
"#,
        )
        .bind(attempt_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?
        .ok_or(PaperStoreError::AttemptNotFound)?;
        if state.0 != "in_progress" || state.1 {
            return Err(PaperStoreError::AttemptClosed);
        }
        let updated = sqlx::query(
            r#"
INSERT INTO attempt_answers (
    attempt_id, question_id, revision_id, answer_payload, grading_status, score
)
SELECT a.id, pq.question_id, pq.revision_id, $3, 'pending'::grading_status, NULL
FROM attempts AS a
JOIN paper_questions AS pq ON pq.paper_id = a.paper_id AND pq.question_id = $2
WHERE a.id = $1 AND a.user_id = $4 AND a.status = 'in_progress'::attempt_status
  AND (a.deadline_at IS NULL OR a.deadline_at > clock_timestamp())
ON CONFLICT (attempt_id, question_id) DO UPDATE
SET answer_payload = EXCLUDED.answer_payload,
    revision_id = EXCLUDED.revision_id,
    grading_status = 'pending'::grading_status,
    score = NULL,
    reviewed_by = NULL,
    feedback = NULL,
    graded_at = NULL,
    submitted_at = now()
"#,
        )
        .bind(attempt_id)
        .bind(question_id)
        .bind(serde_json::to_value(answer).map_err(|error| {
            PaperStoreError::Store(crate::application::StoreError::InvalidData(
                error.to_string(),
            ))
        })?)
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        if updated.rows_affected() == 0 {
            return Err(PaperStoreError::QuestionNotInAttempt);
        }
        transaction
            .commit()
            .await
            .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        fetch_attempt(&self.pool, user_id, attempt_id)
            .await?
            .ok_or(PaperStoreError::AttemptNotFound)
    }

    async fn submit_attempt(
        &self,
        user_id: i64,
        attempt_id: i64,
        evaluations: &[GradedAnswer],
    ) -> Result<StoredAttempt, PaperStoreError> {
        let mut transaction =
            self.pool.begin().await.map_err(|error| {
                PaperStoreError::Store(crate::application::StoreError::from(error))
            })?;
        let status = sqlx::query_scalar::<_, String>(
            "SELECT status::text FROM attempts WHERE id = $1 AND user_id = $2 FOR UPDATE",
        )
        .bind(attempt_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?
        .ok_or(PaperStoreError::AttemptNotFound)?;
        if status != "in_progress" {
            transaction.commit().await.map_err(|error| {
                PaperStoreError::Store(crate::application::StoreError::from(error))
            })?;
            return fetch_attempt(&self.pool, user_id, attempt_id)
                .await?
                .ok_or(PaperStoreError::AttemptNotFound);
        }

        let mut total_score = 0.0_f64;
        let mut needs_review = false;
        for evaluation in evaluations {
            total_score += evaluation.awarded_score.unwrap_or(0.0);
            needs_review |= evaluation.grading_status == GradingStatus::NeedsReview;
            sqlx::query(
                r#"
UPDATE attempt_answers
SET grading_status = $3::grading_status,
    score = $4::double precision,
    graded_at = now()
WHERE attempt_id = $1 AND question_id = $2
"#,
            )
            .bind(attempt_id)
            .bind(evaluation.question_id)
            .bind(evaluation.grading_status.to_string())
            .bind(evaluation.awarded_score)
            .execute(&mut *transaction)
            .await
            .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        }
        let attempt_status = if needs_review {
            "needs_review"
        } else {
            "graded"
        };
        sqlx::query(
            r#"
UPDATE attempts
SET status = $3::attempt_status,
    submitted_at = COALESCE(submitted_at, now()),
    score = CASE
        WHEN $3::attempt_status = 'needs_review'::attempt_status THEN NULL
        ELSE $4::double precision
    END
WHERE id = $1 AND user_id = $2
"#,
        )
        .bind(attempt_id)
        .bind(user_id)
        .bind(attempt_status)
        .bind(total_score)
        .execute(&mut *transaction)
        .await
        .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        transaction
            .commit()
            .await
            .map_err(|error| PaperStoreError::Store(crate::application::StoreError::from(error)))?;
        fetch_attempt(&self.pool, user_id, attempt_id)
            .await?
            .ok_or(PaperStoreError::AttemptNotFound)
    }
}
