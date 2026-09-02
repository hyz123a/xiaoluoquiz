use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
};

use async_trait::async_trait;
use axum::serve;
use reqwest::{Client, Response};
use serde::{Serialize, de::DeserializeOwned};
use tokio::{net::TcpListener, sync::Mutex, task::JoinHandle};
use xiaoluoquiz::{
    application::{
        AdminQuestionFilters, AuthStore, AuthStoreError, GradedAnswer, PaperQuestionSnapshot,
        PaperStore, PaperStoreError, QuestionImportItem, QuestionImportItemStatus,
        QuestionImportReport, StoredAttempt, StoredAttemptQuestion, StoredUser, hash_password,
    },
    domain::{
        AdminPaper, AdminQuestion, AdminQuestionInput, AnswerPayload, AttemptStatus, CandidateInfo,
        CorrectAnswer, GradingStatus, PaperQuestion, PaperRuntimeStatus, PaperStatus,
        PublicQuestion, QuestionBank, QuestionBankInput, QuestionOption, QuestionStatus,
        QuestionType, ScoringQuestion,
        auth::{
            AccountStatus, ClassGroup, CreateClassInput, CreateUserInput, UserIdentity, UserRole,
        },
    },
    server::{AppState, QuestionStore, StoreError, api_router},
};

pub struct TestApp {
    pub address: String,
    api_client: Client,
    server: JoinHandle<()>,
}

impl TestApp {
    pub async fn get(&self, path: &str) -> Response {
        self.api_client
            .get(format!("{}{}", self.address, path))
            .send()
            .await
            .expect("GET request should succeed")
    }

    pub async fn post_json<T: Serialize>(&self, path: &str, payload: &T) -> Response {
        self.api_client
            .post(format!("{}{}", self.address, path))
            .json(payload)
            .send()
            .await
            .expect("POST request should succeed")
    }

    pub async fn login(&self, username: &str, password: &str) -> Response {
        self.post_json(
            "/api/v1/auth/login",
            &serde_json::json!({ "username": username, "password": password }),
        )
        .await
    }

    pub async fn login_as_admin(&self) {
        let response = self.login("admin-001", "InitialPassword123!").await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
    }

    pub async fn json<T: DeserializeOwned>(&self, response: Response) -> T {
        response
            .json()
            .await
            .expect("response should contain valid JSON")
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        self.server.abort();
    }
}

pub async fn spawn_app() -> TestApp {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test server should bind an ephemeral port");
    let address = format!(
        "http://{}",
        listener.local_addr().expect("address should exist")
    );
    let router = api_router(AppState::new(Arc::new(FakeQuestionStore::new())));
    let server = tokio::spawn(async move {
        serve(listener, router)
            .await
            .expect("test server should stop without an error");
    });

    TestApp {
        address,
        api_client: Client::builder()
            .cookie_store(true)
            .build()
            .expect("test HTTP client should build"),
        server,
    }
}

pub fn sample_question() -> PublicQuestion {
    sample_admin_question().as_public()
}

fn sample_admin_question() -> AdminQuestion {
    AdminQuestion {
        id: 1,
        revision_id: 1,
        question_bank_id: 2,
        question_bank_name: "测试题库".to_owned(),
        status: QuestionStatus::Published,
        question_type: QuestionType::SingleChoice,
        stem: "Rust 的包管理工具是什么？".to_owned(),
        blank_count: 0,
        options: vec![
            QuestionOption {
                key: "A".to_owned(),
                text: "npm".to_owned(),
            },
            QuestionOption {
                key: "B".to_owned(),
                text: "Cargo".to_owned(),
            },
        ],
        explanation: Some("B 是正确答案".to_owned()),
        correct_answer: CorrectAnswer::SingleChoice {
            option_key: "B".to_owned(),
        },
    }
}

struct FakeQuestionStore {
    question_banks: Arc<Mutex<BTreeMap<i64, QuestionBank>>>,
    questions: Arc<Mutex<BTreeMap<i64, AdminQuestion>>>,
    next_bank_id: AtomicI64,
    next_id: AtomicI64,
    papers: Arc<Mutex<BTreeMap<i64, AdminPaper>>>,
    next_paper_id: AtomicI64,
    attempts: Arc<Mutex<BTreeMap<i64, FakeAttemptRecord>>>,
    next_attempt_id: AtomicI64,
    users: Arc<Mutex<BTreeMap<i64, StoredUser>>>,
    classes: Arc<Mutex<BTreeMap<i64, ClassGroup>>>,
    sessions: Arc<Mutex<BTreeMap<String, i64>>>,
    failed_logins: Arc<Mutex<BTreeMap<i64, u8>>>,
    next_user_id: AtomicI64,
    next_class_id: AtomicI64,
}

