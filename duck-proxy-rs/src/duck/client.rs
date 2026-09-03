//! Duck.ai HTTP client with Virtual User identity pool, VQD token chaining, telemetry, and V8 challenge solving.

use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use reqwest::header::{HeaderMap, HeaderValue};


use crate::crypto::EphemeralKeypair;
use crate::error::AppError;
use crate::v8::{spawn_v8_actor, V8ActorHandle};
use super::types::*;
pub use super::virtual_user::{
    build_browser_headers_for_ua, extract_vqd_header, platform_for_ua, sec_ch_ua_for_ua,
    VirtualUser, VirtualUserPool, FE_VERSION, USER_AGENT, USER_AGENTS,
};


/// Maximum retry attempts for chat requests.
const MAX_RETRIES: u32 = 3;

/// Upstream Duck.ai client with Virtual User identity pool and concurrency management.
#[derive(Clone)]
pub struct DuckClient {
    user_pool: Arc<VirtualUserPool>,
    upstream_base_url: String,
    fe_version: Arc<RwLock<String>>,
    status_lock: Arc<Mutex<()>>,
    chat_lock: Arc<Mutex<()>>,
    last_status_call: Arc<Mutex<Option<tokio::time::Instant>>>,
    last_chat_call: Arc<Mutex<Option<tokio::time::Instant>>>,
    v8_actor: V8ActorHandle,
}

impl DuckClient {
    /// Creates a new DuckClient with default Virtual User pool and spawned V8 challenge actor.
    pub fn new(upstream_base_url: &str) -> Self {
        Self::with_pool_size(upstream_base_url, 5)
    }

    /// Creates a new DuckClient with a specific Virtual User pool size.
    pub fn with_pool_size(upstream_base_url: &str, pool_size: usize) -> Self {
        let v8_actor = spawn_v8_actor();
        Self::with_v8_actor_and_pool_size(upstream_base_url, v8_actor, pool_size)
    }

    /// Creates a new DuckClient with a custom V8ActorHandle and default pool size.
    pub fn with_v8_actor(upstream_base_url: &str, v8_actor: V8ActorHandle) -> Self {
        Self::with_v8_actor_and_pool_size(upstream_base_url, v8_actor, 5)
    }

    /// Creates a new DuckClient with custom V8ActorHandle and custom pool size.
    pub fn with_v8_actor_and_pool_size(upstream_base_url: &str, v8_actor: V8ActorHandle, pool_size: usize) -> Self {
        let user_pool = Arc::new(VirtualUserPool::new(pool_size));

        Self {
            user_pool,
            upstream_base_url: upstream_base_url.trim_end_matches('/').to_string(),
            fe_version: Arc::new(RwLock::new(FE_VERSION.to_string())),
            status_lock: Arc::new(Mutex::new(())),
            chat_lock: Arc::new(Mutex::new(())),
            last_status_call: Arc::new(Mutex::new(None)),
            last_chat_call: Arc::new(Mutex::new(None)),
            v8_actor,
        }
    }

    /// Access the Virtual User pool.
    pub fn user_pool(&self) -> &VirtualUserPool {
        &self.user_pool
    }

    /// Starts a background token prefetcher (disabled to prevent upstream rate limits).
    pub fn start_background_pool_worker(self: &Arc<Self>) {
        // Disabled background polling to respect Duck.ai rate limits
    }

    /// Returns the browser fingerprint headers required by Duck.ai.
    pub fn browser_headers() -> HeaderMap {
        build_browser_headers_for_ua(USER_AGENT)
    }

    /// Extracts the challenge token from multiple potential response header names.
    pub fn extract_vqd_header(headers: &HeaderMap) -> Option<String> {
        extract_vqd_header(headers)
    }

    /// Warms up session cookies for the primary virtual user.
    pub async fn warm(&self, journey_id: &str) {
        let active_user = self.user_pool.select_user_for_model("default", None).await;
        active_user.warm(&self.upstream_base_url, &self.fe_version, journey_id).await;
    }

