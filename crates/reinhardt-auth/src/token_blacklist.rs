//! Token Blacklist
//!
//! Provides token blacklisting and refresh token management for JWT authentication.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Reason for blacklisting a token
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BlacklistReason {
	/// User logged out
	Logout,
	/// Token was compromised
	Compromised,
	/// Manual revocation by admin
	ManualRevoke,
	/// Token rotation
	Rotated,
}

/// Blacklisted token entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlacklistedToken {
	/// Token JTI (JWT ID)
	pub jti: String,
	/// When the token was blacklisted
	pub blacklisted_at: DateTime<Utc>,
	/// Reason for blacklisting
	pub reason: BlacklistReason,
	/// When the original token expires
	pub expires_at: DateTime<Utc>,
}

/// Statistics for blacklisted tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlacklistStats {
	pub total_blacklisted: usize,
	pub by_logout: usize,
	pub by_compromise: usize,
	pub by_manual_revoke: usize,
	pub by_rotation: usize,
}

/// Token blacklist trait for different storage backends
#[async_trait]
pub trait TokenBlacklist: Send + Sync {
	/// Add a token to the blacklist
	async fn blacklist(
		&self,
		jti: &str,
		expires_at: DateTime<Utc>,
		reason: BlacklistReason,
	) -> Result<(), String>;

	/// Check if a token is blacklisted
	async fn is_blacklisted(&self, jti: &str) -> Result<bool, String>;

	/// Remove expired tokens from blacklist
	async fn cleanup_expired(&self) -> Result<usize, String>;

	/// Get blacklist statistics
	async fn get_stats(&self) -> Result<BlacklistStats, String>;
}

/// In-memory token blacklist implementation
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{InMemoryTokenBlacklist, TokenBlacklist, BlacklistReason};
/// use chrono::{Utc, Duration};
///
/// #[tokio::main]
/// async fn main() {
///     let blacklist = InMemoryTokenBlacklist::new();
///
///     let jti = "token_123";
///     let expires_at = Utc::now() + Duration::hours(1);
///
///     blacklist.blacklist(jti, expires_at, BlacklistReason::Logout).await.unwrap();
///     assert!(blacklist.is_blacklisted(jti).await.unwrap());
/// }
/// ```
pub struct InMemoryTokenBlacklist {
	tokens: Arc<Mutex<HashMap<String, BlacklistedToken>>>,
}

impl InMemoryTokenBlacklist {
	/// Create a new in-memory token blacklist
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::InMemoryTokenBlacklist;
	///
	/// let blacklist = InMemoryTokenBlacklist::new();
	/// ```
	pub fn new() -> Self {
		Self {
			tokens: Arc::new(Mutex::new(HashMap::new())),
		}
	}
}

impl Default for InMemoryTokenBlacklist {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl TokenBlacklist for InMemoryTokenBlacklist {
	async fn blacklist(
		&self,
		jti: &str,
		expires_at: DateTime<Utc>,
		reason: BlacklistReason,
	) -> Result<(), String> {
		let mut tokens = self.tokens.lock().await;
		tokens.insert(
			jti.to_string(),
			BlacklistedToken {
				jti: jti.to_string(),
				blacklisted_at: Utc::now(),
				reason,
				expires_at,
			},
		);
		Ok(())
	}

	async fn is_blacklisted(&self, jti: &str) -> Result<bool, String> {
		let tokens = self.tokens.lock().await;
		Ok(tokens.contains_key(jti))
	}

	async fn cleanup_expired(&self) -> Result<usize, String> {
		let mut tokens = self.tokens.lock().await;
		let now = Utc::now();
		let before_count = tokens.len();

		tokens.retain(|_, token| token.expires_at > now);

		Ok(before_count - tokens.len())
	}

	async fn get_stats(&self) -> Result<BlacklistStats, String> {
		let tokens = self.tokens.lock().await;

		let mut stats = BlacklistStats {
			total_blacklisted: tokens.len(),
			by_logout: 0,
			by_compromise: 0,
			by_manual_revoke: 0,
			by_rotation: 0,
		};

		for token in tokens.values() {
			match token.reason {
				BlacklistReason::Logout => stats.by_logout += 1,
				BlacklistReason::Compromised => stats.by_compromise += 1,
				BlacklistReason::ManualRevoke => stats.by_manual_revoke += 1,
				BlacklistReason::Rotated => stats.by_rotation += 1,
			}
		}

		Ok(stats)
	}
}

/// Refresh token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshToken {
	/// Token JTI
	pub jti: String,
	/// User ID
	pub user_id: String,
	/// Created at
	pub created_at: DateTime<Utc>,
	/// Expires at
	pub expires_at: DateTime<Utc>,
	/// Whether token has been used
	pub is_used: bool,
}

