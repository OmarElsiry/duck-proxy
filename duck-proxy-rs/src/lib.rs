//! Duck.ai OpenAI-compatible proxy library.

pub mod api;
pub mod config;
pub mod crypto;
pub mod duck;
pub mod error;
pub mod state;
pub mod v8;

pub use config::{Config, ConfigError, ModelMapping, ServerConfig};
pub use crypto::{CryptoError, EphemeralKeypair, JwkPublicKey};
pub use error::{AppError, ErrorBody, ErrorDetail, ErrorResponse, OpenAiErrorDetail, OpenAiErrorResponse};
pub use state::AppState;

/// Creates the Axum application router with shared state.
pub fn create_app(config: Config) -> axum::Router {
    let state = AppState::new(config);
    create_app_with_state(state)
}

/// Creates the Axum application router with a pre-configured AppState.
pub fn create_app_with_state(state: AppState) -> axum::Router {
    api::router().with_state(state)
}
