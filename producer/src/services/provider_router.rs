//! Provider routing implementation with API key management and provider requests

use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use rand::Rng;
use tokio::sync::RwLock;

use shared::{ApiFailure, ProviderId, RoutingStrategy};
use crate::error::ProducerResult;
use crate::traits::ProviderRouter;
use crate::types::{ProviderStats, ProviderResponse};

/// Real provider router with API key management and provider requests
pub struct RealProviderRouter {
    stats: Arc<RwLock<HashMap<ProviderId, ProviderStats>>>,
    api_keys: Arc<RwLock<HashMap<ProviderId, String>>>,
    prompt: Arc<RwLock<String>>,
    request_configs: Arc<RwLock<HashMap<ProviderId, shared::RequestConfig>>>,
    routing_strategy: Arc<RwLock<RoutingStrategy>>,
    round_robin_index: Arc<RwLock<usize>>,
}

impl RealProviderRouter {
    /// Create new provider router
    pub fn new() -> Self {
        // Initialize default request configs for each provider
        let mut request_configs = HashMap::new();
        request_configs.insert(ProviderId::OpenAI, shared::RequestConfig::default());
        request_configs.insert(ProviderId::Anthropic, shared::RequestConfig::default());
        request_configs.insert(ProviderId::Gemini, shared::RequestConfig::default());
        
        // Default routing strategy with weighted selection
        let mut default_weights = HashMap::new();
        default_weights.insert(ProviderId::OpenAI, 1.0);
        default_weights.insert(ProviderId::Anthropic, 0.5);
        default_weights.insert(ProviderId::Gemini, 0.3);
        
        Self {
            stats: Arc::new(RwLock::new(HashMap::new())),
            api_keys: Arc::new(RwLock::new(HashMap::new())),
            prompt: Arc::new(RwLock::new(String::new())),
            request_configs: Arc::new(RwLock::new(request_configs)),
            routing_strategy: Arc::new(RwLock::new(RoutingStrategy::Weighted { 
                weights: default_weights 
            })),
            round_robin_index: Arc::new(RwLock::new(0)),
        }
    }

    /// Set the routing strategy
    pub async fn set_routing_strategy(&self, strategy: RoutingStrategy) -> ProducerResult<()> {
        let mut current_strategy = self.routing_strategy.write().await;
        *current_strategy = strategy;
        Ok(())
    }
}

#[async_trait]
impl ProviderRouter for RealProviderRouter {
    async fn set_api_keys(&self, api_keys: HashMap<ProviderId, String>) -> ProducerResult<()> {
        let mut keys = self.api_keys.write().await;
        *keys = api_keys;
        println!("Set {} API keys for providers", keys.len());
        Ok(())
    }
    
    async fn select_provider(&self) -> ProducerResult<ProviderId> {
        let strategy = self.routing_strategy.read().await;

        match &*strategy {
            RoutingStrategy::Weighted { weights } => {
                if weights.is_empty() {
                    return Err(crate::error::ProducerError::ConfigError {
                        message: "No providers configured in weighted strategy".to_string(),
                    });
                }

                // Weighted random selection
                let total_weight: f32 = weights.values().sum();
                let mut rng = rand::thread_rng();
                let mut random_value = rng.r#gen::<f32>() * total_weight;

                for (provider, weight) in weights.iter() {
                    random_value -= weight;
                    if random_value <= 0.0 {
                        return Ok(provider.clone());
                    }
                }
                // Fallback to first provider
                Ok(weights.keys().next().unwrap().clone())
            },
            RoutingStrategy::PriorityOrder { providers } => {
                if providers.is_empty() {
                    return Err(crate::error::ProducerError::ConfigError {
                        message: "No providers configured in priority order strategy".to_string(),
                    });
                }
                // Return first (highest priority) provider
                Ok(providers[0].clone())
            },
            RoutingStrategy::RoundRobin { providers } => {
                if providers.is_empty() {
                    return Err(crate::error::ProducerError::ConfigError {
                        message: "No providers configured in round-robin strategy".to_string(),
                    });
                }
                
                // Round-robin selection through ordered providers
                let mut index = self.round_robin_index.write().await;
                let selected_provider = providers[*index % providers.len()].clone();
                *index = (*index + 1) % providers.len();
                Ok(selected_provider)
            },
        }
    }


