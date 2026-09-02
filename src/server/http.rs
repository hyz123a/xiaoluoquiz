use std::{path::Path, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path as RoutePath, Query, State},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{COOKIE, SET_COOKIE},
    },
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};

use crate::application::{
    AdminQuestionFilters, AuthError, AuthService, AuthStore, AuthStoreError, ExamError,
    ExamService, PaperManagementError, PaperManagementService, PaperStore, PaperStoreError,
    PracticeError, PracticeService, QuestionImportReport, QuestionManagementError,
    QuestionManagementService, QuestionStore, StoreError,
};
use crate::domain::auth::{
    AccountStatus, ClassGroup, CreateClassInput, CreateUserInput, UserIdentity, UserRole,
};
use crate::domain::{
    AdminAttempt, AdminAttemptSummary, AdminPaper, AdminQuestion, AdminQuestionInput,
    AnswerPayload, CandidateInfo, CorrectAnswer, CreatePaperInput, EvaluationStatus, ExamAttempt,
    ExamResult, PublishedPaper, QuestionBank, QuestionBankInput, QuestionImportBatch,
};

#[derive(Clone)]
pub struct AppState {
    pub(crate) auth: Arc<AuthService>,
    pub(crate) practice: Arc<PracticeService>,
    pub(crate) management: Arc<QuestionManagementService>,
    pub(crate) papers: Arc<PaperManagementService>,
    pub(crate) exams: Arc<ExamService>,
}

impl AppState {
    pub fn new<S>(store: Arc<S>) -> Self
    where
        S: QuestionStore + AuthStore + PaperStore + 'static,
    {
        Self::with_initial_password(store, "InitialPassword123!")
    }

    pub fn with_initial_password<S>(store: Arc<S>, initial_password: impl Into<Arc<str>>) -> Self
    where
        S: QuestionStore + AuthStore + PaperStore + 'static,
    {
        let questions: Arc<dyn QuestionStore> = store.clone();
        let auth_store: Arc<dyn AuthStore> = store.clone();
        let paper_store: Arc<dyn PaperStore> = store;
        Self::with_stores(questions, auth_store, paper_store, initial_password)
    }

    pub fn with_stores(
        questions: Arc<dyn QuestionStore>,
        auth_store: Arc<dyn AuthStore>,
        paper_store: Arc<dyn PaperStore>,
        initial_password: impl Into<Arc<str>>,
    ) -> Self {
        Self::with_stores_and_session_ttl(
            questions,
            auth_store,
            paper_store,
            initial_password,
            12 * 60 * 60,
        )
    }

    pub fn with_stores_and_session_ttl(
        questions: Arc<dyn QuestionStore>,
        auth_store: Arc<dyn AuthStore>,
        paper_store: Arc<dyn PaperStore>,
        initial_password: impl Into<Arc<str>>,
        session_ttl_seconds: i64,
    ) -> Self {
        Self {
            auth: Arc::new(
                AuthService::new(auth_store, initial_password)
                    .with_session_ttl(session_ttl_seconds),
            ),
            practice: Arc::new(PracticeService::new(questions.clone())),
            management: Arc::new(QuestionManagementService::new(questions.clone())),
            papers: Arc::new(PaperManagementService::new(questions, paper_store.clone())),
            exams: Arc::new(ExamService::new(paper_store)),
        }
    }
}

