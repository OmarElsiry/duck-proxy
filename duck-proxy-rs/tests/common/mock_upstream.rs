//! Hermetic WireMock Upstream Duck.ai Protocol Simulator.

#![allow(dead_code)]

use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::fixtures::*;
use super::matchers::{DuckHeadersMatcher, DuckPayloadMatcher, SolvedV8ChallengeMatcher};

/// Hermetic Duck.ai Upstream Server mock wrapping a WireMock server instance.
pub struct MockDuckServer {
    pub server: MockServer,
}

impl MockDuckServer {
    /// Start a new mock server on local loopback (127.0.0.1:0).
    pub async fn start() -> Self {
        let server = MockServer::start().await;
        Self { server }
    }

    /// Returns base URL of mock server (e.g. `http://127.0.0.1:41235`).
    pub fn uri(&self) -> String {
        self.server.uri()
    }

    /// Preset: Standard Status endpoint returning 200 OK and initial VQD token.
    pub async fn mock_status_ok(&self, vqd_token: &str) {
        Mock::given(method("GET"))
            .and(path("/duckchat/v1/status"))
            .and(header("x-vqd-accept", "1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("x-vqd-hash-1", vqd_token)
                    .set_body_string("ok"),
            )
            .mount(&self.server)
            .await;
    }

    /// Preset: Status endpoint returning a base64 JavaScript challenge.
    pub async fn mock_status_challenge(&self, challenge_b64: &str) {
        Mock::given(method("GET"))
            .and(path("/duckchat/v1/status"))
            .and(header("x-vqd-accept", "1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("x-vqd-hash-1", challenge_b64)
                    .set_body_string("ok"),
            )
            .mount(&self.server)
            .await;
    }

    /// Preset: Status endpoint failing with 429 for `fail_count` times before succeeding.
    pub async fn mock_status_429_then_ok(&self, fail_count: u64, success_vqd: &str) {
        Mock::given(method("GET"))
            .and(path("/duckchat/v1/status"))
            .respond_with(ResponseTemplate::new(429).set_body_string("ERR_RATE_LIMIT"))
            .up_to_n_times(fail_count)
            .mount(&self.server)
            .await;

        Mock::given(method("GET"))
            .and(path("/duckchat/v1/status"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("x-vqd-hash-1", success_vqd)
                    .set_body_string("ok"),
            )
            .mount(&self.server)
            .await;
    }

    /// Preset: Status endpoint always returning 429.
    pub async fn mock_status_429_always(&self) {
        Mock::given(method("GET"))
            .and(path("/duckchat/v1/status"))
            .respond_with(ResponseTemplate::new(429).set_body_string("rate limit exceeded"))
            .mount(&self.server)
            .await;
    }

    /// Preset: Status endpoint returning an HTTP error.
    pub async fn mock_status_error(&self, status_code: u16) {
        Mock::given(method("GET"))
            .and(path("/duckchat/v1/status"))
            .respond_with(ResponseTemplate::new(status_code).set_body_string("status error"))
            .mount(&self.server)
            .await;
    }

    /// Preset: Chat completion returning standard SSE stream chunks and next VQD token.
    pub async fn mock_chat_ok(&self, model: &str, chunks: &[&str], next_vqd: &str) {
        let sse_body = create_sse_stream_body(chunks);

        Mock::given(method("POST"))
            .and(path("/duckchat/v1/chat"))
            .and(DuckHeadersMatcher::new())
            .and(DuckPayloadMatcher::for_model(model))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream; charset=utf-8")
                    .insert_header("x-vqd-hash-1", next_vqd)
                    .set_body_string(sse_body),
            )
            .up_to_n_times(1)
            .mount(&self.server)
            .await;
    }

    /// Preset: Chat completion for any model returning standard SSE stream chunks.
    pub async fn mock_chat_any_model(&self, chunks: &[&str], next_vqd: &str) {
        let sse_body = create_sse_stream_body(chunks);

        Mock::given(method("POST"))
            .and(path("/duckchat/v1/chat"))
            .and(DuckHeadersMatcher::new())
            .and(DuckPayloadMatcher::any_model())
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream; charset=utf-8")
                    .insert_header("x-vqd-hash-1", next_vqd)
                    .set_body_string(sse_body),
            )
            .mount(&self.server)
            .await;
    }

    /// Preset: Chat completion with injected control frames (`[PING]`, `[LIMIT]`, `[CHAT_TITLE]`).
    pub async fn mock_chat_with_control_frames(
        &self,
        model: &str,
        chunks: &[&str],
        next_vqd: &str,
    ) {
        let sse_body = create_sse_stream_with_control_frames(chunks);

        Mock::given(method("POST"))
            .and(path("/duckchat/v1/chat"))
            .and(DuckHeadersMatcher::new())
            .and(DuckPayloadMatcher::for_model(model))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream; charset=utf-8")
                    .insert_header("x-vqd-hash-1", next_vqd)
                    .set_body_string(sse_body),
            )
            .mount(&self.server)
            .await;
    }

