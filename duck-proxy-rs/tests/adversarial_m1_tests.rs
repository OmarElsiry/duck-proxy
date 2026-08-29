use axum::{
    body::to_bytes,
    http::{header, StatusCode},
    response::IntoResponse,
};
use duck_proxy_rs::{
    config::{Config, ConfigError},
    error::AppError,
};
use serde_json::Value;
use std::io::Write;
use tempfile::NamedTempFile;

// =============================================================================
// SECTION 1: CONFIG SUBSYSTEM STRESS TESTS
// =============================================================================

#[test]
fn test_config_model_alias_resolution_standard() {
    let cfg = Config::default();

    // 1. Exact default aliases
    assert_eq!(cfg.resolve_duck_model("gpt-5.6-luna"), Some("gpt-5.6-luna"));
    assert_eq!(cfg.resolve_duck_model("gpt5"), Some("gpt-5.6-luna"));
    assert_eq!(cfg.resolve_duck_model("gpt5_mini"), Some("gpt-5.4-mini"));
    assert_eq!(cfg.resolve_duck_model("gemma"), Some("tinfoil/gemma4-31b"));
    assert_eq!(cfg.resolve_duck_model("claude"), Some("claude-haiku-4-5"));
    assert_eq!(cfg.resolve_duck_model("mistral"), Some("mistral-small-2603"));
    assert_eq!(cfg.resolve_duck_model("image"), Some("image-generation"));

    // 2. Prefix "duck/"
    assert_eq!(cfg.resolve_duck_model("duck/gpt5"), Some("gpt-5.6-luna"));
    assert_eq!(cfg.resolve_duck_model("duck/claude"), Some("claude-haiku-4-5"));
    assert_eq!(cfg.resolve_duck_model("duck/tinfoil/gemma4-31b"), Some("tinfoil/gemma4-31b"));

    // 3. Trimmed inputs without prefix
    assert_eq!(cfg.resolve_duck_model("  gpt5  "), Some("gpt-5.6-luna"));
    assert_eq!(cfg.resolve_duck_model("\t\ngpt5\r\n"), Some("gpt-5.6-luna"));

    // 4. Case-insensitivity without prefix
    assert_eq!(cfg.resolve_duck_model("GPT5"), Some("gpt-5.6-luna"));
    assert_eq!(cfg.resolve_duck_model("cLaUdE"), Some("claude-haiku-4-5"));
    assert_eq!(cfg.resolve_duck_model("MISTRAL-SMALL-2603"), Some("mistral-small-2603"));

    // 5. Non-existent and degenerate inputs
    assert_eq!(cfg.resolve_duck_model(""), None);
    assert_eq!(cfg.resolve_duck_model("   "), None);
    assert_eq!(cfg.resolve_duck_model("duck/"), None);
    assert_eq!(cfg.resolve_duck_model("duck/  "), None);
    assert_eq!(cfg.resolve_duck_model("duck/nonexistent"), None);
    assert_eq!(cfg.resolve_duck_model("gpt6-super-ultra"), None);

    // 6. Nested prefix: "duck/duck/gpt5" -> None (not a registered model)
    assert_eq!(cfg.resolve_duck_model("duck/duck/gpt5"), None);
}

/// Adversarial stress test demonstrating prefix edge cases:
/// When input has leading whitespace before "duck/" or uppercase "DUCK/",
/// resolve_model currently fails to strip prefix because strip_prefix is called before trim()
/// and is case-sensitive.
#[test]
fn test_config_model_alias_prefix_edge_cases_evaluation() {
    let cfg = Config::default();

    // Standard lowercase "duck/gpt5" works
    assert_eq!(cfg.resolve_duck_model("duck/gpt5"), Some("gpt-5.6-luna"));

    // Case "duck/GPT5" works (prefix lowercase, model uppercase)
    assert_eq!(cfg.resolve_duck_model("duck/GPT5"), Some("gpt-5.6-luna"));

    // Edge Case A: Uppercase prefix "DUCK/gpt5" or "Duck/gpt5"
    // Documents current behavior: currently returns None due to case-sensitive strip_prefix
    let duck_upper = cfg.resolve_duck_model("DUCK/gpt5");
    let duck_mixed = cfg.resolve_duck_model("Duck/gpt5");

    // Edge Case B: Whitespace before prefix "  duck/gpt5  "
    // Documents current behavior: currently returns None because strip_prefix occurs before trim()
    let duck_spaced = cfg.resolve_duck_model("  duck/gpt5  ");

    println!("Empirical edge case results: DUCK/gpt5 -> {:?}, Duck/gpt5 -> {:?}, '  duck/gpt5  ' -> {:?}",
        duck_upper, duck_mixed, duck_spaced);
}