#[derive(Clone)]
struct FakeAttemptRecord {
    user_id: i64,
    attempt: StoredAttempt,
}

impl FakeQuestionStore {
    fn new() -> Self {
        let initial_question = sample_admin_question();
        let mut questions = BTreeMap::new();
        questions.insert(initial_question.id, initial_question);

        let initial_password_hash =
            hash_password("InitialPassword123!").expect("test initial password should hash");
        let mut users = BTreeMap::new();
        users.insert(
            1,
            StoredUser {
                identity: test_user(1, "admin-001", "测试管理员", UserRole::Admin, false),
                password_hash: Some(initial_password_hash.clone()),
                locked: false,
            },
        );
        users.insert(
            2,
            StoredUser {
                identity: test_user(2, "student-001", "测试学生", UserRole::Student, true),
                password_hash: Some(initial_password_hash),
                locked: false,
            },
        );

        let mut question_banks = BTreeMap::new();
        question_banks.insert(
            1,
            QuestionBank {
                id: 1,
                name: "人工智能导论".to_owned(),
                description: Some("人工智能导论课程题目".to_owned()),
                question_count: 0,
            },
        );
        question_banks.insert(
            2,
            QuestionBank {
                id: 2,
                name: "测试题库".to_owned(),
                description: Some("用于本地演示和系统测试的题目".to_owned()),
                question_count: 1,
            },
        );

        Self {
            question_banks: Arc::new(Mutex::new(question_banks)),
            questions: Arc::new(Mutex::new(questions)),
            next_bank_id: AtomicI64::new(3),
            next_id: AtomicI64::new(2),
            papers: Arc::new(Mutex::new(BTreeMap::new())),
            next_paper_id: AtomicI64::new(1),
            attempts: Arc::new(Mutex::new(BTreeMap::new())),
            next_attempt_id: AtomicI64::new(1),
            users: Arc::new(Mutex::new(users)),
            classes: Arc::new(Mutex::new(BTreeMap::new())),
            sessions: Arc::new(Mutex::new(BTreeMap::new())),
            failed_logins: Arc::new(Mutex::new(BTreeMap::new())),
            next_user_id: AtomicI64::new(3),
            next_class_id: AtomicI64::new(1),
        }
    }
}

fn test_user(
    id: i64,
    username: &str,
    display_name: &str,
    role: UserRole,
    must_change_password: bool,
) -> UserIdentity {
    UserIdentity {
        id,
        username: username.to_owned(),
        display_name: display_name.to_owned(),
        role,
        status: AccountStatus::Active,
        must_change_password,
        student_number: None,
        class_name: None,
        created_at: "2026-09-01T00:00:00.000Z".to_owned(),
        last_login_at: None,
    }
}

#[async_trait]
impl QuestionStore for FakeQuestionStore {
    async fn list_question_banks(&self) -> Result<Vec<QuestionBank>, StoreError> {
        let banks = self.question_banks.lock().await;
        let questions = self.questions.lock().await;
        Ok(banks
            .values()
            .cloned()
            .map(|mut bank| {
                bank.question_count = questions
                    .values()
                    .filter(|question| {
                        question.status == QuestionStatus::Published
                            && question.question_bank_id == bank.id
                    })
                    .count() as u32;
                bank
            })
            .collect())
    }

    async fn get_question_bank(&self, id: i64) -> Result<Option<QuestionBank>, StoreError> {
        Ok(self
            .list_question_banks()
            .await?
            .into_iter()
            .find(|bank| bank.id == id))
    }

    async fn create_question_bank(
        &self,
        input: QuestionBankInput,
    ) -> Result<QuestionBank, StoreError> {
        let id = self.next_bank_id.fetch_add(1, Ordering::Relaxed);
        let bank = QuestionBank {
            id,
            name: input.name.trim().to_owned(),
            description: input
                .description
                .map(|description| description.trim().to_owned())
                .filter(|description| !description.is_empty()),
            question_count: 0,
        };
        self.question_banks.lock().await.insert(id, bank.clone());
        Ok(bank)
    }

