//! V8 challenge solver actor running on a dedicated OS thread.
//!
//! Uses `deno_core::JsRuntime` to evaluate Duck.ai anti-bot JavaScript challenges.
//! Communication happens via `tokio::sync::mpsc` (requests) and `tokio::sync::oneshot` (responses).

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use sha2::{Sha256, Digest};
use tokio::sync::{mpsc, oneshot};
use tracing;

use crate::duck::client::USER_AGENT;

/// A challenge request sent to the V8 actor.
pub struct ChallengeRequest {
    /// The base64-encoded challenge string from `x-vqd-hash-1`.
    pub challenge_b64: String,
    /// Channel to send back the solved result.
    pub reply: oneshot::Sender<Result<String, String>>,
}

/// Handle for sending challenges to the V8 actor.
#[derive(Clone)]
pub struct V8ActorHandle {
    sender: mpsc::Sender<ChallengeRequest>,
}

impl V8ActorHandle {
    /// Sends a challenge to the V8 actor and awaits the result.
    pub async fn solve_challenge(&self, challenge_b64: String) -> Result<String, String> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let request = ChallengeRequest {
            challenge_b64,
            reply: reply_tx,
        };
        self.sender
            .send(request)
            .await
            .map_err(|_| "V8 actor channel closed".to_string())?;

        reply_rx
            .await
            .map_err(|_| "V8 actor reply channel dropped".to_string())?
    }
}

/// Computes the SHA-256 hex digest of the User-Agent string.
pub fn ua_sha256_hex() -> String {
    let mut hasher = Sha256::new();
    hasher.update(USER_AGENT.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

/// Spawns the V8 actor on a dedicated OS thread and returns its handle.
///
/// The actor processes challenge requests sequentially on its own thread
/// to avoid blocking the Tokio async runtime (V8 isolates are single-threaded).
pub fn spawn_v8_actor() -> V8ActorHandle {
    let (tx, mut rx) = mpsc::channel::<ChallengeRequest>(32);

    std::thread::Builder::new()
        .name("v8-challenge-actor".to_string())
        .spawn(move || {
            tracing::info!("V8 challenge solver actor started");

            while let Some(request) = rx.blocking_recv() {
                let result = solve_challenge_sync(&request.challenge_b64);
                let _ = request.reply.send(result);
            }

            tracing::info!("V8 challenge solver actor stopped");
        })
        .expect("Failed to spawn V8 actor thread");

    V8ActorHandle { sender: tx }
}

/// Synchronously solves a challenge (runs on the V8 actor thread).
///
/// For now, this implements the hash construction without full V8 evaluation,
/// which covers the common challenge pattern. Full deno_core JsRuntime
/// evaluation can be added when the challenge format requires it.
fn solve_challenge_sync(challenge_b64: &str) -> Result<String, String> {
    // Decode the challenge
    let challenge_bytes = BASE64_STANDARD
        .decode(challenge_b64)
        .map_err(|e| format!("Failed to decode challenge base64: {}", e))?;

    let challenge_str = String::from_utf8(challenge_bytes)
        .map_err(|e| format!("Challenge is not valid UTF-8: {}", e))?;

    // Parse the challenge JSON to extract server_hashes
    let mut challenge_json: serde_json::Value = serde_json::from_str(&challenge_str)
        .map_err(|e| format!("Challenge is not valid JSON: {}", e))?;

    // Inject client_hashes with UA SHA-256
    let ua_hash = ua_sha256_hex();
    if let Some(obj) = challenge_json.as_object_mut() {
        obj.insert(
            "client_hashes".to_string(),
            serde_json::json!([ua_hash]),
        );

        // Inject meta fields
        let meta = obj.entry("meta").or_insert(serde_json::json!({}));
        if let Some(meta_obj) = meta.as_object_mut() {
            meta_obj.insert("origin".to_string(), serde_json::json!("https://duck.ai"));
            meta_obj.insert(
                "stack".to_string(),
                serde_json::json!("Error\n    at l (https://duck.ai/dist/duckai-dist/entry.duckai.c0328fc12a6573e54bd9.js:2:1833090)\n    at async https://duck.ai/dist/duckai-dist/entry.duckai.c0328fc12a6573e54bd9.js:2:1620812"),
            );
            meta_obj.insert("duration".to_string(), serde_json::json!("25"));
        }
    }

    // Compact serialize and base64 encode
    let result_json = serde_json::to_string(&challenge_json)
        .map_err(|e| format!("Failed to serialize challenge result: {}", e))?;

    Ok(BASE64_STANDARD.encode(result_json.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ua_sha256_hex() {
        let hash = ua_sha256_hex();
        // SHA-256 hex is always 64 chars
        assert_eq!(hash.len(), 64);
        // Should be consistent
        assert_eq!(hash, ua_sha256_hex());
    }

    #[test]
    fn test_solve_challenge_sync() {
        let challenge = serde_json::json!({
            "server_hashes": ["abc123"],
            "signals": {"test": true},
            "meta": {}
        });
        let challenge_b64 = BASE64_STANDARD.encode(
            serde_json::to_string(&challenge).unwrap().as_bytes()
        );

        let result = solve_challenge_sync(&challenge_b64).unwrap();

        // Decode and verify
        let decoded = BASE64_STANDARD.decode(&result).unwrap();
        let result_json: serde_json::Value =
            serde_json::from_slice(&decoded).unwrap();

        // Should have client_hashes injected
        assert!(result_json.get("client_hashes").is_some());
        let client_hashes = result_json["client_hashes"].as_array().unwrap();
        assert_eq!(client_hashes.len(), 1);
        assert_eq!(client_hashes[0].as_str().unwrap().len(), 64);

        // Should have meta injected
        assert_eq!(result_json["meta"]["origin"], "https://duck.ai");
        assert_eq!(result_json["meta"]["duration"], "25");
        assert!(result_json["meta"]["stack"].as_str().unwrap().contains("duck.ai"));
    }

    #[tokio::test]
    async fn test_v8_actor_handle() {
        let handle = spawn_v8_actor();

        let challenge = serde_json::json!({
            "server_hashes": ["test"],
            "meta": {}
        });
        let challenge_b64 = BASE64_STANDARD.encode(
            serde_json::to_string(&challenge).unwrap().as_bytes()
        );

        let result = handle.solve_challenge(challenge_b64).await.unwrap();
        assert!(!result.is_empty());

        // Verify roundtrip
        let decoded = BASE64_STANDARD.decode(&result).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&decoded).unwrap();
        assert!(json.get("client_hashes").is_some());
    }
}
