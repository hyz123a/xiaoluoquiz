use axum::http::StatusCode;
use serde_json::{Value, json};

use super::helpers::{TestApp, spawn_app};

fn draft_payload() -> Value {
    json!({
        "question_bank_id": 2,
        "question_type": "single_choice",
        "stem": "管理员创建的题目？",
        "blank_count": 0,
        "options": [
            {"key": "A", "text": "选项 A"},
            {"key": "B", "text": "选项 B"}
        ],
        "explanation": "这是题目解析。",
        "correct_answer": {"type": "single_choice", "option_key": "B"}
    })
}

async fn create_draft(app: &TestApp) -> Value {
    app.login_as_admin().await;
    let response = app
        .post_json("/api/v1/admin/questions", &draft_payload())
        .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    app.json(response).await
}

#[tokio::test]
async fn admin_can_save_a_valid_question_as_a_draft() {
    let app: TestApp = spawn_app().await;

    let body = create_draft(&app).await;

    assert_eq!(body["status"], "draft");
    assert_eq!(body["question_type"], "single_choice");
    assert_eq!(body["stem"], "管理员创建的题目？");
}

#[tokio::test]
async fn admin_cannot_save_an_invalid_question() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;
    let mut payload = draft_payload();
    payload["stem"] = json!("   ");

    let response = app.post_json("/api/v1/admin/questions", &payload).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = app.json(response).await;
    assert_eq!(body["error"], "stem must not be empty");
}

#[tokio::test]
async fn admin_can_list_questions_with_their_status() {
    let app: TestApp = spawn_app().await;
    let _draft = create_draft(&app).await;

    let response = app.get("/api/v1/admin/questions").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = app.json(response).await;
    let items = body["items"].as_array().expect("items should be an array");
    assert!(items.iter().any(|item| item["status"] == "draft"));
    assert!(items.iter().any(|item| item["status"] == "published"));
}

#[tokio::test]
async fn admin_can_filter_questions_by_keyword_bank_type_and_status() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;

    let mut draft = draft_payload();
    draft["stem"] = json!("筛选关键词草稿题目？");
    let draft_response = app.post_json("/api/v1/admin/questions", &draft).await;
    assert_eq!(draft_response.status(), StatusCode::CREATED);

    let mut multiple_choice = draft_payload();
    multiple_choice["question_bank_id"] = json!(1);
    multiple_choice["question_type"] = json!("multiple_choice");
    multiple_choice["stem"] = json!("另一个题库的多选题？");
    multiple_choice["options"] = json!([
        {"key": "A", "text": "选项 A"},
        {"key": "B", "text": "选项 B"},
        {"key": "C", "text": "选项 C"}
    ]);
    multiple_choice["correct_answer"] = json!({
        "type": "multiple_choice",
        "option_keys": ["A", "B"]
    });
    let multiple_response = app
        .post_json("/api/v1/admin/questions", &multiple_choice)
        .await;
    assert_eq!(multiple_response.status(), StatusCode::CREATED);

    let keyword_response = app
        .get("/api/v1/admin/questions?keyword=%E7%AD%9B%E9%80%89")
        .await;
    assert_eq!(keyword_response.status(), StatusCode::OK);
    let keyword_body: Value = app.json(keyword_response).await;
    let keyword_items = keyword_body["items"]
        .as_array()
        .expect("keyword items should be an array");
    assert_eq!(keyword_items.len(), 1);
    assert_eq!(keyword_items[0]["stem"], "筛选关键词草稿题目？");

    let bank_response = app.get("/api/v1/admin/questions?bank_id=1").await;
    assert_eq!(bank_response.status(), StatusCode::OK);
    let bank_body: Value = app.json(bank_response).await;
    let bank_items = bank_body["items"]
        .as_array()
        .expect("bank items should be an array");
    assert!(!bank_items.is_empty());
    assert!(bank_items.iter().all(|item| item["question_bank_id"] == 1));

    let type_response = app
        .get("/api/v1/admin/questions?question_type=multiple_choice")
        .await;
    assert_eq!(type_response.status(), StatusCode::OK);
    let type_body: Value = app.json(type_response).await;
    let type_items = type_body["items"]
        .as_array()
        .expect("type items should be an array");
    assert_eq!(type_items.len(), 1);
    assert_eq!(type_items[0]["question_type"], "multiple_choice");

    let status_response = app.get("/api/v1/admin/questions?status=draft").await;
    assert_eq!(status_response.status(), StatusCode::OK);
    let status_body: Value = app.json(status_response).await;
    let status_items = status_body["items"]
        .as_array()
        .expect("status items should be an array");
    assert!(status_items.len() >= 2);
    assert!(status_items.iter().all(|item| item["status"] == "draft"));
}

