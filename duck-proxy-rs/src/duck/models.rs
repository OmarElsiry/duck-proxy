//! Duck.ai model definitions.

/// Known Duck.ai upstream model identifiers.
pub struct DuckModel {
    pub id: &'static str,
    pub name: &'static str,
    pub owned_by: &'static str,
}

pub const DUCK_MODELS: &[DuckModel] = &[
    DuckModel { id: "gpt-5.6-luna", name: "GPT-5.6 Luna", owned_by: "openai" },
    DuckModel { id: "gpt-5.4-mini", name: "GPT-5.4 Mini", owned_by: "openai" },
    DuckModel { id: "tinfoil/gemma4-31b", name: "Gemma 4 31B", owned_by: "google" },
    DuckModel { id: "claude-haiku-4-5", name: "Claude Haiku 4.5", owned_by: "anthropic" },
    DuckModel { id: "mistral-small-2603", name: "Mistral Small", owned_by: "mistral" },
    DuckModel { id: "image-generation", name: "Image Generation", owned_by: "duck.ai" },
];

/// The default model used for image generation requests.
pub const IMAGE_GEN_CHAT_MODEL: &str = "gpt-5.6-luna";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duck_models_not_empty() {
        assert!(!DUCK_MODELS.is_empty());
    }

    #[test]
    fn test_image_gen_model_exists() {
        assert!(DUCK_MODELS.iter().any(|m| m.id == IMAGE_GEN_CHAT_MODEL));
    }
}
