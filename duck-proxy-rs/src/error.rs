use axum::{
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Top-level OpenAI error response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenAiErrorResponse {
    pub error: OpenAiErrorDetail,
}

/// Detailed error payload matching OpenAI's JSON error schema.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenAiErrorDetail {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub param: Option<String>,
    pub code: Option<String>,
}

/// Backward compatibility and convenience type aliases
pub type ErrorResponse = OpenAiErrorResponse;
pub type ErrorBody = OpenAiErrorDetail;
pub type ErrorDetail = OpenAiErrorDetail;

/// Application-wide error enum for duck-proxy.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Invalid request: {message}")]
    InvalidRequest {
        message: String,
        param: Option<String>,
        code: Option<String>,
    },

    #[error("The model '{model}' does not exist or is not supported.")]
    ModelNotFound { model: String },

    #[error("Not found: {message}")]
    NotFound { message: String },

    #[error("Rate limit exceeded: {message}")]
    RateLimitExceeded {
        message: String,
        retry_after_secs: Option<u64>,
    },

    #[error("Upstream rate limit: {message}")]
    UpstreamRateLimit {
        message: String,
        retry_after_secs: Option<u64>,
    },

    #[error("Bad gateway: {message}")]
    BadGateway {
        message: String,
        code: Option<String>,
    },

    #[error("Gateway timeout: {message}")]
    GatewayTimeout { message: String },

    #[error("Anti-bot challenge solver error: {message}")]
    ChallengeError { message: String },

    #[error("Cryptographic operation failed: {message}")]
    CryptoError { message: String },

    #[error("JSON error: {message}")]
    JsonError { message: String, status: StatusCode },

    #[error("Internal server error: {message}")]
    Internal {
        message: String,
        code: Option<String>,
    },
}

