//! Token rotation for enhanced security
//!
//! Provides automatic token rotation capabilities to enhance security
//! by regularly refreshing authentication tokens.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Token rotation configuration
///
/// # Examples
///
/// ```
/// use reinhardt_auth::TokenRotationConfig;
///
/// let config = TokenRotationConfig::new()
///     .rotation_interval(3600)  // 1 hour
///     .grace_period(300);       // 5 minutes
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenRotationConfig {
	/// Interval in seconds between rotations
	pub rotation_interval: i64,
	/// Grace period in seconds where old token is still valid
	pub grace_period: i64,
	/// Maximum number of active tokens per user
	pub max_active_tokens: usize,
	/// Whether to automatically rotate on each use
	pub rotate_on_use: bool,
}

impl TokenRotationConfig {
	/// Create a new token rotation configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TokenRotationConfig;
	///
	/// let config = TokenRotationConfig::new();
	/// ```
	pub fn new() -> Self {
		Self {
			rotation_interval: 3600, // 1 hour
			grace_period: 300,       // 5 minutes
			max_active_tokens: 5,
			rotate_on_use: false,
		}
	}

	/// Set the rotation interval
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TokenRotationConfig;
	///
	/// let config = TokenRotationConfig::new()
	///     .rotation_interval(7200);  // 2 hours
	/// ```
	pub fn rotation_interval(mut self, seconds: i64) -> Self {
		self.rotation_interval = seconds;
		self
	}

	/// Set the grace period
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TokenRotationConfig;
	///
	/// let config = TokenRotationConfig::new()
	///     .grace_period(600);  // 10 minutes
	/// ```
	pub fn grace_period(mut self, seconds: i64) -> Self {
		self.grace_period = seconds;
		self
	}

	/// Set the maximum number of active tokens
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TokenRotationConfig;
	///
	/// let config = TokenRotationConfig::new()
	///     .max_active_tokens(3);
	/// ```
	pub fn max_active_tokens(mut self, count: usize) -> Self {
		self.max_active_tokens = count;
		self
	}

	/// Set whether to rotate on each use
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TokenRotationConfig;
	///
	/// let config = TokenRotationConfig::new()
	///     .rotate_on_use(true);
	/// ```
	pub fn rotate_on_use(mut self, enabled: bool) -> Self {
		self.rotate_on_use = enabled;
		self
	}
}

impl Default for TokenRotationConfig {
	fn default() -> Self {
		Self::new()
	}
}

/// Token rotation record
///
/// Tracks the rotation history of a token.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::TokenRotationRecord;
///
/// let record = TokenRotationRecord::new("old_token", "new_token", 1234567890);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenRotationRecord {
	/// The old token that was rotated
	pub old_token: String,
	/// The new token that replaced it
	pub new_token: String,
	/// Timestamp when rotation occurred (Unix timestamp)
	pub rotated_at: i64,
	/// User ID associated with the tokens
	pub user_id: i64,
}

impl TokenRotationRecord {
	/// Create a new token rotation record
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TokenRotationRecord;
	///
	/// let record = TokenRotationRecord::new("old_token", "new_token", 1234567890);
	/// assert_eq!(record.old_token(), "old_token");
	/// assert_eq!(record.new_token(), "new_token");
	/// ```
	pub fn new(
		old_token: impl Into<String>,
		new_token: impl Into<String>,
		rotated_at: i64,
	) -> Self {
		Self {
			old_token: old_token.into(),
			new_token: new_token.into(),
			rotated_at,
			user_id: 0,
		}
	}

	/// Set the user ID
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TokenRotationRecord;
	///
	/// let record = TokenRotationRecord::new("old", "new", 123)
	///     .with_user_id(42);
	/// assert_eq!(record.user_id(), 42);
	/// ```
	pub fn with_user_id(mut self, user_id: i64) -> Self {
		self.user_id = user_id;
		self
	}

	/// Get the old token
	pub fn old_token(&self) -> &str {
		&self.old_token
	}

	/// Get the new token
	pub fn new_token(&self) -> &str {
		&self.new_token
	}

	/// Get the rotation timestamp
	pub fn rotated_at(&self) -> i64 {
		self.rotated_at
	}

	/// Get the user ID
	pub fn user_id(&self) -> i64 {
		self.user_id
	}

	/// Check if the grace period has expired
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::TokenRotationRecord;
	///
	/// let record = TokenRotationRecord::new("old", "new", 1000);
	///
	/// assert!(!record.grace_period_expired(1200, 300)); // Within grace period
	/// assert!(record.grace_period_expired(1400, 300));  // Beyond grace period
	/// ```
	pub fn grace_period_expired(&self, current_time: i64, grace_period: i64) -> bool {
		current_time > self.rotated_at + grace_period
	}
}

