//! Integration tests for Automatic Model Fallback Cascade and VQD Token Pool.

mod common;

use common::*;
use serde_json::{json, Value};

#[tokio::test]
async fn test_cascade_fallback_when_primary_model_429() {
    let harness = TestHarness::with_auto_fallback(true).await;

    harness.mock_upstream.mock_status_ok("initial-vqd-token").await;
    harness.mock_upstream.mock_chat_error_for_model("gpt-5.6-luna", 429, "rate limit exceeded").await;
    harness
        .mock_upstream
        .mock_chat_ok(
            "gpt-5.4-mini",
            &["Hello! I am answering via seamless model fallback cascade."],
            "next-chained-vqd",
        )
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "Hello assistant!"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "Hello! I am answering via seamless model fallback cascade."
    );
    assert_eq!(body["model"], "gpt-5.6-luna");
}

#[tokio::test]
async fn test_cascade_fallback_streaming_when_primary_model_429() {
    let harness = TestHarness::with_auto_fallback(true).await;

    harness.mock_upstream.mock_status_ok("initial-vqd-stream").await;
    harness.mock_upstream.mock_chat_error_for_model("gpt-5.6-luna", 429, "rate limit exceeded").await;
    harness
        .mock_upstream
        .mock_chat_ok(
            "gpt-5.4-mini",
            &["Streaming ", "fallback ", "content."],
            "next-stream-vqd",
        )
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "Stream me a response"}],
        "stream": true
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let stream_text = resp.text().await.unwrap();
    assert!(stream_text.contains("Streaming "));
    assert!(stream_text.contains("fallback "));
    assert!(stream_text.contains("content."));
    assert!(stream_text.contains("[DONE]"));
}
