#![cfg(feature = "web")]

use std::{cell::Cell, collections::BTreeMap, rc::Rc};

use gloo_net::http::{Request, Response};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::spawn_local;
use web_sys::{
    Event, HtmlInputElement, HtmlSelectElement, HtmlTextAreaElement, InputEvent, Storage,
    SubmitEvent,
};
use yew::prelude::*;

mod ui;
use ui::*;

use xiaoluoquiz::domain::{
    AdminAttempt, AdminAttemptQuestion, AdminAttemptSummary, AdminPaper, AdminQuestion,
    AdminQuestionInput, AnswerPayload, AttemptStatus, CandidateField, CandidateFieldConfig,
    CandidateInfo, CorrectAnswer, CreatePaperInput, EvaluationStatus, ExamAttempt, ExamQuestion,
    ExamResult, GradingStatus, PaperMode, PaperQuestionInput, PaperRuntimeStatus, PaperStatus,
    PracticeStats, PublicQuestion, PublishedPaper, QuestionBank, QuestionBankInput,
    QuestionImportBatch, QuestionOption, QuestionStatus, QuestionType, ResultVisibility,
    auth::{ClassGroup, CreateClassInput, CreateUserInput, UserIdentity, UserRole},
};

const AUTH_LOGIN_ENDPOINT: &str = "/api/v1/auth/login";
const AUTH_CONFIG_ENDPOINT: &str = "/api/v1/auth/config";
const AUTH_ME_ENDPOINT: &str = "/api/v1/auth/me";
const AUTH_CHANGE_PASSWORD_ENDPOINT: &str = "/api/v1/auth/change-password";
const AUTH_LOGOUT_ENDPOINT: &str = "/api/v1/auth/logout";
const QUESTIONS_ENDPOINT: &str = "/api/v1/questions";
const PRACTICE_STATS_ENDPOINT: &str = "/api/v1/practice/stats";
const QUESTION_BANKS_ENDPOINT: &str = "/api/v1/question-banks";
const ADMIN_QUESTIONS_ENDPOINT: &str = "/api/v1/admin/questions";
const ADMIN_QUESTION_IMPORT_ENDPOINT: &str = "/api/v1/admin/questions/import";
const ADMIN_USERS_ENDPOINT: &str = "/api/v1/admin/users";
const ADMIN_CLASSES_ENDPOINT: &str = "/api/v1/admin/classes";
const ADMIN_PAPERS_ENDPOINT: &str = "/api/v1/admin/papers";
const ADMIN_ATTEMPTS_ENDPOINT: &str = "/api/v1/admin/attempts";
const PAPERS_ENDPOINT: &str = "/api/v1/papers";
const ATTEMPTS_ENDPOINT: &str = "/api/v1/attempts";
const SHANGHAI_OFFSET: &str = "+08:00";
const SHANGHAI_OFFSET_MILLISECONDS: f64 = 8.0 * 60.0 * 60.0 * 1_000.0;

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct AuthResponse {
    user: UserIdentity,
}

#[derive(Debug, Clone, Deserialize)]
struct LoginConfigResponse {
    initial_password: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ApiErrorResponse {
    error: String,
}

#[derive(Debug, Clone, Serialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Clone, Serialize)]
struct ChangePasswordRequest {
    new_password: String,
}

#[derive(Debug, Clone, Deserialize)]
struct QuestionBankListResponse {
    items: Vec<QuestionBank>,
}

#[derive(Debug, Clone, Deserialize)]
struct QuestionListResponse {
    items: Vec<PublicQuestion>,
}

#[derive(Debug, Clone, Deserialize)]
struct AdminQuestionListResponse {
    items: Vec<AdminQuestion>,
}

#[derive(Debug, Clone, Deserialize)]
struct QuestionImportResponse {
    inserted: usize,
    skipped: usize,
    errors: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct UserListResponse {
    items: Vec<UserIdentity>,
}

#[derive(Debug, Clone, Deserialize)]
struct ClassListResponse {
    items: Vec<ClassGroup>,
}

#[derive(Debug, Clone, Deserialize)]
struct AdminPaperListResponse {
    items: Vec<AdminPaper>,
}

#[derive(Debug, Clone, Deserialize)]
struct AdminAttemptListResponse {
    items: Vec<AdminAttemptSummary>,
}

#[derive(Debug, Clone, Serialize)]
struct GradeAttemptRequest {
    question_id: i64,
    score: f64,
    feedback: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct PaperListResponse {
    items: Vec<PublishedPaper>,
}

#[derive(Debug, Clone, Serialize)]
struct SaveAttemptAnswerRequest {
    question_id: i64,
    answer: AnswerPayload,
}

#[derive(Debug, Clone, Serialize)]
struct UpdateRoleRequest {
    role: UserRole,
}

#[derive(Debug, Clone, Serialize)]
struct CheckAnswerRequest {
    answer: AnswerPayload,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct CheckAnswerResponse {
    status: EvaluationStatus,
    correct: Option<bool>,
    explanation: Option<String>,
    correct_answer: CorrectAnswer,
    practice_stats: PracticeStats,
}

#[derive(Debug, Clone, PartialEq)]
enum LoginConfigState {
    Loading,
    Ready(String),
    Error(String),
}

#[derive(Debug, Clone)]
enum AuthLoadState {
    Loading,
    Anonymous,
    Authenticated(UserIdentity),
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
enum PracticeLoadState {
    Loading,
    Ready {
        banks: Vec<QuestionBank>,
        questions: Vec<PublicQuestion>,
    },
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
enum PracticeStatsLoadState {
    Loading,
    Ready(PracticeStats),
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
enum AdminLoadState {
    Loading,
    Ready {
        banks: Vec<QuestionBank>,
        questions: Vec<AdminQuestion>,
    },
    Error(String),
}

#[derive(Debug, Clone, Default, PartialEq)]
struct AdminQuestionFilterValues {
    keyword: String,
    bank_id: Option<i64>,
    question_type: Option<QuestionType>,
    status: Option<QuestionStatus>,
}

impl AdminQuestionFilterValues {
    fn is_active(&self) -> bool {
        !self.keyword.trim().is_empty()
            || self.bank_id.is_some()
            || self.question_type.is_some()
            || self.status.is_some()
    }
}

#[derive(Debug, Clone, PartialEq)]
enum UserLoadState {
    Loading,
    Ready {
        users: Vec<UserIdentity>,
        classes: Vec<ClassGroup>,
    },
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
enum AdminPaperLoadState {
    Loading,
    Ready {
        papers: Vec<AdminPaper>,
        questions: Vec<AdminQuestion>,
    },
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
enum PaperLoadState {
    Loading,
    Ready(PublishedPaper),
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
enum PaperListLoadState {
    Loading,
    Ready(Vec<PublishedPaper>),
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
enum AttemptLoadState {
    Loading,
    Ready(ExamAttempt),
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
enum AdminAttemptLoadState {
    Loading,
    Ready(Vec<AdminAttemptSummary>),
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
enum AdminAttemptDetailLoadState {
    Loading,
    Ready(AdminAttempt),
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
enum ResultLoadState {
    Loading,
    Ready(ExamResult),
    Error(String),
}

#[function_component(App)]
fn app() -> Html {
    html! { <AuthApp /> }
}

#[function_component(AuthApp)]
fn auth_app() -> Html {
    let path = web_sys::window()
        .and_then(|window| window.location().pathname().ok())
        .unwrap_or_else(|| "/".to_owned());
    let auth_state = use_state(|| AuthLoadState::Loading);

    {
        let auth_state = auth_state.clone();
        let path = path.clone();
        use_effect_with(path.clone(), move |_| {
            if path == "/login" {
                auth_state.set(AuthLoadState::Anonymous);
            } else {
                spawn_local(async move {
                    let result: Result<UserIdentity, String> = async {
                        let response = Request::get(AUTH_ME_ENDPOINT)
                            .send()
                            .await
                            .map_err(|error| error.to_string())?;
                        if response.ok() {
                            response
                                .json::<AuthResponse>()
                                .await
                                .map(|payload| payload.user)
                                .map_err(|error| error.to_string())
                        } else if response.status() == 401 {
                            Err("anonymous".to_owned())
                        } else {
                            Err(format!("认证状态加载失败（{}）", response.status()))
                        }
                    }
                    .await;

                    match result {
                        Ok(user) => auth_state.set(AuthLoadState::Authenticated(user)),
                        Err(error) if error == "anonymous" => {
                            auth_state.set(AuthLoadState::Anonymous)
                        }
                        Err(error) => auth_state.set(AuthLoadState::Error(error)),
                    }
                });
            }
            || ()
        });
    }

    let on_authenticated = {
        let auth_state = auth_state.clone();
        Callback::from(move |user: UserIdentity| {
            auth_state.set(AuthLoadState::Authenticated(user));
        })
    };

    match &*auth_state {
        AuthLoadState::Loading => html! {
            <main class={classes!(AUTH_SHELL, "bg-base-200")} data-testid="auth-loading">
                <span class="loading loading-spinner loading-lg text-primary" />
            </main>
        },
        AuthLoadState::Error(error) => html! {
            <main class={classes!(AUTH_SHELL, "bg-base-200")}>
                <div class="alert alert-error max-w-xl" data-testid="auth-error">{ error }</div>
            </main>
        },
        AuthLoadState::Anonymous => html! {
            <LoginPage on_authenticated={on_authenticated.clone()} />
        },
        AuthLoadState::Authenticated(user) if user.must_change_password => html! {
            <ChangePasswordPage user={user.clone()} on_changed={on_authenticated.clone()} />
        },
        AuthLoadState::Authenticated(user) => render_authenticated_route(&path, user.clone()),
    }
}

fn render_authenticated_route(path: &str, user: UserIdentity) -> Html {
    if path.starts_with("/admin/attempts") {
        if user.role != UserRole::Admin {
            html! { <AccessDeniedPage user={user} /> }
        } else if let Some(attempt_id) = route_id(path, "/admin/attempts/", "") {
            html! { <AdminAttemptDetailPage user={user} attempt_id={attempt_id} /> }
        } else {
            html! { <AdminAttemptsApp user={user} /> }
        }
    } else if path.starts_with("/admin/users") {
        if user.role == UserRole::Admin {
            html! { <AdminUsersApp user={user} /> }
        } else {
            html! { <AccessDeniedPage user={user} /> }
        }
    } else if path.starts_with("/admin/papers") {
        if user.role == UserRole::Admin {
            html! { <AdminPapersApp user={user} /> }
        } else {
            html! { <AccessDeniedPage user={user} /> }
        }
    } else if path == "/admin" || path.starts_with("/admin/") {
        if user.role == UserRole::Admin {
            html! { <AdminApp user={user} /> }
        } else {
            html! { <AccessDeniedPage user={user} /> }
        }
    } else if let Some(attempt_id) = route_id(path, "/exam/", "/result") {
        html! { <ExamResultPage user={user} attempt_id={attempt_id} /> }
    } else if let Some(attempt_id) = route_id(path, "/exam/", "") {
        html! { <ExamPage user={user} attempt_id={attempt_id} /> }
    } else if let Some(paper_id) = route_id(path, "/papers/", "/start") {
        html! { <PaperStartPage user={user} paper_id={paper_id} /> }
    } else if path == "/papers" {
        html! { <PapersApp user={user} /> }
    } else {
        html! { <PracticeApp user={user} /> }
    }
}

fn route_id(path: &str, prefix: &str, suffix: &str) -> Option<i64> {
    let value = path.strip_prefix(prefix)?;
    let value = if suffix.is_empty() {
        value
    } else {
        value.strip_suffix(suffix)?
    };
    value.parse().ok()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NavigationItem {
    Practice,
    Papers,
    Questions,
    PaperBuilder,
    Attempts,
    Users,
}

impl NavigationItem {
    fn label(self) -> &'static str {
        match self {
            Self::Practice => "练习",
            Self::Papers => "正式考试",
            Self::Questions => "题目管理",
            Self::PaperBuilder => "试卷组装",
            Self::Attempts => "阅卷记录",
            Self::Users => "账号管理",
        }
    }
}

fn navigation_link_class(active: NavigationItem, item: NavigationItem) -> Classes {
    classes!(
        APP_NAV_LINK,
        if active == item {
            "btn-primary"
        } else {
            "btn-ghost"
        }
    )
}

fn navigation_links(user: &UserIdentity, active: NavigationItem) -> Html {
    html! {
        <>
            <a class={navigation_link_class(active, NavigationItem::Practice)} href="/">{"练习"}</a>
            <a class={navigation_link_class(active, NavigationItem::Papers)} href="/papers">{"正式考试"}</a>
            if user.role == UserRole::Admin {
                <a class={navigation_link_class(active, NavigationItem::Questions)} href="/admin">{"题目管理"}</a>
                <a class={navigation_link_class(active, NavigationItem::PaperBuilder)} href="/admin/papers">{"试卷组装"}</a>
                <a class={navigation_link_class(active, NavigationItem::Attempts)} href="/admin/attempts">{"阅卷记录"}</a>
                <a class={navigation_link_class(active, NavigationItem::Users)} href="/admin/users">{"账号管理"}</a>
            }
        </>
    }
}

#[derive(Properties, PartialEq)]
struct AppShellProps {
    user: UserIdentity,
    eyebrow: AttrValue,
    title: AttrValue,
    subtitle: AttrValue,
    test_id: AttrValue,
    active: NavigationItem,
    children: Children,
}

#[function_component(AppShell)]
fn app_shell(props: &AppShellProps) -> Html {
    html! {
        <main class={APP_SHELL} data-testid={props.test_id.clone()}>
            <div class={APP_CONTAINER} data-testid="app-shell">
                <header class={APP_HEADER}>
                    <div class={APP_BRAND}>
                        <span class={APP_BRAND_MARK} aria-hidden="true">{"XQ"}</span>
                        <div class="min-w-0">
                            <p class="mb-1 text-[0.65rem] font-bold uppercase tracking-[0.2em] text-primary">{ props.eyebrow.clone() }</p>
                            <h1 class="text-2xl font-black tracking-tight text-base-content sm:text-3xl">{ props.title.clone() }</h1>
                            <p class="mt-1 max-w-2xl text-xs text-base-content/65 sm:text-sm">{ props.subtitle.clone() }</p>
                        </div>
                    </div>
                    <div class={APP_HEADER_ACTIONS}>
                        <UserNav user={props.user.clone()} />
                    </div>
                </header>

                <details class={APP_NAV_MOBILE} data-testid="mobile-nav">
                    <summary class={APP_NAV_TOGGLE} data-testid="mobile-nav-toggle">
                        <span>{ format!("当前页面：{}", props.active.label()) }</span>
                        <span class="text-primary" aria-hidden="true">{"菜单"}</span>
                    </summary>
                    <div class={APP_NAV_MENU} data-testid="app-nav-menu">
                        { navigation_links(&props.user, props.active) }
                    </div>
                </details>
                <nav class={APP_NAV_DESKTOP} aria-label="页面导航" data-testid="desktop-app-nav">
                    { navigation_links(&props.user, props.active) }
                </nav>

                <div class={APP_CONTENT}>
                    { for props.children.iter() }
                </div>
            </div>
        </main>
    }
}

#[derive(Properties, PartialEq)]
struct LoginPageProps {
    on_authenticated: Callback<UserIdentity>,
}

#[function_component(LoginPage)]
fn login_page(props: &LoginPageProps) -> Html {
    let login_config = use_state(|| LoginConfigState::Loading);
    {
        let login_config = login_config.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                match get_json::<LoginConfigResponse>(AUTH_CONFIG_ENDPOINT, "默认密码加载失败")
                    .await
                {
                    Ok(payload) => {
                        login_config.set(LoginConfigState::Ready(payload.initial_password))
                    }
                    Err(error) => login_config.set(LoginConfigState::Error(error)),
                }
            });
            || ()
        });
    }

    let username = use_state(String::new);
    let password = use_state(String::new);
    let show_password = use_state(|| false);
    let error = use_state(|| None::<String>);
    let submitting = use_state(|| false);

    let on_submit = {
        let username = username.clone();
        let password = password.clone();
        let error = error.clone();
        let submitting = submitting.clone();
        let on_authenticated = props.on_authenticated.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            if *submitting {
                return;
            }
            error.set(None);
            submitting.set(true);
            let username_value = (*username).clone();
            let password_value = (*password).clone();
            let error = error.clone();
            let submitting = submitting.clone();
            let on_authenticated = on_authenticated.clone();
            spawn_local(async move {
                let result = async {
                    let request = Request::post(AUTH_LOGIN_ENDPOINT)
                        .json(&LoginRequest {
                            username: username_value,
                            password: password_value,
                        })
                        .map_err(|error| error.to_string())?;
                    let response = request.send().await.map_err(|error| error.to_string())?;
                    if response.ok() {
                        response
                            .json::<AuthResponse>()
                            .await
                            .map(|payload| payload.user)
                            .map_err(|error| error.to_string())
                    } else {
                        let status = response.status();
                        let message = response
                            .json::<ApiErrorResponse>()
                            .await
                            .map(|payload| localized_auth_error(&payload.error))
                            .unwrap_or_else(|_| format!("登录失败（{status}）"));
                        Err(message)
                    }
                }
                .await;
                match result {
                    Ok(user) => on_authenticated.emit(user),
                    Err(message) => error.set(Some(message)),
                }
                submitting.set(false);
            });
        })
    };

    let default_password_notice = match &*login_config {
        LoginConfigState::Loading => html! {
            <div class="alert alert-info text-sm" aria-live="polite" data-testid="login-default-password">
                <span>{"正在加载学生账号默认密码…"}</span>
            </div>
        },
        LoginConfigState::Ready(initial_password) => html! {
            <div class="alert alert-info items-start text-sm" aria-live="polite" data-testid="login-default-password">
                <span>
                    <span class="font-semibold">{"学生账号默认密码："}</span>
                    <code class="rounded bg-base-100 px-1.5 py-0.5 font-mono">{ initial_password }</code>
                    <span class="ml-1">{"首次登录后必须修改。"}</span>
                </span>
            </div>
        },
        LoginConfigState::Error(_) => html! {
            <div class="alert alert-warning text-sm" role="alert" data-testid="login-default-password">
                <span>{"学生账号默认密码暂时无法加载，请联系管理员。"}</span>
            </div>
        },
    };

    html! {
        <main class={classes!(AUTH_SHELL, "bg-base-200")} data-testid="login-page">
            <section class="card w-full max-w-md border border-base-300 bg-base-100 shadow-xl">
                <div class="card-body">
                    <p class="text-xs font-bold uppercase tracking-[0.2em] text-primary">{"XIAOLUOQUIZ"}</p>
                    <h1 class="card-title mt-2 text-3xl">{"登录系统"}</h1>
                    <p class="text-sm text-base-content/65">{"使用管理员分发的账号登录。首次登录需要先设置个人密码。"}</p>
                    <div class="mt-4">{ default_password_notice }</div>
                    <form class="mt-4 grid gap-4" onsubmit={on_submit} data-testid="login-form">
                        <label class="form-control w-full">
                            <span class="mb-1 block min-h-5 text-sm font-semibold leading-5">{"登录账号"}</span>
                            <input class="input input-bordered w-full bg-base-100" autocomplete="username" value={(*username).clone()} oninput={text_input_callback(username.clone())} data-testid="login-username" />
                        </label>
                        <label class="form-control w-full">
                            <span class="mb-1 block min-h-5 text-sm font-semibold leading-5">{"密码"}</span>
                            <div class="relative w-full">
                                <input class="input input-bordered w-full bg-base-100 pr-16" type={if *show_password { "text" } else { "password" }} autocomplete="current-password" value={(*password).clone()} oninput={text_input_callback(password.clone())} data-testid="login-password" />
                                <button
                                    class="btn btn-ghost btn-sm absolute right-1 top-1/2 -translate-y-1/2"
                                    type="button"
                                    aria-label={if *show_password { "隐藏密码" } else { "显示密码" }}
                                    aria-pressed={show_password.to_string()}
                                    onclick={Callback::from({
                                        let show_password = show_password.clone();
                                        move |_| show_password.set(!*show_password)
                                    })}
                                    data-testid="toggle-password-visibility"
                                >
                                    { if *show_password { "隐藏" } else { "显示" } }
                                </button>
                            </div>
                        </label>
                        if let Some(message) = &*error {
                            <p class="text-sm text-error" role="alert" data-testid="login-error">{ message }</p>
                        }
                        <button class="btn btn-primary w-full" type="submit" disabled={*submitting} data-testid="login-submit">
                            { if *submitting { "登录中…" } else { "登录" } }
                        </button>
                    </form>
                </div>
            </section>
        </main>
    }
}

#[derive(Properties, PartialEq)]
struct ChangePasswordPageProps {
    user: UserIdentity,
    on_changed: Callback<UserIdentity>,
}

#[function_component(ChangePasswordPage)]
fn change_password_page(props: &ChangePasswordPageProps) -> Html {
    let new_password = use_state(String::new);
    let confirm_password = use_state(String::new);
    let error = use_state(|| None::<String>);
    let submitting = use_state(|| false);

    let on_submit = {
        let new_password = new_password.clone();
        let confirm_password = confirm_password.clone();
        let error = error.clone();
        let submitting = submitting.clone();
        let on_changed = props.on_changed.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            if *submitting {
                return;
            }
            error.set(None);
            if *new_password != *confirm_password {
                error.set(Some("两次输入的密码不一致".to_owned()));
                return;
            }
            submitting.set(true);
            let new_password_value = (*new_password).clone();
            let error = error.clone();
            let submitting = submitting.clone();
            let on_changed = on_changed.clone();
            spawn_local(async move {
                let result = async {
                    let request = Request::post(AUTH_CHANGE_PASSWORD_ENDPOINT)
                        .json(&ChangePasswordRequest {
                            new_password: new_password_value,
                        })
                        .map_err(|error| error.to_string())?;
                    let response = request.send().await.map_err(|error| error.to_string())?;
                    if response.ok() {
                        response
                            .json::<AuthResponse>()
                            .await
                            .map(|payload| payload.user)
                            .map_err(|error| error.to_string())
                    } else {
                        let status = response.status();
                        let message = response
                            .json::<ApiErrorResponse>()
                            .await
                            .map(|payload| payload.error)
                            .unwrap_or_else(|_| format!("修改密码失败（{status}）"));
                        Err(message)
                    }
                }
                .await;
                match result {
                    Ok(user) => on_changed.emit(user),
                    Err(message) => error.set(Some(message)),
                }
                submitting.set(false);
            });
        })
    };

