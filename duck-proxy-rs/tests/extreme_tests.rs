//! Extreme test suite verifying Professional Customization, Audio Dictation,
//! Agent Tool Calling, and Adaptive Maximum Reasoning.

use duck_proxy_rs::api::chat::{
    extract_tool_calls_with_tools,
    normalize_messages_for_duck, resolve_tool_name, ChatMessage, ContentPart, FunctionDefinition,
    InputAudio, MessageContent, ToolDefinition,
    OMNI_PERMISSIONS_PROMPT, PROFESSIONAL_RESPONSE_DIRECTIVE,
};
use duck_proxy_rs::api::audio::{
    transcode_audio_to_webm_if_needed, TranscriptionResponse, VerboseTranscriptionResponse,
};
use duck_proxy_rs::crypto::EphemeralKeypair;
use duck_proxy_rs::duck::payload::{assert_reasoning_mode, build_chat_payload, guard_reasoning_mode};
use duck_proxy_rs::duck::types::{
    max_reasoning_effort_for_model, DuckChatMessage, REASONING_EFFORT_FAST,
    REASONING_EFFORT_MEDIUM, REASONING_EFFORT_REASONING,
};

// ===========================================================================
// 1. Extreme Reasoning Mode Tests (Adaptive Max Reasoning & Invariants)
// ===========================================================================

#[test]
fn test_extreme_adaptive_max_reasoning_matrix() {
    let keypair = EphemeralKeypair::generate().unwrap();

    // Models with "medium" reasoning capability
    let medium_models = vec![
        "gpt-5.4-mini",
        "gpt-5.4",
        "gpt-5.6-terra",
        "gpt-5.6-sol",
        "claude-opus-4-8",
    ];
    for m in medium_models {
        assert_eq!(max_reasoning_effort_for_model(m), REASONING_EFFORT_MEDIUM);
        let payload = build_chat_payload(m, vec![], &keypair, false, "conv-med");
        assert_eq!(payload.reasoning_effort, "medium");
        assert!(payload.is_reasoning_mode());
        assert!(guard_reasoning_mode(&payload).is_ok());
        assert_reasoning_mode(&payload);
    }

    // Models with "low" reasoning capability
    let low_models = vec![
        "gpt-5.6-luna",
        "claude-haiku-4-5",
        "tinfoil/gemma4-31b",
        "mistral-small-2603",
        "unknown-future-model",
    ];
    for m in low_models {
        assert_eq!(max_reasoning_effort_for_model(m), REASONING_EFFORT_REASONING);
        let payload = build_chat_payload(m, vec![], &keypair, false, "conv-low");
        assert_eq!(payload.reasoning_effort, "low");
        assert!(payload.is_reasoning_mode());
        assert!(guard_reasoning_mode(&payload).is_ok());
        assert_reasoning_mode(&payload);
    }
}

#[test]
fn test_extreme_reasoning_guard_catches_all_violations() {
    let keypair = EphemeralKeypair::generate().unwrap();
    let mut payload = build_chat_payload("gpt-5.6-luna", vec![], &keypair, false, "conv-tamper");

    let bad_efforts = vec![
        REASONING_EFFORT_FAST,
        "",
        "none",
        "off",
        "false",
        "disabled",
        "high",
        "super_max",
    ];
    for bad in bad_efforts {
        payload.reasoning_effort = bad.to_string();
        assert!(!payload.is_reasoning_mode());
        let err = guard_reasoning_mode(&payload).expect_err("Must reject illegal effort");
        assert!(err.contains("ReasoningModeGuardViolation"));
    }
}

// ===========================================================================
// 2. Extreme Professional Response Customization Tests
// ===========================================================================

#[test]
fn test_extreme_professional_response_customization_directives() {
    // 1. Check prompt directive presence
    assert!(OMNI_PERMISSIONS_PROMPT.contains("PROFESSIONAL RESPONSE CUSTOMIZATION"));
    assert!(OMNI_PERMISSIONS_PROMPT.contains("TONE & MOOD: Calm, objective, authoritative, highly professional"));
    assert!(OMNI_PERMISSIONS_PROMPT.contains("Zero conversational fluff"));
    assert!(OMNI_PERMISSIONS_PROMPT.contains("LENGTH & DENSITY: Optimal information density"));
    assert!(OMNI_PERMISSIONS_PROMPT.contains("AI ROLE & PERSONA: Elite Principal Staff Systems Architect"));

    assert!(PROFESSIONAL_RESPONSE_DIRECTIVE.contains("TONE & MOOD"));
    assert!(PROFESSIONAL_RESPONSE_DIRECTIVE.contains("LENGTH & DENSITY"));
    assert!(PROFESSIONAL_RESPONSE_DIRECTIVE.contains("AI ROLE & PERSONA"));
    assert!(PROFESSIONAL_RESPONSE_DIRECTIVE.contains("BEHAVIOR"));

    // 2. Verify prompt normalization includes the professional directive at the head of every turn
    let messages = vec![
        ChatMessage::user("Write a high-performance hash map in Rust."),
    ];
    let normalized = normalize_messages_for_duck(&messages, None, None);
    assert!(!normalized.is_empty());
    assert_eq!(normalized[0].role, "user");
    assert!(normalized[0].content.contains("PROFESSIONAL RESPONSE CUSTOMIZATION"));
    assert!(normalized[0].content.contains("Write a high-performance hash map in Rust."));
}

