//! Shared application state.

use std::sync::Arc;
use crate::config::Config;
use crate::duck::client::DuckClient;

/// Application state shared across all handlers via Axum's State extractor.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub duck_client: Arc<DuckClient>,
}

impl AppState {
    /// Creates a new AppState from the given configuration.
    pub fn new(config: Config) -> Self {
        let duck_client = DuckClient::with_pool_size(&config.upstream_base_url, config.virtual_users_count);
        Self {
            config: Arc::new(config),
            duck_client: Arc::new(duck_client),
        }
    }
}

