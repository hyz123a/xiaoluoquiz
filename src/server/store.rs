use async_trait::async_trait;
use sqlx::{FromRow, PgPool, Postgres, Transaction};

use crate::application::{
    AdminQuestionFilters, QuestionImportItem, QuestionImportItemStatus, QuestionImportReport,
    QuestionStore, StoreError,
};
use crate::domain::{
    AdminQuestion, AdminQuestionInput, CorrectAnswer, PublicQuestion, QuestionBank,
    QuestionBankInput, QuestionOption, QuestionStatus, QuestionType, ScoringQuestion,
};

impl From<sqlx::Error> for StoreError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error.to_string())
    }
}

#[derive(Clone)]
pub struct PgQuestionStore {
    pool: PgPool,
}

impl PgQuestionStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(Debug, FromRow)]
struct QuestionBankRow {
    id: i64,
    name: String,
    description: Option<String>,
    question_count: i64,
}

#[derive(Debug, FromRow)]
struct QuestionRow {
    id: i64,
    revision_id: i64,
    question_bank_id: i64,
    question_bank_name: String,
    question_type: String,
    stem: String,
    blank_count: i16,
    option_key: Option<String>,
    option_text: Option<String>,
}

#[derive(Debug, FromRow)]
struct AdminQuestionRow {
    id: i64,
    revision_id: i64,
    question_bank_id: i64,
    question_bank_name: String,
    status: String,
    question_type: String,
    stem: String,
    blank_count: i16,
    explanation: Option<String>,
    option_key: Option<String>,
    option_text: Option<String>,
    answer_payload: serde_json::Value,
}

#[derive(Debug, FromRow)]
struct AnswerRow {
    explanation: Option<String>,
    answer_payload: serde_json::Value,
}

const PUBLIC_LIST_SQL: &str = r#"
SELECT
    q.id,
    r.id AS revision_id,
    q.question_bank_id,
    b.name AS question_bank_name,
    r.question_type::text AS question_type,
    r.stem,
    r.blank_count,
    o.option_key,
    o.option_text
FROM questions AS q
JOIN question_banks AS b ON b.id = q.question_bank_id
JOIN question_revisions AS r ON r.id = q.published_revision_id
LEFT JOIN question_options AS o ON o.revision_id = r.id
WHERE q.status::text = 'published'
"#;

const PUBLIC_ONE_SQL: &str = r#"
SELECT
    q.id,
    r.id AS revision_id,
    q.question_bank_id,
    b.name AS question_bank_name,
    r.question_type::text AS question_type,
    r.stem,
    r.blank_count,
    o.option_key,
    o.option_text
FROM questions AS q
JOIN question_banks AS b ON b.id = q.question_bank_id
JOIN question_revisions AS r ON r.id = q.published_revision_id
LEFT JOIN question_options AS o ON o.revision_id = r.id
WHERE q.status::text = 'published' AND q.id = $1
ORDER BY o.display_order NULLS LAST
"#;

const ADMIN_ONE_SQL: &str = r#"
SELECT
    q.id,
    r.id AS revision_id,
    q.question_bank_id,
    b.name AS question_bank_name,
    q.status::text AS status,
    r.question_type::text AS question_type,
    r.stem,
    r.blank_count,
    r.explanation,
    o.option_key,
    o.option_text,
    a.answer_payload
FROM questions AS q
JOIN question_banks AS b ON b.id = q.question_bank_id
JOIN question_revisions AS r
    ON r.id = COALESCE(
        q.published_revision_id,
        (
            SELECT latest.id
            FROM question_revisions AS latest
            WHERE latest.question_id = q.id
            ORDER BY latest.version DESC
            LIMIT 1
        )
    )
JOIN question_answers AS a ON a.revision_id = r.id
LEFT JOIN question_options AS o ON o.revision_id = r.id
WHERE q.id = $1
ORDER BY o.display_order NULLS LAST
"#;