#[test]
fn test_extreme_professional_customization_merging_with_user_instructions() {
    let messages = vec![
        ChatMessage::system("Tone: Professional, Length: Concise, Persona: Expert"),
        ChatMessage::user("Implement Raft consensus algorithm."),
    ];
    let normalized = normalize_messages_for_duck(&messages, None, None);
    assert_eq!(normalized.len(), 1);
    assert!(normalized[0].content.contains("PROFESSIONAL RESPONSE CUSTOMIZATION"));
    assert!(normalized[0].content.contains("Tone: Professional, Length: Concise"));
    assert!(normalized[0].content.contains("Implement Raft consensus algorithm."));
}

// ===========================================================================
// 3. Extreme Agent Tool Calling Tests (bash, write_file, apply_patch, etc.)
// ===========================================================================

fn make_tool_def(name: &str, desc: &str, params: serde_json::Value) -> ToolDefinition {
    ToolDefinition {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: name.to_string(),
            description: Some(desc.to_string()),
            parameters: Some(params),
        },
    }
}

#[test]
fn test_extreme_resolve_tool_name_aliases() {
    let client_tools = vec![
        make_tool_def("run_command", "Execute bash commands", serde_json::json!({"properties": {"command": {"type": "string"}}})),
        make_tool_def("create_file", "Write files", serde_json::json!({"properties": {"filePath": {"type": "string"}}})),
        make_tool_def("apply_diff", "Apply patch", serde_json::json!({"properties": {"patch": {"type": "string"}}})),
        make_tool_def("view_file", "Read file", serde_json::json!({"properties": {"filePath": {"type": "string"}}})),
        make_tool_def("replace_file_content", "Edit file", serde_json::json!({"properties": {"path": {"type": "string"}}})),
    ];

    // Shell execution aliases -> "run_command"
    for alias in ["bash", "sh", "shell", "terminal", "execute_bash", "run_command", "exec", "Bash"] {
        assert_eq!(resolve_tool_name(alias, Some(&client_tools)), "run_command");
    }

    // Write file aliases -> "create_file"
    for alias in ["write", "write_file", "create_file", "Write_File"] {
        assert_eq!(resolve_tool_name(alias, Some(&client_tools)), "create_file");
    }

    // Patch aliases -> "apply_diff"
    for alias in ["apply_patch", "patch", "apply_diff", "patch_file"] {
        assert_eq!(resolve_tool_name(alias, Some(&client_tools)), "apply_diff");
    }

    // Read file aliases -> "view_file"
    for alias in ["read", "read_file", "view_file", "cat"] {
        assert_eq!(resolve_tool_name(alias, Some(&client_tools)), "view_file");
    }

    // Edit file aliases -> "replace_file_content"
    for alias in ["edit", "edit_file", "replace_file_content"] {
        assert_eq!(resolve_tool_name(alias, Some(&client_tools)), "replace_file_content");
    }
}

#[test]
fn test_extreme_extract_tool_calls_patch_and_bash_synthesis() {
    let tools_with_patch = vec![
        make_tool_def("bash", "run shell", serde_json::json!({})),
        make_tool_def("apply_patch", "apply unified diff patch", serde_json::json!({})),
    ];

    // 1. Explicit <tool_call> with apply_patch
    let text_xml = r#"I will apply the patch:
<tool_call>
{"name": "apply_patch", "arguments": {"patch": "*** Begin Patch ***\n+added line\n*** End Patch ***"}}
</tool_call>"#;
    let calls = extract_tool_calls_with_tools(text_xml, Some(&tools_with_patch)).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].function.name, "apply_patch");
    let args: serde_json::Value = serde_json::from_str(&calls[0].function.arguments).unwrap();
    assert!(args["patch"].as_str().unwrap().contains("+added line"));
    // Both patch and diff arguments populated
    assert_eq!(args["diff"], args["patch"]);

    // 2. Markdown diff block when client HAS apply_patch tool
    let text_diff_block = "Here is the diff to apply:\n```diff\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,4 @@\n+println!(\"hello\");\n```";
    let calls = extract_tool_calls_with_tools(text_diff_block, Some(&tools_with_patch)).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].function.name, "apply_patch");

    // 3. Markdown diff block when client DOES NOT have apply_patch, only bash
    let tools_bash_only = vec![
        make_tool_def("bash", "run shell", serde_json::json!({})),
    ];
    let calls = extract_tool_calls_with_tools(text_diff_block, Some(&tools_bash_only)).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].function.name, "bash");
    let args: serde_json::Value = serde_json::from_str(&calls[0].function.arguments).unwrap();
    assert!(args["command"].as_str().unwrap().starts_with("patch -p1 << 'EOF'"));
}

