//! Duck.ai HTTP client with VQD token chaining, telemetry, session warming, and V8 challenge solving.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use reqwest::header::{HeaderMap, HeaderValue};

use crate::crypto::EphemeralKeypair;
use crate::error::AppError;
use crate::v8::{spawn_v8_actor, V8ActorHandle};
use super::types::*;

/// User-Agent rotation pool for anti-rate-limit resilience.
pub const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
];

/// Primary User-Agent string used for default matching.
pub const USER_AGENT: &str = USER_AGENTS[0];

pub const FE_VERSION: &str = "serp_20260901_082630_ET-936860e07343d04bca3ac6903356b645079e640f";

/// Maximum retry attempts for chat requests.
const MAX_RETRIES: u32 = 3;

/// Per-model session state to prevent cross-model challenge pollution (418 ERR_CHALLENGE).
#[derive(Clone, Debug)]
pub struct ModelSession {
    pub journey_id: String,
    pub conversation_id: String,
    pub pending_challenge: Option<String>,
    pub user_agent: String,
}

impl ModelSession {
    pub fn new() -> Self {
        let ua_idx = (chrono::Utc::now().timestamp_subsec_nanos() as usize) % USER_AGENTS.len();
        Self {
            user_agent: USER_AGENTS[ua_idx].to_string(),
            journey_id: uuid::Uuid::new_v4().simple().to_string(),
            conversation_id: uuid::Uuid::new_v4().to_string(),
            pending_challenge: None,
        }
    }

    pub fn reset(&mut self) {
        let ua_idx = (chrono::Utc::now().timestamp_subsec_nanos() as usize) % USER_AGENTS.len();
        self.user_agent = USER_AGENTS[ua_idx].to_string();
        self.journey_id = uuid::Uuid::new_v4().simple().to_string();
        self.conversation_id = uuid::Uuid::new_v4().to_string();
        self.pending_challenge = None;
    }

    pub fn rotate_user_agent(&mut self) {
        let ua_idx = (chrono::Utc::now().timestamp_subsec_nanos() as usize) % USER_AGENTS.len();
        self.user_agent = USER_AGENTS[ua_idx].to_string();
        self.journey_id = uuid::Uuid::new_v4().simple().to_string();
        self.conversation_id = uuid::Uuid::new_v4().to_string();
        self.pending_challenge = None;
    }
}

pub fn platform_for_ua(ua: &str) -> &'static str {
    if ua.contains("Windows") {
        "\"Windows\""
    } else if ua.contains("Macintosh") {
        "\"macOS\""
    } else {
        "\"Linux\""
    }
}

pub fn sec_ch_ua_for_ua(ua: &str) -> &'static str {
    if ua.contains("Edg/") {
        r#""Not A(Brand";v="8", "Chromium";v="150", "Microsoft Edge";v="150""#
    } else {
        r#""Not;A=Brand";v="8", "Chromium";v="150", "Google Chrome";v="150""#
    }
}

impl Default for ModelSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Upstream Duck.ai client with session and concurrency management.
#[derive(Clone)]
pub struct DuckClient {
    http: reqwest::Client,
    status_http: reqwest::Client,
    upstream_base_url: String,
    sessions: Arc<RwLock<HashMap<String, ModelSession>>>,
    vqd_pool: Arc<Mutex<Vec<String>>>,
    warmed: Arc<RwLock<HashSet<String>>>,
    fe_version: Arc<RwLock<String>>,
    status_lock: Arc<Mutex<()>>,
    chat_lock: Arc<Mutex<()>>,
    last_status_call: Arc<Mutex<Option<tokio::time::Instant>>>,
    last_chat_call: Arc<Mutex<Option<tokio::time::Instant>>>,
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

        let status_http = reqwest::Client::builder()
            .default_headers(Self::browser_headers())
            .cookie_store(true)
            .build()
            .expect("Failed to build status HTTP client");

        let keypair = EphemeralKeypair::generate()
            .expect("Failed to generate ephemeral keypair for Duck.ai client");

