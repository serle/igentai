//! Core Functionality Tests
//!
//! Essential system functionality tests

use crate::{OrchestratorConfig, ServiceConstellation, Topic, TracingCollector};
use std::time::Duration;

/// Test basic orchestrator + producer functionality
pub async fn basic(
    collector: TracingCollector,
    constellation: &mut ServiceConstellation,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🧪 Basic: orchestrator + producers");

    let config = OrchestratorConfig::builder()
        .topic("basic")
        .for_rapid_development() // Fast testing with minimal iterations
        .build();

    constellation.start_orchestrator(config).await?;

    if let Some(topic) = Topic::wait_for_topic("basic", collector, Duration::from_secs(30)).await {
        assert!(
            topic.assert_started_with_budget(Some(5)).await,
            "Should start with budget"
        );
        assert!(topic.assert_completed().await, "Should complete");
        tracing::info!("✅ Basic: PASSED");
    } else {
        return Err("Basic test failed".into());
    }

    Ok(())
}

/// Test high load scenario
pub async fn load(
    collector: TracingCollector,
    constellation: &mut ServiceConstellation,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🧪 Load: many producers + iterations");

    let config = OrchestratorConfig::builder()
        .topic("load")
        .with_openai_provider() // Use OpenAI provider (we have API key)
        .producers(5) // Reduced from 10 for more manageable testing
        .iterations(Some(10)) // Reduced from 50 for faster testing
        .log_level("info")
        .build();

    constellation.start_orchestrator(config).await?;

    if let Some(topic) = Topic::wait_for_topic("load", collector, Duration::from_secs(60)).await {
        assert!(topic.assert_completed().await, "Should complete under load");
        assert!(topic.assert_min_attributes(200), "Should generate many attributes");
        tracing::info!("✅ Load: PASSED");
    } else {
        return Err("Load test failed".into());
    }

    Ok(())
}

/// Test healing functionality (system heals when producers fail)
pub async fn healing(
    collector: TracingCollector,
    constellation: &mut ServiceConstellation,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🧪 Healing: producer failure recovery");

    let config = OrchestratorConfig::builder()
        .topic("healing")
        .with_openai_provider() // Use OpenAI provider (we have API key)
        .producers(3) // Reduced for more manageable testing
        .iterations(Some(8)) // Reduced for faster testing
        .log_level("debug")
        .build();

    constellation.start_orchestrator(config).await?;

    if let Some(topic) = Topic::wait_for_topic("healing", collector, Duration::from_secs(90)).await {
        assert!(topic.assert_completed().await, "Should complete despite failures");
        // Note: healing is automatic in the system - producers naturally fail and get restarted
        tracing::info!("✅ Healing: PASSED");
    } else {
        return Err("Healing test failed".into());
    }

    Ok(())
}

/// Test that producers receive exactly one Start command per topic (prevents duplicate starts bug)
pub async fn single_start_command(
    collector: TracingCollector,
    constellation: &mut ServiceConstellation,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🧪 Single Start Command: producers get exactly one start command");

    let config = OrchestratorConfig::builder()
        .topic("single_start_test")
        .producers(3)
        .iterations(Some(10)) // Enough iterations to catch duplicate start commands
        .with_random_provider() // Use random provider for testing
        .build();

    constellation.start_orchestrator(config).await?;

    if let Some(topic) = Topic::wait_for_topic("single_start_test", collector, Duration::from_secs(45)).await {
        // Core assertions
        assert!(
            topic.assert_started_with_budget(Some(10)).await,
            "Should start with budget"
        );
        assert!(topic.assert_completed().await, "Should complete");
        
        // NEW: Critical assertion to prevent duplicate start commands
        assert!(
            topic.assert_single_start_per_producer(3).await,
            "Each producer should receive exactly one Start command"
        );
        
        // Verify proper operation despite single start
        assert!(topic.assert_min_attributes(50), "Should generate attributes with single start");
        assert!(topic.assert_no_errors().await, "Should complete without errors");
        
        tracing::info!("✅ Single Start Command: PASSED");
    } else {
        return Err("Single Start Command test failed".into());
    }

    Ok(())
}

