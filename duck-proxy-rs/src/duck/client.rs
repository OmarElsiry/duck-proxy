//! Duck.ai HTTP client with VQD token chaining, telemetry, session warming, and V8 challenge solving.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use reqwest::header::{HeaderMap, HeaderValue};

use crate::crypto::EphemeralKeypair;
use crate::error::AppError;
use crate::v8::{spawn_v8_actor, V8ActorHandle};
use super::types::*;

/// User-Agent string used for all Duck.ai requests.
pub const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36";

/// Frontend version header value.
pub const FE_VERSION: &str = "serp_20260827_190157_ET-5738d187a3dbca905a80324bd698765a27bf6e44";

/// Maximum retry attempts for chat requests.
const MAX_RETRIES: u32 = 3;

/// Per-model session state to prevent cross-model challenge pollution (418 ERR_CHALLENGE).
#[derive(Clone, Debug)]
pub struct ModelSession {
    pub journey_id: String,
    pub pending_challenge: Option<String>,
}

impl ModelSession {
    pub fn new() -> Self {
        Self {
            journey_id: uuid::Uuid::new_v4().simple().to_string(),
            pending_challenge: None,
        }
    }
}

impl Default for ModelSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Core Duck.ai HTTP client with VQD token chaining, cookie warming, and V8 challenge solver.
pub struct DuckClient {
    http: reqwest::Client,
    upstream_base_url: String,
    sessions: Arc<RwLock<HashMap<String, ModelSession>>>,
    warmed: Arc<RwLock<bool>>,
    status_lock: Arc<Mutex<()>>,
    keypair: EphemeralKeypair,
    v8_actor: V8ActorHandle,
}

impl DuckClient {
    /// Creates a new DuckClient with browser fingerprint headers and spawned V8 challenge actor.
    pub fn new(upstream_base_url: &str) -> Self {
        let v8_actor = spawn_v8_actor();
        Self::with_v8_actor(upstream_base_url, v8_actor)
    }