impl PgQuestionStore {
    async fn fetch_public_rows(
        &self,
        sql: &str,
        id: Option<i64>,
    ) -> Result<Vec<QuestionRow>, StoreError> {
        let mut query = sqlx::query_as::<_, QuestionRow>(sql);
        if let Some(id) = id {
            query = query.bind(id);
        }
        Ok(query.fetch_all(&self.pool).await?)
    }

    async fn fetch_answer(&self, id: i64) -> Result<Option<AnswerRow>, StoreError> {
        Ok(sqlx::query_as::<_, AnswerRow>(
            r#"
SELECT r.explanation, a.answer_payload
FROM questions AS q
JOIN question_revisions AS r ON r.id = q.published_revision_id
JOIN question_answers AS a ON a.revision_id = r.id
WHERE q.status::text = 'published' AND q.id = $1
"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?)
    }

    async fn fetch_admin_rows(&self, id: i64) -> Result<Vec<AdminQuestionRow>, StoreError> {
        Ok(sqlx::query_as::<_, AdminQuestionRow>(ADMIN_ONE_SQL)
            .bind(id)
            .fetch_all(&self.pool)
            .await?)
    }

    async fn find_existing_question_id(
        transaction: &mut Transaction<'_, Postgres>,
        input: &AdminQuestionInput,
    ) -> Result<Option<i64>, StoreError> {
        Ok(sqlx::query_scalar::<_, i64>(
            r#"
SELECT q.id
FROM questions AS q
JOIN question_revisions AS r
    ON r.id = COALESCE(
        q.published_revision_id,
        (
            SELECT latest.id
            FROM question_revisions AS latest
            WHERE latest.question_id = q.id
            ORDER BY latest.version DESC
            LIMIT 1
        )
    )
WHERE q.question_bank_id = $1
  AND lower(btrim(r.stem)) = lower(btrim($2))
ORDER BY q.id
LIMIT 1
"#,
        )
        .bind(input.question_bank_id)
        .bind(&input.stem)
        .fetch_optional(&mut **transaction)
        .await?)
    }

    async fn insert_draft(
        transaction: &mut Transaction<'_, Postgres>,
        input: &AdminQuestionInput,
    ) -> Result<i64, StoreError> {
        let question_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO questions (question_bank_id, status) VALUES ($1, 'draft'::question_status) RETURNING id",
        )
        .bind(input.question_bank_id)
        .fetch_one(&mut **transaction)
        .await?;
        let blank_count = i16::try_from(input.blank_count)
            .map_err(|_| StoreError::InvalidData("blank_count is too large".to_owned()))?;
        let revision_id = sqlx::query_scalar::<_, i64>(
            r#"
INSERT INTO question_revisions (
    question_id, version, question_type, stem, explanation, blank_count
)
VALUES ($1, 1, $2::question_type, $3, $4, $5)
RETURNING id
"#,
        )
        .bind(question_id)
        .bind(input.question_type.to_string())
        .bind(&input.stem)
        .bind(&input.explanation)
        .bind(blank_count)
        .fetch_one(&mut **transaction)
        .await?;

        for (display_order, option) in input.options.iter().enumerate() {
            let display_order = i32::try_from(display_order)
                .map_err(|_| StoreError::InvalidData("too many options".to_owned()))?;
            sqlx::query(
                r#"
INSERT INTO question_options (revision_id, option_key, option_text, display_order)
VALUES ($1, $2, $3, $4)
"#,
            )
            .bind(revision_id)
            .bind(&option.key)
            .bind(&option.text)
            .bind(display_order)
            .execute(&mut **transaction)
            .await?;
        }

        let answer_payload = serde_json::to_value(&input.correct_answer)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        sqlx::query("INSERT INTO question_answers (revision_id, answer_payload) VALUES ($1, $2)")
            .bind(revision_id)
            .bind(answer_payload)
            .execute(&mut **transaction)
            .await?;
        Self::insert_audit_log(transaction, "question_created", question_id).await?;
        Ok(question_id)
    }

    async fn publish_in_transaction(
        transaction: &mut Transaction<'_, Postgres>,
        question_id: i64,
    ) -> Result<bool, StoreError> {
        let result = sqlx::query(
            r#"
UPDATE questions AS q
SET status = 'published'::question_status,
    published_revision_id = (
        SELECT latest.id
        FROM question_revisions AS latest
        WHERE latest.question_id = q.id
        ORDER BY latest.version DESC
        LIMIT 1
    ),
    published_at = now()
WHERE q.id = $1
  AND q.status = 'draft'::question_status
"#,
        )
        .bind(question_id)
        .execute(&mut **transaction)
        .await?;
        if result.rows_affected() == 0 {
            return Ok(false);
        }
        Self::insert_audit_log(transaction, "question_published", question_id).await?;
        Ok(true)
    }

    async fn insert_audit_log(
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        action: &str,
        question_id: i64,
    ) -> Result<(), StoreError> {
        sqlx::query(
            r#"
INSERT INTO audit_logs (action, entity_type, entity_id, details)
VALUES ($1, 'question', $2, '{}'::jsonb)
"#,
        )
        .bind(action)
        .bind(question_id)
        .execute(&mut **transaction)
        .await?;
        Ok(())
    }

    async fn list_bank_rows(&self) -> Result<Vec<QuestionBankRow>, StoreError> {
        Ok(sqlx::query_as::<_, QuestionBankRow>(
            r#"
SELECT
    b.id,
    b.name,
    b.description,
    COUNT(q.id) FILTER (WHERE q.status = 'published'::question_status)::bigint AS question_count
FROM question_banks AS b
LEFT JOIN questions AS q ON q.question_bank_id = b.id
GROUP BY b.id
ORDER BY b.id
"#,
        )
        .fetch_all(&self.pool)
        .await?)
    }
}

#[async_trait]
impl QuestionStore for PgQuestionStore {
    async fn list_question_banks(&self) -> Result<Vec<QuestionBank>, StoreError> {
        self.list_bank_rows()
            .await?
            .into_iter()
            .map(question_bank_from_row)
            .collect()
    }

