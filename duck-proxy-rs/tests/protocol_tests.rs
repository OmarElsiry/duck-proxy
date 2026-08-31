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
