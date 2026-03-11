//! Session storage migration tools
//!
//! This module provides tools for migrating session data between different backends.
//!
//! ## Example
//!
//! ```rust,no_run,ignore
//! use reinhardt_auth::sessions::migration::{SessionMigrator, Migrator};
//! use reinhardt_auth::sessions::backends::{InMemorySessionBackend};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let source_backend = InMemorySessionBackend::new();
//! let target_backend = InMemorySessionBackend::new();
//!
//! // Create migrator
//! let migrator = SessionMigrator::new(source_backend, target_backend);
//!
//! // Run migration
//! let result = migrator.migrate().await?;
//! println!("Migrated {} sessions, {} failed", result.migrated, result.failed);
//! # Ok(())
//! # }
//! ```

use super::backends::{SessionBackend, SessionError};
use super::cleanup::CleanupableBackend;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Migration result
///
/// # Example
///
/// ```rust
/// use reinhardt_auth::sessions::migration::MigrationResult;
///
/// let result = MigrationResult {
///     total: 100,
///     migrated: 95,
///     failed: 5,
///     errors: vec!["Key 'abc' failed: timeout".to_string()],
/// };
///
/// assert_eq!(result.total, 100);
/// assert_eq!(result.migrated, 95);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationResult {
	/// Total number of sessions to migrate
	pub total: usize,
	/// Number of sessions successfully migrated
	pub migrated: usize,
	/// Number of sessions that failed to migrate
	pub failed: usize,
	/// List of error messages
	pub errors: Vec<String>,
}

impl MigrationResult {
	/// Create a new empty migration result
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::migration::MigrationResult;
	///
	/// let result = MigrationResult::new();
	/// assert_eq!(result.total, 0);
	/// assert_eq!(result.migrated, 0);
	/// ```
	pub fn new() -> Self {
		Self {
			total: 0,
			migrated: 0,
			failed: 0,
			errors: Vec::new(),
		}
	}

	/// Check if migration was successful
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::migration::MigrationResult;
	///
	/// let mut result = MigrationResult::new();
	/// result.total = 10;
	/// result.migrated = 10;
	/// assert!(result.is_successful());
	///
	/// result.failed = 1;
	/// assert!(!result.is_successful());
	/// ```
	pub fn is_successful(&self) -> bool {
		self.failed == 0
	}

	/// Get success rate as percentage
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::migration::MigrationResult;
	///
	/// let mut result = MigrationResult::new();
	/// result.total = 100;
	/// result.migrated = 95;
	/// result.failed = 5;
	///
	/// assert_eq!(result.success_rate(), 95.0);
	/// ```
	pub fn success_rate(&self) -> f64 {
		if self.total == 0 {
			return 0.0;
		}
		(self.migrated as f64 / self.total as f64) * 100.0
	}
}

impl Default for MigrationResult {
	fn default() -> Self {
		Self::new()
	}
}

/// Migration configuration
///
/// # Example
///
/// ```rust
/// use reinhardt_auth::sessions::migration::MigrationConfig;
///
/// let config = MigrationConfig {
///     batch_size: 100,
///     skip_existing: true,
///     verify_migration: false,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct MigrationConfig {
	/// Number of sessions to migrate in one batch
	pub batch_size: usize,
	/// Skip sessions that already exist in target
	pub skip_existing: bool,
	/// Verify each migration by reading back from target
	pub verify_migration: bool,
}

impl Default for MigrationConfig {
	/// Create default migration configuration
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::migration::MigrationConfig;
	///
	/// let config = MigrationConfig::default();
	/// assert_eq!(config.batch_size, 1000);
	/// assert!(!config.skip_existing);
	/// assert!(!config.verify_migration);
	/// ```
	fn default() -> Self {
		Self {
			batch_size: 1000,
			skip_existing: false,
			verify_migration: false,
		}
	}
}

/// Session migrator trait
#[async_trait]
pub trait Migrator {
	/// Run migration
	async fn migrate(&self) -> Result<MigrationResult, SessionError>;

	/// Dry run migration (count only, no actual migration)
	async fn dry_run(&self) -> Result<usize, SessionError>;
}

/// Session migrator for transferring sessions between backends
///
/// # Example
///
/// ```rust,no_run
/// # #[tokio::main]
/// # async fn main() {
/// use reinhardt_auth::sessions::migration::{SessionMigrator, MigrationConfig, Migrator};
/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let source = InMemorySessionBackend::new();
/// let target = InMemorySessionBackend::new();
///
/// let config = MigrationConfig::default();
/// let migrator = SessionMigrator::with_config(source, target, config);
///
/// let result = migrator.migrate().await?;
/// println!("Migration complete: {} sessions migrated", result.migrated);
/// # Ok(())
/// # }
/// # }
/// ```
pub struct SessionMigrator<S: SessionBackend, T: SessionBackend> {
	source: S,
	target: T,
	config: MigrationConfig,
}