        Self {
            http,
            status_http,
            upstream_base_url: upstream_base_url.trim_end_matches('/').to_string(),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            vqd_pool: Arc::new(Mutex::new(Vec::new())),
            warmed: Arc::new(RwLock::new(HashSet::new())),
            fe_version: Arc::new(RwLock::new(FE_VERSION.to_string())),
            status_lock: Arc::new(Mutex::new(())),
            chat_lock: Arc::new(Mutex::new(())),
            last_status_call: Arc::new(Mutex::new(None)),
            last_chat_call: Arc::new(Mutex::new(None)),
            keypair,
            v8_actor,
        }
    }

    /// Starts a background token prefetcher (disabled to prevent upstream rate limits).
    pub fn start_background_pool_worker(self: &Arc<Self>) {
        // Disabled background polling to respect Duck.ai rate limits
    }

    /// Returns the browser fingerprint headers required by Duck.ai.
    fn browser_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", HeaderValue::from_static(USER_AGENT));
        headers.insert("accept-language", HeaderValue::from_static("en-US,en;q=0.9"));
        headers.insert("referer", HeaderValue::from_static("https://duck.ai/"));
        headers.insert(
            "sec-ch-ua",
            HeaderValue::from_static(r#""Not;A=Brand";v="8", "Chromium";v="150", "Google Chrome";v="150""#),
        );
        headers.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
        headers.insert("sec-ch-ua-platform", HeaderValue::from_static(r#""Windows""#));
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

    /// Warms up session cookies for a specific journey ID and refreshes frontend version.
    pub async fn warm(&self, journey_id: &str) {
        {
            let warmed = self.warmed.read().await;
            if warmed.contains(journey_id) {
                return;
            }
        }

        let mut warmed_lock = self.warmed.write().await;
        if warmed_lock.contains(journey_id) {
            return;
        }

        // Fetch landing page once to dynamically extract current fe-version tag and sha
        if let Ok(resp) = self.http.get(&self.upstream_base_url).send().await {
            if let Ok(html) = resp.text().await {
                let tag = html.split("data-version-tag=\"")
                    .nth(1)
                    .and_then(|s| s.split('"').next());
                let sha = html.split("data-version-sha=\"")
                    .nth(1)
                    .and_then(|s| s.split('"').next());
                if let (Some(t), Some(s)) = (tag, sha) {
                    let dynamic_fe_version = format!("{}-{}", t, s);
                    let mut fe_ver = self.fe_version.write().await;
                    *fe_ver = dynamic_fe_version;
                }
            }
        }

        warmed_lock.insert(journey_id.to_string());
    }

    /// Forces a rewarm on the next request.
    pub async fn force_rewarm(&self, journey_id: &str) {
        let mut warmed = self.warmed.write().await;
        warmed.remove(journey_id);
        let mut sessions = self.sessions.write().await;
        for s in sessions.values_mut() {
            s.rotate_user_agent();
        }
        let mut pool = self.vqd_pool.lock().await;
        pool.clear();
    }

    /// Gets or creates a per-model session ensuring unique journey ID.
    pub async fn get_or_create_session(&self, model: &str) -> ModelSession {
        let mut sessions = self.sessions.write().await;
        sessions.entry(model.to_string()).or_default().clone()
    }

    /// Generates telemetry headers for a chat request with natural jitter.
    pub fn generate_telemetry_headers(&self, journey_id: &str) -> Vec<(String, String)> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let jitter = (now_ms as u64 % 200) as i64;
        let start_ms = now_ms - 1500 - jitter;
        let signals = FeSignals {
            start: start_ms,
            events: vec![
                FeEvent { name: "startNewChat_free".to_string(), delta: 50 + (jitter % 20), trusted: None },
                FeEvent { name: "recentChatsListImpression".to_string(), delta: 180 + (jitter % 30), trusted: None },
                FeEvent { name: "action".to_string(), delta: 800 + (jitter % 100), trusted: Some(true) },
            ],
            end: 880 + jitter,
        };
        let signals_json = serde_json::to_string(&signals).unwrap_or_default();
        let signals_b64 = BASE64_STANDARD.encode(signals_json.as_bytes());

        let fe_version = self
            .fe_version
            .try_read()
            .map(|v| v.clone())
            .unwrap_or_else(|_| FE_VERSION.to_string());

        vec![
            ("x-fe-version".to_string(), fe_version),
            ("x-ddg-journey-id".to_string(), journey_id.to_string()),
            ("x-fe-signals".to_string(), signals_b64),
        ]
    }

    /// Fetches an initial raw VQD challenge from /duckchat/v1/status with resilient backoff.
    pub async fn fetch_raw_status_challenge(&self, journey_id: &str, user_agent: &str) -> Result<String, AppError> {
        let _guard = self.status_lock.lock().await;

        // Ensure at least 3.5s spacing between consecutive /status requests to avoid IP rate limits
        {
            let mut last_call = self.last_status_call.lock().await;
            if let Some(prev) = *last_call {
                let elapsed = prev.elapsed();
                if elapsed < tokio::time::Duration::from_millis(3500) {
                    let sleep_dur = tokio::time::Duration::from_millis(3500) - elapsed;
                    tokio::time::sleep(sleep_dur).await;
                }
            }
            *last_call = Some(tokio::time::Instant::now());
        }

        let url = format!("{}/duckchat/v1/status", self.upstream_base_url);

        for attempt in 0..2 {
            let ua = if attempt == 0 {
                user_agent
            } else {
                let idx = (chrono::Utc::now().timestamp_subsec_millis() as usize + attempt as usize) % USER_AGENTS.len();
                USER_AGENTS[idx]
            };

            let req = self.status_http
                .get(&url)
                .header("user-agent", ua)
                .header("sec-ch-ua", sec_ch_ua_for_ua(ua))
                .header("sec-ch-ua-platform", platform_for_ua(ua))
                .header("sec-ch-ua-mobile", "?0")
                .header("accept", "*/*")
                .header("x-vqd-accept", "1")
                .header("referer", "https://duck.ai/");

            let resp = req.send().await?;

            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let from_pool = {
                    let mut pool = self.vqd_pool.lock().await;
                    pool.pop()
                };
                if let Some(token) = from_pool {
                    tracing::info!("Using pooled VQD challenge on status 429");
                    return Ok(token);
                }

                if attempt < 1 {
                    let delay_ms = 35000 + (chrono::Utc::now().timestamp_subsec_millis() as u64 % 500);
                    tracing::warn!("Duck.ai status 429, cooling down for {}ms before retry (attempt 1/2)...", delay_ms);
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                    continue;
                }
                return Err(AppError::upstream_rate_limit(
                    "Duck.ai rate limit exceeded on status endpoint",
                    Some(4),
                ));
            }

            if !resp.status().is_success() {
                if attempt < 1 {
                    self.force_rewarm(journey_id).await;
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    continue;
                }
                return Err(AppError::bad_gateway(format!(
                    "VQD status request failed with HTTP {}",
                    resp.status()
                )));
            }

            if let Some(raw_challenge) = Self::extract_vqd_header(resp.headers()) {
                return Ok(raw_challenge);
            }

            if attempt < 1 {
                self.force_rewarm(journey_id).await;
                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                continue;
            }

            return Err(AppError::bad_gateway("No x-vqd-hash-1 challenge returned in status response"));
        }

        Err(AppError::upstream_rate_limit("Failed to obtain VQD token", Some(4)))
    }

    /// Gets and solves a fresh VQD challenge header for the specified model.
    pub async fn get_solved_challenge_header(&self, model: &str, journey_id: &str) -> Result<String, AppError> {
        let session = self.get_or_create_session(model).await;
        let from_pool = {
            let mut pool = self.vqd_pool.lock().await;
            pool.pop()
        };

        let raw = match from_pool {
            Some(c) => c,
            None => self.fetch_raw_status_challenge(journey_id, &session.user_agent).await?,
        };

        {
            let mut sessions = self.sessions.write().await;
            let s = sessions.entry(model.to_string()).or_default();
            s.conversation_id = uuid::Uuid::new_v4().to_string();
        }

        self.v8_actor.solve_challenge_with_ua(raw, Some(session.user_agent.clone())).await
            .map_err(|e| AppError::bad_gateway(format!("V8 Challenge solver error: {}", e)))
    }

    /// Stores a raw chained VQD challenge for subsequent requests.
    async fn store_chained_challenge(&self, _model: &str, raw_challenge: String) {
        let mut pool = self.vqd_pool.lock().await;
        pool.push(raw_challenge);
    }

    /// Resets session journey ID and challenge for a given model with User-Agent rotation.
    async fn reset_model_session(&self, model: &str) {
        let mut sessions = self.sessions.write().await;
        let mut new_session = ModelSession::new();
        new_session.rotate_user_agent();
        sessions.insert(model.to_string(), new_session);
    }

    /// Sends a chat request to Duck.ai with automatic challenge solving and candidate model fallback cascade.
    pub async fn send_chat_request_cascade(
        &self,
        requested_model: &str,
        messages: &[DuckChatMessage],
        fallback_chain: &[String],
        is_image_gen: bool,
    ) -> Result<(reqwest::Response, String), AppError> {
        let _chat_guard = self.chat_lock.lock().await;
        let mut last_err = None;
        const MAX_CASCADE_ROUNDS: usize = 1;

        for round in 0..MAX_CASCADE_ROUNDS {
            if round > 0 {
                let backoff_ms = 1000 + (chrono::Utc::now().timestamp_subsec_millis() as u64 % 200);
                tracing::warn!(
                    "Model '{}' rate limited or waiting for cooldown, waiting {}ms before round {}/{}...",
                    requested_model,
                    backoff_ms,
                    round + 1,
                    MAX_CASCADE_ROUNDS
                );
                tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
            }

            for (idx, candidate_model) in fallback_chain.iter().enumerate() {
                let session = self.get_or_create_session(candidate_model).await;
                let solved_vqd = match self.get_solved_challenge_header(candidate_model, &session.journey_id).await {
                    Ok(v) => v,
                    Err(e) => {
                        last_err = Some(e);
                        continue;
                    }
                };

                let fresh_conversation_id = uuid::Uuid::new_v4().to_string();
                let payload = crate::duck::payload::build_chat_payload(
                    candidate_model,
                    messages.to_vec(),
                    &self.keypair,
                    is_image_gen,
                    &fresh_conversation_id,
                );

                if idx > 0 || round > 0 {
                    tracing::warn!(
                        "Attempting chat request (round {}/{}) from '{}' to candidate model '{}'...",
                        round + 1,
                        MAX_CASCADE_ROUNDS,
                        requested_model,
                        candidate_model
                    );
                }

                // Minimal spacing between requests to prevent socket congestion
                let min_spacing_ms = if idx > 0 { 200 } else { 1500 };
                {
                    let mut last_call = self.last_chat_call.lock().await;
                    if let Some(prev) = *last_call {
                        let elapsed = prev.elapsed();
                        if elapsed < tokio::time::Duration::from_millis(min_spacing_ms) {
                            let sleep_dur = tokio::time::Duration::from_millis(min_spacing_ms) - elapsed;
                            tokio::time::sleep(sleep_dur).await;
                        }
                    }
                    *last_call = Some(tokio::time::Instant::now());
                }

                let url = format!("{}/duckchat/v1/chat", self.upstream_base_url);
                let mut request = self.http
                    .post(&url)
                    .header("user-agent", &session.user_agent)
                    .header("sec-ch-ua", sec_ch_ua_for_ua(&session.user_agent))
                    .header("sec-ch-ua-platform", platform_for_ua(&session.user_agent))
                    .header("sec-ch-ua-mobile", "?0")
                    .header("x-vqd-hash-1", &solved_vqd)
                    .header("origin", "https://duck.ai")
                    .header("referer", "https://duck.ai/")
                    .header("accept", "text/event-stream")
                    .header("content-type", "application/json");

                for (key, value) in self.generate_telemetry_headers(&session.journey_id) {
                    request = request.header(&key, &value);
                }

                match request.json(&payload).send().await {
                    Ok(resp) => {
                        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                            let maybe_new_vqd = Self::extract_vqd_header(resp.headers());
                            let body_429 = resp.text().await.unwrap_or_default();
                            tracing::warn!("Duck.ai model '{}' rate limited (HTTP 429). Upstream body: {}", candidate_model, body_429);
                            if body_429.contains("ERR_CONVERSATION_LIMIT") || body_429.contains("ERR_SERVICE_UNAVAILABLE") {
                                tracing::info!("Resetting session for model '{}' due to rate limit error: {}", candidate_model, body_429);
                                self.reset_model_session(candidate_model).await;
                                let new_journey = uuid::Uuid::new_v4().simple().to_string();
                                self.warm(&new_journey).await;
                            } else if let Some(new_vqd) = maybe_new_vqd {
                                self.store_chained_challenge(candidate_model, new_vqd).await;
                            }
                            last_err = Some(AppError::upstream_rate_limit(
                                format!("Duck.ai model '{}' rate limited (HTTP 429): {}", candidate_model, body_429),
                                Some(4),
                            ));
                            continue;
                        }

                        if resp.status().as_u16() == 418 {
                            let body_str = resp.text().await.unwrap_or_default();
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&body_str) {
                                if let Some(cd) = val.get("cd") {
                                    let q = cd.get("q").and_then(|v| v.as_str()).unwrap_or("");
                                    let cc = cd.get("cc").and_then(|v| v.as_str()).unwrap_or("duckchat");
                                    let s = cd.get("s").and_then(|v| v.as_str()).unwrap_or("index");
                                    let r = cd.get("r").and_then(|v| v.as_str()).unwrap_or("euw");
                                    let gk = cd.get("gk").and_then(|v| v.as_str()).unwrap_or("");
                                    let p = cd.get("p").and_then(|v| v.as_str()).unwrap_or("");
                                    let o = cd.get("o").and_then(|v| v.as_str()).unwrap_or("");

                                    let p_parts: Vec<&str> = p.split('-').collect();
                                    let acs = if p_parts.len() >= 4 {
                                        format!("{}-{}", p_parts[0], p_parts[3])
                                    } else if p_parts.len() >= 2 {
                                        format!("{}-{}", p_parts[0], p_parts[1])
                                    } else {
                                        p.to_string()
                                    };

                                    let params = [
                                        ("q", q),
                                        ("type", "anomaly"),
                                        ("acs", acs.as_str()),
                                        ("cc", cc),
                                        ("gk", gk),
                                        ("p", p),
                                        ("o", o),
                                        ("s", s),
                                        ("r", r),
                                    ];

                                    tracing::info!("Auto-resolving Duck.ai 418 anomaly for model '{}'...", candidate_model);
                                    let _ = self.http.get(format!("{}/anomaly.js", self.upstream_base_url))
                                        .query(&params)
                                        .header("user-agent", &session.user_agent)
                                        .header("referer", "https://duck.ai/")
                                        .send()
                                        .await;
                                }
                            }

                            tracing::warn!("Resetting model session and warming new journey after 418 challenge for model '{}'...", candidate_model);
                            self.reset_model_session(candidate_model).await;
                            let fresh_session = self.get_or_create_session(candidate_model).await;
                            self.warm(&fresh_session.journey_id).await;
                            tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

                            let fresh_vqd = match self.get_solved_challenge_header(candidate_model, &fresh_session.journey_id).await {
                                Ok(v) => v,
                                Err(_) => solved_vqd.clone(),
                            };

                            let mut retry_req = self.http
                                .post(&url)
                                .header("user-agent", &fresh_session.user_agent)
                                .header("sec-ch-ua", sec_ch_ua_for_ua(&fresh_session.user_agent))
                                .header("sec-ch-ua-platform", platform_for_ua(&fresh_session.user_agent))
                                .header("sec-ch-ua-mobile", "?0")
                                .header("x-vqd-hash-1", &fresh_vqd)
                                .header("origin", "https://duck.ai")
                                .header("referer", "https://duck.ai/")
                                .header("accept", "text/event-stream")
                                .header("content-type", "application/json");

                            for (key, value) in self.generate_telemetry_headers(&fresh_session.journey_id) {
                                retry_req = retry_req.header(&key, &value);
                            }

                            let retry_conversation_id = uuid::Uuid::new_v4().to_string();
                            let retry_payload = crate::duck::payload::build_chat_payload(
                                candidate_model,
                                messages.to_vec(),
                                &self.keypair,
                                is_image_gen,
                                &retry_conversation_id,
                            );

                            if let Ok(retry_resp) = retry_req.json(&retry_payload).send().await {
                                if retry_resp.status().is_success() {
                                    if let Some(chained_vqd) = Self::extract_vqd_header(retry_resp.headers()) {
                                        self.store_chained_challenge(candidate_model, chained_vqd).await;
                                    }
                                    return Ok((retry_resp, candidate_model.clone()));
                                }
                                let retry_status = retry_resp.status();
                                let retry_body = retry_resp.text().await.unwrap_or_default();
                                tracing::warn!("Anomaly retry for '{}' failed with HTTP {}: {}", candidate_model, retry_status, retry_body);
                            }

                            last_err = Some(AppError::bad_gateway(format!(
                                "Duck.ai challenge rejected (418) for model '{}'",
                                candidate_model
                            )));
                            continue;
                        }

                        if !resp.status().is_success() {
                            let status = resp.status();
                            tracing::warn!("Duck.ai model '{}' returned HTTP {}, checking next in fallback chain...", candidate_model, status);
                            last_err = Some(AppError::bad_gateway(format!(
                                "Duck.ai chat request failed with HTTP {}",
                                status
                            )));
                            continue;
                        }

                        // Store chained challenge token from response headers
                        if let Some(chained) = Self::extract_vqd_header(resp.headers()) {
                            self.store_chained_challenge(candidate_model, chained).await;
                        }

                        return Ok((resp, candidate_model.clone()));
                    }
                    Err(err) => {
                        last_err = Some(AppError::from(err));
                    }
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            AppError::upstream_rate_limit("All models in fallback chain were rate limited", Some(4))
        }))
    }

    /// Sends a chat request with an optional pre-solved challenge.
    pub async fn send_chat_request_with_vqd(
        &self,
        payload: &DuckChatRequest,
        given_vqd: Option<String>,
    ) -> Result<reqwest::Response, AppError> {
        let url = format!("{}/duckchat/v1/chat", self.upstream_base_url);
        let model = payload.model.clone();

        for attempt in 0..MAX_RETRIES {
            let session = self.get_or_create_session(&model).await;
            let solved_vqd = match (attempt == 0, &given_vqd) {
                (true, Some(v)) => v.clone(),
                _ => match self.get_solved_challenge_header(&model, &session.journey_id).await {
                    Ok(vqd) => vqd,
                    Err(e) => {
                        if attempt < MAX_RETRIES - 1 {
                            self.force_rewarm(&session.journey_id).await;
                            tokio::time::sleep(tokio::time::Duration::from_millis(500 * (1 << attempt))).await;
                            continue;
                        }
                        return Err(e);
                    }
                },
            };

            let mut request = self.http
                .post(&url)
                .header("user-agent", &session.user_agent)
                .header("sec-ch-ua", sec_ch_ua_for_ua(&session.user_agent))
                .header("sec-ch-ua-platform", platform_for_ua(&session.user_agent))
                .header("sec-ch-ua-mobile", "?0")
                .header("x-vqd-hash-1", &solved_vqd)
                .header("origin", "https://duck.ai")
                .header("referer", "https://duck.ai/")
                .header("accept", "text/event-stream")
                .header("content-type", "application/json");

            for (key, value) in self.generate_telemetry_headers(&session.journey_id) {
                request = request.header(&key, &value);
            }

            let resp = request.json(payload).send().await?;

            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                tracing::warn!("Duck.ai model '{}' rate limited (HTTP 429), triggering immediate cascade...", model);
                self.reset_model_session(&model).await;
                return Err(AppError::upstream_rate_limit(
                    format!("Duck.ai model '{}' rate limited (HTTP 429)", model),
                    Some(4),
                ));
            }

            if resp.status().as_u16() == 418 {
                let body_str = resp.text().await.unwrap_or_default();
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&body_str) {
                    if let Some(cd) = val.get("cd") {
                        let q = cd.get("q").and_then(|v| v.as_str()).unwrap_or("");
                        let cc = cd.get("cc").and_then(|v| v.as_str()).unwrap_or("duckchat");
                        let s = cd.get("s").and_then(|v| v.as_str()).unwrap_or("index");
                        let r = cd.get("r").and_then(|v| v.as_str()).unwrap_or("euw");
                        let gk = cd.get("gk").and_then(|v| v.as_str()).unwrap_or("");
                        let p = cd.get("p").and_then(|v| v.as_str()).unwrap_or("");
                        let o = cd.get("o").and_then(|v| v.as_str()).unwrap_or("");

                        let p_parts: Vec<&str> = p.split('-').collect();
                        let acs = if p_parts.len() >= 4 {
                            format!("{}-{}", p_parts[0], p_parts[3])
                        } else if p_parts.len() >= 2 {
                            format!("{}-{}", p_parts[0], p_parts[1])
                        } else {
                            p.to_string()
                        };

                        let params = [
                            ("q", q),
                            ("type", "anomaly"),
                            ("acs", acs.as_str()),
                            ("cc", cc),
                            ("gk", gk),
                            ("p", p),
                            ("o", o),
                            ("s", s),
                            ("r", r),
                        ];

                        tracing::info!("Auto-resolving Duck.ai 418 anomaly for model '{}'...", model);
                        let _ = self.http.get(format!("{}/anomaly.js", self.upstream_base_url))
                            .query(&params)
                            .header("user-agent", &session.user_agent)
                            .header("referer", "https://duck.ai/")
                            .send()
                            .await;
                    }
                }

                tracing::warn!("Duck.ai challenge rejected (418) for model '{}', resetting session and cascading...", model);
                self.reset_model_session(&model).await;
                return Err(AppError::bad_gateway(format!(
                    "Duck.ai challenge rejected (418) for model '{}'",
                    model
                )));
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

    /// Sends a chat request to Duck.ai with automatic challenge solving and error recovery retry.
    pub async fn send_chat_request(
        &self,
        payload: &DuckChatRequest,
    ) -> Result<reqwest::Response, AppError> {
        self.send_chat_request_with_vqd(payload, None).await
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
