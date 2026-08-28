//! Custom wiremock matchers for verifying Duck.ai upstream protocol headers,
//! telemetry payloads, and durableStream RSA JWK public keys.

#![allow(dead_code)]

use base64::Engine;
use serde_json::Value;
use wiremock::{Match, Request};

/// Matcher that validates the full suite of Duck.ai telemetry and security headers.
pub struct DuckHeadersMatcher {
    pub check_vqd_hash: Option<String>,
}

impl DuckHeadersMatcher {
    pub fn new() -> Self {
        Self {
            check_vqd_hash: None,
        }
    }

    pub fn with_vqd(vqd: impl Into<String>) -> Self {
        Self {
            check_vqd_hash: Some(vqd.into()),
        }
    }
}

impl Default for DuckHeadersMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Match for DuckHeadersMatcher {
    fn matches(&self, request: &Request) -> bool {
        // 1. User-Agent must be Chrome/150 spoof containing BOTH Mozilla/5.0 and Chrome/150
        let ua = match request.headers.get("user-agent") {
            Some(v) => match v.to_str() {
                Ok(s) => s,
                Err(_) => return false,
            },
            None => return false,
        };
        if !ua.contains("Chrome/150") || !ua.contains("Mozilla/5.0") {
            return false;
        }

        // 2. Telemetry: x-fe-version must be present
        if !request.headers.contains_key("x-fe-version") {
            return false;
        }

        // 3. Telemetry: x-ddg-journey-id must be 32 hex chars
        let journey_id = match request.headers.get("x-ddg-journey-id") {
            Some(v) => match v.to_str() {
                Ok(s) => s,
                Err(_) => return false,
            },
            None => return false,
        };
        if journey_id.len() != 32 || !journey_id.chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }

        // 4. Telemetry: x-fe-signals must be valid base64 JSON
        let signals_b64 = match request.headers.get("x-fe-signals") {
            Some(v) => match v.to_str() {
                Ok(s) => s,
                Err(_) => return false,
            },
            None => return false,
        };
        let signals_bytes = match base64::engine::general_purpose::STANDARD.decode(signals_b64) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let signals_json: Value = match serde_json::from_slice(&signals_bytes) {
            Ok(j) => j,
            Err(_) => return false,
        };
        if signals_json.get("start").is_none() || signals_json.get("events").is_none() {
            return false;
        }

        // 5. sec-ch-ua headers
        if !request.headers.contains_key("sec-ch-ua")
            || !request.headers.contains_key("sec-ch-ua-mobile")
            || !request.headers.contains_key("sec-ch-ua-platform")
        {
            return false;
        }

        // 6. Optional x-vqd-hash-1 check
        if let Some(ref expected_vqd) = self.check_vqd_hash {
            let actual_vqd = match request.headers.get("x-vqd-hash-1") {
                Some(v) => match v.to_str() {
                    Ok(s) => s,
                    Err(_) => return false,
                },
                None => return false,
            };
            if actual_vqd != expected_vqd {
                return false;
            }
        }

        true
    }
}

/// Matcher that validates the Duck.ai chat JSON payload structure, model, messages,
/// and durableStream.publicKey RSA JWK format.
pub struct DuckPayloadMatcher {
    pub expected_model: Option<String>,
    pub expect_generate_image: bool,
    pub min_messages: usize,
}

impl DuckPayloadMatcher {
    pub fn for_model(model: impl Into<String>) -> Self {
        Self {
            expected_model: Some(model.into()),
            expect_generate_image: false,
            min_messages: 1,
        }
    }

    pub fn for_image() -> Self {
        Self {
            expected_model: Some("gpt-5.6-luna".to_string()),
            expect_generate_image: true,
            min_messages: 1,
        }
    }

    pub fn any_model() -> Self {
        Self {
            expected_model: None,
            expect_generate_image: false,
            min_messages: 1,
        }
    }
}

impl Match for DuckPayloadMatcher {
    fn matches(&self, request: &Request) -> bool {
        let json: Value = match serde_json::from_slice(&request.body) {
            Ok(v) => v,
            Err(_) => return false,
        };

        // 1. Model match
        if let Some(ref m) = self.expected_model {
            if json.get("model").and_then(|v| v.as_str()) != Some(m.as_str()) {
                return false;
            }
        }

        // 2. Messages array
        let messages = match json.get("messages").and_then(|v| v.as_array()) {
            Some(arr) if arr.len() >= self.min_messages => arr,
            _ => return false,
        };
        for msg in messages {
            if msg.get("role").and_then(|v| v.as_str()).is_none() {
                return false;
            }
        }

        // 3. Image Generation metadata check
        if self.expect_generate_image {
            let gen_img = json
                .pointer("/metadata/toolChoice/GenerateImage")
                .and_then(|v| v.as_bool());
            if gen_img != Some(true) {
                return false;
            }
        }

        // 4. Check durableStream.publicKey JWK format
        let jwk = match json.pointer("/durableStream/publicKey") {
            Some(j) => j,
            None => return false,
        };

        if jwk.get("kty").and_then(|v| v.as_str()) != Some("RSA")
            || jwk.get("alg").and_then(|v| v.as_str()) != Some("RSA-OAEP-256")
            || jwk.get("use").and_then(|v| v.as_str()) != Some("enc")
            || jwk.get("e").and_then(|v| v.as_str()) != Some("AQAB")
            || jwk
                .get("n")
                .and_then(|v| v.as_str())
                .map_or(true, |n| n.is_empty() || n.contains('='))
        {
            return false;
        }

        true
    }
}