    html! {
        <main class={classes!(AUTH_SHELL, "bg-base-200")} data-testid="change-password-page">
            <section class="card w-full max-w-md border border-base-300 bg-base-100 shadow-xl">
                <div class="card-body">
                    <p class="text-xs font-bold uppercase tracking-[0.2em] text-primary">{"XIAOLUOQUIZ / 安全设置"}</p>
                    <h1 class="card-title mt-2 text-3xl">{"首次登录，请修改密码"}</h1>
                    <p class="text-sm text-base-content/65">{ format!("当前账号：{}（{}）", props.user.username, props.user.display_name) }</p>
                    <p class="text-sm text-base-content/65">{"新密码至少 8 位，并且同时包含字母和数字；不能与账号或固定初始密码相同。"}</p>
                    <form class="mt-4 grid gap-4" onsubmit={on_submit} data-testid="change-password-form">
                        <label class="form-control">
                            <span class="label-text text-sm font-semibold">{"新密码"}</span>
                            <input class="input input-bordered bg-base-100" type="password" autocomplete="new-password" value={(*new_password).clone()} oninput={text_input_callback(new_password.clone())} data-testid="change-password-new" />
                        </label>
                        <label class="form-control">
                            <span class="label-text text-sm font-semibold">{"确认新密码"}</span>
                            <input class="input input-bordered bg-base-100" type="password" autocomplete="new-password" value={(*confirm_password).clone()} oninput={text_input_callback(confirm_password.clone())} data-testid="change-password-confirm" />
                        </label>
                        if let Some(message) = &*error {
                            <p class="text-sm text-error" role="alert" data-testid="change-password-error">{ message }</p>
                        }
                        <button class="btn btn-primary w-full" type="submit" disabled={*submitting} data-testid="change-password-submit">
                            { if *submitting { "保存中…" } else { "保存新密码" } }
                        </button>
                    </form>
                </div>
            </section>
        </main>
    }
}

#[derive(Properties, PartialEq)]
struct AccessDeniedPageProps {
    user: UserIdentity,
}

#[function_component(AccessDeniedPage)]
fn access_denied_page(props: &AccessDeniedPageProps) -> Html {
    html! {
        <main class={classes!(AUTH_SHELL, "bg-base-200")} data-testid="access-denied-page">
            <section class="card w-full max-w-lg border border-base-300 bg-base-100 shadow-xl">
                <div class="card-body">
                    <h1 class="card-title">{"没有管理权限"}</h1>
                    <p class="text-sm text-base-content/65">{ format!("账号 {} 当前是普通用户，不能访问管理员页面。", props.user.username) }</p>
                    <a class="btn btn-primary mt-4" href="/">{"返回练习"}</a>
                </div>
            </section>
        </main>
    }
}

#[derive(Properties, PartialEq)]
struct UserNavProps {
    user: UserIdentity,
}

#[function_component(UserNav)]
fn user_nav(props: &UserNavProps) -> Html {
    let logging_out = use_state(|| false);
    let error = use_state(|| None::<String>);
    let on_logout = {
        let logging_out = logging_out.clone();
        let error = error.clone();
        Callback::from(move |_| {
            if *logging_out {
                return;
            }
            logging_out.set(true);
            error.set(None);
            let logging_out = logging_out.clone();
            let error = error.clone();
            spawn_local(async move {
                match Request::post(AUTH_LOGOUT_ENDPOINT).send().await {
                    Ok(response) if response.ok() => {
                        if let Some(window) = web_sys::window() {
                            let _ = window.location().set_href("/login");
                        }
                    }
                    Ok(response) => error.set(Some(format!("退出失败（{}）", response.status()))),
                    Err(message) => error.set(Some(message.to_string())),
                }
                logging_out.set(false);
            });
        })
    };

    html! {
        <div class="flex min-w-0 flex-wrap items-center justify-end gap-2 text-sm">
            <span class="min-w-0 max-w-full truncate rounded-full bg-base-100 px-3 py-2 font-semibold" data-testid="current-user">
                { format!("{} · {}", props.user.display_name, role_label(props.user.role)) }
            </span>
            <button class="btn btn-ghost btn-sm shrink-0" type="button" onclick={on_logout} disabled={*logging_out} data-testid="logout">
                { if *logging_out { "退出中…" } else { "退出登录" } }
            </button>
            if let Some(message) = &*error {
                <span class="max-w-full break-words text-error" role="alert">{ message }</span>
            }
        </div>
    }
}

fn role_label(role: UserRole) -> &'static str {
    match role {
        UserRole::Admin => "管理员",
        UserRole::Student => "学生",
    }
}

fn localized_auth_error(error: &str) -> String {
    match error {
        "invalid credentials" => "账号或密码错误".to_owned(),
        "account is disabled" => "账号已被禁用".to_owned(),
        "authentication required" => "请先登录".to_owned(),
        other => other.to_owned(),
    }
}

#[derive(Properties, PartialEq)]
struct PracticeAppProps {
    user: UserIdentity,
}

type AnswerMap = BTreeMap<i64, AnswerPayload>;

fn practice_answer_storage_key(user_id: i64) -> String {
    format!("xiaoluoquiz.practice.answers.{user_id}")
}

fn exam_answer_storage_key(user_id: i64, attempt_id: i64) -> String {
    format!("xiaoluoquiz.exam.answers.{user_id}.{attempt_id}")
}

fn browser_storage() -> Option<Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn read_answer_map(key: &str) -> AnswerMap {
    browser_storage()
        .and_then(|storage| storage.get_item(key).ok().flatten())
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default()
}

fn answer_should_persist(answer: &AnswerPayload) -> bool {
    !matches!(answer, AnswerPayload::ShortAnswer { .. })
}

fn write_answer_map(key: &str, answers: &AnswerMap) {
    let persisted: AnswerMap = answers
        .iter()
        .filter(|(_, answer)| answer_should_persist(answer))
        .map(|(question_id, answer)| (*question_id, answer.clone()))
        .collect();
    let Ok(value) = serde_json::to_string(&persisted) else {
        return;
    };
    if let Some(storage) = browser_storage() {
        let _ = storage.set_item(key, &value);
    }
}

#[function_component(PracticeApp)]
fn practice_app(props: &PracticeAppProps) -> Html {
    let load_state = use_state(|| PracticeLoadState::Loading);
    let practice_stats_state = use_state(|| PracticeStatsLoadState::Loading);
    let question_type_filter = use_state(|| None::<QuestionType>);
    let question_bank_filter = use_state(|| None::<i64>);
    let question_navigation_expanded = use_state(|| false);
    let active_index = use_state(|| 0_usize);
    let numbers_per_row = use_state(|| 8_usize);
    let answer_storage_key = practice_answer_storage_key(props.user.id);
    let initial_answers = read_answer_map(&answer_storage_key);
    let answers = use_state(move || initial_answers);

    {
        let practice_stats_state = practice_stats_state.clone();
        let user_id = props.user.id;
        use_effect_with(user_id, move |_| {
            practice_stats_state.set(PracticeStatsLoadState::Loading);
            spawn_local(async move {
                match get_json::<PracticeStats>(PRACTICE_STATS_ENDPOINT, "整体正确率加载失败").await
                {
                    Ok(stats) => practice_stats_state.set(PracticeStatsLoadState::Ready(stats)),
                    Err(error) => practice_stats_state.set(PracticeStatsLoadState::Error(error)),
                }
            });
            || ()
        });
    }

    {
        let load_state = load_state.clone();
        let selected_bank = *question_bank_filter;
        use_effect_with(selected_bank, move |_| {
            load_state.set(PracticeLoadState::Loading);
            spawn_local(async move {
                let result: Result<(Vec<QuestionBank>, Vec<PublicQuestion>), String> = async {
                    let banks = get_json::<QuestionBankListResponse>(
                        QUESTION_BANKS_ENDPOINT,
                        "题库列表加载失败",
                    )
                    .await?
                    .items;
                    let endpoint = selected_bank.map_or_else(
                        || QUESTIONS_ENDPOINT.to_owned(),
                        |bank_id| format!("{QUESTIONS_ENDPOINT}?bank_id={bank_id}"),
                    );
                    let questions = get_json::<QuestionListResponse>(&endpoint, "题目列表加载失败")
                        .await?
                        .items;
                    Ok((banks, questions))
                }
                .await;
                match result {
                    Ok((banks, questions)) => {
                        load_state.set(PracticeLoadState::Ready { banks, questions })
                    }
                    Err(error) => load_state.set(PracticeLoadState::Error(error)),
                }
            });
            || ()
        });
    }

    {
        let answer_storage_key = answer_storage_key.clone();
        let answers_snapshot = (*answers).clone();
        use_effect_with(answers_snapshot, move |answers| {
            write_answer_map(&answer_storage_key, answers);
            || ()
        });
    }

    let on_question_type_filter = {
        let question_type_filter = question_type_filter.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            question_type_filter.set(select.value().parse::<QuestionType>().ok());
        })
    };
    let on_question_bank_filter = {
        let question_bank_filter = question_bank_filter.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            question_bank_filter.set(select.value().parse::<i64>().ok());
        })
    };
    let on_numbers_per_row = {
        let numbers_per_row = numbers_per_row.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            if let Ok(value) = select.value().parse::<usize>() {
                numbers_per_row.set(value.clamp(4, 12));
            }
        })
    };
    let on_question_navigation_toggle = {
        let question_navigation_expanded = question_navigation_expanded.clone();
        Callback::from(move |_| question_navigation_expanded.set(!*question_navigation_expanded))
    };
    let on_practice_stats_updated = {
        let practice_stats_state = practice_stats_state.clone();
        Callback::from(move |stats: PracticeStats| {
            practice_stats_state.set(PracticeStatsLoadState::Ready(stats));
        })
    };

    let question_type_value = question_type_filter
        .as_ref()
        .map_or("all", |question_type| question_type_value(*question_type));
    let question_bank_value = question_bank_filter
        .as_ref()
        .map_or_else(String::new, ToString::to_string);
    let banks = match &*load_state {
        PracticeLoadState::Ready { banks, .. } => banks.clone(),
        _ => Vec::new(),
    };
    let visible_questions = match &*load_state {
        PracticeLoadState::Ready { questions, .. } => questions
            .iter()
            .filter(|question| {
                question_type_filter
                    .as_ref()
                    .is_none_or(|kind| &question.question_type == kind)
            })
            .cloned()
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    let question_ids = visible_questions
        .iter()
        .map(|question| question.id)
        .collect::<Vec<_>>();
    {
        let active_index = active_index.clone();
        use_effect_with(question_ids, move |_| {
            active_index.set(0);
            || ()
        });
    }
    let current_index = if visible_questions.is_empty() {
        0
    } else {
        (*active_index).min(visible_questions.len() - 1)
    };
    let go_to_previous = {
        let active_index = active_index.clone();
        Callback::from(move |_| active_index.set((*active_index).saturating_sub(1)))
    };
    let go_to_next = {
        let active_index = active_index.clone();
        let question_count = visible_questions.len();
        Callback::from(move |_| {
            if question_count > 0 {
                active_index.set((*active_index + 1).min(question_count - 1));
            }
        })
    };

    let content = match &*load_state {
        PracticeLoadState::Loading => html! {
            <div class="grid gap-4" data-testid="loading-state">
                <div class="skeleton h-56 w-full rounded-box" />
            </div>
        },
        PracticeLoadState::Error(error) => html! {
            <div class="alert alert-error" data-testid="error-state">
                <span>{ format!("题目列表加载失败：{error}") }</span>
            </div>
        },
        PracticeLoadState::Ready { .. } if visible_questions.is_empty() => html! {
            <div class="rounded-box border border-dashed border-base-300 bg-base-100 p-10 text-center" data-testid="empty-state">
                <p class="text-lg font-bold">{"暂时没有可练习的题目"}</p>
                <p class="mt-2 text-sm text-base-content/60">{"选择其他题库或题型，或者等待管理员发布题目。"}</p>
            </div>
        },
        PracticeLoadState::Ready { .. } => {
            let question = visible_questions[current_index].clone();
            let question_id = question.id;
            let answer = answers.get(&question_id).cloned();
            let on_answer = {
                let answers = answers.clone();
                Callback::from(move |answer: AnswerPayload| {
                    let mut values = (*answers).clone();
                    values.insert(question_id, answer);
                    answers.set(values);
                })
            };
            html! {
                <div class="grid gap-5">
                    <section class="card border border-base-300 bg-base-100 shadow-sm" data-testid="practice-question-numbers">
                        <div class="card-body gap-4 p-4 sm:p-5">
                            <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                                <div>
                                    <h2 class="font-bold">{"题目导航"}</h2>
                                    <p class="mt-1 text-xs text-base-content/60">{"点击展开题号导航；深色圆形按钮表示已经填写答案。"}</p>
                                    { practice_accuracy_view(&practice_stats_state) }
                                </div>
                                <div class="flex flex-wrap items-center gap-2">
                                    <label class="form-control w-full sm:w-40">
                                        <span class="label-text text-xs font-semibold">{"每行题号数量"}</span>
                                        <select class="select select-bordered select-sm bg-base-100" value={numbers_per_row.to_string()} onchange={on_numbers_per_row.clone()} data-testid="practice-question-numbers-per-row">
                                            { for [4_usize, 6, 8, 10, 12].into_iter().map(|value| html! {
                                                <option value={value.to_string()} selected={*numbers_per_row == value}>{ value }</option>
                                            }) }
                                        </select>
                                    </label>
                                    <button
                                        class="btn btn-outline btn-sm"
                                        type="button"
                                        onclick={on_question_navigation_toggle.clone()}
                                        aria-expanded={(*question_navigation_expanded).to_string()}
                                        aria-controls="practice-question-navigation-panel"
                                        aria-label={if *question_navigation_expanded { "收起题目导航" } else { "展开题目导航" }}
                                        data-testid="practice-question-numbers-toggle"
                                    >
                                        { if *question_navigation_expanded { "收起" } else { "展开" } }
                                    </button>
                                </div>
                            </div>
                            if *question_navigation_expanded {
                                <div
                                    id="practice-question-navigation-panel"
                                    class="grid gap-2"
                                    style={format!("grid-template-columns: repeat({}, minmax(0, 1fr));", (*numbers_per_row).clamp(4, 12))}
                                    data-testid="practice-question-navigation-panel"
                                >
                                    { for visible_questions.iter().enumerate().map(|(index, question)| {
                                        let answered = answers
                                            .get(&question.id)
                                            .is_some_and(answer_is_answered);
                                        let active = index == current_index;
                                        let active_index = active_index.clone();
                                        let class = classes!(
                                            "btn",
                                            "btn-circle",
                                            "btn-sm",
                                            if answered { "btn-neutral" } else { "btn-outline" },
                                            if active { "ring-2" } else { "" },
                                            if active { "ring-primary" } else { "" },
                                        );
                                        html! {
                                            <button
                                                class={class}
                                                type="button"
                                                aria-label={format!("第 {} 题", index + 1)}
                                                aria-current={active.then_some("step")}
                                                data-testid={format!("practice-question-number-{index}")}
                                                data-answered={answered.to_string()}
                                                onclick={Callback::from(move |_| active_index.set(index))}
                                            >
                                                { index + 1 }
                                            </button>
                                        }
                                    }) }
                                </div>
                            }
                        </div>
                    </section>
                    <div class="grid gap-5" data-testid="question-list">
                        <QuestionCard
                            key={question_id.to_string()}
                            question={question}
                            answer={answer}
                            on_answer={on_answer}
                            on_practice_stats_updated={on_practice_stats_updated.clone()}
                        />
                    </div>
                    <div class="flex flex-wrap items-center justify-between gap-3">
                        <button class="btn btn-outline" type="button" onclick={go_to_previous.clone()} disabled={current_index == 0} data-testid="practice-previous">{"上一题"}</button>
                        <span class="text-sm font-semibold text-base-content/65">{ format!("第 {} / {} 题", current_index + 1, visible_questions.len()) }</span>
                        <button class="btn btn-primary" type="button" onclick={go_to_next.clone()} disabled={current_index + 1 >= visible_questions.len()} data-testid="practice-next">{"下一题"}</button>
                    </div>
                </div>
            }
        }
    };

    html! {
        <AppShell
            user={props.user.clone()}
            eyebrow="XIAOLUOQUIZ"
            title="开始练习"
            subtitle="先选择题库和题型，再按题号逐题练习；非简答题答案会保存在当前账号的浏览器中。"
            test_id="practice-page"
            active={NavigationItem::Practice}
        >
            <section class={APP_TOOLBAR} aria-label="练习筛选" data-testid="practice-toolbar">
                <div class="min-w-0">
                    <p class="text-sm font-bold">{"选择练习内容"}</p>
                    <p class="mt-1 text-xs text-base-content/60">{"题库筛选由服务端执行，只返回已发布题目；题目本身不携带试卷分值。"}</p>
                </div>
                <div class="grid w-full gap-3 sm:grid-cols-2 lg:max-w-xl">
                    <label class="form-control w-full">
                        <span class="label-text text-sm font-semibold">{"题库"}</span>
                        <select class="select select-bordered w-full bg-base-100" value={question_bank_value.clone()} onchange={on_question_bank_filter} data-testid="question-bank-filter">
                            <option value="" selected={question_bank_filter.is_none()}>{"全部题库"}</option>
                            { for banks.iter().map(|bank| html! {
                                <option value={bank.id.to_string()} selected={Some(bank.id) == *question_bank_filter}>{ &bank.name }</option>
                            }) }
                        </select>
                    </label>
                    <label class="form-control w-full">
                        <span class="label-text text-sm font-semibold">{"题型"}</span>
                        <select class="select select-bordered w-full bg-base-100" value={question_type_value} onchange={on_question_type_filter} data-testid="question-filter">
                            <option value="all" selected={question_type_filter.is_none()}>{"全部题型"}</option>
                            <option value="single_choice" selected={question_type_value == "single_choice"}>{"选择题"}</option>
                            <option value="multiple_choice" selected={question_type_value == "multiple_choice"}>{"多选题"}</option>
                            <option value="fill_blank" selected={question_type_value == "fill_blank"}>{"填空题"}</option>
                            <option value="true_false" selected={question_type_value == "true_false"}>{"判断题"}</option>
                            <option value="short_answer" selected={question_type_value == "short_answer"}>{"简答题"}</option>
                        </select>
                    </label>
                </div>
            </section>
            { content }
        </AppShell>
    }
}

#[derive(Properties, PartialEq)]
struct QuestionCardProps {
    question: PublicQuestion,
    answer: Option<AnswerPayload>,
    on_answer: Callback<AnswerPayload>,
    on_practice_stats_updated: Callback<PracticeStats>,
}

fn practice_accuracy_view(state: &PracticeStatsLoadState) -> Html {
    match state {
        PracticeStatsLoadState::Loading => html! {
            <div class="mt-3 flex min-h-12 items-center rounded-box bg-base-200/60 px-3 py-2 text-sm text-base-content/65" data-testid="practice-accuracy" data-status="loading">
                {"整体正确率加载中…"}
            </div>
        },
        PracticeStatsLoadState::Error(error) => html! {
            <div class="alert alert-error mt-3 min-h-12 py-2 text-sm" data-testid="practice-accuracy" data-status="error" role="alert">
                { format!("整体正确率加载失败：{error}") }
            </div>
        },
        PracticeStatsLoadState::Ready(stats) => {
            let accuracy = stats.accuracy_percent.unwrap_or(0.0);
            html! {
                <div
                    class="mt-3 flex min-w-0 flex-wrap items-baseline gap-x-3 gap-y-1 rounded-box bg-base-200/60 px-3 py-2"
                    data-testid="practice-accuracy"
                    data-status="ready"
                    data-answered-count={stats.answered_count.to_string()}
                    data-correct-count={stats.correct_count.to_string()}
                    data-accuracy-percent={format!("{accuracy:.2}")}
                >
                    <span class="text-xs font-semibold text-base-content/65">{"整体正确率"}</span>
                    <strong class="text-lg text-primary">{ format!("{accuracy:.2}%") }</strong>
                    <span class="text-sm text-base-content/75">{ format!("答对 {} / 已作答 {} 题", stats.correct_count, stats.answered_count) }</span>
                    <span class="text-xs text-base-content/55">{"简答题不计入"}</span>
                    if stats.answered_count == 0 {
                        <span class="basis-full text-xs text-base-content/55">{"暂无计入正确率的作答"}</span>
                    }
                </div>
            }
        }
    }
}

#[function_component(QuestionCard)]
fn question_card(props: &QuestionCardProps) -> Html {
    let question = &props.question;
    let validation_error = use_state(|| None::<String>);
    let result = use_state(|| None::<CheckAnswerResponse>);
    let submitting = use_state(|| false);
    let on_answer = {
        let on_answer = props.on_answer.clone();
        let validation_error = validation_error.clone();
        let result = result.clone();
        Callback::from(move |answer: AnswerPayload| {
            validation_error.set(None);
            result.set(None);
            on_answer.emit(answer);
        })
    };

    let on_submit = {
        let answer = props.answer.clone();
        let validation_error = validation_error.clone();
        let result = result.clone();
        let submitting = submitting.clone();
        let on_practice_stats_updated = props.on_practice_stats_updated.clone();
        let question_type = question.question_type;
        let question_id = question.id;

        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            validation_error.set(None);
            result.set(None);
            let answer = match answer_for_submission(question_type, answer.as_ref()) {
                Ok(answer) => answer,
                Err(message) => {
                    validation_error.set(Some(message));
                    return;
                }
            };

            submitting.set(true);
            let validation_error = validation_error.clone();
            let result = result.clone();
            let submitting = submitting.clone();
            let on_practice_stats_updated = on_practice_stats_updated.clone();
            spawn_local(async move {
                let request = Request::post(&format!("/api/v1/questions/{question_id}/check"))
                    .json(&CheckAnswerRequest { answer });
                match request {
                    Ok(request) => match request.send().await {
                        Ok(response) if response.ok() => {
                            match response.json::<CheckAnswerResponse>().await {
                                Ok(payload) => {
                                    on_practice_stats_updated.emit(payload.practice_stats.clone());
                                    result.set(Some(payload));
                                }
                                Err(error) => validation_error.set(Some(network_error_message(
                                    "读取判题结果失败",
                                    error.to_string(),
                                ))),
                            }
                        }
                        Ok(response) => validation_error
                            .set(Some(read_api_error(response, "提交答案失败").await)),
                        Err(error) => validation_error.set(Some(network_error_message(
                            "提交答案失败",
                            error.to_string(),
                        ))),
                    },
                    Err(error) => validation_error.set(Some(network_error_message(
                        "提交答案失败",
                        error.to_string(),
                    ))),
                }
                submitting.set(false);
            });
        })
    };

    let type_label = question_type_label(question.question_type);
    let result_view = (*result).as_ref().map(|response| {
        html! { <ResultPanel response={response.clone()} /> }
    });

    html! {
        <article class="card min-w-0 overflow-hidden border border-base-300 bg-base-100 shadow-sm" data-testid={format!("question-{}", question.id)}>
            <div class="card-body gap-5">
                <div class="flex min-w-0 flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                    <div class="min-w-0">
                        <div class="flex flex-wrap items-center gap-2">
                            <span class="badge badge-primary badge-outline">{ type_label }</span>
                            <span class="badge badge-ghost">{ &question.question_bank_name }</span>
                        </div>
                        <h2 class="mt-3 break-words text-xl font-bold leading-relaxed">{ &question.stem }</h2>
                    </div>
                </div>

                <form class="grid gap-4" onsubmit={on_submit}>
                    { render_practice_answer_control(question, props.answer.as_ref(), on_answer) }
                    if let Some(message) = &*validation_error {
                        <p class="text-sm text-error" role="alert">{ message }</p>
                    }
                    <button class="btn btn-primary w-full sm:w-auto sm:justify-self-start" type="submit" disabled={*submitting} data-testid="submit-answer">
                        { if *submitting { "提交中…" } else { "提交答案" } }
                    </button>
                </form>

                { result_view }
            </div>
        </article>
    }
}

fn render_practice_answer_control(
    question: &PublicQuestion,
    answer: Option<&AnswerPayload>,
    on_answer: Callback<AnswerPayload>,
) -> Html {
    match question.question_type {
        QuestionType::SingleChoice => html! {
            <div class="grid gap-3" role="radiogroup" aria-label="选择题选项">
                { for question.options.iter().map(|option| {
                    let on_answer = on_answer.clone();
                    let key = option.key.clone();
                    let input_id = format!("practice-question-{}-{}", question.id, option.key);
                    let checked = saved_single_value(answer) .as_deref() == Some(option.key.as_str());
                    html! {
                        <label class="flex min-w-0 cursor-pointer items-center gap-3 rounded-box border border-base-300 bg-base-200/50 p-4 transition-colors has-[:checked]:border-primary has-[:checked]:bg-primary/10" for={input_id.clone()}>
                            <input
                                id={input_id}
                                class="radio radio-primary"
                                type="radio"
                                name={format!("practice-question-{}", question.id)}
                                value={option.key.clone()}
                                checked={checked}
                                onchange={Callback::from(move |_| on_answer.emit(AnswerPayload::SingleChoice { option_key: key.clone() }))}
                            />
                            <span class="min-w-0 break-words font-medium">{ format!("{}．{}", option.key, option.text) }</span>
                        </label>
                    }
                }) }
            </div>
        },
        QuestionType::MultipleChoice => {
            let selected = saved_multiple_values(answer);
            html! {
                <div class="grid gap-3" role="group" aria-label="多选题选项">
                    { for question.options.iter().map(|option| {
                        let on_answer = on_answer.clone();
                        let key = option.key.clone();
                        let key_for_change = key.clone();
                        let input_id = format!("practice-question-{}-{}", question.id, option.key);
                        let checked = selected.iter().any(|value| value == &key);
                        let selected = selected.clone();
                        html! {
                            <label class="flex min-w-0 cursor-pointer items-center gap-3 rounded-box border border-base-300 bg-base-200/50 p-4 transition-colors has-[:checked]:border-primary has-[:checked]:bg-primary/10" for={input_id.clone()}>
                                <input
                                    id={input_id}
                                    class="checkbox checkbox-primary"
                                    type="checkbox"
                                    name={format!("practice-question-{}", question.id)}
                                    value={key.clone()}
                                    checked={checked}
                                    onchange={Callback::from(move |event: Event| {
                                        let input: HtmlInputElement = event.target_unchecked_into();
                                        let mut values = selected.clone();
                                        if input.checked() {
                                            if !values.iter().any(|value| value == &key_for_change) {
                                                values.push(key_for_change.clone());
                                            }
                                        } else {
                                            values.retain(|value| value != &key_for_change);
                                        }
                                        on_answer.emit(AnswerPayload::MultipleChoice { option_keys: values });
                                    })}
                                />
                                <span class="min-w-0 break-words font-medium">{ format!("{}．{}", option.key, option.text) }</span>
                            </label>
                        }
                    }) }
                </div>
            }
        }
        QuestionType::TrueFalse => html! {
            <div class="grid gap-3 sm:grid-cols-2" role="radiogroup" aria-label="判断题选项">
                { for [("true", "正确"), ("false", "错误")].into_iter().map(|(value, label)| {
                    let on_answer = on_answer.clone();
                    let value_for_change = value.to_owned();
                    let input_id = format!("practice-question-{}-{}", question.id, value);
                    let checked = saved_single_value(answer).as_deref() == Some(value);
                    html! {
                        <label class="flex min-w-0 cursor-pointer items-center gap-3 rounded-box border border-base-300 bg-base-200/50 p-4 transition-colors has-[:checked]:border-primary has-[:checked]:bg-primary/10" for={input_id.clone()}>
                            <input
                                id={input_id}
                                class="radio radio-primary"
                                type="radio"
                                name={format!("practice-question-{}", question.id)}
                                value={value}
                                checked={checked}
                                onchange={Callback::from(move |_| on_answer.emit(AnswerPayload::TrueFalse { value: value_for_change == "true" }))}
                            />
                            <span class="font-medium">{ label }</span>
                        </label>
                    }
                }) }
            </div>
        },
        QuestionType::FillBlank => {
            let values = saved_blank_values(answer, question.blank_count.max(1) as usize);
            html! {
                <div class="grid gap-3">
                    { for values.iter().enumerate().map(|(index, value)| {
                        let on_answer = on_answer.clone();
                        let values = values.clone();
                        html! {
                            <label class="form-control" for={format!("practice-question-{}-blank-{}", question.id, index)}>
                                <span class="label-text text-sm font-semibold">{ format!("第 {} 空", index + 1) }</span>
                                <input
                                    id={format!("practice-question-{}-blank-{}", question.id, index)}
                                    class="input input-bordered bg-base-100"
                                    value={value.clone()}
                                    oninput={Callback::from(move |event: InputEvent| {
                                        let input: HtmlInputElement = event.target_unchecked_into();
                                        let mut next_values = values.clone();
                                        next_values[index] = input.value();
                                        on_answer.emit(AnswerPayload::FillBlank { values: next_values });
                                    })}
                                />
                            </label>
                        }
                    }) }
                </div>
            }
        }
        QuestionType::ShortAnswer => {
            let on_answer = on_answer.clone();
            let value = saved_short_value(answer);
            html! {
                <label class="form-control" for={format!("practice-question-{}-short-answer", question.id)}>
                    <span class="label-text text-sm font-semibold">{"你的答案"}</span>
                    <textarea
                        id={format!("practice-question-{}-short-answer", question.id)}
                        class="textarea textarea-bordered min-h-32 bg-base-100"
                        value={value}
                        oninput={Callback::from(move |event: InputEvent| {
                            let input: HtmlTextAreaElement = event.target_unchecked_into();
                            on_answer.emit(AnswerPayload::ShortAnswer { text: input.value() });
                        })}
                    />
                </label>
            }
        }
    }
}

