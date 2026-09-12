//! Tier 1: Feature Coverage Test Suite (>=20 test cases).
//! Covers GET /v1/models, POST /v1/chat/completions (streaming and non-streaming),
//! POST /v1/images/generations, initial VQD handshake, JWK validation, and telemetry.

mod common;

use common::*;
use serde_json::{json, Value};

#[tokio::test]
async fn test_tier1_01_models_list_success() {
    let harness = TestHarness::new().await;
    let resp = harness.get_models().await;

    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body: Value = resp.json().await.expect("Failed to parse models JSON");

    assert_eq!(body["object"], "list");
    let data = body["data"].as_array().expect("Expected data to be array");
    assert!(!data.is_empty(), "Models list should not be empty");
}

#[tokio::test]
async fn test_tier1_02_models_list_schema_and_ownership() {
    let harness = TestHarness::new().await;
    let resp = harness.get_models().await;

    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body: Value = resp.json().await.expect("Failed to parse models JSON");
    let data = body["data"].as_array().unwrap();

    for model in data {
        assert!(model["id"].is_string(), "Model id must be a string");
        assert_eq!(model["object"], "model");
        assert_eq!(model["owned_by"], "duck");
        assert!(model["created"].is_number());
    }
}

#[tokio::test]
async fn test_tier1_03_models_all_aliases_present() {
    let harness = TestHarness::new().await;
    let resp = harness.get_models().await;

    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    let model_ids: Vec<String> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
        .collect();

    let expected = vec![
        "gpt-5.6-luna",
        "gpt5",
        "gpt5_mini",
        "claude",
        "mistral",
        "gemma",
        "image",
    ];
    for expected_id in expected {
        assert!(
            model_ids.iter().any(|id| id == expected_id),
            "Expected model id {} in list: {:?}",
            expected_id,
            model_ids
        );
    }
}

#[tokio::test]
async fn test_tier1_04_models_idempotent_calls() {
    let harness = TestHarness::new().await;
    let resp1 = harness.get_models().await;
    let resp2 = harness.get_models().await;

    assert_eq!(resp1.status(), reqwest::StatusCode::OK);
    assert_eq!(resp2.status(), reqwest::StatusCode::OK);

    let body1: Value = resp1.json().await.unwrap();
    let body2: Value = resp2.json().await.unwrap();

    assert_eq!(body1, body2);
}

#[tokio::test]
async fn test_tier1_05_models_with_auth_bearer_header() {
    let harness = TestHarness::new().await;
    let resp = harness
        .client
        .get(format!("{}/v1/models", harness.server_url))
        .header("authorization", "Bearer sk-antigravity-dummy-token")
        .send()
        .await
        .expect("Request failed");

    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["object"], "list");
}

#[tokio::test]
async fn test_tier1_06_chat_non_stream_single_turn_success() {
    let harness = TestHarness::new().await;
    harness
        .mock_upstream
        .mock_status_ok("test-vqd-token-1")
        .await;
    harness
        .mock_upstream
        .mock_chat_ok("gpt-5.6-luna", &["Hello", " world!"], "test-vqd-token-2")
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "Say hello"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["object"], "chat.completion");
    assert!(body["id"].as_str().unwrap().starts_with("chatcmpl-"));
    assert_eq!(body["choices"][0]["message"]["role"], "assistant");
    assert_eq!(body["choices"][0]["message"]["content"], "Hello world!");
    assert_eq!(body["choices"][0]["finish_reason"], "stop");
    assert_eq!(body["model"], "gpt-5.6-luna");
}

#[tokio::test]
async fn test_tier1_07_chat_non_stream_multi_turn_history() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-multi-1").await;
    harness
        .mock_upstream
        .mock_chat_ok(
            "gpt-5.6-luna",
            &["Paris is the capital of France."],
            "vqd-multi-2",
        )
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [
            {"role": "system", "content": "You are a helpful assistant."},
            {"role": "user", "content": "What is the capital of France?"},
            {"role": "assistant", "content": "It is Paris."},
            {"role": "user", "content": "Confirm please."}
        ],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "Paris is the capital of France."
    );
}

