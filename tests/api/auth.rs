use axum::http::StatusCode;
use serde_json::{Value, json};

use super::helpers::{TestApp, spawn_app};

#[tokio::test]
async fn user_can_log_in_with_the_fixed_initial_password() {
    let app: TestApp = spawn_app().await;

    let response = app
        .post_json(
            "/api/v1/auth/login",
            &json!({
                "username": "student-001",
                "password": "InitialPassword123!"
            }),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = app.json(response).await;
    assert_eq!(body["user"]["username"], "student-001");
    assert_eq!(body["user"]["must_change_password"], true);
}

#[tokio::test]
async fn login_configuration_exposes_student_initial_password() {
    let app: TestApp = spawn_app().await;

    let response = app.get("/api/v1/auth/config").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = app.json(response).await;
    assert_eq!(body["initial_password"], "InitialPassword123!");
}

#[tokio::test]
async fn first_login_must_change_password_before_practice() {
    let app: TestApp = spawn_app().await;
    let login = app.login("student-001", "InitialPassword123!").await;
    assert_eq!(login.status(), StatusCode::OK);

    let blocked = app.get("/api/v1/questions").await;
    assert_eq!(blocked.status(), StatusCode::FORBIDDEN);
    let blocked_body: Value = app.json(blocked).await;
    assert_eq!(blocked_body["code"], "password_change_required");

    let changed = app
        .post_json(
            "/api/v1/auth/change-password",
            &json!({ "new_password": "StudentPassword123!" }),
        )
        .await;
    assert_eq!(changed.status(), StatusCode::OK);

    let questions = app.get("/api/v1/questions").await;
    assert_eq!(questions.status(), StatusCode::OK);
}

#[tokio::test]
async fn invalid_credentials_do_not_create_a_session() {
    let app: TestApp = spawn_app().await;

    let response = app.login("student-001", "wrong-password").await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let protected = app.get("/api/v1/questions").await;
    assert_eq!(protected.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn logout_revokes_the_server_session() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;
    assert_eq!(app.get("/api/v1/questions").await.status(), StatusCode::OK);

    let logout = app.post_json("/api/v1/auth/logout", &json!({})).await;
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);

    let protected = app.get("/api/v1/questions").await;
    assert_eq!(protected.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn disabled_user_cannot_log_in() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;

    let disabled = app
        .post_json("/api/v1/admin/users/2/disable", &json!({}))
        .await;
    assert_eq!(disabled.status(), StatusCode::OK);

    let login = app.login("student-001", "InitialPassword123!").await;
    assert_eq!(login.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admin_can_list_and_create_classes() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;

    let initial = app.get("/api/v1/admin/classes").await;
    assert_eq!(initial.status(), StatusCode::OK);
    let initial_body: Value = app.json(initial).await;
    assert!(initial_body["items"].as_array().is_some_and(Vec::is_empty));

    let created = app
        .post_json(
            "/api/v1/admin/classes",
            &json!({ "name": "软件工程测试班" }),
        )
        .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body: Value = app.json(created).await;
    assert_eq!(created_body["name"], "软件工程测试班");

    let listed = app.get("/api/v1/admin/classes").await;
    assert_eq!(listed.status(), StatusCode::OK);
    let listed_body: Value = app.json(listed).await;
    assert_eq!(listed_body["items"][0]["name"], "软件工程测试班");
}

#[tokio::test]
async fn admin_can_create_a_class_and_assign_it_to_a_user() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;

    let created_class = app
        .post_json(
            "/api/v1/admin/classes",
            &json!({ "name": "软件工程测试班" }),
        )
        .await;
    assert_eq!(created_class.status(), StatusCode::CREATED);
    let class_body: Value = app.json(created_class).await;
    assert_eq!(class_body["name"], "软件工程测试班");
    let class_id = class_body["id"].as_i64().expect("created class has an id");

    let created_user = app
        .post_json(
            "/api/v1/admin/users",
            &json!({
                "username": "student-with-class",
                "display_name": "带班级学生",
                "role": "student",
                "class_id": class_id
            }),
        )
        .await;
    assert_eq!(created_user.status(), StatusCode::CREATED);
    let user_body: Value = app.json(created_user).await;
    assert_eq!(user_body["role"], "student");
    assert_eq!(user_body["class_name"], "软件工程测试班");
}

#[tokio::test]
async fn creating_a_user_with_an_unknown_class_is_rejected() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;

    let response = app
        .post_json(
            "/api/v1/admin/users",
            &json!({
                "username": "student-with-unknown-class",
                "display_name": "未知班级学生",
                "role": "student",
                "class_id": 999_999
            }),
        )
        .await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body: Value = app.json(response).await;
    assert_eq!(body["error"], "class was not found");
}

#[tokio::test]
async fn admin_can_create_and_reset_a_user_without_password_data_in_responses() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;

    let created = app
        .post_json(
            "/api/v1/admin/users",
            &json!({
                "username": "student-002",
                "display_name": "第二位学生",
                "role": "student",
                "student_number": "2026002"
            }),
        )
        .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body: Value = app.json(created).await;
    assert_eq!(created_body["must_change_password"], true);
    assert!(created_body.get("password_hash").is_none());
    let user_id = created_body["id"].as_i64().expect("created user has an id");

    let reset = app
        .post_json(
            &format!("/api/v1/admin/users/{user_id}/reset-password"),
            &json!({}),
        )
        .await;
    assert_eq!(reset.status(), StatusCode::OK);
    let reset_body: Value = app.json(reset).await;
    assert_eq!(reset_body["must_change_password"], true);
    assert!(reset_body.get("password_hash").is_none());
}

#[tokio::test]
async fn ordinary_user_cannot_access_admin_routes() {
    let app: TestApp = spawn_app().await;
    let login = app.login("student-001", "InitialPassword123!").await;
    assert_eq!(login.status(), StatusCode::OK);
    let change = app
        .post_json(
            "/api/v1/auth/change-password",
            &json!({ "new_password": "StudentPassword123!" }),
        )
        .await;
    assert_eq!(change.status(), StatusCode::OK);

    let response = app.get("/api/v1/admin/users").await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn changing_password_invalidates_the_fixed_initial_password() {
    let app: TestApp = spawn_app().await;
    let login = app.login("student-001", "InitialPassword123!").await;
    assert_eq!(login.status(), StatusCode::OK);
    let changed = app
        .post_json(
            "/api/v1/auth/change-password",
            &json!({ "new_password": "StudentPassword123!" }),
        )
        .await;
    assert_eq!(changed.status(), StatusCode::OK);

    let old_login = app.login("student-001", "InitialPassword123!").await;
    assert_eq!(old_login.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn repeated_invalid_logins_temporarily_lock_the_account() {
    let app: TestApp = spawn_app().await;
    for _ in 0..5 {
        let response = app.login("student-001", "wrong-password").await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    let response = app.login("student-001", "InitialPassword123!").await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn the_last_active_admin_cannot_be_disabled() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;

    let response = app
        .post_json("/api/v1/admin/users/1/disable", &json!({}))
        .await;

    assert_eq!(response.status(), StatusCode::CONFLICT);
}
