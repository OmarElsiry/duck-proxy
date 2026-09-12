//! Common test harness, mock server setup, and test utilities.

#![allow(dead_code)]
#![allow(unused_imports)]

pub mod fixtures;
pub mod matchers;
pub mod mock_upstream;

pub use fixtures::*;
pub use matchers::*;
pub use mock_upstream::MockDuckServer;

use serde_json::Value;
use tokio::net::TcpListener;
use tokio_stream::StreamExt;

/// High-level test harness managing the hermetic mock upstream and Axum proxy server.
pub struct TestHarness {
    pub mock_upstream: MockDuckServer,
    pub server_url: String,
    pub client: reqwest::Client,
    pub upstream_url: String,
    _server_handle: tokio::task::JoinHandle<()>,
}

impl Drop for TestHarness {
    fn drop(&mut self) {
        self._server_handle.abort();
    }
}

impl TestHarness {
    /// Initialize a new test harness with mock upstream and test proxy server on loopback.
    pub async fn new() -> Self {
        Self::with_auto_fallback(false).await
    }

    pub async fn with_auto_fallback(auto_fallback: bool) -> Self {
        let mock_upstream = MockDuckServer::start().await;
        let upstream_url = mock_upstream.uri();

        let mut config = duck_proxy_rs::config::Config::default();
        config.upstream_base_url = upstream_url.clone();
        config.server.host = "127.0.0.1".to_string();
        config.server.port = 0; // Ephemeral OS port
        config.auto_fallback = auto_fallback;

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("Failed to bind ephemeral port for test server");
        let local_addr = listener
            .local_addr()
            .expect("Failed to obtain local address");
        let server_url = format!("http://{}", local_addr);

        let app = duck_proxy_rs::create_app(config);

        let server_handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build test reqwest client");

        Self {
            mock_upstream,
            server_url,
            client,
            upstream_url,
            _server_handle: server_handle,
        }
    }

    /// Send `GET /v1/models` request.
    pub async fn get_models(&self) -> reqwest::Response {
        self.client
            .get(format!("{}/v1/models", self.server_url))
            .send()
            .await
            .expect("Failed to execute GET /v1/models")
    }

    /// Send `POST /v1/chat/completions` request.
    pub async fn chat_completions(&self, body: Value) -> reqwest::Response {
        self.client
            .post(format!("{}/v1/chat/completions", self.server_url))
            .json(&body)
            .send()
            .await
            .expect("Failed to execute POST /v1/chat/completions")
    }

    /// Send `POST /v1/images/generations` request.
    pub async fn image_generations(&self, body: Value) -> reqwest::Response {
        self.client
            .post(format!("{}/v1/images/generations", self.server_url))
            .json(&body)
            .send()
            .await
            .expect("Failed to execute POST /v1/images/generations")
    }

    /// Parse SSE response into vector of JSON delta chunks and verify [DONE] terminator.
    /// Accumulates raw byte slices to prevent multi-byte UTF-8 sequence corruption
    /// across TCP chunk boundaries before splitting on `\n\n`.
    pub async fn read_sse_stream(&self, response: reqwest::Response) -> (Vec<Value>, bool) {
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        let mut stream = response.bytes_stream();
        let mut byte_buffer: Vec<u8> = Vec::new();
        let mut chunks = Vec::new();
        let mut saw_done = false;

        while let Some(item) = stream.next().await {
            let bytes = item.expect("Stream read failure");
            byte_buffer.extend_from_slice(&bytes);

            while let Some(pos) = byte_buffer.windows(2).position(|w| w == b"\n\n") {
                let event_bytes: Vec<u8> = byte_buffer.drain(..pos + 2).collect();
                let event_str = String::from_utf8_lossy(&event_bytes[..pos]);

                for line in event_str.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        let trimmed = data.trim();
                        if trimmed == "[DONE]" {
                            saw_done = true;
                        } else if let Ok(json) = serde_json::from_str::<Value>(trimmed) {
                            chunks.push(json);
                        }
                    }
                }
            }
        }

        // Process any remaining bytes in buffer
        if !byte_buffer.is_empty() {
            let event_str = String::from_utf8_lossy(&byte_buffer);
            for line in event_str.lines() {
                if let Some(data) = line.strip_prefix("data: ") {
                    let trimmed = data.trim();
                    if trimmed == "[DONE]" {
                        saw_done = true;
                    } else if let Ok(json) = serde_json::from_str::<Value>(trimmed) {
                        chunks.push(json);
                    }
                }
            }
        }

        (chunks, saw_done)
    }

    /// Helper to accumulate text from SSE stream chunks.
    pub async fn read_sse_text(&self, response: reqwest::Response) -> String {
        let (chunks, _done) = self.read_sse_stream(response).await;
        let mut text = String::new();
        for chunk in chunks {
            if let Some(delta) = chunk
                .pointer("/choices/0/delta/content")
                .and_then(|v| v.as_str())
            {
                text.push_str(delta);
            }
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_harness_drop_aborts_server() {
        let server_url;
        {
            let harness = TestHarness::new().await;
            server_url = harness.server_url.clone();
            let client = reqwest::Client::new();
            let _ = client.get(&server_url).send().await;
        }
        // After harness is dropped, the server task should be aborted
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let client = reqwest::Client::new();
        let resp = client.get(&server_url).send().await;
        assert!(resp.is_err(), "Server should have been aborted on Drop");
    }

    #[tokio::test]
    async fn test_read_sse_stream_preserves_multibyte_utf8_across_chunks() {
        let mock_server = MockDuckServer::start().await;
        let json_chunk1 = json!({
            "action": "success",
            "id": "chunk-1",
            "message": "Rust emoji: 🦀 and greetings: こんにちは"
        });
        let raw_sse = format!("data: {}\n\ndata: [DONE]\n\n", json_chunk1);
        mock_server.mock_chat_raw_sse(&raw_sse, "next_vqd").await;

        let client = reqwest::Client::new();
        let resp = client
            .post(format!("{}/duckchat/v1/chat", mock_server.uri()))
            .send()
            .await
            .expect("Failed to send request");

        let harness = TestHarness::new().await;
        let (chunks, saw_done) = harness.read_sse_stream(resp).await;
        assert!(saw_done);
        assert_eq!(chunks.len(), 1);
        assert_eq!(
            chunks[0]["message"].as_str().unwrap(),
            "Rust emoji: 🦀 and greetings: こんにちは"
        );
    }
}