fn answer_for_submission(
    question_type: QuestionType,
    answer: Option<&AnswerPayload>,
) -> Result<AnswerPayload, String> {
    let Some(answer) = answer else {
        return Err(match question_type {
            QuestionType::SingleChoice => "请选择一个选项",
            QuestionType::MultipleChoice => "请至少选择一个选项",
            QuestionType::TrueFalse => "请选择“正确”或“错误”",
            QuestionType::FillBlank => "请填写所有空格",
            QuestionType::ShortAnswer => "请输入答案",
        }
        .to_owned());
    };
    match (question_type, answer) {
        (QuestionType::SingleChoice, AnswerPayload::SingleChoice { option_key })
            if !option_key.trim().is_empty() =>
        {
            Ok(answer.clone())
        }
        (QuestionType::MultipleChoice, AnswerPayload::MultipleChoice { option_keys })
            if !option_keys.is_empty() =>
        {
            Ok(answer.clone())
        }
        (QuestionType::TrueFalse, AnswerPayload::TrueFalse { .. }) => Ok(answer.clone()),
        (QuestionType::FillBlank, AnswerPayload::FillBlank { values })
            if !values.iter().any(|value| value.trim().is_empty()) =>
        {
            Ok(answer.clone())
        }
        (QuestionType::ShortAnswer, AnswerPayload::ShortAnswer { text })
            if !text.trim().is_empty() =>
        {
            Ok(answer.clone())
        }
        (QuestionType::SingleChoice, _) => Err("请选择一个选项".to_owned()),
        (QuestionType::MultipleChoice, _) => Err("请至少选择一个选项".to_owned()),
        (QuestionType::TrueFalse, _) => Err("请选择“正确”或“错误”".to_owned()),
        (QuestionType::FillBlank, _) => Err("请填写所有空格".to_owned()),
        (QuestionType::ShortAnswer, _) => Err("请输入答案".to_owned()),
    }
}

fn answer_is_answered(answer: &AnswerPayload) -> bool {
    match answer {
        AnswerPayload::SingleChoice { option_key } => !option_key.trim().is_empty(),
        AnswerPayload::MultipleChoice { option_keys } => !option_keys.is_empty(),
        AnswerPayload::FillBlank { values } => values.iter().any(|value| !value.trim().is_empty()),
        AnswerPayload::TrueFalse { .. } => true,
        AnswerPayload::ShortAnswer { text } => !text.trim().is_empty(),
    }
}

#[derive(Properties, PartialEq)]
struct ResultPanelProps {
    response: CheckAnswerResponse,
}

#[function_component(ResultPanel)]
fn result_panel(props: &ResultPanelProps) -> Html {
    let response = &props.response;
    let (alert_class, title) = match response.status {
        EvaluationStatus::Correct => ("alert-success", "回答正确"),
        EvaluationStatus::Incorrect => ("alert-error", "回答错误"),
        EvaluationStatus::NeedsReview => ("alert-warning", "等待批改"),
    };

    html! {
        <div class={classes!("alert", alert_class, "items-start")} data-testid="answer-result">
            <div class="grid gap-2">
                <strong>{ title }</strong>
                <span>{ match response.status {
                    EvaluationStatus::Correct => "服务端判断：回答正确",
                    EvaluationStatus::Incorrect => "服务端判断：回答错误",
                    EvaluationStatus::NeedsReview => "这道题需要人工批改。",
                } }</span>
                if let Some(explanation) = &response.explanation {
                    <p class="text-sm opacity-80">{ explanation }</p>
                }
                <details class="text-sm">
                    <summary class="cursor-pointer font-semibold">{"查看参考答案"}</summary>
                    <p class="mt-2 opacity-80"><CorrectAnswerView answer={response.correct_answer.clone()} /></p>
                </details>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct CorrectAnswerViewProps {
    answer: CorrectAnswer,
}

#[function_component(CorrectAnswerView)]
fn correct_answer_view(props: &CorrectAnswerViewProps) -> Html {
    match &props.answer {
        CorrectAnswer::SingleChoice { option_key } => html! { <>{ option_key }</> },
        CorrectAnswer::MultipleChoice { option_keys } => html! { <>{ option_keys.join("、") }</> },
        CorrectAnswer::FillBlank { accepted } => html! {
            <>{ for accepted.iter().map(|values| html! { <span class="mr-2">{ values.join(" / ") }</span> }) }</>
        },
        CorrectAnswer::TrueFalse { value } => {
            html! { <>{ if *value { "正确" } else { "错误" } }</> }
        }
        CorrectAnswer::ShortAnswer { reference, .. } => html! { <>{ reference }</> },
    }
}

fn question_type_label(question_type: QuestionType) -> &'static str {
    match question_type {
        QuestionType::SingleChoice => "选择题",
        QuestionType::MultipleChoice => "多选题",
        QuestionType::FillBlank => "填空题",
        QuestionType::TrueFalse => "判断题",
        QuestionType::ShortAnswer => "简答题",
    }
}

fn question_type_value(question_type: QuestionType) -> &'static str {
    match question_type {
        QuestionType::SingleChoice => "single_choice",
        QuestionType::MultipleChoice => "multiple_choice",
        QuestionType::FillBlank => "fill_blank",
        QuestionType::TrueFalse => "true_false",
        QuestionType::ShortAnswer => "short_answer",
    }
}

#[derive(Properties, PartialEq)]
struct AdminUsersAppProps {
    user: UserIdentity,
}

#[function_component(AdminUsersApp)]
fn admin_users_app(props: &AdminUsersAppProps) -> Html {
    let load_state = use_state(|| UserLoadState::Loading);
    let refresh_counter = use_state(|| 0_u32);

    {
        let load_state = load_state.clone();
        let refresh_counter = *refresh_counter;
        use_effect_with(refresh_counter, move |_| {
            spawn_local(async move {
                let result = async {
                    let users =
                        get_json::<UserListResponse>(ADMIN_USERS_ENDPOINT, "账号列表加载失败")
                            .await?;
                    let classes =
                        get_json::<ClassListResponse>(ADMIN_CLASSES_ENDPOINT, "班级列表加载失败")
                            .await?;
                    Ok::<_, String>((users.items, classes.items))
                }
                .await;
                match result {
                    Ok((users, classes)) => load_state.set(UserLoadState::Ready { users, classes }),
                    Err(error) => load_state.set(UserLoadState::Error(error)),
                }
            });
            || ()
        });
    }

    let refresh = {
        let refresh_counter = refresh_counter.clone();
        Callback::from(move |_| refresh_counter.set(refresh_counter.wrapping_add(1)))
    };
    let classes = match &*load_state {
        UserLoadState::Ready { classes, .. } => classes.clone(),
        _ => Vec::new(),
    };

    html! {
        <AppShell
            user={props.user.clone()}
            eyebrow="XIAOLUOQUIZ / ADMIN"
            title="账号管理"
            subtitle="管理员维护班级和用户账号。新账号使用登录页显示的固定初始密码，首次登录必须修改。"
            test_id="admin-users-page"
            active={NavigationItem::Users}
        >
            <div class="grid gap-6 xl:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]">
                <div class="grid gap-6">
                    <AdminClassForm on_saved={refresh.clone()} />
                    <AdminUserForm classes={classes} on_saved={refresh.clone()} />
                </div>
                <AdminUserList state={(*load_state).clone()} on_changed={refresh} />
            </div>
        </AppShell>
    }
}

#[derive(Properties, PartialEq)]
struct AdminClassFormProps {
    on_saved: Callback<()>,
}

#[function_component(AdminClassForm)]
fn admin_class_form(props: &AdminClassFormProps) -> Html {
    let name = use_state(String::new);
    let error = use_state(|| None::<String>);
    let saving = use_state(|| false);
    let on_submit = {
        let name = name.clone();
        let error = error.clone();
        let saving = saving.clone();
        let on_saved = props.on_saved.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            if *saving {
                return;
            }
            error.set(None);
            if name.trim().is_empty() {
                error.set(Some("班级名称不能为空".to_owned()));
                return;
            }
            saving.set(true);
            let name_value = (*name).clone();
            let name = name.clone();
            let error = error.clone();
            let saving = saving.clone();
            let on_saved = on_saved.clone();
            spawn_local(async move {
                let result: Result<ClassGroup, String> = post_json(
                    ADMIN_CLASSES_ENDPOINT,
                    &CreateClassInput { name: name_value },
                    "创建班级失败",
                )
                .await;
                match result {
                    Ok(_) => {
                        name.set(String::new());
                        on_saved.emit(());
                    }
                    Err(message) => error.set(Some(message)),
                }
                saving.set(false);
            });
        })
    };
    html! {
        <section class="card border border-base-300 bg-base-100 shadow-sm" data-testid="admin-class-form">
            <div class="card-body">
                <h2 class="card-title">{"班级管理"}</h2>
                <p class="mt-1 text-sm text-base-content/60">{"先创建班级，创建学生账号时再从下拉列表选择。"}</p>
                <form class="mt-4 grid gap-4" onsubmit={on_submit}>
                    <label class="form-control">
                        <span class="label-text text-sm font-semibold">{"班级名称"}</span>
                        <input class="input input-bordered bg-base-100" value={(*name).clone()} oninput={text_input_callback(name.clone())} data-testid="admin-class-name" />
                    </label>
                    if let Some(message) = &*error { <p class="text-sm text-error" role="alert" data-testid="admin-class-form-error">{ message }</p> }
                    <button class="btn btn-outline w-full" type="submit" disabled={*saving} data-testid="admin-class-save">{ if *saving { "创建中…" } else { "创建班级" } }</button>
                </form>
            </div>
        </section>
    }
}

#[derive(Properties, PartialEq)]
struct AdminUserFormProps {
    classes: Vec<ClassGroup>,
    on_saved: Callback<()>,
}

#[function_component(AdminUserForm)]
fn admin_user_form(props: &AdminUserFormProps) -> Html {
    let username = use_state(String::new);
    let display_name = use_state(String::new);
    let role = use_state(UserRole::default);
    let student_number = use_state(String::new);
    let class_id = use_state(|| None::<i64>);
    let error = use_state(|| None::<String>);
    let saving = use_state(|| false);
    let on_role = {
        let role = role.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            role.set(if select.value() == "admin" {
                UserRole::Admin
            } else {
                UserRole::Student
            });
        })
    };
    let on_class = {
        let class_id = class_id.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            class_id.set(select.value().parse::<i64>().ok());
        })
    };
    let on_submit = {
        let username = username.clone();
        let display_name = display_name.clone();
        let role = role.clone();
        let student_number = student_number.clone();
        let class_id = class_id.clone();
        let error = error.clone();
        let saving = saving.clone();
        let on_saved = props.on_saved.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            if *saving {
                return;
            }
            error.set(None);
            saving.set(true);
            let input = CreateUserInput {
                username: (*username).clone(),
                display_name: (*display_name).clone(),
                role: *role,
                student_number: non_empty((*student_number).clone()),
                class_id: *class_id,
            };
            let username = username.clone();
            let display_name = display_name.clone();
            let student_number = student_number.clone();
            let class_id = class_id.clone();
            let error = error.clone();
            let saving = saving.clone();
            let on_saved = on_saved.clone();
            spawn_local(async move {
                let result: Result<UserIdentity, String> =
                    post_json(ADMIN_USERS_ENDPOINT, &input, "创建账号失败").await;
                match result {
                    Ok(_) => {
                        username.set(String::new());
                        display_name.set(String::new());
                        student_number.set(String::new());
                        class_id.set(None);
                        on_saved.emit(());
                    }
                    Err(message) => error.set(Some(message)),
                }
                saving.set(false);
            });
        })
    };
    let selected_class_value = (*class_id).map_or_else(String::new, |value| value.to_string());
    html! {
        <section class="card border border-base-300 bg-base-100 shadow-sm" data-testid="admin-user-form">
            <div class="card-body">
                <h2 class="card-title">{"创建账号"}</h2>
                <p class="mt-1 text-sm text-base-content/60">{"新账号使用登录页显示的固定初始密码，首次登录必须修改。"}</p>
                <form class="mt-4 grid gap-4" onsubmit={on_submit}>
                    <label class="form-control"><span class="label-text text-sm font-semibold">{"登录账号"}</span><input class="input input-bordered bg-base-100" value={(*username).clone()} oninput={text_input_callback(username.clone())} data-testid="admin-user-username" /></label>
                    <label class="form-control"><span class="label-text text-sm font-semibold">{"显示名称"}</span><input class="input input-bordered bg-base-100" value={(*display_name).clone()} oninput={text_input_callback(display_name.clone())} data-testid="admin-user-display-name" /></label>
                    <label class="form-control"><span class="label-text text-sm font-semibold">{"角色"}</span><select class="select select-bordered bg-base-100" value={role_label_value(*role)} onchange={on_role} data-testid="admin-user-role"><option value="student" selected={*role == UserRole::Student}>{"学生"}</option><option value="admin" selected={*role == UserRole::Admin}>{"管理员"}</option></select></label>
                    <label class="form-control"><span class="label-text text-sm font-semibold">{"学号（可选）"}</span><input class="input input-bordered bg-base-100" value={(*student_number).clone()} oninput={text_input_callback(student_number.clone())} data-testid="admin-user-student-number" /></label>
                    <label class="form-control"><span class="label-text text-sm font-semibold">{"班级（可选）"}</span><select class="select select-bordered bg-base-100" value={selected_class_value} onchange={on_class} data-testid="admin-user-class"><option value="" selected={class_id.is_none()}>{"未选择班级"}</option>{ for props.classes.iter().map(|class_group| html! { <option value={class_group.id.to_string()} selected={Some(class_group.id) == *class_id}>{ &class_group.name }</option> }) }</select></label>
                    if let Some(message) = &*error { <p class="text-sm text-error" role="alert" data-testid="admin-user-form-error">{ message }</p> }
                    <button class="btn btn-secondary w-full" type="submit" disabled={*saving} data-testid="admin-user-save">{ if *saving { "创建中…" } else { "创建账号" } }</button>
                </form>
            </div>
        </section>
    }
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn parse_optional_datetime_local(value: &str) -> Result<Option<String>, ()> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let value = if value.len() == 16 {
        format!("{value}:00")
    } else {
        value.to_owned()
    };
    let shanghai_value = format!("{value}{SHANGHAI_OFFSET}");
    if !js_sys::Date::parse(&shanghai_value).is_finite() {
        return Err(());
    }
    Ok(Some(shanghai_value))
}

fn shanghai_datetime_label(value: &str) -> String {
    let timestamp = js_sys::Date::parse(value);
    if !timestamp.is_finite() {
        return value.to_owned();
    }
    let date = js_sys::Date::new(&JsValue::from_f64(timestamp + SHANGHAI_OFFSET_MILLISECONDS));
    let Some(iso) = date.to_iso_string().as_string() else {
        return value.to_owned();
    };
    match (iso.get(..10), iso.get(11..19)) {
        (Some(date), Some(time)) => format!("{date} {time}"),
        _ => value.to_owned(),
    }
}

fn role_label_value(role: UserRole) -> &'static str {
    match role {
        UserRole::Admin => "admin",
        UserRole::Student => "student",
    }
}

#[derive(Properties, PartialEq)]
struct AdminUserListProps {
    state: UserLoadState,
    on_changed: Callback<()>,
}

#[function_component(AdminUserList)]
fn admin_user_list(props: &AdminUserListProps) -> Html {
    let content = match &props.state {
        UserLoadState::Loading => {
            html! { <div class="grid gap-3" data-testid="admin-users-loading-state">{ for (0..3).map(|_| html! { <div class="skeleton h-28 w-full rounded-box" /> }) }</div> }
        }
        UserLoadState::Error(error) => {
            html! { <div class="alert alert-error" data-testid="admin-users-error-state">{ error }</div> }
        }
        UserLoadState::Ready { users, .. } if users.is_empty() => {
            html! { <div class="rounded-box border border-dashed border-base-300 p-10 text-center" data-testid="admin-users-empty-state"><p class="font-bold">{"还没有账号"}</p></div> }
        }
        UserLoadState::Ready { users, .. } => {
            html! { <div class="grid gap-3" data-testid="admin-user-list">{ for users.iter().map(|user| html! { <AdminUserRow key={user.id.to_string()} user={user.clone()} on_changed={props.on_changed.clone()} /> }) }</div> }
        }
    };
    html! {
        <section class="card border border-base-300 bg-base-100 shadow-sm">
            <div class="card-body">
                <div class="flex items-center justify-between gap-4"><div><h2 class="card-title">{"账号列表"}</h2><p class="mt-1 text-sm text-base-content/60">{"只展示账号资料和状态，不展示密码或密码哈希。"}</p></div><button class="btn btn-ghost btn-sm" type="button" onclick={Callback::from({ let on_changed = props.on_changed.clone(); move |_| on_changed.emit(()) })}>{"刷新"}</button></div>
                <div class="mt-4">{ content }</div>
            </div>
        </section>
    }
}

#[derive(Properties, PartialEq)]
struct AdminUserRowProps {
    user: UserIdentity,
    on_changed: Callback<()>,
}

#[function_component(AdminUserRow)]
fn admin_user_row(props: &AdminUserRowProps) -> Html {
    let pending = use_state(|| false);
    let error = use_state(|| None::<String>);
    let user = &props.user;
    let status_action = if user.status == xiaoluoquiz::domain::auth::AccountStatus::Active {
        "disable"
    } else {
        "enable"
    };
    let status_label = if status_action == "disable" {
        "禁用"
    } else {
        "启用"
    };
    let status_callback = admin_user_action_callback(
        user.id,
        status_action,
        pending.clone(),
        error.clone(),
        props.on_changed.clone(),
    );
    let reset_callback = admin_user_action_callback(
        user.id,
        "reset-password",
        pending.clone(),
        error.clone(),
        props.on_changed.clone(),
    );
    let on_role = {
        let pending = pending.clone();
        let error = error.clone();
        let on_changed = props.on_changed.clone();
        let user_id = user.id;
        Callback::from(move |event: Event| {
            if *pending {
                return;
            }
            let select: HtmlSelectElement = event.target_unchecked_into();
            let role = if select.value() == "admin" {
                UserRole::Admin
            } else {
                UserRole::Student
            };
            pending.set(true);
            error.set(None);
            let pending = pending.clone();
            let error = error.clone();
            let on_changed = on_changed.clone();
            spawn_local(async move {
                let result = async {
                    let request = Request::post(&format!("{ADMIN_USERS_ENDPOINT}/{user_id}/role"))
                        .json(&UpdateRoleRequest { role })
                        .map_err(|error| error.to_string())?;
                    let response = request.send().await.map_err(|error| error.to_string())?;
                    if response.ok() {
                        Ok(())
                    } else {
                        Err(read_api_error(response, "角色更新失败").await)
                    }
                }
                .await;
                match result {
                    Ok(()) => on_changed.emit(()),
                    Err(message) => error.set(Some(message)),
                }
                pending.set(false);
            });
        })
    };
    html! {
        <article class="rounded-box border border-base-300 bg-base-100 p-4" data-testid="admin-user-row">
            <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
                <div class="min-w-0"><div class="flex flex-wrap items-center gap-2"><span class="badge badge-outline">{ account_status_label(user.status) }</span><span class="badge badge-primary badge-outline">{ role_label(user.role) }</span>{ if user.must_change_password { html! { <span class="badge badge-warning badge-outline">{"首次登录需改密"}</span> } } else { Html::default() } }</div><h3 class="mt-3 break-words font-bold">{ &user.display_name }</h3><p class="mt-1 break-all text-sm text-base-content/65">{ format!("账号：{}", user.username) }</p>{ if let Some(student_number) = &user.student_number { html! { <p class="text-sm text-base-content/65">{ format!("学号：{student_number}") }</p> } } else { Html::default() } }{ if let Some(class_name) = &user.class_name { html! { <p class="text-sm text-base-content/65">{ format!("班级：{class_name}") }</p> } } else { Html::default() } }</div>
                <div class="flex flex-wrap items-center gap-2 lg:justify-end"><select class="select select-bordered select-sm" value={role_label_value(user.role)} onchange={on_role} aria-label={format!("{}角色", user.username)}><option value="student" selected={user.role == UserRole::Student}>{"学生"}</option><option value="admin" selected={user.role == UserRole::Admin}>{"管理员"}</option></select><button class="btn btn-outline btn-sm" type="button" onclick={status_callback} disabled={*pending} data-testid={format!("{}-user", status_action)}>{ status_label }</button><button class="btn btn-ghost btn-sm" type="button" onclick={reset_callback} disabled={*pending} data-testid="reset-user-password">{"重置密码"}</button></div>
            </div>
            if let Some(message) = &*error { <p class="mt-3 text-sm text-error" role="alert">{ message }</p> }
        </article>
    }
}

fn admin_user_action_callback(
    user_id: i64,
    action: &'static str,
    pending: UseStateHandle<bool>,
    error: UseStateHandle<Option<String>>,
    on_changed: Callback<()>,
) -> Callback<MouseEvent> {
    Callback::from(move |_| {
        if *pending {
            return;
        }
        pending.set(true);
        error.set(None);
        let pending = pending.clone();
        let error = error.clone();
        let on_changed = on_changed.clone();
        spawn_local(async move {
            let result = async {
                let response = Request::post(&format!("{ADMIN_USERS_ENDPOINT}/{user_id}/{action}"))
                    .send()
                    .await
                    .map_err(|error| error.to_string())?;
                if response.ok() {
                    Ok(())
                } else {
                    Err(read_api_error(response, "账号操作失败").await)
                }
            }
            .await;
            match result {
                Ok(()) => on_changed.emit(()),
                Err(message) => error.set(Some(message)),
            }
            pending.set(false);
        });
    })
}

fn account_status_label(status: xiaoluoquiz::domain::auth::AccountStatus) -> &'static str {
    match status {
        xiaoluoquiz::domain::auth::AccountStatus::Active => "已启用",
        xiaoluoquiz::domain::auth::AccountStatus::Disabled => "已禁用",
    }
}

#[derive(Properties, PartialEq)]
struct AdminQuestionBankFormProps {
    on_saved: Callback<()>,
}

#[function_component(AdminQuestionBankForm)]
fn admin_question_bank_form(props: &AdminQuestionBankFormProps) -> Html {
    let name = use_state(String::new);
    let description = use_state(String::new);
    let error = use_state(|| None::<String>);
    let saving = use_state(|| false);
    let on_submit = {
        let name = name.clone();
        let description = description.clone();
        let error = error.clone();
        let saving = saving.clone();
        let on_saved = props.on_saved.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            if *saving {
                return;
            }
            error.set(None);
            if name.trim().is_empty() {
                error.set(Some("题库名称不能为空".to_owned()));
                return;
            }
            saving.set(true);
            let name_value = (*name).clone();
            let description_value = non_empty((*description).clone());
            let name = name.clone();
            let description = description.clone();
            let error = error.clone();
            let saving = saving.clone();
            let on_saved = on_saved.clone();
            spawn_local(async move {
                let result: Result<QuestionBank, String> = post_json(
                    "/api/v1/admin/question-banks",
                    &QuestionBankInput {
                        name: name_value,
                        description: description_value,
                    },
                    "创建题库失败",
                )
                .await;
                match result {
                    Ok(_) => {
                        name.set(String::new());
                        description.set(String::new());
                        on_saved.emit(());
                    }
                    Err(message) => error.set(Some(message)),
                }
                saving.set(false);
            });
        })
    };
    html! {
        <section class="card border border-base-300 bg-base-100 shadow-sm" data-testid="admin-question-bank-form">
            <div class="card-body"><h2 class="card-title">{"题库管理"}</h2><p class="mt-1 text-sm text-base-content/60">{"创建题库后，可以在下面的题目编辑器中选择归属。"}</p><form class="mt-4 grid gap-4" onsubmit={on_submit}><label class="form-control"><span class="label-text text-sm font-semibold">{"题库名称"}</span><input class="input input-bordered bg-base-100" value={(*name).clone()} oninput={text_input_callback(name.clone())} data-testid="admin-question-bank-name" /></label><label class="form-control"><span class="label-text text-sm font-semibold">{"说明（可选）"}</span><textarea class="textarea textarea-bordered min-h-20 bg-base-100" value={(*description).clone()} oninput={text_area_callback(description.clone())} data-testid="admin-question-bank-description" /></label>{ if let Some(message) = &*error { html! { <p class="text-sm text-error" role="alert">{ message }</p> } } else { Html::default() } }<button class="btn btn-outline w-full" type="submit" disabled={*saving} data-testid="admin-question-bank-save">{ if *saving { "创建中…" } else { "创建题库" } }</button></form></div>
        </section>
    }
}

#[derive(Properties, PartialEq)]
struct AdminQuestionBankListProps {
    banks: Vec<QuestionBank>,
}

#[function_component(AdminQuestionBankList)]
fn admin_question_bank_list(props: &AdminQuestionBankListProps) -> Html {
    html! {
        <section class="card border border-base-300 bg-base-100 shadow-sm" data-testid="admin-question-bank-list">
            <div class="card-body"><h2 class="card-title">{"题库列表"}</h2>{ if props.banks.is_empty() { html! { <p class="mt-3 text-sm text-base-content/60">{"还没有题库。"}</p> } } else { html! { <div class="mt-3 grid gap-2">{ for props.banks.iter().map(|bank| html! { <div class="rounded-box border border-base-300 p-3" key={bank.id.to_string()}><div class="flex flex-wrap items-center justify-between gap-2"><strong>{ &bank.name }</strong><span class="badge badge-ghost">{ format!("{} 道已发布题目", bank.question_count) }</span></div>{ if let Some(description) = &bank.description { html! { <p class="mt-1 text-sm text-base-content/60">{ description }</p> } } else { Html::default() } }</div> }) }</div> } } }</div>
        </section>
    }
}

#[derive(Properties, PartialEq)]
struct AdminAppProps {
    user: UserIdentity,
}

