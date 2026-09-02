use axum::http::StatusCode;
use serde_json::Value;

use super::helpers::{TestApp, sample_question, spawn_app};

#[tokio::test]
async fn published_question_list_does_not_expose_answers() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;

    let response = app.get("/api/v1/questions").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = app.json(response).await;
    assert_eq!(body["items"][0]["stem"], sample_question().stem);
    assert!(body["items"][0].get("correct_answer").is_none());
}

#[tokio::test]
async fn published_question_can_be_fetched_by_id() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;

    let response = app.get("/api/v1/questions/1").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = app.json(response).await;
    assert_eq!(body["id"], 1);
    assert_eq!(body["options"][1]["text"], "Cargo");
}

#[tokio::test]
async fn user_can_list_question_banks() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;

    let response = app.get("/api/v1/question-banks").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = app.json(response).await;
    let items = body["items"].as_array().expect("items should be an array");
    assert!(items.iter().any(|item| item["name"] == "人工智能导论"));
    assert!(items.iter().any(|item| item["name"] == "测试题库"));
}

#[tokio::test]
async fn user_can_filter_published_questions_by_question_bank() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;

    let response = app.get("/api/v1/questions?bank_id=999999").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = app.json(response).await;
    assert_eq!(body["items"], serde_json::json!([]));
}