    async fn get_question_bank(&self, id: i64) -> Result<Option<QuestionBank>, StoreError> {
        let row = sqlx::query_as::<_, QuestionBankRow>(
            r#"
SELECT
    b.id,
    b.name,
    b.description,
    COUNT(q.id) FILTER (WHERE q.status = 'published'::question_status)::bigint AS question_count
FROM question_banks AS b
LEFT JOIN questions AS q ON q.question_bank_id = b.id
WHERE b.id = $1
GROUP BY b.id
"#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(question_bank_from_row).transpose()
    }

    async fn create_question_bank(
        &self,
        input: QuestionBankInput,
    ) -> Result<QuestionBank, StoreError> {
        let name = input.name.trim().to_owned();
        let description = input
            .description
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        let row = sqlx::query_as::<_, QuestionBankRow>(
            r#"
INSERT INTO question_banks (name, description)
VALUES ($1, $2)
RETURNING id, name, description, 0::bigint AS question_count
"#,
        )
        .bind(name)
        .bind(description)
        .fetch_one(&self.pool)
        .await?;
        question_bank_from_row(row)
    }

    async fn list_published(
        &self,
        bank_id: Option<i64>,
    ) -> Result<Vec<PublicQuestion>, StoreError> {
        let sql = if bank_id.is_some() {
            format!(
                "{PUBLIC_LIST_SQL} AND q.question_bank_id = $1 ORDER BY q.id, o.display_order NULLS LAST"
            )
        } else {
            format!("{PUBLIC_LIST_SQL} ORDER BY q.id, o.display_order NULLS LAST")
        };
        let mut query = sqlx::query_as::<_, QuestionRow>(&sql);
        if let Some(bank_id) = bank_id {
            query = query.bind(bank_id);
        }
        fold_public_questions(query.fetch_all(&self.pool).await?)
    }

    async fn get_published(&self, id: i64) -> Result<Option<PublicQuestion>, StoreError> {
        let rows = self.fetch_public_rows(PUBLIC_ONE_SQL, Some(id)).await?;
        Ok(fold_public_questions(rows)?.into_iter().next())
    }

    async fn get_for_scoring(&self, id: i64) -> Result<Option<ScoringQuestion>, StoreError> {
        let Some(public) = self.get_published(id).await? else {
            return Ok(None);
        };
        let Some(row) = self.fetch_answer(id).await? else {
            return Err(StoreError::InvalidData(format!(
                "published question {id} has no answer"
            )));
        };
        let correct_answer = serde_json::from_value::<CorrectAnswer>(row.answer_payload)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;

        Ok(Some(ScoringQuestion {
            public,
            explanation: row.explanation,
            correct_answer,
        }))
    }

    async fn create_draft(&self, input: AdminQuestionInput) -> Result<AdminQuestion, StoreError> {
        let mut transaction = self.pool.begin().await?;
        let question_id = Self::insert_draft(&mut transaction, &input).await?;
        transaction.commit().await?;

        self.get_admin(question_id)
            .await?
            .ok_or_else(|| StoreError::InvalidData("created question could not be read".to_owned()))
    }

    async fn list_admin(
        &self,
        filters: &AdminQuestionFilters,
    ) -> Result<Vec<AdminQuestion>, StoreError> {
        let ids = sqlx::query_scalar::<_, i64>(
            r#"
SELECT q.id
FROM questions AS q
JOIN question_banks AS b ON b.id = q.question_bank_id
JOIN question_revisions AS r
    ON r.id = COALESCE(
        q.published_revision_id,
        (
            SELECT latest.id
            FROM question_revisions AS latest
            WHERE latest.question_id = q.id
            ORDER BY latest.version DESC
            LIMIT 1
        )
    )
WHERE ($1::bigint IS NULL OR q.question_bank_id = $1)
  AND ($2::text IS NULL OR r.question_type::text = $2)
  AND ($3::text IS NULL OR q.status::text = $3)
  AND (
      $4::text IS NULL
      OR r.stem ILIKE '%' || $4 || '%'
      OR b.name ILIKE '%' || $4 || '%'
  )
ORDER BY q.updated_at DESC, q.id DESC
"#,
        )
        .bind(filters.bank_id)
        .bind(
            filters
                .question_type
                .map(|question_type| question_type.to_string()),
        )
        .bind(filters.status.map(|status| status.to_string()))
        .bind(filters.keyword.as_deref())
        .fetch_all(&self.pool)
        .await?;
        let mut questions = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(question) = self.get_admin(id).await? {
                questions.push(question);
            }
        }
        Ok(questions)
    }

