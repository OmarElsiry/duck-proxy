//! Integration test suite for Duck.ai Reasoning Mode and Test Guards.

use duck_proxy_rs::crypto::EphemeralKeypair;
use duck_proxy_rs::duck::payload::{assert_reasoning_mode, build_chat_payload, guard_reasoning_mode};
use duck_proxy_rs::duck::types::{DuckChatMessage, REASONING_EFFORT_FAST};

#[test]
fn test_wire_protocol_reasoning_effort_adaptive_max() {
    let keypair = EphemeralKeypair::generate().expect("keypair generation");
    let test_cases = vec![
        ("gpt-5.6-luna", "low"),
        ("gpt-5.4-mini", "medium"), // gpt-5.4-mini supports medium as max
        ("claude-haiku-4-5", "low"),
        ("mistral-small-2603", "low"),
        ("tinfoil/gemma4-31b", "low"),
    ];

    for (model, expected_effort) in test_cases {
        let messages = vec![DuckChatMessage {
            role: "user".to_string(),
            content: "Explain Fermat's Last Theorem step-by-step".to_string(),
        }];

        let payload = build_chat_payload(model, messages, &keypair, false, "conv-reason-wire");

        // 1. In-memory struct verification
        assert_eq!(
            payload.reasoning_effort, expected_effort,
            "Model {} must use max reasoning effort '{}'", model, expected_effort
        );
        assert!(payload.is_reasoning_mode());
        assert_eq!(guard_reasoning_mode(&payload), Ok(()));
        assert_reasoning_mode(&payload);

        // 2. Wire JSON serialization verification (camelCase reasoningEffort)
        let json_val = serde_json::to_value(&payload).expect("Serialization to JSON");
        assert_eq!(
            json_val.get("reasoningEffort").and_then(|v| v.as_str()),
            Some(expected_effort),
            "Wire payload JSON for model {} must contain 'reasoningEffort': '{}'", model, expected_effort
        );
    }
}

#[test]
fn test_test_guard_rejects_non_reasoning_payloads() {
    let keypair = EphemeralKeypair::generate().expect("keypair generation");
    let messages = vec![DuckChatMessage {
        role: "user".to_string(),
        content: "Hello".to_string(),
    }];

    let mut payload = build_chat_payload("gpt-5.6-luna", messages, &keypair, false, "conv-guard-test");
    // Initially valid
    assert!(guard_reasoning_mode(&payload).is_ok());

    // Tamper 1: "none"
    payload.reasoning_effort = REASONING_EFFORT_FAST.to_string();
    assert!(!payload.is_reasoning_mode());
    let err = guard_reasoning_mode(&payload).expect_err("Must reject 'none'");
    assert!(err.contains("ReasoningModeGuardViolation"));

    // Tamper 2: empty string
    payload.reasoning_effort = String::new();
    assert!(!payload.is_reasoning_mode());
    assert!(guard_reasoning_mode(&payload).is_err());

    // Tamper 3: invalid arbitrary value
    payload.reasoning_effort = "invalid_mode".to_string();
    assert!(!payload.is_reasoning_mode());
    assert!(guard_reasoning_mode(&payload).is_err());
}

#[test]
#[should_panic(expected = "ReasoningModeGuardViolation")]
fn test_assert_guard_panics_on_non_reasoning_payload() {
    let keypair = EphemeralKeypair::generate().expect("keypair generation");
    let mut payload = build_chat_payload("gpt-5.6-luna", vec![], &keypair, false, "conv-panic-test");
    payload.reasoning_effort = "none".to_string();
    assert_reasoning_mode(&payload);
}

#[test]
fn test_image_generation_bypasses_reasoning_effort() {
    let keypair = EphemeralKeypair::generate().expect("keypair generation");
    let messages = vec![DuckChatMessage {
        role: "user".to_string(),
        content: "Generate an illustration of a sunset".to_string(),
    }];

    // Image gen request
    let img_payload = build_chat_payload("gpt-5.6-luna", messages, &keypair, true, "conv-img-gen");
    assert_eq!(img_payload.reasoning_effort, "none");
    assert_eq!(img_payload.metadata.tool_choice.generate_image, Some(true));

    let json_val = serde_json::to_value(&img_payload).expect("Serialization");
    assert_eq!(
        json_val.get("reasoningEffort").and_then(|v| v.as_str()),
        Some("none")
    );
}

#[test]
fn test_reasoning_mode_preservation_across_conversations() {
    let keypair = EphemeralKeypair::generate().expect("keypair generation");
    for i in 0..10 {
        let conv_id = format!("conv-multi-{}", i);
        let payload = build_chat_payload(
            "gpt-5.6-luna",
            vec![DuckChatMessage { role: "user".to_string(), content: format!("Query {}", i) }],
            &keypair,
            false,
            &conv_id,
        );
        assert_eq!(payload.reasoning_effort, "low");
        assert!(payload.is_reasoning_mode());
        assert_reasoning_mode(&payload);
    }
}
