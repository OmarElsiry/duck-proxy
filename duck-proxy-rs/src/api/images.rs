//! POST /v1/images/generations handler.

use axum::{extract::State, Json};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::duck::IMAGE_GEN_CHAT_MODEL;
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

    let raw_b64 = tokio::time::timeout(
        std::time::Duration::from_secs(115),
        crate::api::chat::fetch_single_duck_image(
            &state,
            IMAGE_GEN_CHAT_MODEL,
            &req.prompt,
            &[IMAGE_GEN_CHAT_MODEL.to_string()],
        ),
    )
    .await
    .map_err(|_| AppError::bad_gateway("gpt-image-2 generation timed out (>115s)"))?
    .map_err(|e| AppError::bad_gateway(format!("gpt-image-2 generation failed: {}", e)))?;

    let clean_b64 = crate::duck::stream::extract_clean_b64(&raw_b64);

    let filename = crate::api::chat::derive_image_filename(&req.prompt);
    crate::api::chat::save_last_generated_image(&clean_b64);
    if let Some(bytes) = crate::duck::stream::decode_image_base64(&clean_b64) {
        crate::api::chat::save_image_bytes(&filename, &bytes);
        let _ = std::fs::write(format!("/tmp/{}", filename), &bytes);
        let _ = std::fs::write(format!("/home/potterparker/Desktop/Projects/Learnopia/{}", filename), &bytes);
    }

    Ok(Json(ImageGenerationResponse {
        created: Utc::now().timestamp(),
        data: vec![ImageData {
            b64_json: clean_b64,
        }],
    }))
}