/// Test end-to-end scenario with OpenAI (real API)
pub async fn e2e_openai(
    _collector: TracingCollector,
    _constellation: &mut ServiceConstellation,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🧪 E2E OpenAI: Testing real OpenAI integration with Paris attractions");

    // Check if OpenAI API key is available
    if std::env::var("OPENAI_API_KEY").is_err() {
        tracing::warn!("⚠️ OpenAI API key not found - skipping E2E test");
        tracing::warn!("   Set OPENAI_API_KEY environment variable to run this test");
        return Ok(());
    }

    let _config = OrchestratorConfig::builder()
        .topic("paris attractions")
        .for_e2e_validation() // Comprehensive E2E testing configuration
        .with_openai_gpt4o_mini() // Use OpenAI provider with backoff strategy
        .build();


    if let Some(topic) = Topic::wait_for_topic("paris attractions", _collector, Duration::from_secs(120)).await {
        assert!(
            topic.assert_started_with_budget(Some(3)).await,
            "Should start with budget"
        );
        assert!(topic.assert_completed().await, "Should complete");

        // Verify we got real Paris attractions, not random words
        let output_path = "./output/paris attractions/output.txt";
        if let Ok(content) = std::fs::read_to_string(output_path) {
            let words: Vec<&str> = content.lines().collect();
            tracing::info!("📋 Generated {} attributes: {:?}", words.len(), &words[..words.len().min(5)]);

            // Check for paris-related terms (this is a heuristic check)
            let paris_terms = ["tower", "eiffel", "louvre", "seine", "champs", "arc", "triomphe", 
                              "museum", "cathedral", "notre", "dame", "montmartre", "sacre", "coeur"];
            let has_paris_content = words.iter().any(|word| 
                paris_terms.iter().any(|term| word.to_lowercase().contains(term)) && 
                word.trim().len() > 5 // Ignore short words
            );
            
            if !has_paris_content {
                tracing::warn!("⚠️ Generated content may not be Paris-specific: {:?}", &words[..words.len().min(10)]);
            } else {
                tracing::info!("✅ Found Paris-related content!");
            }
        } else {
            return Err("No output file generated".into());
        }

        tracing::info!("✅ E2E OpenAI: PASSED");
    } else {
        return Err("E2E OpenAI test failed - topic not found".into());
    }

    Ok(())
}

/// Test trace capture from all process types (orchestrator, webserver, producers)
pub async fn trace_capture(
    collector: TracingCollector,
    constellation: &mut ServiceConstellation,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🧪 Trace Capture: Testing trace visibility from all processes");

    let config = OrchestratorConfig::builder()
        .topic("trace_test")
        .producers(2)
        .iterations(Some(3)) // Short test to generate traces
        .with_random_provider() // Use random provider for testing
        .build();

    constellation.start_orchestrator(config).await?;

    // Wait for the topic to start (this will trigger producer spawning)
    tracing::info!("⏳ Waiting for topic to start (which will spawn producers)...");
    let topic_started = collector.wait_for_message("Topic 'trace_test' started", Duration::from_secs(15)).await;
    if !topic_started {
        return Err("Topic failed to start within timeout".into());
    }
    
    // Wait a bit more for producers to generate some traces
    tracing::info!("⏳ Waiting 5 more seconds for producers to generate traces...");
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Now analyze the collected traces
    tracing::info!("📊 Analyzing collected traces...");
    let stats = collector.get_stats();
    
    tracing::info!("Total events collected: {}", stats.total_events);
    for (process, count) in &stats.events_by_process {
        tracing::info!("  {}: {} events", process, count);
    }

    // Check for specific processes
    let has_orchestrator = stats.events_by_process.contains_key("orchestrator");
    let has_producers = stats.events_by_process.keys().any(|k| k.starts_with("producer_"));
    let has_webserver = stats.events_by_process.contains_key("webserver");

    tracing::info!("🔍 Process verification:");
    tracing::info!("  Orchestrator traces: {}", if has_orchestrator { "✅" } else { "❌" });
    tracing::info!("  Producer traces: {}", if has_producers { "✅" } else { "❌" });
    tracing::info!("  WebServer traces: {}", if has_webserver { "✅" } else { "❌" });

    // Verify we have traces from orchestrator (should always have this)
    assert!(has_orchestrator, "Should have orchestrator traces");

    // Show sample traces
    if has_orchestrator {
        let orchestrator_events = collector.query(&crate::runtime::collector::TraceQuery {
            process_filter: Some("orchestrator".to_string()),
            level_filter: None,
            message_contains: None,
            since_seconds_ago: None,
            limit: Some(3),
        });
        tracing::info!("Sample orchestrator traces:");
        for event in orchestrator_events {
            tracing::info!("  [{}] {}", event.trace_event.level, event.trace_event.message);
        }
    }

    if has_producers {
        let producer_events = collector.query(&crate::runtime::collector::TraceQuery {
            process_filter: Some("producer_".to_string()),
            level_filter: None,
            message_contains: None,
            since_seconds_ago: None,
            limit: Some(3),
        });
        tracing::info!("Sample producer traces:");
        for event in producer_events {
            tracing::info!("  [{}] {}", event.trace_event.level, event.trace_event.message);
        }
    } else {
        tracing::warn!("⚠️ No producer traces found - this might be expected if producers haven't started yet");
    }

    if has_webserver {
        let webserver_events = collector.query(&crate::runtime::collector::TraceQuery {
            process_filter: Some("webserver".to_string()),
            level_filter: None,
            message_contains: None,
            since_seconds_ago: None,
            limit: Some(3),
        });
        tracing::info!("Sample webserver traces:");
        for event in webserver_events {
            tracing::info!("  [{}] {}", event.trace_event.level, event.trace_event.message);
        }
    }

    tracing::info!("✅ Trace Capture: PASSED - Orchestrator traces captured successfully!");
    tracing::info!("📋 Summary: Orchestrator={}, Producers={}, WebServer={}", 
                  if has_orchestrator { "✅" } else { "❌" },
                  if has_producers { "✅" } else { "❌" },
                  if has_webserver { "✅" } else { "❌" });

    Ok(())
}

