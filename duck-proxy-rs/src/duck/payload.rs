//! Chat payload construction for the Duck.ai wire protocol.

use crate::crypto::EphemeralKeypair;
use super::types::*;

/// Builds a complete Duck.ai chat request payload with all permissions enabled.
pub fn build_chat_payload(
    duck_model: &str,
    messages: Vec<DuckChatMessage>,
    keypair: &EphemeralKeypair,
    is_image_gen: bool,
    conversation_id: &str,
) -> DuckChatRequest {
    let message_id = uuid::Uuid::new_v4().to_string();
    let conversation_id = conversation_id.to_string();

    let public_key = serde_json::to_value(keypair.public_jwk())
        .unwrap_or(serde_json::json!({}));

    let tool_choice = if is_image_gen {
        ToolChoice {
            news_search: false,
            videos_search: false,
            local_search: false,
            weather_forecast: false,
            generate_image: Some(true),
        }
    } else {
        ToolChoice {
            news_search: false,
            videos_search: false,
            local_search: false,
            weather_forecast: false,
            generate_image: None,
        }
    };

    let safe_messages = sanitize_and_fit_messages(messages);

    DuckChatRequest {
        model: duck_model.to_string(),
        metadata: ChatMetadata {
            tool_choice,
        },
        messages: safe_messages,
        can_use_tools: true,
        reasoning_effort: "none".to_string(),
        can_use_approx_location: None,
        can_delegate_image_generation: if is_image_gen { Some(true) } else { None },
        durable_stream: DurableStream {
            message_id,
            conversation_id,
            public_key,
        },
    }
}

/// Maximum total character budget for Duck.ai payloads to prevent ERR_CONVERSATION_LIMIT.
pub const MAX_TOTAL_CHARS: usize = 7500;

/// Sanitizes and smartly trims conversation turns to fit within Duck.ai's context window.
/// Preserves critical system instructions/environment directives at the head and latest user prompt at the tail.
pub fn sanitize_and_fit_messages(messages: Vec<DuckChatMessage>) -> Vec<DuckChatMessage> {
    if messages.is_empty() {
        return messages;
    }

    let mut msgs = messages;
    let total_chars: usize = msgs.iter().map(|m| m.content.len()).sum();
    if total_chars <= MAX_TOTAL_CHARS {
        return msgs;
    }

    // If there is only 1 message, smart truncate the middle if necessary
    if msgs.len() == 1 {
        let content = &msgs[0].content;
        if content.len() > MAX_TOTAL_CHARS {
            let head_len = 2500.min(content.len());
            let tail_len = 4800.min(content.len().saturating_sub(head_len));
            let head_idx = content.char_indices().nth(head_len).map(|(i, _)| i).unwrap_or(head_len);
            let head = &content[..head_idx];
            let tail_start = content.len() - tail_len;
            let tail_idx = content.char_indices().find(|(i, _)| *i >= tail_start).map(|(i, _)| i).unwrap_or(tail_start);
            let tail = &content[tail_idx..];
            msgs[0].content = format!("{}\n\n...[content trimmed to fit context limit]...\n\n{}", head, tail);
        }
        return msgs;
    }

    // When there are multiple messages:
    // msgs[0] has system directives, msgs[last] has the current request.
    // Drop intermediate turns (msgs[1..len-1]) from oldest to newest first.
    while msgs.len() > 2 {
        let current_total: usize = msgs.iter().map(|m| m.content.len()).sum();
        if current_total <= MAX_TOTAL_CHARS {
            break;
        }
        msgs.remove(1);
    }

    // If still over limit with 2 messages (system message + user message)
    let current_total: usize = msgs.iter().map(|m| m.content.len()).sum();
    if current_total > MAX_TOTAL_CHARS && msgs.len() >= 2 {
        let head_budget = 2500;
        if msgs[0].content.len() > head_budget {
            let h = &msgs[0].content;
            let idx = h.char_indices().nth(head_budget).map(|(i, _)| i).unwrap_or(head_budget);
            msgs[0].content = format!("{}\n\n...[instructions truncated]...", &h[..idx]);
        }
        let remaining_budget = MAX_TOTAL_CHARS.saturating_sub(msgs[0].content.len()).max(1000);
        let last_idx = msgs.len() - 1;
        if msgs[last_idx].content.len() > remaining_budget {
            let tail_content = &msgs[last_idx].content;
            let excess = tail_content.len().saturating_sub(remaining_budget);
            let start_idx = tail_content.char_indices().find(|(i, _)| *i >= excess).map(|(i, _)| i).unwrap_or(excess);
            msgs[last_idx].content = tail_content[start_idx..].to_string();
        }
    }

    msgs
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_chat_payload_basic() {
        let keypair = EphemeralKeypair::generate().unwrap();
        let messages = vec![DuckChatMessage {
            role: "user".to_string(),
            content: "hello".to_string(),
        }];

        let payload = build_chat_payload("gpt-5.6-luna", messages, &keypair, false, "conv-1");

        assert_eq!(payload.model, "gpt-5.6-luna");
        assert_eq!(payload.durable_stream.conversation_id, "conv-1");
        assert!(payload.metadata.tool_choice.generate_image.is_none());
        assert_eq!(payload.can_delegate_image_generation, None);
        assert_eq!(payload.can_use_approx_location, None);
        assert_eq!(payload.messages.len(), 1);
        assert_eq!(payload.reasoning_effort, "none");
    }

    #[test]
    fn test_build_chat_payload_image_gen() {
        let keypair = EphemeralKeypair::generate().unwrap();
        let messages = vec![DuckChatMessage {
            role: "user".to_string(),
            content: "draw a cat".to_string(),
        }];

        let payload = build_chat_payload("gpt-5.6-luna", messages, &keypair, true, "conv-2");

        assert_eq!(payload.metadata.tool_choice.generate_image, Some(true));
        assert_eq!(payload.can_delegate_image_generation, Some(true));
        assert_eq!(payload.can_use_approx_location, None);
    }

    #[test]
    fn test_payload_has_unique_ids() {
        let keypair = EphemeralKeypair::generate().unwrap();
        let msg = vec![DuckChatMessage { role: "user".to_string(), content: "test".to_string() }];

        let p1 = build_chat_payload("gpt-5.6-luna", msg.clone(), &keypair, false, "conv-3");
        let p2 = build_chat_payload("gpt-5.6-luna", msg, &keypair, false, "conv-4");

        assert_ne!(p1.durable_stream.message_id, p2.durable_stream.message_id);
        assert_ne!(p1.durable_stream.conversation_id, p2.durable_stream.conversation_id);
    }

    #[test]
    fn test_payload_jwk_serialization() {
        let keypair = EphemeralKeypair::generate().unwrap();
        let msg = vec![DuckChatMessage { role: "user".to_string(), content: "test".to_string() }];
        let payload = build_chat_payload("gpt-5.6-luna", msg, &keypair, false, "conv-5");

        let pk = &payload.durable_stream.public_key;
        assert_eq!(pk["alg"], "RSA-OAEP-256");
        assert_eq!(pk["kty"], "RSA");
        assert_eq!(pk["use"], "enc");
    }
}
