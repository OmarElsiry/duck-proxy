//! Tier 2: Boundary & Corner Cases Test Suite (>=15 test cases).
//! Covers invalid inputs, empty inputs, 429 exponential backoff retries,
//! malformed challenges, upstream error codes (500/502/503/403/418), and truncated streams.

mod common;

use common::*;
use serde_json::{json, Value};

#[tokio::test]
async fn test_tier2_01_chat_missing_model_400() {
    let harness = TestHarness::new().await;
    let payload = json!({
        "messages": [{"role": "user", "content": "hi"}]
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "invalid_request_error");
}

#[tokio::test]
async fn test_tier2_02_chat_unknown_model_400() {
    let harness = TestHarness::new().await;
    let payload = json!({
        "model": "non-existent-gpt-999",
        "messages": [{"role": "user", "content": "hi"}]
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "invalid_request_error");
}

#[tokio::test]
async fn test_tier2_03_chat_empty_messages_list_400() {
    let harness = TestHarness::new().await;
    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": []
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "invalid_request_error");
}

#[tokio::test]
async fn test_tier2_04_chat_missing_messages_field_400() {
    let harness = TestHarness::new().await;
    let payload = json!({
        "model": "gpt-5.6-luna"
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "invalid_request_error");
}

#[tokio::test]
async fn test_tier2_05_chat_malformed_json_body_400() {
    let harness = TestHarness::new().await;
    let resp = harness
        .client
        .post(format!("{}/v1/chat/completions", harness.server_url))
        .header("content-type", "application/json")
        .body("{ not a valid json payload ...")
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "invalid_request_error");
}

#[tokio::test]
async fn test_tier2_06_images_empty_prompt_400() {
    let harness = TestHarness::new().await;
    let payload = json!({
        "prompt": "",
        "model": "image"
    });

    let resp = harness.image_generations(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "invalid_request_error");
}

#[tokio::test]
async fn test_tier2_07_images_missing_prompt_400() {
    let harness = TestHarness::new().await;
    let payload = json!({
        "model": "image"
    });

    let resp = harness.image_generations(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "invalid_request_error");
}

#[tokio::test]
async fn test_tier2_08_chat_empty_message_content_string() {
    let harness = TestHarness::new().await;
    harness
        .mock_upstream
        .mock_status_ok("vqd-empty-content-1")
        .await;
    harness
        .mock_upstream
        .mock_chat_ok(
            "gpt-5.6-luna",
            &["How can I help you?"],
            "vqd-empty-content-2",
        )
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": ""}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "How can I help you?"
    );
}

#[tokio::test]
async fn test_tier2_09_status_429_retry_success_after_2_attempts() {
    let harness = TestHarness::new().await;
    // Status returns 429 for 2 attempts, then succeeds on attempt 3
    harness
        .mock_upstream
        .mock_status_429_then_ok(2, "recovered-vqd-token")
        .await;
    harness
        .mock_upstream
        .mock_chat_ok("gpt-5.6-luna", &["Recovered successfully"], "next-vqd")
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "retry test"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "Recovered successfully"
    );
}

#[tokio::test]
async fn test_tier2_10_status_429_exhausted_5_attempts_failure() {
    let harness = TestHarness::new().await;
    // Status always returns 429 -> exhausts 5 retry attempts
    harness.mock_upstream.mock_status_429_always().await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "exhaustion test"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::TOO_MANY_REQUESTS);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "rate_limit_error");
}

#[tokio::test]
async fn test_tier2_11_chat_429_conversation_limit_error() {
    let harness = TestHarness::new().await;
    harness
        .mock_upstream
        .mock_status_ok("vqd-conv-limit-1")
        .await;
    harness
        .mock_upstream
        .mock_chat_error(429, r#"{"action":"error","type":"ERR_CONVERSATION_LIMIT"}"#)
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "limit test"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::TOO_MANY_REQUESTS);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "rate_limit_error");
}

#[tokio::test]
async fn test_tier2_12_challenge_malformed_base64_handling() {
    let harness = TestHarness::new().await;
    harness
        .mock_upstream
        .mock_status_ok("!!!INVALID_NON_BASE64_HASH@@@")
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "malformed test"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_GATEWAY);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "api_error");
}

#[tokio::test]
async fn test_tier2_13_challenge_upstream_418_rejected() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-418-test").await;
    harness
        .mock_upstream
        .mock_chat_error(418, "challenge rejected")
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "challenge reject test"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert!(
        resp.status() == reqwest::StatusCode::BAD_GATEWAY
            || resp.status() == reqwest::StatusCode::INTERNAL_SERVER_ERROR
    );

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "api_error");
}

#[tokio::test]
async fn test_tier2_14_challenge_sse_err_challenge_event() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-sse-err").await;
    harness
        .mock_upstream
        .mock_chat_sse_error(400, "ERR_CHALLENGE")
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "sse challenge error"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_GATEWAY);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "api_error");
}

#[tokio::test]
async fn test_tier2_15_upstream_500_internal_server_error() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-500").await;
    harness
        .mock_upstream
        .mock_chat_error(500, r#"{"error":"internal crash"}"#)
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "test 500"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_GATEWAY);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "api_error");
    assert!(body["error"]["param"].is_null());
}

#[tokio::test]
async fn test_tier2_16_upstream_502_bad_gateway() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-502").await;
    harness
        .mock_upstream
        .mock_chat_error(502, "Bad Gateway from Cloudflare")
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "test 502"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_GATEWAY);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "api_error");
}

#[tokio::test]
async fn test_tier2_17_upstream_503_service_unavailable() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-503").await;
    harness
        .mock_upstream
        .mock_chat_error(503, "Service Unavailable")
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "test 503"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert!(
        resp.status() == reqwest::StatusCode::BAD_GATEWAY
            || resp.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE
    );

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "api_error");
}

#[tokio::test]
async fn test_tier2_18_upstream_403_forbidden() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-403").await;
    harness
        .mock_upstream
        .mock_chat_error(403, "Cloudflare Bot Detection Block")
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "test 403"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert!(
        resp.status() == reqwest::StatusCode::FORBIDDEN
            || resp.status() == reqwest::StatusCode::BAD_GATEWAY
    );

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "api_error");
}

#[tokio::test]
async fn test_tier2_19_stream_truncated_tcp_connection_drop() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-trunc").await;
    harness
        .mock_upstream
        .mock_chat_truncated_stream(&["Part 1", " Part 2"], "vqd-trunc-next")
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "truncated stream"}],
        "stream": true
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let (chunks, _saw_done) = harness.read_sse_stream(resp).await;
    assert!(!chunks.is_empty(), "Proxy must stream available chunks before truncation");
}

#[tokio::test]
async fn test_tier2_20_stream_empty_sse_stream_handling() {
    let harness = TestHarness::new().await;
    harness
        .mock_upstream
        .mock_status_ok("vqd-empty-stream")
        .await;
    harness
        .mock_upstream
        .mock_chat_raw_sse("", "vqd-next")
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "empty stream"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_GATEWAY);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["type"], "api_error");
}
