//! Virtual User management, identity isolation, and rate-limit mitigation for Duck.ai.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use reqwest::header::{HeaderMap, HeaderValue};

use crate::crypto::EphemeralKeypair;
use crate::error::AppError;
use crate::v8::V8ActorHandle;
use super::types::*;

pub const FE_VERSION: &str = "serp_20260901_082630_ET-936860e07343d04bca3ac6903356b645079e640f";

/// User-Agent rotation pool with realistic modern browser fingerprints across platforms.
pub const USER_AGENTS: &[&str] = &[
    // Linux Chrome 133
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36",
    // Windows Chrome 133
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36",
    // macOS Chrome 133
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36",
    // Windows Edge 133
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36 Edg/133.0.0.0",
    // macOS Safari 18
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_7_2) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.2 Safari/605.1.15",
    // Linux Firefox 135
    "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:135.0) Gecko/20100101 Firefox/135.0",
];

pub const USER_AGENT: &str = USER_AGENTS[0];

pub fn platform_for_ua(ua: &str) -> &'static str {
    if ua.contains("Windows") {
        "\"Windows\""
    } else if ua.contains("Macintosh") || ua.contains("Mac OS") {
        "\"macOS\""
    } else {
        "\"Linux\""
    }
}

pub fn sec_ch_ua_for_ua(ua: &str) -> &'static str {
    if ua.contains("Edg/") {
        r#""Not(A:Brand";v="99", "Microsoft Edge";v="133", "Chromium";v="133""#
    } else if ua.contains("Firefox") {
        r#""Not A(Brand";v="99", "Firefox";v="135""#
    } else if ua.contains("Safari") && !ua.contains("Chrome") {
        r#""Not A(Brand";v="99", "Safari";v="18""#
    } else {
        r#""Not(A:Brand";v="99", "Google Chrome";v="133", "Chromium";v="133""#
    }
}

