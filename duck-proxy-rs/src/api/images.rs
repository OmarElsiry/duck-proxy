//! POST /v1/images/generations handler.

use axum::{extract::State, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::duck::{DuckChatMessage, IMAGE_GEN_CHAT_MODEL};
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

    let fallback_chain = vec![IMAGE_GEN_CHAT_MODEL.to_string(), "gpt-5.4-mini".to_string()];
    let (resp, _) = state.duck_client.send_chat_request_cascade(
        IMAGE_GEN_CHAT_MODEL,
        &messages,
        &fallback_chain,
        true,
    ).await?;
    let body = resp.text().await.map_err(|e| {
        AppError::bad_gateway(format!("Failed to read upstream image response: {}", e))
    })?;

    let mut partials: Vec<String> = Vec::new();
    let mut final_image: Option<String> = None;

    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        let data = if let Some(stripped) = line.strip_prefix("data: ") {
            stripped
        } else if let Some(stripped) = line.strip_prefix("data:") {
            stripped.trim_start()
        } else {
            continue;
        };

        if data == "[DONE]" {
            break;
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
            if let Some(err_action) = json.get("action").and_then(|v| v.as_str()) {
                if err_action == "error" {
                    let msg = json
                        .get("message")
                        .or_else(|| json.get("type"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown upstream error");
                    return Err(AppError::bad_gateway(format!("Upstream error: {}", msg)));
                }
            }

            if let Some(b64) = json.get("b64Image").and_then(|v| v.as_str()) {
                if !b64.is_empty() {
                    final_image = Some(b64.to_string());
                }
            } else if let Some(b64) = json
                .get("data")
                .and_then(|d| d.get("b64Image"))
                .and_then(|v| v.as_str())
            {
                if !b64.is_empty() {
                    final_image = Some(b64.to_string());
                }
            } else if let Some(role) = json.get("role").and_then(|v| v.as_str()) {
                if let Some(res) = json.get("result").and_then(|v| v.as_str()) {
                    if role == "generated-image" || role == "image" || role == "ui-component" {
                        final_image = Some(res.to_string());
                    } else if role == "partial-image" {
                        partials.push(res.to_string());
                    }
                }
            }
        }
    }

    let raw_b64 = if let Some(f) = final_image {
        f
    } else if !partials.is_empty() {
        partials.concat()
    } else {
        return Err(AppError::bad_gateway("No image data received from upstream"));
    };

    let clean_b64 = if raw_b64.starts_with("data:") && raw_b64.contains(',') {
        raw_b64.split_once(',').map(|(_, b)| b.to_string()).unwrap_or(raw_b64)
    } else {
        raw_b64
    };

    Ok(Json(ImageGenerationResponse {
        created: Utc::now().timestamp(),
        data: vec![ImageData {
            b64_json: clean_b64,
        }],
    }))
}
