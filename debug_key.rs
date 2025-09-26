use std::env;

fn main() {
    // Load .env file
    let _ = dotenvy::dotenv();
    
    match env::var("OPENAI_API_KEY") {
        Ok(key) => {
            println!("Key length: {}", key.len());
            println!("Key starts with: {}", &key[..20]);
            println!("Key ends with: {}", &key[key.len()-10..]);
            println!("Key contains whitespace: {}", key.chars().any(|c| c.is_whitespace()));
            
            // Check if it's a service account key
            if key.starts_with("sk-svcacct-") {
                println!("✅ Service account key format detected");
            } else if key.starts_with("sk-proj-") {
                println!("✅ Project key format detected");
            } else if key.starts_with("sk-") {
                println!("✅ Standard key format detected");
            } else {
                println!("❌ Unknown key format");
            }
        }
        Err(e) => {
            println!("Failed to load key: {}", e);
        }
    }
}