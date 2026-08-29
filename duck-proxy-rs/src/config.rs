use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Error types for configuration loading and validation.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read configuration file at '{path}': {source}")]
    IoError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to parse YAML configuration: {0}")]
    YamlError(#[from] serde_yaml::Error),
}

/// Server network binding configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

/// Mapping between OpenAI client model name and Duck.ai upstream model identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelMapping {
    pub model_name: String,
    pub duck_model: String,
}

impl ModelMapping {
    pub fn new(model_name: impl Into<String>, duck_model: impl Into<String>) -> Self {
        Self {
            model_name: model_name.into(),
            duck_model: duck_model.into(),
        }
    }
}

/// Root proxy configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default = "default_model_list")]
    pub model_list: Vec<ModelMapping>,
    #[serde(default = "default_upstream_base_url")]
    pub upstream_base_url: String,
}

fn default_upstream_base_url() -> String {
    "https://duck.ai".to_string()
}

/// Returns the standard default model catalog matching ORIGINAL_REQUEST §6.
pub fn default_model_list() -> Vec<ModelMapping> {
    vec![
        ModelMapping::new("gpt-5.6-luna", "gpt-5.6-luna"),
        ModelMapping::new("gpt-5.4-mini", "gpt-5.4-mini"),
        ModelMapping::new("claude-haiku-4-5", "claude-haiku-4-5"),
        ModelMapping::new("mistral-small-2603", "mistral-small-2603"),
        ModelMapping::new("tinfoil/gemma4-31b", "tinfoil/gemma4-31b"),
        ModelMapping::new("gemma4-31b", "tinfoil/gemma4-31b"),
        ModelMapping::new("image-generation", "image-generation"),
        ModelMapping::new("gpt5", "gpt-5.6-luna"),
        ModelMapping::new("gpt5_mini", "gpt-5.4-mini"),
        ModelMapping::new("gemma", "tinfoil/gemma4-31b"),
        ModelMapping::new("claude", "claude-haiku-4-5"),
        ModelMapping::new("mistral", "mistral-small-2603"),
        ModelMapping::new("image", "image-generation"),
    ]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            model_list: default_model_list(),
            upstream_base_url: default_upstream_base_url(),
        }
    }
}

impl Config {
    /// Load configuration from a YAML string.
    pub fn from_str(yaml_str: &str) -> Result<Self, ConfigError> {
        let config: Config = serde_yaml::from_str(yaml_str)?;
        Ok(config)
    }