/// Helper to build browser fingerprint headers for a given User-Agent.
pub fn build_browser_headers_for_ua(ua: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(ua) {
        headers.insert("user-agent", v);
    }
    headers.insert("accept-language", HeaderValue::from_static("en-US,en;q=0.9"));
    headers.insert("referer", HeaderValue::from_static("https://duck.ai/"));
    if let Ok(v) = HeaderValue::from_str(sec_ch_ua_for_ua(ua)) {
        headers.insert("sec-ch-ua", v);
    }
    headers.insert("sec-ch-ua-mobile", HeaderValue::from_static("?0"));
    if let Ok(v) = HeaderValue::from_str(platform_for_ua(ua)) {
        headers.insert("sec-ch-ua-platform", v);
    }
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

/// Per-model session state for a virtual user.
#[derive(Clone, Debug)]
pub struct ModelSession {
    pub journey_id: String,
    pub conversation_id: String,
    pub pending_challenge: Option<String>,
    pub user_agent: String,
}

impl ModelSession {
    pub fn new(ua: &str) -> Self {
        Self {
            user_agent: ua.to_string(),
            journey_id: uuid::Uuid::new_v4().simple().to_string(),
            conversation_id: uuid::Uuid::new_v4().to_string(),
            pending_challenge: None,
        }
    }

    pub fn reset(&mut self, ua: &str) {
        self.user_agent = ua.to_string();
        self.journey_id = uuid::Uuid::new_v4().simple().to_string();
        self.conversation_id = uuid::Uuid::new_v4().to_string();
        self.pending_challenge = None;
    }
}

impl Default for ModelSession {
    fn default() -> Self {
        Self::new(USER_AGENT)
    }
}

/// An isolated Virtual User identity with dedicated cryptographic keys and cookie storage.
#[derive(Clone)]
pub struct VirtualUser {
    pub id: String,
    pub keypair: EphemeralKeypair,
    pub http: reqwest::Client,
    pub status_http: reqwest::Client,
    pub sessions: Arc<RwLock<HashMap<String, ModelSession>>>,
    pub vqd_pool: Arc<Mutex<Vec<String>>>,
    pub warmed: Arc<RwLock<HashSet<String>>>,
    pub user_agent: String,
    pub rate_limited_until: Arc<RwLock<HashMap<String, chrono::DateTime<chrono::Utc>>>>,
    pub successful_requests: Arc<AtomicU64>,
    pub rate_limited_requests: Arc<AtomicU64>,
}

impl VirtualUser {
    pub fn new(id: &str, ua_index: usize) -> Self {
        let ua = USER_AGENTS[ua_index % USER_AGENTS.len()];
        let keypair = EphemeralKeypair::generate()
            .expect("Failed to generate ephemeral keypair for VirtualUser");

        let mut headers = build_browser_headers_for_ua(ua);
        headers.remove("host");

        let http = reqwest::Client::builder()
            .default_headers(headers.clone())
            .cookie_store(true)
            .build()
            .expect("Failed to build HTTP client for VirtualUser");

        let status_http = reqwest::Client::builder()
            .default_headers(headers)
            .cookie_store(true)
            .build()
            .expect("Failed to build status HTTP client for VirtualUser");

        Self {
            id: id.to_string(),
            keypair,
            http,
            status_http,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            vqd_pool: Arc::new(Mutex::new(Vec::new())),
            warmed: Arc::new(RwLock::new(HashSet::new())),
            user_agent: ua.to_string(),
            rate_limited_until: Arc::new(RwLock::new(HashMap::new())),
            successful_requests: Arc::new(AtomicU64::new(0)),
            rate_limited_requests: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Checks if this virtual user is currently blocked by a 429 rate limit on a specific model.
    pub async fn is_model_rate_limited(&self, model: &str) -> bool {
        let limits = self.rate_limited_until.read().await;
        if let Some(reset_at) = limits.get(model) {
            if chrono::Utc::now() < *reset_at {
                return true;
            }
        }
        false
    }

    /// Marks this virtual user as rate-limited for a model until the specified reset time.
    pub async fn mark_model_rate_limited(&self, model: &str, reset_at: Option<chrono::DateTime<chrono::Utc>>) {
        self.rate_limited_requests.fetch_add(1, Ordering::Relaxed);
        let reset = reset_at.unwrap_or_else(|| chrono::Utc::now() + chrono::Duration::hours(1));
        let mut limits = self.rate_limited_until.write().await;
        limits.insert(model.to_string(), reset);
    }

    /// Warms up session cookies and detects the frontend version for this virtual user.
    pub async fn warm(&self, upstream_base_url: &str, fe_version: &Arc<RwLock<String>>, journey_id: &str) {
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

        if let Ok(resp) = self.http.get(upstream_base_url).send().await {
            if let Ok(html) = resp.text().await {
                let tag = html.split("data-version-tag=\"")
                    .nth(1)
                    .and_then(|s| s.split('"').next());
                let sha = html.split("data-version-sha=\"")
                    .nth(1)
                    .and_then(|s| s.split('"').next());
                if let (Some(t), Some(s)) = (tag, sha) {
                    let dynamic_fe_version = format!("{}-{}", t, s);
                    let mut fe_ver = fe_version.write().await;
                    *fe_ver = dynamic_fe_version;
                }
            }
        }

        warmed_lock.insert(journey_id.to_string());
    }

    /// Forces a rewarm for a specific model session on this virtual user.
    pub async fn force_rewarm(&self, journey_id: &str) {
        let mut warmed = self.warmed.write().await;
        warmed.remove(journey_id);
        let mut sessions = self.sessions.write().await;
        for s in sessions.values_mut() {
            s.reset(&self.user_agent);
        }
        let mut pool = self.vqd_pool.lock().await;
        pool.clear();
    }

    /// Gets or creates a per-model session.
    pub async fn get_or_create_session(&self, model: &str) -> ModelSession {
        let mut sessions = self.sessions.write().await;
        sessions.entry(model.to_string()).or_insert_with(|| ModelSession::new(&self.user_agent)).clone()
    }

    /// Resets a specific model's session.
    pub async fn reset_model_session(&self, model: &str) {
        let mut sessions = self.sessions.write().await;
        let s = ModelSession::new(&self.user_agent);
        sessions.insert(model.to_string(), s);
    }

    /// Generates telemetry headers with natural human-like jitter.
    pub fn generate_telemetry_headers(&self, journey_id: &str, fe_version: &str) -> Vec<(String, String)> {
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

        vec![
            ("x-fe-version".to_string(), fe_version.to_string()),
            ("x-ddg-journey-id".to_string(), journey_id.to_string()),
            ("x-fe-signals".to_string(), signals_b64),
        ]
    }

    /// Fetches an initial raw VQD challenge from /duckchat/v1/status.
    pub async fn fetch_raw_status_challenge(
        &self,
        upstream_base_url: &str,
        _journey_id: &str,
        status_lock: &Arc<Mutex<()>>,
        last_status_call: &Arc<Mutex<Option<tokio::time::Instant>>>,
    ) -> Result<String, AppError> {
        let _guard = status_lock.lock().await;

        // Space out status calls to prevent connection throttling
        {
            let mut last_call = last_status_call.lock().await;
            if let Some(prev) = *last_call {
                let elapsed = prev.elapsed();
                if elapsed < tokio::time::Duration::from_millis(2500) {
                    let sleep_dur = tokio::time::Duration::from_millis(2500) - elapsed;
                    tokio::time::sleep(sleep_dur).await;
                }
            }
            *last_call = Some(tokio::time::Instant::now());
        }

        let url = format!("{}/duckchat/v1/status", upstream_base_url);

        let req = self.status_http
            .get(&url)
            .header("user-agent", &self.user_agent)
            .header("sec-ch-ua", sec_ch_ua_for_ua(&self.user_agent))
            .header("sec-ch-ua-platform", platform_for_ua(&self.user_agent))
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
                tracing::info!("VirtualUser '{}': Using pooled VQD challenge on status 429", self.id);
                return Ok(token);
            }
            return Err(AppError::upstream_rate_limit(
                format!("Duck.ai status rate limit exceeded for VirtualUser '{}'", self.id),
                Some(4),
            ));
        }

        if !resp.status().is_success() {
            return Err(AppError::bad_gateway(format!(
                "VQD status request failed with HTTP {} for VirtualUser '{}'",
                resp.status(),
                self.id
            )));
        }

        if let Some(raw_challenge) = extract_vqd_header(resp.headers()) {
            return Ok(raw_challenge);
        }

        Err(AppError::bad_gateway("No x-vqd challenge returned in status response"))
    }

    /// Solves a VQD challenge for the requested model.
    pub async fn get_solved_challenge_header(
        &self,
        upstream_base_url: &str,
        model: &str,
        journey_id: &str,
        v8_actor: &V8ActorHandle,
        status_lock: &Arc<Mutex<()>>,
        last_status_call: &Arc<Mutex<Option<tokio::time::Instant>>>,
    ) -> Result<String, AppError> {
        let from_pool = {
            let mut pool = self.vqd_pool.lock().await;
            pool.pop()
        };

        let raw = match from_pool {
            Some(c) => c,
            None => self.fetch_raw_status_challenge(upstream_base_url, journey_id, status_lock, last_status_call).await?,
        };

        {
            let mut sessions = self.sessions.write().await;
            let s = sessions.entry(model.to_string()).or_insert_with(|| ModelSession::new(&self.user_agent));
            s.conversation_id = uuid::Uuid::new_v4().to_string();
        }

        v8_actor.solve_challenge_with_ua(raw, Some(self.user_agent.clone())).await
            .map_err(|e| AppError::bad_gateway(format!("V8 Challenge solver error: {}", e)))
    }
}

/// Pool of Virtual Users managing round-robin distribution, failover, and dynamic instantiation.
#[derive(Clone)]
pub struct VirtualUserPool {
    users: Arc<RwLock<Vec<VirtualUser>>>,
    active_index: Arc<AtomicUsize>,
    dynamic_counter: Arc<AtomicUsize>,
    pool_size: usize,
}

impl VirtualUserPool {
    pub fn new(pool_size: usize) -> Self {
        let initial_size = pool_size.max(1);
        let mut users = Vec::with_capacity(initial_size);
        for i in 0..initial_size {
            users.push(VirtualUser::new(&format!("vu-{}", i + 1), i));
        }

        Self {
            users: Arc::new(RwLock::new(users)),
            active_index: Arc::new(AtomicUsize::new(0)),
            dynamic_counter: Arc::new(AtomicUsize::new(initial_size + 1)),
            pool_size: initial_size,
        }
    }

    /// Selects an unblocked virtual user for the requested model.
    pub async fn select_user_for_model(&self, model: &str, preferred_user_id: Option<&str>) -> VirtualUser {
        // 1. If explicit user ID requested, try to find it
        if let Some(pid) = preferred_user_id {
            let users = self.users.read().await;
            if let Some(u) = users.iter().find(|u| u.id == pid) {
                return u.clone();
            }
        }

        // 2. Try to find an unblocked user in the existing pool starting from active_index
        let users = self.users.read().await;
        let total = users.len();
        let start = self.active_index.load(Ordering::Relaxed) % total;

        for offset in 0..total {
            let idx = (start + offset) % total;
            let u = &users[idx];
            if !u.is_model_rate_limited(model).await {
                self.active_index.store(idx, Ordering::Relaxed);
                return u.clone();
            }
        }
        drop(users);

        // 3. All existing users are rate-limited on this model: dynamically spawn a brand new Virtual User!
        let new_num = self.dynamic_counter.fetch_add(1, Ordering::Relaxed);
        let new_id = format!("vu-dyn-{}", new_num);
        tracing::warn!(
            "⚡ All {} virtual users are rate-limited on '{}'. Dynamically spawning new Virtual User '{}'...",
            total,
            model,
            new_id
        );
        let new_user = VirtualUser::new(&new_id, new_num);
        let mut write_guard = self.users.write().await;
        write_guard.push(new_user.clone());
        let new_idx = write_guard.len() - 1;
        self.active_index.store(new_idx, Ordering::Relaxed);
        new_user
    }

    /// Rotates away from a rate-limited virtual user and selects the next available one.
    pub async fn rotate_on_rate_limit(
        &self,
        current_user_id: &str,
        model: &str,
        reset_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> VirtualUser {
        {
            let users = self.users.read().await;
            if let Some(u) = users.iter().find(|u| u.id == current_user_id) {
                u.mark_model_rate_limited(model, reset_at).await;
            }
        }

        self.active_index.fetch_add(1, Ordering::Relaxed);
        self.select_user_for_model(model, None).await
    }

    /// Returns the initial configured pool size.
    pub fn pool_size(&self) -> usize {
        self.pool_size
    }

    /// Returns the total number of virtual users currently in the pool.
    pub async fn total_users(&self) -> usize {
        let users = self.users.read().await;
        users.len()
    }


    /// Returns a list of Virtual User snapshots for dashboard monitoring.
    pub async fn get_snapshots(&self) -> Vec<VirtualUserSnapshot> {
        let users = self.users.read().await;
        let mut snapshots = Vec::new();
        let now = chrono::Utc::now();
        for u in users.iter() {
            let limits = u.rate_limited_until.read().await;
            let mut rate_limits = HashMap::new();
            for (m, dt) in limits.iter() {
                if *dt > now {
                    rate_limits.insert(m.clone(), dt.to_rfc3339());
                }
            }
            snapshots.push(VirtualUserSnapshot {
                id: u.id.clone(),
                user_agent: u.user_agent.clone(),
                rate_limited_models: rate_limits,
                successful_requests: u.successful_requests.load(Ordering::Relaxed),
                rate_limited_requests: u.rate_limited_requests.load(Ordering::Relaxed),
            });
        }
        snapshots
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct VirtualUserSnapshot {
    pub id: String,
    pub user_agent: String,
    pub rate_limited_models: HashMap<String, String>,
    pub successful_requests: u64,
    pub rate_limited_requests: u64,
}