impl<S: SessionBackend, T: SessionBackend> SessionMigrator<S, T> {
	/// Create a new session migrator with default configuration
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::migration::SessionMigrator;
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let source = InMemorySessionBackend::new();
	/// let target = InMemorySessionBackend::new();
	/// let migrator = SessionMigrator::new(source, target);
	/// ```
	pub fn new(source: S, target: T) -> Self {
		Self {
			source,
			target,
			config: MigrationConfig::default(),
		}
	}

	/// Create a new session migrator with custom configuration
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::migration::{SessionMigrator, MigrationConfig};
	/// use reinhardt_auth::sessions::backends::InMemorySessionBackend;
	///
	/// let source = InMemorySessionBackend::new();
	/// let target = InMemorySessionBackend::new();
	/// let config = MigrationConfig {
	///     batch_size: 500,
	///     skip_existing: true,
	///     verify_migration: true,
	/// };
	/// let migrator = SessionMigrator::with_config(source, target, config);
	/// ```
	pub fn with_config(source: S, target: T, config: MigrationConfig) -> Self {
		Self {
			source,
			target,
			config,
		}
	}
}

#[async_trait]
impl<S, T> Migrator for SessionMigrator<S, T>
where
	S: SessionBackend + CleanupableBackend,
	T: SessionBackend,
{
	async fn migrate(&self) -> Result<MigrationResult, SessionError> {
		let mut result = MigrationResult::new();

		// Get all session keys from source
		let all_keys = self.source.get_all_keys().await?;
		result.total = all_keys.len();

		// Migrate in batches
		for chunk in all_keys.chunks(self.config.batch_size) {
			for key in chunk {
				// Skip if exists and configured to skip
				if self.config.skip_existing && self.target.exists(key).await? {
					continue;
				}

				// Load from source
				match self
					.source
					.load::<HashMap<String, serde_json::Value>>(key)
					.await
				{
					Ok(Some(data)) => {
						// Save to target
						match self.target.save(key, &data, None).await {
							Ok(_) => {
								// Verify if configured
								if self.config.verify_migration {
									match self
										.target
										.load::<HashMap<String, serde_json::Value>>(key)
										.await
									{
										Ok(Some(_)) => result.migrated += 1,
										Ok(None) => {
											result.failed += 1;
											result.errors.push(format!(
												"Verification failed for key: {}",
												key
											));
										}
										Err(e) => {
											result.failed += 1;
											result.errors.push(format!(
												"Verification error for key {}: {}",
												key, e
											));
										}
									}
								} else {
									result.migrated += 1;
								}
							}
							Err(e) => {
								result.failed += 1;
								result
									.errors
									.push(format!("Failed to save key {}: {}", key, e));
							}
						}
					}
					Ok(None) => {
						// Session doesn't exist in source, skip
					}
					Err(e) => {
						result.failed += 1;
						result
							.errors
							.push(format!("Failed to load key {}: {}", key, e));
					}
				}
			}
		}

		Ok(result)
	}

	async fn dry_run(&self) -> Result<usize, SessionError> {
		let all_keys = self.source.get_all_keys().await?;
		Ok(all_keys.len())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::sessions::backends::InMemorySessionBackend;
	use rstest::rstest;

	#[rstest]
	#[test]
	fn test_migration_result_new() {
		let result = MigrationResult::new();
		assert_eq!(result.total, 0);
		assert_eq!(result.migrated, 0);
		assert_eq!(result.failed, 0);
		assert!(result.errors.is_empty());
	}

	#[rstest]
	#[test]
	fn test_migration_result_is_successful() {
		let mut result = MigrationResult::new();
		result.total = 10;
		result.migrated = 10;
		assert!(result.is_successful());

		result.failed = 1;
		assert!(!result.is_successful());
	}

	#[rstest]
	#[test]
	fn test_migration_result_success_rate() {
		let mut result = MigrationResult::new();
		result.total = 100;
		result.migrated = 95;
		result.failed = 5;

		assert_eq!(result.success_rate(), 95.0);
	}

	#[rstest]
	#[test]
	fn test_migration_result_success_rate_zero_total() {
		let result = MigrationResult::new();
		assert_eq!(result.success_rate(), 0.0);
	}

	#[rstest]
	#[test]
	fn test_migration_config_default() {
		let config = MigrationConfig::default();
		assert_eq!(config.batch_size, 1000);
		assert!(!config.skip_existing);
		assert!(!config.verify_migration);
	}

	#[rstest]
	#[tokio::test]
	async fn test_migrate_empty_source() {
		// Arrange
		let source = InMemorySessionBackend::new();
		let target = InMemorySessionBackend::new();
		let migrator = SessionMigrator::new(source, target);

		// Act
		let result = migrator.migrate().await.unwrap();

		// Assert
		assert_eq!(result.total, 0);
		assert_eq!(result.migrated, 0);
		assert_eq!(result.failed, 0);
		assert!(result.errors.is_empty());
		assert!(result.is_successful());
	}

	#[rstest]
	#[tokio::test]
	async fn test_migrate_multiple_sessions() {
		// Arrange
		let source = InMemorySessionBackend::new();
		let target = InMemorySessionBackend::new();

		let data1: HashMap<String, serde_json::Value> =
			[("user".into(), serde_json::json!("alice"))].into();
		let data2: HashMap<String, serde_json::Value> =
			[("user".into(), serde_json::json!("bob"))].into();
		let data3: HashMap<String, serde_json::Value> =
			[("user".into(), serde_json::json!("carol"))].into();

		source.save("key1", &data1, None).await.unwrap();
		source.save("key2", &data2, None).await.unwrap();
		source.save("key3", &data3, None).await.unwrap();

		let migrator = SessionMigrator::new(source, target.clone());

		// Act
		let result = migrator.migrate().await.unwrap();

		// Assert
		assert_eq!(result.total, 3);
		assert_eq!(result.migrated, 3);
		assert_eq!(result.failed, 0);
		assert!(target.exists("key1").await.unwrap());
		assert!(target.exists("key2").await.unwrap());
		assert!(target.exists("key3").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_migrate_skip_existing_preserves_target() {
		// Arrange
		let source = InMemorySessionBackend::new();
		let target = InMemorySessionBackend::new();

		let source_data: HashMap<String, serde_json::Value> =
			[("origin".into(), serde_json::json!("source_data"))].into();
		let target_data: HashMap<String, serde_json::Value> =
			[("origin".into(), serde_json::json!("target_data"))].into();

		source.save("key1", &source_data, None).await.unwrap();
		target.save("key1", &target_data, None).await.unwrap();

		let config = MigrationConfig {
			batch_size: 1000,
			skip_existing: true,
			verify_migration: false,
		};
		let migrator = SessionMigrator::with_config(source, target.clone(), config);

		// Act
		let result = migrator.migrate().await.unwrap();

		// Assert
		assert_eq!(result.total, 1);
		// The session was skipped, so migrated count should be 0
		assert_eq!(result.migrated, 0);
		let loaded: Option<HashMap<String, serde_json::Value>> = target.load("key1").await.unwrap();
		assert_eq!(loaded.unwrap()["origin"], serde_json::json!("target_data"));
	}

	#[rstest]
	#[tokio::test]
	async fn test_migrate_with_verification_reads_back() {
		// Arrange
		let source = InMemorySessionBackend::new();
		let target = InMemorySessionBackend::new();

		let data1: HashMap<String, serde_json::Value> =
			[("val".into(), serde_json::json!(1))].into();
		let data2: HashMap<String, serde_json::Value> =
			[("val".into(), serde_json::json!(2))].into();

		source.save("sess_a", &data1, None).await.unwrap();
		source.save("sess_b", &data2, None).await.unwrap();

		let config = MigrationConfig {
			batch_size: 1000,
			skip_existing: false,
			verify_migration: true,
		};
		let migrator = SessionMigrator::with_config(source, target.clone(), config);

		// Act
		let result = migrator.migrate().await.unwrap();

		// Assert
		// With verification enabled, migrated count reflects verified sessions
		assert_eq!(result.total, 2);
		assert_eq!(result.migrated, 2);
		assert_eq!(result.failed, 0);
		assert!(result.is_successful());
		// Verify data actually exists in target
		assert!(target.exists("sess_a").await.unwrap());
		assert!(target.exists("sess_b").await.unwrap());
	}

	#[rstest]
	#[tokio::test]
	async fn test_dry_run_returns_count_without_migrating() {
		// Arrange
		let source = InMemorySessionBackend::new();
		let target = InMemorySessionBackend::new();

		let data: HashMap<String, serde_json::Value> =
			[("key".into(), serde_json::json!("value"))].into();

		source.save("dry1", &data, None).await.unwrap();
		source.save("dry2", &data, None).await.unwrap();
		source.save("dry3", &data, None).await.unwrap();

		let migrator = SessionMigrator::new(source, target.clone());

		// Act
		let count = migrator.dry_run().await.unwrap();

		// Assert
		assert_eq!(count, 3);
		// Target must remain empty after dry run
		assert!(!target.exists("dry1").await.unwrap());
		assert!(!target.exists("dry2").await.unwrap());
		assert!(!target.exists("dry3").await.unwrap());
	}
}