pub fn api_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/config", get(auth_config))
        .route("/api/v1/auth/me", get(me))
        .route("/api/v1/auth/change-password", post(change_password))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/question-banks", get(list_question_banks))
        .route("/api/v1/questions", get(list_questions))
        .route("/api/v1/questions/{id}", get(get_question))
        .route("/api/v1/questions/{id}/check", post(check_answer))
        .route("/api/v1/papers", get(list_papers))
        .route("/api/v1/papers/{id}", get(get_paper))
        .route("/api/v1/papers/{id}/attempts", post(start_attempt))
        .route("/api/v1/attempts/{id}", get(get_attempt))
        .route("/api/v1/attempts/{id}/answers", post(save_attempt_answer))
        .route("/api/v1/attempts/{id}/submit", post(submit_attempt))
        .route("/api/v1/attempts/{id}/result", get(get_attempt_result))
        .route(
            "/api/v1/admin/question-banks",
            post(create_admin_question_bank),
        )
        .route(
            "/api/v1/admin/questions",
            get(list_admin_questions).post(create_admin_question),
        )
        .route(
            "/api/v1/admin/questions/import",
            post(import_admin_questions),
        )
        .route(
            "/api/v1/admin/questions/{id}/publish",
            post(publish_admin_question),
        )
        .route(
            "/api/v1/admin/questions/{id}/archive",
            post(archive_admin_question),
        )
        .route(
            "/api/v1/admin/papers",
            get(list_admin_papers).post(create_admin_paper),
        )
        .route(
            "/api/v1/admin/papers/{id}/publish",
            post(publish_admin_paper),
        )
        .route(
            "/api/v1/admin/papers/{id}/archive",
            post(archive_admin_paper),
        )
        .route("/api/v1/admin/attempts", get(list_admin_attempts))
        .route("/api/v1/admin/attempts/{id}", get(get_admin_attempt))
        .route(
            "/api/v1/admin/attempts/{id}/grade",
            post(grade_admin_attempt),
        )
        .route(
            "/api/v1/admin/users",
            get(list_admin_users).post(create_admin_user),
        )
        .route(
            "/api/v1/admin/classes",
            get(list_admin_classes).post(create_admin_class),
        )
        .route("/api/v1/admin/users/{id}/enable", post(enable_admin_user))
        .route("/api/v1/admin/users/{id}/disable", post(disable_admin_user))
        .route(
            "/api/v1/admin/users/{id}/reset-password",
            post(reset_admin_user_password),
        )
        .route(
            "/api/v1/admin/users/{id}/role",
            post(update_admin_user_role),
        )
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

pub fn application_router(state: AppState, static_dir: impl AsRef<Path>) -> Router {
    let static_dir = static_dir.as_ref();
    let index = static_dir.join("index.html");
    api_router(state).fallback_service(
        ServeDir::new(static_dir)
            .append_index_html_on_directories(true)
            .not_found_service(ServeFile::new(index)),
    )
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

const SESSION_COOKIE: &str = "xiaoluoquiz_session";

#[derive(Debug, Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct ChangePasswordRequest {
    new_password: String,
}

#[derive(Debug, Serialize)]
struct AuthConfigResponse {
    initial_password: String,
}

async fn auth_config(State(state): State<AppState>) -> Json<AuthConfigResponse> {
    Json(AuthConfigResponse {
        initial_password: state.auth.initial_password().to_owned(),
    })
}

#[derive(Debug, Serialize)]
struct AuthResponse {
    user: UserIdentity,
}

async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Response, ApiError> {
    let session = state
        .auth
        .login(&request.username, &request.password)
        .await?;
    let mut response = Json(AuthResponse { user: session.user }).into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{SESSION_COOKIE}={}; Path=/; HttpOnly; SameSite=Lax",
            session.token
        ))
        .expect("session token is hex encoded"),
    );
    Ok(response)
}

async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthResponse>, ApiError> {
    let token = session_token(&headers).ok_or(AuthError::AuthenticationRequired)?;
    Ok(Json(AuthResponse {
        user: state.auth.authenticate(token).await?,
    }))
}

async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ChangePasswordRequest>,
) -> Result<Json<AuthResponse>, ApiError> {
    let token = session_token(&headers).ok_or(AuthError::AuthenticationRequired)?;
    Ok(Json(AuthResponse {
        user: state
            .auth
            .change_password(token, &request.new_password)
            .await?,
    }))
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    if let Some(token) = session_token(&headers) {
        state.auth.logout(token).await?;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_static("xiaoluoquiz_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"),
    );
    Ok(response)
}

