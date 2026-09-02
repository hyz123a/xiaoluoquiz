use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fmt, str::FromStr};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionType {
    SingleChoice,
    MultipleChoice,
    FillBlank,
    TrueFalse,
    ShortAnswer,
}

impl fmt::Display for QuestionType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SingleChoice => "single_choice",
            Self::MultipleChoice => "multiple_choice",
            Self::FillBlank => "fill_blank",
            Self::TrueFalse => "true_false",
            Self::ShortAnswer => "short_answer",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown question type: {0}")]
pub struct InvalidQuestionType(String);

impl FromStr for QuestionType {
    type Err = InvalidQuestionType;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "single_choice" => Ok(Self::SingleChoice),
            "multiple_choice" => Ok(Self::MultipleChoice),
            "fill_blank" => Ok(Self::FillBlank),
            "true_false" => Ok(Self::TrueFalse),
            "short_answer" => Ok(Self::ShortAnswer),
            other => Err(InvalidQuestionType(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionStatus {
    Draft,
    Published,
    Archived,
}

impl fmt::Display for QuestionStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Draft => "draft",
            Self::Published => "published",
            Self::Archived => "archived",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown question status: {0}")]
pub struct InvalidQuestionStatus(String);

impl FromStr for QuestionStatus {
    type Err = InvalidQuestionStatus;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "draft" => Ok(Self::Draft),
            "published" => Ok(Self::Published),
            "archived" => Ok(Self::Archived),
            other => Err(InvalidQuestionStatus(other.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuestionBank {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub question_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuestionBankInput {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum QuestionBankValidationError {
    #[error("question bank name must not be empty")]
    EmptyName,
}

impl QuestionBankInput {
    pub fn validate(&self) -> Result<(), QuestionBankValidationError> {
        if self.name.trim().is_empty() {
            return Err(QuestionBankValidationError::EmptyName);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnswerPayload {
    SingleChoice { option_key: String },
    MultipleChoice { option_keys: Vec<String> },
    FillBlank { values: Vec<String> },
    TrueFalse { value: bool },
    ShortAnswer { text: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CorrectAnswer {
    SingleChoice {
        option_key: String,
    },
    MultipleChoice {
        option_keys: Vec<String>,
    },
    FillBlank {
        accepted: Vec<Vec<String>>,
    },
    TrueFalse {
        value: bool,
    },
    ShortAnswer {
        reference: String,
        #[serde(default)]
        rubric: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationStatus {
    Correct,
    Incorrect,
    NeedsReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Evaluation {
    pub status: EvaluationStatus,
    pub score: Option<f64>,
    pub correct: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuestionOption {
    pub key: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublicQuestion {
    pub id: i64,
    pub revision_id: i64,
    pub question_bank_id: i64,
    pub question_bank_name: String,
    pub question_type: QuestionType,
    pub stem: String,
    pub blank_count: u16,
    pub options: Vec<QuestionOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminQuestionInput {
    pub question_bank_id: i64,
    pub question_type: QuestionType,
    pub stem: String,
    #[serde(default)]
    pub blank_count: u16,
    #[serde(default)]
    pub options: Vec<QuestionOption>,
    pub explanation: Option<String>,
    pub correct_answer: CorrectAnswer,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuestionImportBatch {
    pub items: Vec<AdminQuestionInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdminQuestion {
    pub id: i64,
    pub revision_id: i64,
    pub question_bank_id: i64,
    pub question_bank_name: String,
    pub status: QuestionStatus,
    pub question_type: QuestionType,
    pub stem: String,
    pub blank_count: u16,
    pub options: Vec<QuestionOption>,
    pub explanation: Option<String>,
    pub correct_answer: CorrectAnswer,
}

impl AdminQuestion {
    pub fn as_public(&self) -> PublicQuestion {
        PublicQuestion {
            id: self.id,
            revision_id: self.revision_id,
            question_bank_id: self.question_bank_id,
            question_bank_name: self.question_bank_name.clone(),
            question_type: self.question_type,
            stem: self.stem.clone(),
            blank_count: self.blank_count,
            options: self.options.clone(),
        }
    }

    pub fn as_input(&self) -> AdminQuestionInput {
        AdminQuestionInput {
            question_bank_id: self.question_bank_id,
            question_type: self.question_type,
            stem: self.stem.clone(),
            blank_count: self.blank_count,
            options: self.options.clone(),
            explanation: self.explanation.clone(),
            correct_answer: self.correct_answer.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoringQuestion {
    pub public: PublicQuestion,
    pub explanation: Option<String>,
    pub correct_answer: CorrectAnswer,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum QuestionValidationError {
    #[error("question bank id must be positive")]
    InvalidQuestionBankId,
    #[error("stem must not be empty")]
    EmptyStem,
    #[error("choice question requires at least two unique options")]
    InvalidOptions,
    #[error("correct option must be one of the options")]
    CorrectOptionMissing,
    #[error("multiple-choice question requires at least two correct options")]
    MultipleCorrectOptionsMissing,
    #[error("multiple-choice correct options must be unique")]
    DuplicateCorrectOption,
    #[error("fill-blank question must have at least one blank")]
    FillBlankCountMissing,
    #[error("fill-blank answer count must match blank_count")]
    FillBlankCountMismatch,
    #[error("each blank must have an accepted answer")]
    EmptyAcceptedAnswer,
    #[error("answer type does not match question type")]
    AnswerTypeMismatch,
    #[error("short-answer reference must not be empty")]
    EmptyReference,
}

impl AdminQuestionInput {
    pub fn validate(&self) -> Result<(), QuestionValidationError> {
        if self.question_bank_id <= 0 {
            return Err(QuestionValidationError::InvalidQuestionBankId);
        }
        if self.stem.trim().is_empty() {
            return Err(QuestionValidationError::EmptyStem);
        }

        match (&self.question_type, &self.correct_answer) {
            (QuestionType::SingleChoice, CorrectAnswer::SingleChoice { option_key }) => {
                if self.options.len() < 2 {
                    return Err(QuestionValidationError::InvalidOptions);
                }
                let mut option_keys = HashSet::with_capacity(self.options.len());
                if self.options.iter().any(|option| {
                    option.key.trim().is_empty()
                        || option.text.trim().is_empty()
                        || !option_keys.insert(&option.key)
                }) {
                    return Err(QuestionValidationError::InvalidOptions);
                }
                if !option_keys.contains(option_key) {
                    return Err(QuestionValidationError::CorrectOptionMissing);
                }
            }
            (QuestionType::MultipleChoice, CorrectAnswer::MultipleChoice { option_keys }) => {
                if self.options.len() < 2 {
                    return Err(QuestionValidationError::InvalidOptions);
                }
                let mut available_options = HashSet::with_capacity(self.options.len());
                if self.options.iter().any(|option| {
                    option.key.trim().is_empty()
                        || option.text.trim().is_empty()
                        || !available_options.insert(&option.key)
                }) {
                    return Err(QuestionValidationError::InvalidOptions);
                }
                if option_keys.len() < 2 {
                    return Err(QuestionValidationError::MultipleCorrectOptionsMissing);
                }
                let mut unique_correct_options = HashSet::with_capacity(option_keys.len());
                if option_keys
                    .iter()
                    .any(|option_key| !unique_correct_options.insert(option_key))
                {
                    return Err(QuestionValidationError::DuplicateCorrectOption);
                }
                if option_keys
                    .iter()
                    .any(|option_key| !available_options.contains(option_key))
                {
                    return Err(QuestionValidationError::CorrectOptionMissing);
                }
            }
            (QuestionType::FillBlank, CorrectAnswer::FillBlank { accepted }) => {
                if self.blank_count == 0 {
                    return Err(QuestionValidationError::FillBlankCountMissing);
                }
                if accepted.len() != self.blank_count as usize {
                    return Err(QuestionValidationError::FillBlankCountMismatch);
                }
                if accepted.iter().any(|candidates| {
                    candidates
                        .iter()
                        .all(|candidate| candidate.trim().is_empty())
                }) {
                    return Err(QuestionValidationError::EmptyAcceptedAnswer);
                }
            }
            (QuestionType::TrueFalse, CorrectAnswer::TrueFalse { .. }) => {}
            (QuestionType::ShortAnswer, CorrectAnswer::ShortAnswer { reference, .. }) => {
                if reference.trim().is_empty() {
                    return Err(QuestionValidationError::EmptyReference);
                }
            }
            _ => return Err(QuestionValidationError::AnswerTypeMismatch),
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EvaluationError {
    #[error("answer type does not match question type")]
    AnswerTypeMismatch,
    #[error("fill blank answer count does not match question")]
    FillBlankCountMismatch,
}

pub fn evaluate_answer(
    submitted: &AnswerPayload,
    correct: &CorrectAnswer,
) -> Result<Evaluation, EvaluationError> {
    match (submitted, correct) {
        (
            AnswerPayload::SingleChoice {
                option_key: submitted,
            },
            CorrectAnswer::SingleChoice {
                option_key: correct,
            },
        ) => Ok(binary_evaluation(submitted == correct)),
        (
            AnswerPayload::MultipleChoice {
                option_keys: submitted,
            },
            CorrectAnswer::MultipleChoice {
                option_keys: correct,
            },
        ) => {
            let submitted = submitted.iter().collect::<HashSet<_>>();
            let correct = correct.iter().collect::<HashSet<_>>();
            Ok(binary_evaluation(submitted == correct))
        }
        (AnswerPayload::FillBlank { values: submitted }, CorrectAnswer::FillBlank { accepted }) => {
            if submitted.len() != accepted.len() {
                return Err(EvaluationError::FillBlankCountMismatch);
            }

            let correct = submitted.iter().zip(accepted).all(|(value, accepted)| {
                let normalized = normalize_text(value);
                accepted
                    .iter()
                    .map(|candidate| normalize_text(candidate))
                    .any(|candidate| candidate == normalized)
            });
            Ok(binary_evaluation(correct))
        }
        (
            AnswerPayload::TrueFalse { value: submitted },
            CorrectAnswer::TrueFalse { value: correct },
        ) => Ok(binary_evaluation(submitted == correct)),
        (AnswerPayload::ShortAnswer { .. }, CorrectAnswer::ShortAnswer { .. }) => Ok(Evaluation {
            status: EvaluationStatus::NeedsReview,
            score: None,
            correct: None,
        }),
        _ => Err(EvaluationError::AnswerTypeMismatch),
    }
}

fn binary_evaluation(correct: bool) -> Evaluation {
    Evaluation {
        status: if correct {
            EvaluationStatus::Correct
        } else {
            EvaluationStatus::Incorrect
        },
        score: Some(if correct { 1.0 } else { 0.0 }),
        correct: Some(correct),
    }
}

fn normalize_text(value: &str) -> String {
    value.trim().to_lowercase()
}
