use std::{collections::HashSet, fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::question::{
    AnswerPayload, CorrectAnswer, EvaluationStatus, PublicQuestion, QuestionOption, QuestionType,
};

#[derive(Default, Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PaperMode {
    #[default]
    Exam,
    Practice,
}

impl fmt::Display for PaperMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Exam => "exam",
            Self::Practice => "practice",
        })
    }
}

impl FromStr for PaperMode {
    type Err = InvalidPaperMode;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "exam" => Ok(Self::Exam),
            "practice" => Ok(Self::Practice),
            other => Err(InvalidPaperMode(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown paper mode: {0}")]
pub struct InvalidPaperMode(String);

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PaperStatus {
    Draft,
    Published,
    Archived,
}

impl fmt::Display for PaperStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Draft => "draft",
            Self::Published => "published",
            Self::Archived => "archived",
        })
    }
}

impl FromStr for PaperStatus {
    type Err = InvalidPaperStatus;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "draft" => Ok(Self::Draft),
            "published" => Ok(Self::Published),
            "archived" => Ok(Self::Archived),
            other => Err(InvalidPaperStatus(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown paper status: {0}")]
pub struct InvalidPaperStatus(String);

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PaperRuntimeStatus {
    Upcoming,
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultVisibility {
    #[default]
    AfterSubmit,
    AfterGrading,
    AdminRelease,
}

impl fmt::Display for ResultVisibility {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::AfterSubmit => "after_submit",
            Self::AfterGrading => "after_grading",
            Self::AdminRelease => "admin_release",
        })
    }
}

impl FromStr for ResultVisibility {
    type Err = InvalidResultVisibility;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "after_submit" => Ok(Self::AfterSubmit),
            "after_grading" => Ok(Self::AfterGrading),
            "admin_release" => Ok(Self::AdminRelease),
            other => Err(InvalidResultVisibility(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown result visibility: {0}")]
pub struct InvalidResultVisibility(String);

#[derive(Debug, Clone, Copy, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateField {
    StudentNumber,
    Name,
}

impl fmt::Display for CandidateField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::StudentNumber => "student_number",
            Self::Name => "name",
        })
    }
}

