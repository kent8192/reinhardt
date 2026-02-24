//! State management integration tests

use chrono::{Duration, Utc};
use reinhardt_auth::social::flow::{InMemoryStateStore, StateData, StateStore};
use rstest::*;
use std::sync::Arc;

#[tokio::test]
async fn test_state_store_and_retrieve() {
	// Arrange
	let store = InMemoryStateStore::new();
	let data = StateData::new(
		"test_state".to_string(),
		Some("test_nonce".to_string()),
		Some("test_verifier".to_string()),
	);

	// Act
	store.store(data.clone()).await.unwrap();
	let retrieved = store.retrieve("test_state").await.unwrap();

	// Assert
	assert_eq!(retrieved.state, "test_state");
	assert_eq!(retrieved.nonce, Some("test_nonce".to_string()));
	assert_eq!(retrieved.code_verifier, Some("test_verifier".to_string()));
	assert!(!retrieved.is_expired());
}

#[tokio::test]
async fn test_state_reject_expired() {
	// Arrange
	let store = InMemoryStateStore::new();
	let expired_data = StateData::with_ttl(
		"expired_state".to_string(),
		Some("test_nonce".to_string()),
		Some("test_verifier".to_string()),
		Duration::seconds(-1), // Already expired
	);

	// Act
	store.store(expired_data).await.unwrap();
	let result = store.retrieve("expired_state").await;

	// Assert
	assert!(result.is_err(), "Expired state should be rejected");
}

#[tokio::test]
async fn test_state_removed_after_use() {
	// Arrange
	let store = InMemoryStateStore::new();
	let data = StateData::new(
		"test_state".to_string(),
		None,
		Some("test_verifier".to_string()),
	);

	// Act
	store.store(data).await.unwrap();
	store.remove("test_state").await.unwrap();
	let result = store.retrieve("test_state").await;

	// Assert
	assert!(result.is_err(), "State should be removed");
}

#[tokio::test]
async fn test_state_cleanup_expired() {
	// Arrange
	let store = InMemoryStateStore::new();

	// Store valid and expired data
	let valid_data = StateData::new("valid".to_string(), None, None);
	let expired_data =
		StateData::with_ttl("expired".to_string(), None, None, Duration::seconds(-1));

	store.store(valid_data).await.unwrap();
	store.store(expired_data).await.unwrap();

	// Act - cleanup happens on next store
	let new_data = StateData::new("new".to_string(), None, None);
	store.store(new_data).await.unwrap();

	// Assert
	assert!(
		store.retrieve("valid").await.is_ok(),
		"Valid state should exist"
	);
	assert!(
		store.retrieve("new").await.is_ok(),
		"New state should exist"
	);
	assert!(
		store.retrieve("expired").await.is_err(),
		"Expired state should be cleaned up"
	);
}

#[tokio::test]
async fn test_state_concurrent_operations() {
	// Arrange
	let store = Arc::new(InMemoryStateStore::new());

	// Act - Store multiple states concurrently
	let mut handles = vec![];
	for i in 0..10 {
		let store_clone = Arc::clone(&store);
		let handle = tokio::spawn(async move {
			let data = StateData::new(
				format!("state_{}", i),
				Some(format!("nonce_{}", i)),
				Some(format!("verifier_{}", i)),
			);
			store_clone.store(data).await
		});
		handles.push(handle);
	}

	// Wait for all stores to complete
	for handle in handles {
		handle.await.unwrap().unwrap();
	}

	// Assert - All states should be retrievable
	for i in 0..10 {
		let result = store.retrieve(&format!("state_{}", i)).await;
		assert!(result.is_ok(), "State {} should be retrievable", i);
	}
}

#[tokio::test]
async fn test_state_custom_ttl() {
	// Arrange
	let store = InMemoryStateStore::new();
	let custom_ttl = Duration::minutes(30);
	let data = StateData::with_ttl("custom_ttl_state".to_string(), None, None, custom_ttl);

	// Act
	store.store(data).await.unwrap();
	let retrieved = store.retrieve("custom_ttl_state").await.unwrap();

	// Assert
	assert!(!retrieved.is_expired());
	let remaining = (retrieved.expires_at - Utc::now()).num_seconds();
	assert!(
		remaining >= 29 * 60 && remaining <= 30 * 60,
		"Custom TTL should be approximately 30 minutes, got: {} seconds",
		remaining
	);
}

#[tokio::test]
async fn test_state_with_all_fields() {
	// Arrange
	let store = InMemoryStateStore::new();
	let data = StateData::new(
		"full_state".to_string(),
		Some("full_nonce".to_string()),
		Some("full_verifier".to_string()),
	);

	// Act
	store.store(data).await.unwrap();
	let retrieved = store.retrieve("full_state").await.unwrap();

	// Assert
	assert_eq!(retrieved.state, "full_state");
	assert_eq!(retrieved.nonce, Some("full_nonce".to_string()));
	assert_eq!(retrieved.code_verifier, Some("full_verifier".to_string()));
	assert!(!retrieved.is_expired());
}

#[tokio::test]
async fn test_state_with_none_fields() {
	// Arrange
	let store = InMemoryStateStore::new();
	let data = StateData::new("minimal_state".to_string(), None, None);

	// Act
	store.store(data).await.unwrap();
	let retrieved = store.retrieve("minimal_state").await.unwrap();

	// Assert
	assert_eq!(retrieved.state, "minimal_state");
	assert!(retrieved.nonce.is_none());
	assert!(retrieved.code_verifier.is_none());
}

#[tokio::test]
async fn test_state_overwrite() {
	// Arrange
	let store = InMemoryStateStore::new();
	let data1 = StateData::new(
		"state".to_string(),
		Some("nonce1".to_string()),
		Some("verifier1".to_string()),
	);
	let data2 = StateData::new(
		"state".to_string(),
		Some("nonce2".to_string()),
		Some("verifier2".to_string()),
	);

	// Act
	store.store(data1).await.unwrap();
	store.store(data2).await.unwrap();
	let retrieved = store.retrieve("state").await.unwrap();

	// Assert
	assert_eq!(retrieved.nonce, Some("nonce2".to_string()));
	assert_eq!(retrieved.code_verifier, Some("verifier2".to_string()));
}

#[tokio::test]
async fn test_state_retrieve_nonexistent() {
	// Arrange
	let store = InMemoryStateStore::new();

	// Act
	let result = store.retrieve("nonexistent").await;

	// Assert
	assert!(result.is_err());
}

#[tokio::test]
async fn test_state_remove_nonexistent() {
	// Arrange
	let store = InMemoryStateStore::new();

	// Act
	let result = store.remove("nonexistent").await;

	// Assert
	assert!(result.is_err());
}