#[function_component(AdminApp)]
fn admin_app(props: &AdminAppProps) -> Html {
    let load_state = use_state(|| AdminLoadState::Loading);
    let refresh_counter = use_state(|| 0_u32);
    let filters = use_state(AdminQuestionFilterValues::default);
    {
        let load_state = load_state.clone();
        let refresh_counter = *refresh_counter;
        let filter_snapshot = (*filters).clone();
        let endpoint = admin_questions_endpoint(&filter_snapshot);
        use_effect_with((refresh_counter, filter_snapshot), move |_| {
            load_state.set(AdminLoadState::Loading);
            spawn_local(async move {
                let result: Result<(Vec<QuestionBank>, Vec<AdminQuestion>), String> = async {
                    let banks = get_json::<QuestionBankListResponse>(
                        QUESTION_BANKS_ENDPOINT,
                        "题库列表加载失败",
                    )
                    .await?
                    .items;
                    let questions =
                        get_json::<AdminQuestionListResponse>(&endpoint, "题目列表加载失败")
                            .await?
                            .items;
                    Ok((banks, questions))
                }
                .await;
                match result {
                    Ok((banks, questions)) => {
                        load_state.set(AdminLoadState::Ready { banks, questions })
                    }
                    Err(error) => load_state.set(AdminLoadState::Error(error)),
                }
            });
            || ()
        });
    }
    let refresh = {
        let refresh_counter = refresh_counter.clone();
        Callback::from(move |_| refresh_counter.set(refresh_counter.wrapping_add(1)))
    };
    let on_filters_changed = {
        let filters = filters.clone();
        Callback::from(move |next: AdminQuestionFilterValues| filters.set(next))
    };
    let banks = match &*load_state {
        AdminLoadState::Ready { banks, .. } => banks.clone(),
        _ => Vec::new(),
    };
    html! {
        <AppShell user={props.user.clone()} eyebrow="XIAOLUOQUIZ / ADMIN" title="题目管理" subtitle="需要有效管理员会话；题库和题目操作由服务端执行权限校验。" test_id="admin-page" active={NavigationItem::Questions}>
            <div class="grid gap-6 xl:grid-cols-[minmax(0,0.85fr)_minmax(0,1.15fr)]">
                <div class="grid gap-6">
                    <AdminQuestionBankForm on_saved={refresh.clone()} />
                    <AdminQuestionBankList banks={banks.clone()} />
                    <AdminQuestionImportForm on_saved={refresh.clone()} />
                    { if let AdminLoadState::Ready { banks, .. } = &*load_state { html! { <AdminQuestionForm banks={banks.clone()} on_saved={refresh.clone()} /> } } else { html! { <AdminQuestionForm banks={Vec::new()} on_saved={refresh.clone()} /> } } }
                </div>
                <AdminQuestionList
                    state={(*load_state).clone()}
                    banks={banks}
                    filters={(*filters).clone()}
                    on_filter={on_filters_changed}
                    on_changed={refresh}
                />
            </div>
        </AppShell>
    }
}

#[derive(Properties, PartialEq)]
struct AdminQuestionImportFormProps {
    on_saved: Callback<()>,
}

#[function_component(AdminQuestionImportForm)]
fn admin_question_import_form(props: &AdminQuestionImportFormProps) -> Html {
    let payload = use_state(String::new);
    let error = use_state(|| None::<String>);
    let result = use_state(|| None::<QuestionImportResponse>);
    let saving = use_state(|| false);
    let on_submit = {
        let payload = payload.clone();
        let error = error.clone();
        let result = result.clone();
        let saving = saving.clone();
        let on_saved = props.on_saved.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            if *saving {
                return;
            }
            error.set(None);
            result.set(None);
            let batch = match serde_json::from_str::<QuestionImportBatch>(&payload) {
                Ok(batch) if !batch.items.is_empty() => batch,
                Ok(_) => {
                    error.set(Some("items 不能为空".to_owned()));
                    return;
                }
                Err(parse_error) => {
                    error.set(Some(format!("JSON 格式错误：{parse_error}")));
                    return;
                }
            };
            saving.set(true);
            let payload = payload.clone();
            let error = error.clone();
            let result = result.clone();
            let saving = saving.clone();
            let on_saved = on_saved.clone();
            spawn_local(async move {
                let import_result: Result<QuestionImportResponse, String> =
                    post_json(ADMIN_QUESTION_IMPORT_ENDPOINT, &batch, "批量导入失败").await;
                match import_result {
                    Ok(report) => {
                        payload.set(String::new());
                        result.set(Some(report));
                        on_saved.emit(());
                    }
                    Err(message) => error.set(Some(message)),
                }
                saving.set(false);
            });
        })
    };
    html! {
        <section class="card border border-base-300 bg-base-100 shadow-sm" data-testid="admin-question-import">
            <div class="card-body">
                <h2 class="card-title">{"批量导入题目"}</h2>
                <p class="mt-1 text-sm text-base-content/60">{"粘贴标准 JSON。导入只新增题目；同一题库中规范化后的重复题干会跳过，已有题目不会被覆盖。新题会直接发布，可立即用于练习和组卷。"}</p>
                <form class="mt-4 grid gap-4" onsubmit={on_submit}>
                    <label class="form-control">
                        <span class="label-text text-sm font-semibold">{"JSON 内容"}</span>
                        <textarea
                            class="textarea textarea-bordered min-h-56 bg-base-100 font-mono text-sm"
                            value={(*payload).clone()}
                            oninput={text_area_callback(payload.clone())}
                            placeholder={r#"{"items":[{"question_bank_id":2,"question_type":"single_choice","stem":"题干","options":[{"key":"A","text":"选项 A"},{"key":"B","text":"选项 B"}],"correct_answer":{"type":"single_choice","option_key":"A"}}]}"#}
                            data-testid="admin-question-import-json"
                        />
                    </label>
                    if let Some(message) = &*error {
                        <p class="text-sm text-error" role="alert" data-testid="admin-question-import-error">{ message }</p>
                    }
                    if let Some(report) = &*result {
                        <p class="text-sm text-success" role="status" data-testid="admin-question-import-result">
                            { format!("导入完成：新增 {} 道，跳过 {} 道，错误 {} 道。", report.inserted, report.skipped, report.errors) }
                        </p>
                    }
                    <button class="btn btn-outline w-full" type="submit" disabled={*saving} data-testid="admin-question-import-submit">
                        { if *saving { "导入中…" } else { "导入题目" } }
                    </button>
                </form>
            </div>
        </section>
    }
}

#[derive(Properties, PartialEq)]
struct AdminQuestionFormProps {
    banks: Vec<QuestionBank>,
    on_saved: Callback<()>,
}

#[function_component(AdminQuestionForm)]
fn admin_question_form(props: &AdminQuestionFormProps) -> Html {
    let question_bank_id = use_state(|| None::<i64>);
    let question_type = use_state(|| QuestionType::SingleChoice);
    let stem = use_state(String::new);
    let explanation = use_state(String::new);
    let options = use_state(default_choice_options);
    let correct_option = use_state(|| "A".to_owned());
    let correct_options = use_state(Vec::<String>::new);
    let fill_blank_answer = use_state(String::new);
    let true_value = use_state(|| true);
    let reference = use_state(String::new);
    let rubric = use_state(String::new);
    let error = use_state(|| None::<String>);
    let saving = use_state(|| false);
    {
        let question_bank_id = question_bank_id.clone();
        use_effect_with(props.banks.clone(), move |banks| {
            if question_bank_id.is_none() {
                if let Some(bank) = banks.first() {
                    question_bank_id.set(Some(bank.id));
                }
            }
            || ()
        });
    }
    let on_question_bank = {
        let question_bank_id = question_bank_id.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            question_bank_id.set(select.value().parse::<i64>().ok());
        })
    };
    let on_type = {
        let question_type = question_type.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            question_type.set(match select.value().as_str() {
                "multiple_choice" => QuestionType::MultipleChoice,
                "fill_blank" => QuestionType::FillBlank,
                "true_false" => QuestionType::TrueFalse,
                "short_answer" => QuestionType::ShortAnswer,
                _ => QuestionType::SingleChoice,
            });
        })
    };
    let on_stem = text_area_callback(stem.clone());
    let on_explanation = text_area_callback(explanation.clone());
    let on_fill_blank_answer = text_input_callback(fill_blank_answer.clone());
    let on_true_value = {
        let true_value = true_value.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            true_value.set(select.value() == "true");
        })
    };
    let on_reference = text_area_callback(reference.clone());
    let on_rubric = text_area_callback(rubric.clone());
    let on_submit = {
        let question_bank_id = question_bank_id.clone();
        let question_type = question_type.clone();
        let stem = stem.clone();
        let explanation = explanation.clone();
        let options = options.clone();
        let correct_option = correct_option.clone();
        let correct_options = correct_options.clone();
        let fill_blank_answer = fill_blank_answer.clone();
        let true_value = true_value.clone();
        let reference = reference.clone();
        let rubric = rubric.clone();
        let error = error.clone();
        let saving = saving.clone();
        let on_saved = props.on_saved.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            if *saving {
                return;
            }
            error.set(None);
            let Some(question_bank_id) = *question_bank_id else {
                error.set(Some("请选择题库".to_owned()));
                return;
            };
            let input = match build_admin_question_input(
                question_bank_id,
                *question_type,
                &stem,
                &explanation,
                &options,
                &correct_option,
                &correct_options,
                &fill_blank_answer,
                *true_value,
                &reference,
                &rubric,
            ) {
                Ok(input) => input,
                Err(message) => {
                    error.set(Some(message));
                    return;
                }
            };
            saving.set(true);
            let error = error.clone();
            let saving = saving.clone();
            let on_saved = on_saved.clone();
            spawn_local(async move {
                let result: Result<AdminQuestion, String> =
                    post_json(ADMIN_QUESTIONS_ENDPOINT, &input, "保存题目失败").await;
                match result {
                    Ok(_) => on_saved.emit(()),
                    Err(message) => error.set(Some(message)),
                }
                saving.set(false);
            });
        })
    };
    let selected_bank_value = question_bank_id
        .as_ref()
        .map_or_else(String::new, ToString::to_string);
    html! {
        <section class="card border border-base-300 bg-base-100 shadow-sm" data-testid="admin-form"><div class="card-body"><div><h2 class="card-title">{"新建题目草稿"}</h2><p class="mt-1 text-sm text-base-content/60">{"先选择题库，再录入题面和答案；题目分值在组装试卷时配置。"}</p></div><form class="mt-4 grid gap-4" onsubmit={on_submit}><label class="form-control"><span class="label-text text-sm font-semibold">{"题库"}</span><select class="select select-bordered bg-base-100" value={selected_bank_value} onchange={on_question_bank} disabled={props.banks.is_empty()} data-testid="admin-question-bank">{ for props.banks.iter().map(|bank| html! { <option value={bank.id.to_string()} selected={Some(bank.id) == *question_bank_id}>{ &bank.name }</option> }) }</select></label><label class="form-control"><span class="label-text text-sm font-semibold">{"题型"}</span><select class="select select-bordered bg-base-100" value={question_type.to_string()} onchange={on_type} data-testid="admin-question-type"><option value="single_choice" selected={*question_type == QuestionType::SingleChoice}>{"选择题"}</option><option value="multiple_choice" selected={*question_type == QuestionType::MultipleChoice}>{"多选题"}</option><option value="fill_blank" selected={*question_type == QuestionType::FillBlank}>{"填空题"}</option><option value="true_false" selected={*question_type == QuestionType::TrueFalse}>{"判断题"}</option><option value="short_answer" selected={*question_type == QuestionType::ShortAnswer}>{"简答题"}</option></select></label><label class="form-control"><span class="label-text text-sm font-semibold">{"题干"}</span><textarea class="textarea textarea-bordered min-h-24 bg-base-100" value={(*stem).clone()} oninput={on_stem} data-testid="admin-stem" /></label>{ render_admin_answer_fields(AdminAnswerFields { question_type: *question_type, options: &options, correct_option: &correct_option, correct_options: &correct_options, fill_blank_answer: &fill_blank_answer, on_fill_blank_answer: &on_fill_blank_answer, true_value: *true_value, on_true_value: &on_true_value, reference: &reference, on_reference: &on_reference, rubric: &rubric, on_rubric: &on_rubric }) }<label class="form-control"><span class="label-text text-sm font-semibold">{"解析（可选）"}</span><textarea class="textarea textarea-bordered min-h-24 bg-base-100" value={(*explanation).clone()} oninput={on_explanation} data-testid="admin-explanation" /></label>{ if let Some(message) = &*error { html! { <p class="text-sm text-error" role="alert" data-testid="admin-form-error">{ message }</p> } } else { Html::default() } }<button class="btn btn-secondary w-full" type="submit" disabled={*saving || props.banks.is_empty()} data-testid="admin-save">{ if *saving { "保存中…" } else { "保存草稿" } }</button></form></div></section>
    }
}

#[derive(Properties, PartialEq)]
struct AdminAttemptsAppProps {
    user: UserIdentity,
}

#[function_component(AdminAttemptsApp)]
fn admin_attempts_app(props: &AdminAttemptsAppProps) -> Html {
    let load_state = use_state(|| AdminAttemptLoadState::Loading);
    let refresh_counter = use_state(|| 0_u32);
    {
        let load_state = load_state.clone();
        let refresh_counter = *refresh_counter;
        use_effect_with(refresh_counter, move |_| {
            spawn_local(async move {
                match get_json::<AdminAttemptListResponse>(
                    ADMIN_ATTEMPTS_ENDPOINT,
                    "考试记录加载失败",
                )
                .await
                {
                    Ok(payload) => load_state.set(AdminAttemptLoadState::Ready(payload.items)),
                    Err(error) => load_state.set(AdminAttemptLoadState::Error(error)),
                }
            });
            || ()
        });
    }
    let refresh = {
        let refresh_counter = refresh_counter.clone();
        Callback::from(move |_| refresh_counter.set(refresh_counter.wrapping_add(1)))
    };
    let body = match &*load_state {
        AdminAttemptLoadState::Loading => html! {
            <div class="grid gap-3" data-testid="admin-attempt-loading-state">
                { for (0..3).map(|_| html! { <div class="skeleton h-28 w-full rounded-box" /> }) }
            </div>
        },
        AdminAttemptLoadState::Error(error) => html! {
            <div class="alert alert-error" data-testid="admin-attempt-error-state">{ format!("考试记录加载失败：{error}") }</div>
        },
        AdminAttemptLoadState::Ready(attempts) if attempts.is_empty() => html! {
            <div class="rounded-box border border-dashed border-base-300 bg-base-100 p-10 text-center" data-testid="admin-attempt-empty-state">
                <p class="text-lg font-bold">{"还没有已提交的考试记录"}</p>
                <p class="mt-2 text-sm text-base-content/60">{"学生交卷后，记录会出现在这里。"}</p>
            </div>
        },
        AdminAttemptLoadState::Ready(attempts) => html! {
            <div class="grid gap-3" data-testid="admin-attempt-list">
                { for attempts.iter().map(|attempt| html! {
                    <AdminAttemptSummaryRow key={attempt.id.to_string()} attempt={attempt.clone()} />
                }) }
            </div>
        },
    };
    html! {
        <AppShell
            user={props.user.clone()}
            eyebrow="XIAOLUOQUIZ / ADMIN"
            title="考试记录与阅卷"
            subtitle="查看已提交试卷、考生答案和自动评分结果；简答题可以在详情页人工评分并填写反馈。"
            test_id="admin-attempts-page"
            active={NavigationItem::Attempts}
        >
            <div class="mb-4 flex justify-end"><button class="btn btn-ghost btn-sm" type="button" onclick={refresh}>{"刷新记录"}</button></div>
            { body }
        </AppShell>
    }
}

#[derive(Properties, PartialEq)]
struct AdminAttemptSummaryRowProps {
    attempt: AdminAttemptSummary,
}

#[function_component(AdminAttemptSummaryRow)]
fn admin_attempt_summary_row(props: &AdminAttemptSummaryRowProps) -> Html {
    let attempt = &props.attempt;
    let score = attempt.total_score.map_or_else(
        || "待批改".to_owned(),
        |value| format!("{value:.2} / {:.2}", attempt.max_score),
    );
    html! {
        <article class="rounded-box border border-base-300 bg-base-100 p-4" data-testid="admin-attempt-row">
            <div class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
                <div class="min-w-0">
                    <div class="flex flex-wrap items-center gap-2">
                        <span class={classes!("badge", attempt_status_class(attempt.status))}>{ attempt_status_label(attempt.status) }</span>
                        <span class="badge badge-outline">{ score }</span>
                        <span class="text-xs text-base-content/55">{ format!("已答 {} / {} 题", attempt.answered_count, attempt.question_count) }</span>
                    </div>
                    <h2 class="mt-3 break-words text-lg font-bold">{ &attempt.title }</h2>
                    <p class="mt-1 break-words text-sm text-base-content/65">{ format!("考生：{}", candidate_info_label(&attempt.candidate_info)) }</p>
                    if attempt.needs_review_count > 0 {
                        <p class="mt-1 text-sm text-warning">{ format!("待批改 {} 题", attempt.needs_review_count) }</p>
                    }
                    if let Some(submitted_at) = &attempt.submitted_at {
                        <p class="mt-1 text-xs text-base-content/55">{ format!("交卷时间（上海时间）：{}", shanghai_datetime_label(submitted_at)) }</p>
                    }
                </div>
                <a class="btn btn-primary btn-sm shrink-0" href={format!("/admin/attempts/{}", attempt.id)} data-testid="view-admin-attempt">{"查看与阅卷"}</a>
            </div>
        </article>
    }
}

#[derive(Properties, PartialEq)]
struct AdminAttemptDetailPageProps {
    user: UserIdentity,
    attempt_id: i64,
}

#[function_component(AdminAttemptDetailPage)]
fn admin_attempt_detail_page(props: &AdminAttemptDetailPageProps) -> Html {
    let load_state = use_state(|| AdminAttemptDetailLoadState::Loading);
    {
        let load_state = load_state.clone();
        let attempt_id = props.attempt_id;
        use_effect_with(attempt_id, move |_| {
            spawn_local(async move {
                match get_json::<AdminAttempt>(
                    &format!("{ADMIN_ATTEMPTS_ENDPOINT}/{attempt_id}"),
                    "考试记录详情加载失败",
                )
                .await
                {
                    Ok(attempt) => load_state.set(AdminAttemptDetailLoadState::Ready(attempt)),
                    Err(error) => load_state.set(AdminAttemptDetailLoadState::Error(error)),
                }
            });
            || ()
        });
    }
    let on_updated = {
        let load_state = load_state.clone();
        Callback::from(move |attempt: AdminAttempt| {
            load_state.set(AdminAttemptDetailLoadState::Ready(attempt))
        })
    };
    let body = match &*load_state {
        AdminAttemptDetailLoadState::Loading => {
            html! { <div class="grid gap-4" data-testid="admin-attempt-detail-loading-state"><div class="skeleton h-36 w-full rounded-box" /><div class="skeleton h-72 w-full rounded-box" /></div> }
        }
        AdminAttemptDetailLoadState::Error(error) => {
            html! { <div class="alert alert-error" data-testid="admin-attempt-detail-error-state">{ format!("无法查看考试记录：{error}") }</div> }
        }
        AdminAttemptDetailLoadState::Ready(attempt) => {
            let score = attempt.total_score.map_or_else(
                || "待批改".to_owned(),
                |value| format!("{value:.2} / {:.2}", attempt.max_score),
            );
            html! {
                <div class="grid gap-6">
                    <section class="card border border-base-300 bg-base-100 shadow-sm">
                        <div class="card-body gap-3">
                            <div class="flex flex-wrap items-center gap-2"><span class={classes!("badge", attempt_status_class(attempt.status))} data-testid="admin-attempt-status">{ attempt_status_label(attempt.status) }</span><span class="badge badge-outline" data-testid="admin-attempt-total-score">{ score }</span></div>
                            <h2 class="card-title break-words text-2xl">{ &attempt.title }</h2>
                            <p class="text-sm text-base-content/65">{ format!("考生账号 ID：{} · {}", attempt.user_id, candidate_info_label(&attempt.candidate_info)) }</p>
                            if let Some(submitted_at) = &attempt.submitted_at { <p class="text-sm text-base-content/65">{ format!("交卷时间（上海时间）：{}", shanghai_datetime_label(submitted_at)) }</p> }
                            <a class="btn btn-ghost btn-sm w-fit" href="/admin/attempts">{"返回考试记录"}</a>
                        </div>
                    </section>
                    <section class="grid gap-4" data-testid="admin-attempt-question-list">
                        { for attempt.questions.iter().map(|question| html! { <AdminAttemptQuestionCard key={question.question_id.to_string()} attempt_id={attempt.id} question={question.clone()} on_updated={on_updated.clone()} /> }) }
                    </section>
                </div>
            }
        }
    };
    html! {
        <AppShell user={props.user.clone()} eyebrow="XIAOLUOQUIZ / ADMIN" title="考试记录详情" subtitle="查看本次考试的逐题答案、自动评分和人工阅卷内容。" test_id="admin-attempt-detail-page" active={NavigationItem::Attempts}>
            { body }
        </AppShell>
    }
}

#[derive(Properties, PartialEq)]
struct AdminAttemptQuestionCardProps {
    attempt_id: i64,
    question: AdminAttemptQuestion,
    on_updated: Callback<AdminAttempt>,
}

#[function_component(AdminAttemptQuestionCard)]
fn admin_attempt_question_card(props: &AdminAttemptQuestionCardProps) -> Html {
    let question = &props.question;
    let score = use_state(|| {
        question
            .awarded_score
            .map_or_else(String::new, |value| format!("{value:.2}"))
    });
    let feedback = use_state(|| question.feedback.clone().unwrap_or_default());
    let saving = use_state(|| false);
    let error = use_state(|| None::<String>);
    let on_submit = {
        let score = score.clone();
        let feedback = feedback.clone();
        let saving = saving.clone();
        let error = error.clone();
        let on_updated = props.on_updated.clone();
        let attempt_id = props.attempt_id;
        let question_id = question.question_id;
        let max_score = question.max_score;
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            if *saving {
                return;
            }
            error.set(None);
            let value = match score.parse::<f64>() {
                Ok(value) if value.is_finite() && (0.0..=max_score).contains(&value) => value,
                _ => {
                    error.set(Some(format!("分数必须在 0 到 {:.2} 之间", max_score)));
                    return;
                }
            };
            saving.set(true);
            let feedback_value = non_empty((*feedback).clone());
            let saving = saving.clone();
            let error = error.clone();
            let on_updated = on_updated.clone();
            spawn_local(async move {
                let result: Result<AdminAttempt, String> = post_json(
                    &format!("{ADMIN_ATTEMPTS_ENDPOINT}/{attempt_id}/grade"),
                    &GradeAttemptRequest {
                        question_id,
                        score: value,
                        feedback: feedback_value,
                    },
                    "保存批改结果失败",
                )
                .await;
                match result {
                    Ok(attempt) => on_updated.emit(attempt),
                    Err(message) => error.set(Some(message)),
                }
                saving.set(false);
            });
        })
    };
    let needs_manual_grade = question.question_type == QuestionType::ShortAnswer
        || question.grading_status == GradingStatus::NeedsReview;
    html! {
        <article class="card border border-base-300 bg-base-100 shadow-sm" data-testid="admin-attempt-question">
            <div class="card-body gap-4">
                <div class="flex items-start gap-3"><span class="badge badge-primary mt-1">{ question.position + 1 }</span><div class="min-w-0 flex-1"><div class="flex flex-wrap items-center gap-2"><span class="badge badge-outline">{ question_type_label(question.question_type) }</span><span class="badge badge-ghost">{ format!("满分 {:.2}", question.max_score) }</span><span class={classes!("badge", grading_status_class(question.grading_status))}>{ grading_status_label(question.grading_status) }</span></div><h2 class="mt-3 break-words text-lg font-bold">{ &question.stem }</h2></div></div>
                <div class="grid gap-2 rounded-box bg-base-200/60 p-4 text-sm"><p><strong>{"学生答案："}</strong>{ if let Some(answer) = &question.answer { html! { <AnswerView answer={answer.clone()} /> } } else { html! { <span class="text-base-content/60">{"未作答"}</span> } } }</p><p><strong>{"参考答案："}</strong><CorrectAnswerView answer={question.correct_answer.clone()} /></p>{ if let Some(explanation) = &question.explanation { html! { <p><strong>{"解析："}</strong>{ explanation }</p> } } else { Html::default() } }<p><strong>{"当前得分："}</strong>{ question.awarded_score.map_or_else(|| "待批改".to_owned(), |value| format!("{value:.2} / {:.2}", question.max_score)) }</p>{ if let Some(feedback) = &question.feedback { html! { <p><strong>{"已有批改意见："}</strong>{ feedback }</p> } } else { Html::default() } }</div>
                if needs_manual_grade {
                    if question.answer.is_some() {
                        <form class="grid gap-3 rounded-box border border-warning/30 bg-warning/5 p-4" onsubmit={on_submit}>
                            <h3 class="font-bold">{"人工批改简答题"}</h3>
                            <label class="form-control"><span class="label-text text-sm font-semibold">{"得分"}</span><input class="input input-bordered bg-base-100" type="number" min="0" max={question.max_score.to_string()} step="0.01" value={(*score).clone()} oninput={text_input_callback(score.clone())} data-testid="admin-grade-score" /></label>
                            <label class="form-control"><span class="label-text text-sm font-semibold">{"批改意见（可选）"}</span><textarea class="textarea textarea-bordered min-h-24 bg-base-100" value={(*feedback).clone()} oninput={text_area_callback(feedback.clone())} data-testid="admin-grade-feedback" /></label>
                            if let Some(message) = &*error { <p class="text-sm text-error" role="alert">{ message }</p> }
                            <button class="btn btn-primary w-fit" type="submit" disabled={*saving} data-testid="admin-grade-save">{ if *saving { "保存中…" } else { "保存批改结果" } }</button>
                        </form>
                    } else {
                        <div class="alert alert-warning">{"学生没有提交答案，不能对这道题进行人工评分。"}</div>
                    }
                }
            </div>
        </article>
    }
}

fn text_input_callback(state: UseStateHandle<String>) -> Callback<InputEvent> {
    Callback::from(move |event: InputEvent| {
        let input: HtmlInputElement = event.target_unchecked_into();
        state.set(input.value());
    })
}

fn text_area_callback(state: UseStateHandle<String>) -> Callback<InputEvent> {
    Callback::from(move |event: InputEvent| {
        let input: HtmlTextAreaElement = event.target_unchecked_into();
        state.set(input.value());
    })
}

fn default_choice_options() -> Vec<QuestionOption> {
    ["A", "B", "C", "D"]
        .into_iter()
        .map(|key| QuestionOption {
            key: key.to_owned(),
            text: String::new(),
        })
        .collect()
}

