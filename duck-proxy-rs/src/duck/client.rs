//! Duck.ai HTTP client with VQD token chaining and telemetry.

use std::sync::Arc;
use tokio::sync::RwLock;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use reqwest::header::{HeaderMap, HeaderValue};

use crate::crypto::EphemeralKeypair;
use crate::error::AppError;
use super::types::*;

/// User-Agent string used for all Duck.ai requests.
pub const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36";

/// Frontend version header value.
pub const FE_VERSION: &str = "serp_20260827_190157_ET-5738d187a3dbca905a80324bd698765a27bf6e44";

/// Maximum retry attempts for 429 responses.
const MAX_RETRIES: u32 = 5;

/// Initial backoff delay in seconds for 429 retries.
const BACKOFF_BASE_SECS: f64 = 0.1;

/// Core Duck.ai HTTP client with VQD token chaining.
pub struct DuckClient {
    http: reqwest::Client,
    upstream_base_url: String,
    pending_hash: Arc<RwLock<Option<String>>>,
    keypair: EphemeralKeypair,
}

impl DuckClient {
    /// Creates a new DuckClient with browser fingerprint headers.
    pub fn new(upstream_base_url: &str) -> Self {
        let mut default_headers = Self::browser_headers();
        // Remove host-specific headers that reqwest manages
        default_headers.remove("host");

        let http = reqwest::Client::builder()
            .default_headers(default_headers)
            .cookie_store(true)
            .build()
            .expect("Failed to build HTTP client");

        let keypair = EphemeralKeypair::generate()
            .expect("Failed to generate ephemeral RSA keypair");

        Self {
            http,
            upstream_base_url: upstream_base_url.trim_end_matches('/').to_string(),
            pending_hash: Arc::new(RwLock::new(None)),
            keypair,
        }
    }

    /// Returns the browser fingerprint headers required by Duck.ai.
    fn browser_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", HeaderValue::from_static(USER_AGENT));
        headers.insert("accept-language", HeaderValue::from_static("en-US,en;q=0.9"));
        headers.insert("referer", HeaderValue::from_static("https://duck.ai/"));
        headers.insert("origin", HeaderValue::from_static("https://duck.ai"));
        headers.insert(
            "sec-ch-ua",
            HeaderValue::from_static(r#""Not;A=Brand";v="8", "Chromium";v="150", "Google Chrome";v="150""#),
        );
        headers.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
        headers.insert("sec-ch-ua-platform", HeaderValue::from_static("\"Linux\""));
        headers.insert("sec-fetch-dest", HeaderValue::from_static("empty"));
        headers.insert("sec-fetch-mode", HeaderValue::from_static("cors"));
        headers.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        headers
    }

    /// Generates telemetry headers for a chat request.
    pub fn generate_telemetry_headers() -> Vec<(String, String)> {
        let journey_id = uuid::Uuid::new_v4().simple().to_string();

        let now_ms = chrono::Utc::now().timestamp_millis();
        let signals = FeSignals {
            start: now_ms - 25000,
            events: vec![
                FeEvent { name: "onboarding_impression".to_string(), delta: 240, trusted: None },
                FeEvent { name: "action".to_string(), delta: 12000, trusted: Some(true) },
                FeEvent { name: "startNewChat_free".to_string(), delta: 25000, trusted: None },
            ],
            end: 25100,
        };
        let signals_json = serde_json::to_string(&signals).unwrap_or_default();
        let signals_b64 = BASE64_STANDARD.encode(signals_json.as_bytes());

        vec![
            ("x-fe-version".to_string(), FE_VERSION.to_string()),
            ("x-ddg-journey-id".to_string(), journey_id),
            ("x-fe-signals".to_string(), signals_b64),
        ]
    }

    /// Fetches an initial VQD token from Duck.ai /duckchat/v1/status.
    /// Retries up to MAX_RETRIES times with exponential backoff on 429.
    pub async fn fetch_initial_vqd(&self) -> Result<String, AppError> {
        let url = format!("{}/duckchat/v1/status", self.upstream_base_url);

        for attempt in 0..MAX_RETRIES {
            let resp = self.http
                .get(&url)
                .header("x-vqd-accept", "1")
                .send()
                .await?;

            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                if attempt < MAX_RETRIES - 1 {
                    let delay = BACKOFF_BASE_SECS * 2.0_f64.powi(attempt as i32);
                    tracing::warn!(
                        "VQD status returned 429, retrying in {:.1}s (attempt {}/{})",
                        delay, attempt + 1, MAX_RETRIES
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs_f64(delay)).await;
                    continue;
                }
                return Err(AppError::upstream_rate_limit(
                    "Duck.ai rate limit exceeded after maximum retries",
                    Some(4),
                ));
            }

            if !resp.status().is_success() {
                return Err(AppError::bad_gateway(format!(
                    "VQD status request failed with HTTP {}",
                    resp.status()
                )));
            }

            // Extract x-vqd-hash-1 from response headers
            if let Some(vqd) = resp.headers().get("x-vqd-hash-1") {
                return vqd.to_str()
                    .map(|s| s.to_string())
                    .map_err(|_| AppError::bad_gateway("Invalid x-vqd-hash-1 header encoding"));
            }

            return Err(AppError::bad_gateway("No x-vqd-hash-1 in status response"));
        }

