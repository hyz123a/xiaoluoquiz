use axum::http::StatusCode;
use serde_json::{Value, json};

use super::helpers::{TestApp, spawn_app};

fn paper_payload(question_id: i64) -> Value {
    json!({
        "title": "Rust 基础正式考试",
        "description": "完成全部题目后交卷。",
        "audience": "软件工程测试班",
        "mode": "exam",
        "duration_seconds": 3600,
        "max_attempts": 1,
        "allow_resume": true,
        "auto_save": true,
        "auto_submit": true,
        "candidate_fields": [{"key": "student_number", "required": true}],
        "result_visibility": "after_submit",
        "allow_preview": false,
        "questions": [{"question_id": question_id, "score": 2.0}]
    })
}

async fn create_paper_for_question(app: &TestApp, question_id: i64) -> Value {
    app.login_as_admin().await;
    let response = app
        .post_json("/api/v1/admin/papers", &paper_payload(question_id))
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    app.json(response).await
}

async fn create_paper(app: &TestApp) -> Value {
    create_paper_for_question(app, 1).await
}

async fn publish_paper(app: &TestApp, paper_id: i64) -> Value {
    let response = app
        .post_json(
            &format!("/api/v1/admin/papers/{paper_id}/publish"),
            &json!({}),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    app.json(response).await
}

async fn login_as_student(app: &TestApp) {
    let logout = app.post_json("/api/v1/auth/logout", &json!({})).await;
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);
    let login = app.login("student-001", "InitialPassword123!").await;
    assert_eq!(login.status(), StatusCode::OK);
    let changed = app
        .post_json(
            "/api/v1/auth/change-password",
            &json!({"new_password": "StudentPassword123!"}),
        )
        .await;
    assert_eq!(changed.status(), StatusCode::OK);
}

#[tokio::test]
async fn admin_can_assemble_publish_and_archive_a_paper() {
    let app = spawn_app().await;
    let draft = create_paper(&app).await;

    assert_eq!(draft["status"], "draft");
    assert_eq!(draft["title"], "Rust 基础正式考试");
    assert_eq!(draft["items"][0]["question_id"], 1);
    assert_eq!(draft["items"][0]["score"], 2.0);
    assert_eq!(draft["total_score"], 2.0);

    let paper_id = draft["id"].as_i64().expect("paper should have an id");
    let published = publish_paper(&app, paper_id).await;
    assert_eq!(published["status"], "published");

    let archive = app
        .post_json(
            &format!("/api/v1/admin/papers/{paper_id}/archive"),
            &json!({}),
        )
        .await;
    assert_eq!(archive.status(), StatusCode::OK);
    let archived: Value = app.json(archive).await;
    assert_eq!(archived["status"], "archived");
}

#[tokio::test]
async fn admin_cannot_assemble_a_paper_with_an_unpublished_question() {
    let app = spawn_app().await;
    app.login_as_admin().await;

    let response = app
        .post_json("/api/v1/admin/papers", &paper_payload(2))
        .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = app.json(response).await;
    assert_eq!(body["error"], "selected question is not published");
}