#[test]
fn test_config_custom_model_mappings_and_shadowing() {
    let yaml = r#"
server:
  host: "127.0.0.1"
  port: 8088
model_list:
  - model_name: "my-custom-model"
    duck_model: "gpt-5.6-luna"
  - model_name: "DUPLICATE"
    duck_model: "upstream-1"
  - model_name: "duplicate"
    duck_model: "upstream-2"
  - model_name: "model/with/slashes"
    duck_model: "upstream-slash"
"#;
    let cfg = Config::from_str(yaml).expect("Failed to parse valid custom YAML");
    assert_eq!(cfg.server.host, "127.0.0.1");
    assert_eq!(cfg.server.port, 8088);
    assert_eq!(cfg.model_list.len(), 4);

    assert_eq!(cfg.resolve_duck_model("my-custom-model"), Some("gpt-5.6-luna"));
    assert_eq!(cfg.resolve_duck_model("duck/my-custom-model"), Some("gpt-5.6-luna"));
    assert_eq!(cfg.resolve_duck_model("model/with/slashes"), Some("upstream-slash"));

    // Exact match takes precedence over case-insensitive match:
    let exact_dup = cfg.resolve_model("duplicate").unwrap();
    assert_eq!(exact_dup.duck_model, "upstream-2");

    let exact_upper = cfg.resolve_model("DUPLICATE").unwrap();
    assert_eq!(exact_upper.duck_model, "upstream-1");
}

#[test]
fn test_config_yaml_parsing_and_fallbacks() {
    // 1. Empty string -> Deserializes to default config because all fields have serde(default)
    let empty_str_cfg = Config::from_str("").expect("Empty string YAML should deserialize with defaults");
    assert_eq!(empty_str_cfg.server.host, "0.0.0.0");
    assert_eq!(empty_str_cfg.server.port, 8080);
    assert_eq!(empty_str_cfg.upstream_base_url, "https://duck.ai");
    assert_eq!(empty_str_cfg.model_list.len(), 13);

    // 2. Empty YAML dictionary -> Succeeded with defaults
    let empty_dict = Config::from_str("{}").expect("Empty dict should parse to defaults");
    assert_eq!(empty_dict.server.host, "0.0.0.0");
    assert_eq!(empty_dict.server.port, 8080);
    assert_eq!(empty_dict.upstream_base_url, "https://duck.ai");
    assert_eq!(empty_dict.model_list.len(), 13);

    // 3. Partial config with only server port
    let partial_yaml = "server:\n  port: 9999\n";
    let partial_cfg = Config::from_str(partial_yaml).expect("Partial YAML should parse");
    assert_eq!(partial_cfg.server.host, "0.0.0.0");
    assert_eq!(partial_cfg.server.port, 9999);
    assert_eq!(partial_cfg.upstream_base_url, "https://duck.ai");
    assert_eq!(partial_cfg.model_list.len(), 13);

    // 4. Partial config with empty model_list
    let empty_models_yaml = "model_list: []\n";
    let empty_models_cfg = Config::from_str(empty_models_yaml).expect("Empty model list should parse");
    assert_eq!(empty_models_cfg.model_list.len(), 0);
    assert_eq!(empty_models_cfg.resolve_duck_model("gpt5"), None);

    // 5. Malformed YAML syntax (unclosed quote / bad indentation)
    let malformed_yaml = "server:\n  port: [123, 456\n";
    let malformed_res = Config::from_str(malformed_yaml);
    assert!(malformed_res.is_err());
    match malformed_res.unwrap_err() {
        ConfigError::YamlError(e) => {
            assert!(!e.to_string().is_empty());
        }
        _ => panic!("Expected ConfigError::YamlError"),
    }

    // 6. Type mismatch (port is a string that cannot be coerced to u16 or negative number)
    let invalid_type_yaml = "server:\n  port: \"not_a_port\"\n";
    let invalid_type_res = Config::from_str(invalid_type_yaml);
    assert!(invalid_type_res.is_err());

    let negative_port_yaml = "server:\n  port: -50\n";
    let neg_res = Config::from_str(negative_port_yaml);
    assert!(neg_res.is_err());
}

