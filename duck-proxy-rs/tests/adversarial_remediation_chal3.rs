//! Adversarial Verification Test Suite for Iteration 2 (e2e_chal_3).
//! Empirically tests all 4 remediation areas from Challenger 1:
//! 1. DuckHeadersMatcher User-Agent strictness (rejects Firefox, Safari, Curl, non-Chrome/150).
//! 2. read_sse_stream multi-byte UTF-8 splitting across arbitrary TCP chunk boundaries.
//! 3. TestHarness Drop lifecycle server task cleanup and socket release.
//! 4. SolvedV8ChallengeMatcher SHA256 hex string and meta field validations.

mod common;

use base64::Engine;
use common::*;
use serde_json::json;
use std::str::FromStr;
use tokio::net::TcpListener;
use tokio::time::{sleep, Duration};
use wiremock::http::Method;
use wiremock::{Match, Request};

fn make_wiremock_request(headers: &[(&str, &str)], body: &[u8]) -> Request {
    let mut header_map = wiremock::http::HeaderMap::new();
    for (k, v) in headers {
        header_map.insert(
            wiremock::http::HeaderName::from_str(k).unwrap(),
            wiremock::http::HeaderValue::from_str(v).unwrap(),
        );
    }
    Request {
        url: "http://127.0.0.1/duckchat/v1/chat".parse().unwrap(),
        method: Method::POST,
        headers: header_map,
        body: body.to_vec(),
    }
}

fn valid_telemetry_signals() -> String {
    base64::engine::general_purpose::STANDARD
        .encode(r#"{"start":1724867000,"events":[{"e":"action","t":10}],"end":20}"#)
}

// =============================================================================
// 1. DUCK HEADERS MATCHER USER-AGENT ADVERSARIAL TESTS
// =============================================================================

#[test]
fn test_adversarial_ua_rejection_non_chrome_browsers() {
    let matcher = DuckHeadersMatcher::new();
    let signals = valid_telemetry_signals();

    let non_chrome_uas = [
        // Firefox Desktop (Windows, Linux, macOS)
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:109.0) Gecko/20100101 Firefox/119.0",
        "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:120.0) Gecko/20100101 Firefox/120.0",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.1; rv:120.0) Gecko/20100101 Firefox/120.0",
        // Safari Desktop & Mobile
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15",
        "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
        // CLI & HTTP libraries
        "curl/7.88.1",
        "curl/8.4.0",
        "Wget/1.21.3",
        "python-requests/2.31.0",
        "Go-http-client/1.1",
        "reqwest/0.12.0",
        // Obsolete / Other Chrome versions (not Chrome/150)
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/151.0.0.0 Safari/537.36",
        // Spoof missing Mozilla/5.0
        "Chrome/150.0.0.0 Safari/537.36",
        "CustomBot/1.0 (Chrome/150.0.0.0)",
        // Empty or whitespace
        "",
        "   ",
    ];

    for ua in non_chrome_uas {
        let req = make_wiremock_request(
            &[
                ("user-agent", ua),
                ("x-fe-version", "serp_20260827"),
                ("x-ddg-journey-id", "0123456789abcdef0123456789abcdef"),
                ("x-fe-signals", &signals),
                ("sec-ch-ua", r#""Chromium";v="150""#),
                ("sec-ch-ua-mobile", "?0"),
                ("sec-ch-ua-platform", r#""Linux""#),
            ],
            b"{}",
        );
        assert!(
            !matcher.matches(&req),
            "DuckHeadersMatcher INCORRECTLY ACCEPTED non-Chrome/150 User-Agent: '{}'",
            ua
        );
    }
}

#[test]
fn test_adversarial_ua_acceptance_valid_chrome_150() {
    let matcher = DuckHeadersMatcher::new();
    let signals = valid_telemetry_signals();

    let valid_uas = [
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
    ];

    for ua in valid_uas {
        let req = make_wiremock_request(
            &[
                ("user-agent", ua),
                ("x-fe-version", "serp_20260827"),
                ("x-ddg-journey-id", "0123456789abcdef0123456789abcdef"),
                ("x-fe-signals", &signals),
                ("sec-ch-ua", r#""Chromium";v="150""#),
                ("sec-ch-ua-mobile", "?0"),
                ("sec-ch-ua-platform", r#""Linux""#),
            ],
            b"{}",
        );
        assert!(
            matcher.matches(&req),
            "DuckHeadersMatcher REJECTED valid Chrome/150 User-Agent: '{}'",
            ua
        );
    }
}

// =============================================================================
// 2. READ SSE STREAM UTF-8 BOUNDARY STRESS TESTS
// =============================================================================

async fn spawn_raw_chunked_server(raw_chunks: Vec<Vec<u8>>) -> (String, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let url = format!("http://{}", addr);

    let handle = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;

            let response_headers = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n";
            let _ = socket.write_all(response_headers.as_bytes()).await;

            for chunk in raw_chunks {
                let chunk_len_hex = format!("{:X}\r\n", chunk.len());
                let _ = socket.write_all(chunk_len_hex.as_bytes()).await;
                let _ = socket.write_all(&chunk).await;
                let _ = socket.write_all(b"\r\n").await;
                let _ = socket.flush().await;
            }
            // End chunked transfer
            let _ = socket.write_all(b"0\r\n\r\n").await;
            let _ = socket.flush().await;
        }
    });

    (url, handle)
}