    async fn list_published(
        &self,
        bank_id: Option<i64>,
    ) -> Result<Vec<PublicQuestion>, StoreError> {
        let questions = self.questions.lock().await;
        Ok(questions
            .values()
            .filter(|question| {
                question.status == QuestionStatus::Published
                    && bank_id.is_none_or(|bank_id| question.question_bank_id == bank_id)
            })
            .map(AdminQuestion::as_public)
            .collect())
    }

    async fn get_published(&self, id: i64) -> Result<Option<PublicQuestion>, StoreError> {
        let questions = self.questions.lock().await;
        Ok(questions
            .get(&id)
            .filter(|question| question.status == QuestionStatus::Published)
            .map(AdminQuestion::as_public))
    }

    async fn get_for_scoring(&self, id: i64) -> Result<Option<ScoringQuestion>, StoreError> {
        let questions = self.questions.lock().await;
        Ok(questions
            .get(&id)
            .filter(|question| question.status == QuestionStatus::Published)
            .map(|question| ScoringQuestion {
                public: question.as_public(),
                explanation: question.explanation.clone(),
                correct_answer: question.correct_answer.clone(),
            }))
    }

    async fn create_draft(&self, input: AdminQuestionInput) -> Result<AdminQuestion, StoreError> {
        let bank = self
            .get_question_bank(input.question_bank_id)
            .await?
            .ok_or_else(|| StoreError::InvalidData("question bank was not found".to_owned()))?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let question = AdminQuestion {
            id,
            revision_id: id,
            question_bank_id: input.question_bank_id,
            question_bank_name: bank.name,
            status: QuestionStatus::Draft,
            question_type: input.question_type,
            stem: input.stem,
            blank_count: input.blank_count,
            options: input.options,
            explanation: input.explanation,
            correct_answer: input.correct_answer,
        };
        self.questions.lock().await.insert(id, question.clone());
        Ok(question)
    }

    async fn list_admin(
        &self,
        filters: &AdminQuestionFilters,
    ) -> Result<Vec<AdminQuestion>, StoreError> {
        let keyword = filters.keyword.as_deref().map(str::to_lowercase);
        Ok(self
            .questions
            .lock()
            .await
            .values()
            .filter(|question| {
                filters
                    .bank_id
                    .is_none_or(|bank_id| question.question_bank_id == bank_id)
            })
            .filter(|question| {
                filters
                    .question_type
                    .is_none_or(|question_type| question.question_type == question_type)
            })
            .filter(|question| {
                filters
                    .status
                    .is_none_or(|status| question.status == status)
            })
            .filter(|question| {
                keyword.as_ref().is_none_or(|keyword| {
                    question.stem.to_lowercase().contains(keyword)
                        || question.question_bank_name.to_lowercase().contains(keyword)
                })
            })
            .cloned()
            .collect())
    }

    async fn import_published(
        &self,
        inputs: Vec<AdminQuestionInput>,
    ) -> Result<QuestionImportReport, StoreError> {
        let banks = self.question_banks.lock().await;
        let mut questions = self.questions.lock().await;
        let mut staged_questions = questions.clone();
        let mut next_id = self.next_id.load(Ordering::Relaxed);
        let mut report = QuestionImportReport {
            inserted: 0,
            skipped: 0,
            errors: 0,
            items: Vec::with_capacity(inputs.len()),
        };

        for (index, input) in inputs.iter().enumerate() {
            let normalized_stem = input.stem.trim().to_lowercase();
            if let Some((question_id, _)) = staged_questions.iter().find(|(_, question)| {
                question.question_bank_id == input.question_bank_id
                    && question.stem.trim().to_lowercase() == normalized_stem
            }) {
                report.skipped += 1;
                report.items.push(QuestionImportItem {
                    index,
                    status: QuestionImportItemStatus::Skipped,
                    question_id: Some(*question_id),
                    error: None,
                });
                continue;
            }

            let bank = banks
                .get(&input.question_bank_id)
                .ok_or_else(|| StoreError::InvalidData("question bank was not found".to_owned()))?;
            let question_id = next_id;
            next_id += 1;
            staged_questions.insert(
                question_id,
                AdminQuestion {
                    id: question_id,
                    revision_id: question_id,
                    question_bank_id: input.question_bank_id,
                    question_bank_name: bank.name.clone(),
                    status: QuestionStatus::Published,
                    question_type: input.question_type,
                    stem: input.stem.clone(),
                    blank_count: input.blank_count,
                    options: input.options.clone(),
                    explanation: input.explanation.clone(),
                    correct_answer: input.correct_answer.clone(),
                },
            );
            report.inserted += 1;
            report.items.push(QuestionImportItem {
                index,
                status: QuestionImportItemStatus::Inserted,
                question_id: Some(question_id),
                error: None,
            });
        }

        *questions = staged_questions;
        self.next_id.store(next_id, Ordering::Relaxed);
        Ok(report)
    }