    async fn set_routing_strategy(&self, strategy: RoutingStrategy) -> ProducerResult<()> {
        let mut current_strategy = self.routing_strategy.write().await;
        *current_strategy = strategy.clone();
        println!("Updated routing strategy to {:?}", strategy);
        Ok(())
    }

    async fn get_request_config(&self, provider: &ProviderId) -> shared::GenerationConfig {
        let configs = self.request_configs.read().await;
        configs.get(provider).cloned().unwrap_or_default()
    }
    
    async fn set_prompt(&self, prompt: String) -> ProducerResult<()> {
        let mut current_prompt = self.prompt.write().await;
        *current_prompt = prompt;
        println!("Updated prompt");
        Ok(())
    }

    async fn set_generation_config(&self, config: shared::GenerationConfig) -> ProducerResult<()> {
        let mut configs = self.request_configs.write().await;
        // Apply the config to all providers (could be made provider-specific later)
        for (_, request_config) in configs.iter_mut() {
            *request_config = config.clone();
        }
        println!("Updated generation config for all providers");
        Ok(())
    }

    async fn get_provider_stats(&self) -> ProducerResult<HashMap<ProviderId, ProviderStats>> {
        let stats = self.stats.read().await;
        Ok(stats.clone())
    }

    async fn make_provider_request(&self, provider: &ProviderId, model: &str, topic: &str) -> Result<ProviderResponse, ApiFailure> {
        // Get the stored prompt and combine it with the topic
        let prompt_text = {
            let prompt = self.prompt.read().await;
            if prompt.is_empty() {
                format!("Generate unique attributes for topic: {}", topic)
            } else {
                format!("{} Topic: {}", prompt.clone(), topic)
            }
        };
        
        match provider {
            ProviderId::OpenAI => self.make_openai_request(model, &prompt_text).await,
            ProviderId::Anthropic => self.make_anthropic_request(model, &prompt_text).await,
            ProviderId::Gemini => self.make_gemini_request(model, &prompt_text).await,
        }
    }
}

impl RealProviderRouter {
    /// Make OpenAI API request (private helper)
    async fn make_openai_request(&self, model: &str, prompt: &str) -> Result<ProviderResponse, ApiFailure> {
        let api_keys = self.api_keys.read().await;
        let api_key = api_keys.get(&ProviderId::OpenAI)
            .ok_or_else(|| ApiFailure::AuthenticationFailed)?;

        let client = reqwest::Client::new();
        let request_start = std::time::Instant::now();
        
        let request_body = serde_json::json!({
            "model": model,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ],
            "max_tokens": 150,
            "temperature": 0.7
        });

