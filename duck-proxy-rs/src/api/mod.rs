//! OpenAI-compatible API route handlers.

pub mod audio;
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
        .route("/v1/audio/transcriptions", post(audio::handle_transcription))
        .route("/v1/audio/translations", post(audio::handle_translation))
        // Local Image Serving for in-chat inline media rendering
        .route("/image/:filename", get(chat::serve_image))
        .route("/image", get(chat::serve_image_query))
}