    /// Forces a rewarm on the next request.
    pub async fn force_rewarm(&self, journey_id: &str) {
        let active_user = self.user_pool.select_user_for_model("default", None).await;
        active_user.force_rewarm(journey_id).await;
    }

    /// Gets or creates a per-model session ensuring unique journey ID on the active virtual user.
    pub async fn get_or_create_session(&self, model: &str) -> super::virtual_user::ModelSession {
        let active_user = self.user_pool.select_user_for_model(model, None).await;
        active_user.get_or_create_session(model).await
    }

    /// Generates telemetry headers for a chat request with natural jitter.
    pub fn generate_telemetry_headers(&self, journey_id: &str) -> Vec<(String, String)> {
        let fe_version = self
            .fe_version
            .try_read()
            .map(|v| v.clone())
            .unwrap_or_else(|_| FE_VERSION.to_string());
        let active_user = VirtualUser::new("temp", 0);
        active_user.generate_telemetry_headers(journey_id, &fe_version)
    }

    /// Fetches an initial raw VQD challenge from /duckchat/v1/status with resilient backoff.
    pub async fn fetch_raw_status_challenge(&self, journey_id: &str, _user_agent: &str) -> Result<String, AppError> {
        let active_user = self.user_pool.select_user_for_model("default", None).await;
        active_user.fetch_raw_status_challenge(
            &self.upstream_base_url,
            journey_id,
            &self.status_lock,
            &self.last_status_call,
        ).await
    }

    /// Gets and solves a fresh VQD challenge header for the specified model.
    pub async fn get_solved_challenge_header(&self, model: &str, journey_id: &str) -> Result<String, AppError> {
        let active_user = self.user_pool.select_user_for_model(model, None).await;
        active_user.get_solved_challenge_header(
            &self.upstream_base_url,
            model,
            journey_id,
            &self.v8_actor,
            &self.status_lock,
            &self.last_status_call,
        ).await
    }