/// Matcher for solved V8 Challenge header validating 64-character SHA-256 hex hash
/// and full metadata fields (`origin`, `stack`, `duration`).
pub struct SolvedV8ChallengeMatcher;

impl Match for SolvedV8ChallengeMatcher {
    fn matches(&self, request: &Request) -> bool {
        let vqd = match request.headers.get("x-vqd-hash-1") {
            Some(v) => match v.to_str() {
                Ok(s) => s,
                Err(_) => return false,
            },
            None => return false,
        };

        // Must decode to valid JSON with client_hashes and meta
        let bytes = match base64::engine::general_purpose::STANDARD.decode(vqd) {
            Ok(b) => b,
            Err(_) => return false,
        };

        let json: Value = match serde_json::from_slice(&bytes) {
            Ok(j) => j,
            Err(_) => return false,
        };

        // 1. client_hashes array must have at least one element
        let hashes = match json.get("client_hashes").and_then(|v| v.as_array()) {
            Some(h) if !h.is_empty() => h,
            _ => return false,
        };

        // 2. First hash must be a valid 64-character hexadecimal SHA-256 string
        let first_hash = match hashes[0].as_str() {
            Some(s) if s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit()) => s,
            _ => return false,
        };
        if first_hash.is_empty() {
            return false;
        }

        // 3. meta.origin must be present and non-empty URL
        let origin = match json.pointer("/meta/origin").and_then(|v| v.as_str()) {
            Some(o) if !o.is_empty() => o,
            _ => return false,
        };
        if !origin.starts_with("http://") && !origin.starts_with("https://") {
            return false;
        }