fn session_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let (name, value) = cookie.trim().split_once('=')?;
                (name == SESSION_COOKIE && !value.is_empty()).then_some(value)
            })
        })
}

async fn require_user(state: &AppState, headers: &HeaderMap) -> Result<UserIdentity, ApiError> {
    let token = session_token(headers).ok_or(AuthError::AuthenticationRequired)?;
    let user = state.auth.authenticate(token).await?;
    if user.must_change_password {
        return Err(AuthError::PasswordChangeRequired.into());
    }
    Ok(user)
}

async fn require_admin(state: &AppState, headers: &HeaderMap) -> Result<UserIdentity, ApiError> {
    let user = require_user(state, headers).await?;
    if user.role != UserRole::Admin {
        return Err(AuthError::Forbidden.into());
    }
    Ok(user)
}

#[derive(Debug, Serialize)]
struct QuestionBankListResponse {
    items: Vec<QuestionBank>,
}

#[derive(Debug, Deserialize)]
struct QuestionListQuery {
    bank_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct AdminQuestionListQuery {
    keyword: Option<String>,
    bank_id: Option<i64>,
    question_type: Option<crate::domain::QuestionType>,
    status: Option<crate::domain::QuestionStatus>,
}

#[derive(Debug, Serialize)]
struct QuestionListResponse {
    items: Vec<crate::domain::PublicQuestion>,
}

#[derive(Debug, Serialize)]
struct AdminQuestionListResponse {
    items: Vec<AdminQuestion>,
}

async fn list_question_banks(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<QuestionBankListResponse>, ApiError> {
    let _user = require_user(&state, &headers).await?;
    Ok(Json(QuestionBankListResponse {
        items: state.practice.list_question_banks().await?,
    }))
}

async fn list_questions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<QuestionListQuery>,
) -> Result<Json<QuestionListResponse>, ApiError> {
    let _user = require_user(&state, &headers).await?;
    Ok(Json(QuestionListResponse {
        items: state.practice.list_published(query.bank_id).await?,
    }))
}

async fn get_question(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
) -> Result<Json<crate::domain::PublicQuestion>, ApiError> {
    let _user = require_user(&state, &headers).await?;
    state
        .practice
        .get_published(id)
        .await?
        .map(Json)
        .ok_or(ApiError::NotFound)
}

#[derive(Debug, Deserialize)]
struct CheckAnswerRequest {
    answer: AnswerPayload,
}

#[derive(Debug, Serialize)]
struct CheckAnswerResponse {
    question_id: i64,
    status: EvaluationStatus,
    correct: Option<bool>,
    explanation: Option<String>,
    correct_answer: CorrectAnswer,
}

async fn check_answer(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
    Json(request): Json<CheckAnswerRequest>,
) -> Result<Json<CheckAnswerResponse>, ApiError> {
    let _user = require_user(&state, &headers).await?;
    let checked = state.practice.check_answer(id, &request.answer).await?;

    Ok(Json(CheckAnswerResponse {
        question_id: checked.question_id,
        status: checked.evaluation.status,
        correct: checked.evaluation.correct,
        explanation: checked.explanation,
        correct_answer: checked.correct_answer,
    }))
}

async fn list_admin_questions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AdminQuestionListQuery>,
) -> Result<Json<AdminQuestionListResponse>, ApiError> {
    let _admin = require_admin(&state, &headers).await?;
    Ok(Json(AdminQuestionListResponse {
        items: state
            .management
            .list(AdminQuestionFilters {
                keyword: query.keyword,
                bank_id: query.bank_id,
                question_type: query.question_type,
                status: query.status,
            })
            .await?,
    }))
}

async fn import_admin_questions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(batch): Json<QuestionImportBatch>,
) -> Result<Json<QuestionImportReport>, ApiError> {
    let _admin = require_admin(&state, &headers).await?;
    Ok(Json(state.management.import_add_only(batch).await?))
}