/// Token rotation manager
///
/// Manages automatic token rotation and grace periods.
///
/// # Examples
///
/// ```
/// use reinhardt_auth::{AutoTokenRotationManager, TokenRotationConfig};
///
/// let config = TokenRotationConfig::new();
/// let manager = AutoTokenRotationManager::new(config);
/// ```
#[derive(Debug, Clone)]
pub struct AutoTokenRotationManager {
	config: TokenRotationConfig,
	rotation_history: Arc<RwLock<HashMap<String, TokenRotationRecord>>>,
}

impl AutoTokenRotationManager {
	/// Create a new token rotation manager
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::{AutoTokenRotationManager, TokenRotationConfig};
	///
	/// let config = TokenRotationConfig::new();
	/// let manager = AutoTokenRotationManager::new(config);
	/// ```
	pub fn new(config: TokenRotationConfig) -> Self {
		Self {
			config,
			rotation_history: Arc::new(RwLock::new(HashMap::new())),
		}
	}

	/// Check if a token should be rotated
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_auth::{AutoTokenRotationManager, TokenRotationConfig};
	///
	/// let config = TokenRotationConfig::new()
	///     .rotation_interval(3600);
	/// let manager = AutoTokenRotationManager::new(config);
	///
	/// // Token created 2 hours ago should be rotated
	/// assert!(manager.should_rotate(1000, 5000));
	/// ```
	pub fn should_rotate(&self, token_created_at: i64, current_time: i64) -> bool {
		current_time - token_created_at >= self.config.rotation_interval
	}

	/// Record a token rotation
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_auth::{AutoTokenRotationManager, TokenRotationConfig, TokenRotationRecord};
	///
	/// # async fn example() {
	/// let config = TokenRotationConfig::new();
	/// let manager = AutoTokenRotationManager::new(config);
	///
	/// let record = TokenRotationRecord::new("old", "new", 1234567890)
	///     .with_user_id(42);
	/// manager.record_rotation(record).await;
	/// # }
	/// ```
	pub async fn record_rotation(&self, record: TokenRotationRecord) {
		let mut history = self.rotation_history.write().await;
		history.insert(record.old_token.clone(), record);
	}

	/// Get the new token for a rotated token
	///
	/// Returns None if the token hasn't been rotated or grace period expired.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_auth::{AutoTokenRotationManager, TokenRotationConfig, TokenRotationRecord};
	///
	/// # async fn example() {
	/// let config = TokenRotationConfig::new()
	///     .grace_period(300);
	/// let manager = AutoTokenRotationManager::new(config);
	///
	/// let record = TokenRotationRecord::new("old", "new", 1000)
	///     .with_user_id(42);
	/// manager.record_rotation(record).await;
	///
	/// // Within grace period
	/// assert_eq!(manager.get_rotated_token("old", 1200).await, Some("new".to_string()));
	///
	/// // Beyond grace period
	/// assert_eq!(manager.get_rotated_token("old", 2000).await, None);
	/// # }
	/// ```
	pub async fn get_rotated_token(&self, old_token: &str, current_time: i64) -> Option<String> {
		let history = self.rotation_history.read().await;
		history.get(old_token).and_then(|record| {
			if record.grace_period_expired(current_time, self.config.grace_period) {
				None
			} else {
				Some(record.new_token.clone())
			}
		})
	}

	/// Clean up expired rotation records
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_auth::{AutoTokenRotationManager, TokenRotationConfig, TokenRotationRecord};
	///
	/// # async fn example() {
	/// let config = TokenRotationConfig::new()
	///     .grace_period(300);
	/// let manager = AutoTokenRotationManager::new(config);
	///
	/// manager.record_rotation(TokenRotationRecord::new("old1", "new1", 1000)).await;
	/// manager.record_rotation(TokenRotationRecord::new("old2", "new2", 2000)).await;
	///
	/// // Cleanup at time 2200: old1 (1000+300=1300) is expired, old2 (2000+300=2300) is not
	/// let removed = manager.cleanup_expired(2200).await;
	/// assert_eq!(removed, 1); // Only old1 is expired
	/// # }
	/// ```
	pub async fn cleanup_expired(&self, current_time: i64) -> usize {
		let mut history = self.rotation_history.write().await;
		let before_count = history.len();
		history.retain(|_, record| {
			!record.grace_period_expired(current_time, self.config.grace_period)
		});
		before_count - history.len()
	}

	/// Get the configuration
	pub fn config(&self) -> &TokenRotationConfig {
		&self.config
	}

	/// Get the number of rotation records
	pub async fn rotation_count(&self) -> usize {
		self.rotation_history.read().await.len()
	}

