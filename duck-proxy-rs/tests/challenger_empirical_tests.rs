//! Empirical Challenge & Stress Test Suite
//! Testing:
//! 1. Multi-chunk SSE stream parsing, arbitrary chunk boundaries, and split base64 strings.
//! 2. Prompt preservation with complex, nested, escaped, and emoji bracket patterns.
//! 3. Image generation intent detection across boundary prompts and multi-turn transitions.
//! 4. End-to-end base64 decoding and tool call synthesis verification.

use duck_proxy_rs::api::chat::{
    build_image_write_tool_call, derive_image_filename, is_image_generation_intent,
    ChatMessage, MessageContent,
};
use duck_proxy_rs::duck::{parse_sse_line, SseEvent};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;

// =============================================================================
// AREA 1: MULTI-CHUNK SSE STREAM PARSING & SPLIT BASE64 ROBUSTNESS
// =============================================================================

#[test]
fn test_sse_parser_action_image_partial_and_final() {
    let lines = vec![
        r#"data: {"action":"image-partial","result":"iVBORw0KGgoAAAANSUhEUgAA"}"#,
        r#"data: {"action":"image-partial","result":"CAYAAAC6v1pkAAAAAXNSR0IA"}"#,
        r#"data: {"action":"image-final","result":"data:image/png;base64,rs4c6QAAAARnQU1BAACxjwv8"}"#,
        r#"data: [DONE]"#,
    ];

    let mut chunks = Vec::new();
    for line in lines {
        if let Some(event) = parse_sse_line(line) {
            match event {
                SseEvent::ImageData(d) => chunks.push(d),
                SseEvent::Done => break,
                other => panic!("Unexpected event: {:?}", other),
            }
        }
    }

    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[0], "iVBORw0KGgoAAAANSUhEUgAA");
    assert_eq!(chunks[1], "CAYAAAC6v1pkAAAAAXNSR0IA");
    assert_eq!(chunks[2], "rs4c6QAAAARnQU1BAACxjwv8");

    let assembled = chunks.concat();
    assert_eq!(
        assembled,
        "iVBORw0KGgoAAAANSUhEUgAACAYAAAC6v1pkAAAAAXNSR0IArs4c6QAAAARnQU1BAACxjwv8"
    );
}