    async fn get_admin(&self, id: i64) -> Result<Option<AdminQuestion>, StoreError> {
        Ok(self.questions.lock().await.get(&id).cloned())
    }

    async fn publish(&self, id: i64) -> Result<Option<AdminQuestion>, StoreError> {
        let mut questions = self.questions.lock().await;
        let Some(question) = questions.get_mut(&id) else {
            return Ok(None);
        };
        question.status = QuestionStatus::Published;
        Ok(Some(question.clone()))
    }

    async fn archive(&self, id: i64) -> Result<Option<AdminQuestion>, StoreError> {
        let mut questions = self.questions.lock().await;
        let Some(question) = questions.get_mut(&id) else {
            return Ok(None);
        };
        question.status = QuestionStatus::Archived;
        Ok(Some(question.clone()))
    }
}

fn published_paper_view(
    paper: &AdminPaper,
    current_attempt: Option<&FakeAttemptRecord>,
) -> xiaoluoquiz::domain::PublishedPaper {
    xiaoluoquiz::domain::PublishedPaper {
        id: paper.id,
        title: paper.title.clone(),
        description: paper.description.clone(),
        audience: paper.audience.clone(),
        mode: paper.mode,
        runtime_status: PaperRuntimeStatus::Open,
        open_at: paper.open_at.clone(),
        close_at: paper.close_at.clone(),
        duration_seconds: paper.duration_seconds,
        max_attempts: paper.max_attempts,
        allow_resume: paper.allow_resume,
        auto_save: paper.auto_save,
        auto_submit: paper.auto_submit,
        candidate_fields: paper.candidate_fields.clone(),
        result_visibility: paper.result_visibility,
        allow_preview: paper.allow_preview,
        question_count: paper.items.len() as u16,
        total_score: paper.total_score,
        current_attempt_id: current_attempt.map(|record| record.attempt.id),
        current_attempt_status: current_attempt.map(|record| record.attempt.status),
    }
}

fn stored_attempt_question(
    question: &AdminQuestion,
    item: &PaperQuestion,
) -> StoredAttemptQuestion {
    StoredAttemptQuestion {
        question_id: item.question_id,
        revision_id: item.revision_id,
        position: item.position,
        score: item.score,
        question_type: question.question_type,
        stem: question.stem.clone(),
        blank_count: question.blank_count,
        options: question.options.clone(),
        correct_answer: question.correct_answer.clone(),
        explanation: question.explanation.clone(),
        answer: None,
        grading_status: GradingStatus::Pending,
        awarded_score: None,
        feedback: None,
    }
}

#[async_trait]
impl PaperStore for FakeQuestionStore {
    async fn list_admin_papers(&self) -> Result<Vec<AdminPaper>, PaperStoreError> {
        Ok(self.papers.lock().await.values().cloned().collect())
    }

    async fn get_admin_paper(&self, id: i64) -> Result<Option<AdminPaper>, PaperStoreError> {
        Ok(self.papers.lock().await.get(&id).cloned())
    }

    async fn create_paper_draft(
        &self,
        _actor_user_id: i64,
        input: &xiaoluoquiz::domain::CreatePaperInput,
        items: &[PaperQuestionSnapshot],
    ) -> Result<AdminPaper, PaperStoreError> {
        let id = self.next_paper_id.fetch_add(1, Ordering::Relaxed);
        let items = items
            .iter()
            .map(PaperQuestionSnapshot::as_public)
            .collect::<Vec<_>>();
        let paper = AdminPaper {
            id,
            status: PaperStatus::Draft,
            title: input.title.clone(),
            description: input.description.clone(),
            audience: input.audience.clone(),
            mode: input.mode,
            open_at: input.open_at.clone(),
            close_at: input.close_at.clone(),
            duration_seconds: input.duration_seconds,
            max_attempts: input.max_attempts,
            allow_resume: input.allow_resume,
            auto_save: input.auto_save,
            auto_submit: input.auto_submit,
            candidate_fields: input.candidate_fields.clone(),
            result_visibility: input.result_visibility,
            allow_preview: input.allow_preview,
            total_score: items.iter().map(|item| item.score).sum(),
            items,
        };
        self.papers.lock().await.insert(id, paper.clone());
        Ok(paper)
    }

