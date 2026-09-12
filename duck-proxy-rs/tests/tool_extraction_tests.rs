use duck_proxy_rs::api::chat::extract_tool_calls;

#[test]
fn test_extract_tag_tool_call() {
    let text = "Here is the command:\n<tool_call>\n{\"name\": \"bash\", \"arguments\": {\"command\": \"ls -la\"}}\n</tool_call>\nPlease check.";
    let tool_calls = extract_tool_calls(text);
    assert!(tool_calls.is_some(), "Should extract tag tool call");
    let tcs = tool_calls.unwrap();
    assert_eq!(tcs.len(), 1);
    assert_eq!(tcs[0].function.name, "bash");
    assert!(tcs[0].function.arguments.contains("ls -la"));
}

#[test]
fn test_extract_json_block_tool_call() {
    let text = "```json\n{\n  \"name\": \"bash\",\n  \"arguments\": {\n    \"command\": \"pwd\"\n  }\n}\n```";
    let tool_calls = extract_tool_calls(text);
    assert!(tool_calls.is_some(), "Should extract json code block tool call");
    let tcs = tool_calls.unwrap();
    assert_eq!(tcs.len(), 1);
    assert_eq!(tcs[0].function.name, "bash");
    assert!(tcs[0].function.arguments.contains("pwd"));
}

#[test]
fn test_extract_implicit_file_code_block() {
    let text = "I will create `main.py` with the following code:\n```python\ndef main():\n    print('Hello World')\n\nif __name__ == '__main__':\n    main()\n```";
    let tool_calls = extract_tool_calls(text);
    assert!(tool_calls.is_some(), "Should extract implicit markdown code block");
    let tcs = tool_calls.unwrap();
    assert_eq!(tcs.len(), 1);
    assert_eq!(tcs[0].function.name, "bash");
    assert!(tcs[0].function.arguments.contains("cat << 'EOF' > main.py"));
    assert!(tcs[0].function.arguments.contains("def main():"));
}

#[test]
fn test_normalize_write_to_bash() {
    let text = "<tool_call>{\"name\": \"write\", \"arguments\": {\"filePath\": \"config.json\", \"content\": \"{\\\"debug\\\": true}\"}}</tool_call>";
    let tool_calls = extract_tool_calls(text);
    assert!(tool_calls.is_some());
    let tcs = tool_calls.unwrap();
    assert_eq!(tcs.len(), 1);
    assert_eq!(tcs[0].function.name, "bash");
    assert!(tcs[0].function.arguments.contains("config.json"));
    assert!(tcs[0].function.arguments.contains("cat << 'EOF' > config.json"));
}

#[test]
fn test_heredoc_sanitization() {
    let text = "```bash\ncat << 'EOF' > readme.md\n# Title\nDescription\nEOF\n```";
    let tool_calls = extract_tool_calls(text);
    assert!(tool_calls.is_some());
    let tcs = tool_calls.unwrap();
    assert_eq!(tcs.len(), 1);
    let cmd = &tcs[0].function.arguments;
    assert!(!cmd.contains("cat << 'EOF' > readme.md\ncat << 'EOF'"), "Should not contain nested duplicate heredocs");
}

#[test]
fn test_resolve_tool_name_casing() {
    use duck_proxy_rs::api::chat::{resolve_tool_name, ToolDefinition, FunctionDefinition};

    let client_tools = vec![
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "Bash".to_string(),
                description: Some("Execute command".to_string()),
                parameters: None,
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "Write".to_string(),
                description: Some("Write file".to_string()),
                parameters: None,
            },
        },
    ];

    assert_eq!(resolve_tool_name("bash", Some(&client_tools)), "Bash");
    assert_eq!(resolve_tool_name("BASH", Some(&client_tools)), "Bash");
    assert_eq!(resolve_tool_name("Bash", Some(&client_tools)), "Bash");
    assert_eq!(resolve_tool_name("write", Some(&client_tools)), "Write");
    assert_eq!(resolve_tool_name("unknown", Some(&client_tools)), "unknown");
}

#[test]
fn test_markdown_code_block_resolves_to_client_bash() {
    use duck_proxy_rs::api::chat::{extract_tool_calls_with_tools, ToolDefinition, FunctionDefinition};

    let client_tools = vec![
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: "Bash".to_string(),
                description: Some("Execute bash command".to_string()),
                parameters: None,
            },
        },
    ];

    let text = "You can run these commands:\n```bash\ngit clone https://github.com/omarelsiry/Learnopia.git\ncd Learnopia\nflutter pub get\n```";
    let tool_calls = extract_tool_calls_with_tools(text, Some(&client_tools));
    assert!(tool_calls.is_some(), "Should extract markdown code block");
    let tcs = tool_calls.unwrap();
    assert_eq!(tcs.len(), 1);
    assert_eq!(tcs[0].function.name, "Bash", "Tool name must match client tool casing 'Bash'");
    assert!(tcs[0].function.arguments.contains("git clone https://github.com/omarelsiry/Learnopia.git"));
}

#[test]
fn test_sanitize_assistant_content() {
    use duck_proxy_rs::api::chat::sanitize_assistant_content;

    let refusal_with_tools = "The `bash` tool isn’t available in this session, so those calls cannot execute. The available tools here are web search, document retrieval, related-search suggestions, and image generation.";
    assert_eq!(sanitize_assistant_content(refusal_with_tools, true), None);

    let refusal_without_tools = "The `bash` tool isn’t available in this session, so those calls cannot execute. The available tools here are web search, document retrieval, related-search suggestions, and image generation.";
    let cleaned = sanitize_assistant_content(refusal_without_tools, false);
    assert!(cleaned.is_some());
    assert!(!cleaned.unwrap().contains("The `bash` tool isn’t available"));
}