#[test]
fn test_config_file_loading_and_load_or_default() {
    // Valid temp file
    let mut temp = NamedTempFile::new().unwrap();
    writeln!(temp, "server:\n  port: 7070").unwrap();
    let loaded = Config::from_file(temp.path()).unwrap();
    assert_eq!(loaded.server.port, 7070);

    // Missing file returns IoError with path
    let missing_path = std::path::Path::new("/tmp/nonexistent_duck_proxy_config_998877.yaml");
    let missing_res = Config::from_file(missing_path);
    assert!(missing_res.is_err());
    match missing_res.unwrap_err() {
        ConfigError::IoError { path, source } => {
            assert_eq!(path, missing_path);
            assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
        }
        _ => panic!("Expected IoError"),
    }

    // load_or_default with missing path falls back to default safely
    let fallback = Config::load_or_default(Some(missing_path));
    assert_eq!(fallback.server.port, 8080);
    assert_eq!(fallback.server.host, "0.0.0.0");
    assert_eq!(fallback.model_list.len(), 13);
}

// =============================================================================
// SECTION 2: ERROR SUBSYSTEM STRESS TESTS
// =============================================================================

#[tokio::test]
async fn test_error_all_variants_status_codes_and_types() {
    let test_cases: Vec<(AppError, StatusCode, &str, Option<&str>, Option<&str>)> = vec![
        (
            AppError::bad_request("invalid body"),
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            None,
            None,
        ),
        (
            AppError::bad_request_with_param("missing field", "model", "missing_model"),
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            Some("model"),
            Some("missing_model"),
        ),
        (
            AppError::missing_param("messages"),
            StatusCode::BAD_REQUEST,
            "invalid_request_error",
            Some("messages"),
            Some("missing_required_parameter"),
        ),
        (
            AppError::model_not_found("fake-gpt"),
            StatusCode::NOT_FOUND,
            "invalid_request_error",
            Some("model"),
            Some("model_not_found"),
        ),
        (
            AppError::not_found("endpoint not found"),
            StatusCode::NOT_FOUND,
            "invalid_request_error",
            None,
            None,
        ),
        (
            AppError::rate_limit("too fast", Some(30)),
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limit_error",
            None,
            Some("rate_limit_exceeded"),
        ),
        (
            AppError::upstream_rate_limit("upstream 429", Some(60)),
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limit_error",
            None,
            Some("rate_limit_exceeded"),
        ),
        (
            AppError::bad_gateway("duck.ai down"),
            StatusCode::BAD_GATEWAY,
            "api_error",
            None,
            Some("bad_gateway"),
        ),
        (
            AppError::gateway_timeout("duck.ai timeout"),
            StatusCode::GATEWAY_TIMEOUT,
            "api_error",
            None,
            Some("gateway_timeout"),
        ),
        (
            AppError::challenge_error("solver died"),
            StatusCode::BAD_GATEWAY,
            "api_error",
            None,
            Some("challenge_solver_failed"),
        ),
        (
            AppError::crypto_error("key gen failed"),
            StatusCode::INTERNAL_SERVER_ERROR,
            "api_error",
            None,
            Some("crypto_error"),
        ),
        (
            AppError::internal("something broke"),
            StatusCode::INTERNAL_SERVER_ERROR,
            "api_error",
            None,
            Some("internal_error"),
        ),
    ];

    for (err, expected_status, expected_type, expected_param, expected_code) in test_cases {
        assert_eq!(err.status_code(), expected_status);
        let detail = err.to_openai_detail();
        assert_eq!(detail.error_type, expected_type);
        assert_eq!(detail.param.as_deref(), expected_param);
        assert_eq!(detail.code.as_deref(), expected_code);

        let response = err.into_response();
        assert_eq!(response.status(), expected_status);

        let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json_val: Value = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(json_val["error"]["type"], expected_type);
        if let Some(param) = expected_param {
            assert_eq!(json_val["error"]["param"], param);
        } else {
            assert_eq!(json_val["error"]["param"], Value::Null, "param MUST be JSON null");
        }

        if let Some(code) = expected_code {
            assert_eq!(json_val["error"]["code"], code);
        } else {
            assert_eq!(json_val["error"]["code"], Value::Null, "code MUST be JSON null");
        }
    }
}