impl AppError {
    // -------------------------------------------------------------------------
    // Helper Constructors
    // -------------------------------------------------------------------------

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::InvalidRequest {
            message: message.into(),
            param: None,
            code: None,
        }
    }

    pub fn bad_request_with_param(
        message: impl Into<String>,
        param: impl Into<String>,
        code: impl Into<String>,
    ) -> Self {
        Self::InvalidRequest {
            message: message.into(),
            param: Some(param.into()),
            code: Some(code.into()),
        }
    }

    pub fn missing_param(param_name: impl Into<String>) -> Self {
        let param = param_name.into();
        Self::InvalidRequest {
            message: format!("Missing required parameter: '{}'.", param),
            param: Some(param),
            code: Some("missing_required_parameter".to_string()),
        }
    }

    pub fn model_not_found(model: impl Into<String>) -> Self {
        Self::ModelNotFound {
            model: model.into(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound {
            message: message.into(),
        }
    }

    pub fn rate_limit(message: impl Into<String>, retry_after_secs: Option<u64>) -> Self {
        Self::RateLimitExceeded {
            message: message.into(),
            retry_after_secs,
        }
    }

    pub fn upstream_rate_limit(message: impl Into<String>, retry_after_secs: Option<u64>) -> Self {
        Self::UpstreamRateLimit {
            message: message.into(),
            retry_after_secs,
        }
    }

    pub fn bad_gateway(message: impl Into<String>) -> Self {
        Self::BadGateway {
            message: message.into(),
            code: Some("bad_gateway".to_string()),
        }
    }

    pub fn gateway_timeout(message: impl Into<String>) -> Self {
        Self::GatewayTimeout {
            message: message.into(),
        }
    }

    pub fn challenge_error(message: impl Into<String>) -> Self {
        Self::ChallengeError {
            message: message.into(),
        }
    }

    pub fn crypto_error(message: impl Into<String>) -> Self {
        Self::CryptoError {
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
            code: Some("internal_error".to_string()),
        }
    }

    // -------------------------------------------------------------------------
    // Status Code & Metadata Extraction
    // -------------------------------------------------------------------------

    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidRequest { .. } => StatusCode::BAD_REQUEST,
            Self::ModelNotFound { .. } => StatusCode::NOT_FOUND,
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::RateLimitExceeded { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::UpstreamRateLimit { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::BadGateway { .. } => StatusCode::BAD_GATEWAY,
            Self::GatewayTimeout { .. } => StatusCode::GATEWAY_TIMEOUT,
            Self::ChallengeError { .. } => StatusCode::BAD_GATEWAY,
            Self::CryptoError { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::JsonError { status, .. } => *status,
            Self::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn retry_after(&self) -> Option<u64> {
        match self {
            Self::RateLimitExceeded { retry_after_secs, .. } => *retry_after_secs,
            Self::UpstreamRateLimit { retry_after_secs, .. } => *retry_after_secs,
            _ => None,
        }
    }

    pub fn to_openai_detail(&self) -> OpenAiErrorDetail {
        match self {
            Self::InvalidRequest { message, param, code } => OpenAiErrorDetail {
                message: message.clone(),
                error_type: "invalid_request_error".to_string(),
                param: param.clone(),
                code: code.clone(),
            },
            Self::ModelNotFound { model } => OpenAiErrorDetail {
                message: format!("The model '{}' does not exist or is not supported.", model),
                error_type: "invalid_request_error".to_string(),
                param: Some("model".to_string()),
                code: Some("model_not_found".to_string()),
            },
            Self::NotFound { message } => OpenAiErrorDetail {
                message: message.clone(),
                error_type: "invalid_request_error".to_string(),
                param: None,
                code: None,
            },
            Self::RateLimitExceeded { message, .. } => OpenAiErrorDetail {
                message: message.clone(),
                error_type: "rate_limit_error".to_string(),
                param: None,
                code: Some("rate_limit_exceeded".to_string()),
            },
            Self::UpstreamRateLimit { message, .. } => OpenAiErrorDetail {
                message: message.clone(),
                error_type: "rate_limit_error".to_string(),
                param: None,
                code: Some("rate_limit_exceeded".to_string()),
            },
            Self::BadGateway { message, code } => OpenAiErrorDetail {
                message: message.clone(),
                error_type: "api_error".to_string(),
                param: None,
                code: code.clone().or_else(|| Some("bad_gateway".to_string())),
            },
            Self::GatewayTimeout { message } => OpenAiErrorDetail {
                message: message.clone(),
                error_type: "api_error".to_string(),
                param: None,
                code: Some("gateway_timeout".to_string()),
            },
            Self::ChallengeError { message } => OpenAiErrorDetail {
                message: format!("Anti-bot challenge solver failed: {}", message),
                error_type: "api_error".to_string(),
                param: None,
                code: Some("challenge_solver_failed".to_string()),
            },
            Self::CryptoError { message } => OpenAiErrorDetail {
                message: format!("Cryptographic operation error: {}", message),
                error_type: "api_error".to_string(),
                param: None,
                code: Some("crypto_error".to_string()),
            },
            Self::JsonError { message, .. } => OpenAiErrorDetail {
                message: message.clone(),
                error_type: "invalid_request_error".to_string(),
                param: None,
                code: Some("invalid_json".to_string()),
            },
            Self::Internal { message, code } => OpenAiErrorDetail {
                message: message.clone(),
                error_type: "api_error".to_string(),
                param: None,
                code: code.clone().or_else(|| Some("internal_error".to_string())),
            },
        }
    }
}

// -----------------------------------------------------------------------------
// Axum IntoResponse Implementation
// -----------------------------------------------------------------------------

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let retry_after = self.retry_after();
        let detail = self.to_openai_detail();
        let payload = OpenAiErrorResponse { error: detail };

        let mut response = (status, Json(payload)).into_response();

        if let Some(secs) = retry_after {
            if let Ok(header_val) = HeaderValue::from_str(&secs.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, header_val);
            }
        }

        response
    }
}

// -----------------------------------------------------------------------------
// Standard From Conversions
// -----------------------------------------------------------------------------

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::GatewayTimeout {
                message: format!("Upstream request timed out: {}", err),
            }
        } else if err.is_connect() {
            Self::BadGateway {
                message: format!("Failed to establish connection to upstream Duck.ai: {}", err),
                code: Some("upstream_connect_failed".to_string()),
            }
        } else {
            Self::BadGateway {
                message: format!("Upstream network error: {}", err),
                code: Some("upstream_network_error".to_string()),
            }
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        Self::JsonError {
            message: format!("JSON parsing or serialization error: {}", err),
            status: StatusCode::BAD_REQUEST,
        }
    }
}