#[tokio::test]
async fn test_tier1_08_chat_non_stream_alias_resolution_gpt5() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-alias-1").await;
    harness
        .mock_upstream
        .mock_chat_ok("gpt-5.6-luna", &["Resolved from gpt5"], "vqd-alias-2")
        .await;

    let payload = json!({
        "model": "gpt5",
        "messages": [{"role": "user", "content": "test alias"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "Resolved from gpt5"
    );
}

#[tokio::test]
async fn test_tier1_09_chat_non_stream_alias_resolution_claude() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-claude-1").await;
    harness
        .mock_upstream
        .mock_chat_ok(
            "claude-haiku-4-5",
            &["Resolved from claude"],
            "vqd-claude-2",
        )
        .await;

    let payload = json!({
        "model": "claude",
        "messages": [{"role": "user", "content": "test claude"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "Resolved from claude"
    );
}

#[tokio::test]
async fn test_tier1_10_chat_non_stream_alias_resolution_gemma() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-gemma-1").await;
    harness
        .mock_upstream
        .mock_chat_ok(
            "tinfoil/gemma4-31b",
            &["Resolved from gemma"],
            "vqd-gemma-2",
        )
        .await;

    let payload = json!({
        "model": "gemma",
        "messages": [{"role": "user", "content": "test gemma"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "Resolved from gemma"
    );
}

#[tokio::test]
async fn test_tier1_11_chat_non_stream_alias_resolution_mistral() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-mistral-1").await;
    harness
        .mock_upstream
        .mock_chat_ok(
            "mistral-small-2603",
            &["Resolved from mistral"],
            "vqd-mistral-2",
        )
        .await;

    let payload = json!({
        "model": "mistral",
        "messages": [{"role": "user", "content": "test mistral"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "Resolved from mistral"
    );
}

#[tokio::test]
async fn test_tier1_12_chat_non_stream_content_parts_array() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-parts-1").await;
    harness
        .mock_upstream
        .mock_chat_ok("gpt-5.6-luna", &["Combined parts response"], "vqd-parts-2")
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": "Part 1 "},
                    {"type": "text", "text": "Part 2"}
                ]
            }
        ],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "Combined parts response"
    );
}

#[tokio::test]
async fn test_tier1_13_chat_streaming_basic_chunks_and_done() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-stream-1").await;
    harness
        .mock_upstream
        .mock_chat_ok("gpt-5.6-luna", &["Rust", " is", " fast."], "vqd-stream-2")
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "stream test"}],
        "stream": true
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/event-stream"
    );

    let (chunks, saw_done) = harness.read_sse_stream(resp).await;
    assert!(saw_done, "SSE stream must terminate with [DONE]");
    let completion_chunks: Vec<_> = chunks
        .into_iter()
        .filter(|c| c.get("object").and_then(|o| o.as_str()) == Some("chat.completion.chunk"))
        .collect();
    assert!(completion_chunks.len() >= 3);

    assert_eq!(completion_chunks[0]["choices"][0]["delta"]["content"], "Rust");
    assert_eq!(completion_chunks[1]["choices"][0]["delta"]["content"], " is");
    assert_eq!(completion_chunks[2]["choices"][0]["delta"]["content"], " fast.");
    assert_eq!(completion_chunks[0]["object"], "chat.completion.chunk");
}

#[tokio::test]
async fn test_tier1_14_chat_streaming_control_frame_filtering() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-ctrl-1").await;
    harness
        .mock_upstream
        .mock_chat_with_control_frames(
            "gpt-5.6-luna",
            &["Clean content 1", " Clean content 2"],
            "vqd-ctrl-2",
        )
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "control frame test"}],
        "stream": true
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let text = harness.read_sse_text(resp).await;
    assert_eq!(text, "Clean content 1 Clean content 2");
    assert!(!text.contains("[PING]"));
    assert!(!text.contains("[LIMIT]"));
    assert!(!text.contains("[CHAT_TITLE]"));
}

#[tokio::test]
async fn test_tier1_15_chat_streaming_multi_turn_stream() {
    let harness = TestHarness::new().await;
    harness
        .mock_upstream
        .mock_status_ok("vqd-stream-multi-1")
        .await;
    harness
        .mock_upstream
        .mock_chat_ok(
            "gpt-5.6-luna",
            &["Multi-turn", " streaming", " success."],
            "vqd-stream-multi-2",
        )
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [
            {"role": "user", "content": "Turn 1"},
            {"role": "assistant", "content": "Reply 1"},
            {"role": "user", "content": "Turn 2"}
        ],
        "stream": true
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let text = harness.read_sse_text(resp).await;
    assert_eq!(text, "Multi-turn streaming success.");
}