	/// Get all rotation records (for testing purposes)
	///
	/// Returns a copy of all rotation records currently stored.
	pub async fn rotation_records(&self) -> Vec<TokenRotationRecord> {
		self.rotation_history
			.read()
			.await
			.values()
			.cloned()
			.collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_config_creation() {
		let config = TokenRotationConfig::new();
		assert_eq!(config.rotation_interval, 3600);
		assert_eq!(config.grace_period, 300);
		assert_eq!(config.max_active_tokens, 5);
		assert!(!config.rotate_on_use);
	}

	#[test]
	fn test_config_builder() {
		let config = TokenRotationConfig::new()
			.rotation_interval(7200)
			.grace_period(600)
			.max_active_tokens(3)
			.rotate_on_use(true);

		assert_eq!(config.rotation_interval, 7200);
		assert_eq!(config.grace_period, 600);
		assert_eq!(config.max_active_tokens, 3);
		assert!(config.rotate_on_use);
	}

	#[test]
	fn test_rotation_record_creation() {
		let record = TokenRotationRecord::new("old_token", "new_token", 1234567890);
		assert_eq!(record.old_token(), "old_token");
		assert_eq!(record.new_token(), "new_token");
		assert_eq!(record.rotated_at(), 1234567890);
		assert_eq!(record.user_id(), 0);
	}

	#[test]
	fn test_rotation_record_with_user_id() {
		let record = TokenRotationRecord::new("old", "new", 123).with_user_id(42);
		assert_eq!(record.user_id(), 42);
	}

	#[test]
	fn test_grace_period_expired() {
		let record = TokenRotationRecord::new("old", "new", 1000);

		assert!(!record.grace_period_expired(1200, 300)); // 200s elapsed, grace = 300s
		assert!(!record.grace_period_expired(1300, 300)); // 300s elapsed, grace = 300s
		assert!(record.grace_period_expired(1301, 300)); // 301s elapsed, grace = 300s
	}

	#[tokio::test]
	async fn test_manager_creation() {
		let config = TokenRotationConfig::new();
		let manager = AutoTokenRotationManager::new(config.clone());
		assert_eq!(manager.config().rotation_interval, config.rotation_interval);
		assert_eq!(manager.rotation_count().await, 0);
	}

	#[test]
	fn test_should_rotate() {
		let config = TokenRotationConfig::new().rotation_interval(3600);
		let manager = AutoTokenRotationManager::new(config);

		assert!(!manager.should_rotate(5000, 7000)); // 2000s elapsed < 3600s
		assert!(!manager.should_rotate(5000, 8599)); // 3599s elapsed < 3600s
		assert!(manager.should_rotate(5000, 8600)); // 3600s elapsed = 3600s
		assert!(manager.should_rotate(5000, 10000)); // 5000s elapsed > 3600s
	}

	#[tokio::test]
	async fn test_record_and_get_rotation() {
		let config = TokenRotationConfig::new().grace_period(300);
		let manager = AutoTokenRotationManager::new(config);

		let record = TokenRotationRecord::new("old_token", "new_token", 1000).with_user_id(42);
		manager.record_rotation(record).await;

		// Within grace period
		assert_eq!(
			manager.get_rotated_token("old_token", 1200).await,
			Some("new_token".to_string())
		);

		// Beyond grace period
		assert_eq!(manager.get_rotated_token("old_token", 2000).await, None);
	}

	#[tokio::test]
	async fn test_cleanup_expired() {
		let config = TokenRotationConfig::new().grace_period(300);
		let manager = AutoTokenRotationManager::new(config);

		manager
			.record_rotation(TokenRotationRecord::new("old1", "new1", 1000))
			.await;
		manager
			.record_rotation(TokenRotationRecord::new("old2", "new2", 2000))
			.await;
		manager
			.record_rotation(TokenRotationRecord::new("old3", "new3", 3000))
			.await;

		assert_eq!(manager.rotation_count().await, 3);

		let removed = manager.cleanup_expired(2500).await;
		assert_eq!(removed, 2); // old1 (expires 1300) and old2 (expires 2300) are both expired at 2500
		assert_eq!(manager.rotation_count().await, 1);

		let removed2 = manager.cleanup_expired(3500).await;
		assert_eq!(removed2, 1); // old3 is now expired (3000 + 300 = 3300 < 3500)
		assert_eq!(manager.rotation_count().await, 0);
	}

	#[tokio::test]
	async fn test_multiple_rotations() {
		let config = TokenRotationConfig::new().grace_period(300);
		let manager = AutoTokenRotationManager::new(config);

		manager
			.record_rotation(
				TokenRotationRecord::new("token1_v1", "token1_v2", 1000).with_user_id(1),
			)
			.await;
		manager
			.record_rotation(
				TokenRotationRecord::new("token2_v1", "token2_v2", 1100).with_user_id(2),
			)
			.await;

		assert_eq!(
			manager.get_rotated_token("token1_v1", 1200).await,
			Some("token1_v2".to_string())
		);
		assert_eq!(
			manager.get_rotated_token("token2_v1", 1200).await,
			Some("token2_v2".to_string())
		);
	}
}