#[tokio::test]
async fn test_adversarial_sse_utf8_split_across_single_byte_chunks() {
    // Complex UTF-8 test string containing ASCII, 2-byte, 3-byte, 4-byte emojis, and symbols
    let complex_text = "Hello 🌍! Rust 🦀 and Duck 🦆: 日本語「こんにちは」、한국어「안녕하세요」、العربية «مرحبا»، Math ∑(x²)≈∞, accents: café & façade!";
    
    let json_payload = json!({
        "action": "success",
        "id": "chunk-fuzz-1",
        "message": complex_text
    });
    let sse_event = format!("data: {}\n\ndata: [DONE]\n\n", json_payload);
    let sse_bytes = sse_event.into_bytes();

    // Stream the SSE body 1 byte per TCP chunk (worst-case fragmentation)
    let byte_chunks: Vec<Vec<u8>> = sse_bytes.into_iter().map(|b| vec![b]).collect();

    let (url, server_handle) = spawn_raw_chunked_server(byte_chunks).await;

    let client = reqwest::Client::new();
    let resp = client.get(&url).send().await.expect("Failed to send request");

    let harness = TestHarness::new().await;
    let (chunks, saw_done) = harness.read_sse_stream(resp).await;
    let _ = server_handle.await;

    assert!(saw_done, "Expected [DONE] to be parsed");
    assert_eq!(chunks.len(), 1, "Expected 1 parsed chunk");
    let actual_msg = chunks[0]["message"].as_str().unwrap();
    assert_eq!(actual_msg, complex_text, "Multi-byte UTF-8 corrupted during byte-by-byte chunking!");
    assert!(!actual_msg.contains('\u{FFFD}'), "Replacement char found in decoded UTF-8!");
}