        let response = client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ApiFailure::NetworkError(e.to_string()))?;

        let response_time = request_start.elapsed();

        if !response.status().is_success() {
            return match response.status().as_u16() {
                401 => Err(ApiFailure::AuthenticationFailed),
                429 => Err(ApiFailure::RateLimitExceeded),
                503 => Err(ApiFailure::ServiceUnavailable),
                _ => Err(ApiFailure::ServerError(response.status().to_string())),
            };
        }

        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ApiFailure::InvalidRequest(format!("Failed to parse response: {}", e)))?;

        let content = response_json
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
            .ok_or_else(|| ApiFailure::InvalidRequest("No content in response".to_string()))?;

        let usage = response_json.get("usage");
        let total_tokens = usage
            .and_then(|u| u.get("total_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as u32;
        let prompt_tokens = usage
            .and_then(|u| u.get("prompt_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as u32;
        let completion_tokens = usage
            .and_then(|u| u.get("completion_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as u32;

        Ok(ProviderResponse {
            content: content.to_string(),
            tokens_used: total_tokens,
            prompt_tokens,
            completion_tokens,
            model_used: model.to_string(),
            response_time,
        })
    }

    /// Make Anthropic API request (private helper)
    async fn make_anthropic_request(&self, model: &str, prompt: &str) -> Result<ProviderResponse, ApiFailure> {
        let api_keys = self.api_keys.read().await;
        let api_key = api_keys.get(&ProviderId::Anthropic)
            .ok_or_else(|| ApiFailure::AuthenticationFailed)?;

        let client = reqwest::Client::new();
        let request_start = std::time::Instant::now();
        
        let request_body = serde_json::json!({
            "model": model,
            "max_tokens": 150,
            "messages": [
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        });

        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ApiFailure::NetworkError(e.to_string()))?;

        let response_time = request_start.elapsed();

        if !response.status().is_success() {
            return match response.status().as_u16() {
                401 => Err(ApiFailure::AuthenticationFailed),
                429 => Err(ApiFailure::RateLimitExceeded),
                503 => Err(ApiFailure::ServiceUnavailable),
                _ => Err(ApiFailure::ServerError(response.status().to_string())),
            };
        }

        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ApiFailure::InvalidRequest(format!("Failed to parse response: {}", e)))?;

        let content = response_json
            .get("content")
            .and_then(|content| content.get(0))
            .and_then(|item| item.get("text"))
            .and_then(|text| text.as_str())
            .ok_or_else(|| ApiFailure::InvalidRequest("No content in response".to_string()))?;

        let usage = response_json.get("usage");
        let input_tokens = usage
            .and_then(|u| u.get("input_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as u32;
        let output_tokens = usage
            .and_then(|u| u.get("output_tokens"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as u32;

        Ok(ProviderResponse {
            content: content.to_string(),
            tokens_used: input_tokens + output_tokens,
            prompt_tokens: input_tokens,
            completion_tokens: output_tokens,
            model_used: model.to_string(),
            response_time,
        })
    }

    /// Make Gemini API request (private helper)
    async fn make_gemini_request(&self, model: &str, prompt: &str) -> Result<ProviderResponse, ApiFailure> {
        let api_keys = self.api_keys.read().await;
        let api_key = api_keys.get(&ProviderId::Gemini)
            .ok_or_else(|| ApiFailure::AuthenticationFailed)?;

        let client = reqwest::Client::new();
        let request_start = std::time::Instant::now();
        
        let request_body = serde_json::json!({
            "contents": [
                {
                    "parts": [
                        {
                            "text": prompt
                        }
                    ]
                }
            ],
            "generationConfig": {
                "maxOutputTokens": 150,
                "temperature": 0.7
            }
        });

        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}", model, api_key);
        
        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ApiFailure::NetworkError(e.to_string()))?;

        let response_time = request_start.elapsed();

        if !response.status().is_success() {
            return match response.status().as_u16() {
                401 => Err(ApiFailure::AuthenticationFailed),
                429 => Err(ApiFailure::RateLimitExceeded),
                503 => Err(ApiFailure::ServiceUnavailable),
                _ => Err(ApiFailure::ServerError(response.status().to_string())),
            };
        }

        let response_json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ApiFailure::InvalidRequest(format!("Failed to parse response: {}", e)))?;

        let content = response_json
            .get("candidates")
            .and_then(|candidates| candidates.get(0))
            .and_then(|candidate| candidate.get("content"))
            .and_then(|content| content.get("parts"))
            .and_then(|parts| parts.get(0))
            .and_then(|part| part.get("text"))
            .and_then(|text| text.as_str())
            .ok_or_else(|| ApiFailure::InvalidRequest("No content in response".to_string()))?;

        // Gemini doesn't always provide token counts in the response
        let usage_metadata = response_json.get("usageMetadata");
        let prompt_token_count = usage_metadata
            .and_then(|u| u.get("promptTokenCount"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as u32;
        let candidates_token_count = usage_metadata
            .and_then(|u| u.get("candidatesTokenCount"))
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as u32;

        Ok(ProviderResponse {
            content: content.to_string(),
            tokens_used: prompt_token_count + candidates_token_count,
            prompt_tokens: prompt_token_count,
            completion_tokens: candidates_token_count,
            model_used: model.to_string(),
            response_time,
        })
    }

}