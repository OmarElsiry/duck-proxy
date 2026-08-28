//! POST /v1/chat/completions handler — streaming and non-streaming.

use axum::{
    extract::State,
    response::{
        sse::{Event, Sse},
        IntoResponse, Response,
    },
    Json,
};
use chrono::Utc;
use futures::stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

use crate::duck::{build_chat_payload, parse_sse_line, DuckChatMessage, SseEvent};
use crate::error::AppError;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPart {
    #[serde(rename = "type")]
    pub part_type: Option<String>,
    pub text: Option<String>,
}

impl MessageContent {
    pub fn to_text(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Parts(parts) => {
                parts
                    .iter()
                    .filter_map(|p| p.text.as_deref())
                    .collect::<Vec<_>>()
                    .join("")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Non-streaming response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Debug, Serialize)]
pub struct Choice {
    pub index: u32,
    pub message: ResponseMessage,
    pub finish_reason: String,
}

#[derive(Debug, Serialize)]
pub struct ResponseMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// ---------------------------------------------------------------------------
// Streaming chunk types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
}

#[derive(Debug, Serialize)]
pub struct ChunkChoice {
    pub index: u32,
    pub delta: Delta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// Handler for POST /v1/chat/completions.
pub async fn chat_completions(
    State(state): State<AppState>,
    req: Result<Json<ChatCompletionRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Response, AppError> {
    let Json(req) = req.map_err(|e| {
        AppError::bad_request(format!("Invalid JSON payload: {}", e))
    })?;

    if req.messages.is_empty() {
        return Err(AppError::bad_request_with_param(
            "Messages array cannot be empty",
            "messages",
            "missing_required_parameter",
        ));
    }

    // Resolve model
    let duck_model = state
        .config
        .resolve_duck_model(&req.model)
        .ok_or_else(|| {
            AppError::bad_request_with_param(
                format!("The model '{}' does not exist or is not supported.", req.model),
                "model",
                "model_not_found",
            )
        })?
        .to_string();

    // Convert messages
    let messages: Vec<DuckChatMessage> = req
        .messages
        .iter()
        .map(|m| DuckChatMessage {
            role: m.role.clone(),
            content: m.content.to_text(),
        })
        .collect();

    // Build payload
    let payload = build_chat_payload(
        &duck_model,
        messages,
        state.duck_client.keypair(),
        false,
    );

    // Send to Duck.ai
    let resp = state.duck_client.send_chat_request(&payload).await?;
    let completion_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = Utc::now().timestamp();

    if req.stream {
        handle_streaming(resp, completion_id, created, duck_model).await
    } else {
        handle_non_streaming(resp, completion_id, created, duck_model).await
    }
}

/// Collects the full response and returns an OpenAI chat.completion object.
async fn handle_non_streaming(
    resp: reqwest::Response,
    completion_id: String,
    created: i64,
    model: String,
) -> Result<Response, AppError> {
    let body = resp.text().await.map_err(|e| {
        AppError::bad_gateway(format!("Failed to read upstream response: {}", e))
    })?;

    let mut accumulated = String::new();
    for line in body.lines() {
        if let Some(event) = parse_sse_line(line) {
            match event {
                SseEvent::Token(t) => accumulated.push_str(&t),
                SseEvent::Done => break,
                SseEvent::Error(e) => {
                    return Err(AppError::bad_gateway(format!("Upstream error: {}", e)));
                }
                SseEvent::ImageData(_) => {} // Ignore images in chat endpoint
            }
        }
    }

    if accumulated.is_empty() {
        return Err(AppError::bad_gateway("Empty response from upstream"));
    }

    let response = ChatCompletionResponse {
        id: completion_id,
        object: "chat.completion".to_string(),
        created,
        model,
        choices: vec![Choice {
            index: 0,
            message: ResponseMessage {
                role: "assistant".to_string(),
                content: accumulated,
            },
            finish_reason: "stop".to_string(),
        }],
        usage: Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        },
    };

    Ok(Json(response).into_response())
}

/// Streams SSE chunks as they arrive from Duck.ai.
async fn handle_streaming(
    resp: reqwest::Response,
    completion_id: String,
    created: i64,
    model: String,
) -> Result<Response, AppError> {
    let body = resp.text().await.map_err(|e| {
        AppError::bad_gateway(format!("Failed to read upstream response: {}", e))
    })?;

    let mut events: Vec<Result<Event, Infallible>> = Vec::new();

    for line in body.lines() {
        if let Some(sse_event) = parse_sse_line(line) {
            match sse_event {
                SseEvent::Token(token) => {
                    let chunk = ChatCompletionChunk {
                        id: completion_id.clone(),
                        object: "chat.completion.chunk".to_string(),
                        created,
                        model: model.clone(),
                        choices: vec![ChunkChoice {
                            index: 0,
                            delta: Delta {
                                role: None,
                                content: Some(token),
                            },
                            finish_reason: None,
                        }],
                    };
                    let json = serde_json::to_string(&chunk).unwrap_or_default();
                    events.push(Ok(Event::default().data(json)));
                }
                SseEvent::Done => {
                    events.push(Ok(Event::default().data("[DONE]")));
                    break;
                }
                SseEvent::Error(_) | SseEvent::ImageData(_) => {}
            }
        }
    }

    let sse_stream = stream::iter(events);
    Ok(Sse::new(sse_stream).into_response())
}
