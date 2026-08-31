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
