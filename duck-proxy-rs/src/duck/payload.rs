//! Chat payload construction for the Duck.ai wire protocol.

use crate::crypto::EphemeralKeypair;
use super::types::*;

/// Builds a complete Duck.ai chat request payload.
pub fn build_chat_payload(
    duck_model: &str,
    messages: Vec<DuckChatMessage>,
    keypair: &EphemeralKeypair,
    is_image_gen: bool,
) -> DuckChatRequest {
    let message_id = uuid::Uuid::new_v4().to_string();
    let conversation_id = uuid::Uuid::new_v4().to_string();

    let public_key = serde_json::to_value(keypair.public_jwk())
        .unwrap_or(serde_json::json!({}));

    let tool_choice = if is_image_gen {
        ToolChoice {
            generate_image: Some(true),
            ..Default::default()
        }
    } else {
        ToolChoice::default()
    };

    DuckChatRequest {
        model: duck_model.to_string(),
        metadata: ChatMetadata {
            can_use_web_search: true,
            tool_choice,
        },
        messages,
        can_use_tools: true,
        reasoning_effort: "none".to_string(),
        can_use_approx_location: None,
        can_delegate_image_generation: if is_image_gen { Some(true) } else { None },
        can_use_web_search: true,
        can_upload_files: None,
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

        let payload = build_chat_payload("gpt-5.6-luna", messages, &keypair, false);

        assert_eq!(payload.model, "gpt-5.6-luna");
        assert!(payload.metadata.can_use_web_search);
        assert!(payload.metadata.tool_choice.generate_image.is_none());
        assert!(payload.can_delegate_image_generation.is_none());
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

        let payload = build_chat_payload("gpt-5.6-luna", messages, &keypair, true);

        assert_eq!(payload.metadata.tool_choice.generate_image, Some(true));
        assert_eq!(payload.can_delegate_image_generation, Some(true));
    }

    #[test]
    fn test_payload_has_unique_ids() {
        let keypair = EphemeralKeypair::generate().unwrap();
        let msg = vec![DuckChatMessage { role: "user".to_string(), content: "test".to_string() }];

        let p1 = build_chat_payload("gpt-5.6-luna", msg.clone(), &keypair, false);
        let p2 = build_chat_payload("gpt-5.6-luna", msg, &keypair, false);

        assert_ne!(p1.durable_stream.message_id, p2.durable_stream.message_id);
        assert_ne!(p1.durable_stream.conversation_id, p2.durable_stream.conversation_id);
    }

    #[test]
    fn test_payload_jwk_serialization() {
        let keypair = EphemeralKeypair::generate().unwrap();
        let msg = vec![DuckChatMessage { role: "user".to_string(), content: "test".to_string() }];
        let payload = build_chat_payload("gpt-5.6-luna", msg, &keypair, false);

        let pk = &payload.durable_stream.public_key;
        assert_eq!(pk["alg"], "RSA-OAEP-256");
        assert_eq!(pk["kty"], "RSA");
        assert_eq!(pk["use"], "enc");
    }
}