#[tokio::test]
async fn test_adversarial_sse_utf8_arbitrary_chunk_slicing_fuzz() {
    let messages = [
        "🦀 Crab Emoji 🦀",
        "🚀 Rocket into orbit 🪐",
        "Greek letters: α β γ δ ε ζ η θ ι κ λ μ ν ξ ο π ρ σ τ υ φ χ ψ ω",
        "Hieroglyphs: 𓀀 𓀁 𓀂 𓀃 𓀄 𓀅 and music 𝄞 𝄢",
        "Multi-line\nwith newlines and \"quotes\" and escaped \\ characters",
    ];

    for (idx, msg) in messages.iter().enumerate() {
        let chunk_json = json!({
            "action": "success",
            "id": format!("chunk-{}", idx),
            "delta": {
                "content": msg
            }
        });
        let raw_sse = format!("data: {}\n\ndata: [DONE]\n\n", chunk_json);
        let sse_bytes = raw_sse.into_bytes();

        // Test chunk sizes from 1 byte up to 7 bytes
        for chunk_size in 1..=7 {
            let byte_chunks: Vec<Vec<u8>> = sse_bytes
                .chunks(chunk_size)
                .map(|s| s.to_vec())
                .collect();

            let (url, server_handle) = spawn_raw_chunked_server(byte_chunks).await;

            let client = reqwest::Client::new();
            let resp = client.get(&url).send().await.expect("Failed to send request");

            let harness = TestHarness::new().await;
            let (parsed_chunks, saw_done) = harness.read_sse_stream(resp).await;
            let _ = server_handle.await;

            assert!(saw_done, "Expected saw_done=true for chunk_size {}", chunk_size);
            assert_eq!(
                parsed_chunks.len(),
                1,
                "Failed to parse chunk with chunk_size {}",
                chunk_size
            );
            let parsed_content = parsed_chunks[0]["delta"]["content"].as_str().unwrap();
            assert_eq!(
                parsed_content, *msg,
                "Mismatch with chunk_size {}: expected '{}', got '{}'",
                chunk_size, msg, parsed_content
            );
            assert!(
                !parsed_content.contains('\u{FFFD}'),
                "Found Unicode replacement char with chunk_size {}",
                chunk_size
            );
        }
    }
}

// =============================================================================
// 3. TEST HARNESS DROP & LIFECYCLE ADVERSARIAL TESTS
// =============================================================================

#[tokio::test]
async fn test_adversarial_harness_drop_terminates_listener_immediately() {
    let server_url;
    {
        let harness = TestHarness::new().await;
        server_url = harness.server_url.clone();

        // Verify the listener is active and responding
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(500))
            .build()
            .unwrap();

        let resp = client.get(&server_url).send().await;
        assert!(resp.is_ok(), "Server should accept connection while harness lives");
    }

    // Harness dropped here. Let tokio abort the server task.
    sleep(Duration::from_millis(50)).await;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()
        .unwrap();

    let resp = client.get(&server_url).send().await;
    assert!(
        resp.is_err(),
        "Server still accepting connections after TestHarness was dropped! Task was leaked."
    );
}

#[tokio::test]
async fn test_adversarial_harness_repeated_lifecycle_stress() {
    // Rapidly instantiate and drop 20 harnesses to ensure no task or port leaks
    for _ in 0..20 {
        let harness = TestHarness::new().await;
        let url = harness.server_url.clone();
        drop(harness);

        // Verify dropped server no longer accepts requests
        sleep(Duration::from_millis(15)).await;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(200))
            .build()
            .unwrap();
        let resp = client.get(&url).send().await;
        assert!(resp.is_err(), "Server leaked across rapid iterations: {}", url);
    }
}

// =============================================================================
// 4. SOLVED V8 CHALLENGE MATCHER ADVERSARIAL TESTS
// =============================================================================