    async fn import_published(
        &self,
        inputs: Vec<AdminQuestionInput>,
    ) -> Result<QuestionImportReport, StoreError> {
        let mut transaction = self.pool.begin().await?;
        let mut report = QuestionImportReport {
            inserted: 0,
            skipped: 0,
            errors: 0,
            items: Vec::with_capacity(inputs.len()),
        };

        for (index, input) in inputs.iter().enumerate() {
            if let Some(question_id) =
                Self::find_existing_question_id(&mut transaction, input).await?
            {
                report.skipped += 1;
                report.items.push(QuestionImportItem {
                    index,
                    status: QuestionImportItemStatus::Skipped,
                    question_id: Some(question_id),
                    error: None,
                });
                continue;
            }

            let question_id = Self::insert_draft(&mut transaction, input).await?;
            if !Self::publish_in_transaction(&mut transaction, question_id).await? {
                return Err(StoreError::InvalidData(
                    "newly imported question could not be published".to_owned(),
                ));
            }
            report.inserted += 1;
            report.items.push(QuestionImportItem {
                index,
                status: QuestionImportItemStatus::Inserted,
                question_id: Some(question_id),
                error: None,
            });
        }

        transaction.commit().await?;
        Ok(report)
    }

    async fn get_admin(&self, id: i64) -> Result<Option<AdminQuestion>, StoreError> {
        fold_admin_question(self.fetch_admin_rows(id).await?)
    }

