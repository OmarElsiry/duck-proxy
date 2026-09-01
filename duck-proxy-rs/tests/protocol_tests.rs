use duck_proxy_rs::api::chat::{
    normalize_messages_for_duck, format_tools_system_instructions,
    ChatMessage, ToolDefinition, FunctionDefinition
};
use duck_proxy_rs::config::Config;

#[test]
fn test_omni_permissions_prompt_injected() {
    let messages = vec![
        ChatMessage::user("Create a file named hello.py")
    ];

    let normalized = normalize_messages_for_duck(&messages, None, None);
    assert!(!normalized.is_empty(), "Normalized messages should not be empty");
    assert_eq!(normalized[0].role, "user");
    assert!(
        normalized[0].content.contains("[ENVIRONMENT & PERMISSION DIRECTIVES]"),
        "System prompt should contain permission directives"
    );
    assert!(
        normalized[0].content.contains("Create a file named hello.py"),
        "User prompt must be preserved"
    );
}

#[test]
fn test_tool_instructions_formatting() {
    let tools = vec![
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "bash".to_string(),
                description: Some("Execute bash command".to_string()),
                parameters: None,
            }
        }
    ];

    let formatted = format_tools_system_instructions(Some(&tools), None);
    assert!(formatted.is_some(), "Tools formatting should return Some");
    let text = formatted.unwrap();
    assert!(text.contains("AVAILABLE TOOLS"));
    assert!(text.contains("- Tool: bash"));
    assert!(text.contains("<tool_call>"));
}

#[test]
fn test_model_alias_resolution() {
    let config = Config::default();
    let gpt_model = config.resolve_model("gpt-5.6-luna");
    assert!(gpt_model.is_some());
    assert_eq!(gpt_model.unwrap().duck_model, "gpt-5.6-luna");

    let claude_model = config.resolve_model("claude-haiku-4-5");
    assert!(claude_model.is_some());
    assert_eq!(claude_model.unwrap().duck_model, "claude-haiku-4-5");
}

#[test]
fn test_multi_turn_history_normalization() {
    let messages = vec![
        ChatMessage::user("First question"),
        ChatMessage::assistant("First answer"),
        ChatMessage::user("Second question"),
    ];

    let normalized = normalize_messages_for_duck(&messages, None, None);
    assert_eq!(normalized.len(), 3);
    assert_eq!(normalized[0].role, "user");
    assert_eq!(normalized[1].role, "assistant");
    assert_eq!(normalized[2].role, "user");
    assert_eq!(normalized[1].content, "First answer");
    assert_eq!(normalized[2].content, "Second question");
}

#[test]
fn test_image_generation_intent_detection() {
    use duck_proxy_rs::api::chat::is_image_generation_intent;

    let img_msgs_1 = vec![ChatMessage::user("can u gen img of a horse")];
    assert!(is_image_generation_intent("gpt-5.6-luna", "duckproxy/gpt-5.6-luna", &img_msgs_1));

    let img_msgs_2 = vec![ChatMessage::user("generate an image of a neon cyber duck")];
    assert!(is_image_generation_intent("gpt-5.6-luna", "gpt-5.6-luna", &img_msgs_2));

    let img_msgs_3 = vec![ChatMessage::user("draw a sunset over the mountains")];
    assert!(is_image_generation_intent("gpt-5.6-luna", "gpt-5.6-luna", &img_msgs_3));

    let code_msgs = vec![ChatMessage::user("Write a function to calculate Fibonacci in Rust")];
    assert!(!is_image_generation_intent("gpt-5.6-luna", "gpt-5.6-luna", &code_msgs));
}

#[test]
fn test_derive_image_filename() {
    use duck_proxy_rs::api::chat::derive_image_filename;

    assert_eq!(derive_image_filename("can u gen img of a horse"), "horse.png");
    assert_eq!(derive_image_filename("can u gen img of a knight and add your base url on top of it as small icon"), "knight_base_url.png");
    assert_eq!(derive_image_filename("generate a picture of futuristic cyberpunk cat"), "futuristic_cyberpunk_cat.png");
    assert_eq!(derive_image_filename(""), "image.png");
}

#[test]
fn test_build_image_write_tool_call() {
    use duck_proxy_rs::api::chat::build_image_write_tool_call;

    let b64_dummy = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
    let (tool_call, cmd) = build_image_write_tool_call(b64_dummy, "knight_icon.png");

    assert_eq!(tool_call.function.name, "bash");
    assert!(!cmd.contains(b64_dummy), "Command must NOT contain raw base64 data to avoid console dump");
    assert!(cmd.contains("knight_icon.png"));
    assert!(cmd.contains("realpath"));
}

#[test]
fn test_multi_chunk_image_assembly() {
    use duck_proxy_rs::api::chat::build_image_write_tool_call;

    let chunks = [
        "iVBORw0KGgoAAAANSUhEUgAA".to_string(),
        "CAYAAAC6v1pkAAAAAXNSR0IA".to_string(),
        "rs4c6QAAAARnQU1BAACxjwv8".to_string(),
        "YQUAAAAJcEhZcwAADsMAAA7D".to_string(),
    ];

    let assembled = chunks.concat();
    assert_eq!(
        assembled,
        "iVBORw0KGgoAAAANSUhEUgAACAYAAAC6v1pkAAAAAXNSR0IArs4c6QAAAARnQU1BAACxjwv8YQUAAAAJcEhZcwAADsMAAA7D"
    );

    let (tool_call, cmd) = build_image_write_tool_call(&assembled, "multi_chunk_test.png");
    assert_eq!(tool_call.function.name, "bash");
    assert!(cmd.contains("multi_chunk_test.png"));
}