#[test]
fn test_adversarial_solved_v8_challenge_matcher_comprehensive() {
    let matcher = SolvedV8ChallengeMatcher;

    // 1. Valid payload
    let valid_sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let valid_json = json!({
        "client_hashes": [valid_sha256],
        "meta": {
            "origin": "https://duck.ai",
            "stack": "Error\n    at l (https://duck.ai/dist/duckai-dist/entry.duckai.c0328fc12a6573e54bd9.js:2:1833090)",
            "duration": "25"
        }
    });
    let valid_b64 = base64::engine::general_purpose::STANDARD.encode(valid_json.to_string());
    let req = make_wiremock_request(&[("x-vqd-hash-1", &valid_b64)], b"{}");
    assert!(matcher.matches(&req), "Valid solved challenge rejected!");

    // 2. Hash length variations (invalid lengths)
    let invalid_lengths = [
        "", // 0 chars
        "e3b0c44298fc1c149afbf4c8996fb924", // 32 chars (MD5)
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b85", // 63 chars
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b8555", // 65 chars
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", // 128 chars
    ];
    for bad_hash in invalid_lengths {
        let bad_json = json!({
            "client_hashes": [bad_hash],
            "meta": {
                "origin": "https://duck.ai",
                "stack": "Error\n at l (https://duck.ai)",
                "duration": "25"
            }
        });
        let b64 = base64::engine::general_purpose::STANDARD.encode(bad_json.to_string());
        let req = make_wiremock_request(&[("x-vqd-hash-1", &b64)], b"{}");
        assert!(
            !matcher.matches(&req),
            "SolvedV8ChallengeMatcher accepted bad hash length: '{}'",
            bad_hash
        );
    }

    // 3. Non-hex characters in 64-char hash
    let non_hex_hashes = [
        "g3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", // 'g'
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b85z", // 'z'
        "e3b0c44298fc1c149afbf4c8996fb924-7ae41e4649b934ca495991b7852b855", // '-'
        "e3b0c44298fc1c149afbf4c8996fb924 7ae41e4649b934ca495991b7852b855", // space
    ];
    for bad_hash in non_hex_hashes {
        let bad_json = json!({
            "client_hashes": [bad_hash],
            "meta": {
                "origin": "https://duck.ai",
                "stack": "Error\n at l (https://duck.ai)",
                "duration": "25"
            }
        });
        let b64 = base64::engine::general_purpose::STANDARD.encode(bad_json.to_string());
        let req = make_wiremock_request(&[("x-vqd-hash-1", &b64)], b"{}");
        assert!(
            !matcher.matches(&req),
            "SolvedV8ChallengeMatcher accepted non-hex hash: '{}'",
            bad_hash
        );
    }

    // 4. Invalid origin values
    let invalid_origins = [
        "",
        "duck.ai", // missing scheme
        "ftp://duck.ai",
        "file:///duck.ai",
        "javascript:alert(1)",
    ];
    for bad_origin in invalid_origins {
        let bad_json = json!({
            "client_hashes": [valid_sha256],
            "meta": {
                "origin": bad_origin,
                "stack": "Error\n at l (https://duck.ai)",
                "duration": "25"
            }
        });
        let b64 = base64::engine::general_purpose::STANDARD.encode(bad_json.to_string());
        let req = make_wiremock_request(&[("x-vqd-hash-1", &b64)], b"{}");
        assert!(
            !matcher.matches(&req),
            "SolvedV8ChallengeMatcher accepted invalid origin: '{}'",
            bad_origin
        );
    }

    // 5. Invalid stack values
    let invalid_stacks = [
        "",
        "Everything is fine",
        "Some arbitrary string without keywords",
    ];
    for bad_stack in invalid_stacks {
        let bad_json = json!({
            "client_hashes": [valid_sha256],
            "meta": {
                "origin": "https://duck.ai",
                "stack": bad_stack,
                "duration": "25"
            }
        });
        let b64 = base64::engine::general_purpose::STANDARD.encode(bad_json.to_string());
        let req = make_wiremock_request(&[("x-vqd-hash-1", &b64)], b"{}");
        assert!(
            !matcher.matches(&req),
            "SolvedV8ChallengeMatcher accepted invalid stack: '{}'",
            bad_stack
        );
    }

    // 6. Valid duration types (string and number) vs invalid (empty string, null, array)
    let valid_durations = [json!("25"), json!("100"), json!(25), json!(150.5)];
    for dur in valid_durations {
        let test_json = json!({
            "client_hashes": [valid_sha256],
            "meta": {
                "origin": "https://duck.ai",
                "stack": "Error\n at l (https://duck.ai)",
                "duration": dur
            }
        });
        let b64 = base64::engine::general_purpose::STANDARD.encode(test_json.to_string());
        let req = make_wiremock_request(&[("x-vqd-hash-1", &b64)], b"{}");
        assert!(
            matcher.matches(&req),
            "SolvedV8ChallengeMatcher rejected valid duration: {:?}",
            dur
        );
    }

    let invalid_durations = [json!(""), json!(null), json!([]), json!({})];
    for bad_dur in invalid_durations {
        let bad_json = json!({
            "client_hashes": [valid_sha256],
            "meta": {
                "origin": "https://duck.ai",
                "stack": "Error\n at l (https://duck.ai)",
                "duration": bad_dur
            }
        });
        let b64 = base64::engine::general_purpose::STANDARD.encode(bad_json.to_string());
        let req = make_wiremock_request(&[("x-vqd-hash-1", &b64)], b"{}");
        assert!(
            !matcher.matches(&req),
            "SolvedV8ChallengeMatcher accepted invalid duration: {:?}",
            bad_dur
        );
    }
}