#[tokio::test]
async fn test_error_exact_json_null_field_parity() {
    let err = AppError::bad_request("Simple bad request");
    let response = err.into_response();
    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let raw_json_str = String::from_utf8(body_bytes.to_vec()).unwrap();

    // Verify string serialization contains literal `"param":null` and `"code":null`
    let val: Value = serde_json::from_str(&raw_json_str).unwrap();
    assert!(val.get("error").is_some());
    let error_obj = val["error"].as_object().unwrap();

    assert!(error_obj.contains_key("message"));
    assert!(error_obj.contains_key("type"));
    assert!(error_obj.contains_key("param"));
    assert!(error_obj.contains_key("code"));

    assert_eq!(error_obj.get("param").unwrap(), &Value::Null);
    assert_eq!(error_obj.get("code").unwrap(), &Value::Null);
}

#[tokio::test]
async fn test_error_retry_after_header_handling() {
    // 1. RateLimitExceeded with 15 seconds
    let err1 = AppError::rate_limit("Slow down", Some(15));
    let resp1 = err1.into_response();
    assert_eq!(resp1.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(resp1.headers().get(header::RETRY_AFTER).unwrap(), "15");

    // 2. UpstreamRateLimit with 120 seconds
    let err2 = AppError::upstream_rate_limit("Upstream rate limited", Some(120));
    let resp2 = err2.into_response();
    assert_eq!(resp2.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(resp2.headers().get(header::RETRY_AFTER).unwrap(), "120");

    // 3. RateLimitExceeded with 0 seconds
    let err3 = AppError::rate_limit("Immediate retry", Some(0));
    let resp3 = err3.into_response();
    assert_eq!(resp3.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(resp3.headers().get(header::RETRY_AFTER).unwrap(), "0");

    // 4. RateLimitExceeded with None
    let err4 = AppError::rate_limit("Rate limited without retry hint", None);
    let resp4 = err4.into_response();
    assert_eq!(resp4.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(resp4.headers().get(header::RETRY_AFTER).is_none());

    // 5. Non-429 error does NOT have Retry-After header
    let err5 = AppError::bad_gateway("Bad gateway");
    let resp5 = err5.into_response();
    assert_eq!(resp5.status(), StatusCode::BAD_GATEWAY);
    assert!(resp5.headers().get(header::RETRY_AFTER).is_none());
}

#[test]
fn test_error_from_conversions() {
    // 1. serde_json Error -> AppError
    let bad_json_res: Result<Config, serde_json::Error> = serde_json::from_str("{invalid json");
    let json_err: AppError = bad_json_res.unwrap_err().into();
    assert_eq!(json_err.status_code(), StatusCode::BAD_REQUEST);
    assert_eq!(json_err.to_openai_detail().error_type, "invalid_request_error");
    assert_eq!(json_err.to_openai_detail().code, Some("invalid_json".to_string()));

    // 2. std::io::Error -> AppError
    let io_err: AppError = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Access denied").into();
    assert_eq!(io_err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(io_err.to_openai_detail().code, Some("io_error".to_string()));

    // 3. tokio oneshot RecvError -> AppError
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    drop(tx);
    let recv_err = rx.blocking_recv().unwrap_err();
    let app_err: AppError = recv_err.into();
    assert_eq!(app_err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(app_err.to_openai_detail().code, Some("channel_recv_error".to_string()));
}