    async fn publish_paper(
        &self,
        _actor_user_id: i64,
        id: i64,
    ) -> Result<Option<AdminPaper>, PaperStoreError> {
        let mut papers = self.papers.lock().await;
        let Some(paper) = papers.get_mut(&id) else {
            return Ok(None);
        };
        paper.status = PaperStatus::Published;
        Ok(Some(paper.clone()))
    }

    async fn archive_paper(
        &self,
        _actor_user_id: i64,
        id: i64,
    ) -> Result<Option<AdminPaper>, PaperStoreError> {
        let mut papers = self.papers.lock().await;
        let Some(paper) = papers.get_mut(&id) else {
            return Ok(None);
        };
        paper.status = PaperStatus::Archived;
        Ok(Some(paper.clone()))
    }

    async fn list_published_papers(
        &self,
        user_id: i64,
    ) -> Result<Vec<xiaoluoquiz::domain::PublishedPaper>, PaperStoreError> {
        let papers = self.papers.lock().await;
        let attempts = self.attempts.lock().await;
        Ok(papers
            .values()
            .filter(|paper| paper.status == PaperStatus::Published)
            .map(|paper| {
                let current = attempts.values().find(|record| {
                    record.user_id == user_id
                        && record.attempt.paper_id == paper.id
                        && record.attempt.status == AttemptStatus::InProgress
                });
                published_paper_view(paper, current)
            })
            .collect())
    }

    async fn get_published_paper(
        &self,
        user_id: i64,
        id: i64,
    ) -> Result<Option<xiaoluoquiz::domain::PublishedPaper>, PaperStoreError> {
        let papers = self.papers.lock().await;
        let Some(paper) = papers
            .get(&id)
            .filter(|paper| paper.status == PaperStatus::Published)
        else {
            return Ok(None);
        };
        let attempts = self.attempts.lock().await;
        let current = attempts.values().find(|record| {
            record.user_id == user_id
                && record.attempt.paper_id == paper.id
                && record.attempt.status == AttemptStatus::InProgress
        });
        Ok(Some(published_paper_view(paper, current)))
    }

    async fn create_attempt(
        &self,
        user_id: i64,
        paper_id: i64,
        candidate_info: CandidateInfo,
    ) -> Result<StoredAttempt, PaperStoreError> {
        let paper = self
            .papers
            .lock()
            .await
            .get(&paper_id)
            .filter(|paper| paper.status == PaperStatus::Published)
            .cloned()
            .ok_or(PaperStoreError::PaperNotFound)?;

        let mut attempts = self.attempts.lock().await;
        if let Some(existing) = attempts.values().find(|record| {
            record.user_id == user_id
                && record.attempt.paper_id == paper_id
                && record.attempt.status == AttemptStatus::InProgress
        }) {
            if paper.allow_resume {
                return Ok(existing.attempt.clone());
            }
        }
        let count = attempts
            .values()
            .filter(|record| record.user_id == user_id && record.attempt.paper_id == paper_id)
            .count();
        if count >= paper.max_attempts as usize {
            return Err(PaperStoreError::MaxAttemptsReached);
        }

        let questions = self.questions.lock().await;
        let mut stored_questions = Vec::with_capacity(paper.items.len());
        for item in &paper.items {
            let question = questions
                .get(&item.question_id)
                .ok_or(PaperStoreError::Store(StoreError::InvalidData(
                    "paper question was not found".to_owned(),
                )))?;
            stored_questions.push(stored_attempt_question(question, item));
        }
        drop(questions);

        let attempt_id = self.next_attempt_id.fetch_add(1, Ordering::Relaxed);
        let attempt = StoredAttempt {
            id: attempt_id,
            user_id,
            paper_id,
            title: paper.title,
            status: AttemptStatus::InProgress,
            started_at: "2026-09-01T00:00:00.000Z".to_owned(),
            deadline_at: paper
                .duration_seconds
                .map(|_| "2026-09-01T01:00:00.000Z".to_owned()),
            auto_submit: paper.auto_submit,
            deadline_reached: false,
            submitted_at: None,
            candidate_info,
            max_score: paper.total_score,
            total_score: None,
            result_visibility: paper.result_visibility,
            questions: stored_questions,
        };
        attempts.insert(
            attempt_id,
            FakeAttemptRecord {
                user_id,
                attempt: attempt.clone(),
            },
        );
        Ok(attempt)
    }

