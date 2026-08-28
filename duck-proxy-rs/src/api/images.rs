//! POST /v1/images/generations handler.

use axum::{extract::State, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::duck::{build_chat_payload, parse_sse_line, DuckChatMessage, SseEvent, IMAGE_GEN_CHAT_MODEL};
use crate::error::AppError;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ImageGenerationRequest {
    pub prompt: String,
    #[serde(default = "default_image_model")]
    pub model: String,
}

fn default_image_model() -> String {
    "image".to_string()
}

#[derive(Debug, Serialize)]
pub struct ImageGenerationResponse {
    pub created: i64,
    pub data: Vec<ImageData>,
}

#[derive(Debug, Serialize)]
pub struct ImageData {
    pub b64_json: String,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// Handler for POST /v1/images/generations.
pub async fn generate_image(
    State(state): State<AppState>,
    req: Result<Json<ImageGenerationRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<ImageGenerationResponse>, AppError> {
    let Json(req) = req.map_err(|e| {
        AppError::bad_request(format!("Invalid JSON payload: {}", e))
    })?;

    if req.prompt.is_empty() {
        return Err(AppError::bad_request_with_param(
            "Prompt cannot be empty",
            "prompt",
            "missing_required_parameter",
        ));
    }

    let messages = vec![DuckChatMessage {
        role: "user".to_string(),
        content: req.prompt,
    }];

    let payload = build_chat_payload(
        IMAGE_GEN_CHAT_MODEL,
        messages,
        state.duck_client.keypair(),
        true,
    );

    let resp = state.duck_client.send_chat_request(&payload).await?;
    let body = resp.text().await.map_err(|e| {
        AppError::bad_gateway(format!("Failed to read upstream image response: {}", e))
    })?;

    let mut accumulated_b64 = String::new();
    for line in body.lines() {
        if let Some(event) = parse_sse_line(line) {
            match event {
                SseEvent::ImageData(chunk) => {
                    let clean = if chunk.starts_with("data:image/") {
                        if let Some((_, b64_part)) = chunk.split_once(',') {
                            b64_part
                        } else {
                            &chunk
                        }
                    } else {
                        &chunk
                    };
                    accumulated_b64.push_str(clean);
                }
                SseEvent::Done => break,
                SseEvent::Error(e) => {
                    return Err(AppError::bad_gateway(format!("Upstream error: {}", e)));
                }
                _ => {}
            }
        }
    }

    if !accumulated_b64.is_empty() {
        Ok(Json(ImageGenerationResponse {
            created: Utc::now().timestamp(),
            data: vec![ImageData {
                b64_json: accumulated_b64,
            }],
        }))
    } else {
        Err(AppError::bad_gateway("No image data received from upstream"))
    }
}
