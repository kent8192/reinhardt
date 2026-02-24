//! Registry-based migration source
//!
//! Loads migrations from the compile-time global registry using `linkme`.

use super::{Migration, MigrationError, MigrationSource, Result};
use crate::migrations::registry::{MigrationRegistry, global_registry};
use async_trait::async_trait;

/// Migration source that loads from the global registry
///
/// Uses `linkme` to collect migrations registered at compile-time via
/// the `collect_migrations!` macro.
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_db::migrations::{RegistrySource, MigrationSource};
/// #[tokio::test]
/// async fn test_registry_source() {
///     let source = RegistrySource::new();
///     let migrations = source.all_migrations().await.unwrap();
///     // migrations contains all compile-time registered migrations
/// }
/// ```
pub struct RegistrySource;

impl RegistrySource {
	/// Create a new RegistrySource
	pub fn new() -> Self {
		Self
	}
}

impl Default for RegistrySource {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl MigrationSource for RegistrySource {
	async fn all_migrations(&self) -> Result<Vec<Migration>> {
		let registry = global_registry();
		Ok(registry.all_migrations())
	}

	async fn migrations_for_app(&self, app_label: &str) -> Result<Vec<Migration>> {
		let registry = global_registry();
		Ok(registry.migrations_for_app(app_label))
	}

	async fn get_migration(&self, app_label: &str, name: &str) -> Result<Migration> {
		let registry = global_registry();
		let migrations = registry.migrations_for_app(app_label);
		migrations
			.into_iter()
			.find(|m| m.name == name)
			.ok_or_else(|| MigrationError::NotFound(format!("{}.{}", app_label, name)))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::migrations::MigrationError;
	use serial_test::serial;

	#[tokio::test]
	#[serial(global_registry)]
	async fn test_registry_source_new() {
		let registry = global_registry();
		registry.clear();

		let source = RegistrySource::new();

		// Initially empty (we cleared it)
		let migrations = source.all_migrations().await.unwrap();
		assert_eq!(migrations.len(), 0);

		registry.clear();
	}

	#[tokio::test]
	#[serial(global_registry)]
	async fn test_registry_source_all_migrations() {
		let registry = global_registry();
		registry.clear();

		registry
			.register(Migration {
				app_label: "polls".to_string(),
				name: "0001_initial".to_string(),
				operations: vec![],
				dependencies: vec![],
				atomic: true,
				initial: None,
				replaces: vec![],
				state_only: false,
				database_only: false,
				swappable_dependencies: vec![],
				optional_dependencies: vec![],
			})
			.unwrap();

		registry
			.register(Migration {
				app_label: "users".to_string(),
				name: "0001_initial".to_string(),
				operations: vec![],
				dependencies: vec![],
				atomic: true,
				initial: None,
				replaces: vec![],
				state_only: false,
				database_only: false,
				swappable_dependencies: vec![],
				optional_dependencies: vec![],
			})
			.unwrap();

		let source = RegistrySource::new();
		let migrations = source.all_migrations().await.unwrap();

		assert_eq!(migrations.len(), 2);

		registry.clear();
	}

	#[tokio::test]
	#[serial(global_registry)]
	async fn test_registry_source_migrations_for_app() {
		let registry = global_registry();
		registry.clear();

		registry
			.register(Migration {
				app_label: "polls".to_string(),
				name: "0001_initial".to_string(),
				operations: vec![],
				dependencies: vec![],
				atomic: true,
				initial: None,
				replaces: vec![],
				state_only: false,
				database_only: false,
				swappable_dependencies: vec![],
				optional_dependencies: vec![],
			})
			.unwrap();

		registry
			.register(Migration {
				app_label: "polls".to_string(),
				name: "0002_add_field".to_string(),
				operations: vec![],
				dependencies: vec![],
				atomic: true,
				initial: None,
				replaces: vec![],
				state_only: false,
				database_only: false,
				swappable_dependencies: vec![],
				optional_dependencies: vec![],
			})
			.unwrap();

		registry
			.register(Migration {
				app_label: "users".to_string(),
				name: "0001_initial".to_string(),
				operations: vec![],
				dependencies: vec![],
				atomic: true,
				initial: None,
				replaces: vec![],
				state_only: false,
				database_only: false,
				swappable_dependencies: vec![],
				optional_dependencies: vec![],
			})
			.unwrap();

		let source = RegistrySource::new();
		let polls_migrations = source.migrations_for_app("polls").await.unwrap();

		assert_eq!(polls_migrations.len(), 2);
		assert!(polls_migrations.iter().all(|m| m.app_label == "polls"));

		let users_migrations = source.migrations_for_app("users").await.unwrap();
		assert_eq!(users_migrations.len(), 1);

		registry.clear();
	}

	#[tokio::test]
	#[serial(global_registry)]
	async fn test_registry_source_get_migration() {
		let registry = global_registry();
		registry.clear();

		registry
			.register(Migration {
				app_label: "polls".to_string(),
				name: "0001_initial".to_string(),
				operations: vec![],
				dependencies: vec![],
				atomic: true,
				initial: None,
				replaces: vec![],
				state_only: false,
				database_only: false,
				swappable_dependencies: vec![],
				optional_dependencies: vec![],
			})
			.unwrap();

		let source = RegistrySource::new();
		let migration = source.get_migration("polls", "0001_initial").await.unwrap();

		assert_eq!(migration.app_label, "polls");
		assert_eq!(migration.name, "0001_initial");

		registry.clear();
	}

	#[tokio::test]
	#[serial(global_registry)]
	async fn test_registry_source_get_migration_not_found() {
		let registry = global_registry();
		registry.clear();

		let source = RegistrySource::new();
		let result = source.get_migration("polls", "0001_nonexistent").await;

		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), MigrationError::NotFound(_)));

		registry.clear();
	}
}