/// Refresh token store trait
#[async_trait]
pub trait RefreshTokenStore: Send + Sync {
	/// Store a refresh token
	async fn store(&self, token: RefreshToken) -> Result<(), String>;

	/// Get a refresh token by JTI
	async fn get(&self, jti: &str) -> Result<Option<RefreshToken>, String>;

	/// Mark token as used
	async fn mark_used(&self, jti: &str) -> Result<(), String>;

	/// Delete a refresh token
	async fn delete(&self, jti: &str) -> Result<(), String>;

	/// Cleanup expired tokens
	async fn cleanup_expired(&self) -> Result<usize, String>;
}

/// In-memory refresh token store
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{InMemoryRefreshTokenStore, RefreshTokenStore, RefreshToken};
/// use chrono::{Utc, Duration};
///
/// #[tokio::main]
/// async fn main() {
///     let store = InMemoryRefreshTokenStore::new();
///
///     let token = RefreshToken {
///         jti: "refresh_123".to_string(),
///         user_id: "user_456".to_string(),
///         created_at: Utc::now(),
///         expires_at: Utc::now() + Duration::days(7),
///         is_used: false,
///     };
///
///     store.store(token.clone()).await.unwrap();
///     let retrieved = store.get(&token.jti).await.unwrap();
///     assert!(retrieved.is_some());
/// }
/// ```
pub struct InMemoryRefreshTokenStore {
	tokens: Arc<Mutex<HashMap<String, RefreshToken>>>,
}

impl InMemoryRefreshTokenStore {
	/// Create a new in-memory refresh token store
	pub fn new() -> Self {
		Self {
			tokens: Arc::new(Mutex::new(HashMap::new())),
		}
	}
}

impl Default for InMemoryRefreshTokenStore {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl RefreshTokenStore for InMemoryRefreshTokenStore {
	async fn store(&self, token: RefreshToken) -> Result<(), String> {
		let mut tokens = self.tokens.lock().await;
		tokens.insert(token.jti.clone(), token);
		Ok(())
	}

	async fn get(&self, jti: &str) -> Result<Option<RefreshToken>, String> {
		let tokens = self.tokens.lock().await;
		Ok(tokens.get(jti).cloned())
	}

	async fn mark_used(&self, jti: &str) -> Result<(), String> {
		let mut tokens = self.tokens.lock().await;
		if let Some(token) = tokens.get_mut(jti) {
			token.is_used = true;
			Ok(())
		} else {
			Err("Token not found".to_string())
		}
	}

	async fn delete(&self, jti: &str) -> Result<(), String> {
		let mut tokens = self.tokens.lock().await;
		tokens.remove(jti);
		Ok(())
	}

	async fn cleanup_expired(&self) -> Result<usize, String> {
		let mut tokens = self.tokens.lock().await;
		let now = Utc::now();
		let before_count = tokens.len();

		tokens.retain(|_, token| token.expires_at > now);

		Ok(before_count - tokens.len())
	}
}

/// Token rotation manager
///
/// Manages automatic token rotation for enhanced security.
pub struct TokenRotationManager {
	blacklist: Arc<dyn TokenBlacklist>,
	refresh_store: Arc<dyn RefreshTokenStore>,
}

impl TokenRotationManager {
	/// Create a new token rotation manager
	pub fn new(
		blacklist: Arc<dyn TokenBlacklist>,
		refresh_store: Arc<dyn RefreshTokenStore>,
	) -> Self {
		Self {
			blacklist,
			refresh_store,
		}
	}

