//! Real API key management service
//!
//! Manages API keys for different LLM providers from environment variables
//! with support for .env files and validation.

use crate::error::{OrchestratorError, OrchestratorResult};
use crate::traits::ApiKeySource;
use async_trait::async_trait;
use shared::{process_debug, ProviderId};
use std::collections::HashMap;

/// Real API key source using environment variables
pub struct RealApiKeySource {
    /// Whether to use random provider only (disables env var loading)
    random_only: bool,
}

impl RealApiKeySource {
    /// Create new API key source
    pub fn new() -> Self {
        Self { random_only: false }
    }

    /// Create new API key source that only provides Random provider
    pub fn random_only() -> Self {
        Self { random_only: true }
    }

    /// Load API keys from environment
    fn load_keys_from_env() -> HashMap<ProviderId, String> {
        let mut keys = HashMap::new();

        // OpenAI
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            if !key.trim().is_empty() {
                keys.insert(ProviderId::OpenAI, key.trim().to_string());
            }
        }

        // Anthropic
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            if !key.trim().is_empty() {
                keys.insert(ProviderId::Anthropic, key.trim().to_string());
            }
        }

        // Google/Gemini
        for env_var in &["GOOGLE_API_KEY", "GEMINI_API_KEY"] {
            if let Ok(key) = std::env::var(env_var) {
                if !key.trim().is_empty() {
                    keys.insert(ProviderId::Gemini, key.trim().to_string());
                    break;
                }
            }
        }

        // Random provider (optional - can be set to "dummy" for consistency)
        if let Ok(key) = std::env::var("RANDOM_API_KEY") {
            if !key.trim().is_empty() {
                keys.insert(ProviderId::Random, key.trim().to_string());
            }
        }

        keys
    }

    /// Load .env file if present
    fn load_dotenv() {
        // Try to load .env file, but don't fail if it doesn't exist
        // First try current directory, then parent directory (like producer does)
        if let Ok(_) = dotenv::dotenv() {
            tracing::debug!("📄 Loaded .env file from current directory");
        } else if let Ok(_) = dotenv::from_path("../.env") {
            tracing::debug!("📄 Loaded .env file from parent directory");
        } else {
            tracing::debug!("📄 No .env file found - using environment variables");
        }
    }

    /// Validate that we have at least one API key
    fn validate_keys(keys: &HashMap<ProviderId, String>) -> OrchestratorResult<()> {
        if keys.is_empty() {
            return Err(OrchestratorError::config(
                "No API keys found. Please set at least one of: OPENAI_API_KEY, ANTHROPIC_API_KEY, GOOGLE_API_KEY, or RANDOM_API_KEY=dummy for testing"
            ));
        }

        // Validate key format (basic check) - skip Random provider
        for (provider, key) in keys {
            // Skip validation for Random provider since it's only used for testing
            if *provider == ProviderId::Random {
                continue;
            }

            if key.len() < 10 {
                return Err(OrchestratorError::config(format!(
                    "API key for {provider:?} appears to be invalid (too short)"
                )));
            }

            // Provider-specific validations
            match provider {
                ProviderId::OpenAI => {
                    if !key.starts_with("sk-") {
                        return Err(OrchestratorError::config("OpenAI API key should start with 'sk-'"));
                    }
                }
                ProviderId::Anthropic => {
                    if !key.starts_with("sk-ant-") {
                        return Err(OrchestratorError::config(
                            "Anthropic API key should start with 'sk-ant-'",
                        ));
                    }
                }
                ProviderId::Gemini => {
                    // Google API keys typically start with "AIza" but can vary
                    if key.len() < 20 {
                        return Err(OrchestratorError::config(
                            "Google/Gemini API key appears to be too short",
                        ));
                    }
                }
                ProviderId::Random => {
                    // Should never reach here due to continue above
                    unreachable!("Random provider validation should be skipped");
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl ApiKeySource for RealApiKeySource {
    async fn get_api_keys(&self) -> OrchestratorResult<HashMap<ProviderId, String>> {
        // If random-only, return just the Random provider key
        if self.random_only {
            let mut keys = HashMap::new();
            keys.insert(ProviderId::Random, "dummy-test-key".to_string());
            process_debug!(shared::ProcessId::current(), "🎲 Using Random provider only");
            return Ok(keys);
        }

        // Load .env file first
        Self::load_dotenv();

        // Load keys from environment
        let keys = Self::load_keys_from_env();

        // Validate keys
        Self::validate_keys(&keys)?;

        process_debug!(
            shared::ProcessId::current(),
            "🔑 Loaded API keys for providers: {:?}",
            keys.keys().collect::<Vec<_>>()
        );

        Ok(keys)
    }

    async fn has_api_key(&self, provider_id: ProviderId) -> bool {
        match self.get_api_keys().await {
            Ok(keys) => keys.contains_key(&provider_id),
            Err(_) => false,
        }
    }

    async fn get_provider_key(&self, provider_id: ProviderId) -> OrchestratorResult<Option<String>> {
        let keys = self.get_api_keys().await?;
        Ok(keys.get(&provider_id).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_key_source_no_keys() {
        // Test non-test mode without any environment keys set
        let source = RealApiKeySource::new();
        let result = source.get_api_keys().await;

        // Should fail with no keys (unless environment has keys)
        // Note: This test might pass if environment has real API keys
        if result.is_err() {
            assert!(result.unwrap_err().to_string().contains("No API keys found"));
        }
    }

    #[tokio::test]
    async fn test_api_key_source_random_only() {
        // Random-only should return Random provider with dummy key
        let source = RealApiKeySource::random_only();
        let result = source.get_api_keys().await;

        assert!(result.is_ok());
        let keys = result.unwrap();
        assert_eq!(keys.len(), 1);
        assert!(keys.contains_key(&ProviderId::Random));
        assert_eq!(keys.get(&ProviderId::Random).unwrap(), "dummy-test-key");
    }

    #[tokio::test]
    async fn test_has_api_key_random_only() {
        let source = RealApiKeySource::random_only();

        assert!(source.has_api_key(ProviderId::Random).await);
        assert!(!source.has_api_key(ProviderId::OpenAI).await);
    }

    #[tokio::test]
    async fn test_get_provider_key_random_only() {
        let source = RealApiKeySource::random_only();

        let key = source.get_provider_key(ProviderId::Random).await.unwrap();
        assert_eq!(key, Some("dummy-test-key".to_string()));

        let no_key = source.get_provider_key(ProviderId::OpenAI).await.unwrap();
        assert_eq!(no_key, None);
    }

    #[tokio::test]
    async fn test_env_vs_random_only() {
        // Random-only should always return Random provider
        let random_source = RealApiKeySource::random_only();
        let random_keys = random_source.get_api_keys().await.unwrap();
        assert_eq!(random_keys.len(), 1);
        assert!(random_keys.contains_key(&ProviderId::Random));

        // Production mode may have different results based on environment
        let env_source = RealApiKeySource::new();
        let prod_result = env_source.get_api_keys().await;
        // Don't assert on prod_result since it depends on environment
        // Just verify it doesn't panic
        let _ = prod_result;
    }
}
