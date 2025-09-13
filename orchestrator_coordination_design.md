# Orchestrator Central Coordination Design

## Core Coordination Responsibilities

### 1. Routing Configuration Management
- **Monitor producer performance** via ProducerStats
- **Dynamically adjust routing strategies** based on success rates and response times
- **Send ProducerRequest::UpdateRoutingStrategy** to redistribute load
- **Coordinate provider failover** when performance degrades

### 2. Unique Attribute Integration
- **Receive ProducerResponse::DataUpdate** from all producers
- **Maintain global unique attribute set** using bloom filters
- **Coordinate duplicate detection** across all producers
- **Send consolidated attributes** to webserver via TaskUpdate::NewAttributes

### 3. Bloom Filter Coordination
- **Generate global bloom filter** from unique attribute set
- **Distribute updated filters** via ProducerRequest::UpdateBloomFilter
- **Monitor filter effectiveness** through ProducerStats
- **Optimize filter parameters** based on performance data

### 4. Dynamic Prompt and Strategy Management
- **Receive TaskRequest::TopicRequest** from webserver
- **Distribute topic/prompt updates** via ProducerRequest::UpdatePrompt
- **Adjust routing strategies** based on real-time performance
- **Coordinate prompt changes** across all active producers

## Enhanced Message Processing Loop

```rust
pub async fn coordination_cycle(&mut self) -> OrchestratorResult<()> {
    // 1. Process all incoming messages
    let producer_responses = self.collect_producer_responses().await?;
    let producer_stats = self.collect_producer_stats().await?;
    let task_requests = self.collect_task_requests().await?;
    
    // 2. Update global state and make decisions
    let new_attributes = self.integrate_unique_attributes(producer_responses).await?;
    let routing_decisions = self.analyze_performance_and_decide(producer_stats).await?;
    let prompt_updates = self.process_task_requests(task_requests).await?;
    
    // 3. Generate bloom filter updates
    let bloom_filter = self.generate_updated_bloom_filter(&new_attributes).await?;
    
    // 4. Send coordinated commands
    self.send_routing_updates(routing_decisions).await?;
    self.send_prompt_updates(prompt_updates).await?;
    self.distribute_bloom_filter(bloom_filter).await?;
    
    // 5. Broadcast system updates
    self.broadcast_new_attributes(new_attributes).await?;
    self.broadcast_system_metrics().await?;
    
    Ok(())
}
```

## Key Orchestrator Decision-Making Logic

### Performance-Based Routing
```rust
async fn analyze_performance_and_decide(&self, stats: Vec<ProducerStats>) -> RoutingDecisions {
    for producer_stat in stats {
        // Analyze success rates, response times, provider performance
        if provider_performance.success_rate < 0.8 {
            // Reduce load on failing provider
            routing_decisions.push(RouteUpdate::ReduceLoad(provider_id));
        }
        if provider_performance.avg_response_time > threshold {
            // Switch to faster provider
            routing_decisions.push(RouteUpdate::SwitchProvider(producer_id, faster_provider));
        }
    }
    routing_decisions
}
```

### Bloom Filter Optimization
```rust
async fn generate_updated_bloom_filter(&self, new_attributes: &[String]) -> BloomFilter {
    // Update global unique set
    self.unique_attributes.extend(new_attributes);
    
    // Generate optimized bloom filter
    let filter = BloomFilter::optimal_for_size(self.unique_attributes.len());
    for attr in &self.unique_attributes {
        filter.insert(attr);
    }
    
    filter
}
```

## Message Flow Coordination

### 1. WebServer → Orchestrator → Producers
TaskRequest::TopicRequest → ProducerRequest::Start (to all producers)
TaskRequest::StopGeneration → ProducerRequest::Stop (to all producers)

### 2. Producers → Orchestrator → WebServer  
ProducerResponse::DataUpdate → TaskUpdate::NewAttributes (aggregated)
ProducerStats → TaskStats (performance summary)

### 3. Producer ↔ Orchestrator ↔ Producer
ProducerStats (from A) → ProducerRequest::UpdateRoutingStrategy (to B)
Performance analysis → ProducerRequest::UpdateBloomFilter (to all)

## Integration Points

### File System Coordination
- Write unique attributes to disk as they arrive
- Maintain bloom filter state persistence
- Coordinate file sync across all producers

### Process Health Management
- Monitor producer health via ProducerStats
- Coordinate graceful restart when producers fail
- Redistribute load during recovery

### Real-time Web Updates
- Stream new attributes to web clients immediately
- Broadcast performance metrics continuously
- Send system status updates on state changes