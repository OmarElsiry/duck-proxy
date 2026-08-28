//! OpenAI-compatible API route handlers.

pub mod chat;
pub mod images;
pub mod models;

use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

/// Assembles the API router with all OpenAI-compatible endpoints.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/models", get(models::list_models))
        .route("/v1/chat/completions", post(chat::chat_completions))
        .route("/v1/images/generations", post(images::generate_image))
}
