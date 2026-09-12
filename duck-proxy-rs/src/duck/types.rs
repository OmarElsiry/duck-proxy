//! Wire types for the Duck.ai chat protocol.

use serde::{Deserialize, Serialize};

/// A single chat message in the Duck.ai wire format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuckChatMessage {
    pub role: String,
    pub content: String,
}

/// Tool choice flags sent in chat metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolChoice {
    #[serde(rename = "NewsSearch")]
    pub news_search: bool,
    #[serde(rename = "VideosSearch")]
    pub videos_search: bool,
    #[serde(rename = "LocalSearch")]
    pub local_search: bool,
    #[serde(rename = "WeatherForecast")]
    pub weather_forecast: bool,
    #[serde(rename = "GenerateImage", skip_serializing_if = "Option::is_none")]
    pub generate_image: Option<bool>,
}

/// Metadata block in the chat request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMetadata {
    pub tool_choice: ToolChoice,
}

/// Durable stream info for the chat request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DurableStream {
    pub message_id: String,
    pub conversation_id: String,
    pub public_key: serde_json::Value,
}

/// Upstream Duck.ai wire value for reasoning mode ("low").
pub const REASONING_EFFORT_REASONING: &str = "low";

/// Upstream Duck.ai wire value for medium reasoning effort ("medium").
pub const REASONING_EFFORT_MEDIUM: &str = "medium";

/// Upstream Duck.ai wire value for fast / non-reasoning mode ("none").
pub const REASONING_EFFORT_FAST: &str = "none";

/// Client specifier for maximum reasoning effort.
pub const REASONING_EFFORT_MAX: &str = "max";

/// Returns the maximum allowable reasoning effort for a given Duck.ai model.
///
/// Upstream Duck.ai allows ["none", "low", "medium"] for gpt-5.4-mini, gpt-5.4, gpt-5.6-terra, gpt-5.6-sol,
/// and claude-opus-4-8; and allows ["none", "low"] for gpt-5.6-luna, claude-haiku-4-5, tinfoil/gemma4-31b, etc.
pub fn max_reasoning_effort_for_model(duck_model: &str) -> &'static str {
    match duck_model.to_lowercase().as_str() {
        "gpt-5.4-mini" | "gpt-5.4" | "gpt-5.6-terra" | "gpt-5.6-sol" | "claude-opus-4-8" => {
            REASONING_EFFORT_MEDIUM
        }
        _ => REASONING_EFFORT_REASONING,
    }
}

/// The full Duck.ai chat request wire payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuckChatRequest {
    pub model: String,
    pub metadata: ChatMetadata,
    pub messages: Vec<DuckChatMessage>,
    pub can_use_tools: bool,
    pub reasoning_effort: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_use_approx_location: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub can_delegate_image_generation: Option<bool>,
    pub durable_stream: DurableStream,
}

impl DuckChatRequest {
    /// Returns true if this request is configured in Duck.ai reasoning mode ("low" or "medium").
    pub fn is_reasoning_mode(&self) -> bool {
        self.reasoning_effort == REASONING_EFFORT_REASONING
            || self.reasoning_effort == REASONING_EFFORT_MEDIUM
    }
}

/// Frontend telemetry event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeEvent {
    pub name: String,
    pub delta: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trusted: Option<bool>,
}

/// Frontend telemetry signals payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeSignals {
    pub start: i64,
    pub events: Vec<FeEvent>,
    pub end: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_choice_serialization() {
        let tc = ToolChoice::default();
        let json = serde_json::to_value(&tc).unwrap();
        assert_eq!(json["NewsSearch"], false);
        assert_eq!(json["VideosSearch"], false);
        assert!(json.get("GenerateImage").is_none());
    }

    #[test]
    fn test_tool_choice_with_image_gen() {
        let tc = ToolChoice {
            generate_image: Some(true),
            ..Default::default()
        };
        let json = serde_json::to_value(&tc).unwrap();
        assert_eq!(json["GenerateImage"], true);
    }

    #[test]
    fn test_duck_chat_request_camel_case() {
        let req = DuckChatRequest {
            model: "gpt-5.6-luna".to_string(),
            metadata: ChatMetadata {
                tool_choice: ToolChoice::default(),
            },
            messages: vec![DuckChatMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
            }],
            can_use_tools: true,
            reasoning_effort: "none".to_string(),
            can_use_approx_location: Some(true),
            can_delegate_image_generation: None,
            durable_stream: DurableStream {
                message_id: "abc".to_string(),
                conversation_id: "def".to_string(),
                public_key: serde_json::json!({}),
            },
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("canUseTools").is_some());
        assert!(json.get("reasoningEffort").is_some());
        assert!(json.get("durableStream").is_some());
        assert!(json.get("canUseApproxLocation").is_some());
    }

    #[test]
    fn test_fe_signals_serialization() {
        let signals = FeSignals {
            start: 1000,
            events: vec![
                FeEvent { name: "action".to_string(), delta: 500, trusted: Some(true) },
                FeEvent { name: "view".to_string(), delta: 100, trusted: None },
            ],
            end: 1500,
        };
        let json = serde_json::to_value(&signals).unwrap();
        assert_eq!(json["start"], 1000);
        assert_eq!(json["events"][0]["trusted"], true);
        // "trusted" should be absent for second event
        assert!(json["events"][1].get("trusted").is_none());
    }
}