#[tokio::test]
async fn test_adversarial_wiremock_integration_with_solved_v8_matcher() {
    let mock_server = MockDuckServer::start().await;
    let valid_sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    let valid_json = json!({
        "client_hashes": [valid_sha256],
        "meta": {
            "origin": "https://duck.ai",
            "stack": "Error\n    at l (https://duck.ai/dist/duckai-dist/entry.duckai.c0328fc12a6573e54bd9.js:2:1833090)",
            "duration": "25"
        }
    });
    let valid_b64 = base64::engine::general_purpose::STANDARD.encode(valid_json.to_string());

    // Register route requiring SolvedV8ChallengeMatcher
    mock_server
        .mock_chat_with_solved_challenge("gpt-5.6-luna", &["Solved challenge response"], "next-vqd-123")
        .await;

    let signals = valid_telemetry_signals();
    let client = reqwest::Client::new();

    // 1. Request with valid solved challenge -> Should succeed with 200 OK
    let resp = client
        .post(format!("{}/duckchat/v1/chat", mock_server.uri()))
        .header(
            "user-agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
        )
        .header("x-fe-version", "serp_20260827")
        .header("x-ddg-journey-id", "0123456789abcdef0123456789abcdef")
        .header("x-fe-signals", &signals)
        .header("sec-ch-ua", r#""Chromium";v="150""#)
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", r#""Linux""#)
        .header("x-vqd-hash-1", &valid_b64)
        .json(&json!({
            "model": "gpt-5.6-luna",
            "messages": [{"role": "user", "content": "hello"}],
            "durableStream": {
                "publicKey": {
                    "kty": "RSA",
                    "alg": "RSA-OAEP-256",
                    "use": "enc",
                    "e": "AQAB",
                    "n": "validUnpaddedModulus123"
                }
            }
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    // 2. Request with invalid challenge (32-char MD5 hash) -> WireMock should return 404 (no match)
    let bad_json = json!({
        "client_hashes": ["e3b0c44298fc1c149afbf4c8996fb924"],
        "meta": {
            "origin": "https://duck.ai",
            "stack": "Error\n at l (https://duck.ai)",
            "duration": "25"
        }
    });
    let bad_b64 = base64::engine::general_purpose::STANDARD.encode(bad_json.to_string());

    let resp_bad = client
        .post(format!("{}/duckchat/v1/chat", mock_server.uri()))
        .header(
            "user-agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/150.0.0.0 Safari/537.36",
        )
        .header("x-fe-version", "serp_20260827")
        .header("x-ddg-journey-id", "0123456789abcdef0123456789abcdef")
        .header("x-fe-signals", &signals)
        .header("sec-ch-ua", r#""Chromium";v="150""#)
        .header("sec-ch-ua-mobile", "?0")
        .header("sec-ch-ua-platform", r#""Linux""#)
        .header("x-vqd-hash-1", &bad_b64)
        .json(&json!({
            "model": "gpt-5.6-luna",
            "messages": [{"role": "user", "content": "hello"}],
            "durableStream": {
                "publicKey": {
                    "kty": "RSA",
                    "alg": "RSA-OAEP-256",
                    "use": "enc",
                    "e": "AQAB",
                    "n": "validUnpaddedModulus123"
                }
            }
        }))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(
        resp_bad.status(),
        reqwest::StatusCode::NOT_FOUND,
        "WireMock matched request despite invalid challenge hash!"
    );
}
