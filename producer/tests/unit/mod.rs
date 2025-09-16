//! Unit tests for producer components

#[cfg(test)]
mod tests {
    use super::super::fixtures::*;
    use producer::*;

    #[test]
    fn test_producer_error_creation() {
        let error = ProducerError::config("test error");
        match error {
            ProducerError::ConfigError { message } => {
                assert_eq!(message, "test error");
            }
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn test_producer_config_creation() {
        let addr = "127.0.0.1:6001".parse().unwrap();
        let config = ProducerConfig::new(addr, "test topic".to_string());
        
        assert_eq!(config.orchestrator_addr, addr);
        assert_eq!(config.topic, "test topic");
        assert_eq!(config.max_concurrent_requests, 10);
    }

    #[test]
    fn test_api_request_creation() {
        use shared::ProviderId;
        
        let request = create_test_request(ProviderId::OpenAI, "test prompt".to_string());
        assert_eq!(request.provider, ProviderId::OpenAI);
        assert_eq!(request.prompt, "test prompt");
        assert_eq!(request.max_tokens, 150);
    }

    #[test]
    fn test_api_response_success() {
        use shared::ProviderId;
        
        let response = create_success_response(
            ProviderId::OpenAI, 
            "test response".to_string(), 
            100
        );
        
        assert!(response.success);
        assert_eq!(response.provider, ProviderId::OpenAI);
        assert_eq!(response.content, "test response");
        assert_eq!(response.tokens_used, 100);
    }

    #[test]
    fn test_api_response_error() {
        use shared::ProviderId;
        
        let response = create_error_response(
            ProviderId::Anthropic, 
            "rate limit".to_string()
        );
        
        assert!(!response.success);
        assert_eq!(response.provider, ProviderId::Anthropic);
        assert_eq!(response.error_message, Some("rate limit".to_string()));
        assert_eq!(response.tokens_used, 0);
    }
}