    async fn publish(&self, id: i64) -> Result<Option<AdminQuestion>, StoreError> {
        let mut transaction = self.pool.begin().await?;
        if !Self::publish_in_transaction(&mut transaction, id).await? {
            transaction.rollback().await?;
            return Ok(None);
        }
        transaction.commit().await?;
        self.get_admin(id).await
    }

    async fn archive(&self, id: i64) -> Result<Option<AdminQuestion>, StoreError> {
        let mut transaction = self.pool.begin().await?;
        let result =
            sqlx::query("UPDATE questions SET status = 'archived'::question_status WHERE id = $1")
                .bind(id)
                .execute(&mut *transaction)
                .await?;
        if result.rows_affected() == 0 {
            transaction.rollback().await?;
            return Ok(None);
        }
        Self::insert_audit_log(&mut transaction, "question_archived", id).await?;
        transaction.commit().await?;
        self.get_admin(id).await
    }
}

fn question_bank_from_row(row: QuestionBankRow) -> Result<QuestionBank, StoreError> {
    let question_count = u32::try_from(row.question_count).map_err(|_| {
        StoreError::InvalidData("question bank question count is out of range".to_owned())
    })?;
    Ok(QuestionBank {
        id: row.id,
        name: row.name,
        description: row.description,
        question_count,
    })
}

fn fold_public_questions(rows: Vec<QuestionRow>) -> Result<Vec<PublicQuestion>, StoreError> {
    let mut questions: Vec<PublicQuestion> = Vec::new();

    for row in rows {
        if let Some(question) = questions.last_mut() {
            if question.id == row.id {
                if let (Some(key), Some(text)) = (row.option_key, row.option_text) {
                    question.options.push(QuestionOption { key, text });
                }
                continue;
            }
        }

        let question_type = row
            .question_type
            .parse::<QuestionType>()
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let blank_count = u16::try_from(row.blank_count)
            .map_err(|_| StoreError::InvalidData("blank_count must not be negative".to_owned()))?;
        let mut question = PublicQuestion {
            id: row.id,
            revision_id: row.revision_id,
            question_bank_id: row.question_bank_id,
            question_bank_name: row.question_bank_name,
            question_type,
            stem: row.stem,
            blank_count,
            options: Vec::new(),
        };
        if let (Some(key), Some(text)) = (row.option_key, row.option_text) {
            question.options.push(QuestionOption { key, text });
        }
        questions.push(question);
    }

    Ok(questions)
}

fn fold_admin_question(rows: Vec<AdminQuestionRow>) -> Result<Option<AdminQuestion>, StoreError> {
    let Some(first) = rows.first() else {
        return Ok(None);
    };
    let id = first.id;
    let revision_id = first.revision_id;
    let question_bank_id = first.question_bank_id;
    let question_bank_name = first.question_bank_name.clone();
    let stem = first.stem.clone();
    let explanation = first.explanation.clone();
    let answer_payload = first.answer_payload.clone();
    let question_type = first
        .question_type
        .parse::<QuestionType>()
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    let status = first
        .status
        .parse::<QuestionStatus>()
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    let blank_count = u16::try_from(first.blank_count)
        .map_err(|_| StoreError::InvalidData("blank_count must not be negative".to_owned()))?;
    let correct_answer = serde_json::from_value::<CorrectAnswer>(answer_payload)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    let mut options = Vec::new();
    for row in rows {
        if let (Some(key), Some(text)) = (row.option_key, row.option_text) {
            options.push(QuestionOption { key, text });
        }
    }

    Ok(Some(AdminQuestion {
        id,
        revision_id,
        question_bank_id,
        question_bank_name,
        status,
        question_type,
        stem,
        blank_count,
        options,
        explanation,
        correct_answer,
    }))
}