fn next_choice_key(options: &[QuestionOption]) -> String {
    let mut index = options.len();
    loop {
        let key = choice_key_for_index(index);
        if !options.iter().any(|option| option.key == key) {
            return key;
        }
        index += 1;
    }
}

fn choice_key_for_index(index: usize) -> String {
    let mut index = index + 1;
    let mut key = String::new();
    while index > 0 {
        index -= 1;
        key.push((b'A' + (index % 26) as u8) as char);
        index /= 26;
    }
    key.chars().rev().collect()
}

struct AdminAnswerFields<'a> {
    question_type: QuestionType,
    options: &'a UseStateHandle<Vec<QuestionOption>>,
    correct_option: &'a UseStateHandle<String>,
    correct_options: &'a UseStateHandle<Vec<String>>,
    fill_blank_answer: &'a UseStateHandle<String>,
    on_fill_blank_answer: &'a Callback<InputEvent>,
    true_value: bool,
    on_true_value: &'a Callback<Event>,
    reference: &'a UseStateHandle<String>,
    on_reference: &'a Callback<InputEvent>,
    rubric: &'a UseStateHandle<String>,
    on_rubric: &'a Callback<InputEvent>,
}

fn render_admin_answer_fields(fields: AdminAnswerFields<'_>) -> Html {
    let AdminAnswerFields {
        question_type,
        options,
        correct_option,
        correct_options,
        fill_blank_answer,
        on_fill_blank_answer,
        true_value,
        on_true_value,
        reference,
        on_reference,
        rubric,
        on_rubric,
    } = fields;

    match question_type {
        QuestionType::SingleChoice | QuestionType::MultipleChoice => {
            let options_for_add = options.clone();
            let on_add_option = Callback::from(move |_| {
                let mut values = (*options_for_add).clone();
                let key = next_choice_key(&values);
                values.push(QuestionOption {
                    key,
                    text: String::new(),
                });
                options_for_add.set(values);
            });
            let option_rows = options.iter().map(|option| {
                let key = option.key.clone();
                let key_for_input = key.clone();
                let options_for_input = options.clone();
                let on_option = Callback::from(move |event: InputEvent| {
                    let input: HtmlInputElement = event.target_unchecked_into();
                    let mut values = (*options_for_input).clone();
                    if let Some(option) = values
                        .iter_mut()
                        .find(|option| option.key == key_for_input)
                    {
                        option.text = input.value();
                    }
                    options_for_input.set(values);
                });
                let options_for_remove = options.clone();
                let correct_option_for_remove = correct_option.clone();
                let correct_options_for_remove = correct_options.clone();
                let key_for_remove = key.clone();
                let on_remove = Callback::from(move |_| {
                    if (*options_for_remove).len() <= 2 {
                        return;
                    }
                    let mut values = (*options_for_remove).clone();
                    values.retain(|option| option.key != key_for_remove);
                    options_for_remove.set(values.clone());
                    if *correct_option_for_remove == key_for_remove {
                        correct_option_for_remove.set(
                            values
                                .first()
                                .map(|option| option.key.clone())
                                .unwrap_or_default(),
                        );
                    }
                    let mut selected = (*correct_options_for_remove).clone();
                    selected.retain(|option_key| option_key != &key_for_remove);
                    correct_options_for_remove.set(selected);
                });
                html! {
                    <div class="flex min-w-0 items-end gap-2" key={key.clone()} data-testid={format!("admin-choice-option-{key}")}>
                        <label class="form-control min-w-0 flex-1">
                            <span class="label-text text-sm">{ format!("选项 {key}") }</span>
                            <input class="input input-bordered bg-base-100" value={option.text.clone()} oninput={on_option} data-testid={format!("admin-option-{key}")} />
                        </label>
                        <button class="btn btn-ghost btn-sm shrink-0" type="button" onclick={on_remove} disabled={options.len() <= 2} data-testid={format!("admin-remove-option-{key}")}>{"删除"}</button>
                    </div>
                }
            });
            let choice_config = match question_type {
                QuestionType::SingleChoice => html! {
                    <label class="form-control">
                        <span class="label-text text-sm">{"正确选项"}</span>
                        <select class="select select-bordered bg-base-100" onchange={Callback::from({
                            let correct_option = correct_option.clone();
                            move |event: Event| {
                                let select: HtmlSelectElement = event.target_unchecked_into();
                                correct_option.set(select.value());
                            }
                        })} value={(**correct_option).clone()} data-testid="admin-correct-option">
                            { for options.iter().map(|option| html! {
                                <option value={option.key.clone()} selected={**correct_option == option.key}>{ &option.key }</option>
                            }) }
                        </select>
                    </label>
                },
                QuestionType::MultipleChoice => html! {
                    <fieldset class="grid gap-2">
                        <legend class="label-text text-sm">{"正确选项（至少选择两个）"}</legend>
                        <div class="grid gap-2 sm:grid-cols-2" data-testid="admin-correct-options">
                            { for options.iter().map(|option| {
                                let correct_options = correct_options.clone();
                                let key = option.key.clone();
                                let checked = (*correct_options).iter().any(|value| value == &key);
                                html! {
                                    <label class="label cursor-pointer justify-start gap-3 rounded-box border border-base-300 bg-base-100 px-3 py-2">
                                        <input
                                            class="checkbox checkbox-primary"
                                            type="checkbox"
                                            checked={checked}
                                            onchange={Callback::from(move |event: Event| {
                                                let input: HtmlInputElement = event.target_unchecked_into();
                                                let mut values = (*correct_options).clone();
                                                if input.checked() {
                                                    if !values.iter().any(|value| value == &key) {
                                                        values.push(key.clone());
                                                    }
                                                } else {
                                                    values.retain(|value| value != &key);
                                                }
                                                correct_options.set(values);
                                            })}
                                            data-testid={format!("admin-correct-option-{key}")}
                                        />
                                        <span>{ &option.key }</span>
                                    </label>
                                }
                            }) }
                        </div>
                    </fieldset>
                },
                _ => Html::default(),
            };
            html! {
                <div class="grid gap-3 rounded-box bg-base-200/60 p-4">
                    <div class="flex flex-wrap items-center justify-between gap-3">
                        <div>
                            <p class="text-sm font-bold">{ if question_type == QuestionType::SingleChoice { "选择题配置" } else { "多选题配置" } }</p>
                            <p class="mt-1 text-xs text-base-content/60">{"选项数量不固定，可添加或删除选项；至少保留两个选项。"}</p>
                        </div>
                        <button class="btn btn-outline btn-sm" type="button" onclick={on_add_option} data-testid="admin-add-option">{"添加选项"}</button>
                    </div>
                    <div class="grid gap-3" data-testid="admin-choice-options">
                        { for option_rows }
                    </div>
                    { choice_config }
                </div>
            }
        }
        QuestionType::FillBlank => html! {
            <div class="grid gap-3 rounded-box bg-base-200/60 p-4">
                <p class="text-sm font-bold">{"填空题配置"}</p>
                <p class="text-xs text-base-content/60">{"当前编辑器先支持一个空；多个空可通过 API 配置。多个可接受答案用逗号分隔。"}</p>
                <label class="form-control">
                    <span class="label-text text-sm">{"可接受答案"}</span>
                    <input class="input input-bordered bg-base-100" value={(**fill_blank_answer).clone()} oninput={on_fill_blank_answer.clone()} data-testid="admin-fill-blank-answer" />
                </label>
            </div>
        },
        QuestionType::TrueFalse => html! {
            <div class="grid gap-3 rounded-box bg-base-200/60 p-4">
                <p class="text-sm font-bold">{"判断题配置"}</p>
                <label class="form-control">
                    <span class="label-text text-sm">{"标准答案"}</span>
                    <select class="select select-bordered bg-base-100" onchange={on_true_value.clone()} value={if true_value { "true" } else { "false" }} data-testid="admin-true-false-answer">
                        <option value="true" selected={true_value}>{"正确"}</option>
                        <option value="false" selected={!true_value}>{"错误"}</option>
                    </select>
                </label>
            </div>
        },
        QuestionType::ShortAnswer => html! {
            <div class="grid gap-3 rounded-box bg-base-200/60 p-4">
                <p class="text-sm font-bold">{"简答题配置"}</p>
                <label class="form-control">
                    <span class="label-text text-sm">{"参考答案"}</span>
                    <textarea class="textarea textarea-bordered min-h-24 bg-base-100" value={(**reference).clone()} oninput={on_reference.clone()} data-testid="admin-reference" />
                </label>
                <label class="form-control">
                    <span class="label-text text-sm">{"评分要点（可选）"}</span>
                    <textarea class="textarea textarea-bordered min-h-20 bg-base-100" value={(**rubric).clone()} oninput={on_rubric.clone()} data-testid="admin-rubric" />
                </label>
            </div>
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn build_admin_question_input(
    question_bank_id: i64,
    question_type: QuestionType,
    stem: &str,
    explanation: &str,
    options: &[QuestionOption],
    correct_option: &str,
    correct_options: &[String],
    fill_blank_answer: &str,
    true_value: bool,
    reference: &str,
    rubric: &str,
) -> Result<AdminQuestionInput, String> {
    let explanation = (!explanation.trim().is_empty()).then(|| explanation.to_owned());
    let choice_options = options
        .iter()
        .filter(|option| !option.text.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>();
    let (blank_count, options, correct_answer) = match question_type {
        QuestionType::SingleChoice => (
            0,
            choice_options,
            CorrectAnswer::SingleChoice {
                option_key: correct_option.to_owned(),
            },
        ),
        QuestionType::MultipleChoice => (
            0,
            choice_options,
            CorrectAnswer::MultipleChoice {
                option_keys: correct_options.to_vec(),
            },
        ),
        QuestionType::FillBlank => {
            let candidates: Vec<String> = fill_blank_answer
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
                .collect();
            (
                1,
                Vec::new(),
                CorrectAnswer::FillBlank {
                    accepted: vec![candidates],
                },
            )
        }
        QuestionType::TrueFalse => (
            0,
            Vec::new(),
            CorrectAnswer::TrueFalse { value: true_value },
        ),
        QuestionType::ShortAnswer => (
            0,
            Vec::new(),
            CorrectAnswer::ShortAnswer {
                reference: reference.to_owned(),
                rubric: (!rubric.trim().is_empty()).then(|| rubric.to_owned()),
            },
        ),
    };

    let input = AdminQuestionInput {
        question_bank_id,
        question_type,
        stem: stem.to_owned(),
        blank_count,
        options,
        explanation,
        correct_answer,
    };
    input.validate().map_err(|error| error.to_string())?;
    Ok(input)
}

#[derive(Properties, PartialEq)]
struct AdminQuestionListProps {
    state: AdminLoadState,
    banks: Vec<QuestionBank>,
    filters: AdminQuestionFilterValues,
    on_filter: Callback<AdminQuestionFilterValues>,
    on_changed: Callback<()>,
}

#[function_component(AdminQuestionList)]
fn admin_question_list(props: &AdminQuestionListProps) -> Html {
    let keyword = use_state(|| props.filters.keyword.clone());
    let bank_id = use_state(|| props.filters.bank_id);
    let question_type = use_state(|| props.filters.question_type);
    let status = use_state(|| props.filters.status);
    {
        let keyword = keyword.clone();
        let bank_id = bank_id.clone();
        let question_type = question_type.clone();
        let status = status.clone();
        use_effect_with(props.filters.clone(), move |filters| {
            keyword.set(filters.keyword.clone());
            bank_id.set(filters.bank_id);
            question_type.set(filters.question_type);
            status.set(filters.status);
            || ()
        });
    }
    let on_bank = {
        let bank_id = bank_id.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            bank_id.set(select.value().parse::<i64>().ok());
        })
    };
    let on_type = {
        let question_type = question_type.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            question_type.set(select.value().parse::<QuestionType>().ok());
        })
    };
    let on_status = {
        let status = status.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            status.set(select.value().parse::<QuestionStatus>().ok());
        })
    };
    let on_submit = {
        let keyword = keyword.clone();
        let bank_id = bank_id.clone();
        let question_type = question_type.clone();
        let status = status.clone();
        let on_filter = props.on_filter.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            on_filter.emit(AdminQuestionFilterValues {
                keyword: keyword.trim().to_owned(),
                bank_id: *bank_id,
                question_type: *question_type,
                status: *status,
            });
        })
    };
    let on_reset = {
        let keyword = keyword.clone();
        let bank_id = bank_id.clone();
        let question_type = question_type.clone();
        let status = status.clone();
        let on_filter = props.on_filter.clone();
        Callback::from(move |_| {
            keyword.set(String::new());
            bank_id.set(None);
            question_type.set(None);
            status.set(None);
            on_filter.emit(AdminQuestionFilterValues::default());
        })
    };
    let selected_bank_value = (*bank_id).map_or_else(String::new, |value| value.to_string());
    let selected_type_value = (*question_type)
        .map(|value| value.to_string())
        .unwrap_or_default();
    let selected_status_value = (*status).map(|value| value.to_string()).unwrap_or_default();
    let content = match &props.state {
        AdminLoadState::Loading => html! {
            <div class="grid gap-3" data-testid="admin-loading-state">
                { for (0..3).map(|_| html! { <div class="skeleton h-24 w-full rounded-box" /> }) }
            </div>
        },
        AdminLoadState::Error(error) => html! {
            <div class="alert alert-error" data-testid="admin-error-state">
                <span>{ format!("管理题目加载失败：{error}") }</span>
            </div>
        },
        AdminLoadState::Ready { questions, .. } if questions.is_empty() => html! {
            <div class="rounded-box border border-dashed border-base-300 p-10 text-center" data-testid="admin-empty-state">
                <p class="font-bold">{ if props.filters.is_active() { "没有符合筛选条件的题目" } else { "还没有题目" } }</p>
                <p class="mt-2 text-sm text-base-content/60">{ if props.filters.is_active() { "可以调整关键字、题库、题型或状态后重试。" } else { "保存第一道题目草稿后，它会出现在这里。" } }</p>
                if props.filters.is_active() {
                    <button class="btn btn-ghost btn-sm mt-4" type="button" onclick={on_reset.clone()} data-testid="admin-question-filter-reset-empty">{"清除筛选"}</button>
                }
            </div>
        },
        AdminLoadState::Ready { questions, .. } => html! {
            <div class="grid gap-3" data-testid="admin-question-list">
                { for questions.iter().map(|question| html! {
                    <AdminQuestionRow key={question.id.to_string()} question={question.clone()} on_changed={props.on_changed.clone()} />
                }) }
            </div>
        },
    };

    html! {
        <section class="card border border-base-300 bg-base-100 shadow-sm">
            <div class="card-body">
                <div class="flex items-center justify-between gap-4">
                    <div>
                        <h2 class="card-title">{"题目列表"}</h2>
                        <p class="mt-1 text-sm text-base-content/60">{"草稿、已发布和已下线题目"}</p>
                    </div>
                    <button class="btn btn-ghost btn-sm" type="button" onclick={Callback::from({
                        let on_changed = props.on_changed.clone();
                        move |_| on_changed.emit(())
                    })}> {"刷新"} </button>
                </div>
                <form class="mt-4 grid gap-3 rounded-box bg-base-200/60 p-4" onsubmit={on_submit} data-testid="admin-question-filters">
                    <label class="form-control">
                        <span class="label-text text-sm font-semibold">{"关键字"}</span>
                        <input class="input input-bordered bg-base-100" value={(*keyword).clone()} placeholder="搜索题干或题库名称" oninput={text_input_callback(keyword.clone())} data-testid="admin-question-keyword" />
                    </label>
                    <div class="grid gap-3 sm:grid-cols-3">
                        <label class="form-control">
                            <span class="label-text text-sm font-semibold">{"题库"}</span>
                            <select class="select select-bordered bg-base-100" value={selected_bank_value} onchange={on_bank} data-testid="admin-question-bank-filter">
                                <option value="" selected={bank_id.is_none()}> {"全部题库"}</option>
                                { for props.banks.iter().map(|bank| html! { <option value={bank.id.to_string()} selected={Some(bank.id) == *bank_id}>{ &bank.name }</option> }) }
                            </select>
                        </label>
                        <label class="form-control">
                            <span class="label-text text-sm font-semibold">{"题型"}</span>
                            <select class="select select-bordered bg-base-100" value={selected_type_value} onchange={on_type} data-testid="admin-question-type-filter">
                                <option value="" selected={question_type.is_none()}> {"全部题型"}</option>
                                <option value="single_choice" selected={*question_type == Some(QuestionType::SingleChoice)}> {"选择题"}</option>
                                <option value="multiple_choice" selected={*question_type == Some(QuestionType::MultipleChoice)}> {"多选题"}</option>
                                <option value="fill_blank" selected={*question_type == Some(QuestionType::FillBlank)}> {"填空题"}</option>
                                <option value="true_false" selected={*question_type == Some(QuestionType::TrueFalse)}> {"判断题"}</option>
                                <option value="short_answer" selected={*question_type == Some(QuestionType::ShortAnswer)}> {"简答题"}</option>
                            </select>
                        </label>
                        <label class="form-control">
                            <span class="label-text text-sm font-semibold">{"状态"}</span>
                            <select class="select select-bordered bg-base-100" value={selected_status_value} onchange={on_status} data-testid="admin-question-status-filter">
                                <option value="" selected={status.is_none()}> {"全部状态"}</option>
                                <option value="draft" selected={*status == Some(QuestionStatus::Draft)}> {"草稿"}</option>
                                <option value="published" selected={*status == Some(QuestionStatus::Published)}> {"已发布"}</option>
                                <option value="archived" selected={*status == Some(QuestionStatus::Archived)}> {"已下线"}</option>
                            </select>
                        </label>
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <button class="btn btn-primary btn-sm" type="submit" data-testid="admin-question-filter-submit">{"筛选"}</button>
                        <button class="btn btn-ghost btn-sm" type="button" onclick={on_reset} data-testid="admin-question-filter-reset">{"清除筛选"}</button>
                    </div>
                </form>
                <div class="mt-4">{ content }</div>
            </div>
        </section>
    }
}

#[derive(Properties, PartialEq)]
struct AdminQuestionRowProps {
    question: AdminQuestion,
    on_changed: Callback<()>,
}

#[function_component(AdminQuestionRow)]
fn admin_question_row(props: &AdminQuestionRowProps) -> Html {
    let pending = use_state(|| false);
    let error = use_state(|| None::<String>);
    let question = &props.question;

    let publish = admin_transition_callback(
        question.id,
        "publish",
        pending.clone(),
        error.clone(),
        props.on_changed.clone(),
    );
    let archive = admin_transition_callback(
        question.id,
        "archive",
        pending.clone(),
        error.clone(),
        props.on_changed.clone(),
    );

    html! {
        <article class="rounded-box border border-base-300 bg-base-100 p-4" data-testid="admin-question-row">
            <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                <div class="min-w-0">
                    <div class="flex flex-wrap items-center gap-2">
                        <span class={classes!("badge", question_status_class(question.status))}>{ question_status_label(question.status) }</span>
                        <span class="badge badge-outline">{ question_type_label(question.question_type) }</span>
                        <span class="badge badge-ghost">{ &question.question_bank_name }</span>
                    </div>
                    <h3 class="mt-3 break-words font-bold">{ &question.stem }</h3>
                </div>
                <div class="flex shrink-0 gap-2">
                    if question.status == QuestionStatus::Draft {
                        <button class="btn btn-primary btn-sm" type="button" onclick={publish} disabled={*pending} data-testid="publish-question">{"发布"}</button>
                    }
                    if question.status == QuestionStatus::Published {
                        <button class="btn btn-outline btn-sm" type="button" onclick={archive} disabled={*pending} data-testid="archive-question">{"下线"}</button>
                    }
                </div>
            </div>
            if let Some(message) = &*error {
                <p class="mt-3 text-sm text-error" role="alert">{ message }</p>
            }
        </article>
    }
}

fn admin_transition_callback(
    question_id: i64,
    action: &'static str,
    pending: UseStateHandle<bool>,
    error: UseStateHandle<Option<String>>,
    on_changed: Callback<()>,
) -> Callback<MouseEvent> {
    Callback::from(move |_| {
        if *pending {
            return;
        }
        pending.set(true);
        error.set(None);
        let pending = pending.clone();
        let error = error.clone();
        let on_changed = on_changed.clone();
        spawn_local(async move {
            let result = async {
                let response =
                    Request::post(&format!("/api/v1/admin/questions/{question_id}/{action}"))
                        .send()
                        .await
                        .map_err(|error| error.to_string())?;
                if response.ok() {
                    Ok(())
                } else {
                    Err(format!("操作失败（{}）", response.status()))
                }
            }
            .await;
            match result {
                Ok(()) => on_changed.emit(()),
                Err(message) => error.set(Some(message)),
            }
            pending.set(false);
        });
    })
}

fn question_status_label(status: QuestionStatus) -> &'static str {
    match status {
        QuestionStatus::Draft => "草稿",
        QuestionStatus::Published => "已发布",
        QuestionStatus::Archived => "已下线",
    }
}

fn question_status_class(status: QuestionStatus) -> &'static str {
    match status {
        QuestionStatus::Draft => "badge-warning",
        QuestionStatus::Published => "badge-success",
        QuestionStatus::Archived => "badge-ghost",
    }
}

#[derive(Clone, PartialEq)]
struct SelectedPaperQuestion {
    question_id: i64,
    score: String,
}

fn admin_questions_endpoint(filters: &AdminQuestionFilterValues) -> String {
    let mut parameters = Vec::new();
    if !filters.keyword.trim().is_empty() {
        parameters.push(format!(
            "keyword={}",
            encode_query_component(filters.keyword.trim())
        ));
    }
    if let Some(bank_id) = filters.bank_id {
        parameters.push(format!("bank_id={bank_id}"));
    }
    if let Some(question_type) = filters.question_type {
        parameters.push(format!("question_type={question_type}"));
    }
    if let Some(status) = filters.status {
        parameters.push(format!("status={status}"));
    }
    if parameters.is_empty() {
        ADMIN_QUESTIONS_ENDPOINT.to_owned()
    } else {
        format!("{ADMIN_QUESTIONS_ENDPOINT}?{}", parameters.join("&"))
    }
}

fn encode_query_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }
    encoded
}

fn network_error_message(action: &str, error: String) -> String {
    let lower = error.to_ascii_lowercase();
    if lower.contains("load failed")
        || lower.contains("failed to fetch")
        || lower.contains("networkerror")
    {
        format!("{action}：网络连接失败，请检查服务器是否正在运行后重试")
    } else {
        format!("{action}：{error}")
    }
}

async fn get_json<T>(endpoint: &str, fallback: &str) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let response = Request::get(endpoint)
        .send()
        .await
        .map_err(|error| network_error_message(fallback, error.to_string()))?;
    if !response.ok() {
        return Err(read_api_error(response, fallback).await);
    }
    response
        .json::<T>()
        .await
        .map_err(|error| network_error_message(fallback, error.to_string()))
}

async fn post_empty<T>(endpoint: &str, fallback: &str) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let response = Request::post(endpoint)
        .send()
        .await
        .map_err(|error| network_error_message(fallback, error.to_string()))?;
    if !response.ok() {
        return Err(read_api_error(response, fallback).await);
    }
    response
        .json::<T>()
        .await
        .map_err(|error| network_error_message(fallback, error.to_string()))
}

async fn post_json<P, T>(endpoint: &str, payload: &P, fallback: &str) -> Result<T, String>
where
    P: Serialize,
    T: DeserializeOwned,
{
    let request = Request::post(endpoint)
        .json(payload)
        .map_err(|error| network_error_message(fallback, error.to_string()))?;
    let response = request
        .send()
        .await
        .map_err(|error| network_error_message(fallback, error.to_string()))?;
    if !response.ok() {
        return Err(read_api_error(response, fallback).await);
    }
    response
        .json::<T>()
        .await
        .map_err(|error| network_error_message(fallback, error.to_string()))
}

async fn read_api_error(response: Response, fallback: &str) -> String {
    let status = response.status();
    response
        .json::<ApiErrorResponse>()
        .await
        .map(|payload| localize_paper_error(&payload.error))
        .unwrap_or_else(|_| format!("{fallback}（{status}）"))
}

fn localize_paper_error(error: &str) -> String {
    match error {
        "paper is not available" => "试卷当前不可参加".to_owned(),
        "maximum attempts reached" => "已达到这份试卷的最大考试次数".to_owned(),
        "attempt is already submitted" => "这次考试已经交卷，不能继续修改".to_owned(),
        "result is not available" => "考试结果暂时不可查看".to_owned(),
        "paper was not found" | "not found" => "试卷不存在或已不可见".to_owned(),
        "candidate field is required: student_number" => "请填写学号".to_owned(),
        "candidate field is required: name" => "请填写姓名".to_owned(),
        "selected question is not published" => "只能选择已发布题目".to_owned(),
        "selected question version changed" => "题目版本已经变化，请重新组卷".to_owned(),
        "paper is already in published state" => "试卷已经发布".to_owned(),
        "paper is already in archived state" => "试卷已经下线".to_owned(),
        other => other.to_owned(),
    }
}

#[derive(Properties, PartialEq)]
struct AdminPapersAppProps {
    user: UserIdentity,
}

