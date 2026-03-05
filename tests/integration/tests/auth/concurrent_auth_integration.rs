//! Concurrent Authentication Integration Tests
//!
//! This module contains tests for concurrent access scenarios in authentication,
//! including simultaneous login attempts, token operations, and session management.
//!
//! # Test Categories
//!
//! - Concurrent Login: Multiple simultaneous authentication attempts
//! - Token Concurrency: Parallel token creation and validation
//! - Session Race Conditions: Concurrent session updates
//! - Thread Safety: Verifying Send + Sync implementations

use reinhardt_auth::token_storage::InMemoryTokenStorage;
use reinhardt_auth::{
	Argon2Hasher, BaseUser, DefaultUser, PasswordHasher, StoredToken, TokenStorage,
};
use rstest::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Barrier;
use uuid::Uuid;

// =============================================================================
// Fixtures
// =============================================================================

#[fixture]
fn argon2_hasher() -> Argon2Hasher {
	Argon2Hasher::default()
}

#[fixture]
fn token_storage() -> Arc<InMemoryTokenStorage> {
	Arc::new(InMemoryTokenStorage::new())
}

// =============================================================================
// Concurrent Password Hashing Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_concurrent_password_hashing(argon2_hasher: Argon2Hasher) {
	let hasher = Arc::new(argon2_hasher);
	let passwords: Vec<String> = (0..10).map(|i| format!("password_{}", i)).collect();
	let barrier = Arc::new(Barrier::new(passwords.len()));

	let mut handles = Vec::new();

	for password in passwords.clone() {
		let h = hasher.clone();
		let b = barrier.clone();

		let handle = tokio::spawn(async move {
			b.wait().await;
			h.hash(&password)
		});

		handles.push(handle);
	}

	let results: Vec<_> = futures::future::join_all(handles).await;

	// All hashing operations should succeed
	for (i, result) in results.iter().enumerate() {
		assert!(result.is_ok(), "Task {} should complete without panic", i);
		let hash_result = result.as_ref().unwrap();
		assert!(hash_result.is_ok(), "Hash {} should succeed", i);
	}

	// Verify each hash is unique
	let hashes: Vec<String> = results.into_iter().map(|r| r.unwrap().unwrap()).collect();

	for i in 0..hashes.len() {
		for j in (i + 1)..hashes.len() {
			assert_ne!(
				hashes[i], hashes[j],
				"Hashes should be unique (collision between {} and {})",
				i, j
			);
		}
	}
}

#[rstest]
#[tokio::test]
async fn test_concurrent_password_verification(argon2_hasher: Argon2Hasher) {
	let hasher = Arc::new(argon2_hasher);
	let password = "shared_password_123";
	let hash = hasher.hash(password).unwrap();
	let hash = Arc::new(hash);

	let barrier = Arc::new(Barrier::new(10));
	let success_count = Arc::new(AtomicUsize::new(0));

	let mut handles = Vec::new();

	for _ in 0..10 {
		let h = hasher.clone();
		let hash_clone = hash.clone();
		let b = barrier.clone();
		let count = success_count.clone();

		let handle = tokio::spawn(async move {
			b.wait().await;
			let result = h.verify(password, &hash_clone);
			if result.is_ok() && result.unwrap() {
				count.fetch_add(1, Ordering::SeqCst);
			}
		});

		handles.push(handle);
	}

	futures::future::join_all(handles).await;

	assert_eq!(
		success_count.load(Ordering::SeqCst),
		10,
		"All concurrent verifications should succeed"
	);
}