    async fn get_attempt(
        &self,
        user_id: i64,
        attempt_id: i64,
    ) -> Result<Option<StoredAttempt>, PaperStoreError> {
        Ok(self
            .attempts
            .lock()
            .await
            .get(&attempt_id)
            .filter(|record| record.user_id == user_id)
            .map(|record| record.attempt.clone()))
    }

    async fn list_admin_attempts(&self) -> Result<Vec<StoredAttempt>, PaperStoreError> {
        Ok(self
            .attempts
            .lock()
            .await
            .values()
            .filter(|record| record.attempt.status != AttemptStatus::InProgress)
            .map(|record| record.attempt.clone())
            .collect())
    }

    async fn get_admin_attempt(
        &self,
        attempt_id: i64,
    ) -> Result<Option<StoredAttempt>, PaperStoreError> {
        Ok(self
            .attempts
            .lock()
            .await
            .get(&attempt_id)
            .map(|record| record.attempt.clone()))
    }

    async fn grade_answer(
        &self,
        _actor_user_id: i64,
        attempt_id: i64,
        question_id: i64,
        score: f64,
        feedback: Option<String>,
    ) -> Result<StoredAttempt, PaperStoreError> {
        let mut attempts = self.attempts.lock().await;
        let record = attempts
            .get_mut(&attempt_id)
            .ok_or(PaperStoreError::AttemptNotFound)?;
        if record.attempt.status == AttemptStatus::InProgress {
            return Err(PaperStoreError::AttemptNotSubmitted);
        }
        let question = record
            .attempt
            .questions
            .iter_mut()
            .find(|question| question.question_id == question_id)
            .ok_or(PaperStoreError::QuestionNotInAttempt)?;
        if question.answer.is_none() {
            return Err(PaperStoreError::AnswerNotSaved);
        }
        question.grading_status = GradingStatus::Graded;
        question.awarded_score = Some(score);
        question.feedback = feedback;
        record.attempt.status = if record
            .attempt
            .questions
            .iter()
            .any(|question| question.grading_status == GradingStatus::NeedsReview)
        {
            AttemptStatus::NeedsReview
        } else {
            AttemptStatus::Graded
        };
        record.attempt.total_score = Some(
            record
                .attempt
                .questions
                .iter()
                .map(|question| question.awarded_score.unwrap_or(0.0))
                .sum(),
        );
        Ok(record.attempt.clone())
    }

    async fn save_answer(
        &self,
        user_id: i64,
        attempt_id: i64,
        question_id: i64,
        answer: AnswerPayload,
    ) -> Result<StoredAttempt, PaperStoreError> {
        let mut attempts = self.attempts.lock().await;
        let record = attempts
            .get_mut(&attempt_id)
            .filter(|record| record.user_id == user_id)
            .ok_or(PaperStoreError::AttemptNotFound)?;
        if record.attempt.status != AttemptStatus::InProgress {
            return Err(PaperStoreError::AttemptClosed);
        }
        let question = record
            .attempt
            .questions
            .iter_mut()
            .find(|question| question.question_id == question_id)
            .ok_or(PaperStoreError::QuestionNotInAttempt)?;
        question.answer = Some(answer);
        Ok(record.attempt.clone())
    }

