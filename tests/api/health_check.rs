use axum::http::StatusCode;
use serde_json::Value;

use super::helpers::{TestApp, spawn_app};

#[tokio::test]
async fn health_check_returns_ok() {
    let app: TestApp = spawn_app().await;

    let response = app.get("/api/v1/health").await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = app.json(response).await;
    assert_eq!(body["status"], "ok");
}
