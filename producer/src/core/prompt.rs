//! Prompt enhancement and exclusion list management

use crate::core::processor::Processor;
use crate::types::ProducerState;
use shared::{GenerationConfig, ProviderId};
use std::collections::HashMap;

/// Provider context window and token limits
#[derive(Debug, Clone)]
pub struct ProviderLimits {
    pub context_window: u32,
    pub max_output_tokens: u32,
    pub estimated_tokens_per_word: f32, // Average tokens per word for exclusion list
}

/// Handles prompt enhancement with dynamic exclusion lists
pub struct PromptHandler {
    /// Provider-specific limits and configurations
    provider_limits: HashMap<ProviderId, ProviderLimits>,

    /// Maximum percentage of context window to use for exclusions
    max_exclusion_percentage: f32,

    /// Minimum number of exclusions to include (if available)
    min_exclusions: usize,
}

impl PromptHandler {
    /// Create new prompt handler with default provider limits
    pub fn new() -> Self {
        let mut provider_limits = HashMap::new();

        // OpenAI GPT-4o-mini limits
        provider_limits.insert(
            ProviderId::OpenAI,
            ProviderLimits {
                context_window: 128000,
                max_output_tokens: 16384,
                estimated_tokens_per_word: 1.3, // English averages ~1.3 tokens per word
            },
        );

        // Anthropic Claude-3-Sonnet limits
        provider_limits.insert(
            ProviderId::Anthropic,
            ProviderLimits {
                context_window: 200000,
                max_output_tokens: 4096,
                estimated_tokens_per_word: 1.3,
            },
        );

        // Google Gemini Pro limits
        provider_limits.insert(
            ProviderId::Gemini,
            ProviderLimits {
                context_window: 30720,
                max_output_tokens: 2048,
                estimated_tokens_per_word: 1.2, // Gemini might be slightly more efficient
            },
        );

        // Random provider (unlimited, but we'll use reasonable defaults)
        provider_limits.insert(
            ProviderId::Random,
            ProviderLimits {
                context_window: 8192,
                max_output_tokens: 1000,
                estimated_tokens_per_word: 1.0, // Simple word-based counting
            },
        );

        Self {
            provider_limits,
            max_exclusion_percentage: 0.3, // Use up to 30% of context window for exclusions
            min_exclusions: 10,            // Always include at least 10 exclusions if available
        }
    }

    /// Calculate optimal number of exclusions based on provider and generation config
    pub fn calculate_optimal_exclusions(
        &self,
        provider: ProviderId,
        generation_config: Option<&GenerationConfig>,
    ) -> usize {
        let limits = self.provider_limits.get(&provider).unwrap_or(&ProviderLimits {
            context_window: 4096,
            max_output_tokens: 1000,
            estimated_tokens_per_word: 1.3,
        });

        // Calculate available tokens for input (context window - output tokens - base prompt overhead)
        let requested_output_tokens = generation_config
            .map(|gc| gc.max_tokens)
            .unwrap_or(limits.max_output_tokens);

        let base_prompt_overhead = 200; // Estimate for base prompt structure and instructions
        let available_input_tokens = limits
            .context_window
            .saturating_sub(requested_output_tokens)
            .saturating_sub(base_prompt_overhead);

        // Reserve tokens for exclusions based on percentage
        let exclusion_token_budget = (available_input_tokens as f32 * self.max_exclusion_percentage) as u32;

        // Convert token budget to word count
        let max_exclusion_words = (exclusion_token_budget as f32 / limits.estimated_tokens_per_word) as usize;

        // Ensure we have at least minimum exclusions, but respect token limits
        std::cmp::max(self.min_exclusions, max_exclusion_words)
    }

