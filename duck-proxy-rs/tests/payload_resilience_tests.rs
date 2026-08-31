use duck_proxy_rs::duck::payload::build_chat_payload;
use duck_proxy_rs::duck::types::DuckChatMessage;
use duck_proxy_rs::crypto::EphemeralKeypair;

#[test]
fn test_payload_truncation_preserves_prompt_at_end() {
    let keypair = EphemeralKeypair::generate().unwrap();
    let old_turn_1 = DuckChatMessage {
        role: "user".to_string(),
        content: "A".repeat(4000),
    };
    let old_turn_2 = DuckChatMessage {
        role: "assistant".to_string(),
        content: "B".repeat(4000),
    };
    let active_prompt = DuckChatMessage {
        role: "user".to_string(),
        content: "Create a calculator in Python".to_string(),
    };

    let messages = vec![old_turn_1, old_turn_2, active_prompt];
    let payload = build_chat_payload("gpt-5.6-luna", messages, &keypair, false, "conv-123");

    let total_chars: usize = payload.messages.iter().map(|m| m.content.len()).sum();
    assert!(total_chars <= 7500, "Total payload chars must be <= 7500, got {}", total_chars);
    assert!(
        payload.messages.last().unwrap().content.contains("Create a calculator in Python"),
        "The active user prompt must be preserved at the end"
    );
}

#[test]
fn test_single_oversized_message_truncation() {
    let keypair = EphemeralKeypair::generate().unwrap();
    let massive_prompt = DuckChatMessage {
        role: "user".to_string(),
        content: format!("{} CRITICAL_PROMPT_END", "X".repeat(10000)),
    };

    let payload = build_chat_payload("gpt-5.6-luna", vec![massive_prompt], &keypair, false, "conv-123");
    let total_chars: usize = payload.messages.iter().map(|m| m.content.len()).sum();
    assert!(total_chars <= 7500, "Truncated message must be <= 7500 chars");
    assert!(
        payload.messages[0].content.contains("CRITICAL_PROMPT_END"),
        "Ending prompt must be preserved after front-truncation"
    );
}