    async fn submit_attempt(
        &self,
        user_id: i64,
        attempt_id: i64,
        evaluations: &[GradedAnswer],
    ) -> Result<StoredAttempt, PaperStoreError> {
        let mut attempts = self.attempts.lock().await;
        let record = attempts
            .get_mut(&attempt_id)
            .filter(|record| record.user_id == user_id)
            .ok_or(PaperStoreError::AttemptNotFound)?;
        if record.attempt.status != AttemptStatus::InProgress {
            return Ok(record.attempt.clone());
        }
        let mut total_score = 0.0;
        let mut needs_review = false;
        for evaluation in evaluations {
            let Some(question) = record
                .attempt
                .questions
                .iter_mut()
                .find(|question| question.question_id == evaluation.question_id)
            else {
                return Err(PaperStoreError::QuestionNotInAttempt);
            };
            question.grading_status = evaluation.grading_status;
            question.awarded_score = evaluation.awarded_score;
            if evaluation.grading_status == GradingStatus::NeedsReview {
                needs_review = true;
            }
            total_score += evaluation.awarded_score.unwrap_or(0.0);
        }
        record.attempt.status = if needs_review {
            AttemptStatus::NeedsReview
        } else {
            AttemptStatus::Graded
        };
        record.attempt.submitted_at = Some("2026-09-01T01:00:00.000Z".to_owned());
        record.attempt.total_score = if needs_review {
            None
        } else {
            Some(total_score)
        };
        Ok(record.attempt.clone())
    }
}

#[async_trait]
impl AuthStore for FakeQuestionStore {
    async fn find_user_by_username(
        &self,
        username: &str,
    ) -> Result<Option<StoredUser>, AuthStoreError> {
        Ok(self
            .users
            .lock()
            .await
            .values()
            .find(|user| user.identity.username == username)
            .cloned())
    }

    async fn find_user_by_session(
        &self,
        token_hash: &str,
    ) -> Result<Option<StoredUser>, AuthStoreError> {
        let Some(user_id) = self.sessions.lock().await.get(token_hash).copied() else {
            return Ok(None);
        };
        Ok(self
            .users
            .lock()
            .await
            .get(&user_id)
            .filter(|user| user.identity.status == AccountStatus::Active)
            .cloned())
    }

    async fn create_session(
        &self,
        user_id: i64,
        token_hash: &str,
        _ttl_seconds: i64,
    ) -> Result<(), AuthStoreError> {
        if !self.users.lock().await.contains_key(&user_id) {
            return Err(AuthStoreError::InvalidData("user was not found".to_owned()));
        }
        self.sessions
            .lock()
            .await
            .insert(token_hash.to_owned(), user_id);
        Ok(())
    }

    async fn revoke_session(&self, token_hash: &str) -> Result<(), AuthStoreError> {
        self.sessions.lock().await.remove(token_hash);
        Ok(())
    }

    async fn record_login(&self, user_id: i64) -> Result<(), AuthStoreError> {
        self.failed_logins.lock().await.remove(&user_id);
        let mut users = self.users.lock().await;
        let Some(user) = users.get_mut(&user_id) else {
            return Err(AuthStoreError::InvalidData("user was not found".to_owned()));
        };
        user.identity.last_login_at = Some("2026-09-01T00:00:00.000Z".to_owned());
        user.locked = false;
        Ok(())
    }

    async fn record_failed_login(&self, user_id: i64) -> Result<(), AuthStoreError> {
        let attempts = {
            let mut failures = self.failed_logins.lock().await;
            let attempts = failures.entry(user_id).or_insert(0);
            *attempts = attempts.saturating_add(1);
            *attempts
        };
        let mut users = self.users.lock().await;
        let Some(user) = users.get_mut(&user_id) else {
            return Err(AuthStoreError::InvalidData("user was not found".to_owned()));
        };
        user.locked = attempts >= 5;
        Ok(())
    }

    async fn update_password(
        &self,
        user_id: i64,
        password_hash: &str,
        _actor_user_id: Option<i64>,
        _action: &str,
    ) -> Result<(), AuthStoreError> {
        let mut users = self.users.lock().await;
        let Some(user) = users.get_mut(&user_id) else {
            return Err(AuthStoreError::InvalidData("user was not found".to_owned()));
        };
        user.password_hash = Some(password_hash.to_owned());
        user.identity.must_change_password = false;
        user.locked = false;
        self.failed_logins.lock().await.remove(&user_id);
        Ok(())
    }

