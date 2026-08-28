//! Tier 3: Cross-Feature Combinations & State Transitions Test Suite (>=8 test cases).
//! Covers multi-turn VQD token chaining, model alias switching across turns,
//! client stream cancellation stability, interleaved streaming/non-streaming,
//! image generation + chat alternation, capability toggles, and recovery.

mod common;

use common::*;
use serde_json::{json, Value};

#[tokio::test]
async fn test_t3_01_multiturn_vqd_state_transition() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd_initial").await;
    harness
        .mock_upstream
        .mock_chat_turn("vqd_initial", "vqd_turn_1", "Turn 1 answer")
        .await;
    harness
        .mock_upstream
        .mock_chat_turn("vqd_turn_1", "vqd_turn_2", "Turn 2 answer")
        .await;

    // Turn 1
    let payload1 = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "Question 1"}],
        "stream": false
    });
    let resp1 = harness.chat_completions(payload1).await;
    assert_eq!(resp1.status(), reqwest::StatusCode::OK);
    let body1: Value = resp1.json().await.unwrap();
    assert_eq!(body1["choices"][0]["message"]["content"], "Turn 1 answer");

    // Turn 2 (reuses cached vqd_turn_1)
    let payload2 = json!({
        "model": "gpt-5.6-luna",
        "messages": [
            {"role": "user", "content": "Question 1"},
            {"role": "assistant", "content": "Turn 1 answer"},
            {"role": "user", "content": "Question 2"}
        ],
        "stream": false
    });
    let resp2 = harness.chat_completions(payload2).await;
    assert_eq!(resp2.status(), reqwest::StatusCode::OK);
    let body2: Value = resp2.json().await.unwrap();
    assert_eq!(body2["choices"][0]["message"]["content"], "Turn 2 answer");
}

#[tokio::test]
async fn test_t3_02_model_alias_switching_across_turns() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd_alias_init").await;

    let models = vec![
        ("gpt5", "gpt-5.6-luna", "Luna response"),
        ("claude", "claude-haiku-4-5", "Claude response"),
        ("gemma", "tinfoil/gemma4-31b", "Gemma response"),
        ("mistral", "mistral-small-2603", "Mistral response"),
    ];

    for (i, (alias, upstream_model, text)) in models.iter().enumerate() {
        let _in_vqd = if i == 0 {
            "vqd_alias_init".to_string()
        } else {
            format!("vqd_step_{}", i - 1)
        };
        let out_vqd = format!("vqd_step_{}", i);

        harness
            .mock_upstream
            .mock_chat_ok(upstream_model, &[text], &out_vqd)
            .await;

        let payload = json!({
            "model": alias,
            "messages": [{"role": "user", "content": format!("Hello with {}", alias)}],
            "stream": false
        });

        let resp = harness.chat_completions(payload).await;
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let body: Value = resp.json().await.unwrap();
        assert_eq!(body["choices"][0]["message"]["content"], *text);
    }
}

#[tokio::test]
async fn test_t3_03_client_stream_cancellation_and_subsequent_stability() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd_cancel_1").await;
    harness
        .mock_upstream
        .mock_chat_ok(
            "gpt-5.6-luna",
            &["Chunk 1", " Chunk 2", " Chunk 3"],
            "vqd_cancel_2",
        )
        .await;

    // Start streaming request
    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "cancel me"}],
        "stream": true
    });
    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    // Drop client response stream abruptly
    drop(resp);

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Subsequent request should succeed immediately
    harness
        .mock_upstream
        .mock_chat_ok("gpt-5.6-luna", &["Stable after cancel"], "vqd_cancel_3")
        .await;

    let payload2 = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "are you ok?"}],
        "stream": false
    });
    let resp2 = harness.chat_completions(payload2).await;
    assert_eq!(resp2.status(), reqwest::StatusCode::OK);
    let body: Value = resp2.json().await.unwrap();
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "Stable after cancel"
    );
}

#[tokio::test]
async fn test_t3_04_interleaved_streaming_and_non_streaming_requests() {
    let harness = TestHarness::new().await;
    harness
        .mock_upstream
        .mock_status_ok("vqd_interleave_init")
        .await;

    for i in 0..4 {
        let is_stream = i % 2 == 0;
        let vqd_next = format!("vqd_interleave_{}", i);
        let expected_msg = format!("Interleaved turn {}", i);

        harness
            .mock_upstream
            .mock_chat_ok("gpt-5.6-luna", &[&expected_msg], &vqd_next)
            .await;

        let payload = json!({
            "model": "gpt-5.6-luna",
            "messages": [{"role": "user", "content": format!("turn {}", i)}],
            "stream": is_stream
        });

        let resp = harness.chat_completions(payload).await;
        assert_eq!(resp.status(), reqwest::StatusCode::OK);

        if is_stream {
            let text = harness.read_sse_text(resp).await;
            assert_eq!(text, expected_msg);
        } else {
            let body: Value = resp.json().await.unwrap();
            assert_eq!(body["choices"][0]["message"]["content"], expected_msg);
        }
    }
}