// =============================================================================
// Concurrent Token Storage Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_concurrent_token_creation(token_storage: Arc<InMemoryTokenStorage>) {
	let barrier = Arc::new(Barrier::new(100));

	let mut handles = Vec::new();

	for i in 0..100 {
		let storage = token_storage.clone();
		let b = barrier.clone();
		let token = format!("token_{}", i);

		let handle = tokio::spawn(async move {
			b.wait().await;
			storage
				.store(StoredToken {
					token,
					user_id: i,
					expires_at: None,
					metadata: Default::default(),
				})
				.await
		});

		handles.push(handle);
	}

	let results: Vec<_> = futures::future::join_all(handles).await;

	// All store operations should succeed
	for (i, result) in results.iter().enumerate() {
		assert!(result.is_ok(), "Task {} should complete without panic", i);
	}

	// Verify all tokens are stored
	assert_eq!(
		token_storage.len().await,
		100,
		"All 100 tokens should be stored"
	);
}

#[rstest]
#[tokio::test]
async fn test_concurrent_token_lookup(token_storage: Arc<InMemoryTokenStorage>) {
	// Pre-populate storage
	for i in 0..50 {
		let token = format!("token_{}", i);
		token_storage
			.store(StoredToken {
				token,
				user_id: i,
				expires_at: None,
				metadata: Default::default(),
			})
			.await
			.unwrap();
	}

	let barrier = Arc::new(Barrier::new(100));
	let found_count = Arc::new(AtomicUsize::new(0));

	let mut handles = Vec::new();

	// 50 lookups for existing tokens, 50 for non-existing
	for i in 0..100 {
		let storage = token_storage.clone();
		let b = barrier.clone();
		let count = found_count.clone();
		let token = format!("token_{}", i % 75); // Some will exist, some won't

		let handle = tokio::spawn(async move {
			b.wait().await;
			if storage.get(&token).await.is_ok() {
				count.fetch_add(1, Ordering::SeqCst);
			}
		});

		handles.push(handle);
	}

	futures::future::join_all(handles).await;

	// Tokens 0-49 should be found, others not
	// We're looking up tokens 0-74 (some twice), but only 0-49 exist
	let found = found_count.load(Ordering::SeqCst);
	assert!(found > 0, "Should find some tokens");
	assert!(found <= 100, "Found count should not exceed attempts");
}

#[rstest]
#[tokio::test]
async fn test_concurrent_token_revocation(token_storage: Arc<InMemoryTokenStorage>) {
	// Pre-populate storage
	for i in 0..100 {
		let token = format!("token_{}", i);
		token_storage
			.store(StoredToken {
				token,
				user_id: i,
				expires_at: None,
				metadata: Default::default(),
			})
			.await
			.unwrap();
	}

	let barrier = Arc::new(Barrier::new(100));

	let mut handles = Vec::new();

	// Revoke all tokens concurrently
	for i in 0..100 {
		let storage = token_storage.clone();
		let b = barrier.clone();
		let token = format!("token_{}", i);

		let handle = tokio::spawn(async move {
			b.wait().await;
			let _ = storage.delete(&token).await;
		});

		handles.push(handle);
	}

	futures::future::join_all(handles).await;

	// All tokens should be revoked
	assert_eq!(token_storage.len().await, 0, "All tokens should be revoked");

	// Verify no tokens remain
	for i in 0..100 {
		let token = format!("token_{}", i);
		assert!(
			token_storage.get(&token).await.is_err(),
			"Token {} should not exist after revocation",
			i
		);
	}
}

// =============================================================================
// Concurrent User Operations Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_concurrent_user_password_updates() {
	let user = Arc::new(tokio::sync::RwLock::new(DefaultUser {
		id: Uuid::new_v4(),
		username: "concurrent_user".to_string(),
		email: "concurrent@example.com".to_string(),
		first_name: "Concurrent".to_string(),
		last_name: "User".to_string(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_staff: false,
		is_superuser: false,
		date_joined: chrono::Utc::now(),
		user_permissions: Vec::new(),
		groups: Vec::new(),
	}));

	let barrier = Arc::new(Barrier::new(10));
	let success_count = Arc::new(AtomicUsize::new(0));

	let mut handles = Vec::new();

	for i in 0..10 {
		let u = user.clone();
		let b = barrier.clone();
		let count = success_count.clone();
		let password = format!("password_{}", i);

		let handle = tokio::spawn(async move {
			b.wait().await;
			let mut user = u.write().await;
			if user.set_password(&password).is_ok() {
				count.fetch_add(1, Ordering::SeqCst);
			}
		});

		handles.push(handle);
	}

	futures::future::join_all(handles).await;

	assert_eq!(
		success_count.load(Ordering::SeqCst),
		10,
		"All password updates should succeed"
	);

	// The user should have a password hash set
	let user = user.read().await;
	assert!(
		user.password_hash.is_some(),
		"User should have a password hash"
	);
}