async fn create_admin_question_bank(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<QuestionBankInput>,
) -> Result<(StatusCode, Json<QuestionBank>), ApiError> {
    let _admin = require_admin(&state, &headers).await?;
    let bank = state.management.create_bank(input).await?;
    Ok((StatusCode::CREATED, Json(bank)))
}

async fn create_admin_question(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<AdminQuestionInput>,
) -> Result<(StatusCode, Json<AdminQuestion>), ApiError> {
    let _admin = require_admin(&state, &headers).await?;
    let question = state.management.create_draft(input).await?;
    Ok((StatusCode::CREATED, Json(question)))
}

async fn publish_admin_question(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
) -> Result<Json<AdminQuestion>, ApiError> {
    let _admin = require_admin(&state, &headers).await?;
    Ok(Json(state.management.publish(id).await?))
}

async fn archive_admin_question(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
) -> Result<Json<AdminQuestion>, ApiError> {
    let _admin = require_admin(&state, &headers).await?;
    Ok(Json(state.management.archive(id).await?))
}

#[derive(Debug, Serialize)]
struct PaperListResponse {
    items: Vec<PublishedPaper>,
}

#[derive(Debug, Serialize)]
struct AdminPaperListResponse {
    items: Vec<AdminPaper>,
}

#[derive(Debug, Serialize)]
struct AdminAttemptListResponse {
    items: Vec<AdminAttemptSummary>,
}