#[function_component(AdminPapersApp)]
fn admin_papers_app(props: &AdminPapersAppProps) -> Html {
    let load_state = use_state(|| AdminPaperLoadState::Loading);
    let refresh_counter = use_state(|| 0_u32);

    {
        let load_state = load_state.clone();
        let refresh_counter = *refresh_counter;
        use_effect_with(refresh_counter, move |_| {
            spawn_local(async move {
                let result: Result<(Vec<AdminPaper>, Vec<AdminQuestion>), String> = async {
                    let papers =
                        get_json::<AdminPaperListResponse>(ADMIN_PAPERS_ENDPOINT, "试卷加载失败")
                            .await?
                            .items;
                    let questions = get_json::<AdminQuestionListResponse>(
                        ADMIN_QUESTIONS_ENDPOINT,
                        "题目加载失败",
                    )
                    .await?
                    .items;
                    Ok((papers, questions))
                }
                .await;

                match result {
                    Ok((papers, questions)) => {
                        load_state.set(AdminPaperLoadState::Ready { papers, questions })
                    }
                    Err(error) => load_state.set(AdminPaperLoadState::Error(error)),
                }
            });
            || ()
        });
    }

    let refresh = {
        let refresh_counter = refresh_counter.clone();
        Callback::from(move |_| refresh_counter.set(refresh_counter.wrapping_add(1)))
    };

    let content = match &*load_state {
        AdminPaperLoadState::Loading => html! {
            <div class="grid gap-4 lg:grid-cols-2" data-testid="admin-papers-loading-state">
                { for (0..3).map(|_| html! { <div class="skeleton h-40 w-full rounded-box" /> }) }
            </div>
        },
        AdminPaperLoadState::Error(error) => html! {
            <div class="alert alert-error" data-testid="admin-papers-error-state">
                { format!("试卷页面加载失败：{error}") }
            </div>
        },
        AdminPaperLoadState::Ready { papers, questions } => html! {
            <div class="grid gap-6 xl:grid-cols-[minmax(0,0.85fr)_minmax(0,1.15fr)]">
                <AdminPaperForm questions={questions.clone()} on_saved={refresh.clone()} />
                <AdminPaperList papers={papers.clone()} on_changed={refresh.clone()} />
            </div>
        },
    };

    html! {
        <AppShell
            user={props.user.clone()}
            eyebrow="XIAOLUOQUIZ / ADMIN"
            title="试卷组装"
            subtitle="从已发布题目中固定版本组装试卷，保存草稿后再发布给学生参加。"
            test_id="admin-papers-page"
            active={NavigationItem::PaperBuilder}
        >
            { content }
        </AppShell>
    }
}

#[derive(Properties, PartialEq)]
struct AdminPaperFormProps {
    questions: Vec<AdminQuestion>,
    on_saved: Callback<()>,
}

#[function_component(AdminPaperForm)]
fn admin_paper_form(props: &AdminPaperFormProps) -> Html {
    let title = use_state(String::new);
    let description = use_state(String::new);
    let audience = use_state(String::new);
    let mode = use_state(|| PaperMode::Exam);
    let open_at = use_state(String::new);
    let close_at = use_state(String::new);
    let duration_minutes = use_state(|| "60".to_owned());
    let max_attempts = use_state(|| "1".to_owned());
    let allow_resume = use_state(|| true);
    let auto_save = use_state(|| true);
    let auto_submit = use_state(|| true);
    let selected_questions = use_state(Vec::<SelectedPaperQuestion>::new);
    let require_student_number = use_state(|| true);
    let require_name = use_state(|| false);
    let result_visibility = use_state(|| ResultVisibility::AfterSubmit);
    let allow_preview = use_state(|| false);
    let error = use_state(|| None::<String>);
    let saving = use_state(|| false);

    let on_mode = {
        let mode = mode.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            mode.set(if select.value() == "practice" {
                PaperMode::Practice
            } else {
                PaperMode::Exam
            });
        })
    };
    let on_result_visibility = {
        let result_visibility = result_visibility.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            let value = match select.value().as_str() {
                "after_grading" => ResultVisibility::AfterGrading,
                "admin_release" => ResultVisibility::AdminRelease,
                _ => ResultVisibility::AfterSubmit,
            };
            result_visibility.set(value);
        })
    };
    let on_question_toggle = {
        let selected_questions = selected_questions.clone();
        Callback::from(move |(question_id, checked): (i64, bool)| {
            let mut selected = (*selected_questions).clone();
            if checked {
                if !selected.iter().any(|item| item.question_id == question_id) {
                    selected.push(SelectedPaperQuestion {
                        question_id,
                        score: "1".to_owned(),
                    });
                }
            } else {
                selected.retain(|item| item.question_id != question_id);
            }
            selected_questions.set(selected);
        })
    };

    let on_submit = {
        let title = title.clone();
        let description = description.clone();
        let audience = audience.clone();
        let mode = mode.clone();
        let open_at = open_at.clone();
        let close_at = close_at.clone();
        let duration_minutes = duration_minutes.clone();
        let max_attempts = max_attempts.clone();
        let allow_resume = allow_resume.clone();
        let auto_save = auto_save.clone();
        let auto_submit = auto_submit.clone();
        let selected_questions = selected_questions.clone();
        let require_student_number = require_student_number.clone();
        let require_name = require_name.clone();
        let result_visibility = result_visibility.clone();
        let allow_preview = allow_preview.clone();
        let error = error.clone();
        let saving = saving.clone();
        let on_saved = props.on_saved.clone();

        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            if *saving {
                return;
            }
            error.set(None);

            let duration_minutes_value = match duration_minutes.parse::<i64>() {
                Ok(value) if value > 0 => value,
                _ => {
                    error.set(Some("答题时长必须是大于 0 的整数分钟".to_owned()));
                    return;
                }
            };
            let max_attempts_value = match max_attempts.parse::<u16>() {
                Ok(value) if value > 0 => value,
                _ => {
                    error.set(Some("最大考试次数必须是大于 0 的整数".to_owned()));
                    return;
                }
            };
            if title.trim().is_empty() {
                error.set(Some("试卷标题不能为空".to_owned()));
                return;
            }
            if selected_questions.is_empty() {
                error.set(Some("请至少选择一道已发布题目".to_owned()));
                return;
            }

            let mut questions = Vec::with_capacity(selected_questions.len());
            for selected in selected_questions.iter() {
                let score = match selected.score.parse::<f64>() {
                    Ok(value) if value.is_finite() && value > 0.0 => value,
                    _ => {
                        error.set(Some("题目分值必须是大于 0 的数字".to_owned()));
                        return;
                    }
                };
                questions.push(PaperQuestionInput {
                    question_id: selected.question_id,
                    score: Some(score),
                });
            }

            let candidate_fields = if *mode == PaperMode::Exam {
                let mut fields = Vec::new();
                if *require_student_number {
                    fields.push(CandidateFieldConfig {
                        key: CandidateField::StudentNumber,
                        required: true,
                    });
                }
                if *require_name {
                    fields.push(CandidateFieldConfig {
                        key: CandidateField::Name,
                        required: true,
                    });
                }
                fields
            } else {
                Vec::new()
            };
            let open_at_value = match parse_optional_datetime_local((*open_at).as_str()) {
                Ok(value) => value,
                Err(()) => {
                    error.set(Some("开放时间格式无效".to_owned()));
                    return;
                }
            };
            let close_at_value = match parse_optional_datetime_local((*close_at).as_str()) {
                Ok(value) => value,
                Err(()) => {
                    error.set(Some("截止时间格式无效".to_owned()));
                    return;
                }
            };
            let input = CreatePaperInput {
                title: (*title).clone(),
                description: non_empty((*description).clone()),
                audience: non_empty((*audience).clone()),
                mode: *mode,
                open_at: open_at_value,
                close_at: close_at_value,
                duration_seconds: Some(duration_minutes_value.saturating_mul(60)),
                max_attempts: max_attempts_value,
                allow_resume: *allow_resume,
                auto_save: *auto_save,
                auto_submit: *auto_submit,
                candidate_fields,
                result_visibility: *result_visibility,
                allow_preview: *allow_preview,
                questions,
            };

            saving.set(true);
            let error = error.clone();
            let saving = saving.clone();
            let title = title.clone();
            let description = description.clone();
            let audience = audience.clone();
            let open_at = open_at.clone();
            let close_at = close_at.clone();
            let selected_questions = selected_questions.clone();
            let on_saved = on_saved.clone();
            spawn_local(async move {
                let result: Result<AdminPaper, String> =
                    post_json(ADMIN_PAPERS_ENDPOINT, &input, "创建试卷失败").await;
                match result {
                    Ok(_) => {
                        title.set(String::new());
                        description.set(String::new());
                        audience.set(String::new());
                        open_at.set(String::new());
                        close_at.set(String::new());
                        selected_questions.set(Vec::new());
                        on_saved.emit(());
                    }
                    Err(message) => error.set(Some(message)),
                }
                saving.set(false);
            });
        })
    };

    let published_questions = props
        .questions
        .iter()
        .filter(|question| question.status == QuestionStatus::Published);
    let selected_items = selected_questions
        .iter()
        .enumerate()
        .map(|(index, selected)| {
            let question_stem = props
                .questions
                .iter()
                .find(|question| question.id == selected.question_id)
                .map(|question| question.stem.clone())
                .unwrap_or_else(|| "题目已不存在".to_owned());
            let selected_questions = selected_questions.clone();
            let question_id = selected.question_id;
            let on_score = Callback::from(move |event: InputEvent| {
                let input: HtmlInputElement = event.target_unchecked_into();
                let mut values = (*selected_questions).clone();
                if let Some(item) = values.iter_mut().find(|item| item.question_id == question_id) {
                    item.score = input.value();
                }
                selected_questions.set(values);
            });
            html! {
                <div key={question_id.to_string()} class="grid gap-2 rounded-box border border-primary/25 bg-primary/5 p-3" data-testid={format!("selected-paper-question-{question_id}")}>
                    <div class="flex items-start gap-3">
                        <span class="badge badge-primary mt-1">{ index + 1 }</span>
                        <p class="min-w-0 flex-1 text-sm font-semibold">{ question_stem }</p>
                    </div>
                    <label class="form-control sm:ml-9 sm:max-w-48">
                        <span class="label-text text-xs">{"卷面分值（组卷时填写）"}</span>
                        <input class="input input-bordered input-sm bg-base-100" type="number" min="0.01" step="0.01" value={selected.score.clone()} oninput={on_score} data-testid={format!("admin-paper-score-{question_id}")} />
                    </label>
                </div>
            }
        });

    html! {
        <section class="card border border-base-300 bg-base-100 shadow-sm" data-testid="admin-paper-form">
            <div class="card-body">
                <h2 class="card-title">{"新建试卷草稿"}</h2>
                <p class="mt-1 text-sm text-base-content/60">{"题目按选择顺序固定为试卷顺序，发布后不能直接修改。"}</p>
                <form class="mt-4 grid gap-4" onsubmit={on_submit}>
                    <label class="form-control">
                        <span class="label-text text-sm font-semibold">{"试卷标题"}</span>
                        <input class="input input-bordered bg-base-100" value={(*title).clone()} oninput={text_input_callback(title.clone())} data-testid="admin-paper-title" />
                    </label>
                    <label class="form-control">
                        <span class="label-text text-sm font-semibold">{"说明（可选）"}</span>
                        <textarea class="textarea textarea-bordered min-h-20 bg-base-100" value={(*description).clone()} oninput={text_area_callback(description.clone())} data-testid="admin-paper-description" />
                    </label>
                    <label class="form-control">
                        <span class="label-text text-sm font-semibold">{"考试对象（可选）"}</span>
                        <input class="input input-bordered bg-base-100" value={(*audience).clone()} oninput={text_input_callback(audience.clone())} data-testid="admin-paper-audience" />
                    </label>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <label class="form-control">
                            <span class="label-text text-sm font-semibold">{"模式"}</span>
                            <select class="select select-bordered bg-base-100" value={paper_mode_value(*mode)} onchange={on_mode} data-testid="admin-paper-mode">
                                <option value="exam" selected={*mode == PaperMode::Exam}>{"正式考试"}</option>
                                <option value="practice" selected={*mode == PaperMode::Practice}>{"练习试卷"}</option>
                            </select>
                        </label>
                        <label class="form-control">
                            <span class="label-text text-sm font-semibold">{"答题时长（分钟）"}</span>
                            <input class="input input-bordered bg-base-100" type="number" min="1" step="1" value={(*duration_minutes).clone()} oninput={text_input_callback(duration_minutes.clone())} data-testid="admin-paper-duration" />
                        </label>
                    </div>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <label class="form-control">
                            <span class="label-text text-sm font-semibold">{"开放时间（上海时间，可选）"}</span>
                            <input class="input input-bordered bg-base-100" type="datetime-local" value={(*open_at).clone()} oninput={text_input_callback(open_at.clone())} data-testid="admin-paper-open-at" />
                        </label>
                        <label class="form-control">
                            <span class="label-text text-sm font-semibold">{"截止时间（上海时间，可选）"}</span>
                            <input class="input input-bordered bg-base-100" type="datetime-local" value={(*close_at).clone()} oninput={text_input_callback(close_at.clone())} data-testid="admin-paper-close-at" />
                        </label>
                    </div>
                    <div class="grid gap-4 sm:grid-cols-2">
                        <label class="form-control">
                            <span class="label-text text-sm font-semibold">{"最大考试次数"}</span>
                            <input class="input input-bordered bg-base-100" type="number" min="1" step="1" value={(*max_attempts).clone()} oninput={text_input_callback(max_attempts.clone())} data-testid="admin-paper-max-attempts" />
                        </label>
                        <label class="form-control">
                            <span class="label-text text-sm font-semibold">{"结果可见性"}</span>
                            <select class="select select-bordered bg-base-100" value={result_visibility_value(*result_visibility)} onchange={on_result_visibility} data-testid="admin-paper-result-visibility">
                                <option value="after_submit" selected={*result_visibility == ResultVisibility::AfterSubmit}>{"交卷后可见"}</option>
                                <option value="after_grading" selected={*result_visibility == ResultVisibility::AfterGrading}>{"批改后可见"}</option>
                                <option value="admin_release" selected={*result_visibility == ResultVisibility::AdminRelease}>{"管理员发布后可见"}</option>
                            </select>
                        </label>
                    </div>
                    <div class="grid gap-3 rounded-box bg-base-200/60 p-4">
                        <p class="text-sm font-bold">{"答题规则"}</p>
                        <label class="label cursor-pointer justify-start gap-3">
                            <input class="checkbox checkbox-primary" type="checkbox" checked={*allow_resume} onchange={Callback::from({ let allow_resume = allow_resume.clone(); move |event: Event| { let input: HtmlInputElement = event.target_unchecked_into(); allow_resume.set(input.checked()); } })} data-testid="admin-paper-allow-resume" />
                            <span class="label-text">{"允许中途退出后继续"}</span>
                        </label>
                        <label class="label cursor-pointer justify-start gap-3">
                            <input class="checkbox checkbox-primary" type="checkbox" checked={*auto_save} onchange={Callback::from({ let auto_save = auto_save.clone(); move |event: Event| { let input: HtmlInputElement = event.target_unchecked_into(); auto_save.set(input.checked()); } })} data-testid="admin-paper-auto-save" />
                            <span class="label-text">{"允许保存答案"}</span>
                        </label>
                        <label class="label cursor-pointer justify-start gap-3">
                            <input class="checkbox checkbox-primary" type="checkbox" checked={*auto_submit} onchange={Callback::from({ let auto_submit = auto_submit.clone(); move |event: Event| { let input: HtmlInputElement = event.target_unchecked_into(); auto_submit.set(input.checked()); } })} data-testid="admin-paper-auto-submit" />
                            <span class="label-text">{"到时自动结束答题"}</span>
                        </label>
                    </div>
                    <div class="grid gap-3 rounded-box bg-base-200/60 p-4">
                        <label class="label cursor-pointer justify-start gap-3">
                            <input class="checkbox checkbox-primary" type="checkbox" checked={*require_student_number} disabled={*mode == PaperMode::Practice} onchange={Callback::from({ let require_student_number = require_student_number.clone(); move |event: Event| { let input: HtmlInputElement = event.target_unchecked_into(); require_student_number.set(input.checked()); } })} data-testid="admin-paper-require-student-number" />
                            <span class="label-text">{"要求填写学号"}</span>
                        </label>
                        <label class="label cursor-pointer justify-start gap-3">
                            <input class="checkbox checkbox-primary" type="checkbox" checked={*require_name} disabled={*mode == PaperMode::Practice} onchange={Callback::from({ let require_name = require_name.clone(); move |event: Event| { let input: HtmlInputElement = event.target_unchecked_into(); require_name.set(input.checked()); } })} data-testid="admin-paper-require-name" />
                            <span class="label-text">{"要求填写姓名"}</span>
                        </label>
                        <label class="label cursor-pointer justify-start gap-3">
                            <input class="checkbox checkbox-primary" type="checkbox" checked={*allow_preview} onchange={Callback::from({ let allow_preview = allow_preview.clone(); move |event: Event| { let input: HtmlInputElement = event.target_unchecked_into(); allow_preview.set(input.checked()); } })} data-testid="admin-paper-allow-preview" />
                            <span class="label-text">{"允许开始前预览题目"}</span>
                        </label>
                    </div>
                    <div class="grid gap-3">
                        <div>
                            <h3 class="font-bold">{"选择已发布题目"}</h3>
                            <p class="mt-1 text-xs text-base-content/60">{"勾选顺序就是试卷题目顺序；已选题目可在下方调整分值。"}</p>
                        </div>
                        <div class="grid gap-3" data-testid="admin-paper-question-list">
                            { for published_questions.map(|question| {
                                let question_id = question.id;
                                let checked = selected_questions.iter().any(|item| item.question_id == question_id);
                                let on_question_toggle = on_question_toggle.clone();
                                let on_toggle = Callback::from(move |event: Event| {
                                    let input: HtmlInputElement = event.target_unchecked_into();
                                    on_question_toggle.emit((question_id, input.checked()));
                                });
                                html! {
                                    <label key={question_id.to_string()} class="flex cursor-pointer items-start gap-3 rounded-box border border-base-300 bg-base-200/40 p-3 transition-colors has-[:checked]:border-primary has-[:checked]:bg-primary/10" data-testid="admin-paper-question">
                                        <input class="checkbox checkbox-primary mt-0.5" type="checkbox" checked={checked} onchange={on_toggle} />
                                        <span class="min-w-0">
                                            <span class="flex flex-wrap items-center gap-2">
                                                <span class="badge badge-outline">{ question_type_label(question.question_type) }</span>
                                                <span class="badge badge-ghost">{ &question.question_bank_name }</span>
                                            </span>
                                            <span class="mt-1 block text-sm font-medium">{ &question.stem }</span>
                                        </span>
                                    </label>
                                }
                            }) }
                            if props.questions.iter().all(|question| question.status != QuestionStatus::Published) {
                                <div class="rounded-box border border-dashed border-base-300 p-6 text-center text-sm text-base-content/60">{"当前没有已发布题目可供选择。"}</div>
                            }
                        </div>
                    </div>
                    if !selected_questions.is_empty() {
                        <div class="grid gap-3 rounded-box bg-base-200/40 p-4" data-testid="selected-paper-question-list">
                            <h3 class="font-bold">{"试卷题目顺序与分值"}</h3>
                            { for selected_items }
                        </div>
                    }
                    if let Some(message) = &*error {
                        <p class="text-sm text-error" role="alert" data-testid="admin-paper-form-error">{ message }</p>
                    }
                    <button class="btn btn-secondary w-full" type="submit" disabled={*saving} data-testid="admin-paper-save">
                        { if *saving { "保存中…" } else { "保存试卷草稿" } }
                    </button>
                </form>
            </div>
        </section>
    }
}

#[derive(Properties, PartialEq)]
struct AdminPaperListProps {
    papers: Vec<AdminPaper>,
    on_changed: Callback<()>,
}

#[function_component(AdminPaperList)]
fn admin_paper_list(props: &AdminPaperListProps) -> Html {
    let content = if props.papers.is_empty() {
        html! {
            <div class="rounded-box border border-dashed border-base-300 p-10 text-center" data-testid="admin-paper-empty-state">
                <p class="font-bold">{"还没有试卷"}</p>
                <p class="mt-2 text-sm text-base-content/60">{"保存试卷草稿后，它会出现在这里。"}</p>
            </div>
        }
    } else {
        html! {
            <div class="grid gap-3" data-testid="admin-paper-list">
                { for props.papers.iter().map(|paper| html! {
                    <AdminPaperRow key={paper.id.to_string()} paper={paper.clone()} on_changed={props.on_changed.clone()} />
                }) }
            </div>
        }
    };

    html! {
        <section class="card border border-base-300 bg-base-100 shadow-sm">
            <div class="card-body">
                <div class="flex items-center justify-between gap-4">
                    <div>
                        <h2 class="card-title">{"试卷列表"}</h2>
                        <p class="mt-1 text-sm text-base-content/60">{"草稿、已发布和已下线试卷"}</p>
                    </div>
                    <button class="btn btn-ghost btn-sm" type="button" onclick={Callback::from({ let on_changed = props.on_changed.clone(); move |_| on_changed.emit(()) })}>{{"刷新"}}</button>
                </div>
                <div class="mt-4">{ content }</div>
            </div>
        </section>
    }
}

#[derive(Properties, PartialEq)]
struct AdminPaperRowProps {
    paper: AdminPaper,
    on_changed: Callback<()>,
}

#[function_component(AdminPaperRow)]
fn admin_paper_row(props: &AdminPaperRowProps) -> Html {
    let pending = use_state(|| false);
    let error = use_state(|| None::<String>);
    let paper = &props.paper;
    let publish = paper_transition_callback(
        paper.id,
        "publish",
        pending.clone(),
        error.clone(),
        props.on_changed.clone(),
    );
    let archive = paper_transition_callback(
        paper.id,
        "archive",
        pending.clone(),
        error.clone(),
        props.on_changed.clone(),
    );

    html! {
        <article class="rounded-box border border-base-300 bg-base-100 p-4" data-testid="admin-paper-row">
            <div class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
                <div class="min-w-0">
                    <div class="flex flex-wrap items-center gap-2">
                        <span class={classes!("badge", paper_status_class(paper.status))}>{ paper_status_label(paper.status) }</span>
                        <span class="badge badge-outline">{ paper_mode_label(paper.mode) }</span>
                        <span class="text-xs text-base-content/50">{ format!("{} 道题 · {:.2} 分", paper.items.len(), paper.total_score) }</span>
                    </div>
                    <h3 class="mt-3 break-words text-lg font-bold">{ &paper.title }</h3>
                    if let Some(description) = &paper.description {
                        <p class="mt-1 whitespace-pre-wrap text-sm text-base-content/65">{ description }</p>
                    }
                    <p class="mt-2 text-xs text-base-content/55">
                        { format!("{} 次机会 · {} · 结果：{}", paper.max_attempts, duration_label(paper.duration_seconds), result_visibility_label(paper.result_visibility)) }
                    </p>
                    if let Some(open_at) = &paper.open_at {
                        <p class="mt-2 text-xs text-base-content/55" data-testid="admin-paper-open-time">{ format!("开放时间（上海时间）：{}", shanghai_datetime_label(open_at)) }</p>
                    }
                    if let Some(close_at) = &paper.close_at {
                        <p class="text-xs text-base-content/55" data-testid="admin-paper-close-time">{ format!("截止时间（上海时间）：{}", shanghai_datetime_label(close_at)) }</p>
                    }
                </div>
                <div class="flex shrink-0 flex-wrap gap-2 sm:justify-end">
                    if paper.status == PaperStatus::Draft {
                        <button class="btn btn-primary btn-sm" type="button" onclick={publish} disabled={*pending} data-testid="publish-paper">{"发布"}</button>
                    }
                    if paper.status == PaperStatus::Published {
                        <button class="btn btn-outline btn-sm" type="button" onclick={archive} disabled={*pending} data-testid="archive-paper">{"下线"}</button>
                    }
                </div>
            </div>
            if !paper.candidate_fields.is_empty() {
                <p class="mt-3 text-sm text-base-content/65">{ format!("考生字段：{}", paper.candidate_fields.iter().map(|field| candidate_field_label(field.key)).collect::<Vec<_>>().join("、")) }</p>
            }
            if let Some(message) = &*error {
                <p class="mt-3 text-sm text-error" role="alert">{ message }</p>
            }
        </article>
    }
}

fn paper_transition_callback(
    paper_id: i64,
    action: &'static str,
    pending: UseStateHandle<bool>,
    error: UseStateHandle<Option<String>>,
    on_changed: Callback<()>,
) -> Callback<MouseEvent> {
    Callback::from(move |_| {
        if *pending {
            return;
        }
        pending.set(true);
        error.set(None);
        let pending = pending.clone();
        let error = error.clone();
        let on_changed = on_changed.clone();
        spawn_local(async move {
            let result: Result<AdminPaper, String> = post_empty(
                &format!("{ADMIN_PAPERS_ENDPOINT}/{paper_id}/{action}"),
                "试卷状态更新失败",
            )
            .await;
            match result {
                Ok(_) => on_changed.emit(()),
                Err(message) => error.set(Some(message)),
            }
            pending.set(false);
        });
    })
}

fn paper_mode_value(mode: PaperMode) -> &'static str {
    match mode {
        PaperMode::Exam => "exam",
        PaperMode::Practice => "practice",
    }
}

fn paper_mode_label(mode: PaperMode) -> &'static str {
    match mode {
        PaperMode::Exam => "正式考试",
        PaperMode::Practice => "练习试卷",
    }
}

fn paper_status_label(status: PaperStatus) -> &'static str {
    match status {
        PaperStatus::Draft => "草稿",
        PaperStatus::Published => "已发布",
        PaperStatus::Archived => "已下线",
    }
}

fn paper_status_class(status: PaperStatus) -> &'static str {
    match status {
        PaperStatus::Draft => "badge-warning",
        PaperStatus::Published => "badge-success",
        PaperStatus::Archived => "badge-ghost",
    }
}

fn result_visibility_value(visibility: ResultVisibility) -> &'static str {
    match visibility {
        ResultVisibility::AfterSubmit => "after_submit",
        ResultVisibility::AfterGrading => "after_grading",
        ResultVisibility::AdminRelease => "admin_release",
    }
}

fn result_visibility_label(visibility: ResultVisibility) -> &'static str {
    match visibility {
        ResultVisibility::AfterSubmit => "交卷后可见",
        ResultVisibility::AfterGrading => "批改后可见",
        ResultVisibility::AdminRelease => "管理员发布后可见",
    }
}

fn candidate_field_label(field: CandidateField) -> &'static str {
    match field {
        CandidateField::StudentNumber => "学号",
        CandidateField::Name => "姓名",
    }
}

fn duration_label(duration_seconds: Option<i64>) -> String {
    duration_seconds.map_or_else(
        || "不限时".to_owned(),
        |seconds| format!("{} 分钟", (seconds + 59) / 60),
    )
}

#[derive(Properties, PartialEq)]
struct PapersAppProps {
    user: UserIdentity,
}

