//! End-to-end tests for Virtual User isolation and automatic switching on 429 ERR_USER_LIMIT.

mod common;

use common::*;
use serde_json::{json, Value};
use duck_proxy_rs::duck::virtual_user::{VirtualUser, VirtualUserPool};
use duck_proxy_rs::duck::payload::{sanitize_and_fit_messages, MAX_TOTAL_CHARS};
use duck_proxy_rs::duck::types::DuckChatMessage;


#[tokio::test]
async fn test_virtual_user_crypto_and_cookie_isolation() {
    let vu1 = VirtualUser::new("vu-1", 0);
    let vu2 = VirtualUser::new("vu-2", 1);

    // Verify cryptographic keys are distinct
    assert_ne!(vu1.keypair.public_jwk().n, vu2.keypair.public_jwk().n);
    assert_ne!(vu1.user_agent, vu2.user_agent);

    // Verify rate limit tracking is independent
    vu1.mark_model_rate_limited("gpt-5.6-luna", None).await;
    assert!(vu1.is_model_rate_limited("gpt-5.6-luna").await);
    assert!(!vu2.is_model_rate_limited("gpt-5.6-luna").await);
}

#[tokio::test]
async fn test_virtual_user_pool_selection_and_dynamic_spawn() {
    let pool = VirtualUserPool::new(2);
    assert_eq!(pool.total_users().await, 2);

    let u1 = pool.select_user_for_model("gpt-5.6-luna", None).await;
    assert_eq!(u1.id, "vu-1");

    // Mark vu-1 as rate limited
    u1.mark_model_rate_limited("gpt-5.6-luna", None).await;

    // Next selection should pick vu-2
    let u2 = pool.select_user_for_model("gpt-5.6-luna", None).await;
    assert_eq!(u2.id, "vu-2");

    // Mark vu-2 as rate limited too
    u2.mark_model_rate_limited("gpt-5.6-luna", None).await;

    // Next selection should dynamically spawn a new virtual user!
    let u3 = pool.select_user_for_model("gpt-5.6-luna", None).await;
    assert!(u3.id.starts_with("vu-dyn-"));
    assert_eq!(pool.total_users().await, 3);
}

#[test]
fn test_context_fitting_preserves_instructions_and_prompt() {
    let system_instructions = "A".repeat(3000);
    let intermediate_turn_1 = "B".repeat(2000);
    let intermediate_turn_2 = "C".repeat(2000);
    let user_prompt = "D".repeat(2000);

    let messages = vec![
        DuckChatMessage { role: "user".to_string(), content: system_instructions },
        DuckChatMessage { role: "assistant".to_string(), content: intermediate_turn_1 },
        DuckChatMessage { role: "user".to_string(), content: intermediate_turn_2 },
        DuckChatMessage { role: "user".to_string(), content: user_prompt },
    ];

    let fitted = sanitize_and_fit_messages(messages);
    let total_len: usize = fitted.iter().map(|m| m.content.len()).sum();
    assert!(total_len <= MAX_TOTAL_CHARS);

    // Verify first message (system instructions) and last message (prompt) are preserved
    assert!(fitted.first().unwrap().content.contains("AAAAA"));
    assert!(fitted.last().unwrap().content.contains("DDDDD"));
}

#[tokio::test]
async fn test_auto_switch_virtual_user_when_upstream_returns_429() {
    let harness = TestHarness::with_auto_fallback(false).await;

    harness.mock_upstream.mock_status_ok("initial-vqd-token").await;

    // First attempt on VU #1 returns 429 ERR_USER_LIMIT (1 time)
    let error_body = r#"{"action":"error","status":429,"type":"ERR_USER_LIMIT","r":"fixed_cost_window_limit","fixedCostWindowUsage":{"windows":[{"id":"day","percentUsed":100,"resetAt":"2026-09-02T00:00:00.000Z","isBlocked":true}]}}"#;
    harness.mock_upstream.mock_chat_error_for_model_times("gpt-5.6-luna", 429, error_body, 1).await;

    // Second attempt on VU #2 succeeds seamlessly!
    harness.mock_upstream.mock_chat_ok(
        "gpt-5.6-luna",
        &["Response from fresh Virtual User identity!"],
        "chained-vqd-2",
    ).await;


    let payload = json!({
        "model": "gpt-5.6-luna",
        "messages": [{"role": "user", "content": "Hello via virtual user"}],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "Response from fresh Virtual User identity!"
    );
}