#[tokio::test]
async fn ordinary_user_cannot_manage_papers() {
    let app = spawn_app().await;
    let login = app.login("student-001", "InitialPassword123!").await;
    assert_eq!(login.status(), StatusCode::OK);
    let changed = app
        .post_json(
            "/api/v1/auth/change-password",
            &json!({"new_password": "StudentPassword123!"}),
        )
        .await;
    assert_eq!(changed.status(), StatusCode::OK);

    let response = app.get("/api/v1/admin/papers").await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn student_can_start_save_submit_and_read_their_exam_result() {
    let app = spawn_app().await;
    let draft = create_paper(&app).await;
    let paper_id = draft["id"].as_i64().expect("paper should have an id");
    publish_paper(&app, paper_id).await;
    login_as_student(&app).await;

    let papers = app.get("/api/v1/papers").await;
    assert_eq!(papers.status(), StatusCode::OK);
    let papers_body: Value = app.json(papers).await;
    assert_eq!(papers_body["items"][0]["id"], paper_id);
    assert_eq!(papers_body["items"][0]["total_score"], 2.0);

    let missing_candidate = app
        .post_json(&format!("/api/v1/papers/{paper_id}/attempts"), &json!({}))
        .await;
    assert_eq!(missing_candidate.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let missing_body: Value = app.json(missing_candidate).await;
    assert_eq!(
        missing_body["error"],
        "candidate field is required: student_number"
    );

    let started = app
        .post_json(
            &format!("/api/v1/papers/{paper_id}/attempts"),
            &json!({"student_number": "2026001"}),
        )
        .await;
    assert_eq!(started.status(), StatusCode::CREATED);
    let attempt: Value = app.json(started).await;
    let attempt_id = attempt["id"].as_i64().expect("attempt should have an id");
    assert_eq!(attempt["status"], "in_progress");
    assert_eq!(attempt["candidate_info"]["student_number"], "2026001");
    assert!(attempt["questions"][0].get("correct_answer").is_none());

    let saved = app
        .post_json(
            &format!("/api/v1/attempts/{attempt_id}/answers"),
            &json!({
                "question_id": 1,
                "answer": {"type": "single_choice", "option_key": "B"}
            }),
        )
        .await;
    assert_eq!(saved.status(), StatusCode::OK);

    let reloaded = app.get(&format!("/api/v1/attempts/{attempt_id}")).await;
    assert_eq!(reloaded.status(), StatusCode::OK);
    let reloaded_body: Value = app.json(reloaded).await;
    assert_eq!(
        reloaded_body["questions"][0]["saved_answer"]["option_key"],
        "B"
    );

    let submitted = app
        .post_json(&format!("/api/v1/attempts/{attempt_id}/submit"), &json!({}))
        .await;
    assert_eq!(submitted.status(), StatusCode::OK);
    let result: Value = app.json(submitted).await;
    assert_eq!(result["status"], "graded");
    assert_eq!(result["total_score"], 2.0);
    assert_eq!(result["items"][0]["awarded_score"], 2.0);
    assert_eq!(result["items"][0]["correct_answer"]["option_key"], "B");

    let repeated = app
        .post_json(&format!("/api/v1/attempts/{attempt_id}/submit"), &json!({}))
        .await;
    assert_eq!(repeated.status(), StatusCode::OK);
    let repeated_body: Value = app.json(repeated).await;
    assert_eq!(repeated_body["total_score"], 2.0);

    let result_get = app
        .get(&format!("/api/v1/attempts/{attempt_id}/result"))
        .await;
    assert_eq!(result_get.status(), StatusCode::OK);
}

#[tokio::test]
async fn student_cannot_start_the_same_single_attempt_twice() {
    let app = spawn_app().await;
    let draft = create_paper(&app).await;
    let paper_id = draft["id"].as_i64().expect("paper should have an id");
    publish_paper(&app, paper_id).await;
    login_as_student(&app).await;

    let first = app
        .post_json(
            &format!("/api/v1/papers/{paper_id}/attempts"),
            &json!({"student_number": "2026001"}),
        )
        .await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let first_body: Value = app.json(first).await;

    let resumed = app
        .post_json(
            &format!("/api/v1/papers/{paper_id}/attempts"),
            &json!({"student_number": "2026001"}),
        )
        .await;
    assert_eq!(resumed.status(), StatusCode::CREATED);
    let resumed_body: Value = app.json(resumed).await;
    assert_eq!(resumed_body["id"], first_body["id"]);
}

#[tokio::test]
async fn submitted_attempt_cannot_be_changed_or_started_again() {
    let app = spawn_app().await;
    let draft = create_paper(&app).await;
    let paper_id = draft["id"].as_i64().expect("paper should have an id");
    publish_paper(&app, paper_id).await;
    login_as_student(&app).await;

    let started = app
        .post_json(
            &format!("/api/v1/papers/{paper_id}/attempts"),
            &json!({"student_number": "2026001"}),
        )
        .await;
    let attempt: Value = app.json(started).await;
    let attempt_id = attempt["id"].as_i64().expect("attempt should have an id");

    let submitted = app
        .post_json(&format!("/api/v1/attempts/{attempt_id}/submit"), &json!({}))
        .await;
    assert_eq!(submitted.status(), StatusCode::OK);

    let save_after_submit = app
        .post_json(
            &format!("/api/v1/attempts/{attempt_id}/answers"),
            &json!({
                "question_id": 1,
                "answer": {"type": "single_choice", "option_key": "B"}
            }),
        )
        .await;
    assert_eq!(save_after_submit.status(), StatusCode::CONFLICT);

    let start_again = app
        .post_json(
            &format!("/api/v1/papers/{paper_id}/attempts"),
            &json!({"student_number": "2026001"}),
        )
        .await;
    assert_eq!(start_again.status(), StatusCode::CONFLICT);

    let logout = app.post_json("/api/v1/auth/logout", &json!({})).await;
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);
    app.login_as_admin().await;
    let other_user_read = app.get(&format!("/api/v1/attempts/{attempt_id}")).await;
    assert_eq!(other_user_read.status(), StatusCode::NOT_FOUND);
}

async fn create_and_publish_short_question(app: &TestApp) -> i64 {
    app.login_as_admin().await;
    let created = app
        .post_json(
            "/api/v1/admin/questions",
            &json!({
                "question_bank_id": 2,
                "question_type": "short_answer",
                "stem": "请说明 Rust 的所有权规则。",
                "blank_count": 0,
                "options": [],
                "explanation": "所有权保证内存安全。",
                "correct_answer": {
                    "type": "short_answer",
                    "reference": "所有权保证每个值只有一个所有者。",
                    "rubric": "说明唯一所有者和生命周期即可。"
                }
            }),
        )
        .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body: Value = app.json(created).await;
    let question_id = created_body["id"]
        .as_i64()
        .expect("question should have an id");
    let published = app
        .post_json(
            &format!("/api/v1/admin/questions/{question_id}/publish"),
            &json!({}),
        )
        .await;
    assert_eq!(published.status(), StatusCode::OK);
    question_id
}

#[tokio::test]
async fn short_answer_is_marked_as_needing_review_after_submission() {
    let app = spawn_app().await;
    let question_id = create_and_publish_short_question(&app).await;
    let draft = create_paper_for_question(&app, question_id).await;
    let paper_id = draft["id"].as_i64().expect("paper should have an id");
    publish_paper(&app, paper_id).await;
    login_as_student(&app).await;

    let started = app
        .post_json(
            &format!("/api/v1/papers/{paper_id}/attempts"),
            &json!({"student_number": "2026001"}),
        )
        .await;
    let attempt: Value = app.json(started).await;
    let attempt_id = attempt["id"].as_i64().expect("attempt should have an id");
    let saved = app
        .post_json(
            &format!("/api/v1/attempts/{attempt_id}/answers"),
            &json!({
                "question_id": question_id,
                "answer": {"type": "short_answer", "text": "所有权让值有唯一所有者。"}
            }),
        )
        .await;
    assert_eq!(saved.status(), StatusCode::OK);

    let submitted = app
        .post_json(&format!("/api/v1/attempts/{attempt_id}/submit"), &json!({}))
        .await;
    assert_eq!(submitted.status(), StatusCode::OK);
    let result: Value = app.json(submitted).await;
    assert_eq!(result["status"], "needs_review");
    assert!(result["total_score"].is_null());
    assert_eq!(result["items"][0]["grading_status"], "needs_review");
    assert_eq!(result["items"][0]["status"], "needs_review");
    assert!(result["items"][0]["awarded_score"].is_null());
}

#[tokio::test]
async fn admin_can_list_view_and_grade_a_submitted_attempt() {
    let app = spawn_app().await;
    let question_id = create_and_publish_short_question(&app).await;
    let draft = create_paper_for_question(&app, question_id).await;
    let paper_id = draft["id"].as_i64().expect("paper should have an id");
    publish_paper(&app, paper_id).await;
    login_as_student(&app).await;

    let started = app
        .post_json(
            &format!("/api/v1/papers/{paper_id}/attempts"),
            &json!({"student_number": "2026001"}),
        )
        .await;
    let attempt: Value = app.json(started).await;
    let attempt_id = attempt["id"].as_i64().expect("attempt should have an id");
    let saved = app
        .post_json(
            &format!("/api/v1/attempts/{attempt_id}/answers"),
            &json!({
                "question_id": question_id,
                "answer": {"type": "short_answer", "text": "所有权让值有唯一所有者。"}
            }),
        )
        .await;
    assert_eq!(saved.status(), StatusCode::OK);
    let submitted = app
        .post_json(&format!("/api/v1/attempts/{attempt_id}/submit"), &json!({}))
        .await;
    assert_eq!(submitted.status(), StatusCode::OK);

    let logout = app.post_json("/api/v1/auth/logout", &json!({})).await;
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);
    app.login_as_admin().await;

    let listed = app.get("/api/v1/admin/attempts").await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body: Value = app.json(listed).await;
    let listed_attempt = listed_body["items"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["id"] == attempt_id))
        .expect("submitted attempt should be visible to administrators");
    assert_eq!(listed_attempt["status"], "needs_review");
    assert_eq!(
        listed_attempt["candidate_info"]["student_number"],
        "2026001"
    );

    let detail = app
        .get(&format!("/api/v1/admin/attempts/{attempt_id}"))
        .await;
    assert_eq!(detail.status(), StatusCode::OK);
    let detail_body: Value = app.json(detail).await;
    assert_eq!(detail_body["questions"][0]["question_id"], question_id);
    assert_eq!(
        detail_body["questions"][0]["answer"]["text"],
        "所有权让值有唯一所有者。"
    );

    let graded = app
        .post_json(
            &format!("/api/v1/admin/attempts/{attempt_id}/grade"),
            &json!({
                "question_id": question_id,
                "score": 1.5,
                "feedback": "说明了唯一所有者，表述清楚。"
            }),
        )
        .await;
    assert_eq!(graded.status(), StatusCode::OK);
    let graded_body: Value = app.json(graded).await;
    assert_eq!(graded_body["status"], "graded");
    assert_eq!(graded_body["total_score"], 1.5);
    assert_eq!(graded_body["questions"][0]["awarded_score"], 1.5);
    assert_eq!(
        graded_body["questions"][0]["feedback"],
        "说明了唯一所有者，表述清楚。"
    );
}

#[tokio::test]
async fn ordinary_user_cannot_read_admin_attempts() {
    let app = spawn_app().await;
    let login = app.login("student-001", "InitialPassword123!").await;
    assert_eq!(login.status(), StatusCode::OK);
    let changed = app
        .post_json(
            "/api/v1/auth/change-password",
            &json!({"new_password": "StudentPassword123!"}),
        )
        .await;
    assert_eq!(changed.status(), StatusCode::OK);

    let response = app.get("/api/v1/admin/attempts").await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