#[function_component(PapersApp)]
fn papers_app(props: &PapersAppProps) -> Html {
    let load_state = use_state(|| PaperListLoadState::Loading);

    {
        let load_state = load_state.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                match get_json::<PaperListResponse>(PAPERS_ENDPOINT, "试卷列表加载失败").await
                {
                    Ok(payload) => load_state.set(PaperListLoadState::Ready(payload.items)),
                    Err(error) => load_state.set(PaperListLoadState::Error(error)),
                }
            });
            || ()
        });
    }

    let content = match &*load_state {
        PaperListLoadState::Loading => html! {
            <div class="grid gap-4 md:grid-cols-2" data-testid="paper-loading-state">
                { for (0..3).map(|_| html! { <div class="skeleton h-44 w-full rounded-box" /> }) }
            </div>
        },
        PaperListLoadState::Error(error) => html! {
            <div class="alert alert-error" data-testid="paper-error-state">
                { format!("试卷加载失败：{error}") }
            </div>
        },
        PaperListLoadState::Ready(papers) if papers.is_empty() => html! {
            <div class="rounded-box border border-dashed border-base-300 bg-base-100 p-10 text-center" data-testid="paper-empty-state">
                <p class="text-lg font-bold">{"当前没有可参加的正式考试"}</p>
                <p class="mt-2 text-sm text-base-content/60">{"管理员发布试卷后，它会出现在这里。"}</p>
            </div>
        },
        PaperListLoadState::Ready(papers) => html! {
            <div class="grid gap-5 lg:grid-cols-2" data-testid="published-paper-list">
                { for papers.iter().map(|paper| html! { <PaperCard paper={paper.clone()} /> }) }
            </div>
        },
    };

    html! {
        <AppShell
            user={props.user.clone()}
            eyebrow="XIAOLUOQUIZ / EXAM"
            title="正式考试"
            subtitle="进入考试前填写考生信息；答题过程会保存到服务端，刷新页面后可以继续。"
            test_id="paper-list"
            active={NavigationItem::Papers}
        >
            { content }
        </AppShell>
    }
}

#[derive(Properties, PartialEq)]
struct PaperCardProps {
    paper: PublishedPaper,
}

#[function_component(PaperCard)]
fn paper_card(props: &PaperCardProps) -> Html {
    let paper = &props.paper;
    let action = if let (Some(attempt_id), Some(AttemptStatus::InProgress)) =
        (paper.current_attempt_id, paper.current_attempt_status)
    {
        html! {
            <a class="btn btn-primary" href={format!("/exam/{attempt_id}")} data-testid="continue-paper">{"继续考试"}</a>
        }
    } else if paper.runtime_status == PaperRuntimeStatus::Open {
        html! {
            <a class="btn btn-primary" href={format!("/papers/{}/start", paper.id)} data-testid="start-paper">{"开始考试"}</a>
        }
    } else {
        html! {
            <button class="btn btn-disabled" type="button" disabled={true} data-testid="start-paper">
                { paper_runtime_status_label(paper.runtime_status) }
            </button>
        }
    };

    html! {
        <article class="card w-full min-w-0 border border-base-300 bg-base-100 shadow-sm" data-testid="paper-card">
            <div class="card-body gap-4">
                <div class="flex items-start justify-between gap-4">
                    <div class="min-w-0">
                        <div class="flex flex-wrap items-center gap-2">
                            <span class={classes!("badge", paper_runtime_status_class(paper.runtime_status))}>{ paper_runtime_status_label(paper.runtime_status) }</span>
                            <span class="badge badge-outline">{ paper_mode_label(paper.mode) }</span>
                        </div>
                        <h2 class="mt-3 break-words text-xl font-bold">{ &paper.title }</h2>
                    </div>
                    <span class="shrink-0 text-sm font-semibold text-base-content/55">{ format!("{:.2} 分", paper.total_score) }</span>
                </div>
                if let Some(description) = &paper.description {
                    <p class="whitespace-pre-wrap text-sm text-base-content/65">{ description }</p>
                }
                <div class="grid gap-2 rounded-box bg-base-200/60 p-3 text-sm text-base-content/70">
                    <p>{ format!("{} 道题 · {} · 最多 {} 次", paper.question_count, duration_label(paper.duration_seconds), paper.max_attempts) }</p>
                    if let Some(audience) = &paper.audience {
                        <p>{ format!("考试对象：{audience}") }</p>
                    }
                    if let Some(open_at) = &paper.open_at {
                        <p data-testid="paper-open-time">{ format!("开放时间（上海时间）：{}", shanghai_datetime_label(open_at)) }</p>
                    }
                    if let Some(close_at) = &paper.close_at {
                        <p data-testid="paper-close-time">{ format!("截止时间（上海时间）：{}", shanghai_datetime_label(close_at)) }</p>
                    }
                    if !paper.candidate_fields.is_empty() {
                        <p>{ format!("开始前填写：{}", paper.candidate_fields.iter().filter(|field| field.required).map(|field| candidate_field_label(field.key)).collect::<Vec<_>>().join("、")) }</p>
                    }
                </div>
                <div class="flex flex-wrap items-center justify-between gap-3">
                    <span class="text-xs text-base-content/55">{ format!("结果：{}", result_visibility_label(paper.result_visibility)) }</span>
                    { action }
                </div>
            </div>
        </article>
    }
}

fn paper_runtime_status_label(status: PaperRuntimeStatus) -> &'static str {
    match status {
        PaperRuntimeStatus::Upcoming => "未开始",
        PaperRuntimeStatus::Open => "进行中",
        PaperRuntimeStatus::Closed => "已结束",
    }
}

fn paper_runtime_status_class(status: PaperRuntimeStatus) -> &'static str {
    match status {
        PaperRuntimeStatus::Upcoming => "badge-warning",
        PaperRuntimeStatus::Open => "badge-success",
        PaperRuntimeStatus::Closed => "badge-ghost",
    }
}

#[derive(Properties, PartialEq)]
struct PaperStartPageProps {
    user: UserIdentity,
    paper_id: i64,
}

#[function_component(PaperStartPage)]
fn paper_start_page(props: &PaperStartPageProps) -> Html {
    let load_state = use_state(|| PaperLoadState::Loading);
    let student_number = {
        let initial = props.user.student_number.clone().unwrap_or_default();
        use_state(move || initial)
    };
    let name = {
        let initial = props.user.display_name.clone();
        use_state(move || initial)
    };
    let error = use_state(|| None::<String>);
    let starting = use_state(|| false);

    {
        let load_state = load_state.clone();
        let paper_id = props.paper_id;
        use_effect_with(paper_id, move |_| {
            spawn_local(async move {
                match get_json::<PublishedPaper>(
                    &format!("{PAPERS_ENDPOINT}/{paper_id}"),
                    "试卷详情加载失败",
                )
                .await
                {
                    Ok(paper) => load_state.set(PaperLoadState::Ready(paper)),
                    Err(error) => load_state.set(PaperLoadState::Error(error)),
                }
            });
            || ()
        });
    }

    let required_student_number = match &*load_state {
        PaperLoadState::Ready(paper) => paper
            .candidate_fields
            .iter()
            .any(|field| field.key == CandidateField::StudentNumber && field.required),
        _ => false,
    };
    let required_name = match &*load_state {
        PaperLoadState::Ready(paper) => paper
            .candidate_fields
            .iter()
            .any(|field| field.key == CandidateField::Name && field.required),
        _ => false,
    };

    let on_submit = {
        let student_number = student_number.clone();
        let name = name.clone();
        let error = error.clone();
        let starting = starting.clone();
        let paper_id = props.paper_id;
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            if *starting {
                return;
            }
            error.set(None);
            let candidate_info = CandidateInfo {
                student_number: non_empty((*student_number).clone()),
                name: non_empty((*name).clone()),
            };
            if required_student_number && candidate_info.student_number.is_none() {
                error.set(Some("请填写学号".to_owned()));
                return;
            }
            if required_name && candidate_info.name.is_none() {
                error.set(Some("请填写姓名".to_owned()));
                return;
            }

            starting.set(true);
            let error = error.clone();
            let starting = starting.clone();
            spawn_local(async move {
                let result: Result<ExamAttempt, String> = post_json(
                    &format!("{PAPERS_ENDPOINT}/{paper_id}/attempts"),
                    &candidate_info,
                    "开始考试失败",
                )
                .await;
                match result {
                    Ok(attempt) => navigate_to(&format!("/exam/{}", attempt.id)),
                    Err(message) => error.set(Some(message)),
                }
                starting.set(false);
            });
        })
    };

    let body = match &*load_state {
        PaperLoadState::Loading => html! {
            <div class="grid gap-4" data-testid="paper-start-loading-state">
                <div class="skeleton h-28 w-full rounded-box" />
                <div class="skeleton h-72 w-full rounded-box" />
            </div>
        },
        PaperLoadState::Error(message) => html! {
            <div class="alert alert-error" data-testid="paper-start-error-state">{ format!("无法开始考试：{message}") }</div>
        },
        PaperLoadState::Ready(paper) => {
            let resumed_attempt = match (paper.current_attempt_id, paper.current_attempt_status) {
                (Some(attempt_id), Some(AttemptStatus::InProgress)) => Some(attempt_id),
                _ => None,
            };
            html! {
                <div class="grid gap-6 lg:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]">
                    <section class="card border border-base-300 bg-base-100 shadow-sm">
                        <div class="card-body gap-4">
                            <div class="flex flex-wrap items-center gap-2">
                                <span class={classes!("badge", paper_runtime_status_class(paper.runtime_status))}>{ paper_runtime_status_label(paper.runtime_status) }</span>
                                <span class="badge badge-outline">{ paper_mode_label(paper.mode) }</span>
                            </div>
                            <h2 class="card-title text-2xl">{ &paper.title }</h2>
                            if let Some(description) = &paper.description {
                                <p class="whitespace-pre-wrap text-sm text-base-content/65">{ description }</p>
                            }
                            <div class="grid gap-2 rounded-box bg-base-200/60 p-4 text-sm text-base-content/70">
                                <p>{ format!("{} 道题 · {:.2} 分", paper.question_count, paper.total_score) }</p>
                                <p>{ format!("答题时长：{} · 最大次数：{}", duration_label(paper.duration_seconds), paper.max_attempts) }</p>
                                if let Some(open_at) = &paper.open_at {
                                    <p data-testid="paper-start-open-time">{ format!("开放时间（上海时间）：{}", shanghai_datetime_label(open_at)) }</p>
                                }
                                if let Some(close_at) = &paper.close_at {
                                    <p data-testid="paper-start-close-time">{ format!("截止时间（上海时间）：{}", shanghai_datetime_label(close_at)) }</p>
                                }
                            </div>
                            if let Some(attempt_id) = resumed_attempt {
                                <div class="alert alert-info items-start">
                                    <div>
                                        <strong>{"检测到未完成的考试"}</strong>
                                        <p class="text-sm">{"可以继续上次保存的答题记录。"}</p>
                                    </div>
                                    <a class="btn btn-info btn-sm" href={format!("/exam/{attempt_id}")} data-testid="continue-exam">{"继续考试"}</a>
                                </div>
                            }
                        </div>
                    </section>
                    <section class="card border border-base-300 bg-base-100 shadow-sm">
                        <div class="card-body">
                            <h2 class="card-title">{"填写考生信息"}</h2>
                            <p class="mt-1 text-sm text-base-content/60">{"这些信息会和本次答题记录一起保存。"}</p>
                            if paper.candidate_fields.is_empty() {
                                <p class="mt-5 rounded-box bg-base-200/60 p-4 text-sm text-base-content/65">{"这份试卷不要求额外填写考生信息。"}</p>
                            }
                            <form class="mt-5 grid gap-4" onsubmit={on_submit}>
                                { for paper.candidate_fields.iter().map(|field| match field.key {
                                    CandidateField::StudentNumber => html! {
                                        <label class="form-control">
                                            <span class="label-text text-sm font-semibold">{"学号"}{ if field.required { "（必填）" } else { "" } }</span>
                                            <input class="input input-bordered bg-base-100" autocomplete="off" required={field.required} value={(*student_number).clone()} oninput={text_input_callback(student_number.clone())} data-testid="candidate-student-number" />
                                        </label>
                                    },
                                    CandidateField::Name => html! {
                                        <label class="form-control">
                                            <span class="label-text text-sm font-semibold">{"姓名"}{ if field.required { "（必填）" } else { "" } }</span>
                                            <input class="input input-bordered bg-base-100" autocomplete="name" required={field.required} value={(*name).clone()} oninput={text_input_callback(name.clone())} data-testid="candidate-name" />
                                        </label>
                                    },
                                }) }
                                if paper.runtime_status != PaperRuntimeStatus::Open {
                                    <div class="alert alert-warning">{ format!("当前状态为“{}”，暂时不能开始。", paper_runtime_status_label(paper.runtime_status)) }</div>
                                }
                                if let Some(message) = &*error {
                                    <p class="text-sm text-error" role="alert" data-testid="paper-start-error">{ message }</p>
                                }
                                <button class="btn btn-primary w-full" type="submit" disabled={*starting || paper.runtime_status != PaperRuntimeStatus::Open || resumed_attempt.is_some()} data-testid="start-exam">
                                    { if *starting { "开始中…" } else if resumed_attempt.is_some() { "请继续上次考试" } else { "开始正式考试" } }
                                </button>
                            </form>
                        </div>
                    </section>
                </div>
            }
        }
    };

    html! {
        <AppShell
            user={props.user.clone()}
            eyebrow="XIAOLUOQUIZ / EXAM"
            title="考试准备"
            subtitle="确认考试信息后再开始计时。"
            test_id="paper-start-page"
            active={NavigationItem::Papers}
        >
            { body }
        </AppShell>
    }
}

#[derive(Properties, PartialEq)]
struct ExamPageProps {
    user: UserIdentity,
    attempt_id: i64,
}

#[function_component(ExamPage)]
fn exam_page(props: &ExamPageProps) -> Html {
    let load_state = use_state(|| AttemptLoadState::Loading);
    let show_submit_dialog = use_state(|| false);
    let submitting = use_state(|| false);
    let submit_error = use_state(|| None::<String>);
    let active_index = use_state(|| 0_usize);
    let question_navigation_expanded = use_state(|| false);
    let numbers_per_row = use_state(|| 8_usize);
    let answer_storage_key = exam_answer_storage_key(props.user.id, props.attempt_id);
    let initial_answers = read_answer_map(&answer_storage_key);
    let answers = use_state(move || initial_answers);

    {
        let load_state = load_state.clone();
        let answers = answers.clone();
        let attempt_id = props.attempt_id;
        use_effect_with(attempt_id, move |_| {
            spawn_local(async move {
                match get_json::<ExamAttempt>(
                    &format!("{ATTEMPTS_ENDPOINT}/{attempt_id}"),
                    "考试答题记录加载失败",
                )
                .await
                {
                    Ok(attempt) => {
                        let mut next_answers = (*answers).clone();
                        for question in &attempt.questions {
                            if let std::collections::btree_map::Entry::Vacant(entry) =
                                next_answers.entry(question.question_id)
                            {
                                if let Some(answer) = &question.saved_answer {
                                    if answer_should_persist(answer) {
                                        entry.insert(answer.clone());
                                    }
                                }
                            }
                        }
                        answers.set(next_answers);
                        load_state.set(AttemptLoadState::Ready(attempt));
                    }
                    Err(error) => load_state.set(AttemptLoadState::Error(error)),
                }
            });
            || ()
        });
    }

    {
        let answer_storage_key = answer_storage_key.clone();
        let answers_snapshot = (*answers).clone();
        use_effect_with(answers_snapshot, move |answers| {
            write_answer_map(&answer_storage_key, answers);
            || ()
        });
    }

    let attempt_timing = match &*load_state {
        AttemptLoadState::Ready(attempt) if attempt.status == AttemptStatus::InProgress => {
            (attempt.deadline_at.clone(), attempt.auto_submit)
        }
        _ => (None, false),
    };
    let remaining_seconds = use_state(|| None::<i64>);
    {
        let remaining_seconds = remaining_seconds.clone();
        let submit_error = submit_error.clone();
        let show_submit_dialog = show_submit_dialog.clone();
        let load_state = load_state.clone();
        let answers = answers.clone();
        let attempt_id = props.attempt_id;
        use_effect_with(attempt_timing, move |(deadline, should_auto_submit)| {
            let deadline_ms = deadline
                .as_deref()
                .map(js_sys::Date::parse)
                .filter(|timestamp| timestamp.is_finite());
            let attempt_questions = match &*load_state {
                AttemptLoadState::Ready(attempt) => attempt.questions.clone(),
                _ => Vec::new(),
            };
            let auto_submit_started = Rc::new(Cell::new(false));
            let cleanup: Box<dyn FnOnce()> = if let Some(deadline_ms) = deadline_ms {
                let auto_submit_started = auto_submit_started.clone();
                let submit_error = submit_error.clone();
                let show_submit_dialog = show_submit_dialog.clone();
                let should_auto_submit = *should_auto_submit;
                let update_remaining = move || {
                    let seconds = ((deadline_ms - js_sys::Date::now()) / 1_000.0).ceil();
                    remaining_seconds.set(Some(seconds.max(0.0) as i64));
                    if seconds <= 0.0 && should_auto_submit && !auto_submit_started.get() {
                        auto_submit_started.set(true);
                        let answers = answers.clone();
                        let questions = attempt_questions.clone();
                        let submit_error = submit_error.clone();
                        let show_submit_dialog = show_submit_dialog.clone();
                        spawn_local(async move {
                            if let Err(message) =
                                save_exam_answers(attempt_id, &questions, &answers).await
                            {
                                submit_error.set(Some(message));
                                show_submit_dialog.set(true);
                                return;
                            }
                            match post_empty::<ExamResult>(
                                &format!("{ATTEMPTS_ENDPOINT}/{attempt_id}/submit"),
                                "自动交卷失败",
                            )
                            .await
                            {
                                Ok(_) => navigate_to(&format!("/exam/{attempt_id}/result")),
                                Err(message) => {
                                    submit_error.set(Some(message));
                                    show_submit_dialog.set(true);
                                }
                            }
                        });
                    }
                };
                update_remaining();
                let callback = Closure::wrap(Box::new(update_remaining) as Box<dyn FnMut()>);
                let window = web_sys::window();
                let interval_id = window.as_ref().and_then(|window| {
                    window
                        .set_interval_with_callback_and_timeout_and_arguments_0(
                            callback.as_ref().unchecked_ref(),
                            1_000,
                        )
                        .ok()
                });
                Box::new(move || {
                    if let (Some(window), Some(interval_id)) = (window, interval_id) {
                        window.clear_interval_with_handle(interval_id);
                    }
                    drop(callback);
                })
            } else {
                remaining_seconds.set(None);
                Box::new(|| {})
            };
            cleanup
        });
    }

    let remaining_label = remaining_seconds.map_or_else(
        || "剩余时间：计算中".to_owned(),
        |seconds| {
            if seconds == 0 {
                "剩余时间：已到截止时间".to_owned()
            } else {
                format!("剩余时间：{}", remaining_duration_label(seconds))
            }
        },
    );

    let question_ids = match &*load_state {
        AttemptLoadState::Ready(attempt) => attempt
            .questions
            .iter()
            .map(|question| question.question_id)
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    {
        let active_index = active_index.clone();
        use_effect_with(question_ids, move |question_ids| {
            active_index.set(
                question_ids
                    .len()
                    .checked_sub(1)
                    .map_or(0, |last_index| (*active_index).min(last_index)),
            );
            || ()
        });
    }
    let question_count = match &*load_state {
        AttemptLoadState::Ready(attempt) => attempt.questions.len(),
        _ => 0,
    };
    let current_index = if question_count == 0 {
        0
    } else {
        (*active_index).min(question_count - 1)
    };
    let go_to_previous = {
        let active_index = active_index.clone();
        Callback::from(move |_| active_index.set((*active_index).saturating_sub(1)))
    };
    let go_to_next = {
        let active_index = active_index.clone();
        Callback::from(move |_| {
            if question_count > 0 {
                active_index.set((*active_index + 1).min(question_count - 1));
            }
        })
    };
    let on_numbers_per_row = {
        let numbers_per_row = numbers_per_row.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            if let Ok(value) = select.value().parse::<usize>() {
                numbers_per_row.set(value.clamp(4, 12));
            }
        })
    };
    let on_question_navigation_toggle = {
        let question_navigation_expanded = question_navigation_expanded.clone();
        Callback::from(move |_| question_navigation_expanded.set(!*question_navigation_expanded))
    };

    let on_attempt_changed = {
        let load_state = load_state.clone();
        let answers = answers.clone();
        Callback::from(move |attempt: ExamAttempt| {
            let mut next_answers = (*answers).clone();
            for question in &attempt.questions {
                if let std::collections::btree_map::Entry::Vacant(entry) =
                    next_answers.entry(question.question_id)
                {
                    if let Some(answer) = &question.saved_answer {
                        if answer_should_persist(answer) {
                            entry.insert(answer.clone());
                        }
                    }
                }
            }
            answers.set(next_answers);
            load_state.set(AttemptLoadState::Ready(attempt));
        })
    };
    let on_open_submit = {
        let show_submit_dialog = show_submit_dialog.clone();
        let submit_error = submit_error.clone();
        Callback::from(move |_| {
            submit_error.set(None);
            show_submit_dialog.set(true);
        })
    };
    let on_cancel_submit = {
        let show_submit_dialog = show_submit_dialog.clone();
        Callback::from(move |_| show_submit_dialog.set(false))
    };
    let on_confirm_submit = {
        let show_submit_dialog = show_submit_dialog.clone();
        let submitting = submitting.clone();
        let submit_error = submit_error.clone();
        let load_state = load_state.clone();
        let answers = answers.clone();
        let attempt_id = props.attempt_id;
        Callback::from(move |_| {
            if *submitting {
                return;
            }
            let questions = match &*load_state {
                AttemptLoadState::Ready(attempt) => attempt.questions.clone(),
                _ => return,
            };
            submit_error.set(None);
            submitting.set(true);
            let saved_answers = (*answers).clone();
            let show_submit_dialog = show_submit_dialog.clone();
            let submitting = submitting.clone();
            let submit_error = submit_error.clone();
            spawn_local(async move {
                if let Err(message) =
                    save_exam_answers(attempt_id, &questions, &saved_answers).await
                {
                    submit_error.set(Some(message));
                    submitting.set(false);
                    return;
                }

                match post_empty::<ExamResult>(
                    &format!("{ATTEMPTS_ENDPOINT}/{attempt_id}/submit"),
                    "交卷失败",
                )
                .await
                {
                    Ok(_) => {
                        show_submit_dialog.set(false);
                        navigate_to(&format!("/exam/{attempt_id}/result"));
                    }
                    Err(message) => submit_error.set(Some(message)),
                }
                submitting.set(false);
            });
        })
    };

    let body = match &*load_state {
        AttemptLoadState::Loading => html! {
            <div class="grid gap-4" data-testid="exam-loading-state">
                <div class="skeleton h-28 w-full rounded-box" />
                <div class="skeleton h-96 w-full rounded-box" />
            </div>
        },
        AttemptLoadState::Error(error) => html! {
            <div class="alert alert-error" data-testid="exam-error-state">{ format!("无法打开考试：{error}") }</div>
        },
        AttemptLoadState::Ready(attempt) => {
            let read_only = attempt.status != AttemptStatus::InProgress;
            let score_label = attempt.total_score.map_or_else(
                || "尚未评分".to_owned(),
                |score| format!("{score:.2} / {:.2}", attempt.max_score),
            );
            let answered_count = attempt
                .questions
                .iter()
                .filter(|question| {
                    answer_for_exam_question(&answers, question)
                        .is_some_and(|answer| answer_is_answered(&answer))
                })
                .count();
            let unanswered_count = attempt.questions.len().saturating_sub(answered_count);
            let question = attempt.questions[current_index].clone();
            let question_id = question.question_id;
            let current_answer = answer_for_exam_question(&answers, &question);
            let on_answer = {
                let answers = answers.clone();
                Callback::from(move |answer: AnswerPayload| {
                    let mut values = (*answers).clone();
                    values.insert(question_id, answer);
                    answers.set(values);
                })
            };
            html! {
                <div class="grid gap-6">
                    <section class="card border border-base-300 bg-base-100 shadow-sm">
                        <div class="card-body gap-4">
                            <div class="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
                                <div class="min-w-0">
                                    <div class="flex flex-wrap items-center gap-2">
                                        <span class={classes!("badge", attempt_status_class(attempt.status))}>{ attempt_status_label(attempt.status) }</span>
                                        <span class="badge badge-outline">{ format!("得分：{score_label}") }</span>
                                    </div>
                                    <h2 class="mt-3 break-words text-2xl font-bold">{ &attempt.title }</h2>
                                    <p class="mt-2 text-sm text-base-content/65">{ format!("考生：{}{}", candidate_info_label(&attempt.candidate_info), if unanswered_count == 0 { "" } else { " · 尚有未答题" }) }</p>
                                </div>
                                <div class="flex shrink-0 flex-wrap gap-2 md:justify-end">
                                    if read_only {
                                        <a class="btn btn-primary" href={format!("/exam/{}/result", attempt.id)} data-testid="exam-result-link">{"查看结果"}</a>
                                    } else {
                                        <button class="btn btn-primary" type="button" onclick={on_open_submit.clone()} data-testid="submit-exam">{"提交试卷"}</button>
                                    }
                                </div>
                            </div>
                            <div class="grid gap-2 rounded-box bg-base-200/60 p-4 text-sm text-base-content/70 sm:grid-cols-4">
                                <p>{ format!("已答 {} / {} 题", answered_count, attempt.questions.len()) }</p>
                                <p class="font-semibold text-primary" data-testid="exam-remaining-time">{ &remaining_label }</p>
                                <p data-testid="exam-start-time">{ format!("开始时间（上海时间）：{}", shanghai_datetime_label(&attempt.started_at)) }</p>
                                if let Some(deadline_at) = &attempt.deadline_at {
                                    <p data-testid="exam-deadline">{ format!("截止时间（上海时间）：{}", shanghai_datetime_label(deadline_at)) }</p>
                                } else {
                                    <p>{"服务端截止：不限时"}</p>
                                }
                            </div>
                            if read_only {
                                <div class="alert alert-info">{"这次考试已经结束，答题内容为只读。"}</div>
                            } else if unanswered_count > 0 {
                                <div class="alert alert-warning">{ format!("还有 {} 道题未回答，提交后未回答题目按 0 分或待批改处理。", unanswered_count) }</div>
                            }
                        </div>
                    </section>
                    <section class="card border border-base-300 bg-base-100 shadow-sm" data-testid="exam-question-numbers">
                        <div class="card-body gap-4 p-4 sm:p-5">
                            <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                                <div>
                                    <h2 class="font-bold">{"题目导航"}</h2>
                                    <p class="mt-1 text-xs text-base-content/60">{"点击展开题号导航；深色圆形按钮表示已经填写答案。"}</p>
                                </div>
                                <div class="flex flex-wrap items-center gap-2">
                                    <label class="form-control w-full sm:w-40">
                                        <span class="label-text text-xs font-semibold">{"每行题号数量"}</span>
                                        <select class="select select-bordered select-sm bg-base-100" value={numbers_per_row.to_string()} onchange={on_numbers_per_row.clone()} data-testid="exam-question-numbers-per-row">
                                            { for [4_usize, 6, 8, 10, 12].into_iter().map(|value| html! {
                                                <option value={value.to_string()} selected={*numbers_per_row == value}>{ value }</option>
                                            }) }
                                        </select>
                                    </label>
                                    <button
                                        class="btn btn-outline btn-sm"
                                        type="button"
                                        onclick={on_question_navigation_toggle.clone()}
                                        aria-expanded={(*question_navigation_expanded).to_string()}
                                        aria-controls="exam-question-navigation-panel"
                                        aria-label={if *question_navigation_expanded { "收起题目导航" } else { "展开题目导航" }}
                                        data-testid="exam-question-numbers-toggle"
                                    >
                                        { if *question_navigation_expanded { "收起" } else { "展开" } }
                                    </button>
                                </div>
                            </div>
                            if *question_navigation_expanded {
                                <div
                                    id="exam-question-navigation-panel"
                                    class="grid gap-2"
                                    style={format!("grid-template-columns: repeat({}, minmax(0, 1fr));", (*numbers_per_row).clamp(4, 12))}
                                    data-testid="exam-question-navigation-panel"
                                >
                                    { for attempt.questions.iter().enumerate().map(|(index, question)| {
                                        let answered = answer_for_exam_question(&answers, question)
                                            .is_some_and(|answer| answer_is_answered(&answer));
                                        let active = index == current_index;
                                        let active_index = active_index.clone();
                                        let class = classes!(
                                            "btn",
                                            "btn-circle",
                                            "btn-sm",
                                            if answered { "btn-neutral" } else { "btn-outline" },
                                            if active { "ring-2" } else { "" },
                                            if active { "ring-primary" } else { "" },
                                        );
                                        html! {
                                            <button
                                                class={class}
                                                type="button"
                                                aria-label={format!("第 {} 题", index + 1)}
                                                aria-current={active.then_some("step")}
                                                data-testid={format!("exam-question-number-{index}")}
                                                data-answered={answered.to_string()}
                                                onclick={Callback::from(move |_| active_index.set(index))}
                                            >
                                                { index + 1 }
                                            </button>
                                        }
                                    }) }
                                </div>
                            }
                        </div>
                    </section>
                    <ExamQuestionCard
                        key={question_id.to_string()}
                        attempt_id={attempt.id}
                        question={question}
                        answer={current_answer}
                        read_only={read_only}
                        on_answer={on_answer}
                        on_changed={on_attempt_changed.clone()}
                    />
                    <div class="flex flex-wrap items-center justify-between gap-3">
                        <button class="btn btn-outline" type="button" onclick={go_to_previous.clone()} disabled={current_index == 0} data-testid="exam-previous">{"上一题"}</button>
                        <span class="text-sm font-semibold text-base-content/65">{ format!("第 {} / {} 题", current_index + 1, attempt.questions.len()) }</span>
                        <button class="btn btn-primary" type="button" onclick={go_to_next.clone()} disabled={current_index + 1 >= attempt.questions.len()} data-testid="exam-next">{"下一题"}</button>
                    </div>
                </div>
            }
        }
    };

    let submit_unanswered_count = match &*load_state {
        AttemptLoadState::Ready(attempt) => Some(
            attempt
                .questions
                .iter()
                .filter(|question| {
                    answer_for_exam_question(&answers, question)
                        .is_none_or(|answer| !answer_is_answered(&answer))
                })
                .count(),
        ),
        _ => None,
    };

    html! {
        <AppShell
            user={props.user.clone()}
            eyebrow="XIAOLUOQUIZ / EXAM"
            title="正式考试答题"
            subtitle="答案会保存到当前浏览器；交卷前会再次同步到服务端，提交按钮会在服务端完成最终评分。"
            test_id="exam-page"
            active={NavigationItem::Papers}
        >
            { body }
            if *show_submit_dialog {
                <div class="modal modal-open" role="dialog" aria-modal="true" data-testid="submit-exam-dialog">
                    <div class="modal-box">
                        <h2 class="text-xl font-bold">{"确认提交试卷？"}</h2>
                        <p class="mt-3 text-sm text-base-content/70">{"提交后不能继续修改答案，服务端会立即进行自动评分；简答题会进入待批改状态。"}</p>
                        if let Some(unanswered_count) = submit_unanswered_count {
                            if unanswered_count > 0 {
                                <p class="mt-3 text-sm text-warning">{ format!("当前还有 {} 道题未回答。", unanswered_count) }</p>
                            }
                        }
                        if let Some(message) = &*submit_error {
                            <p class="mt-3 text-sm text-error" role="alert">{ message }</p>
                        }
                        <div class="modal-action">
                            <button class="btn btn-ghost" type="button" onclick={on_cancel_submit.clone()} disabled={*submitting} data-testid="cancel-submit-exam">{"再检查一下"}</button>
                            <button class="btn btn-primary" type="button" onclick={on_confirm_submit} disabled={*submitting} data-testid="confirm-submit-exam">
                                { if *submitting { "提交中…" } else { "确认交卷" } }
                            </button>
                        </div>
                    </div>
                </div>
            }
        </AppShell>
    }
}