    /// Sends a chat request to Duck.ai with automatic challenge solving, Virtual User switching, and candidate model fallback cascade.
    pub async fn send_chat_request_cascade(
        &self,
        requested_model: &str,
        messages: &[DuckChatMessage],
        fallback_chain: &[String],
        is_image_gen: bool,
    ) -> Result<(reqwest::Response, String), AppError> {
        let _chat_guard = self.chat_lock.lock().await;
        let mut last_err = None;

        for (idx, candidate_model) in fallback_chain.iter().enumerate() {
            // For each candidate model, attempt request across virtual users with automatic rotation on 429
            const MAX_VU_ATTEMPTS: usize = 4;
            let mut current_vu = self.user_pool.select_user_for_model(candidate_model, None).await;

            for vu_attempt in 0..MAX_VU_ATTEMPTS {
                current_vu.warm(&self.upstream_base_url, &self.fe_version, candidate_model).await;
                let session = current_vu.get_or_create_session(candidate_model).await;

                let solved_vqd = match current_vu.get_solved_challenge_header(
                    &self.upstream_base_url,
                    candidate_model,
                    &session.journey_id,
                    &self.v8_actor,
                    &self.status_lock,
                    &self.last_status_call,
                ).await {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!(
                            "VirtualUser '{}' failed to obtain VQD challenge for '{}': {:?}. Rotating to next virtual user...",
                            current_vu.id,
                            candidate_model,
                            e
                        );
                        current_vu = self.user_pool.rotate_on_rate_limit(&current_vu.id, candidate_model, None).await;
                        last_err = Some(e);
                        continue;
                    }
                };

                let fresh_conversation_id = uuid::Uuid::new_v4().to_string();
                let payload = crate::duck::payload::build_chat_payload(
                    candidate_model,
                    messages.to_vec(),
                    &current_vu.keypair,
                    is_image_gen,
                    &fresh_conversation_id,
                );

                if idx > 0 || vu_attempt > 0 {
                    tracing::warn!(
                        "Chat request attempt (VU: {}, attempt {}/{}) from '{}' to candidate model '{}'...",
                        current_vu.id,
                        vu_attempt + 1,
                        MAX_VU_ATTEMPTS,
                        requested_model,
                        candidate_model
                    );
                }

                // Minimal spacing between requests to prevent socket congestion
                let min_spacing_ms = if idx > 0 || vu_attempt > 0 { 200 } else { 1500 };
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
                let fe_version = self.fe_version.read().await.clone();
                let mut request = current_vu.http
                    .post(&url)
                    .header("user-agent", &current_vu.user_agent)
                    .header("sec-ch-ua", sec_ch_ua_for_ua(&current_vu.user_agent))
                    .header("sec-ch-ua-platform", platform_for_ua(&current_vu.user_agent))
                    .header("sec-ch-ua-mobile", "?0")
                    .header("x-vqd-hash-1", &solved_vqd)
                    .header("origin", "https://duck.ai")
                    .header("referer", "https://duck.ai/")
                    .header("accept", "text/event-stream")
                    .header("content-type", "application/json");

                for (key, value) in current_vu.generate_telemetry_headers(&session.journey_id, &fe_version) {
                    request = request.header(&key, &value);
                }

                match request.json(&payload).send().await {
                    Ok(resp) => {
                        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                            let body_429 = resp.text().await.unwrap_or_default();
                            tracing::warn!(
                                "Duck.ai model '{}' rate limited (HTTP 429) for VirtualUser '{}'. Upstream body: {}",
                                candidate_model,
                                current_vu.id,
                                body_429
                            );

                            let reset_at = serde_json::from_str::<serde_json::Value>(&body_429)
                                .ok()
                                .and_then(|v| {
                                    v.pointer("/fixedCostWindowUsage/windows/0/resetAt")
                                        .and_then(|r| r.as_str())
                                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                                        .map(|dt| dt.with_timezone(&chrono::Utc))
                                });

                            // Automatically rotate to next virtual user with fresh keypair & cookie jar
                            current_vu = self.user_pool.rotate_on_rate_limit(&current_vu.id, candidate_model, reset_at).await;
                            tracing::info!(
                                "🔄 Switched to VirtualUser '{}' to bypass rate limit for model '{}'",
                                current_vu.id,
                                candidate_model
                            );
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

                                    tracing::info!("Auto-resolving Duck.ai 418 anomaly for VirtualUser '{}' on model '{}'...", current_vu.id, candidate_model);
                                    let _ = current_vu.http.get(format!("{}/anomaly.js", self.upstream_base_url))
                                        .query(&params)
                                        .header("user-agent", &current_vu.user_agent)
                                        .header("referer", "https://duck.ai/")
                                        .send()
                                        .await;
                                }
                            }

                            tracing::warn!("Resetting session and warming new journey after 418 for VirtualUser '{}' on model '{}'...", current_vu.id, candidate_model);
                            current_vu.reset_model_session(candidate_model).await;
                            let fresh_session = current_vu.get_or_create_session(candidate_model).await;
                            current_vu.warm(&self.upstream_base_url, &self.fe_version, &fresh_session.journey_id).await;
                            tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

                            let fresh_vqd = match current_vu.get_solved_challenge_header(
                                &self.upstream_base_url,
                                candidate_model,
                                &fresh_session.journey_id,
                                &self.v8_actor,
                                &self.status_lock,
                                &self.last_status_call,
                            ).await {
                                Ok(v) => v,
                                Err(_) => solved_vqd.clone(),
                            };

                            let mut retry_req = current_vu.http
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

                            for (key, value) in current_vu.generate_telemetry_headers(&fresh_session.journey_id, &fe_version) {
                                retry_req = retry_req.header(&key, &value);
                            }

                            let retry_conversation_id = uuid::Uuid::new_v4().to_string();
                            let retry_payload = crate::duck::payload::build_chat_payload(
                                candidate_model,
                                messages.to_vec(),
                                &current_vu.keypair,
                                is_image_gen,
                                &retry_conversation_id,
                            );

                            if let Ok(retry_resp) = retry_req.json(&retry_payload).send().await {
                                if retry_resp.status().is_success() {
                                    if let Some(chained_vqd) = extract_vqd_header(retry_resp.headers()) {
                                        let mut pool = current_vu.vqd_pool.lock().await;
                                        pool.push(chained_vqd);
                                    }
                                    current_vu.successful_requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                    return Ok((retry_resp, candidate_model.clone()));
                                }
                            }

                            // Rotate to next virtual user to bypass sticky 418 challenge
                            current_vu = self.user_pool.rotate_on_rate_limit(&current_vu.id, candidate_model, None).await;
                            tracing::warn!("🔄 Switched to VirtualUser '{}' after 418 challenge on model '{}'", current_vu.id, candidate_model);

                            last_err = Some(AppError::bad_gateway(format!(
                                "Duck.ai challenge rejected (418) for model '{}'",
                                candidate_model
                            )));
                            continue;
                        }


                        if !resp.status().is_success() {
                            let status = resp.status();
                            tracing::warn!("Duck.ai model '{}' returned HTTP {}, checking next...", candidate_model, status);
                            last_err = Some(AppError::bad_gateway(format!(
                                "Duck.ai chat request failed with HTTP {}",
                                status
                            )));
                            continue;
                        }

                        // Store chained challenge token from response headers
                        if let Some(chained) = extract_vqd_header(resp.headers()) {
                            let mut pool = current_vu.vqd_pool.lock().await;
                            pool.push(chained);
                        }

                        current_vu.successful_requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        return Ok((resp, candidate_model.clone()));
                    }
                    Err(err) => {
                        last_err = Some(AppError::from(err));
                    }
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            AppError::upstream_rate_limit("All models and virtual users in fallback chain were rate limited", Some(4))
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
        let active_user = self.user_pool.select_user_for_model(&model, None).await;

