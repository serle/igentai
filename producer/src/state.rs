//! Producer state management

use std::sync::Arc;
use tokio::sync::RwLock;
use crate::types::ProducerState;

/// Shared producer state wrapper
pub type SharedProducerState = Arc<RwLock<ProducerState>>;

/// Create new shared producer state
pub fn create_shared_state(state: ProducerState) -> SharedProducerState {
    Arc::new(RwLock::new(state))
}