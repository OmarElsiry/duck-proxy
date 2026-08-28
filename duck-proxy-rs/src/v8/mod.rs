//! V8 JavaScript runtime and browser DOM challenge solver.

pub mod actor;
pub mod stubs;

pub use actor::{spawn_v8_actor, ua_sha256_hex, V8ActorHandle};
pub use stubs::{
    extract_html_lookup, generate_browser_stubs, wrap_challenge_code, HtmlLookupEntry,
    BROWSER_STUBS_TEMPLATE,
};
