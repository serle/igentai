//! Integration tests for .env file loading functionality

use std::env;
use std::fs;
use crate::services::api_keys::RealApiKeySource;
use crate::traits::ApiKeySource;

/// Test that RealApiKeySource can load API keys from a .env file
#[tokio::test]
async fn test_env_file_loading() {
    // Save current working directory
    let original_dir = env::current_dir().unwrap();
    
    // Create a temporary directory for this test
    let temp_dir = tempfile::tempdir().unwrap();
    env::set_current_dir(&temp_dir).unwrap();
    
    // Create a test .env file
    let env_content = "OPENAI_API_KEY=test-openai-from-env\nANTHROPIC_API_KEY=test-anthropic-from-env\n";
    fs::write(".env", env_content).unwrap();
    
    // Clear any existing environment variables that might interfere
    env::remove_var("OPENAI_API_KEY");
    env::remove_var("ANTHROPIC_API_KEY");
    
    // Create API key source and get keys
    let api_source = RealApiKeySource;
    let result = api_source.get_api_keys().await;
    
    // Restore original directory
    env::set_current_dir(original_dir).unwrap();
    
    // Verify the result
    assert!(result.is_ok(), "Expected API keys to be loaded successfully");
    let keys = result.unwrap();
    
    // Should have both keys from .env file
    assert_eq!(keys.len(), 2);
    
    // Check that we got the expected keys
    let openai_key = keys.iter().find(|kv| kv.key == "OPENAI_API_KEY");
    let anthropic_key = keys.iter().find(|kv| kv.key == "ANTHROPIC_API_KEY");
    
    assert!(openai_key.is_some(), "Should have OPENAI_API_KEY");
    assert!(anthropic_key.is_some(), "Should have ANTHROPIC_API_KEY");
    assert_eq!(openai_key.unwrap().value, "test-openai-from-env");
    assert_eq!(anthropic_key.unwrap().value, "test-anthropic-from-env");
}

/// Test that API key validation works correctly
#[tokio::test]
async fn test_api_key_validation_functionality() {
    // This test verifies that the RealApiKeySource can successfully validate
    // and return API keys, demonstrating that .env loading is working
    
    // Create API key source and get keys
    let api_source = RealApiKeySource;
    let result = api_source.get_api_keys().await;
    
    // Since we know there's an OPENAI_API_KEY in the .env file, this should succeed
    assert!(result.is_ok(), "Expected API keys to be loaded successfully");
    let keys = result.unwrap();
    
    // Should have at least the OPENAI_API_KEY (required)
    assert!(!keys.is_empty(), "Should have at least one API key");
    
    // Should have the required OPENAI_API_KEY
    let openai_key = keys.iter().find(|kv| kv.key == "OPENAI_API_KEY");
    assert!(openai_key.is_some(), "Should have OPENAI_API_KEY");
    assert!(!openai_key.unwrap().value.is_empty(), "OPENAI_API_KEY should not be empty");
}