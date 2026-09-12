//! End-to-End Integration Tests for IDE (ZCODE / Cursor / Cline / Roo / Zed) Tools & Permissions.

mod common;

use common::*;
use serde_json::{json, Value};

#[tokio::test]
async fn test_ide_omni_permissions_system_prompt_propagation() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-perm-1").await;
    harness
        .mock_upstream
        .mock_chat_ok(
            "gpt-5.6-luna",
            &["I have full permissions to modify the repository and will apply the requested changes."],
            "vqd-perm-2",
        )
        .await;

    let payload = json!({
        "model": "gpt5",
        "messages": [
            {"role": "user", "content": "Please refactor the user authentication module in src/auth.rs and cut release v1.0.0."}
        ],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["object"], "chat.completion");
    let content = body["choices"][0]["message"]["content"].as_str().unwrap();
    assert!(content.contains("full permissions"));
    assert_eq!(body["choices"][0]["finish_reason"], "stop");
}

#[tokio::test]
async fn test_ide_tool_definitions_and_schema_injection() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-tools-1").await;
    harness
        .mock_upstream
        .mock_chat_ok(
            "gpt-5.6-luna",
            &[r#"<tool_call>{"name": "edit_file", "arguments": {"path": "src/main.py", "content": "print('hello updated')"}}</tool_call>"#],
            "vqd-tools-2",
        )
        .await;

    let payload = json!({
        "model": "gpt5",
        "messages": [
            {"role": "user", "content": "Update src/main.py to print hello updated."}
        ],
        "tools": [
            {
                "type": "function",
                "function": {
                    "name": "edit_file",
                    "description": "Edits or replaces the content of a file in the workspace",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {"type": "string"},
                            "content": {"type": "string"}
                        },
                        "required": ["path", "content"]
                    }
                }
            }
        ],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["choices"][0]["finish_reason"], "tool_calls");
    let tool_calls = body["choices"][0]["message"]["tool_calls"].as_array().expect("Expected tool_calls array");
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0]["type"], "function");
    assert_eq!(tool_calls[0]["function"]["name"], "edit_file");
    assert!(tool_calls[0]["function"]["arguments"].as_str().unwrap().contains("src/main.py"));
}

#[tokio::test]
async fn test_ide_tool_call_response_json_parsing() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-json-1").await;
    harness
        .mock_upstream
        .mock_chat_ok(
            "gpt-5.6-luna",
            &[r#"{"name": "run_command", "arguments": {"command": "cargo test"}}"#],
            "vqd-json-2",
        )
        .await;

    let payload = json!({
        "model": "gpt5",
        "messages": [
            {"role": "user", "content": "Run the tests for the project."}
        ],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["choices"][0]["finish_reason"], "tool_calls");
    let tool_calls = body["choices"][0]["message"]["tool_calls"].as_array().unwrap();
    assert_eq!(tool_calls[0]["function"]["name"], "run_command");
    assert!(tool_calls[0]["function"]["arguments"].as_str().unwrap().contains("cargo test"));
}

#[tokio::test]
async fn test_ide_multi_turn_tool_execution_history() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-history-1").await;
    harness
        .mock_upstream
        .mock_chat_ok(
            "claude-haiku-4-5",
            &["The file contains 42 lines. All tests now pass and release v1.0.0 is ready."],
            "vqd-history-2",
        )
        .await;

    let payload = json!({
        "model": "claude",
        "messages": [
            {"role": "developer", "content": "You are ZCODE AI coding assistant with repository editing tools."},
            {"role": "user", "content": "Inspect src/lib.rs"},
            {
                "role": "assistant",
                "content": null,
                "tool_calls": [
                    {
                        "id": "call_abc123",
                        "type": "function",
                        "function": {
                            "name": "view_file",
                            "arguments": "{\"path\": \"src/lib.rs\"}"
                        }
                    }
                ]
            },
            {
                "role": "tool",
                "tool_call_id": "call_abc123",
                "content": "pub fn add(a: i32, b: i32) -> i32 { a + b }"
            },
            {"role": "user", "content": "Now finish the task and release."}
        ],
        "stream": false
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["choices"][0]["finish_reason"], "stop");
    let content = body["choices"][0]["message"]["content"].as_str().unwrap();
    assert!(content.contains("All tests now pass"));
}

#[tokio::test]
async fn test_ide_streaming_with_tools_and_permissions() {
    let harness = TestHarness::new().await;
    harness.mock_upstream.mock_status_ok("vqd-stream-1").await;
    harness
        .mock_upstream
        .mock_chat_ok(
            "gpt-5.6-luna",
            &["I am modifying ", "the codebase ", "now."],
            "vqd-stream-2",
        )
        .await;

    let payload = json!({
        "model": "gpt5",
        "messages": [
            {"role": "user", "content": "Edit the config.yaml file."}
        ],
        "tools": [
            {
                "type": "function",
                "function": {
                    "name": "edit_file",
                    "description": "Edits file",
                    "parameters": {}
                }
            }
        ],
        "stream": true
    });

    let resp = harness.chat_completions(payload).await;
    assert_eq!(resp.status(), reqwest::StatusCode::OK);

    let (chunks, saw_done) = harness.read_sse_stream(resp).await;
    assert!(saw_done);
    let completion_chunks: Vec<_> = chunks
        .into_iter()
        .filter(|c| c.get("object").and_then(|o| o.as_str()) == Some("chat.completion.chunk"))
        .collect();
    assert!(completion_chunks.len() >= 3);
    assert_eq!(completion_chunks[0]["choices"][0]["delta"]["content"], "I am modifying ");
    assert_eq!(completion_chunks[1]["choices"][0]["delta"]["content"], "the codebase ");
    assert_eq!(completion_chunks[2]["choices"][0]["delta"]["content"], "now.");
}
