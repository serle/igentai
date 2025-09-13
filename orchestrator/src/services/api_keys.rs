//! Production API key management implementation
//!
//! This module provides environment variable-based API key management for various
//! AI providers. The orchestrator requires at least one API key to be present
//! and will pass all available keys to producer processes.
//!
//! ## Configuration Sources
//! API keys are loaded from:
//! 1. `.env` file in the current directory or parent directories (if present)
//! 2. System environment variables
//! 
//! Environment variables take precedence over .env file values.
//!
//! ## Required Keys
//! - `OPENAI_API_KEY`: OpenAI API access key (required)
//!
//! ## Optional Keys
//! The following provider keys are optional and will be included if available:
//! - `ANTHROPIC_API_KEY`: Anthropic Claude API key
//! - `GOOGLE_API_KEY`, `GOOGLE_AI_API_KEY`: Google Gemini API keys
//! - `COHERE_API_KEY`: Cohere API key
//! - `HUGGINGFACE_API_KEY`, `HF_TOKEN`: Hugging Face API keys
//! - `MISTRAL_API_KEY`: Mistral AI API key
//! - `GROQ_API_KEY`: Groq API key

use crate::traits::{ApiKeySource, KeyValuePair, RequiredKeyMissing};

/// Real API key source using environment variables
pub struct RealApiKeySource;

impl RealApiKeySource {
    /// List of required API keys that must be present
    const REQUIRED_KEYS: &'static [&'static str] = &["OPENAI_API_KEY"];
    
    /// List of optional API keys for various AI providers
    const OPTIONAL_KEYS: &'static [&'static str] = &[
        // Anthropic (Claude)
        "ANTHROPIC_API_KEY",
        // Google (Gemini)
        "GOOGLE_API_KEY",
        "GOOGLE_AI_API_KEY", 
        // Cohere
        "COHERE_API_KEY",
        // Hugging Face
        "HUGGINGFACE_API_KEY",
        "HF_TOKEN",
        // Mistral AI
        "MISTRAL_API_KEY",
        // Groq
        "GROQ_API_KEY",
    ];

    /// Initialize environment by loading .env file if present
    /// 
    /// This is called once to load environment variables from .env file.
    /// It's safe to call multiple times as dotenv ignores already set variables.
    fn init_env() {
        // Try to load .env file from current directory or parent directories
        // This will silently fail if no .env file is found, which is fine
        let _ = dotenv::dotenv();
    }
}

#[async_trait::async_trait]
impl ApiKeySource for RealApiKeySource {
    async fn get_api_keys(&self) -> Result<Vec<KeyValuePair>, RequiredKeyMissing> {
        // Initialize environment from .env file if present
        Self::init_env();
        
        let mut available_keys = Vec::new();
        let mut missing_required = Vec::new();
        
        // Check required keys
        for &key_name in Self::REQUIRED_KEYS {
            match std::env::var(key_name) {
                Ok(value) => {
                    available_keys.push(KeyValuePair {
                        key: key_name.to_string(),
                        value,
                    });
                }
                Err(_) => {
                    missing_required.push(key_name);
                }
            }
        }
        
        // Check optional keys
        for &key_name in Self::OPTIONAL_KEYS {
            if let Ok(value) = std::env::var(key_name) {
                available_keys.push(KeyValuePair {
                    key: key_name.to_string(),
                    value,
                });
            }
        }
        
        // Return error if any required keys are missing
        if !missing_required.is_empty() {
            return Err(RequiredKeyMissing {
                key_name: missing_required.join(", "),
                message: format!(
                    "Missing required API keys: {}. These keys must be set as environment variables.",
                    missing_required.join(", ")
                ),
            });
        }
        
        // Log successful validation
        println!("API keys validated successfully:");
        println!("  Required keys: {}", Self::REQUIRED_KEYS.join(", "));
        let optional_available: Vec<&str> = available_keys
            .iter()
            .filter(|kv| Self::OPTIONAL_KEYS.contains(&kv.key.as_str()))
            .map(|kv| kv.key.as_str())
            .collect();
        if !optional_available.is_empty() {
            println!("  Optional keys available: {}", optional_available.join(", "));
        }
        
        Ok(available_keys)
    }
}