    async fn create_user(
        &self,
        _actor_user_id: i64,
        input: &CreateUserInput,
        password_hash: &str,
    ) -> Result<UserIdentity, AuthStoreError> {
        let class_name = match input.class_id {
            Some(class_id) => self
                .classes
                .lock()
                .await
                .get(&class_id)
                .map(|class| class.name.clone()),
            None => None,
        };
        let mut users = self.users.lock().await;
        if users
            .values()
            .any(|user| user.identity.username == input.username)
        {
            return Err(AuthStoreError::Conflict(
                "username already exists".to_owned(),
            ));
        }
        let id = self.next_user_id.fetch_add(1, Ordering::Relaxed);
        let identity = UserIdentity {
            id,
            username: input.username.clone(),
            display_name: input.display_name.clone(),
            role: input.role,
            status: AccountStatus::Active,
            must_change_password: true,
            student_number: input.student_number.clone(),
            class_name,
            created_at: "2026-09-01T00:00:00.000Z".to_owned(),
            last_login_at: None,
        };
        users.insert(
            id,
            StoredUser {
                identity: identity.clone(),
                password_hash: Some(password_hash.to_owned()),
                locked: false,
            },
        );
        Ok(identity)
    }

    async fn list_users(&self) -> Result<Vec<UserIdentity>, AuthStoreError> {
        Ok(self
            .users
            .lock()
            .await
            .values()
            .map(|user| user.identity.clone())
            .collect())
    }

    async fn list_classes(&self) -> Result<Vec<ClassGroup>, AuthStoreError> {
        Ok(self.classes.lock().await.values().cloned().collect())
    }

    async fn find_class(&self, class_id: i64) -> Result<Option<ClassGroup>, AuthStoreError> {
        Ok(self.classes.lock().await.get(&class_id).cloned())
    }

    async fn create_class(
        &self,
        _actor_user_id: i64,
        input: &CreateClassInput,
    ) -> Result<ClassGroup, AuthStoreError> {
        let mut classes = self.classes.lock().await;
        if classes.values().any(|class| class.name == input.name) {
            return Err(AuthStoreError::Conflict(
                "class name already exists".to_owned(),
            ));
        }
        let id = self.next_class_id.fetch_add(1, Ordering::Relaxed);
        let class = ClassGroup {
            id,
            name: input.name.clone(),
            created_at: "2026-09-01T00:00:00.000Z".to_owned(),
        };
        classes.insert(id, class.clone());
        Ok(class)
    }

    async fn get_user(&self, user_id: i64) -> Result<Option<UserIdentity>, AuthStoreError> {
        Ok(self
            .users
            .lock()
            .await
            .get(&user_id)
            .map(|user| user.identity.clone()))
    }

    async fn count_active_admins(&self) -> Result<u64, AuthStoreError> {
        Ok(self
            .users
            .lock()
            .await
            .values()
            .filter(|user| {
                user.identity.role == UserRole::Admin
                    && user.identity.status == AccountStatus::Active
            })
            .count() as u64)
    }

    async fn update_status(
        &self,
        _actor_user_id: i64,
        user_id: i64,
        status: AccountStatus,
    ) -> Result<UserIdentity, AuthStoreError> {
        let mut users = self.users.lock().await;
        let Some(user) = users.get_mut(&user_id) else {
            return Err(AuthStoreError::InvalidData("user was not found".to_owned()));
        };
        user.identity.status = status;
        Ok(user.identity.clone())
    }

    async fn reset_password(
        &self,
        _actor_user_id: i64,
        user_id: i64,
        password_hash: &str,
    ) -> Result<UserIdentity, AuthStoreError> {
        let identity = {
            let mut users = self.users.lock().await;
            let Some(user) = users.get_mut(&user_id) else {
                return Err(AuthStoreError::InvalidData("user was not found".to_owned()));
            };
            user.password_hash = Some(password_hash.to_owned());
            user.identity.must_change_password = true;
            user.locked = false;
            user.identity.clone()
        };
        self.sessions.lock().await.retain(|_, id| *id != user_id);
        Ok(identity)
    }

    async fn update_role(
        &self,
        _actor_user_id: i64,
        user_id: i64,
        role: UserRole,
    ) -> Result<UserIdentity, AuthStoreError> {
        let mut users = self.users.lock().await;
        let Some(user) = users.get_mut(&user_id) else {
            return Err(AuthStoreError::InvalidData("user was not found".to_owned()));
        };
        user.identity.role = role;
        Ok(user.identity.clone())
    }

    async fn ensure_bootstrap_admin(
        &self,
        _username: &str,
        _display_name: &str,
        _password_hash: &str,
    ) -> Result<(), AuthStoreError> {
        Ok(())
    }
}
