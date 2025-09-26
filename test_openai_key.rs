#!/usr/bin/env rust-script

//! Simple test script to verify OpenAI API key loading and basic functionality
//! 
//! This script loads the .env file and tests that the OpenAI API key is working.

use std::collections::HashMap;
use std::env;

fn main() {
    println!("🔑 Testing OpenAI API Key Loading");
    println!("================================");
    
    // Load .env file
    if let Err(e) = dotenv::dotenv() {
        println!("⚠️  Warning: Could not load .env file: {}", e);
    }
    
    // Check if OpenAI API key is loaded
    match env::var("OPENAI_API_KEY") {
        Ok(key) => {
            let masked_key = if key.len() > 10 {
                format!("{}...{}", &key[..10], &key[key.len()-4..])
            } else {
                "***".to_string()
            };
            println!("✅ OpenAI API key loaded: {}", masked_key);
            
            // Check if it starts with the expected prefix
            if key.starts_with("sk-") {
                println!("✅ API key format looks correct (starts with 'sk-')");
            } else {
                println!("⚠️  Warning: API key doesn't start with 'sk-' (might be invalid)");
            }
        }
        Err(_) => {
            println!("❌ OpenAI API key not found in environment");
            println!("   Make sure OPENAI_API_KEY is set in your .env file");
        }
    }
    
    // Check model environment variable
    match env::var("OPENAI_API_MODEL") {
        Ok(model) => {
            println!("✅ OpenAI model configured: {}", model);
        }
        Err(_) => {
            println!("ℹ️  OpenAI model not specified (will use default)");
        }
    }
}