#[test]
fn test_extreme_argument_normalization() {
    let tools = vec![
        make_tool_def("write_file", "write", serde_json::json!({})),
    ];
    let text = r#"<tool_call>
{"name": "write_file", "arguments": {"filePath": "test.txt", "content": "hello"}}
</tool_call>"#;
    let calls = extract_tool_calls_with_tools(text, Some(&tools)).unwrap();
    let args: serde_json::Value = serde_json::from_str(&calls[0].function.arguments).unwrap();
    // Verify all common path schemas are normalized
    assert_eq!(args["path"], "test.txt");
    assert_eq!(args["filePath"], "test.txt");
    assert_eq!(args["file_path"], "test.txt");
}

// ===========================================================================
// 4. Extreme Audio Dictation & Voice Chat Tests
// ===========================================================================

#[test]
fn test_extreme_audio_transcription_schemas() {
    let resp = TranscriptionResponse {
        text: "Testing speech to text transcription".to_string(),
    };
    let json = serde_json::to_string(&resp).unwrap();
    assert_eq!(json, r#"{"text":"Testing speech to text transcription"}"#);

    let verbose = VerboseTranscriptionResponse {
        task: "transcribe".to_string(),
        language: "english".to_string(),
        duration: 2.5,
        text: "Testing speech to text transcription".to_string(),
        segments: vec![],
    };
    let json_verbose = serde_json::to_string(&verbose).unwrap();
    assert!(json_verbose.contains(r#""task":"transcribe""#));
    assert!(json_verbose.contains(r#""duration":2.5"#));
}

#[tokio::test]
async fn test_extreme_audio_transcode_helper() {
    // WebM data passes through untouched
    let webm_data = vec![0x1A, 0x45, 0xDF, 0xA3];
    let (out_data, out_mime) = transcode_audio_to_webm_if_needed(webm_data.clone(), "audio/webm").await;
    assert_eq!(out_data, webm_data);
    assert_eq!(out_mime, "audio/webm");

    // Empty non-webm data gracefully returns
    let empty_wav = vec![];
    let (out_empty, _) = transcode_audio_to_webm_if_needed(empty_wav, "audio/wav").await;
    assert!(out_empty.is_empty());
}

#[test]
fn test_extreme_voice_chat_input_audio_in_chat_completion() {
    let part_audio = ContentPart {
        part_type: Some("input_audio".to_string()),
        text: None,
        input_audio: Some(InputAudio {
            data: "UklGRiQAAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQAAAAA=".to_string(),
            format: "wav".to_string(),
        }),
    };
    let part_text = ContentPart {
        part_type: Some("text".to_string()),
        text: Some(" Please summarize the above recording.".to_string()),
        input_audio: None,
    };

    let msg = ChatMessage {
        role: "user".to_string(),
        content: Some(MessageContent::Parts(vec![part_audio, part_text])),
        name: None,
        tool_call_id: None,
        tool_calls: None,
        function_call: None,
    };

    let normalized = normalize_messages_for_duck(&[msg], None, None);
    assert_eq!(normalized.len(), 1);
    assert!(normalized[0].content.contains("[Voice Audio Recording input]"));
    assert!(normalized[0].content.contains("Please summarize the above recording."));
}

// ===========================================================================
// 5. Stress & Multi-Turn Concurrency Simulation
// ===========================================================================

#[test]
fn test_extreme_concurrent_payload_construction_stress() {
    let keypair = EphemeralKeypair::generate().unwrap();
    let handles: Vec<_> = (0..50)
        .map(|i| {
            let kp = keypair.clone();
            std::thread::spawn(move || {
                let model = if i % 2 == 0 { "gpt-5.4-mini" } else { "gpt-5.6-luna" };
                let msg = vec![DuckChatMessage {
                    role: "user".to_string(),
                    content: format!("Thread query {}", i),
                }];
                let payload = build_chat_payload(model, msg, &kp, false, &format!("conv-{}", i));
                assert!(payload.is_reasoning_mode());
                assert_reasoning_mode(&payload);
                if model == "gpt-5.4-mini" {
                    assert_eq!(payload.reasoning_effort, "medium");
                } else {
                    assert_eq!(payload.reasoning_effort, "low");
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread joined successfully");
    }
}