impl FromStr for CandidateField {
    type Err = InvalidCandidateField;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "student_number" => Ok(Self::StudentNumber),
            "name" => Ok(Self::Name),
            other => Err(InvalidCandidateField(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown candidate field: {0}")]
pub struct InvalidCandidateField(String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateFieldConfig {
    pub key: CandidateField,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateInfo {
    pub student_number: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperQuestionInput {
    pub question_id: i64,
    #[serde(default)]
    pub score: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreatePaperInput {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub audience: Option<String>,
    #[serde(default)]
    pub mode: PaperMode,
    #[serde(default)]
    pub open_at: Option<String>,
    #[serde(default)]
    pub close_at: Option<String>,
    #[serde(default)]
    pub duration_seconds: Option<i64>,
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u16,
    #[serde(default = "default_true")]
    pub allow_resume: bool,
    #[serde(default = "default_true")]
    pub auto_save: bool,
    #[serde(default = "default_true")]
    pub auto_submit: bool,
    #[serde(default)]
    pub candidate_fields: Vec<CandidateFieldConfig>,
    #[serde(default)]
    pub result_visibility: ResultVisibility,
    #[serde(default)]
    pub allow_preview: bool,
    #[serde(default)]
    pub questions: Vec<PaperQuestionInput>,
}

fn default_max_attempts() -> u16 {
    1
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PaperValidationError {
    #[error("title must not be empty")]
    EmptyTitle,
    #[error("paper must contain at least one question")]
    EmptyQuestions,
    #[error("question cannot be selected more than once: {0}")]
    DuplicateQuestion(i64),
    #[error("question id must be positive")]
    InvalidQuestionId,
    #[error("question score must be finite and greater than zero")]
    InvalidQuestionScore,
    #[error("max attempts must be greater than zero")]
    InvalidMaxAttempts,
    #[error("duration must be greater than zero")]
    InvalidDuration,
    #[error("practice papers cannot collect candidate information")]
    PracticeCandidateFields,
    #[error("candidate field cannot be selected more than once")]
    DuplicateCandidateField,
}

impl CreatePaperInput {
    pub fn validate(&self) -> Result<(), PaperValidationError> {
        if self.title.trim().is_empty() {
            return Err(PaperValidationError::EmptyTitle);
        }
        if self.questions.is_empty() {
            return Err(PaperValidationError::EmptyQuestions);
        }
        if self.max_attempts == 0 {
            return Err(PaperValidationError::InvalidMaxAttempts);
        }
        if self.duration_seconds.is_some_and(|duration| duration <= 0) {
            return Err(PaperValidationError::InvalidDuration);
        }
        if self.mode == PaperMode::Practice && !self.candidate_fields.is_empty() {
            return Err(PaperValidationError::PracticeCandidateFields);
        }

        let mut question_ids = HashSet::with_capacity(self.questions.len());
        for question in &self.questions {
            if question.question_id <= 0 {
                return Err(PaperValidationError::InvalidQuestionId);
            }
            if !question_ids.insert(question.question_id) {
                return Err(PaperValidationError::DuplicateQuestion(
                    question.question_id,
                ));
            }
            if question
                .score
                .is_none_or(|score| !score.is_finite() || score <= 0.0)
            {
                return Err(PaperValidationError::InvalidQuestionScore);
            }
        }

        let mut candidate_fields = HashSet::with_capacity(self.candidate_fields.len());
        for field in &self.candidate_fields {
            if !candidate_fields.insert(field.key) {
                return Err(PaperValidationError::DuplicateCandidateField);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptStatus {
    InProgress,
    Submitted,
    NeedsReview,
    Graded,
}

impl fmt::Display for AttemptStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InProgress => "in_progress",
            Self::Submitted => "submitted",
            Self::NeedsReview => "needs_review",
            Self::Graded => "graded",
        })
    }
}

impl FromStr for AttemptStatus {
    type Err = InvalidAttemptStatus;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "in_progress" => Ok(Self::InProgress),
            "submitted" => Ok(Self::Submitted),
            "needs_review" => Ok(Self::NeedsReview),
            "graded" => Ok(Self::Graded),
            other => Err(InvalidAttemptStatus(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown attempt status: {0}")]
pub struct InvalidAttemptStatus(String);

#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GradingStatus {
    Pending,
    NeedsReview,
    Graded,
}

impl fmt::Display for GradingStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Pending => "pending",
            Self::NeedsReview => "needs_review",
            Self::Graded => "graded",
        })
    }
}

impl FromStr for GradingStatus {
    type Err = InvalidGradingStatus;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "needs_review" => Ok(Self::NeedsReview),
            "graded" => Ok(Self::Graded),
            other => Err(InvalidGradingStatus(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown grading status: {0}")]
pub struct InvalidGradingStatus(String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperQuestion {
    pub question_id: i64,
    pub revision_id: i64,
    pub position: u16,
    pub score: f64,
    pub question: PublicQuestion,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminPaper {
    pub id: i64,
    pub status: PaperStatus,
    pub title: String,
    pub description: Option<String>,
    pub audience: Option<String>,
    pub mode: PaperMode,
    pub open_at: Option<String>,
    pub close_at: Option<String>,
    pub duration_seconds: Option<i64>,
    pub max_attempts: u16,
    pub allow_resume: bool,
    pub auto_save: bool,
    pub auto_submit: bool,
    pub candidate_fields: Vec<CandidateFieldConfig>,
    pub result_visibility: ResultVisibility,
    pub allow_preview: bool,
    pub items: Vec<PaperQuestion>,
    pub total_score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublishedPaper {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub audience: Option<String>,
    pub mode: PaperMode,
    pub runtime_status: PaperRuntimeStatus,
    pub open_at: Option<String>,
    pub close_at: Option<String>,
    pub duration_seconds: Option<i64>,
    pub max_attempts: u16,
    pub allow_resume: bool,
    pub auto_save: bool,
    pub auto_submit: bool,
    pub candidate_fields: Vec<CandidateFieldConfig>,
    pub result_visibility: ResultVisibility,
    pub allow_preview: bool,
    pub question_count: u16,
    pub total_score: f64,
    pub current_attempt_id: Option<i64>,
    pub current_attempt_status: Option<AttemptStatus>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExamQuestion {
    pub question_id: i64,
    pub revision_id: i64,
    pub position: u16,
    pub score: f64,
    pub question_type: QuestionType,
    pub stem: String,
    pub blank_count: u16,
    pub options: Vec<QuestionOption>,
    pub saved_answer: Option<AnswerPayload>,
    pub grading_status: GradingStatus,
    pub awarded_score: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExamAttempt {
    pub id: i64,
    pub paper_id: i64,
    pub title: String,
    pub status: AttemptStatus,
    pub started_at: String,
    pub deadline_at: Option<String>,
    pub auto_submit: bool,
    pub submitted_at: Option<String>,
    pub candidate_info: CandidateInfo,
    pub max_score: f64,
    pub total_score: Option<f64>,
    pub unanswered_count: u16,
    pub questions: Vec<ExamQuestion>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExamResultItem {
    pub question_id: i64,
    pub position: u16,
    pub stem: String,
    pub question_type: QuestionType,
    pub max_score: f64,
    pub answer: Option<AnswerPayload>,
    pub awarded_score: Option<f64>,
    pub answered: bool,
    pub status: Option<EvaluationStatus>,
    pub grading_status: GradingStatus,
    pub correct_answer: Option<CorrectAnswer>,
    pub explanation: Option<String>,
    pub feedback: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExamResult {
    pub attempt_id: i64,
    pub paper_id: i64,
    pub title: String,
    pub status: AttemptStatus,
    pub submitted_at: Option<String>,
    pub max_score: f64,
    pub total_score: Option<f64>,
    pub items: Vec<ExamResultItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminAttemptSummary {
    pub id: i64,
    pub paper_id: i64,
    pub title: String,
    pub status: AttemptStatus,
    pub submitted_at: Option<String>,
    pub candidate_info: CandidateInfo,
    pub max_score: f64,
    pub total_score: Option<f64>,
    pub question_count: u16,
    pub answered_count: u16,
    pub needs_review_count: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminAttemptQuestion {
    pub question_id: i64,
    pub position: u16,
    pub stem: String,
    pub question_type: QuestionType,
    pub max_score: f64,
    pub answer: Option<AnswerPayload>,
    pub correct_answer: CorrectAnswer,
    pub awarded_score: Option<f64>,
    pub grading_status: GradingStatus,
    pub explanation: Option<String>,
    pub feedback: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminAttempt {
    pub id: i64,
    pub user_id: i64,
    pub paper_id: i64,
    pub title: String,
    pub status: AttemptStatus,
    pub started_at: String,
    pub deadline_at: Option<String>,
    pub submitted_at: Option<String>,
    pub candidate_info: CandidateInfo,
    pub max_score: f64,
    pub total_score: Option<f64>,
    pub questions: Vec<AdminAttemptQuestion>,
}