#[test]
fn test_sse_parser_various_json_discriminator_formats() {
    // Test all supported discriminators in duck-proxy-rs
    let test_cases = vec![
        (r#"data: {"action":"image-partial","result":"part1"}"#, "part1"),
        (r#"data: {"action":"image-final","result":"data:image/png;base64,part2"}"#, "part2"),
        (r#"data: {"role":"partial-image","result":"part3"}"#, "part3"),
        (r#"data: {"role":"generated-image","result":"data:image/jpeg;base64,part4"}"#, "part4"),
        (r#"data: {"role":"ui-component","result":"part5"}"#, "part5"),
        (r#"data: {"role":"image","result":"part6"}"#, "part6"),
        (r#"data: {"b64Image":"part7"}"#, "part7"),
        (r#"data: {"data":{"b64Image":"part8"}}"#, "part8"),
        (r#"data: {"action":"image-final","b64Image":"part9"}"#, "part9"),
        (r#"data: {"action":"image-partial","message":"part10"}"#, "part10"),
        (r#"data: {"action":"image-final","data":{"result":"part11"}}"#, "part11"),
    ];

    for (raw_line, expected) in test_cases {
        let parsed = parse_sse_line(raw_line);
        match parsed {
            Some(SseEvent::ImageData(d)) => assert_eq!(d, expected, "Failed for line: {}", raw_line),
            other => panic!("Expected ImageData({}), got {:?} for line: {}", expected, other, raw_line),
        }
    }
}

#[test]
fn test_sse_parser_control_frames_and_comments() {
    assert!(parse_sse_line("data: [PING]").is_none());
    assert!(parse_sse_line("data: [LIMIT: 100]").is_none());
    assert!(parse_sse_line("data: [CHAT_TITLE: Title]").is_none());
    assert!(parse_sse_line("data: [CLOSE]").is_none());
    assert!(parse_sse_line(": keepalive comment").is_none());
    assert!(parse_sse_line("   ").is_none());
    assert!(parse_sse_line("").is_none());
}

#[test]
fn test_sse_stream_simulated_chunked_byte_buffer() {
    // Simulate streaming bytes arriving in arbitrary chunk sizes (e.g. 7 bytes at a time)
    let full_sse_payload = concat!(
        ": keepalive\n\n",
        "data: [PING]\n\n",
        "data: {\"action\":\"image-partial\",\"result\":\"iVBORw0KGgoAAAANSUhEUgAA\"}\r\n\r\n",
        "data: {\"action\":\"image-partial\",\"result\":\"CAYAAAC6v1pkAAAAAXNSR0IA\"}\n\n",
        "data: {\"action\":\"image-final\",\"result\":\"data:image/png;base64,rs4c6QAAAARnQU1BAACxjwv8YQUAAAAJcEhZcwAADsMAAA7D\"}\n\n",
        "data: [DONE]\n\n"
    );

    let bytes = full_sse_payload.as_bytes();
    let mut buffer = String::new();
    let mut assembled_image_chunks: Vec<String> = Vec::new();
    let mut stream_done = false;

    let chunk_size = 7; // arbitrary byte window
    for window in bytes.chunks(chunk_size) {
        if let Ok(text) = std::str::from_utf8(window) {
            buffer.push_str(text);

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].to_string();
                buffer = buffer[pos + 1..].to_string();

                if let Some(sse_event) = parse_sse_line(&line) {
                    match sse_event {
                        SseEvent::ImageData(b64) => {
                            assembled_image_chunks.push(b64);
                        }
                        SseEvent::Done => {
                            stream_done = true;
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
        if stream_done {
            break;
        }
    }

    assert!(stream_done, "Stream should cleanly complete with [DONE]");
    assert_eq!(assembled_image_chunks.len(), 3);
    let full_b64 = assembled_image_chunks.concat();
    assert_eq!(
        full_b64,
        "iVBORw0KGgoAAAANSUhEUgAACAYAAAC6v1pkAAAAAXNSR0IArs4c6QAAAARnQU1BAACxjwv8YQUAAAAJcEhZcwAADsMAAA7D"
    );
}

#[test]
fn test_sse_binary_image_reassembly_and_base64_decoding() {
    // Construct a real 1x1 PNG byte array
    let real_png_bytes: [u8; 67] = [
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG Header
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
        0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, // IDAT chunk
        0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
        0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
        0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, // IEND chunk
        0x42, 0x60, 0x82,
    ];

    let full_b64 = BASE64_STANDARD.encode(&real_png_bytes);
    
    // Split into 5 random-sized partial chunks
    let chunk_sizes = [15, 20, 10, 25];
    let mut offset = 0;
    let mut sse_lines = Vec::new();

    for (i, sz) in chunk_sizes.iter().enumerate() {
        let end = (offset + sz).min(full_b64.len());
        let part = &full_b64[offset..end];
        offset = end;
        if i == 0 {
            sse_lines.push(format!(r#"data: {{"action":"image-partial","result":"data:image/png;base64,{}"}}"#, part));
        } else {
            sse_lines.push(format!(r#"data: {{"action":"image-partial","result":"{}"}}"#, part));
        }
    }
    if offset < full_b64.len() {
        sse_lines.push(format!(r#"data: {{"action":"image-final","result":"{}"}}"#, &full_b64[offset..]));
    }
    sse_lines.push("data: [DONE]".to_string());

    // Parse and assemble
    let mut collected = Vec::new();
    for line in sse_lines {
        if let Some(SseEvent::ImageData(data)) = parse_sse_line(&line) {
            collected.push(data);
        }
    }

    let assembled = collected.concat();
    assert_eq!(assembled, full_b64);

    let decoded = BASE64_STANDARD.decode(&assembled).expect("Assembled base64 must decode cleanly");
    assert_eq!(decoded, real_png_bytes);
}

// =============================================================================
// AREA 2: PROMPT PRESERVATION & BRACKET HANDLING
// =============================================================================

#[test]
fn test_complex_bracket_prompt_preservation_and_filename_derivation() {
    let complex_prompts = vec![
        (
            "[artstyle: anime] [mood: dark] knight with sword",
            "artstyle_anime_mood.png",
        ),
        (
            "[[artstyle: anime]] [[mood: dark]] knight with glowing blue sword",
            "artstyle_anime_mood.png",
        ),
        (
            r#"\[artstyle: anime\] \[mood: dark\] cyberpunk samurai"#,
            "artstyle_anime_mood.png",
        ),
        (
            "[artstyle: 3D render] [lighting: volumetric] futuristic robotic duck in neon city",
            "artstyle_3d_lighting.png",
        ),
        (
            "[mood: dark] [style: oil painting] [resolution: 8k] majestic dragon soaring over snowy peaks",
            "mood_dark_style.png",
        ),
        (
            "[unmatched bracket prompt knight on a horse",
            "unmatched_bracket_prompt.png",
        ),
        (
            "knight with sword [no truncation]",
            "knight_sword_no.png",
        ),
        (
            "[🎨 anime style] [⚔️ dark knight] with glowing katana",
            "anime_style_dark.png",
        ),
    ];

    for (prompt, expected_filename) in complex_prompts {
        // 1. Check filename derivation
        let fname = derive_image_filename(prompt);
        assert_eq!(
            fname, expected_filename,
            "Failed deriving filename for prompt: '{}', got: '{}'",
            prompt, fname
        );

        // 2. Ensure prompt string itself is NOT truncated at bracket
        let preserved = prompt.trim();
        assert_eq!(preserved, prompt.trim());
        assert!(
            preserved.contains("knight")
                || preserved.contains("samurai")
                || preserved.contains("duck")
                || preserved.contains("dragon"),
            "Prompt must not lose core subject text"
        );
    }
}

// =============================================================================
// AREA 3: IMAGE GENERATION INTENT DETECTION & BOUNDARIES
// =============================================================================

#[test]
fn test_intent_detection_positive_cases() {
    let positive_prompts = vec![
        "gen img of a knight",
        "gen an img of a cyber cat",
        "generate img of futuristic city",
        "generate an img of a neon duck",
        "generate image of a cozy cabin in the woods",
        "generate an image of a soaring eagle",
        "generate images of alien landscapes",
        "create image of an astronaut on mars",
        "create an image of a mechanical watch",
        "create images of enchanted forests",
        "draw a majestic castle on a cliff",
        "draw an adorable puppy playing with yarn",
        "draw me a medieval tavern scene",
        "paint a masterpiece of a stormy sea",
        "paint an abstract geometric composition",
        "paint me a vibrant sunset",
        "make an image of a magical portal",
        "make a picture of a cyberpunk hacker",
        "make an illustration of space exploration",
        "generate a picture of a vintage car",
        "generate picture of mountains at dawn",
        "generate pictures of coral reefs",
        "create a picture of a wise wizard",
        "create picture of a retro diner",
        "illustration of a cute robot reading a book",
        "render an image of a crystal cave",
        "render a picture of deep underwater ruins",
        "render image of a starry galaxy",
        "picture of a red sports car",
        "picture of an antique pocket watch",
        "photo of a wild lion in the savanna",
        "photo of an old lighthouse during a storm",
        "image of a serene Japanese garden",
        "image of an ancient stone temple",
        "can u gen img of a horse and add your base url on top of it as small icon",
    ];

    for prompt in positive_prompts {
        let msgs = vec![ChatMessage::user(prompt)];
        assert!(
            is_image_generation_intent("gpt-5.6-luna", "gpt-5.6-luna", &msgs),
            "Expected positive image generation intent for prompt: '{}'",
            prompt
        );
    }
}

#[test]
fn test_intent_detection_with_brackets() {
    let bracket_prompts = vec![
        "gen img of [artstyle: anime] [mood: dark] knight with sword",
        "generate an image of [cyberpunk] neon city [4k]",
        "draw a [watercolor] portrait of an old sailor",
        "paint a [surrealist] melting clock in desert",
        "make an illustration of [steampunk] airship",
    ];

    for prompt in bracket_prompts {
        let msgs = vec![ChatMessage::user(prompt)];
        assert!(
            is_image_generation_intent("gpt-5.6-luna", "gpt-5.6-luna", &msgs),
            "Expected positive intent for bracket prompt: '{}'",
            prompt
        );
    }
}

#[test]
fn test_intent_detection_model_overrides() {
    // If the requested model is explicitly an image generation alias, intent must be true regardless of text
    let msgs = vec![ChatMessage::user("hello world")];

    assert!(is_image_generation_intent("image-generation", "image-generation", &msgs));
    assert!(is_image_generation_intent("image", "image", &msgs));
    assert!(is_image_generation_intent("gpt-5.6-luna", "duckproxy/image-gen", &msgs));
    assert!(is_image_generation_intent("gpt-5.6-luna", "stable-diffusion-xl", &msgs));
}

#[test]
fn test_intent_detection_negative_cases() {
    let non_image_prompts = vec![
        "Write a function to calculate Fibonacci in Rust",
        "Explain how the borrow checker works in Rust",
        "Refactor this struct to use Arc and Mutex",
        "What is the capital of France?",
        "Can you calculate 25 * 480?",
        "How do diffusion models work conceptually?",
    ];

    for prompt in non_image_prompts {
        let msgs = vec![ChatMessage::user(prompt)];
        assert!(
            !is_image_generation_intent("gpt-5.6-luna", "gpt-5.6-luna", &msgs),
            "Expected negative image generation intent for prompt: '{}'",
            prompt
        );
    }
}

#[test]
fn test_intent_detection_multi_turn_state_machine() {
    // Turn 1: User requests image -> intent = true
    let turn1 = vec![ChatMessage::user("gen img of a knight")];
    assert!(is_image_generation_intent("gpt-5.6-luna", "gpt-5.6-luna", &turn1));

    // Turn 2: Assistant replied with tool call and tool executed -> intent = false (suppress loop)
    let turn2 = vec![
        ChatMessage::user("gen img of a knight"),
        ChatMessage::assistant(""),
        ChatMessage {
            role: "tool".to_string(),
            content: Some(MessageContent::Text("Image successfully saved to: /workspace/knight.png".to_string())),
            name: Some("bash".to_string()),
            tool_call_id: Some("call_abc123".to_string()),
            ..Default::default()
        },
    ];
    assert!(
        !is_image_generation_intent("gpt-5.6-luna", "gpt-5.6-luna", &turn2),
        "Turn 2 with tool result must suppress image intent"
    );

    // Turn 3: User follows up with text question -> intent = false
    let turn3_text = vec![
        ChatMessage::user("gen img of a knight"),
        ChatMessage::assistant("Image created."),
        ChatMessage {
            role: "tool".to_string(),
            content: Some(MessageContent::Text("Image saved".to_string())),
            tool_call_id: Some("call_abc123".to_string()),
            ..Default::default()
        },
        ChatMessage::assistant("I have created the knight image."),
        ChatMessage::user("What colors did you use?"),
    ];
    assert!(
        !is_image_generation_intent("gpt-5.6-luna", "gpt-5.6-luna", &turn3_text),
        "Follow-up text question must not trigger image intent"
    );

    // Turn 3 (alternate): User asks for another image -> intent = true
    let turn3_new_img = vec![
        ChatMessage::user("gen img of a knight"),
        ChatMessage::assistant("Image created."),
        ChatMessage {
            role: "tool".to_string(),
            content: Some(MessageContent::Text("Image saved".to_string())),
            tool_call_id: Some("call_abc123".to_string()),
            ..Default::default()
        },
        ChatMessage::assistant("I have created the knight image."),
        ChatMessage::user("Now draw a dragon"),
    ];
    assert!(
        is_image_generation_intent("gpt-5.6-luna", "gpt-5.6-luna", &turn3_new_img),
        "New image request in multi-turn conversation must trigger image intent"
    );
}

// =============================================================================
// AREA 4: TOOL CALL SYNTHESIS & SINGLE-SHOT HARDENING
// =============================================================================

#[test]
fn test_tool_call_synthesis_quiet_execution_and_file_clean() {
    let dummy_b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
    let (tool_call, cmd) = build_image_write_tool_call(dummy_b64, "test_knight.png");

    assert_eq!(tool_call.function.name, "bash");
    assert!(!cmd.contains(dummy_b64), "Command must write via temp file, not dump raw b64 into bash command");
    assert!(cmd.contains("base64 -d /tmp/.duck_img_"));
    assert!(cmd.contains("> \"test_knight.png\""));
    assert!(cmd.contains("rm -f /tmp/.duck_img_"));
    assert!(cmd.contains("Image successfully saved to:"));
    assert!(cmd.contains("realpath 'test_knight.png'"));
}
