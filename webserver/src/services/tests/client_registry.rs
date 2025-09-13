//! Tests for the ClientRegistry service

use super::fixtures::*;
use super::helpers::*;
use crate::services::RealClientRegistry;
use crate::traits::ClientRegistry;

mod real_client_registry_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_client_registry() {
        let state = create_test_state();
        let registry = RealClientRegistry::new(state.clone());
        
        // Test that registry is created successfully
        assert_eq!(registry.get_connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_connection_count_starts_at_zero() {
        let state = create_test_state();
        let registry = RealClientRegistry::new(state);
        
        assert_eq!(registry.get_connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_add_client() {
        let state = create_test_state();
        let registry = RealClientRegistry::new(state);
        
        let connection = create_test_client_connection();
        let client_id = connection.id.clone();
        
        let result = registry.add_client(connection).await;
        assert!(result.is_ok());
        
        assert_eq!(registry.get_connection_count().await, 1);
        
        // Test that we can get the client back
        let retrieved = registry.get_client(&client_id).await;
        assert!(retrieved.is_ok());
    }

    #[tokio::test]
    async fn test_remove_client() {
        let state = create_test_state();
        let registry = RealClientRegistry::new(state);
        
        let connection = create_test_client_connection();
        let client_id = connection.id.clone();
        
        // Add client first
        let _ = registry.add_client(connection).await;
        assert_eq!(registry.get_connection_count().await, 1);
        
        // Remove client
        let result = registry.remove_client(&client_id).await;
        assert!(result.is_ok());
        
        assert_eq!(registry.get_connection_count().await, 0);
        
        // Client should no longer be retrievable
        let retrieved = registry.get_client(&client_id).await;
        assert!(retrieved.is_err());
    }

    #[tokio::test]
    async fn test_remove_nonexistent_client() {
        let state = create_test_state();
        let registry = RealClientRegistry::new(state);
        
        let client_id = generate_client_id();
        
        // Try to remove a client that doesn't exist
        let result = registry.remove_client(&client_id).await;
        assert!(result.is_ok()); // Should succeed gracefully
        
        assert_eq!(registry.get_connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_multiple_clients() {
        let state = create_test_state();
        let registry = RealClientRegistry::new(state);
        
        let mut client_ids = vec![];
        
        // Add multiple clients
        for _ in 0..5 {
            let connection = create_test_client_connection();
            let client_id = connection.id.clone();
            client_ids.push(client_id);
            
            let result = registry.add_client(connection).await;
            assert!(result.is_ok());
        }
        
        assert_eq!(registry.get_connection_count().await, 5);
        
        // Remove some clients
        for client_id in &client_ids[0..2] {
            let result = registry.remove_client(client_id).await;
            assert!(result.is_ok());
        }
        
        assert_eq!(registry.get_connection_count().await, 3);
        
        // Verify correct clients remain
        for client_id in &client_ids[0..2] {
            let retrieved = registry.get_client(client_id).await;
            assert!(retrieved.is_err());
        }
        for client_id in &client_ids[2..5] {
            let retrieved = registry.get_client(client_id).await;
            assert!(retrieved.is_ok());
        }
    }

    #[tokio::test]
    async fn test_get_all_clients() {
        let state = create_test_state();
        let registry = RealClientRegistry::new(state);
        
        let mut client_ids = vec![];
        
        // Add clients
        for _ in 0..3 {
            let connection = create_test_client_connection();
            let client_id = connection.id.clone();
            client_ids.push(client_id);
            
            let _ = registry.add_client(connection).await;
        }
        
        let all_clients = registry.get_all_clients().await;
        assert_eq!(all_clients.len(), 3);
        
        // Verify all client IDs are present
        for client_id in &client_ids {
            assert!(all_clients.contains(client_id));
        }
    }

    #[tokio::test]
    async fn test_cleanup_stale_connections() {
        let state = create_test_state();
        let registry = RealClientRegistry::new(state);
        
        let mut client_ids = vec![];
        
        // Add clients
        for _ in 0..3 {
            let connection = create_test_client_connection();
            let client_id = connection.id.clone();
            client_ids.push(client_id);
            
            let _ = registry.add_client(connection).await;
        }
        
        assert_eq!(registry.get_connection_count().await, 3);
        
        // Cleanup stale connections
        let result = registry.cleanup_stale_connections().await;
        assert!(result.is_ok());
        
        // For this test, assume no connections were stale
        assert_eq!(registry.get_connection_count().await, 3);
    }

    #[tokio::test]
    async fn test_concurrent_operations() {
        let state = create_test_state();
        let registry = RealClientRegistry::new(state);
        
        // Spawn multiple concurrent add operations
        let mut handles = vec![];
        
        for _ in 0..10 {
            let registry_clone = registry.clone();
            let handle = tokio::spawn(async move {
                let connection = create_test_client_connection();
                registry_clone.add_client(connection).await
            });
            handles.push(handle);
        }
        
        // Wait for all operations to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
        }
        
        // Verify final count
        assert_eq!(registry.get_connection_count().await, 10);
    }

    #[tokio::test]
    async fn test_get_client_not_found() {
        let state = create_test_state();
        let registry = RealClientRegistry::new(state);
        
        let client_id = generate_client_id();
        
        // Try to get a client that doesn't exist
        let result = registry.get_client(&client_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_client_connection_lifecycle() {
        let state = create_test_state();
        let registry = RealClientRegistry::new(state);
        
        let connection = create_test_client_connection();
        let client_id = connection.id.clone();
        
        // Test full lifecycle
        assert_eq!(registry.get_connection_count().await, 0);
        
        // Add client
        let _ = registry.add_client(connection).await;
        assert_eq!(registry.get_connection_count().await, 1);
        
        // Get client
        let retrieved = registry.get_client(&client_id).await;
        assert!(retrieved.is_ok());
        
        // Remove client
        let _ = registry.remove_client(&client_id).await;
        assert_eq!(registry.get_connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_error_handling() {
        let state = create_test_state();
        let registry = RealClientRegistry::new(state);
        
        let client_id = generate_client_id();
        
        // Test various error conditions
        
        // 1. Get non-existent client
        let result = registry.get_client(&client_id).await;
        assert!(result.is_err());
        
        // 2. Remove non-existent client (should succeed gracefully)
        let result = registry.remove_client(&client_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_state_consistency() {
        let state = create_test_state();
        let registry = RealClientRegistry::new(state);
        
        let connection = create_test_client_connection();
        let client_id = connection.id.clone();
        
        // Perform multiple operations
        let _ = registry.add_client(connection).await;
        let _ = registry.get_client(&client_id).await;
        let _ = registry.remove_client(&client_id).await;
        
        // State should be consistent
        assert_eq!(registry.get_connection_count().await, 0);
        
        // Operations after removal should handle gracefully
        let result = registry.get_client(&client_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_performance() {
        let state = create_test_state();
        let registry = RealClientRegistry::new(state);
        
        // Test performance of repeated operations
        let iterations = 10;
        let start = std::time::Instant::now();
        
        for _ in 0..iterations {
            let connection = create_test_client_connection();
            let client_id = connection.id.clone();
            let _ = registry.add_client(connection).await;
            let _ = registry.get_client(&client_id).await;
            let _ = registry.remove_client(&client_id).await;
        }
        
        let duration = start.elapsed();
        let avg_duration = duration / iterations;
        
        // Operations should complete in reasonable time
        assert!(avg_duration < std::time::Duration::from_millis(10));
    }
}