mod auth;
mod paper;

pub use auth::{
    AuthError, AuthService, AuthStore, AuthStoreError, AuthenticatedSession, StoredUser,
    hash_password, hash_session_token,
};
pub use paper::{
    ExamError, ExamService, GradedAnswer, PaperManagementError, PaperManagementService,
    PaperQuestionSnapshot, PaperStore, PaperStoreError, StoredAttempt, StoredAttemptQuestion,
};

use std::sync::Arc;

use async_trait::async_trait;
use serde::Serialize;
use thiserror::Error;

use crate::domain::{
    AdminQuestion, AdminQuestionInput, AnswerPayload, CorrectAnswer, Evaluation, EvaluationError,
    PublicQuestion, QuestionBank, QuestionBankInput, QuestionBankValidationError,
    QuestionImportBatch, QuestionStatus, QuestionType, QuestionValidationError, ScoringQuestion,
    evaluate_answer,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AdminQuestionFilters {
    pub keyword: Option<String>,
    pub bank_id: Option<i64>,
    pub question_type: Option<QuestionType>,
    pub status: Option<QuestionStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionImportItemStatus {
    Inserted,
    Skipped,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuestionImportItem {
    pub index: usize,
    pub status: QuestionImportItemStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuestionImportReport {
    pub inserted: usize,
    pub skipped: usize,
    pub errors: usize,
    pub items: Vec<QuestionImportItem>,
}

impl QuestionImportReport {
    fn validation_errors(items: Vec<QuestionImportItem>) -> Self {
        Self {
            inserted: 0,
            skipped: 0,
            errors: items.len(),
            items,
        }
    }
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database operation failed: {0}")]
    Database(String),
    #[error("invalid question data: {0}")]
    InvalidData(String),
}

#[async_trait]
pub trait QuestionStore: Send + Sync {
    async fn list_question_banks(&self) -> Result<Vec<QuestionBank>, StoreError>;
    async fn get_question_bank(&self, id: i64) -> Result<Option<QuestionBank>, StoreError>;
    async fn create_question_bank(
        &self,
        input: QuestionBankInput,
    ) -> Result<QuestionBank, StoreError>;
    async fn list_published(&self, bank_id: Option<i64>)
    -> Result<Vec<PublicQuestion>, StoreError>;
    async fn get_published(&self, id: i64) -> Result<Option<PublicQuestion>, StoreError>;
    async fn get_for_scoring(&self, id: i64) -> Result<Option<ScoringQuestion>, StoreError>;
    async fn create_draft(&self, input: AdminQuestionInput) -> Result<AdminQuestion, StoreError>;
    async fn list_admin(
        &self,
        filters: &AdminQuestionFilters,
    ) -> Result<Vec<AdminQuestion>, StoreError>;
    async fn import_published(
        &self,
        inputs: Vec<AdminQuestionInput>,
    ) -> Result<QuestionImportReport, StoreError>;
    async fn get_admin(&self, id: i64) -> Result<Option<AdminQuestion>, StoreError>;
    async fn publish(&self, id: i64) -> Result<Option<AdminQuestion>, StoreError>;
    async fn archive(&self, id: i64) -> Result<Option<AdminQuestion>, StoreError>;
}

#[derive(Debug, Error)]
pub enum PracticeError {
    #[error("question not found")]
    NotFound,
    #[error(transparent)]
    Evaluation(#[from] EvaluationError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckedAnswer {
    pub question_id: i64,
    pub evaluation: Evaluation,
    pub explanation: Option<String>,
    pub correct_answer: CorrectAnswer,
}

#[derive(Clone)]
pub struct PracticeService {
    questions: Arc<dyn QuestionStore>,
}

impl PracticeService {
    pub fn new(questions: Arc<dyn QuestionStore>) -> Self {
        Self { questions }
    }

    pub async fn list_question_banks(&self) -> Result<Vec<QuestionBank>, StoreError> {
        self.questions.list_question_banks().await
    }

    pub async fn list_published(
        &self,
        bank_id: Option<i64>,
    ) -> Result<Vec<PublicQuestion>, StoreError> {
        self.questions.list_published(bank_id).await
    }

    pub async fn get_published(&self, id: i64) -> Result<Option<PublicQuestion>, StoreError> {
        self.questions.get_published(id).await
    }

    pub async fn check_answer(
        &self,
        id: i64,
        answer: &AnswerPayload,
    ) -> Result<CheckedAnswer, PracticeError> {
        let question = self
            .questions
            .get_for_scoring(id)
            .await?
            .ok_or(PracticeError::NotFound)?;
        let evaluation = evaluate_answer(answer, &question.correct_answer)?;

        Ok(CheckedAnswer {
            question_id: id,
            evaluation,
            explanation: question.explanation,
            correct_answer: question.correct_answer,
        })
    }
}

#[derive(Debug, Error)]
pub enum QuestionManagementError {
    #[error("question not found")]
    NotFound,
    #[error("question bank not found")]
    QuestionBankNotFound,
    #[error("question bank name is already in use")]
    QuestionBankNameTaken,
    #[error("bulk import validation failed")]
    ImportValidation(QuestionImportReport),
    #[error(transparent)]
    InvalidInput(#[from] QuestionValidationError),
    #[error(transparent)]
    InvalidQuestionBankInput(#[from] QuestionBankValidationError),
    #[error("question cannot transition from {0} to the requested state")]
    InvalidState(QuestionStatus),
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(Clone)]
pub struct QuestionManagementService {
    questions: Arc<dyn QuestionStore>,
}

impl QuestionManagementService {
    pub fn new(questions: Arc<dyn QuestionStore>) -> Self {
        Self { questions }
    }

    pub async fn list_banks(&self) -> Result<Vec<QuestionBank>, QuestionManagementError> {
        Ok(self.questions.list_question_banks().await?)
    }

    pub async fn create_bank(
        &self,
        input: QuestionBankInput,
    ) -> Result<QuestionBank, QuestionManagementError> {
        input.validate()?;
        let normalized_name = input.name.trim();
        if self
            .questions
            .list_question_banks()
            .await?
            .iter()
            .any(|bank| bank.name == normalized_name)
        {
            return Err(QuestionManagementError::QuestionBankNameTaken);
        }
        Ok(self.questions.create_question_bank(input).await?)
    }

    pub async fn list(
        &self,
        filters: AdminQuestionFilters,
    ) -> Result<Vec<AdminQuestion>, QuestionManagementError> {
        let filters = AdminQuestionFilters {
            keyword: filters
                .keyword
                .map(|keyword| keyword.trim().to_owned())
                .filter(|keyword| !keyword.is_empty()),
            ..filters
        };
        Ok(self.questions.list_admin(&filters).await?)
    }

    pub async fn import_add_only(
        &self,
        batch: QuestionImportBatch,
    ) -> Result<QuestionImportReport, QuestionManagementError> {
        if batch.items.is_empty() {
            return Err(QuestionManagementError::ImportValidation(
                QuestionImportReport::validation_errors(vec![QuestionImportItem {
                    index: 0,
                    status: QuestionImportItemStatus::Error,
                    question_id: None,
                    error: Some("items must not be empty".to_owned()),
                }]),
            ));
        }

        let mut errors = Vec::new();
        for (index, input) in batch.items.iter().enumerate() {
            if let Err(error) = input.validate() {
                errors.push(QuestionImportItem {
                    index,
                    status: QuestionImportItemStatus::Error,
                    question_id: None,
                    error: Some(error.to_string()),
                });
                continue;
            }
            if self
                .questions
                .get_question_bank(input.question_bank_id)
                .await?
                .is_none()
            {
                errors.push(QuestionImportItem {
                    index,
                    status: QuestionImportItemStatus::Error,
                    question_id: None,
                    error: Some("question bank was not found".to_owned()),
                });
            }
        }
        if !errors.is_empty() {
            return Err(QuestionManagementError::ImportValidation(
                QuestionImportReport::validation_errors(errors),
            ));
        }

        Ok(self.questions.import_published(batch.items).await?)
    }

    pub async fn create_draft(
        &self,
        input: AdminQuestionInput,
    ) -> Result<AdminQuestion, QuestionManagementError> {
        input.validate()?;
        if self
            .questions
            .get_question_bank(input.question_bank_id)
            .await?
            .is_none()
        {
            return Err(QuestionManagementError::QuestionBankNotFound);
        }
        Ok(self.questions.create_draft(input).await?)
    }

    async fn require_question(&self, id: i64) -> Result<AdminQuestion, QuestionManagementError> {
        self.questions
            .get_admin(id)
            .await?
            .ok_or(QuestionManagementError::NotFound)
    }

    pub async fn publish(&self, id: i64) -> Result<AdminQuestion, QuestionManagementError> {
        let question = self.require_question(id).await?;
        if question.status != QuestionStatus::Draft {
            return Err(QuestionManagementError::InvalidState(question.status));
        }

        question.as_input().validate()?;

        self.questions
            .publish(id)
            .await?
            .ok_or(QuestionManagementError::NotFound)
    }

    pub async fn archive(&self, id: i64) -> Result<AdminQuestion, QuestionManagementError> {
        let question = self.require_question(id).await?;
        if question.status == QuestionStatus::Archived {
            return Ok(question);
        }

        self.questions
            .archive(id)
            .await?
            .ok_or(QuestionManagementError::NotFound)
    }
}