// =============================================================================
// Race Condition Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_concurrent_read_write_token(token_storage: Arc<InMemoryTokenStorage>) {
	let token = "race_test_token";
	let user_id: i64 = 1;

	// Initial store
	token_storage
		.store(StoredToken {
			token: token.to_string(),
			user_id,
			expires_at: None,
			metadata: Default::default(),
		})
		.await
		.unwrap();

	let barrier = Arc::new(Barrier::new(20));
	let read_attempts = Arc::new(AtomicUsize::new(0));
	let write_success = Arc::new(AtomicUsize::new(0));

	let mut handles = Vec::new();

	// 10 readers
	for _ in 0..10 {
		let storage = token_storage.clone();
		let b = barrier.clone();
		let attempts = read_attempts.clone();

		let handle = tokio::spawn(async move {
			b.wait().await;
			for _ in 0..100 {
				// Track attempts, not success - success depends on timing
				let _ = storage.get(token).await;
				attempts.fetch_add(1, Ordering::SeqCst);
			}
		});

		handles.push(handle);
	}

	// 10 writers (alternating store/delete)
	for i in 0..10 {
		let storage = token_storage.clone();
		let b = barrier.clone();
		let count = write_success.clone();

		let handle = tokio::spawn(async move {
			b.wait().await;
			for j in 0..100 {
				if (i + j) % 2 == 0 {
					let _ = storage
						.store(StoredToken {
							token: token.to_string(),
							user_id,
							expires_at: None,
							metadata: Default::default(),
						})
						.await;
				} else {
					let _ = storage.delete(token).await;
				}
				count.fetch_add(1, Ordering::SeqCst);
			}
		});

		handles.push(handle);
	}

	futures::future::join_all(handles).await;

	// All writes should succeed
	assert_eq!(
		write_success.load(Ordering::SeqCst),
		1000,
		"All write operations should complete"
	);

	// All reads should be attempted (success depends on timing with store/delete)
	assert_eq!(
		read_attempts.load(Ordering::SeqCst),
		1000,
		"All read operations should be attempted"
	);
}

// =============================================================================
// Thread Safety Tests
// =============================================================================

#[rstest]
fn test_hasher_is_send_sync() {
	fn assert_send_sync<T: Send + Sync>() {}
	assert_send_sync::<Argon2Hasher>();
}

#[rstest]
fn test_token_storage_is_send_sync() {
	fn assert_send_sync<T: Send + Sync>() {}
	assert_send_sync::<InMemoryTokenStorage>();
}

// =============================================================================
// High Contention Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_high_contention_same_token(token_storage: Arc<InMemoryTokenStorage>) {
	let token = "high_contention_token";
	let user_id: i64 = 1;

	let barrier = Arc::new(Barrier::new(100));
	let operations = Arc::new(AtomicUsize::new(0));

	let mut handles = Vec::new();

	// 100 tasks all operating on the same token
	for _i in 0..100 {
		let storage = token_storage.clone();
		let b = barrier.clone();
		let ops = operations.clone();

		let handle = tokio::spawn(async move {
			b.wait().await;

			// Each task does store, get, delete
			let _ = storage
				.store(StoredToken {
					token: token.to_string(),
					user_id,
					expires_at: None,
					metadata: Default::default(),
				})
				.await;
			let _ = storage.get(token).await;
			let _ = storage.delete(token).await;

			ops.fetch_add(3, Ordering::SeqCst);
		});

		handles.push(handle);
	}

	futures::future::join_all(handles).await;

	assert_eq!(
		operations.load(Ordering::SeqCst),
		300,
		"All 300 operations should complete"
	);
}