        for attempt in 0..MAX_RETRIES {
            let session = active_user.get_or_create_session(&model).await;
            let solved_vqd = match (attempt == 0, &given_vqd) {
                (true, Some(v)) => v.clone(),
                _ => match active_user.get_solved_challenge_header(
                    &self.upstream_base_url,
                    &model,
                    &session.journey_id,
                    &self.v8_actor,
                    &self.status_lock,
                    &self.last_status_call,
                ).await {
                    Ok(vqd) => vqd,
                    Err(e) => {
                        if attempt < MAX_RETRIES - 1 {
                            active_user.force_rewarm(&session.journey_id).await;
                            tokio::time::sleep(tokio::time::Duration::from_millis(500 * (1 << attempt))).await;
                            continue;
                        }
                        return Err(e);
                    }
                },
            };

            let fe_version = self.fe_version.read().await.clone();
            let mut request = active_user.http
                .post(&url)
                .header("user-agent", &active_user.user_agent)
                .header("sec-ch-ua", sec_ch_ua_for_ua(&active_user.user_agent))
                .header("sec-ch-ua-platform", platform_for_ua(&active_user.user_agent))
                .header("sec-ch-ua-mobile", "?0")
                .header("x-vqd-hash-1", &solved_vqd)
                .header("origin", "https://duck.ai")
                .header("referer", "https://duck.ai/")
                .header("accept", "text/event-stream")
                .header("content-type", "application/json");

            for (key, value) in active_user.generate_telemetry_headers(&session.journey_id, &fe_version) {
                request = request.header(&key, &value);
            }

            let resp = request.json(payload).send().await?;

            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                tracing::warn!("Duck.ai model '{}' rate limited (HTTP 429), triggering rotation...", model);
                active_user.mark_model_rate_limited(&model, None).await;
                return Err(AppError::upstream_rate_limit(
                    format!("Duck.ai model '{}' rate limited (HTTP 429)", model),
                    Some(4),
                ));
            }

            if resp.status().as_u16() == 418 {
                tracing::warn!("Duck.ai challenge rejected (418) for model '{}', resetting session...", model);
                active_user.reset_model_session(&model).await;
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
            if let Some(new_vqd) = extract_vqd_header(resp.headers()) {
                let mut pool = active_user.vqd_pool.lock().await;
                pool.push(new_vqd);
            }

            active_user.successful_requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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

    /// Returns a fresh ephemeral keypair.
    pub fn keypair(&self) -> EphemeralKeypair {
        EphemeralKeypair::generate().expect("Failed to generate ephemeral keypair")
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
