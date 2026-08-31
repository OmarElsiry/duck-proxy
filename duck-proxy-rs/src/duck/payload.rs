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

    let mut safe_messages = messages;
    // Duck.ai free tier rejects payloads exceeding 8,000 characters with ERR_CONVERSATION_LIMIT
    const MAX_TOTAL_CHARS: usize = 7500;
    let mut total_chars: usize = safe_messages.iter().map(|m| m.content.len()).sum();
    if total_chars > MAX_TOTAL_CHARS {
        // Drop older history messages first
        while safe_messages.len() > 1 && total_chars > MAX_TOTAL_CHARS {
            let removed = safe_messages.remove(0);
            total_chars = total_chars.saturating_sub(removed.content.len());
        }
        // If single remaining message still exceeds limit, truncate from the beginning to preserve the prompt at the end
        if total_chars > MAX_TOTAL_CHARS && !safe_messages.is_empty() {
            let excess = total_chars - MAX_TOTAL_CHARS;
            let current = &safe_messages[0].content;
            if current.len() > excess {
                let start_idx = current.char_indices().nth(excess).map(|(i, _)| i).unwrap_or(excess);
                safe_messages[0].content = current[start_idx..].to_string();
            }
        }
    }

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