    /// Build enhanced prompt with dynamically sized exclusion list
    pub async fn build_enhanced_prompt(
        &self,
        base_prompt: &str,
        provider: ProviderId,
        generation_config: Option<&GenerationConfig>,
        state: &tokio::sync::RwLock<ProducerState>,
        processor: &tokio::sync::RwLock<Processor>,
    ) -> String {
        // Calculate optimal exclusion count for this provider/config
        let optimal_exclusions = self.calculate_optimal_exclusions(provider, generation_config);

        // Get orchestrator seen values from state
        let orchestrator_seen_values = {
            let state_guard = state.read().await;
            state_guard.seen_values_from_orchestrator.clone()
        };

        // Get seen values from orchestrator (authoritative source) and processor stats
        let (combined_seen_values, processor_stats) = {
            let processor_guard = processor.read().await;
            let stats = processor_guard.get_stats();
            (orchestrator_seen_values.unwrap_or(Vec::new()).clone(), stats)
        };

        // Build exclusion list with optimal size
        let (existing_entries, bloom_info) = if combined_seen_values.is_empty() {
            ("None".to_string(), String::new())
        } else {
            // Take the most recent entries up to optimal count
            let recent_entries: Vec<String> = combined_seen_values.iter().take(optimal_exclusions).cloned().collect();

            let exclusion_info = if combined_seen_values.len() > optimal_exclusions {
                format!(
                    "(showing {} most recent of {} total)",
                    optimal_exclusions,
                    combined_seen_values.len()
                )
            } else {
                format!("(showing all {} entries)", recent_entries.len())
            };

            let bloom_info = format!(
                "\n\n🔍 DEDUPLICATION SYSTEM ACTIVE:\n- {} unique entries already discovered\n- Advanced bloom filter tracking with {:.1}% false positive rate\n- Exclusion list optimized for {} provider (max tokens: {}, exclusions: {})\n- CRITICAL: You must avoid ALL entries listed above, including:\n  * Exact matches\n  * Similar spellings or variations\n  * Alternative names for the same item\n  * Translations or different languages for the same concept\n- Focus on generating completely NEW and UNIQUE entries only",
                processor_stats.total_unique_attributes,
                processor_stats.bloom_filter_false_positive_rate * 100.0,
                provider,
                self.provider_limits.get(&provider).map(|l| l.context_window).unwrap_or(4096),
                recent_entries.len()
            );

            (format!("{}\n{}", recent_entries.join("\n"), exclusion_info), bloom_info)
        };

        // Get request size from generation config
        let request_size = generation_config.map(|gc| gc.request_size).unwrap_or(100);

        // Build enhanced prompt with provider-optimized exclusion template
        let enhanced_prompt = format!(
            r#"Generate {request_size} new entries about: {base_prompt}

CRITICAL FORMATTING REQUIREMENTS:
- Words must be strictly alphanumeric (letters and numbers only)
- Words must be lowercase
- One entry per line
- No punctuation, spaces, or special characters
- Examples: "parismuseum", "tokyotower", "londonbridge"

Only generate canonical names, in English when available. Omit any descriptions of the entries.
Previous entries:
{existing_entries}{bloom_info}
Remember:
- Your entries should be entirely unique from the previous
- Entries should be specific
- One entry per line
- Do NOT repeat any previously seen entries, even with slight variations"#
        );

        enhanced_prompt
    }

    /// Update provider limits (for configuration changes)
    pub fn update_provider_limits(&mut self, provider: ProviderId, limits: ProviderLimits) {
        self.provider_limits.insert(provider, limits);
    }

    /// Set exclusion parameters
    pub fn set_exclusion_parameters(&mut self, max_percentage: f32, min_exclusions: usize) {
        self.max_exclusion_percentage = max_percentage.clamp(0.1, 0.8); // 10%-80% range
        self.min_exclusions = min_exclusions;
    }

    /// Get provider limits for debugging/monitoring
    pub fn get_provider_limits(&self, provider: ProviderId) -> Option<&ProviderLimits> {
        self.provider_limits.get(&provider)
    }

    /// Estimate tokens for a given text (rough approximation)
    pub fn estimate_tokens(&self, text: &str, provider: ProviderId) -> u32 {
        let words = text.split_whitespace().count() as f32;
        let tokens_per_word = self
            .provider_limits
            .get(&provider)
            .map(|l| l.estimated_tokens_per_word)
            .unwrap_or(1.3);

        (words * tokens_per_word) as u32
    }
}

impl Default for PromptHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exclusion_calculation() {
        let handler = PromptHandler::new();

        // Test OpenAI with different output token requests
        let low_output_config = GenerationConfig {
            model: "gpt-4o-mini".to_string(),
            batch_size: 1,
            context_window: 128000,
            max_tokens: 1000,
            temperature: 0.7,
            request_size: 80,
        };

        let high_output_config = GenerationConfig {
            model: "gpt-4o-mini".to_string(),
            batch_size: 1,
            context_window: 128000,
            max_tokens: 10000,
            temperature: 0.7,
            request_size: 120,
        };

        let low_exclusions = handler.calculate_optimal_exclusions(ProviderId::OpenAI, Some(&low_output_config));
        let high_exclusions = handler.calculate_optimal_exclusions(ProviderId::OpenAI, Some(&high_output_config));

        // Should allow more exclusions when output tokens are lower
        assert!(low_exclusions > high_exclusions);
        assert!(low_exclusions >= handler.min_exclusions);
        assert!(high_exclusions >= handler.min_exclusions);
    }

    #[test]
    fn test_provider_differences() {
        let handler = PromptHandler::new();

        let config = GenerationConfig {
            model: "test".to_string(),
            batch_size: 1,
            context_window: 4096,
            max_tokens: 1000,
            temperature: 0.7,
            request_size: 75,
        };

        let openai_exclusions = handler.calculate_optimal_exclusions(ProviderId::OpenAI, Some(&config));
        let gemini_exclusions = handler.calculate_optimal_exclusions(ProviderId::Gemini, Some(&config));

        // OpenAI has a larger context window, so should allow more exclusions
        assert!(openai_exclusions > gemini_exclusions);
    }

    #[test]
    fn test_token_estimation() {
        let handler = PromptHandler::new();

        let text = "This is a test sentence with several words";
        let estimated_tokens = handler.estimate_tokens(text, ProviderId::OpenAI);

        // Should be roughly equal to word count * tokens_per_word
        let word_count = text.split_whitespace().count() as u32;
        assert!(estimated_tokens >= word_count); // At least one token per word
        assert!(estimated_tokens <= word_count * 2); // But not too many more
    }
}