    /// Preset: Chat completion returning generated image in standard role format.
    pub async fn mock_chat_image(&self, b64_image: &str, next_vqd: &str) {
        let sse_body = create_image_sse_body(b64_image);

        Mock::given(method("POST"))
            .and(path("/duckchat/v1/chat"))
            .and(DuckHeadersMatcher::new())
            .and(DuckPayloadMatcher::for_image())
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream; charset=utf-8")
                    .insert_header("x-vqd-hash-1", next_vqd)
                    .set_body_string(sse_body),
            )
            .mount(&self.server)
            .await;
    }

    /// Preset: Chat completion returning nested data.b64Image.
    pub async fn mock_chat_nested_image(&self, b64_image: &str, next_vqd: &str) {
        let sse_body = create_nested_image_sse_body(b64_image);

        Mock::given(method("POST"))
            .and(path("/duckchat/v1/chat"))
            .and(DuckHeadersMatcher::new())
            .and(DuckPayloadMatcher::for_image())
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream; charset=utf-8")
                    .insert_header("x-vqd-hash-1", next_vqd)
                    .set_body_string(sse_body),
            )
            .mount(&self.server)
            .await;
    }

    /// Preset: Chat completion returning partial image chunks.
    pub async fn mock_chat_partial_images(&self, parts: &[&str], next_vqd: &str) {
        let sse_body = create_partial_image_sse_body(parts);

        Mock::given(method("POST"))
            .and(path("/duckchat/v1/chat"))
            .and(DuckHeadersMatcher::new())
            .and(DuckPayloadMatcher::for_image())
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream; charset=utf-8")
                    .insert_header("x-vqd-hash-1", next_vqd)
                    .set_body_string(sse_body),
            )
            .mount(&self.server)
            .await;
    }

    /// Preset: Specific multi-turn chat expectation matching incoming VQD token.
    pub async fn mock_chat_turn(&self, match_vqd: &str, return_vqd: &str, response_text: &str) {
        let sse_body = create_sse_stream_body(&[response_text]);

        Mock::given(method("POST"))
            .and(path("/duckchat/v1/chat"))
            .and(DuckHeadersMatcher::with_vqd(match_vqd))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream; charset=utf-8")
                    .insert_header("x-vqd-hash-1", return_vqd)
                    .set_body_string(sse_body),
            )
            .mount(&self.server)
            .await;
    }

    /// Preset: Chat completion returning HTTP error status.
    pub async fn mock_chat_error(&self, status_code: u16, error_body: &str) {
        Mock::given(method("POST"))
            .and(path("/duckchat/v1/chat"))
            .respond_with(ResponseTemplate::new(status_code).set_body_string(error_body))
            .mount(&self.server)
            .await;
    }

    /// Preset: Chat completion returning HTTP error status for a specific model.
    pub async fn mock_chat_error_for_model(&self, model: &str, status_code: u16, error_body: &str) {
        Mock::given(method("POST"))
            .and(path("/duckchat/v1/chat"))
            .and(DuckPayloadMatcher::for_model(model))
            .respond_with(ResponseTemplate::new(status_code).set_body_string(error_body))
            .mount(&self.server)
            .await;
    }

    /// Preset: Chat completion returning SSE error frame.
    pub async fn mock_chat_sse_error(&self, status: u16, error_type: &str) {
        let sse_body = format!(
            "data: {{\"action\":\"error\",\"status\":{},\"type\":\"{}\"}}\n\n",
            status, error_type
        );

        Mock::given(method("POST"))
            .and(path("/duckchat/v1/chat"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream; charset=utf-8")
                    .set_body_string(sse_body),
            )
            .mount(&self.server)
            .await;
    }

    /// Preset: Truncated stream ending abruptly without [DONE].
    pub async fn mock_chat_truncated_stream(&self, chunks: &[&str], next_vqd: &str) {
        let mut sse_body = String::new();
        let base_ts = 1724867000i64;
        for (i, chunk) in chunks.iter().enumerate() {
            sse_body.push_str(&create_sse_chunk(
                &format!("chatcmpl-chunk-{}", i),
                chunk,
                base_ts + i as i64,
            ));
        }

        Mock::given(method("POST"))
            .and(path("/duckchat/v1/chat"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream; charset=utf-8")
                    .insert_header("x-vqd-hash-1", next_vqd)
                    .set_body_string(sse_body),
            )
            .mount(&self.server)
            .await;
    }

    /// Preset: Raw SSE body.
    pub async fn mock_chat_raw_sse(&self, sse_body: &str, next_vqd: &str) {
        Mock::given(method("POST"))
            .and(path("/duckchat/v1/chat"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream; charset=utf-8")
                    .insert_header("x-vqd-hash-1", next_vqd)
                    .set_body_string(sse_body),
            )
            .mount(&self.server)
            .await;
    }

    /// Preset: Chat completion requiring a valid solved V8 challenge in x-vqd-hash-1 header.
    pub async fn mock_chat_with_solved_challenge(
        &self,
        model: &str,
        chunks: &[&str],
        next_vqd: &str,
    ) {
        let sse_body = create_sse_stream_body(chunks);

        Mock::given(method("POST"))
            .and(path("/duckchat/v1/chat"))
            .and(DuckHeadersMatcher::new())
            .and(SolvedV8ChallengeMatcher)
            .and(DuckPayloadMatcher::for_model(model))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream; charset=utf-8")
                    .insert_header("x-vqd-hash-1", next_vqd)
                    .set_body_string(sse_body),
            )
            .mount(&self.server)
            .await;
    }

    /// Register default generic mock routes for high-concurrency / generic tests.
    pub async fn register_default_routes(&self) {
        self.mock_status_ok("default-vqd-token").await;

        let sse_body = create_sse_stream_body(&["Default", " response", " chunk."]);
        Mock::given(method("POST"))
            .and(path("/duckchat/v1/chat"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/event-stream; charset=utf-8")
                    .insert_header("x-vqd-hash-1", "next-default-vqd-token")
                    .set_body_string(sse_body),
            )
            .mount(&self.server)
            .await;
    }
}
