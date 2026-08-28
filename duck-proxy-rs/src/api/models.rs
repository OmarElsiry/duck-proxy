//! GET /v1/models handler — returns OpenAI-compatible model list.

use axum::{extract::State, Json};
use chrono::Utc;
use serde::Serialize;

use crate::state::AppState;

/// A single model object in the OpenAI response format.
#[derive(Debug, Serialize)]
pub struct ModelObject {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

/// The response for GET /v1/models.
#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    pub object: String,
    pub data: Vec<ModelObject>,
}

/// Handler for GET /v1/models.
pub async fn list_models(State(state): State<AppState>) -> Json<ModelsResponse> {
    let now = Utc::now().timestamp();
    let data = state
        .config
        .model_list
        .iter()
        .map(|m| ModelObject {
            id: m.model_name.clone(),
            object: "model".to_string(),
            created: now,
            owned_by: "duck".to_string(),
        })
        .collect();

    Json(ModelsResponse {
        object: "list".to_string(),
        data,
    })
}