        Err(AppError::upstream_rate_limit(
            "Failed to obtain VQD token after retries",
            Some(4),
        ))
    }

    /// Returns the cached VQD token or fetches a new one.
    pub async fn get_or_fetch_vqd(&self) -> Result<String, AppError> {
        // Try to consume cached hash first
        {
            let mut hash = self.pending_hash.write().await;
            if let Some(h) = hash.take() {
                return Ok(h);
            }
        }
        self.fetch_initial_vqd().await
    }

    /// Stores a VQD token for the next request (token chaining).
    async fn store_vqd(&self, token: String) {
        let mut hash = self.pending_hash.write().await;
        *hash = Some(token);
    }

    /// Sends a chat request to Duck.ai and returns the raw response for streaming.
    pub async fn send_chat_request(
        &self,
        payload: &DuckChatRequest,
    ) -> Result<reqwest::Response, AppError> {
        let url = format!("{}/duckchat/v1/chat", self.upstream_base_url);
        let vqd = self.get_or_fetch_vqd().await?;

        let mut request = self.http
            .post(&url)
            .header("x-vqd-hash-1", &vqd)
            .header("accept", "text/event-stream")
            .header("content-type", "application/json");

        // Add telemetry headers
        for (key, value) in Self::generate_telemetry_headers() {
            request = request.header(&key, &value);
        }

        let resp = request.json(payload).send().await?;

        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::upstream_rate_limit(
                "Duck.ai chat endpoint rate limited",
                Some(4),
            ));
        }

        if !resp.status().is_success() {
            return Err(AppError::bad_gateway(format!(
                "Duck.ai chat returned HTTP {}",
                resp.status()
            )));
        }

        // Chain the VQD token from response
        if let Some(new_vqd) = resp.headers().get("x-vqd-hash-1") {
            if let Ok(s) = new_vqd.to_str() {
                self.store_vqd(s.to_string()).await;
            }
        }

        Ok(resp)
    }

    /// Returns a reference to the ephemeral keypair.
    pub fn keypair(&self) -> &EphemeralKeypair {
        &self.keypair
    }

    /// Returns the upstream base URL.
    pub fn upstream_base_url(&self) -> &str {
        &self.upstream_base_url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_headers() {
        let headers = DuckClient::browser_headers();
        assert_eq!(
            headers.get("user-agent").unwrap().to_str().unwrap(),
            USER_AGENT
        );
        assert_eq!(
            headers.get("origin").unwrap().to_str().unwrap(),
            "https://duck.ai"
        );
        assert_eq!(
            headers.get("sec-fetch-mode").unwrap().to_str().unwrap(),
            "cors"
        );
    }

    #[test]
    fn test_telemetry_headers() {
        let headers = DuckClient::generate_telemetry_headers();
        assert_eq!(headers.len(), 3);

        let (fe_key, fe_val) = &headers[0];
        assert_eq!(fe_key, "x-fe-version");
        assert_eq!(fe_val, FE_VERSION);

        let (journey_key, journey_val) = &headers[1];
        assert_eq!(journey_key, "x-ddg-journey-id");
        assert_eq!(journey_val.len(), 32); // UUID simple format

        let (signals_key, signals_val) = &headers[2];
        assert_eq!(signals_key, "x-fe-signals");
        // Should be valid base64
        let decoded = BASE64_STANDARD.decode(signals_val).expect("Invalid base64");
        let signals: FeSignals = serde_json::from_slice(&decoded).expect("Invalid JSON");
        assert_eq!(signals.events.len(), 3);
        assert_eq!(signals.events[0].name, "onboarding_impression");
    }

    #[test]
    fn test_client_creation() {
        let client = DuckClient::new("https://duck.ai");
        assert_eq!(client.upstream_base_url(), "https://duck.ai");
        assert_eq!(client.keypair().public_jwk().alg, "RSA-OAEP-256");
    }
}
