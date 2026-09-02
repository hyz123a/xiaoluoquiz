use axum::http::StatusCode;
use serde_json::{Value, json};

use super::helpers::{TestApp, spawn_app};

#[tokio::test]
async fn submitted_answer_is_evaluated_by_the_server() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;

    let response = app
        .post_json(
            "/api/v1/questions/1/check",
            &json!({"answer":{"type":"single_choice","option_key":"B"}}),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = app.json(response).await;
    assert_eq!(body["status"], "correct");
    assert_eq!(body["correct"], true);
    assert!(body.get("score").is_none());
    assert_eq!(body["correct_answer"]["option_key"], "B");
}

#[tokio::test]
async fn submitted_multiple_choice_answer_is_evaluated_by_the_server() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;

    let created = app
        .post_json(
            "/api/v1/admin/questions",
            &json!({
                "question_bank_id": 2,
                "question_type": "multiple_choice",
                "stem": "多选题评分测试？",
                "blank_count": 0,
                "options": [
                    {"key": "A", "text": "选项 A"},
                    {"key": "B", "text": "选项 B"},
                    {"key": "C", "text": "选项 C"},
                    {"key": "D", "text": "选项 D"},
                    {"key": "E", "text": "选项 E"}
                ],
                "explanation": "A、C、E 正确。",
                "correct_answer": {
                    "type": "multiple_choice",
                    "option_keys": ["A", "C", "E"]
                }
            }),
        )
        .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body: Value = app.json(created).await;
    let question_id = created_body["id"]
        .as_i64()
        .expect("multiple-choice question should have an id");

    let published = app
        .post_json(
            &format!("/api/v1/admin/questions/{question_id}/publish"),
            &json!({}),
        )
        .await;
    assert_eq!(published.status(), StatusCode::OK);

    let correct = app
        .post_json(
            &format!("/api/v1/questions/{question_id}/check"),
            &json!({
                "answer": {"type": "multiple_choice", "option_keys": ["E", "A", "C"]}
            }),
        )
        .await;
    assert_eq!(correct.status(), StatusCode::OK);
    let correct_body: Value = app.json(correct).await;
    assert_eq!(correct_body["status"], "correct");
    assert_eq!(correct_body["correct"], true);

    let incorrect = app
        .post_json(
            &format!("/api/v1/questions/{question_id}/check"),
            &json!({
                "answer": {"type": "multiple_choice", "option_keys": ["A", "C"]}
            }),
        )
        .await;
    assert_eq!(incorrect.status(), StatusCode::OK);
    let incorrect_body: Value = app.json(incorrect).await;
    assert_eq!(incorrect_body["status"], "incorrect");
    assert_eq!(incorrect_body["correct"], false);
}
