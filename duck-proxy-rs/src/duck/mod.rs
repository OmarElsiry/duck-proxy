//! Core Duck.ai client and protocol engine.

pub mod client;
pub mod fallback_image;
pub mod models;
pub mod payload;
pub mod stream;
pub mod types;
pub mod virtual_user;

pub use client::DuckClient;
pub use fallback_image::{generate_procedural_image_base64, generate_raster_image_base64};
pub use models::{DuckModel, DUCK_MODELS, IMAGE_GEN_CHAT_MODEL};
pub use payload::{assert_reasoning_mode, build_chat_payload, guard_reasoning_mode};
pub use stream::{parse_sse_line, SseEvent};
pub use types::*;
pub use virtual_user::{VirtualUser, VirtualUserPool, VirtualUserSnapshot, USER_AGENTS, USER_AGENT};