	/// Rotate a refresh token
	///
	/// Marks the old token as used and blacklists it, then stores the new token.
	pub async fn rotate_token(
		&self,
		old_jti: &str,
		new_token: RefreshToken,
		old_expires_at: DateTime<Utc>,
	) -> Result<(), String> {
		// Mark old token as used
		self.refresh_store.mark_used(old_jti).await?;

		// Blacklist old token
		self.blacklist
			.blacklist(old_jti, old_expires_at, BlacklistReason::Rotated)
			.await?;

		// Store new token
		self.refresh_store.store(new_token).await?;

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use chrono::Duration;

	#[tokio::test]
	async fn test_blacklist_token() {
		let blacklist = InMemoryTokenBlacklist::new();
		let jti = "token_123";
		let expires_at = Utc::now() + Duration::hours(1);

		blacklist
			.blacklist(jti, expires_at, BlacklistReason::Logout)
			.await
			.unwrap();

		assert!(blacklist.is_blacklisted(jti).await.unwrap());
	}

	#[tokio::test]
	async fn test_blacklist_cleanup() {
		let blacklist = InMemoryTokenBlacklist::new();

		// Add expired token
		let expired_jti = "expired_token";
		let expired_at = Utc::now() - Duration::hours(1);
		blacklist
			.blacklist(expired_jti, expired_at, BlacklistReason::Logout)
			.await
			.unwrap();

		// Add valid token
		let valid_jti = "valid_token";
		let valid_expires = Utc::now() + Duration::hours(1);
		blacklist
			.blacklist(valid_jti, valid_expires, BlacklistReason::Logout)
			.await
			.unwrap();

		// Cleanup
		let removed = blacklist.cleanup_expired().await.unwrap();
		assert_eq!(removed, 1);

		assert!(!blacklist.is_blacklisted(expired_jti).await.unwrap());
		assert!(blacklist.is_blacklisted(valid_jti).await.unwrap());
	}

	#[tokio::test]
	async fn test_blacklist_stats() {
		let blacklist = InMemoryTokenBlacklist::new();
		let expires_at = Utc::now() + Duration::hours(1);

		blacklist
			.blacklist("token1", expires_at, BlacklistReason::Logout)
			.await
			.unwrap();
		blacklist
			.blacklist("token2", expires_at, BlacklistReason::Compromised)
			.await
			.unwrap();
		blacklist
			.blacklist("token3", expires_at, BlacklistReason::Logout)
			.await
			.unwrap();

		let stats = blacklist.get_stats().await.unwrap();
		assert_eq!(stats.total_blacklisted, 3);
		assert_eq!(stats.by_logout, 2);
		assert_eq!(stats.by_compromise, 1);
	}

	#[tokio::test]
	async fn test_refresh_token_store() {
		let store = InMemoryRefreshTokenStore::new();

		let token = RefreshToken {
			jti: "refresh_123".to_string(),
			user_id: "user_456".to_string(),
			created_at: Utc::now(),
			expires_at: Utc::now() + Duration::days(7),
			is_used: false,
		};

		store.store(token.clone()).await.unwrap();

		let retrieved = store.get(&token.jti).await.unwrap();
		assert!(retrieved.is_some());
		assert!(!retrieved.unwrap().is_used);

		store.mark_used(&token.jti).await.unwrap();
		let used_token = store.get(&token.jti).await.unwrap().unwrap();
		assert!(used_token.is_used);
	}

	#[tokio::test]
	async fn test_token_rotation() {
		let blacklist = Arc::new(InMemoryTokenBlacklist::new());
		let refresh_store = Arc::new(InMemoryRefreshTokenStore::new());
		let manager = TokenRotationManager::new(blacklist.clone(), refresh_store.clone());

		let old_token = RefreshToken {
			jti: "old_token".to_string(),
			user_id: "user_123".to_string(),
			created_at: Utc::now(),
			expires_at: Utc::now() + Duration::days(7),
			is_used: false,
		};

		refresh_store.store(old_token.clone()).await.unwrap();

		let new_token = RefreshToken {
			jti: "new_token".to_string(),
			user_id: "user_123".to_string(),
			created_at: Utc::now(),
			expires_at: Utc::now() + Duration::days(7),
			is_used: false,
		};

		manager
			.rotate_token(&old_token.jti, new_token.clone(), old_token.expires_at)
			.await
			.unwrap();

		// Check old token is blacklisted and marked as used
		assert!(blacklist.is_blacklisted(&old_token.jti).await.unwrap());
		let old_from_store = refresh_store.get(&old_token.jti).await.unwrap().unwrap();
		assert!(old_from_store.is_used);

		// Check new token is stored
		let new_from_store = refresh_store.get(&new_token.jti).await.unwrap();
		assert!(new_from_store.is_some());
	}
}