#[derive(Properties, PartialEq)]
struct ExamQuestionCardProps {
    attempt_id: i64,
    question: ExamQuestion,
    answer: Option<AnswerPayload>,
    read_only: bool,
    on_answer: Callback<AnswerPayload>,
    on_changed: Callback<ExamAttempt>,
}

#[function_component(ExamQuestionCard)]
fn exam_question_card(props: &ExamQuestionCardProps) -> Html {
    let question = &props.question;
    let saving = use_state(|| false);
    let error = use_state(|| None::<String>);
    let save_status = use_state(|| None::<String>);
    let on_save = {
        let answer = props.answer.clone();
        let saving = saving.clone();
        let error = error.clone();
        let save_status = save_status.clone();
        let on_changed = props.on_changed.clone();
        let question_type = question.question_type;
        let question_id = question.question_id;
        let attempt_id = props.attempt_id;
        Callback::from(move |_| {
            if *saving {
                return;
            }
            error.set(None);
            save_status.set(None);
            let answer = match answer_for_submission(question_type, answer.as_ref()) {
                Ok(answer) => answer,
                Err(message) => {
                    error.set(Some(message));
                    return;
                }
            };
            saving.set(true);
            let saving = saving.clone();
            let error = error.clone();
            let save_status = save_status.clone();
            let on_changed = on_changed.clone();
            spawn_local(async move {
                let result: Result<ExamAttempt, String> = post_json(
                    &format!("{ATTEMPTS_ENDPOINT}/{attempt_id}/answers"),
                    &SaveAttemptAnswerRequest {
                        question_id,
                        answer,
                    },
                    "保存答案失败",
                )
                .await;
                match result {
                    Ok(attempt) => {
                        save_status.set(Some("已保存到服务端".to_owned()));
                        on_changed.emit(attempt);
                    }
                    Err(message) => error.set(Some(message)),
                }
                saving.set(false);
            });
        })
    };

    html! {
        <article class="card border border-base-300 bg-base-100 shadow-sm" data-testid="exam-question">
            <div class="card-body gap-5">
                <div class="flex items-start justify-between gap-4">
                    <div class="flex min-w-0 items-start gap-3">
                        <span class="badge badge-primary mt-1">{ question.position + 1 }</span>
                        <div class="min-w-0">
                            <div class="flex flex-wrap items-center gap-2">
                                <span class="badge badge-outline">{ question_type_label(question.question_type) }</span>
                                <span class="text-xs text-base-content/55">{ format!("{} 分", question.score) }</span>
                            </div>
                            <h2 class="mt-3 break-words text-xl font-bold leading-relaxed">{ &question.stem }</h2>
                        </div>
                    </div>
                    <span class={classes!("badge", grading_status_class(question.grading_status))}>{ grading_status_label(question.grading_status) }</span>
                </div>
                { render_exam_answer_control(
                    question,
                    props.answer.as_ref(),
                    props.on_answer.clone(),
                    props.read_only,
                ) }
                <div class="flex flex-wrap items-center gap-3">
                    <button class="btn btn-primary btn-sm" type="button" onclick={on_save} disabled={props.read_only || *saving} data-testid="save-exam-answer">
                        { if *saving { "保存中…" } else { "保存这道题" } }
                    </button>
                    if let Some(status) = &*save_status {
                        <span class="text-sm text-success" data-testid="answer-save-status">{ status }</span>
                    }
                    if let Some(message) = &*error {
                        <span class="text-sm text-error" role="alert">{ message }</span>
                    }
                </div>
            </div>
        </article>
    }
}

fn render_exam_answer_control(
    question: &ExamQuestion,
    answer: Option<&AnswerPayload>,
    on_answer: Callback<AnswerPayload>,
    read_only: bool,
) -> Html {
    match question.question_type {
        QuestionType::SingleChoice => html! {
            <div class="grid gap-3" role="radiogroup" aria-label="考试选择题选项">
                { for question.options.iter().map(|option| {
                    let on_answer = on_answer.clone();
                    let key = option.key.clone();
                    let key_for_change = key.clone();
                    let input_id = format!("exam-question-{}-{}", question.question_id, option.key);
                    html! {
                        <label class="flex min-w-0 cursor-pointer items-center gap-3 rounded-box border border-base-300 bg-base-200/50 p-4 transition-colors has-[:checked]:border-primary has-[:checked]:bg-primary/10" for={input_id.clone()}>
                            <input
                                id={input_id}
                                class="radio radio-primary"
                                type="radio"
                                name={format!("exam-question-{}", question.question_id)}
                                value={key.clone()}
                                checked={saved_single_value(answer).as_deref() == Some(key.as_str())}
                                disabled={read_only}
                                onchange={Callback::from(move |_| on_answer.emit(AnswerPayload::SingleChoice { option_key: key_for_change.clone() }))}
                            />
                            <span class="min-w-0 break-words font-medium">{ format!("{}．{}", option.key, option.text) }</span>
                        </label>
                    }
                }) }
            </div>
        },
        QuestionType::MultipleChoice => {
            let selected = saved_multiple_values(answer);
            html! {
                <div class="grid gap-3" role="group" aria-label="考试多选题选项">
                    { for question.options.iter().map(|option| {
                        let on_answer = on_answer.clone();
                        let key = option.key.clone();
                        let key_for_change = key.clone();
                        let input_id = format!("exam-question-{}-{}", question.question_id, option.key);
                        let checked = selected.iter().any(|value| value == &key);
                        let selected = selected.clone();
                        html! {
                            <label class="flex min-w-0 cursor-pointer items-center gap-3 rounded-box border border-base-300 bg-base-200/50 p-4 transition-colors has-[:checked]:border-primary has-[:checked]:bg-primary/10" for={input_id.clone()}>
                                <input
                                    id={input_id}
                                    class="checkbox checkbox-primary"
                                    type="checkbox"
                                    name={format!("exam-question-{}", question.question_id)}
                                    value={key.clone()}
                                    checked={checked}
                                    disabled={read_only}
                                    onchange={Callback::from(move |event: Event| {
                                        let input: HtmlInputElement = event.target_unchecked_into();
                                        let mut values = selected.clone();
                                        if input.checked() {
                                            if !values.iter().any(|value| value == &key_for_change) {
                                                values.push(key_for_change.clone());
                                            }
                                        } else {
                                            values.retain(|value| value != &key_for_change);
                                        }
                                        on_answer.emit(AnswerPayload::MultipleChoice { option_keys: values });
                                    })}
                                />
                                <span class="min-w-0 break-words font-medium">{ format!("{}．{}", option.key, option.text) }</span>
                            </label>
                        }
                    }) }
                </div>
            }
        }
        QuestionType::TrueFalse => html! {
            <div class="grid gap-3 sm:grid-cols-2" role="radiogroup" aria-label="考试判断题选项">
                { for [("true", "正确"), ("false", "错误")].into_iter().map(|(value, label)| {
                    let on_answer = on_answer.clone();
                    let value_for_change = value.to_owned();
                    let input_id = format!("exam-question-{}-{}", question.question_id, value);
                    html! {
                        <label class="flex min-w-0 cursor-pointer items-center gap-3 rounded-box border border-base-300 bg-base-200/50 p-4 transition-colors has-[:checked]:border-primary has-[:checked]:bg-primary/10" for={input_id.clone()}>
                            <input
                                id={input_id}
                                class="radio radio-primary"
                                type="radio"
                                name={format!("exam-question-{}", question.question_id)}
                                value={value}
                                checked={saved_single_value(answer).as_deref() == Some(value)}
                                disabled={read_only}
                                onchange={Callback::from(move |_| on_answer.emit(AnswerPayload::TrueFalse { value: value_for_change == "true" }))}
                            />
                            <span class="font-medium">{ label }</span>
                        </label>
                    }
                }) }
            </div>
        },
        QuestionType::FillBlank => {
            let values = saved_blank_values(answer, question.blank_count.max(1) as usize);
            html! {
                <div class="grid gap-3">
                    { for values.iter().enumerate().map(|(index, value)| {
                        let on_answer = on_answer.clone();
                        let values = values.clone();
                        html! {
                            <label class="form-control" for={format!("exam-question-{}-blank-{}", question.question_id, index)}>
                                <span class="label-text text-sm font-semibold">{ format!("第 {} 空", index + 1) }</span>
                                <input
                                    id={format!("exam-question-{}-blank-{}", question.question_id, index)}
                                    class="input input-bordered bg-base-100"
                                    value={value.clone()}
                                    disabled={read_only}
                                    oninput={Callback::from(move |event: InputEvent| {
                                        let input: HtmlInputElement = event.target_unchecked_into();
                                        let mut next_values = values.clone();
                                        next_values[index] = input.value();
                                        on_answer.emit(AnswerPayload::FillBlank { values: next_values });
                                    })}
                                />
                            </label>
                        }
                    }) }
                </div>
            }
        }
        QuestionType::ShortAnswer => {
            let on_answer = on_answer.clone();
            let value = saved_short_value(answer);
            html! {
                <label class="form-control" for={format!("exam-question-{}-short-answer", question.question_id)}>
                    <span class="label-text text-sm font-semibold">{"你的答案"}</span>
                    <textarea
                        id={format!("exam-question-{}-short-answer", question.question_id)}
                        class="textarea textarea-bordered min-h-32 bg-base-100"
                        value={value}
                        disabled={read_only}
                        oninput={Callback::from(move |event: InputEvent| {
                            let input: HtmlTextAreaElement = event.target_unchecked_into();
                            on_answer.emit(AnswerPayload::ShortAnswer { text: input.value() });
                        })}
                    />
                </label>
            }
        }
    }
}

fn answer_for_exam_question(answers: &AnswerMap, question: &ExamQuestion) -> Option<AnswerPayload> {
    answers
        .get(&question.question_id)
        .cloned()
        .or_else(|| question.saved_answer.clone())
}

async fn save_exam_answers(
    attempt_id: i64,
    questions: &[ExamQuestion],
    answers: &AnswerMap,
) -> Result<(), String> {
    for question in questions {
        let answer = answers
            .get(&question.question_id)
            .cloned()
            .or_else(|| question.saved_answer.clone());
        let Some(answer) = answer else {
            continue;
        };
        let Ok(answer) = answer_for_submission(question.question_type, Some(&answer)) else {
            continue;
        };
        let result: Result<ExamAttempt, String> = post_json(
            &format!("{ATTEMPTS_ENDPOINT}/{attempt_id}/answers"),
            &SaveAttemptAnswerRequest {
                question_id: question.question_id,
                answer,
            },
            "交卷前保存答案失败",
        )
        .await;
        result.map(|_| ())?;
    }
    Ok(())
}

fn saved_single_value(answer: Option<&AnswerPayload>) -> Option<String> {
    match answer {
        Some(AnswerPayload::SingleChoice { option_key }) => Some(option_key.clone()),
        Some(AnswerPayload::TrueFalse { value }) => Some(value.to_string()),
        _ => None,
    }
}

fn saved_multiple_values(answer: Option<&AnswerPayload>) -> Vec<String> {
    match answer {
        Some(AnswerPayload::MultipleChoice { option_keys }) => option_keys.clone(),
        _ => Vec::new(),
    }
}

fn saved_blank_values(answer: Option<&AnswerPayload>, blank_count: usize) -> Vec<String> {
    let mut values = match answer {
        Some(AnswerPayload::FillBlank { values }) => values.clone(),
        _ => Vec::new(),
    };
    values.resize(blank_count, String::new());
    values
}

fn saved_short_value(answer: Option<&AnswerPayload>) -> String {
    match answer {
        Some(AnswerPayload::ShortAnswer { text }) => text.clone(),
        _ => String::new(),
    }
}

#[derive(Properties, PartialEq)]
struct ExamResultPageProps {
    user: UserIdentity,
    attempt_id: i64,
}

#[function_component(ExamResultPage)]
fn exam_result_page(props: &ExamResultPageProps) -> Html {
    let load_state = use_state(|| ResultLoadState::Loading);

    {
        let load_state = load_state.clone();
        let attempt_id = props.attempt_id;
        use_effect_with(attempt_id, move |_| {
            spawn_local(async move {
                match get_json::<ExamResult>(
                    &format!("{ATTEMPTS_ENDPOINT}/{attempt_id}/result"),
                    "考试结果加载失败",
                )
                .await
                {
                    Ok(result) => load_state.set(ResultLoadState::Ready(result)),
                    Err(error) => load_state.set(ResultLoadState::Error(error)),
                }
            });
            || ()
        });
    }

    let body = match &*load_state {
        ResultLoadState::Loading => html! {
            <div class="grid gap-4" data-testid="exam-result-loading-state">
                <div class="skeleton h-36 w-full rounded-box" />
                <div class="skeleton h-52 w-full rounded-box" />
            </div>
        },
        ResultLoadState::Error(error) => html! {
            <div class="alert alert-error" data-testid="exam-result-error">{ format!("无法查看考试结果：{error}") }</div>
        },
        ResultLoadState::Ready(result) => html! {
            <div class="grid gap-6">
                <section class="card border border-base-300 bg-base-100 shadow-sm">
                    <div class="card-body gap-4">
                        <div class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
                            <div>
                                <div class="flex flex-wrap items-center gap-2">
                                    <span class={classes!("badge", attempt_status_class(result.status))}>{ attempt_status_label(result.status) }</span>
                                    <span class="badge badge-outline">{ format!("{} 道题", result.items.len()) }</span>
                                </div>
                                <h2 class="mt-3 text-2xl font-bold">{ &result.title }</h2>
                                if let Some(submitted_at) = &result.submitted_at {
                                    <p class="mt-2 text-sm text-base-content/65" data-testid="exam-submitted-time">{ format!("交卷时间（上海时间）：{}", shanghai_datetime_label(submitted_at)) }</p>
                                }
                            </div>
                            <div class="rounded-box bg-primary/10 px-5 py-4 text-center">
                                <p class="text-xs font-bold text-primary">{"总分"}</p>
                                <p class="mt-1 text-3xl font-black text-primary" data-testid="exam-total-score">
                                    { result.total_score.map_or_else(|| "待批改".to_owned(), |score| format!("{score:.2} / {:.2}", result.max_score)) }
                                </p>
                            </div>
                        </div>
                        if result.status == AttemptStatus::NeedsReview {
                            <div class="alert alert-warning">{"部分简答题需要人工批改，当前总分可能还不是最终成绩。"}</div>
                        }
                    </div>
                </section>
                <section class="grid gap-4" data-testid="exam-result-list">
                    { for result.items.iter().map(|item| html! { <ExamResultItemView item={item.clone()} /> }) }
                </section>
            </div>
        },
    };

    html! {
        <AppShell
            user={props.user.clone()}
            eyebrow="XIAOLUOQUIZ / RESULT"
            title="考试结果"
            subtitle="这里只展示当前账号自己的答题记录和服务端评分结果。"
            test_id="exam-result-page"
            active={NavigationItem::Papers}
        >
            { body }
        </AppShell>
    }
}

#[derive(Properties, PartialEq)]
struct ExamResultItemViewProps {
    item: xiaoluoquiz::domain::ExamResultItem,
}

#[function_component(ExamResultItemView)]
fn exam_result_item_view(props: &ExamResultItemViewProps) -> Html {
    let item = &props.item;
    let status_label = item
        .status
        .map(evaluation_status_label)
        .unwrap_or_else(|| grading_status_label(item.grading_status));
    let status_class = item
        .status
        .map(evaluation_status_class)
        .unwrap_or_else(|| grading_status_class(item.grading_status));
    html! {
        <article class="card border border-base-300 bg-base-100 shadow-sm" data-testid="exam-result-item">
            <div class="card-body gap-4">
                <div class="flex items-start gap-3">
                    <span class="badge badge-primary mt-1">{ item.position + 1 }</span>
                    <div class="min-w-0 flex-1">
                        <div class="flex flex-wrap items-center gap-2">
                            <span class="badge badge-outline">{ question_type_label(item.question_type) }</span>
                            <span class={classes!("badge", status_class)}>{ status_label }</span>
                            <span class="text-xs text-base-content/55">{ format!("{:.2} 分", item.max_score) }</span>
                        </div>
                        <h2 class="mt-3 break-words text-lg font-bold leading-relaxed">{ &item.stem }</h2>
                    </div>
                </div>
                <div class="grid gap-2 rounded-box bg-base-200/60 p-4 text-sm">
                    <p>
                        <strong>{"你的答案："}</strong>
                        if let Some(answer) = &item.answer {
                            <AnswerView answer={answer.clone()} />
                        } else {
                            <span class="text-base-content/60">{"未作答"}</span>
                        }
                    </p>
                    <p>
                        <strong>{"得分："}</strong>
                        { item.awarded_score.map_or_else(|| "待批改".to_owned(), |score| format!("{score:.2} / {:.2}", item.max_score)) }
                    </p>
                    if let Some(correct_answer) = &item.correct_answer {
                        <p><strong>{"参考答案："}</strong><CorrectAnswerView answer={correct_answer.clone()} /></p>
                    }
                    if let Some(explanation) = &item.explanation {
                        <p><strong>{"解析："}</strong>{ explanation }</p>
                    }
                    if let Some(feedback) = &item.feedback {
                        <p><strong>{"批改意见："}</strong>{ feedback }</p>
                    }
                </div>
            </div>
        </article>
    }
}

#[derive(Properties, PartialEq)]
struct AnswerViewProps {
    answer: AnswerPayload,
}

#[function_component(AnswerView)]
fn answer_view(props: &AnswerViewProps) -> Html {
    match &props.answer {
        AnswerPayload::SingleChoice { option_key } => html! { <span>{ option_key }</span> },
        AnswerPayload::MultipleChoice { option_keys } => {
            html! { <span>{ option_keys.join("、") }</span> }
        }
        AnswerPayload::FillBlank { values } => html! { <span>{ values.join(" / ") }</span> },
        AnswerPayload::TrueFalse { value } => {
            html! { <span>{ if *value { "正确" } else { "错误" } }</span> }
        }
        AnswerPayload::ShortAnswer { text } => {
            html! { <span class="whitespace-pre-wrap">{ text }</span> }
        }
    }
}

fn candidate_info_label(info: &CandidateInfo) -> String {
    match (&info.student_number, &info.name) {
        (Some(student_number), Some(name)) => format!("{} / {}", student_number, name),
        (Some(student_number), None) => student_number.clone(),
        (None, Some(name)) => name.clone(),
        (None, None) => "未填写".to_owned(),
    }
}

fn attempt_status_label(status: AttemptStatus) -> &'static str {
    match status {
        AttemptStatus::InProgress => "进行中",
        AttemptStatus::Submitted => "已提交",
        AttemptStatus::NeedsReview => "待批改",
        AttemptStatus::Graded => "已评分",
    }
}

fn attempt_status_class(status: AttemptStatus) -> &'static str {
    match status {
        AttemptStatus::InProgress => "badge-info",
        AttemptStatus::Submitted => "badge-warning",
        AttemptStatus::NeedsReview => "badge-warning",
        AttemptStatus::Graded => "badge-success",
    }
}

fn remaining_duration_label(seconds: i64) -> String {
    let hours = seconds / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let seconds = seconds % 60;
    if hours > 0 {
        format!("{hours} 小时 {minutes:02} 分 {seconds:02} 秒")
    } else {
        format!("{minutes:02} 分 {seconds:02} 秒")
    }
}

fn grading_status_label(status: GradingStatus) -> &'static str {
    match status {
        GradingStatus::Pending => "未评分",
        GradingStatus::NeedsReview => "待批改",
        GradingStatus::Graded => "已评分",
    }
}

fn grading_status_class(status: GradingStatus) -> &'static str {
    match status {
        GradingStatus::Pending => "badge-ghost",
        GradingStatus::NeedsReview => "badge-warning",
        GradingStatus::Graded => "badge-success",
    }
}

fn evaluation_status_label(status: EvaluationStatus) -> &'static str {
    match status {
        EvaluationStatus::Correct => "回答正确",
        EvaluationStatus::Incorrect => "回答错误",
        EvaluationStatus::NeedsReview => "待批改",
    }
}

fn evaluation_status_class(status: EvaluationStatus) -> &'static str {
    match status {
        EvaluationStatus::Correct => "badge-success",
        EvaluationStatus::Incorrect => "badge-error",
        EvaluationStatus::NeedsReview => "badge-warning",
    }
}

fn navigate_to(path: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.location().set_href(path);
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
