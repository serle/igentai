//! Optimization strategy trait definitions
//!
//! This module contains only the trait definitions for optimization strategies,
//! keeping interfaces clean and focused.

use super::types::*;
use crate::error::OrchestratorResult;
use async_trait::async_trait;

/// Core optimization strategy trait that all optimizers must implement
#[mockall::automock]
#[async_trait]
pub trait OptimizerStrategy: Send + Sync {
    /// Generate optimization recommendations based on current context
    /// 
    /// This is the primary method that analyzes the current state and produces
    /// actionable recommendations for improving performance.
    async fn optimize(&self, context: OptimizationContext) -> OrchestratorResult<OptimizationResult>;
    
    /// Provide performance feedback to improve future optimizations
    /// 
    /// This allows the optimizer to learn from actual performance outcomes
    /// and adjust its strategies accordingly.
    async fn update_performance(&mut self, feedback: PerformanceFeedback);
    
    /// Reset internal state for a new optimization session
    /// 
    /// Called when starting a new topic or major configuration change.
    async fn reset(&mut self);
    
    /// Get current internal state for debugging and monitoring
    /// 
    /// This should return information about the optimizer's current state
    /// without exposing internal implementation details.
    async fn get_state(&self) -> OptimizerState;
}