    /// Load configuration from a specific file path.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let path_ref = path.as_ref();
        let content = std::fs::read_to_string(path_ref).map_err(|e| ConfigError::IoError {
            path: path_ref.to_path_buf(),
            source: e,
        })?;
        Self::from_str(&content)
    }

    /// Load configuration from the provided path or fallback to `./config.yaml` or default config.
    pub fn load_or_default<P: AsRef<Path>>(path: Option<P>) -> Self {
        if let Some(p) = path {
            let p_ref = p.as_ref();
            match Self::from_file(p_ref) {
                Ok(cfg) => {
                    tracing::info!("Loaded configuration from custom path: {}", p_ref.display());
                    return cfg;
                }
                Err(err) => {
                    tracing::warn!(
                        "Failed to load configuration from {}: {}. Falling back to defaults.",
                        p_ref.display(),
                        err
                    );
                }
            }
        }

        let default_path = Path::new("config.yaml");
        if default_path.exists() {
            match Self::from_file(default_path) {
                Ok(cfg) => {
                    tracing::info!("Loaded configuration from ./config.yaml");
                    return cfg;
                }
                Err(err) => {
                    tracing::warn!(
                        "Failed to parse ./config.yaml: {}. Falling back to in-memory defaults.",
                        err
                    );
                }
            }
        }

        tracing::info!("Using standard in-memory default configuration.");
        Self::default()
    }

    /// Resolve an incoming requested model name or alias to a `ModelMapping`.
    ///
    /// Handles:
    /// 1. Optional `duck/` prefix stripping (e.g. `duck/gpt5` -> `gpt5`).
    /// 2. Exact match against `model_name`.
    /// 3. Exact match against `duck_model`.
    /// 4. Case-insensitive match against `model_name`.
    /// 5. Case-insensitive match against `duck_model`.
    pub fn resolve_model(&self, requested: &str) -> Option<&ModelMapping> {
        let normalized = requested.strip_prefix("duck/").unwrap_or(requested).trim();

        // 1. Exact match on model_name
        if let Some(m) = self.model_list.iter().find(|m| m.model_name == normalized) {
            return Some(m);
        }

        // 2. Exact match on duck_model
        if let Some(m) = self.model_list.iter().find(|m| m.duck_model == normalized) {
            return Some(m);
        }

        // 3. Case-insensitive match on model_name
        if let Some(m) = self
            .model_list
            .iter()
            .find(|m| m.model_name.eq_ignore_ascii_case(normalized))
        {
            return Some(m);
        }

        // 4. Case-insensitive match on duck_model
        if let Some(m) = self
            .model_list
            .iter()
            .find(|m| m.duck_model.eq_ignore_ascii_case(normalized))
        {
            return Some(m);
        }

        None
    }

    /// Helper returning only the upstream Duck.ai model identifier string.
    pub fn resolve_duck_model(&self, requested: &str) -> Option<&str> {
        self.resolve_model(requested).map(|m| m.duck_model.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert_eq!(cfg.server.host, "0.0.0.0");
        assert_eq!(cfg.server.port, 8080);
        assert_eq!(cfg.upstream_base_url, "https://duck.ai");
        assert_eq!(cfg.model_list.len(), 13);

        assert_eq!(
            cfg.resolve_duck_model("gpt-5.6-luna"),
            Some("gpt-5.6-luna")
        );
        assert_eq!(cfg.resolve_duck_model("gpt5"), Some("gpt-5.6-luna"));
        assert_eq!(cfg.resolve_duck_model("gpt5_mini"), Some("gpt-5.4-mini"));
        assert_eq!(cfg.resolve_duck_model("claude"), Some("claude-haiku-4-5"));
        assert_eq!(
            cfg.resolve_duck_model("mistral"),
            Some("mistral-small-2603")
        );
        assert_eq!(
            cfg.resolve_duck_model("gemma"),
            Some("tinfoil/gemma4-31b")
        );
        assert_eq!(cfg.resolve_duck_model("image"), Some("image-generation"));
    }

    #[test]
    fn test_yaml_deserialization_full() {
        let yaml = r#"
server:
  host: "127.0.0.1"
  port: 9090
upstream_base_url: "http://127.0.0.1:8888"
model_list:
  - model_name: custom-model
    duck_model: upstream-model-x
"#;
        let cfg = Config::from_str(yaml).expect("Failed to parse YAML");
        assert_eq!(cfg.server.host, "127.0.0.1");
        assert_eq!(cfg.server.port, 9090);
        assert_eq!(cfg.upstream_base_url, "http://127.0.0.1:8888");
        assert_eq!(cfg.model_list.len(), 1);
        assert_eq!(
            cfg.resolve_duck_model("custom-model"),
            Some("upstream-model-x")
        );
    }

    #[test]
    fn test_yaml_deserialization_partial() {
        let yaml = r#"
server:
  port: 3000
"#;
        let cfg = Config::from_str(yaml).expect("Failed to parse partial YAML");
        assert_eq!(cfg.server.host, "0.0.0.0");
        assert_eq!(cfg.server.port, 3000);
        assert_eq!(cfg.upstream_base_url, "https://duck.ai");
        assert_eq!(cfg.model_list.len(), 13);
    }

    #[test]
    fn test_resolve_model_prefix_and_case() {
        let cfg = Config::default();

        // Prefix stripping
        assert_eq!(cfg.resolve_duck_model("duck/gpt5"), Some("gpt-5.6-luna"));
        assert_eq!(
            cfg.resolve_duck_model("duck/claude"),
            Some("claude-haiku-4-5")
        );

        // Case insensitivity
        assert_eq!(cfg.resolve_duck_model("GPT5"), Some("gpt-5.6-luna"));
        assert_eq!(cfg.resolve_duck_model("Claude"), Some("claude-haiku-4-5"));
        assert_eq!(
            cfg.resolve_duck_model("MISTRAL"),
            Some("mistral-small-2603")
        );

        // Resolving by direct upstream model name
        assert_eq!(
            cfg.resolve_duck_model("tinfoil/gemma4-31b"),
            Some("tinfoil/gemma4-31b")
        );

        // Unknown model
        assert_eq!(cfg.resolve_duck_model("nonexistent-model"), None);
    }

    #[test]
    fn test_from_file_and_fallback() {
        let yaml_content = r#"
server:
  host: "0.0.0.0"
  port: 8081
"#;
        let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
        temp_file
            .write_all(yaml_content.as_bytes())
            .expect("Failed to write to temp file");

        let cfg = Config::from_file(temp_file.path()).expect("Failed to load from temp file");
        assert_eq!(cfg.server.port, 8081);

        // Non-existent file error
        let missing_path = Path::new("non_existent_file_path_12345.yaml");
        let err = Config::from_file(missing_path);
        assert!(err.is_err());
        match err.unwrap_err() {
            ConfigError::IoError { path, .. } => {
                assert_eq!(path, missing_path);
            }
            _ => panic!("Expected IoError"),
        }

        // load_or_default with missing path falls back to default
        let fallback_cfg = Config::load_or_default(Some(missing_path));
        assert_eq!(fallback_cfg.server.port, 8080);
    }

    #[test]
    fn test_invalid_yaml_error() {
        let invalid_yaml = "server:\n  host: [invalid, yaml, structure";
        let err = Config::from_str(invalid_yaml);
        assert!(err.is_err());
        match err.unwrap_err() {
            ConfigError::YamlError(_) => (),
            _ => panic!("Expected YamlError"),
        }
    }
}
