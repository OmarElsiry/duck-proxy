use base64::Engine;
use duck_proxy_rs::duck::payload::build_chat_payload;
use duck_proxy_rs::duck::types::DuckChatMessage;
use duck_proxy_rs::crypto::EphemeralKeypair;

#[test]
fn test_payload_truncation_preserves_prompt_at_end() {
    let keypair = EphemeralKeypair::generate().unwrap();
    let old_turn_1 = DuckChatMessage {
        role: "user".to_string(),
        content: "A".repeat(4000),
    };
    let old_turn_2 = DuckChatMessage {
        role: "assistant".to_string(),
        content: "B".repeat(4000),
    };
    let active_prompt = DuckChatMessage {
        role: "user".to_string(),
        content: "Create a calculator in Python".to_string(),
    };

    let messages = vec![old_turn_1, old_turn_2, active_prompt];
    let payload = build_chat_payload("gpt-5.6-luna", messages, &keypair, false, "conv-123");

    let total_chars: usize = payload.messages.iter().map(|m| m.content.len()).sum();
    assert!(total_chars <= 7500, "Total payload chars must be <= 7500, got {}", total_chars);
    assert!(payload.is_reasoning_mode());
    assert_eq!(payload.reasoning_effort, "low");
    assert!(
        payload.messages.last().unwrap().content.contains("Create a calculator in Python"),
        "The active user prompt must be preserved at the end"
    );
}

#[test]
fn test_single_oversized_message_truncation() {
    let keypair = EphemeralKeypair::generate().unwrap();
    let massive_prompt = DuckChatMessage {
        role: "user".to_string(),
        content: format!("{} CRITICAL_PROMPT_END", "X".repeat(10000)),
    };

    let payload = build_chat_payload("gpt-5.6-luna", vec![massive_prompt], &keypair, false, "conv-123");
    let total_chars: usize = payload.messages.iter().map(|m| m.content.len()).sum();
    assert!(total_chars <= 7500, "Truncated message must be <= 7500 chars");
    assert!(
        payload.messages[0].content.contains("CRITICAL_PROMPT_END"),
        "Ending prompt must be preserved after front-truncation"
    );
}

#[tokio::test]
async fn test_live_chat_to_duck() {
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();

    let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36";
    let status_resp = client
        .get("https://duck.ai/duckchat/v1/status")
        .header("User-Agent", ua)
        .header("Accept", "*/*")
        .header("x-vqd-accept", "1")
        .header("Referer", "https://duck.ai/")
        .send()
        .await
        .unwrap();

    println!("Status response code: {}", status_resp.status());
    let raw_challenge = status_resp.headers().get("x-vqd-hash-1").unwrap().to_str().unwrap().to_string();
    println!("Raw challenge length: {}", raw_challenge.len());

    let solved_vqd = duck_proxy_rs::v8::actor::solve_challenge_sync(&raw_challenge, Some(ua)).unwrap();
    println!("Solved VQD length: {}", solved_vqd.len());
    let decoded_json = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(&solved_vqd).unwrap()).unwrap();
    println!("Decoded Solved VQD:\n{}", decoded_json);

    let journey_id = uuid::Uuid::new_v4().to_string();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let signals = serde_json::json!({
        "start": now_ms - 1500,
        "events": [
            {"name": "startNewChat_free", "delta": 55},
            {"name": "recentChatsListImpression", "delta": 190},
            {"name": "action", "delta": 850, "trusted": true}
        ],
        "end": 920
    });
    let signals_b64 = base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(&signals).unwrap().as_bytes());

    let keypair = EphemeralKeypair::generate().unwrap();
    let payload = build_chat_payload("gpt-5.6-luna", vec![DuckChatMessage {
        role: "user".to_string(),
        content: "hello, reply with one word: pong".to_string(),
    }], &keypair, false, "live-test-1");

    let chat_resp = client
        .post("https://duck.ai/duckchat/v1/chat")
        .header("user-agent", ua)
        .header("sec-ch-ua", r#""Not(A:Brand";v="99", "Google Chrome";v="133", "Chromium";v="133""#)
        .header("sec-ch-ua-platform", r#""Windows""#)
        .header("sec-ch-ua-mobile", "?0")
        .header("x-fe-version", "serp_20260911_040826_ET-8dfa658aa194acbf0d24973c7449759e577ff187")
        .header("x-ddg-journey-id", &journey_id)
        .header("x-fe-signals", &signals_b64)
        .header("x-vqd-hash-1", &solved_vqd)
        .header("origin", "https://duck.ai")
        .header("referer", "https://duck.ai/")
        .header("accept", "text/event-stream")
        .header("content-type", "application/json")
        .json(&payload)
        .send()
        .await
        .unwrap();

    println!("Chat response code: {}", chat_resp.status());
    let body = chat_resp.text().await.unwrap();
    println!("Chat response body (first 300 chars): {}", &body[..300.min(body.len())]);
}

