//! SSE stream parser for Duck.ai responses.

/// Parsed SSE event from the Duck.ai upstream.
#[derive(Debug, Clone)]
pub enum SseEvent {
    /// A text token chunk.
    Token(String),
    /// Stream is done.
    Done,
    /// Base64 image data received.
    ImageData(String),
    /// An error from the upstream.
    Error(String),
}

/// Helper to extract clean base64 data, removing data URI prefixes if present.
fn extract_clean_b64(res: &str) -> String {
    if res.starts_with("data:image/") || res.starts_with("data:") {
        if let Some((_, b64_part)) = res.split_once(',') {
            b64_part.to_string()
        } else {
            res.to_string()
        }
    } else {
        res.to_string()
    }
}

/// Parses a single SSE line from the Duck.ai response stream.
///
/// Duck.ai sends lines like:
/// - `data: [DONE]`
/// - `data: {"role":"assistant","message":"hello"}`
/// - `data: {"action":"image-partial","result":"..."}`
/// - `data: {"action":"image-final","result":"..."}`
/// - `data: {"role":"partial-image","result":"..."}`
/// - `data: {"role":"generated-image","result":"..."}`
/// - `data: {"b64Image":"..."}`
/// - `data: {"data":{"b64Image":"..."}}`
pub fn parse_sse_line(line: &str) -> Option<SseEvent> {
    let line = line.trim();

    if line.is_empty() || line.starts_with(':') {
        return None;
    }

    let data = if let Some(stripped) = line.strip_prefix("data: ") {
        stripped
    } else {
        let stripped = line.strip_prefix("data:")?;
        stripped.trim_start()
    };

    if data == "[DONE]" {
        return Some(SseEvent::Done);
    }

    // Ignore control frames: starts with '[' (e.g. [PING], [LIMIT: ...], [CHAT_TITLE: ...])
    if data.starts_with('[') {
        return None;
    }

    // Try parsing as JSON
    match serde_json::from_str::<serde_json::Value>(data) {
        Ok(json) => {
            // Check for top-level b64Image
            if let Some(b64) = json.get("b64Image").and_then(|v| v.as_str()) {
                if !b64.is_empty() {
                    return Some(SseEvent::ImageData(extract_clean_b64(b64)));
                }
            }

            // Check for nested data.b64Image
            if let Some(b64) = json
                .get("data")
                .and_then(|d| d.get("b64Image"))
                .and_then(|v| v.as_str())
            {
                if !b64.is_empty() {
                    return Some(SseEvent::ImageData(extract_clean_b64(b64)));
                }
            }

            // Check for action: "image-partial" or "image-final"
            if let Some(action) = json.get("action").and_then(|v| v.as_str()) {
                if action == "image-partial" || action == "image-final" {
                    let maybe_val = json
                        .get("result")
                        .or_else(|| json.get("b64Image"))
                        .or_else(|| json.get("image"))
                        .or_else(|| json.get("message"))
                        .and_then(|v| v.as_str())
                        .or_else(|| {
                            json.get("data").and_then(|d| {
                                if let Some(s) = d.as_str() {
                                    Some(s)
                                } else {
                                    d.get("b64Image")
                                        .or_else(|| d.get("result"))
                                        .and_then(|v| v.as_str())
                                }
                            })
                        });
                    if let Some(res) = maybe_val {
                        let img_data = extract_clean_b64(res);
                        if !img_data.is_empty() {
                            return Some(SseEvent::ImageData(img_data));
                        }
                    }
                }
            }

            // Check for image/component roles (generated-image, partial-image, ui-component, image)
            if let Some(role) = json.get("role").and_then(|v| v.as_str()) {
                if role == "generated-image"
                    || role == "partial-image"
                    || role == "ui-component"
                    || role == "image"
                {
                    let maybe_val = json
                        .get("result")
                        .or_else(|| json.get("b64Image"))
                        .or_else(|| json.get("image"))
                        .and_then(|v| v.as_str())
                        .or_else(|| {
                            json.get("data").and_then(|d| {
                                if let Some(s) = d.as_str() {
                                    Some(s)
                                } else {
                                    d.get("b64Image")
                                        .or_else(|| d.get("result"))
                                        .and_then(|v| v.as_str())
                                }
                            })
                        });
                    if let Some(res) = maybe_val {
                        let img_data = extract_clean_b64(res);
                        if !img_data.is_empty() {
                            return Some(SseEvent::ImageData(img_data));
                        }
                    }
                }
            }

            // Check for error action
            if let Some(err_action) = json.get("action").and_then(|v| v.as_str()) {
                if err_action == "error" {
                    let msg = json
                        .get("message")
                        .or_else(|| json.get("type"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown upstream error");
                    return Some(SseEvent::Error(msg.to_string()));
                }
            }

            // Check for text message
            if let Some(message) = json.get("message").and_then(|v| v.as_str()) {
                return Some(SseEvent::Token(message.to_string()));
            }

            // If it has a role but no message, skip (initial role announcement)
            if json.get("role").is_some() && json.get("message").is_none() {
                return None;
            }

            None
        }
        Err(_) => {
            // Not JSON — treat raw data as a token if non-empty and not a control frame
            if !data.is_empty() && !data.starts_with('[') {
                Some(SseEvent::Token(data.to_string()))
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_done() {
        let event = parse_sse_line("data: [DONE]");
        assert!(matches!(event, Some(SseEvent::Done)));
    }

    #[test]
    fn test_parse_control_frames() {
        assert!(parse_sse_line("data: [PING]").is_none());
        assert!(parse_sse_line("data: [LIMIT: 100]").is_none());
        assert!(parse_sse_line("data: [CHAT_TITLE: Title]").is_none());
        assert!(parse_sse_line("data: [CLOSE]").is_none());
    }

    #[test]
    fn test_parse_token() {
        let event = parse_sse_line(r#"data: {"role":"assistant","message":"Hello"}"#);
        match event {
            Some(SseEvent::Token(t)) => assert_eq!(t, "Hello"),
            other => panic!("Expected Token, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_image_data() {
        let event = parse_sse_line(r#"data: {"b64Image":"abc123base64data"}"#);
        match event {
            Some(SseEvent::ImageData(d)) => assert_eq!(d, "abc123base64data"),
            other => panic!("Expected ImageData, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_nested_image_data() {
        let event = parse_sse_line(r#"data: {"role":"assistant","data":{"b64Image":"nested_b64"}}"#);
        match event {
            Some(SseEvent::ImageData(d)) => assert_eq!(d, "nested_b64"),
            other => panic!("Expected ImageData, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_generated_image_data_uri() {
        let event = parse_sse_line(
            r#"data: {"role":"generated-image","result":"data:image/png;base64,iVBORw0KGgoAAAANSUhEUg"}"#,
        );
        match event {
            Some(SseEvent::ImageData(d)) => assert_eq!(d, "iVBORw0KGgoAAAANSUhEUg"),
            other => panic!("Expected ImageData, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_partial_image_data() {
        let event = parse_sse_line(r#"data: {"role":"partial-image","result":"chunk1_data"}"#);
        match event {
            Some(SseEvent::ImageData(d)) => assert_eq!(d, "chunk1_data"),
            other => panic!("Expected ImageData, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_action_image_partial() {
        let event = parse_sse_line(r#"data: {"action":"image-partial","result":"action_partial_chunk"}"#);
        match event {
            Some(SseEvent::ImageData(d)) => assert_eq!(d, "action_partial_chunk"),
            other => panic!("Expected ImageData, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_action_image_final() {
        let event = parse_sse_line(r#"data: {"action":"image-final","result":"data:image/png;base64,action_final_b64"}"#);
        match event {
            Some(SseEvent::ImageData(d)) => assert_eq!(d, "action_final_b64"),
            other => panic!("Expected ImageData, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_action_image_final_b64_field() {
        let event = parse_sse_line(r#"data: {"action":"image-final","b64Image":"final_b64_direct"}"#);
        match event {
            Some(SseEvent::ImageData(d)) => assert_eq!(d, "final_b64_direct"),
            other => panic!("Expected ImageData, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_error() {
        let event = parse_sse_line(r#"data: {"action":"error","message":"rate limited"}"#);
        match event {
            Some(SseEvent::Error(msg)) => assert_eq!(msg, "rate limited"),
            other => panic!("Expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_empty_line() {
        assert!(parse_sse_line("").is_none());
    }

    #[test]
    fn test_parse_comment_line() {
        assert!(parse_sse_line(": this is a comment").is_none());
    }

    #[test]
    fn test_parse_role_only() {
        let event = parse_sse_line(r#"data: {"role":"assistant"}"#);
        assert!(event.is_none());
    }

    #[test]
    fn test_parse_no_data_prefix() {
        assert!(parse_sse_line("event: message").is_none());
    }
}
