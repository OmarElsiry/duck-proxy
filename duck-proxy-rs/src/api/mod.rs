//! OpenAI-compatible API route handlers.

pub mod chat;
pub mod dashboard;
pub mod images;
pub mod models;

use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

/// Assembles the API router with all OpenAI-compatible endpoints and web dashboard.
pub fn router() -> Router<AppState> {
    Router::new()
        // Web Dashboard (Uber Minimalist Command Center)
        .route("/", get(dashboard::dashboard_handler))
        .route("/app", get(dashboard::dashboard_handler))
        // OpenAI-Compatible v1 Endpoints
        .route("/v1/models", get(models::list_models))
        .route("/v1/chat/completions", post(chat::chat_completions))
        .route("/v1/responses", post(chat::chat_completions))
        .route("/v1/images/generations", post(images::generate_image))
}