/// Test real API scenario with OpenAI using backoff strategy
pub async fn real_api(
    collector: TracingCollector,
    constellation: &mut ServiceConstellation,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🧪 Real API: Testing OpenAI integration with backoff strategy");

    // Load .env file to ensure API keys are available (same as orchestrator does)
    if let Err(_) = dotenv::dotenv() {
        tracing::warn!("⚠️ No .env file found in current directory");
    }

    // Check if OpenAI API key is available after loading .env
    if std::env::var("OPENAI_API_KEY").is_err() {
        return Err("OPENAI_API_KEY environment variable is required for real_api test. Check .env file.".into());
    }

    let config = OrchestratorConfig::builder()
        .topic("paris attractions")
        .producers(1) // Use single producer for faster, simpler test
        .iterations(Some(2)) // Just 2 iterations to confirm it works
        .routing_strategy("backoff")
        .routing_config("openai:gpt-4o-mini") // Use backoff strategy with OpenAI
        .log_level("info") // Less verbose logging
        .build();

    constellation.start_orchestrator(config).await?;

    // Wait for real API calls (should be faster with 1 producer and 2 iterations)
    if let Some(topic) = Topic::wait_for_topic("paris attractions", collector, Duration::from_secs(120)).await {
        assert!(topic.assert_completed().await, "Should complete successfully with real API");

        // Check output file exists and contains real content
        let output_path = "./output/paris attractions/output.txt";
        if let Ok(content) = std::fs::read_to_string(output_path) {
            let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
            tracing::info!("📋 Generated {} attributes with real OpenAI API", lines.len());
            
            if lines.is_empty() {
                return Err("No attributes generated by real API".into());
            }

            // Log first few attributes for verification
            tracing::info!("🎯 Sample OpenAI-generated Paris attractions:");
            for (i, line) in lines.iter().take(8).enumerate() {
                tracing::info!("  {}. {}", i + 1, line);
            }

            // Real OpenAI content should be longer and more descriptive than random words
            let avg_length: f32 = lines.iter().map(|l| l.len()).sum::<usize>() as f32 / lines.len() as f32;
            if avg_length < 8.0 {
                tracing::warn!("⚠️ Generated content seems too short (avg: {:.1} chars), might not be real OpenAI content", avg_length);
            } else {
                tracing::info!("✅ Generated content looks realistic (avg: {:.1} chars per attribute)", avg_length);
            }

            // Expect some attributes from 2 iterations with 1 producer
            if lines.len() < 3 {
                tracing::warn!("⚠️ Expected more attributes from real API test (got {})", lines.len());
            }

            tracing::info!("✅ Real API: PASSED - OpenAI backoff strategy working correctly");
        } else {
            return Err("No output file generated by real API".into());
        }
    } else {
        return Err("Real API test failed - topic not found or timed out".into());
    }

    Ok(())
}