#[tokio::test]
async fn test_tier1_16_chat_streaming_response_headers() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-hdr-1").await;
    harness
        .mock_upstream
        .mock_chat_ok("gpt-5.6-luna", &["header check"], "vqd-hdr-2")
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "test"}],
        "stream": true
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(content_type.contains("text/event-stream"));

    let cache_control = resp
        .headers()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(cache_control.contains("no-cache"));
}

#[tokio::test]
async fn test_tier1_17_image_generation_standard_prompt() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-img-1").await;
    harness
        .mock_upstream
        .mock_chat_image(SAMPLE_1X1_PNG_B64, "vqd-img-2")
        .await;

    let payload = json!({
        "prompt": "a red rubber duck in space",
        "model": "image"
    });

    let resp = harness.image_generations(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: Value = resp.json().await.unwrap();
    assert!(body["created"].is_number());
    assert_eq!(body["data"][0]["b64_json"], SAMPLE_1X1_PNG_B64);
}

#[tokio::test]
async fn test_tier1_18_image_generation_b64_json_response() {
    let harness = TestHarness::new().await;
    harness
        .mock_upstream
        .mock_status_ok("vqd-img-schema-1")
        .await;
    harness
        .mock_upstream
        .mock_chat_image("c2FtcGxlLWJhc2U2NC1pbWFnZQ==", "vqd-img-schema-2")
        .await;

    let payload = json!({
        "prompt": "a futuristic city",
        "model": "image"
    });

    let resp = harness.image_generations(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: Value = resp.json().await.unwrap();
    assert!(body["data"].is_array());
    assert_eq!(body["data"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"][0]["b64_json"], "c2FtcGxlLWJhc2U2NC1pbWFnZQ==");
}

#[tokio::test]
async fn test_tier1_19_image_generation_nested_b64_extraction() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-nested-1").await;
    harness
        .mock_upstream
        .mock_chat_nested_image("bmVzdGVkLWJhc2U2NA==", "vqd-nested-2")
        .await;

    let payload = json!({
        "prompt": "nested extraction test",
        "model": "image"
    });

    let resp = harness.image_generations(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["data"][0]["b64_json"], "bmVzdGVkLWJhc2U2NA==");
}

#[tokio::test]
async fn test_tier1_20_image_generation_partial_image_assembly() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-partial-1").await;
    harness
        .mock_upstream
        .mock_chat_partial_images(&["chunk1_", "chunk2_", "chunk3"], "vqd-partial-2")
        .await;

    let payload = json!({
        "prompt": "chunked duck",
        "model": "image"
    });

    let resp = harness.image_generations(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["data"][0]["b64_json"], "chunk1_chunk2_chunk3");
}

#[tokio::test]
async fn test_tier1_21_protocol_vqd_initial_handshake() {
    let harness = TestHarness::new().await;
    harness
        .mock_upstream
        .mock_status_ok("fresh-initial-vqd-token")
        .await;
    harness
        .mock_upstream
        .mock_chat_ok("gpt-5.6-luna", &["handshake ok"], "next-vqd-token")
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "trigger handshake"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
}

#[tokio::test]
async fn test_tier1_22_protocol_rsa_jwk_public_key_format() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("jwk-test-vqd").await;
    // DuckPayloadMatcher automatically verifies the RSA JWK public key structure and modulus
    harness
        .mock_upstream
        .mock_chat_ok("gpt-5.6-luna", &["jwk verified"], "jwk-next-vqd")
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "verify jwk"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
}

#[tokio::test]
async fn test_tier1_23_protocol_telemetry_headers_present() {
    let harness = TestHarness::new().await;
    harness
        .mock_upstream
        .mock_status_ok("telemetry-vqd-1")
        .await;
    // DuckHeadersMatcher checks x-fe-version, x-ddg-journey-id, x-fe-signals
    harness
        .mock_upstream
        .mock_chat_ok("gpt-5.6-luna", &["telemetry verified"], "telemetry-vqd-2")
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "verify telemetry"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
}

#[tokio::test]
async fn test_tier1_24_protocol_browser_fingerprint_headers() {
    let harness = TestHarness::new().await;
    harness
        .mock_upstream
        .mock_status_ok("fingerprint-vqd-1")
        .await;
    // DuckHeadersMatcher validates UA, sec-ch-ua, etc.
    harness
        .mock_upstream
        .mock_chat_ok(
            "gpt-5.6-luna",
            &["fingerprint verified"],
            "fingerprint-vqd-2",
        )
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "verify fingerprint"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
}