    /// Creates a new DuckClient with a custom V8ActorHandle.
    pub fn with_v8_actor(upstream_base_url: &str, v8_actor: V8ActorHandle) -> Self {
        let mut default_headers = Self::browser_headers();
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
            sessions: Arc::new(RwLock::new(HashMap::new())),
            warmed: Arc::new(RwLock::new(false)),
            status_lock: Arc::new(Mutex::new(())),
            keypair,
            v8_actor,
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
        headers.insert("sec-ch-ua-platform", HeaderValue::from_static(r#""Linux""#));
        headers.insert("sec-fetch-dest", HeaderValue::from_static("empty"));
        headers.insert("sec-fetch-mode", HeaderValue::from_static("cors"));
        headers.insert("sec-fetch-site", HeaderValue::from_static("same-origin"));
        headers
    }

    /// Extracts the challenge token from multiple potential response header names.
    pub fn extract_vqd_header(headers: &HeaderMap) -> Option<String> {
        for name in &["x-vqd-hash-1", "x-vqd-4", "x-vqd-hash", "x-vqd"] {
            if let Some(val) = headers.get(*name) {
                if let Ok(s) = val.to_str() {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
        None
    }

    /// Warms up session cookies by visiting the homepage and auth/token endpoints.
    pub async fn warm(&self) {
        {
            let is_warmed = *self.warmed.read().await;
            if is_warmed {
                return;
            }
        }

        let mut warmed_lock = self.warmed.write().await;
        if *warmed_lock {
            return;
        }

        let _ = self.http.get(&format!("{}/", self.upstream_base_url)).send().await;
        let _ = self.http.get(&format!("{}/duckchat/v1/auth/token", self.upstream_base_url)).send().await;
        *warmed_lock = true;
    }

    /// Retrieves or initializes the session for a given model.
    pub async fn get_or_create_session(&self, model: &str) -> ModelSession {
        let mut sessions = self.sessions.write().await;
        sessions.entry(model.to_string()).or_default().clone()
    }

    /// Generates telemetry headers for a chat request.
    pub fn generate_telemetry_headers(&self, journey_id: &str) -> Vec<(String, String)> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let start_ms = now_ms - 30000;
        let signals = FeSignals {
            start: start_ms,
            events: vec![
                FeEvent { name: "onboarding_impression".to_string(), delta: 240, trusted: None },
                FeEvent { name: "action".to_string(), delta: 12000, trusted: Some(true) },
                FeEvent { name: "onboarding_impression".to_string(), delta: 15500, trusted: None },
                FeEvent { name: "onboarding_finish".to_string(), delta: 25000, trusted: None },
                FeEvent { name: "startNewChat_free".to_string(), delta: 27500, trusted: None },
            ],
            end: 28800,
        };
        let signals_json = serde_json::to_string(&signals).unwrap_or_default();
        let signals_b64 = BASE64_STANDARD.encode(signals_json.as_bytes());

        vec![
            ("x-fe-version".to_string(), FE_VERSION.to_string()),
            ("x-ddg-journey-id".to_string(), journey_id.to_string()),
            ("x-fe-signals".to_string(), signals_b64),
        ]
    }

    /// Fetches an initial raw VQD challenge from /duckchat/v1/status.
    pub async fn fetch_raw_status_challenge(&self, journey_id: &str) -> Result<String, AppError> {
        self.warm().await;
        let _guard = self.status_lock.lock().await;
        let url = format!("{}/duckchat/v1/status", self.upstream_base_url);

        for attempt in 0..5 {
            let resp = self.http
                .get(&url)
                .header("accept", "*/*")
                .header("x-vqd-accept", "1")
                .header("x-ddg-journey-id", journey_id)
                .header("cache-control", "no-store")
                .header("pragma", "no-cache")
                .header("referer", "https://duck.ai/")
                .header("origin", "https://duck.ai")
                .send()
                .await?;

            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                if attempt < 4 {
                    tracing::warn!("Duck.ai status 429, waiting 3.5s (attempt {}/5)...", attempt + 1);
                    tokio::time::sleep(tokio::time::Duration::from_millis(3500)).await;
                    continue;
                }
                return Err(AppError::upstream_rate_limit(
                    "Duck.ai rate limit exceeded on status endpoint",
                    Some(4),
                ));
            }

            if !resp.status().is_success() {
                return Err(AppError::bad_gateway(format!(
                    "VQD status request failed with HTTP {}",
                    resp.status()
                )));
            }

            if let Some(raw_challenge) = Self::extract_vqd_header(resp.headers()) {
                return Ok(raw_challenge);
            }

            return Err(AppError::bad_gateway("No x-vqd-hash-1 challenge returned in status response"));
        }

        Err(AppError::upstream_rate_limit("Failed to obtain VQD token", Some(4)))
    }

    /// Gets and solves a VQD challenge header for the specified model.
    pub async fn get_solved_challenge_header(&self, model: &str, journey_id: &str) -> Result<String, AppError> {
        let raw_chal = {
            let mut sessions = self.sessions.write().await;
            let session = sessions.entry(model.to_string()).or_default();
            session.pending_challenge.take()
        };

        let raw = match raw_chal {
            Some(c) => c,
            None => self.fetch_raw_status_challenge(journey_id).await?,
        };

        self.v8_actor.solve_challenge(raw).await
            .map_err(|e| AppError::bad_gateway(format!("V8 Challenge solver error: {}", e)))
    }

    /// Stores a raw chained VQD challenge from upstream response for a model.
    async fn store_chained_challenge(&self, model: &str, raw_challenge: String) {
        let mut sessions = self.sessions.write().await;
        let session = sessions.entry(model.to_string()).or_default();
        session.pending_challenge = Some(raw_challenge);
    }

    /// Resets session journey ID and challenge for a given model.
    async fn reset_model_session(&self, model: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.insert(model.to_string(), ModelSession::new());
    }

    /// Sends a chat request to Duck.ai with automatic challenge solving and error recovery retry.
    pub async fn send_chat_request(
        &self,
        payload: &DuckChatRequest,
    ) -> Result<reqwest::Response, AppError> {
        let url = format!("{}/duckchat/v1/chat", self.upstream_base_url);
        let model = payload.model.clone();

        for attempt in 0..MAX_RETRIES {
            let session = self.get_or_create_session(&model).await;
            let solved_vqd = self.get_solved_challenge_header(&model, &session.journey_id).await?;

            let mut request = self.http
                .post(&url)
                .header("x-vqd-hash-1", &solved_vqd)
                .header("accept", "text/event-stream")
                .header("content-type", "application/json");

            for (key, value) in self.generate_telemetry_headers(&session.journey_id) {
                request = request.header(&key, &value);
            }

            let resp = request.json(payload).send().await?;

            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                if attempt < MAX_RETRIES - 1 {
                    self.reset_model_session(&model).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(500 * (1 << attempt))).await;
                    continue;
                }
                return Err(AppError::upstream_rate_limit(
                    "Duck.ai chat endpoint rate limited",
                    Some(4),
                ));
            }

            if resp.status().as_u16() == 418 {
                tracing::warn!("Duck.ai challenge rejected (418) for model '{}', resetting session and retrying...", model);
                self.reset_model_session(&model).await;
                if attempt < MAX_RETRIES - 1 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500 * (1 << attempt))).await;
                    continue;
                }
            }

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(AppError::bad_gateway(format!(
                    "Duck.ai chat returned HTTP {} {}",
                    status, body
                )));
            }

            // Chain next challenge if returned
            if let Some(new_vqd) = Self::extract_vqd_header(resp.headers()) {
                self.store_chained_challenge(&model, new_vqd).await;
            }

            return Ok(resp);
        }

        Err(AppError::bad_gateway("Failed to complete chat request after retries"))
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
            headers.get("referer").unwrap().to_str().unwrap(),
            "https://duck.ai/"
        );
    }

    #[test]
    fn test_extract_vqd_header() {
        let mut headers = HeaderMap::new();
        headers.insert("x-vqd-hash-1", HeaderValue::from_static("test-vqd-1"));
        assert_eq!(
            DuckClient::extract_vqd_header(&headers),
            Some("test-vqd-1".to_string())
        );

        let mut headers2 = HeaderMap::new();
        headers2.insert("x-vqd-4", HeaderValue::from_static("test-vqd-4"));
        assert_eq!(
            DuckClient::extract_vqd_header(&headers2),
            Some("test-vqd-4".to_string())
        );
    }
}
