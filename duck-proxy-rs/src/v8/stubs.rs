//! Browser DOM and environment stubs for V8 challenge evaluation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Embedded browser stubs template loaded at compile time.
pub const BROWSER_STUBS_TEMPLATE: &str = include_str!("stubs.js");

/// Represents an entry in the HTML lookup table for DOM element parsing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HtmlLookupEntry {
    pub html: String,
    pub count: usize,
}

/// Generates the complete JavaScript stubs source by replacing placeholders with
/// the target User-Agent string and serialized HTML lookup table.
pub fn generate_browser_stubs(user_agent: &str, html_lookup_json: Option<&str>) -> String {
    let ua_escaped = serde_json::to_string(user_agent)
        .unwrap_or_else(|_| format!("\"{}\"", user_agent.replace('"', "\\\"")));
    let lookup_str = html_lookup_json.unwrap_or("{}");

    BROWSER_STUBS_TEMPLATE
        .replace("__DDG_REAL_UA__", &ua_escaped)
        .replace("__DDG_HTML_LOOKUP__", lookup_str)
}

/// Wraps a raw JavaScript challenge expression in a Promise handler that records
/// the resolved result into `__R` or caught error into `__E`.
pub fn wrap_challenge_code(challenge_js: &str) -> String {
    format!(
        "Promise.resolve({}).then(function(v){{ __R = v; }}).catch(function(e){{ __E = String((e && e.stack) || e); }});",
        challenge_js
    )
}

/// Extracts potential HTML string literals from challenge JavaScript code and constructs
/// a lookup table mapping raw snippets to normalized HTML and tag counts.
pub fn extract_html_lookup(js_code: &str) -> HashMap<String, HtmlLookupEntry> {
    let mut lookup = HashMap::new();
    let mut seen = std::collections::HashSet::new();

    // Simple parser for HTML tags in single/double quoted strings
    let chars: Vec<char> = js_code.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if (chars[i] == '\'' || chars[i] == '"') && i + 2 < len && chars[i + 1] == '<' {
            let quote = chars[i];
            let start = i + 1;
            let mut j = start;
            let mut escaped = false;

            while j < len && (chars[j] != quote || escaped) && (j - start) < 400 {
                escaped = chars[j] == '\\' && !escaped;
                j += 1;
            }

            if j < len && chars[j] == quote {
                let candidate: String = chars[start..j].iter().collect();
                if !candidate.is_empty() && !seen.contains(&candidate) {
                    seen.insert(candidate.clone());
                    let count = count_html_elements(&candidate);
                    lookup.insert(
                        candidate.clone(),
                        HtmlLookupEntry {
                            html: candidate,
                            count,
                        },
                    );
                }
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }

    lookup
}

/// Counts the number of HTML opening tags in a fragment.
fn count_html_elements(html: &str) -> usize {
    let mut count = 0;
    let mut in_tag = false;
    let mut tag_name = String::new();
    let chars: Vec<char> = html.chars().collect();

    for i in 0..chars.len() {
        if chars[i] == '<' && i + 1 < chars.len() && chars[i + 1] != '/' && chars[i + 1] != '!' {
            in_tag = true;
            tag_name.clear();
        } else if in_tag {
            if chars[i].is_whitespace() || chars[i] == '>' || chars[i] == '/' {
                if !tag_name.is_empty() {
                    count += 1;
                    in_tag = false;
                }
            } else if chars[i].is_ascii_alphanumeric() {
                tag_name.push(chars[i]);
            }
        }
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stubs_template_contains_placeholders() {
        assert!(BROWSER_STUBS_TEMPLATE.contains("__DDG_REAL_UA__"));
        assert!(BROWSER_STUBS_TEMPLATE.contains("__DDG_HTML_LOOKUP__"));
        assert!(BROWSER_STUBS_TEMPLATE.contains("__defGlobal('window'"));
        assert!(BROWSER_STUBS_TEMPLATE.contains("__defGlobal('document'"));
        assert!(BROWSER_STUBS_TEMPLATE.contains("__defGlobal('navigator'"));
        assert!(BROWSER_STUBS_TEMPLATE.contains("__defGlobal('screen'"));
        assert!(BROWSER_STUBS_TEMPLATE.contains("__defGlobal('location'"));
        assert!(BROWSER_STUBS_TEMPLATE.contains("__defGlobal('getComputedStyle'"));
        assert!(BROWSER_STUBS_TEMPLATE.contains("__defGlobal('__DDG_BE_VERSION__'"));
    }

    #[test]
    fn test_generate_browser_stubs_replaces_placeholders() {
        let ua = "Mozilla/5.0 (Test UA) Chrome/150.0.0.0";
        let stubs = generate_browser_stubs(ua, None);
        assert!(!stubs.contains("__DDG_REAL_UA__"));
        assert!(!stubs.contains("__DDG_HTML_LOOKUP__"));
        assert!(stubs.contains("\"Mozilla/5.0 (Test UA) Chrome/150.0.0.0\""));
        assert!(stubs.contains("{}"));
    }

    #[test]
    fn test_generate_browser_stubs_with_custom_lookup() {
        let ua = "CustomAgent/1.0";
        let lookup_json = "{\"<div></div>\":{\"html\":\"<div></div>\",\"count\":1}}";
        let stubs = generate_browser_stubs(ua, Some(lookup_json));
        assert!(stubs.contains(lookup_json));
    }

    #[test]
    fn test_wrap_challenge_code() {
        let code = "async function() { return { ok: true }; }()";
        let wrapped = wrap_challenge_code(code);
        assert!(wrapped.starts_with("Promise.resolve("));
        assert!(wrapped.contains(".then(function(v){ __R = v; })"));
        assert!(wrapped.contains(".catch(function(e){ __E = String((e && e.stack) || e); });"));
    }

    #[test]
    fn test_extract_html_lookup() {
        let js = "var x = '<div><span>test</span></div>'; var y = '<p class=\"abc\">hello</p>';";
        let lookup = extract_html_lookup(js);
        assert_eq!(lookup.len(), 2);
        assert!(lookup.contains_key("<div><span>test</span></div>"));
        assert!(lookup.contains_key("<p class=\"abc\">hello</p>"));
        assert_eq!(lookup.get("<div><span>test</span></div>").unwrap().count, 2);
        assert_eq!(lookup.get("<p class=\"abc\">hello</p>").unwrap().count, 1);
    }
}
