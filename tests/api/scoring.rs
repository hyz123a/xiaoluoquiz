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
async fn practice_accuracy_counts_each_non_short_question_once_and_excludes_short_answers() {
    let app: TestApp = spawn_app().await;
    app.login_as_admin().await;

    let initial = app.get("/api/v1/practice/stats").await;
    assert_eq!(initial.status(), StatusCode::OK);
    let initial_body: Value = app.json(initial).await;
    assert_eq!(initial_body["answered_count"], 0);
    assert_eq!(initial_body["correct_count"], 0);
    assert!(initial_body["accuracy_percent"].is_null());

    let first_correct = app
        .post_json(
            "/api/v1/questions/1/check",
            &json!({"answer":{"type":"single_choice","option_key":"B"}}),
        )
        .await;
    assert_eq!(first_correct.status(), StatusCode::OK);
    let first_correct_body: Value = app.json(first_correct).await;
    assert_eq!(first_correct_body["practice_stats"]["answered_count"], 1);
    assert_eq!(first_correct_body["practice_stats"]["correct_count"], 1);
    assert_eq!(
        first_correct_body["practice_stats"]["accuracy_percent"],
        100.0
    );

    let repeated_incorrect = app
        .post_json(
            "/api/v1/questions/1/check",
            &json!({"answer":{"type":"single_choice","option_key":"A"}}),
        )
        .await;
    assert_eq!(repeated_incorrect.status(), StatusCode::OK);
    let repeated_incorrect_body: Value = app.json(repeated_incorrect).await;
    assert_eq!(repeated_incorrect_body["status"], "incorrect");
    assert_eq!(
        repeated_incorrect_body["practice_stats"]["answered_count"],
        1
    );
    assert_eq!(
        repeated_incorrect_body["practice_stats"]["correct_count"],
        1
    );

    let created = app
        .post_json(
            "/api/v1/admin/questions",
            &json!({
                "question_bank_id": 2,
                "question_type": "single_choice",
                "stem": "练习正确率重复统计测试题？",
                "blank_count": 0,
                "options": [
                    {"key": "A", "text": "错误"},
                    {"key": "B", "text": "正确"}
                ],
                "explanation": null,
                "correct_answer": {"type": "single_choice", "option_key": "A"}
            }),
        )
        .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body: Value = app.json(created).await;
    let question_id = created_body["id"]
        .as_i64()
        .expect("created question should have an id");
    let published = app
        .post_json(
            &format!("/api/v1/admin/questions/{question_id}/publish"),
            &json!({}),
        )
        .await;
    assert_eq!(published.status(), StatusCode::OK);

    let first_incorrect = app
        .post_json(
            &format!("/api/v1/questions/{question_id}/check"),
            &json!({"answer":{"type":"single_choice","option_key":"B"}}),
        )
        .await;
    assert_eq!(first_incorrect.status(), StatusCode::OK);
    let first_incorrect_body: Value = app.json(first_incorrect).await;
    assert_eq!(first_incorrect_body["practice_stats"]["answered_count"], 2);
    assert_eq!(first_incorrect_body["practice_stats"]["correct_count"], 1);
    assert_eq!(
        first_incorrect_body["practice_stats"]["accuracy_percent"],
        50.0
    );

    let repeated_correct = app
        .post_json(
            &format!("/api/v1/questions/{question_id}/check"),
            &json!({"answer":{"type":"single_choice","option_key":"A"}}),
        )
        .await;
    assert_eq!(repeated_correct.status(), StatusCode::OK);
    let repeated_correct_body: Value = app.json(repeated_correct).await;
    assert_eq!(repeated_correct_body["practice_stats"]["answered_count"], 2);
    assert_eq!(repeated_correct_body["practice_stats"]["correct_count"], 1);
    assert_eq!(
        repeated_correct_body["practice_stats"]["accuracy_percent"],
        50.0
    );

    let short_created = app
        .post_json(
            "/api/v1/admin/questions",
            &json!({
                "question_bank_id": 2,
                "question_type": "short_answer",
                "stem": "练习正确率不统计简答题？",
                "blank_count": 0,
                "options": [],
                "explanation": null,
                "correct_answer": {
                    "type": "short_answer",
                    "reference": "不计入整体正确率。",
                    "rubric": null
                }
            }),
        )
        .await;
    assert_eq!(short_created.status(), StatusCode::CREATED);
    let short_created_body: Value = app.json(short_created).await;
    let short_id = short_created_body["id"]
        .as_i64()
        .expect("short-answer question should have an id");
    let short_published = app
        .post_json(
            &format!("/api/v1/admin/questions/{short_id}/publish"),
            &json!({}),
        )
        .await;
    assert_eq!(short_published.status(), StatusCode::OK);

    let short_checked = app
        .post_json(
            &format!("/api/v1/questions/{short_id}/check"),
            &json!({"answer":{"type":"short_answer","text":"我的答案"}}),
        )
        .await;
    assert_eq!(short_checked.status(), StatusCode::OK);
    let short_checked_body: Value = app.json(short_checked).await;
    assert_eq!(short_checked_body["status"], "needs_review");
    assert_eq!(short_checked_body["practice_stats"]["answered_count"], 2);
    assert_eq!(short_checked_body["practice_stats"]["correct_count"], 1);
    assert_eq!(
        short_checked_body["practice_stats"]["accuracy_percent"],
        50.0
    );

    let final_stats = app.get("/api/v1/practice/stats").await;
    assert_eq!(final_stats.status(), StatusCode::OK);
    let final_body: Value = app.json(final_stats).await;
    assert_eq!(final_body["answered_count"], 2);
    assert_eq!(final_body["correct_count"], 1);
    assert_eq!(final_body["accuracy_percent"], 50.0);
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