#[tokio::test]
async fn admin_can_import_only_new_questions_and_skip_duplicates() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;

    let mut duplicate = draft_payload();
    duplicate["stem"] = json!("  RUST 的包管理工具是什么？  ");
    duplicate["explanation"] = json!("导入内容不能覆盖已有解析。");

    let mut new_question = draft_payload();
    new_question["stem"] = json!("批量导入的新题目？");

    let response = app
        .post_json(
            "/api/v1/admin/questions/import",
            &json!({"items": [duplicate, new_question]}),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = app.json(response).await;
    assert_eq!(body["inserted"], 1);
    assert_eq!(body["skipped"], 1);
    assert_eq!(body["errors"], 0);
    assert_eq!(body["items"][0]["status"], "skipped");
    assert_eq!(body["items"][0]["question_id"], 1);
    assert_eq!(body["items"][1]["status"], "inserted");

    let listed = app
        .get("/api/v1/admin/questions?keyword=%E6%89%B9%E9%87%8F%E5%AF%BC%E5%85%A5")
        .await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body: Value = app.json(listed).await;
    assert_eq!(listed_body["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(listed_body["items"][0]["status"], "published");

    let published = app.get("/api/v1/questions?bank_id=2").await;
    assert_eq!(published.status(), StatusCode::OK);
    let published_body: Value = app.json(published).await;
    assert!(
        published_body["items"]
            .as_array()
            .expect("published items should be an array")
            .iter()
            .any(|item| item["stem"] == "批量导入的新题目？")
    );

    let original = app.get("/api/v1/admin/questions?keyword=Rust").await;
    assert_eq!(original.status(), StatusCode::OK);
    let original_body: Value = app.json(original).await;
    assert_eq!(original_body["items"][0]["explanation"], "B 是正确答案");
}

#[tokio::test]
async fn invalid_bulk_import_is_atomic_and_reports_errors() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;

    let mut valid = draft_payload();
    valid["stem"] = json!("原子性测试中不应保存的题目？");
    let mut invalid = draft_payload();
    invalid["stem"] = json!("   ");

    let response = app
        .post_json(
            "/api/v1/admin/questions/import",
            &json!({"items": [valid, invalid]}),
        )
        .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = app.json(response).await;
    assert_eq!(body["inserted"], 0);
    assert_eq!(body["skipped"], 0);
    assert_eq!(body["errors"], 1);
    assert_eq!(body["items"][0]["index"], 1);
    assert_eq!(body["items"][0]["error"], "stem must not be empty");

    let listed = app
        .get("/api/v1/admin/questions?keyword=%E5%8E%9F%E5%AD%90%E6%80%A7%E6%B5%8B%E8%AF%95")
        .await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body: Value = app.json(listed).await;
    assert_eq!(listed_body["items"], json!([]));
}

#[tokio::test]
async fn admin_can_publish_a_saved_draft() {
    let app: TestApp = spawn_app().await;
    let draft = create_draft(&app).await;
    let question_id = draft["id"].as_i64().expect("draft should have an id");

    let response = app
        .post_json(
            &format!("/api/v1/admin/questions/{question_id}/publish"),
            &json!({}),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = app.json(response).await;
    assert_eq!(body["status"], "published");
}

#[tokio::test]
async fn admin_can_archive_a_published_question() {
    let app: TestApp = spawn_app().await;
    let draft = create_draft(&app).await;
    let question_id = draft["id"].as_i64().expect("draft should have an id");

    let publish = app
        .post_json(
            &format!("/api/v1/admin/questions/{question_id}/publish"),
            &json!({}),
        )
        .await;
    assert_eq!(publish.status(), StatusCode::OK);

    let response = app
        .post_json(
            &format!("/api/v1/admin/questions/{question_id}/archive"),
            &json!({}),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = app.json(response).await;
    assert_eq!(body["status"], "archived");
}

#[tokio::test]
async fn admin_can_create_a_multiple_choice_question_with_variable_options() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;

    let response = app
        .post_json(
            "/api/v1/admin/questions",
            &json!({
                "question_bank_id": 2,
                "question_type": "multiple_choice",
                "stem": "哪些是 Rust 工具？",
                "blank_count": 0,
                "options": [
                    {"key": "A", "text": "Cargo"},
                    {"key": "B", "text": "npm"},
                    {"key": "C", "text": "rustc"},
                    {"key": "D", "text": "pip"},
                    {"key": "E", "text": "Clippy"}
                ],
                "explanation": "Cargo、rustc 和 Clippy 是 Rust 工具。",
                "correct_answer": {
                    "type": "multiple_choice",
                    "option_keys": ["A", "C", "E"]
                }
            }),
        )
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let body: Value = app.json(response).await;
    assert_eq!(body["question_type"], "multiple_choice");
    assert_eq!(body["options"].as_array().map(Vec::len), Some(5));
    assert_eq!(
        body["correct_answer"]["option_keys"],
        json!(["A", "C", "E"])
    );
}

#[tokio::test]
async fn admin_can_create_a_question_bank_and_see_it_in_the_list() {
    let app = spawn_app().await;
    app.login_as_admin().await;

    let response = app
        .post_json(
            "/api/v1/admin/question-banks",
            &json!({
                "name": "新增题库",
                "description": "用于管理员录入题目"
            }),
        )
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);
    let created: Value = app.json(response).await;
    assert_eq!(created["name"], "新增题库");
    assert_eq!(created["description"], "用于管理员录入题目");

    let listed = app.get("/api/v1/question-banks").await;
    assert_eq!(listed.status(), StatusCode::OK);
    let body: Value = app.json(listed).await;
    assert!(
        body["items"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["name"] == "新增题库"))
    );
}