#[derive(Debug, Deserialize)]
struct GradeAttemptRequest {
    question_id: i64,
    score: f64,
    feedback: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SaveAttemptAnswerRequest {
    question_id: i64,
    answer: AnswerPayload,
}

async fn list_admin_papers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AdminPaperListResponse>, ApiError> {
    let _admin = require_admin(&state, &headers).await?;
    Ok(Json(AdminPaperListResponse {
        items: state.papers.list().await?,
    }))
}

async fn list_admin_attempts(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AdminAttemptListResponse>, ApiError> {
    let _admin = require_admin(&state, &headers).await?;
    Ok(Json(AdminAttemptListResponse {
        items: state.exams.list_admin_attempts().await?,
    }))
}

async fn get_admin_attempt(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
) -> Result<Json<AdminAttempt>, ApiError> {
    let _admin = require_admin(&state, &headers).await?;
    Ok(Json(state.exams.get_admin_attempt(id).await?))
}

async fn grade_admin_attempt(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
    Json(request): Json<GradeAttemptRequest>,
) -> Result<Json<AdminAttempt>, ApiError> {
    let admin = require_admin(&state, &headers).await?;
    Ok(Json(
        state
            .exams
            .grade_admin_answer(
                admin.id,
                id,
                request.question_id,
                request.score,
                request.feedback,
            )
            .await?,
    ))
}

async fn create_admin_paper(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreatePaperInput>,
) -> Result<(StatusCode, Json<AdminPaper>), ApiError> {
    let admin = require_admin(&state, &headers).await?;
    Ok((
        StatusCode::CREATED,
        Json(state.papers.create_draft(admin.id, input).await?),
    ))
}

async fn publish_admin_paper(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
) -> Result<Json<AdminPaper>, ApiError> {
    let admin = require_admin(&state, &headers).await?;
    Ok(Json(state.papers.publish(admin.id, id).await?))
}

async fn archive_admin_paper(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
) -> Result<Json<AdminPaper>, ApiError> {
    let admin = require_admin(&state, &headers).await?;
    Ok(Json(state.papers.archive(admin.id, id).await?))
}

async fn list_papers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PaperListResponse>, ApiError> {
    let user = require_user(&state, &headers).await?;
    Ok(Json(PaperListResponse {
        items: state.exams.list_papers(user.id).await?,
    }))
}

async fn get_paper(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
) -> Result<Json<PublishedPaper>, ApiError> {
    let user = require_user(&state, &headers).await?;
    Ok(Json(state.exams.get_paper(user.id, id).await?))
}

async fn start_attempt(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
    Json(candidate_info): Json<CandidateInfo>,
) -> Result<(StatusCode, Json<ExamAttempt>), ApiError> {
    let user = require_user(&state, &headers).await?;
    Ok((
        StatusCode::CREATED,
        Json(state.exams.start(user.id, id, candidate_info).await?),
    ))
}

async fn get_attempt(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
) -> Result<Json<ExamAttempt>, ApiError> {
    let user = require_user(&state, &headers).await?;
    Ok(Json(state.exams.get_attempt(user.id, id).await?))
}

async fn save_attempt_answer(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
    Json(request): Json<SaveAttemptAnswerRequest>,
) -> Result<Json<ExamAttempt>, ApiError> {
    let user = require_user(&state, &headers).await?;
    Ok(Json(
        state
            .exams
            .save_answer(user.id, id, request.question_id, request.answer)
            .await?,
    ))
}

async fn submit_attempt(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
) -> Result<Json<ExamResult>, ApiError> {
    let user = require_user(&state, &headers).await?;
    Ok(Json(state.exams.submit(user.id, id).await?))
}

async fn get_attempt_result(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
) -> Result<Json<ExamResult>, ApiError> {
    let user = require_user(&state, &headers).await?;
    Ok(Json(state.exams.result(user.id, id).await?))
}

#[derive(Debug, Serialize)]
struct UserListResponse {
    items: Vec<UserIdentity>,
}

#[derive(Debug, Serialize)]
struct ClassListResponse {
    items: Vec<ClassGroup>,
}

#[derive(Debug, Deserialize)]
struct UpdateRoleRequest {
    role: UserRole,
}

async fn list_admin_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserListResponse>, ApiError> {
    let admin = require_admin(&state, &headers).await?;
    Ok(Json(UserListResponse {
        items: state.auth.list_users(&admin).await?,
    }))
}

async fn list_admin_classes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ClassListResponse>, ApiError> {
    let admin = require_admin(&state, &headers).await?;
    Ok(Json(ClassListResponse {
        items: state.auth.list_classes(&admin).await?,
    }))
}

async fn create_admin_class(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateClassInput>,
) -> Result<(StatusCode, Json<ClassGroup>), ApiError> {
    let admin = require_admin(&state, &headers).await?;
    let class_group = state.auth.create_class(&admin, input).await?;
    Ok((StatusCode::CREATED, Json(class_group)))
}

async fn create_admin_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<CreateUserInput>,
) -> Result<(StatusCode, Json<UserIdentity>), ApiError> {
    let admin = require_admin(&state, &headers).await?;
    let user = state.auth.create_user(&admin, input).await?;
    Ok((StatusCode::CREATED, Json(user)))
}

async fn enable_admin_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
) -> Result<Json<UserIdentity>, ApiError> {
    let admin = require_admin(&state, &headers).await?;
    Ok(Json(
        state
            .auth
            .set_status(&admin, id, AccountStatus::Active)
            .await?,
    ))
}

async fn disable_admin_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
) -> Result<Json<UserIdentity>, ApiError> {
    let admin = require_admin(&state, &headers).await?;
    Ok(Json(
        state
            .auth
            .set_status(&admin, id, AccountStatus::Disabled)
            .await?,
    ))
}

async fn reset_admin_user_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
) -> Result<Json<UserIdentity>, ApiError> {
    let admin = require_admin(&state, &headers).await?;
    Ok(Json(state.auth.reset_password(&admin, id).await?))
}

async fn update_admin_user_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    RoutePath(id): RoutePath<i64>,
    Json(request): Json<UpdateRoleRequest>,
) -> Result<Json<UserIdentity>, ApiError> {
    let admin = require_admin(&state, &headers).await?;
    Ok(Json(
        state.auth.update_role(&admin, id, request.role).await?,
    ))
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: &'static str,
}

