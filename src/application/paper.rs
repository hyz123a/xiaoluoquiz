use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use crate::domain::{
    AdminAttempt, AdminAttemptQuestion, AdminAttemptSummary, AdminPaper, AnswerPayload,
    AttemptStatus, CandidateField, CandidateInfo, CreatePaperInput, EvaluationStatus, ExamAttempt,
    ExamResult, ExamResultItem, GradingStatus, PaperQuestion, PaperStatus, PaperValidationError,
    PublishedPaper, QuestionType, ResultVisibility, evaluate_answer,
};

use super::{QuestionStore, StoreError};

#[derive(Debug, Clone)]
pub struct PaperQuestionSnapshot {
    pub question_id: i64,
    pub revision_id: i64,
    pub position: u16,
    pub score: f64,
    pub question: crate::domain::PublicQuestion,
}

impl PaperQuestionSnapshot {
    pub fn as_public(&self) -> PaperQuestion {
        PaperQuestion {
            question_id: self.question_id,
            revision_id: self.revision_id,
            position: self.position,
            score: self.score,
            question: self.question.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoredAttemptQuestion {
    pub question_id: i64,
    pub revision_id: i64,
    pub position: u16,
    pub score: f64,
    pub question_type: QuestionType,
    pub stem: String,
    pub blank_count: u16,
    pub options: Vec<crate::domain::QuestionOption>,
    pub correct_answer: crate::domain::CorrectAnswer,
    pub explanation: Option<String>,
    pub answer: Option<AnswerPayload>,
    pub grading_status: GradingStatus,
    pub awarded_score: Option<f64>,
    pub feedback: Option<String>,
}

impl StoredAttemptQuestion {
    fn as_exam_question(&self) -> crate::domain::ExamQuestion {
        crate::domain::ExamQuestion {
            question_id: self.question_id,
            revision_id: self.revision_id,
            position: self.position,
            score: self.score,
            question_type: self.question_type,
            stem: self.stem.clone(),
            blank_count: self.blank_count,
            options: self.options.clone(),
            saved_answer: self.answer.clone(),
            grading_status: self.grading_status,
            awarded_score: self.awarded_score,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoredAttempt {
    pub id: i64,
    pub user_id: i64,
    pub paper_id: i64,
    pub title: String,
    pub status: AttemptStatus,
    pub started_at: String,
    pub deadline_at: Option<String>,
    pub auto_submit: bool,
    pub deadline_reached: bool,
    pub submitted_at: Option<String>,
    pub candidate_info: CandidateInfo,
    pub max_score: f64,
    pub total_score: Option<f64>,
    pub result_visibility: ResultVisibility,
    pub questions: Vec<StoredAttemptQuestion>,
}

impl StoredAttempt {
    pub fn as_exam_attempt(&self) -> ExamAttempt {
        let unanswered_count = self
            .questions
            .iter()
            .filter(|question| question.answer.is_none())
            .count() as u16;
        ExamAttempt {
            id: self.id,
            paper_id: self.paper_id,
            title: self.title.clone(),
            status: self.status,
            started_at: self.started_at.clone(),
            deadline_at: self.deadline_at.clone(),
            auto_submit: self.auto_submit,
            submitted_at: self.submitted_at.clone(),
            candidate_info: self.candidate_info.clone(),
            max_score: self.max_score,
            total_score: self.total_score,
            unanswered_count,
            questions: self
                .questions
                .iter()
                .map(StoredAttemptQuestion::as_exam_question)
                .collect(),
        }
    }

    pub fn as_admin_summary(&self) -> AdminAttemptSummary {
        AdminAttemptSummary {
            id: self.id,
            paper_id: self.paper_id,
            title: self.title.clone(),
            status: self.status,
            submitted_at: self.submitted_at.clone(),
            candidate_info: self.candidate_info.clone(),
            max_score: self.max_score,
            total_score: self.total_score,
            question_count: self.questions.len() as u16,
            answered_count: self
                .questions
                .iter()
                .filter(|question| question.answer.is_some())
                .count() as u16,
            needs_review_count: self
                .questions
                .iter()
                .filter(|question| question.grading_status == GradingStatus::NeedsReview)
                .count() as u16,
        }
    }

    pub fn as_admin_attempt(&self) -> AdminAttempt {
        AdminAttempt {
            id: self.id,
            user_id: self.user_id,
            paper_id: self.paper_id,
            title: self.title.clone(),
            status: self.status,
            started_at: self.started_at.clone(),
            deadline_at: self.deadline_at.clone(),
            submitted_at: self.submitted_at.clone(),
            candidate_info: self.candidate_info.clone(),
            max_score: self.max_score,
            total_score: self.total_score,
            questions: self
                .questions
                .iter()
                .map(|question| AdminAttemptQuestion {
                    question_id: question.question_id,
                    position: question.position,
                    stem: question.stem.clone(),
                    question_type: question.question_type,
                    max_score: question.score,
                    answer: question.answer.clone(),
                    correct_answer: question.correct_answer.clone(),
                    awarded_score: question.awarded_score,
                    grading_status: question.grading_status,
                    explanation: question.explanation.clone(),
                    feedback: question.feedback.clone(),
                })
                .collect(),
        }
    }
}
#[derive(Debug, Clone)]
pub struct GradedAnswer {
    pub question_id: i64,
    pub grading_status: GradingStatus,
    pub awarded_score: Option<f64>,
}

#[derive(Debug, Error)]
pub enum PaperStoreError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("paper was not found")]
    PaperNotFound,
    #[error("selected question is not published")]
    QuestionNotPublished,
    #[error("paper is not available")]
    PaperUnavailable,
    #[error("maximum attempts reached")]
    MaxAttemptsReached,
    #[error("attempt was not found")]
    AttemptNotFound,
    #[error("attempt is already submitted")]
    AttemptClosed,
    #[error("question does not belong to this attempt")]
    QuestionNotInAttempt,
    #[error("answer type does not match question type")]
    InvalidAnswer,
    #[error("attempt has not been submitted")]
    AttemptNotSubmitted,
    #[error("answer has not been saved")]
    AnswerNotSaved,
    #[error("grade is invalid")]
    InvalidGrade,
    #[error("result is not available")]
    ResultUnavailable,
}

#[async_trait]
pub trait PaperStore: Send + Sync {
    async fn list_admin_papers(&self) -> Result<Vec<AdminPaper>, PaperStoreError>;
    async fn get_admin_paper(&self, id: i64) -> Result<Option<AdminPaper>, PaperStoreError>;
    async fn create_paper_draft(
        &self,
        actor_user_id: i64,
        input: &CreatePaperInput,
        items: &[PaperQuestionSnapshot],
    ) -> Result<AdminPaper, PaperStoreError>;
    async fn publish_paper(
        &self,
        actor_user_id: i64,
        id: i64,
    ) -> Result<Option<AdminPaper>, PaperStoreError>;
    async fn archive_paper(
        &self,
        actor_user_id: i64,
        id: i64,
    ) -> Result<Option<AdminPaper>, PaperStoreError>;

    async fn list_published_papers(
        &self,
        user_id: i64,
    ) -> Result<Vec<PublishedPaper>, PaperStoreError>;
    async fn get_published_paper(
        &self,
        user_id: i64,
        id: i64,
    ) -> Result<Option<PublishedPaper>, PaperStoreError>;
    async fn create_attempt(
        &self,
        user_id: i64,
        paper_id: i64,
        candidate_info: CandidateInfo,
    ) -> Result<StoredAttempt, PaperStoreError>;
    async fn get_attempt(
        &self,
        user_id: i64,
        attempt_id: i64,
    ) -> Result<Option<StoredAttempt>, PaperStoreError>;
    async fn list_admin_attempts(&self) -> Result<Vec<StoredAttempt>, PaperStoreError>;
    async fn get_admin_attempt(
        &self,
        attempt_id: i64,
    ) -> Result<Option<StoredAttempt>, PaperStoreError>;
    async fn grade_answer(
        &self,
        actor_user_id: i64,
        attempt_id: i64,
        question_id: i64,
        score: f64,
        feedback: Option<String>,
    ) -> Result<StoredAttempt, PaperStoreError>;
    async fn save_answer(
        &self,
        user_id: i64,
        attempt_id: i64,
        question_id: i64,
        answer: AnswerPayload,
    ) -> Result<StoredAttempt, PaperStoreError>;
    async fn submit_attempt(
        &self,
        user_id: i64,
        attempt_id: i64,
        evaluations: &[GradedAnswer],
    ) -> Result<StoredAttempt, PaperStoreError>;
}

#[derive(Debug, Error)]
pub enum PaperManagementError {
    #[error("paper was not found")]
    NotFound,
    #[error(transparent)]
    InvalidInput(#[from] PaperValidationError),
    #[error("selected question is not published")]
    QuestionNotPublished,
    #[error("selected question version is no longer current")]
    QuestionRevisionChanged,
    #[error(transparent)]
    QuestionStore(#[from] StoreError),
    #[error("paper cannot transition from {0} to the requested state")]
    InvalidState(PaperStatus),
    #[error(transparent)]
    Store(#[from] PaperStoreError),
}

#[derive(Clone)]
pub struct PaperManagementService {
    questions: Arc<dyn QuestionStore>,
    papers: Arc<dyn PaperStore>,
}

impl PaperManagementService {
    pub fn new(questions: Arc<dyn QuestionStore>, papers: Arc<dyn PaperStore>) -> Self {
        Self { questions, papers }
    }

    pub async fn list(&self) -> Result<Vec<AdminPaper>, PaperManagementError> {
        Ok(self.papers.list_admin_papers().await?)
    }

    pub async fn create_draft(
        &self,
        actor_user_id: i64,
        input: CreatePaperInput,
    ) -> Result<AdminPaper, PaperManagementError> {
        input.validate()?;
        let mut items = Vec::with_capacity(input.questions.len());
        for (position, selected) in input.questions.iter().enumerate() {
            let Some(question) = self.questions.get_published(selected.question_id).await? else {
                return Err(PaperManagementError::QuestionNotPublished);
            };
            let score = selected.score.ok_or(PaperManagementError::InvalidInput(
                PaperValidationError::InvalidQuestionScore,
            ))?;
            let position = u16::try_from(position).map_err(|_| {
                PaperManagementError::InvalidInput(PaperValidationError::InvalidQuestionId)
            })?;
            let revision_id = question.revision_id;
            items.push(PaperQuestionSnapshot {
                question_id: selected.question_id,
                revision_id,
                position,
                score,
                question,
            });
        }
        Ok(self
            .papers
            .create_paper_draft(actor_user_id, &input, &items)
            .await?)
    }

    pub async fn publish(
        &self,
        actor_user_id: i64,
        id: i64,
    ) -> Result<AdminPaper, PaperManagementError> {
        let paper = self
            .papers
            .get_admin_paper(id)
            .await?
            .ok_or(PaperManagementError::NotFound)?;
        if paper.status != PaperStatus::Draft {
            return Err(PaperManagementError::InvalidState(paper.status));
        }
        for item in &paper.items {
            let Some(question) = self.questions.get_published(item.question_id).await? else {
                return Err(PaperManagementError::QuestionNotPublished);
            };
            if question.revision_id != item.revision_id {
                return Err(PaperManagementError::QuestionRevisionChanged);
            }
        }
        self.papers
            .publish_paper(actor_user_id, id)
            .await?
            .ok_or(PaperManagementError::NotFound)
    }

    pub async fn archive(
        &self,
        actor_user_id: i64,
        id: i64,
    ) -> Result<AdminPaper, PaperManagementError> {
        let paper = self
            .papers
            .get_admin_paper(id)
            .await?
            .ok_or(PaperManagementError::NotFound)?;
        if paper.status == PaperStatus::Archived {
            return Ok(paper);
        }
        self.papers
            .archive_paper(actor_user_id, id)
            .await?
            .ok_or(PaperManagementError::NotFound)
    }
}

#[derive(Debug, Error)]
pub enum ExamError {
    #[error("paper was not found")]
    PaperNotFound,
    #[error("paper is not available")]
    PaperUnavailable,
    #[error("maximum attempts reached")]
    MaxAttemptsReached,
    #[error("attempt was not found")]
    AttemptNotFound,
    #[error("attempt is already submitted")]
    AttemptClosed,
    #[error("question does not belong to this attempt")]
    QuestionNotInAttempt,
    #[error("answer type does not match question type")]
    InvalidAnswer,
    #[error("attempt has not been submitted")]
    AttemptNotSubmitted,
    #[error("answer has not been saved")]
    AnswerNotSaved,
    #[error("grade is invalid")]
    InvalidGrade,
    #[error("candidate field is required: {0}")]
    RequiredCandidateField(String),
    #[error("result is not available")]
    ResultUnavailable,
    #[error(transparent)]
    Store(#[from] PaperStoreError),
}

#[derive(Clone)]
pub struct ExamService {
    papers: Arc<dyn PaperStore>,
}

impl ExamService {
    pub fn new(papers: Arc<dyn PaperStore>) -> Self {
        Self { papers }
    }

    async fn load_attempt(
        &self,
        user_id: i64,
        attempt_id: i64,
    ) -> Result<StoredAttempt, ExamError> {
        let attempt = self
            .papers
            .get_attempt(user_id, attempt_id)
            .await
            .map_err(map_paper_store_error)?
            .ok_or(ExamError::AttemptNotFound)?;
        if attempt.status == AttemptStatus::InProgress
            && attempt.deadline_reached
            && attempt.auto_submit
        {
            let evaluations = grade_attempt(&attempt);
            return self
                .papers
                .submit_attempt(user_id, attempt_id, &evaluations)
                .await
                .map_err(map_paper_store_error);
        }
        Ok(attempt)
    }

    pub async fn list_papers(&self, user_id: i64) -> Result<Vec<PublishedPaper>, ExamError> {
        self.papers
            .list_published_papers(user_id)
            .await
            .map_err(map_paper_store_error)
    }

    pub async fn get_paper(
        &self,
        user_id: i64,
        paper_id: i64,
    ) -> Result<PublishedPaper, ExamError> {
        self.papers
            .get_published_paper(user_id, paper_id)
            .await
            .map_err(map_paper_store_error)?
            .ok_or(ExamError::PaperNotFound)
    }

    pub async fn start(
        &self,
        user_id: i64,
        paper_id: i64,
        candidate_info: CandidateInfo,
    ) -> Result<ExamAttempt, ExamError> {
        let paper = self.get_paper(user_id, paper_id).await?;
        validate_candidate_info(&paper, &candidate_info)?;
        if let (Some(attempt_id), Some(AttemptStatus::InProgress)) =
            (paper.current_attempt_id, paper.current_attempt_status)
        {
            let _ = self.load_attempt(user_id, attempt_id).await?;
        }
        Ok(self
            .papers
            .create_attempt(user_id, paper_id, candidate_info)
            .await
            .map_err(map_paper_store_error)?
            .as_exam_attempt())
    }

    pub async fn get_attempt(
        &self,
        user_id: i64,
        attempt_id: i64,
    ) -> Result<ExamAttempt, ExamError> {
        Ok(self
            .load_attempt(user_id, attempt_id)
            .await?
            .as_exam_attempt())
    }

    pub async fn save_answer(
        &self,
        user_id: i64,
        attempt_id: i64,
        question_id: i64,
        answer: AnswerPayload,
    ) -> Result<ExamAttempt, ExamError> {
        let attempt = self.load_attempt(user_id, attempt_id).await?;
        if attempt.status != AttemptStatus::InProgress || attempt.deadline_reached {
            return Err(ExamError::AttemptClosed);
        }
        let Some(question) = attempt
            .questions
            .iter()
            .find(|question| question.question_id == question_id)
        else {
            return Err(ExamError::QuestionNotInAttempt);
        };
        evaluate_answer(&answer, &question.correct_answer).map_err(|_| ExamError::InvalidAnswer)?;
        Ok(self
            .papers
            .save_answer(user_id, attempt_id, question_id, answer)
            .await
            .map_err(map_paper_store_error)?
            .as_exam_attempt())
    }

    pub async fn submit(&self, user_id: i64, attempt_id: i64) -> Result<ExamResult, ExamError> {
        let attempt = self.load_attempt(user_id, attempt_id).await?;
        if attempt.status != AttemptStatus::InProgress {
            return self.result_from_stored(&attempt);
        }

        let evaluations = grade_attempt(&attempt);
        let submitted = self
            .papers
            .submit_attempt(user_id, attempt_id, &evaluations)
            .await
            .map_err(map_paper_store_error)?;
        self.result_from_stored(&submitted)
    }

    pub async fn result(&self, user_id: i64, attempt_id: i64) -> Result<ExamResult, ExamError> {
        let attempt = self.load_attempt(user_id, attempt_id).await?;
        self.result_from_stored(&attempt)
    }

    pub async fn list_admin_attempts(&self) -> Result<Vec<AdminAttemptSummary>, ExamError> {
        Ok(self
            .papers
            .list_admin_attempts()
            .await
            .map_err(map_paper_store_error)?
            .into_iter()
            .map(|attempt| attempt.as_admin_summary())
            .collect())
    }

    pub async fn get_admin_attempt(&self, attempt_id: i64) -> Result<AdminAttempt, ExamError> {
        self.papers
            .get_admin_attempt(attempt_id)
            .await
            .map_err(map_paper_store_error)?
            .map(|attempt| attempt.as_admin_attempt())
            .ok_or(ExamError::AttemptNotFound)
    }

    pub async fn grade_admin_answer(
        &self,
        actor_user_id: i64,
        attempt_id: i64,
        question_id: i64,
        score: f64,
        feedback: Option<String>,
    ) -> Result<AdminAttempt, ExamError> {
        let attempt = self
            .papers
            .get_admin_attempt(attempt_id)
            .await
            .map_err(map_paper_store_error)?
            .ok_or(ExamError::AttemptNotFound)?;
        if attempt.status == AttemptStatus::InProgress {
            return Err(ExamError::AttemptNotSubmitted);
        }
        let question = attempt
            .questions
            .iter()
            .find(|question| question.question_id == question_id)
            .ok_or(ExamError::QuestionNotInAttempt)?;
        if question.answer.is_none() {
            return Err(ExamError::AnswerNotSaved);
        }
        if !score.is_finite() || !(0.0..=question.score).contains(&score) {
            return Err(ExamError::InvalidGrade);
        }
        let feedback = feedback
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        Ok(self
            .papers
            .grade_answer(
                actor_user_id,
                attempt_id,
                question_id,
                round_score(score),
                feedback,
            )
            .await
            .map_err(map_paper_store_error)?
            .as_admin_attempt())
    }

    fn result_from_stored(&self, attempt: &StoredAttempt) -> Result<ExamResult, ExamError> {
        if attempt.status == AttemptStatus::InProgress {
            return Err(ExamError::ResultUnavailable);
        }
        if matches!(attempt.result_visibility, ResultVisibility::AdminRelease)
            || (attempt.result_visibility == ResultVisibility::AfterGrading
                && attempt.status == AttemptStatus::NeedsReview)
        {
            return Err(ExamError::ResultUnavailable);
        }

        let items = attempt
            .questions
            .iter()
            .map(|question| result_item(question, attempt.result_visibility))
            .collect();
        Ok(ExamResult {
            attempt_id: attempt.id,
            paper_id: attempt.paper_id,
            title: attempt.title.clone(),
            status: attempt.status,
            submitted_at: attempt.submitted_at.clone(),
            max_score: attempt.max_score,
            total_score: attempt.total_score,
            items,
        })
    }
}

fn map_paper_store_error(error: PaperStoreError) -> ExamError {
    match error {
        PaperStoreError::PaperNotFound => ExamError::PaperNotFound,
        PaperStoreError::QuestionNotPublished => ExamError::PaperUnavailable,
        PaperStoreError::PaperUnavailable => ExamError::PaperUnavailable,
        PaperStoreError::MaxAttemptsReached => ExamError::MaxAttemptsReached,
        PaperStoreError::AttemptNotFound => ExamError::AttemptNotFound,
        PaperStoreError::AttemptClosed => ExamError::AttemptClosed,
        PaperStoreError::QuestionNotInAttempt => ExamError::QuestionNotInAttempt,
        PaperStoreError::InvalidAnswer => ExamError::InvalidAnswer,
        PaperStoreError::AttemptNotSubmitted => ExamError::AttemptNotSubmitted,
        PaperStoreError::AnswerNotSaved => ExamError::AnswerNotSaved,
        PaperStoreError::InvalidGrade => ExamError::InvalidGrade,
        PaperStoreError::ResultUnavailable => ExamError::ResultUnavailable,
        PaperStoreError::Store(error) => ExamError::Store(PaperStoreError::Store(error)),
    }
}

fn validate_candidate_info(
    paper: &PublishedPaper,
    candidate_info: &CandidateInfo,
) -> Result<(), ExamError> {
    for field in &paper.candidate_fields {
        let value = match field.key {
            CandidateField::StudentNumber => candidate_info.student_number.as_deref(),
            CandidateField::Name => candidate_info.name.as_deref(),
        };
        if field.required && value.is_none_or(|value| value.trim().is_empty()) {
            return Err(ExamError::RequiredCandidateField(field.key.to_string()));
        }
    }
    Ok(())
}

fn grade_attempt(attempt: &StoredAttempt) -> Vec<GradedAnswer> {
    attempt.questions.iter().map(grade_question).collect()
}

fn grade_question(question: &StoredAttemptQuestion) -> GradedAnswer {
    let Some(answer) = question.answer.as_ref() else {
        return match question.correct_answer {
            crate::domain::CorrectAnswer::ShortAnswer { .. } => GradedAnswer {
                question_id: question.question_id,
                grading_status: GradingStatus::NeedsReview,
                awarded_score: None,
            },
            _ => GradedAnswer {
                question_id: question.question_id,
                grading_status: GradingStatus::Graded,
                awarded_score: Some(0.0),
            },
        };
    };

    match evaluate_answer(answer, &question.correct_answer) {
        Ok(evaluation) => GradedAnswer {
            question_id: question.question_id,
            grading_status: if evaluation.status == EvaluationStatus::NeedsReview {
                GradingStatus::NeedsReview
            } else {
                GradingStatus::Graded
            },
            awarded_score: evaluation
                .score
                .map(|ratio| round_score(ratio * question.score)),
        },
        Err(_) => GradedAnswer {
            question_id: question.question_id,
            grading_status: GradingStatus::Graded,
            awarded_score: Some(0.0),
        },
    }
}

fn result_item(question: &StoredAttemptQuestion, visibility: ResultVisibility) -> ExamResultItem {
    let evaluation = question
        .answer
        .as_ref()
        .and_then(|answer| evaluate_answer(answer, &question.correct_answer).ok());
    let (status, awarded_score) = if let Some(evaluation) = evaluation {
        (Some(evaluation.status), question.awarded_score)
    } else if question.answer.is_some() {
        (Some(EvaluationStatus::Incorrect), question.awarded_score)
    } else {
        (None, question.awarded_score.or(Some(0.0)))
    };
    let show_details = visibility == ResultVisibility::AfterSubmit;
    ExamResultItem {
        question_id: question.question_id,
        position: question.position,
        stem: question.stem.clone(),
        question_type: question.question_type,
        max_score: question.score,
        answer: question.answer.clone(),
        awarded_score,
        answered: question.answer.is_some(),
        status,
        grading_status: question.grading_status,
        correct_answer: show_details.then(|| question.correct_answer.clone()),
        explanation: show_details.then(|| question.explanation.clone()).flatten(),
        feedback: question.feedback.clone(),
    }
}

fn round_score(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