        // 4. meta.stack must be present and contain error stack information
        let stack = match json.pointer("/meta/stack").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s,
            _ => return false,
        };
        if !stack.contains("Error") && !stack.contains("duckai") && !stack.contains("https://") {
            return false;
        }

        // 5. meta.duration must be present as a non-empty string or number
        let duration_valid = match json.pointer("/meta/duration") {
            Some(Value::String(s)) => !s.is_empty(),
            Some(Value::Number(_)) => true,
            _ => false,
        };
        if !duration_valid {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use wiremock::http::{HeaderMap, HeaderName, HeaderValue, Method};

    fn make_test_request(headers: &[(&str, &str)], body: &[u8]) -> Request {
        let mut header_map = HeaderMap::new();
        for (k, v) in headers {
            header_map.insert(
                HeaderName::from_str(k).unwrap(),
                HeaderValue::from_str(v).unwrap(),
            );
        }
        Request {
            url: "http://127.0.0.1/duckchat/v1/chat".parse().unwrap(),
            method: Method::POST,
            headers: header_map,
            body: body.to_vec(),
        }
    }

    #[test]
    fn test_user_agent_matcher_strictness() {
        let matcher = DuckHeadersMatcher::new();
        let valid_signals = base64::engine::general_purpose::STANDARD
            .encode(r#"{"start":1724867000,"events":[{"e":"action","t":10}],"end":20}"#);

        // 1. Valid Chrome/150 UA
        let req_valid = make_test_request(
            &[
                (
                    "user-agent",
                    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
                ),
                ("x-fe-version", "serp_20260827"),
                ("x-ddg-journey-id", "0123456789abcdef0123456789abcdef"),
                ("x-fe-signals", &valid_signals),
                ("sec-ch-ua", r#""Chromium";v="150""#),
                ("sec-ch-ua-mobile", "?0"),
                ("sec-ch-ua-platform", r#""Linux""#),
            ],
            b"{}",
        );
        assert!(matcher.matches(&req_valid));

        // 2. Invalid Firefox UA (contains Mozilla/5.0 but NOT Chrome/150)
        let req_firefox = make_test_request(
            &[
                (
                    "user-agent",
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:109.0) Gecko/20100101 Firefox/119.0",
                ),
                ("x-fe-version", "serp_20260827"),
                ("x-ddg-journey-id", "0123456789abcdef0123456789abcdef"),
                ("x-fe-signals", &valid_signals),
                ("sec-ch-ua", r#""Chromium";v="150""#),
                ("sec-ch-ua-mobile", "?0"),
                ("sec-ch-ua-platform", r#""Linux""#),
            ],
            b"{}",
        );
        assert!(!matcher.matches(&req_firefox));

        // 3. Invalid Safari UA (contains Mozilla/5.0 but NOT Chrome/150)
        let req_safari = make_test_request(
            &[
                (
                    "user-agent",
                    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15",
                ),
                ("x-fe-version", "serp_20260827"),
                ("x-ddg-journey-id", "0123456789abcdef0123456789abcdef"),
                ("x-fe-signals", &valid_signals),
                ("sec-ch-ua", r#""Chromium";v="150""#),
                ("sec-ch-ua-mobile", "?0"),
                ("sec-ch-ua-platform", r#""Linux""#),
            ],
            b"{}",
        );
        assert!(!matcher.matches(&req_safari));

        // 4. Missing Mozilla/5.0
        let req_no_moz = make_test_request(
            &[
                ("user-agent", "Chrome/150.0.0.0 CustomBot/1.0"),
                ("x-fe-version", "serp_20260827"),
                ("x-ddg-journey-id", "0123456789abcdef0123456789abcdef"),
                ("x-fe-signals", &valid_signals),
                ("sec-ch-ua", r#""Chromium";v="150""#),
                ("sec-ch-ua-mobile", "?0"),
                ("sec-ch-ua-platform", r#""Linux""#),
            ],
            b"{}",
        );
        assert!(!matcher.matches(&req_no_moz));
    }

    #[test]
    fn test_solved_v8_challenge_matcher_validation() {
        let matcher = SolvedV8ChallengeMatcher;

        // 1. Valid challenge payload
        let valid_json = serde_json::json!({
            "client_hashes": ["e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"],
            "meta": {
                "origin": "https://duck.ai",
                "stack": "Error\n    at l (https://duck.ai/dist/duckai-dist/entry.duckai.c0328fc12a6573e54bd9.js:2:1833090)",
                "duration": "25"
            }
        });
        let valid_b64 = base64::engine::general_purpose::STANDARD.encode(valid_json.to_string());
        let req_valid = make_test_request(&[("x-vqd-hash-1", &valid_b64)], b"{}");
        assert!(matcher.matches(&req_valid));

        // 2. Invalid hash: 32-char instead of 64-char
        let invalid_hash_json = serde_json::json!({
            "client_hashes": ["e3b0c44298fc1c149afbf4c8996fb924"],
            "meta": {
                "origin": "https://duck.ai",
                "stack": "Error\n    at l (https://duck.ai/dist/duckai-dist/entry.duckai.c0328fc12a6573e54bd9.js:2:1833090)",
                "duration": "25"
            }
        });
        let invalid_hash_b64 =
            base64::engine::general_purpose::STANDARD.encode(invalid_hash_json.to_string());
        let req_invalid_hash = make_test_request(&[("x-vqd-hash-1", &invalid_hash_b64)], b"{}");
        assert!(!matcher.matches(&req_invalid_hash));

        // 3. Invalid hash: non-hex character 'z'
        let non_hex_json = serde_json::json!({
            "client_hashes": ["z3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"],
            "meta": {
                "origin": "https://duck.ai",
                "stack": "Error\n    at l (https://duck.ai/dist/duckai-dist/entry.duckai.c0328fc12a6573e54bd9.js:2:1833090)",
                "duration": "25"
            }
        });
        let non_hex_b64 =
            base64::engine::general_purpose::STANDARD.encode(non_hex_json.to_string());
        let req_non_hex = make_test_request(&[("x-vqd-hash-1", &non_hex_b64)], b"{}");
        assert!(!matcher.matches(&req_non_hex));

        // 4. Missing meta.origin
        let no_origin_json = serde_json::json!({
            "client_hashes": ["e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"],
            "meta": {
                "stack": "Error\n    at l (https://duck.ai)",
                "duration": "25"
            }
        });
        let no_origin_b64 =
            base64::engine::general_purpose::STANDARD.encode(no_origin_json.to_string());
        let req_no_origin = make_test_request(&[("x-vqd-hash-1", &no_origin_b64)], b"{}");
        assert!(!matcher.matches(&req_no_origin));

        // 5. Missing meta.stack
        let no_stack_json = serde_json::json!({
            "client_hashes": ["e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"],
            "meta": {
                "origin": "https://duck.ai",
                "duration": "25"
            }
        });
        let no_stack_b64 =
            base64::engine::general_purpose::STANDARD.encode(no_stack_json.to_string());
        let req_no_stack = make_test_request(&[("x-vqd-hash-1", &no_stack_b64)], b"{}");
        assert!(!matcher.matches(&req_no_stack));

        // 6. Missing meta.duration
        let no_duration_json = serde_json::json!({
            "client_hashes": ["e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"],
            "meta": {
                "origin": "https://duck.ai",
                "stack": "Error\n    at l (https://duck.ai)"
            }
        });
        let no_duration_b64 =
            base64::engine::general_purpose::STANDARD.encode(no_duration_json.to_string());
        let req_no_duration = make_test_request(&[("x-vqd-hash-1", &no_duration_b64)], b"{}");
        assert!(!matcher.matches(&req_no_duration));
    }
}
