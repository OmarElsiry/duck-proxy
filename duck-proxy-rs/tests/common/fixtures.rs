//! Hermetic test fixtures, constants, and payload generators for Duck.ai proxy E2E tests.

#![allow(dead_code)]

use base64::Engine;
use serde_json::json;

pub const MOCK_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36";

pub const MOCK_FE_VERSION: &str =
    "serp_20260827_190157_ET-5738d187a3dbca905a80324bd698765a27bf6e44";

pub const SAMPLE_1X1_PNG_B64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

pub const SAMPLE_1X1_PNG_BYTES: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0x64, 0xF8, 0xCF, 0x50,
    0x0F, 0x00, 0x03, 0x86, 0x01, 0x80, 0x5A, 0x34, 0x7D, 0x6B, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

pub const SAMPLE_V8_CHALLENGE_JS: &str = r#"
Promise.resolve({
  client_hashes: ["seed_hash_0_init", "seed_hash_1_init"],
  meta: { origin: "", stack: "", duration: 0 }
})
"#;

pub fn sample_v8_challenge_b64() -> String {
    base64::engine::general_purpose::STANDARD.encode(SAMPLE_V8_CHALLENGE_JS.trim())
}

pub fn sample_valid_signals_b64() -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let payload = json!({
        "start": now_ms,
        "events": [
            { "e": "onboarding_impression", "t": 10 },
            { "e": "action", "t": 35 },
            { "e": "startNewChat_free", "t": 50 }
        ],
        "end": 65
    });

    base64::engine::general_purpose::STANDARD.encode(payload.to_string())
}

pub fn create_sse_chunk(id: &str, content: &str, created: i64) -> String {
    let json_val = json!({
        "action": "success",
        "id": id,
        "created": created,
        "message": content
    });
    format!("data: {}\n\n", json_val)
}

pub fn create_sse_stream_body(chunks: &[&str]) -> String {
    let mut body = String::new();
    let base_ts = 1724867000i64;
    for (i, chunk) in chunks.iter().enumerate() {
        body.push_str(&create_sse_chunk(
            &format!("chatcmpl-chunk-{}", i),
            chunk,
            base_ts + i as i64,
        ));
    }
    body.push_str("data: [DONE]\n\n");
    body
}

pub fn create_sse_stream_with_control_frames(chunks: &[&str]) -> String {
    let mut body = String::new();
    body.push_str("data: [PING]\n\n");
    body.push_str("data: [CHAT_TITLE: Rust Proxy Integration Test]\n\n");
    body.push_str("data: [LIMIT: 100]\n\n");

    let base_ts = 1724867000i64;
    for (i, chunk) in chunks.iter().enumerate() {
        body.push_str(&create_sse_chunk(
            &format!("chatcmpl-chunk-{}", i),
            chunk,
            base_ts + i as i64,
        ));
        if i == 0 {
            body.push_str("data: [PING]\n\n");
        }
    }
    body.push_str("data: [LIMIT: 99]\n\n");
    body.push_str("data: [DONE]\n\n");
    body
}

pub fn create_image_sse_body(b64_image: &str) -> String {
    format!(
        "data: {{\"role\":\"generated-image\",\"result\":\"data:image/png;base64,{}\"}}\n\ndata: [DONE]\n\n",
        b64_image
    )
}

pub fn create_nested_image_sse_body(b64_image: &str) -> String {
    format!(
        "data: {{\"role\":\"assistant\",\"data\":{{\"b64Image\":\"{}\"}}}}\n\ndata: [DONE]\n\n",
        b64_image
    )
}

pub fn create_partial_image_sse_body(parts: &[&str]) -> String {
    let mut body = String::new();
    for part in parts {
        body.push_str(&format!(
            "data: {{\"role\":\"partial-image\",\"result\":\"{}\"}}\n\n",
            part
        ));
    }
    body.push_str("data: [DONE]\n\n");
    body
}