impl From<rsa::Error> for AppError {
    fn from(err: rsa::Error) -> Self {
        Self::CryptoError {
            message: format!("RSA error: {}", err),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::Internal {
            message: format!("I/O error: {}", err),
            code: Some("io_error".to_string()),
        }
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for AppError {
    fn from(_: tokio::sync::oneshot::error::RecvError) -> Self {
        Self::Internal {
            message: "Worker actor communication channel was dropped unexpectedly.".to_string(),
            code: Some("channel_recv_error".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    #[tokio::test]
    async fn test_bad_request_error_into_response() {
        let err = AppError::bad_request("Missing 'messages' in payload");
        assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);

        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: OpenAiErrorResponse = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body.error.message, "Missing 'messages' in payload");
        assert_eq!(body.error.error_type, "invalid_request_error");
        assert_eq!(body.error.param, None);
        assert_eq!(body.error.code, None);
    }

    #[tokio::test]
    async fn test_model_not_found_into_response() {
        let err = AppError::model_not_found("gpt-nonexistent");
        assert_eq!(err.status_code(), StatusCode::NOT_FOUND);

        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: OpenAiErrorResponse = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(
            body.error.message,
            "The model 'gpt-nonexistent' does not exist or is not supported."
        );
        assert_eq!(body.error.error_type, "invalid_request_error");
        assert_eq!(body.error.param, Some("model".to_string()));
        assert_eq!(body.error.code, Some("model_not_found".to_string()));
    }

    #[tokio::test]
    async fn test_rate_limit_with_retry_after_header() {
        let err = AppError::rate_limit("Rate limit exceeded, please back off", Some(15));
        assert_eq!(err.status_code(), StatusCode::TOO_MANY_REQUESTS);

        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response.headers().get(header::RETRY_AFTER).unwrap(),
            "15"
        );

        let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: OpenAiErrorResponse = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body.error.error_type, "rate_limit_error");
        assert_eq!(body.error.code, Some("rate_limit_exceeded".to_string()));
    }

    #[tokio::test]
    async fn test_bad_gateway_into_response() {
        let err = AppError::bad_gateway("Upstream Duck.ai service unavailable");
        assert_eq!(err.status_code(), StatusCode::BAD_GATEWAY);

        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

        let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body: OpenAiErrorResponse = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(body.error.message, "Upstream Duck.ai service unavailable");
        assert_eq!(body.error.error_type, "api_error");
        assert_eq!(body.error.code, Some("bad_gateway".to_string()));
    }

    #[tokio::test]
    async fn test_json_serialization_null_fields() {
        let err_detail = OpenAiErrorDetail {
            message: "Test error".to_string(),
            error_type: "api_error".to_string(),
            param: None,
            code: None,
        };
        let envelope = OpenAiErrorResponse { error: err_detail };

        let json_str = serde_json::to_string(&envelope).unwrap();
        let json_val: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(json_val["error"]["param"], serde_json::Value::Null);
        assert_eq!(json_val["error"]["code"], serde_json::Value::Null);
        assert_eq!(json_val["error"]["type"], "api_error");
        assert_eq!(json_val["error"]["message"], "Test error");
    }
}