#[test]
fn test_prompt_preservation_with_brackets() {
    use duck_proxy_rs::api::chat::derive_image_filename;

    let prompt_with_brackets = "[cyberpunk] knight [in neon 4k]";
    let filename = derive_image_filename(prompt_with_brackets);
    assert_eq!(filename, "cyberpunk_knight_neon.png");

    let raw_prompt = "[cyberpunk] knight [in neon 4k]";
    let prompt = raw_prompt.trim().to_string();
    assert_eq!(prompt, "[cyberpunk] knight [in neon 4k]");
    assert!(prompt.contains("[cyberpunk]"));
    assert!(prompt.contains("[in neon 4k]"));
}

#[test]
fn test_parse_sse_image_discriminators() {
    use duck_proxy_rs::duck::{parse_sse_line, SseEvent};

    // 1. action: image-partial
    let ev1 = parse_sse_line(r#"data: {"action":"image-partial","result":"partial_chunk_1"}"#);
    match ev1 {
        Some(SseEvent::ImageData(d)) => assert_eq!(d, "partial_chunk_1"),
        other => panic!("Expected ImageData, got {:?}", other),
    }

    // 2. action: image-final
    let ev2 = parse_sse_line(r#"data: {"action":"image-final","result":"data:image/png;base64,final_chunk"}"#);
    match ev2 {
        Some(SseEvent::ImageData(d)) => assert_eq!(d, "final_chunk"),
        other => panic!("Expected ImageData, got {:?}", other),
    }

    // 3. role: partial-image
    let ev3 = parse_sse_line(r#"data: {"role":"partial-image","result":"partial_chunk_2"}"#);
    match ev3 {
        Some(SseEvent::ImageData(d)) => assert_eq!(d, "partial_chunk_2"),
        other => panic!("Expected ImageData, got {:?}", other),
    }

    // 4. role: generated-image
    let ev4 = parse_sse_line(r#"data: {"role":"generated-image","result":"data:image/png;base64,gen_img_data"}"#);
    match ev4 {
        Some(SseEvent::ImageData(d)) => assert_eq!(d, "gen_img_data"),
        other => panic!("Expected ImageData, got {:?}", other),
    }

    // 5. top-level b64Image
    let ev5 = parse_sse_line(r#"data: {"b64Image":"direct_b64_data"}"#);
    match ev5 {
        Some(SseEvent::ImageData(d)) => assert_eq!(d, "direct_b64_data"),
        other => panic!("Expected ImageData, got {:?}", other),
    }

    // 6. nested data.b64Image
    let ev6 = parse_sse_line(r#"data: {"role":"assistant","data":{"b64Image":"nested_b64_data"}}"#);
    match ev6 {
        Some(SseEvent::ImageData(d)) => assert_eq!(d, "nested_b64_data"),
        other => panic!("Expected ImageData, got {:?}", other),
    }
}

#[test]
fn test_payload_retains_image_gen_flag() {
    use duck_proxy_rs::crypto::EphemeralKeypair;
    use duck_proxy_rs::duck::payload::build_chat_payload;
    use duck_proxy_rs::duck::types::DuckChatMessage;

    let keypair = EphemeralKeypair::generate().unwrap();
    let messages = vec![DuckChatMessage {
        role: "user".to_string(),
        content: "draw a glowing dragon".to_string(),
    }];

    // With is_image_gen = true
    let payload = build_chat_payload("gpt-5.6-luna", messages.clone(), &keypair, true, "conv-retry-418");
    assert_eq!(payload.metadata.tool_choice.generate_image, Some(true));
    assert_eq!(payload.can_delegate_image_generation, Some(true));
    assert_eq!(payload.model, "gpt-5.6-luna");

    // With is_image_gen = false
    let text_payload = build_chat_payload("gpt-5.6-luna", messages, &keypair, false, "conv-text");
    assert_eq!(text_payload.metadata.tool_choice.generate_image, None);
    assert_eq!(text_payload.can_delegate_image_generation, None);
}

#[test]
fn test_turn2_tool_turn_completion_intent_suppression() {
    use duck_proxy_rs::api::chat::{is_image_generation_intent, ChatMessage};

    // Turn 1: User asks for image -> intent is true
    let turn1_messages = vec![ChatMessage::user("gen img of a knight")];
    assert!(is_image_generation_intent("gpt-5.6-luna", "gpt-5.6-luna", &turn1_messages));

    // Turn 2: User asked for image, assistant emitted tool call, tool returned result -> intent is false
    let turn2_messages = vec![
        ChatMessage::user("gen img of a knight"),
        ChatMessage::assistant("Calling bash tool to write image..."),
        ChatMessage {
            role: "tool".to_string(),
            content: Some(duck_proxy_rs::api::chat::MessageContent::Text("Image successfully saved to: /workspace/knight.png".to_string())),
            name: Some("bash".to_string()),
            tool_call_id: Some("call_12345".to_string()),
            ..Default::default()
        },
    ];
    assert!(!is_image_generation_intent("gpt-5.6-luna", "gpt-5.6-luna", &turn2_messages),
        "Turn 2 with tool result must suppress image generation intent to allow single-shot completion"
    );
}