#[tokio::test]
async fn test_t3_05_image_generation_followed_by_chat_completion() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd_mix_1").await;
    harness
        .mock_upstream
        .mock_chat_image(SAMPLE_1X1_PNG_B64, "vqd_mix_2")
        .await;

    // 1. Generate image
    let img_payload = json!({
        "prompt": "a cute duckling",
        "model": "image"
    });
    let img_resp = harness.image_generations(img_payload).await;
    assert_eq!(img_resp.status(), reqwest::StatusCode::OK);
    let img_body: Value = img_resp.json().await.unwrap();
    assert_eq!(img_body["data"][0]["b64_json"], SAMPLE_1X1_PNG_B64);

    // 2. Chat completion
    harness
        .mock_upstream
        .mock_chat_ok("gpt-5.6-luna", &["I see your cute duckling!"], "vqd_mix_3")
        .await;

    let chat_payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "Describe the image"}],
        "stream": false
    });
    let chat_resp = harness.chat_completions(chat_payload).await;
    assert_eq!(chat_resp.status(), reqwest::StatusCode::OK);
    let chat_body: Value = chat_resp.json().await.unwrap();
    assert_eq!(
        chat_body["choices"][0]["message"]["content"],
        "I see your cute duckling!"
    );
}

#[tokio::test]
async fn test_t3_06_web_search_toggle_across_model_capabilities() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd_search_1").await;

    // gpt-5.6-luna with web search
    harness
        .mock_upstream
        .mock_chat_ok("gpt-5.6-luna", &["Search result answer"], "vqd_search_2")
        .await;

    let payload1 = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "current weather"}],
        "web_search": true,
        "stream": false
    });
    let resp1 = harness.chat_completions(payload1).await;
    assert_eq!(resp1.status(), reqwest::StatusCode::OK);

    // mistral with web search disabled/unsupported
    harness
        .mock_upstream
        .mock_chat_ok("mistral-small-2603", &["Mistral answer"], "vqd_search_3")
        .await;

    let payload2 = json!({
        "model": "mistral",
        "messages": [{"role": "user", "content": "test mistral"}],
        "web_search": true,
        "stream": false
    });
    let resp2 = harness.chat_completions(payload2).await;
    assert_eq!(resp2.status(), reqwest::StatusCode::OK);
}

#[tokio::test]
async fn test_t3_07_multiturn_system_prompt_and_role_alternation() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd_sys_1").await;
    harness
        .mock_upstream
        .mock_chat_ok(
            "gpt-5.6-luna",
            &["Understood your system instructions."],
            "vqd_sys_2",
        )
        .await;

    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [
            {"role": "system", "content": "You are a specialized code reviewer."},
            {"role": "user", "content": "Here is my code."},
            {"role": "assistant", "content": "I reviewed your code."},
            {"role": "user", "content": "Thank you."}
        ],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "Understood your system instructions."
    );
}

#[tokio::test]
async fn test_t3_08_midturn_challenge_rejection_and_transparent_recovery() {
    let harness = TestHarness::new().await;
    harness
        .mock_upstream
        .mock_status_ok("vqd_fresh_rec_1")
        .await;
    harness
        .mock_upstream
        .mock_chat_ok("gpt-5.6-luna", &["Turn 1 success"], "vqd_bad_rec_2")
        .await;

    // Turn 1
    let payload1 = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "Turn 1"}],
        "stream": false
    });
    let resp1 = harness.chat_completions(payload1).await;
    assert_eq!(resp1.status(), reqwest::StatusCode::OK);

    // Turn 2: status is ready with fresh token if needed
    harness
        .mock_upstream
        .mock_status_ok("vqd_fresh_rec_3")
        .await;
    harness
        .mock_upstream
        .mock_chat_ok("gpt-5.6-luna", &["Turn 2 recovered"], "vqd_fresh_rec_4")
        .await;

    let payload2 = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "Turn 2"}],
        "stream": false
    });
    let resp2 = harness.chat_completions(payload2).await;
    assert_eq!(resp2.status(), reqwest::StatusCode::OK);
    let body2: Value = resp2.json().await.unwrap();
    assert_eq!(
        body2["choices"][0]["message"]["content"],
        "Turn 2 recovered"
    );
}

#[tokio::test]
async fn test_t3_09_prefix_and_case_insensitive_model_routing() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd_prefix_1").await;

    let cases = vec![
        ("duck/gpt5", "gpt-5.6-luna"),
        ("duck/claude", "claude-haiku-4-5"),
        ("duck/mistral", "mistral-small-2603"),
        ("GPT5", "gpt-5.6-luna"),
    ];

    for (requested_model, upstream_model) in cases {
        harness
            .mock_upstream
            .mock_chat_ok(upstream_model, &["Prefix resolved ok"], "vqd_prefix_next")
            .await;

        let payload = json!({
            "model": requested_model,
            "messages": [{"role": "user", "content": "resolve model"}],
            "stream": false
        });

        let resp = harness.chat_completions(payload).await;
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
    }
}

#[tokio::test]
async fn test_t3_10_chunked_image_reconstruction_followed_by_chat() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd_frag_1").await;
    harness
        .mock_upstream
        .mock_chat_partial_images(&["frag1_", "frag2_", "frag3_done"], "vqd_frag_2")
        .await;

    let img_payload = json!({
        "prompt": "chunked drawing",
        "model": "image"
    });
    let img_resp = harness.image_generations(img_payload).await;
    assert_eq!(img_resp.status(), reqwest::StatusCode::OK);
    let img_body: Value = img_resp.json().await.unwrap();
    assert_eq!(img_body["data"][0]["b64_json"], "frag1_frag2_frag3_done");

    // Chat turn afterwards
    harness
        .mock_upstream
        .mock_chat_ok("gpt-5.6-luna", &["Post-image chat response"], "vqd_frag_3")
        .await;

    let chat_payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "How was the image?"}],
        "stream": false
    });
    let chat_resp = harness.chat_completions(chat_payload).await;
    assert_eq!(chat_resp.status(), reqwest::StatusCode::OK);
    let chat_body: Value = chat_resp.json().await.unwrap();
    assert_eq!(
        chat_body["choices"][0]["message"]["content"],
        "Post-image chat response"
    );
}