// =============================================================================
// Use Case Tests
// =============================================================================

#[rstest]
#[tokio::test]
async fn test_simulated_login_spike(argon2_hasher: Argon2Hasher) {
	// Simulate a spike of 50 concurrent login attempts
	let hasher = Arc::new(argon2_hasher);
	let password = "user_password_123";
	let hash = Arc::new(hasher.hash(password).unwrap());

	let barrier = Arc::new(Barrier::new(50));
	let successful_logins = Arc::new(AtomicUsize::new(0));
	let failed_logins = Arc::new(AtomicUsize::new(0));

	let mut handles = Vec::new();

	for i in 0..50 {
		let h = hasher.clone();
		let hash_clone = hash.clone();
		let b = barrier.clone();
		let success = successful_logins.clone();
		let failed = failed_logins.clone();

		// Some users enter correct password, some wrong
		let attempt_password = if i % 5 == 0 {
			"wrong_password"
		} else {
			password
		};

		let handle = tokio::spawn(async move {
			b.wait().await;
			match h.verify(attempt_password, &hash_clone) {
				Ok(true) => {
					success.fetch_add(1, Ordering::SeqCst);
				}
				Ok(false) | Err(_) => {
					failed.fetch_add(1, Ordering::SeqCst);
				}
			}
		});

		handles.push(handle);
	}

	futures::future::join_all(handles).await;

	let successful = successful_logins.load(Ordering::SeqCst);
	let failed = failed_logins.load(Ordering::SeqCst);

	// 10 should fail (every 5th), 40 should succeed
	assert_eq!(successful + failed, 50, "All 50 attempts should complete");
	assert_eq!(failed, 10, "10 attempts should fail");
	assert_eq!(successful, 40, "40 attempts should succeed");
}

#[rstest]
#[tokio::test]
async fn test_session_token_management_flow(token_storage: Arc<InMemoryTokenStorage>) {
	// Simulate realistic session management
	let num_users = 20;
	let operations_per_user = 5;

	let barrier = Arc::new(Barrier::new(num_users));
	let total_operations = Arc::new(AtomicUsize::new(0));

	let mut handles = Vec::new();

	for user_id in 0..num_users {
		let storage = token_storage.clone();
		let b = barrier.clone();
		let ops = total_operations.clone();

		let handle = tokio::spawn(async move {
			b.wait().await;

			for op in 0..operations_per_user {
				let token = format!("session_{}_{}", user_id, op);

				// Store new session
				storage
					.store(StoredToken {
						token: token.clone(),
						user_id: user_id as i64,
						expires_at: None,
						metadata: Default::default(),
					})
					.await
					.unwrap();

				// Verify session
				let retrieved = storage.get(&token).await;
				assert!(retrieved.is_ok(), "Session should exist after store");

				// Simulate session usage delay
				tokio::time::sleep(tokio::time::Duration::from_micros(100)).await;

				// Delete old session
				if op > 0 {
					let old_token = format!("session_{}_{}", user_id, op - 1);
					let _ = storage.delete(&old_token).await;
				}

				ops.fetch_add(1, Ordering::SeqCst);
			}
		});

		handles.push(handle);
	}

	futures::future::join_all(handles).await;

	assert_eq!(
		total_operations.load(Ordering::SeqCst),
		num_users * operations_per_user,
		"All operations should complete"
	);

	// Each user should have their latest session
	assert_eq!(
		token_storage.len().await,
		num_users,
		"Each user should have exactly one active session"
	);
}