#[derive(Debug)]
enum ApiError {
    NotFound,
    Evaluation(crate::domain::EvaluationError),
    Store(StoreError),
    Management(QuestionManagementError),
    PaperManagement(PaperManagementError),
    Exam(ExamError),
    Auth(AuthError),
}

impl From<AuthError> for ApiError {
    fn from(error: AuthError) -> Self {
        Self::Auth(error)
    }
}
impl From<StoreError> for ApiError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl From<PracticeError> for ApiError {
    fn from(error: PracticeError) -> Self {
        match error {
            PracticeError::NotFound => Self::NotFound,
            PracticeError::Evaluation(error) => Self::Evaluation(error),
            PracticeError::Store(error) => Self::Store(error),
        }
    }
}

impl From<QuestionManagementError> for ApiError {
    fn from(error: QuestionManagementError) -> Self {
        Self::Management(error)
    }
}

impl From<PaperManagementError> for ApiError {
    fn from(error: PaperManagementError) -> Self {
        Self::PaperManagement(error)
    }
}

impl From<ExamError> for ApiError {
    fn from(error: ExamError) -> Self {
        Self::Exam(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse { error: "not found" }),
            )
                .into_response(),
            Self::Evaluation(error) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "error": error.to_string() })),
            )
                .into_response(),
            Self::Store(error) => internal_error(error),
            Self::Management(error) => match error {
                QuestionManagementError::NotFound => (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse { error: "not found" }),
                )
                    .into_response(),
                QuestionManagementError::ImportValidation(report) => {
                    (StatusCode::UNPROCESSABLE_ENTITY, Json(report)).into_response()
                }
                QuestionManagementError::InvalidInput(error) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({ "error": error.to_string() })),
                )
                    .into_response(),
                QuestionManagementError::InvalidQuestionBankInput(error) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({ "error": error.to_string() })),
                )
                    .into_response(),
                QuestionManagementError::QuestionBankNotFound => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(ErrorResponse {
                        error: "question bank was not found",
                    }),
                )
                    .into_response(),
                QuestionManagementError::QuestionBankNameTaken => (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "question bank name is already in use",
                    }),
                )
                    .into_response(),
                QuestionManagementError::InvalidState(status) => (
                    StatusCode::CONFLICT,
                    Json(json!({
                        "error": format!("question is already in {status} state")
                    })),
                )
                    .into_response(),
                QuestionManagementError::Store(error) => internal_error(error),
            },
            Self::PaperManagement(error) => match error {
                PaperManagementError::NotFound => (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse { error: "not found" }),
                )
                    .into_response(),
                PaperManagementError::InvalidInput(error) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({ "error": error.to_string() })),
                )
                    .into_response(),
                PaperManagementError::QuestionNotPublished => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(ErrorResponse {
                        error: "selected question is not published",
                    }),
                )
                    .into_response(),
                PaperManagementError::QuestionRevisionChanged => (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "selected question version changed",
                    }),
                )
                    .into_response(),
                PaperManagementError::QuestionStore(error) => internal_error(error),
                PaperManagementError::InvalidState(status) => (
                    StatusCode::CONFLICT,
                    Json(json!({
                        "error": format!("paper is already in {status} state")
                    })),
                )
                    .into_response(),
                PaperManagementError::Store(error) => internal_paper_store_error(error),
            },
            Self::Exam(error) => match error {
                ExamError::PaperNotFound | ExamError::AttemptNotFound => (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse { error: "not found" }),
                )
                    .into_response(),
                ExamError::PaperUnavailable => (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "paper is not available",
                    }),
                )
                    .into_response(),
                ExamError::MaxAttemptsReached => (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "maximum attempts reached",
                    }),
                )
                    .into_response(),
                ExamError::AttemptClosed => (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "attempt is already submitted",
                    }),
                )
                    .into_response(),
                ExamError::QuestionNotInAttempt => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(ErrorResponse {
                        error: "question does not belong to this attempt",
                    }),
                )
                    .into_response(),
                ExamError::InvalidAnswer => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(ErrorResponse {
                        error: "answer type does not match question type",
                    }),
                )
                    .into_response(),
                ExamError::AttemptNotSubmitted => (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "attempt has not been submitted",
                    }),
                )
                    .into_response(),
                ExamError::AnswerNotSaved => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(ErrorResponse {
                        error: "answer has not been saved",
                    }),
                )
                    .into_response(),
                ExamError::InvalidGrade => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(ErrorResponse {
                        error: "grade is invalid",
                    }),
                )
                    .into_response(),
                ExamError::RequiredCandidateField(field) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({ "error": format!("candidate field is required: {field}") })),
                )
                    .into_response(),
                ExamError::ResultUnavailable => (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "result is not available",
                    }),
                )
                    .into_response(),
                ExamError::Store(error) => internal_paper_store_error(error),
            },
            Self::Auth(error) => match error {
                AuthError::InvalidCredentials => (
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        error: "invalid credentials",
                    }),
                )
                    .into_response(),
                AuthError::AccountDisabled => (
                    StatusCode::FORBIDDEN,
                    Json(ErrorResponse {
                        error: "account is disabled",
                    }),
                )
                    .into_response(),
                AuthError::AccountLocked => (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(ErrorResponse {
                        error: "account temporarily locked",
                    }),
                )
                    .into_response(),
                AuthError::AuthenticationRequired => (
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        error: "authentication required",
                    }),
                )
                    .into_response(),
                AuthError::PasswordChangeRequired => (
                    StatusCode::FORBIDDEN,
                    Json(json!({
                        "error": "password change required",
                        "code": "password_change_required"
                    })),
                )
                    .into_response(),
                AuthError::Forbidden => (
                    StatusCode::FORBIDDEN,
                    Json(ErrorResponse { error: "forbidden" }),
                )
                    .into_response(),
                AuthError::UserNotFound => (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse { error: "not found" }),
                )
                    .into_response(),
                AuthError::InvalidUsername(message) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({ "error": format!("username is invalid: {message}") })),
                )
                    .into_response(),
                AuthError::InvalidDisplayName => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(ErrorResponse {
                        error: "display name must not be empty",
                    }),
                )
                    .into_response(),
                AuthError::UsernameTaken => (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "username is already in use",
                    }),
                )
                    .into_response(),
                AuthError::InvalidClassName => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(ErrorResponse {
                        error: "class name must not be empty",
                    }),
                )
                    .into_response(),
                AuthError::ClassNameTaken => (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "class name is already in use",
                    }),
                )
                    .into_response(),
                AuthError::ClassNotFound => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(ErrorResponse {
                        error: "class was not found",
                    }),
                )
                    .into_response(),
                AuthError::CannotModifyLastAdmin => (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: "cannot modify the last active administrator",
                    }),
                )
                    .into_response(),
                AuthError::InvalidPassword(error) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({ "error": error.to_string() })),
                )
                    .into_response(),
                AuthError::PasswordHashing => internal_auth_error("password hashing failed"),
                AuthError::Store(error) => internal_auth_store_error(error),
            },
        }
    }
}

fn internal_error(error: StoreError) -> Response {
    tracing::error!(error = ?error, "question store request failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "internal server error",
        }),
    )
        .into_response()
}

fn internal_paper_store_error(error: PaperStoreError) -> Response {
    match error {
        PaperStoreError::Store(error) => internal_error(error),
        error => {
            tracing::error!(error = ?error, "paper store request failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "internal server error",
                }),
            )
                .into_response()
        }
    }
}

fn internal_auth_error(message: &'static str) -> Response {
    tracing::error!(message, "authentication request failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: message }),
    )
        .into_response()
}

fn internal_auth_store_error(error: AuthStoreError) -> Response {
    tracing::error!(error = ?error, "account store request failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "internal server error",
        }),
    )
        .into_response()
}
