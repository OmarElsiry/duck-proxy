//! GET /v1/models handler — returns OpenAI-compatible model list.

use axum::{extract::State, Json};
use chrono::Utc;
use serde::Serialize;

use crate::state::AppState;

/// Reasoning effort preset expected by Codex CLI.
#[derive(Debug, Serialize, Clone)]
pub struct ReasoningEffortPreset {
    pub effort: String,
    pub description: String,
}

/// A single model object in the OpenAI response format.
#[derive(Debug, Serialize, Clone)]
pub struct ModelObject {
    pub id: String,
    pub slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
    pub supported_reasoning_levels: Vec<ReasoningEffortPreset>,
}

/// The response for GET /v1/models.
#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    pub object: String,
    pub data: Vec<ModelObject>,
    pub models: Vec<ModelObject>,
}

/// Handler for GET /v1/models.
pub async fn list_models(State(state): State<AppState>) -> Json<ModelsResponse> {
    let now = Utc::now().timestamp();
    let levels = vec![
        ReasoningEffortPreset {
            effort: "low".to_string(),
            description: "Fast responses".to_string(),
        },
        ReasoningEffortPreset {
            effort: "medium".to_string(),
            description: "Balanced reasoning".to_string(),
        },
        ReasoningEffortPreset {
            effort: "high".to_string(),
            description: "Deep reasoning".to_string(),
        },
    ];
    let data: Vec<ModelObject> = state
        .config
        .model_list
        .iter()
        .map(|m| ModelObject {
            id: m.model_name.clone(),
            slug: m.model_name.clone(),
            display_name: Some(m.model_name.clone()),
            object: "model".to_string(),
            created: now,
            owned_by: "duck".to_string(),
            supported_reasoning_levels: levels.clone(),
        })
        .collect();

    Json(ModelsResponse {
        object: "list".to_string(),
        models: data.clone(),
        data,
    })
}
