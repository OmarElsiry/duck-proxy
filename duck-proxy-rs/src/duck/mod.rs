//! Core Duck.ai client and protocol engine.

pub mod client;
pub mod models;
pub mod payload;
pub mod stream;
pub mod types;

pub use client::DuckClient;
pub use models::{DuckModel, DUCK_MODELS, IMAGE_GEN_CHAT_MODEL};
pub use payload::build_chat_payload;
pub use stream::{parse_sse_line, SseEvent};
pub use types::*;
