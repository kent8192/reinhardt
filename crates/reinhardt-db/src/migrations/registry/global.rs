//! Global Migration Registry
//!
//! Production registry using linkme's distributed_slice for compile-time migration collection.
//! Supports runtime registration for dynamic scenarios.

use super::traits::MigrationRegistry;
use crate::migrations::Migration;
use linkme::distributed_slice;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::sync::RwLock;

/// Type for migration providers collected at compile-time
pub type MigrationProvider = fn() -> Vec<Migration>;

/// Distributed slice for collecting migration providers at compile-time
#[distributed_slice]
pub static MIGRATION_PROVIDERS: [MigrationProvider];

/// Global migration registry using linkme's distributed_slice
pub struct GlobalRegistry {
	/// Runtime-registered migrations (for testing and dynamic scenarios)
	runtime_migrations: RwLock<Vec<Migration>>,
}

impl Default for GlobalRegistry {
	fn default() -> Self {
		Self::new()
	}
}

impl GlobalRegistry {
	/// Creates a new global registry instance
	pub const fn new() -> Self {
		Self {
			runtime_migrations: RwLock::new(Vec::new()),
		}
	}

	/// Returns a static reference to the global registry instance
	pub fn instance() -> &'static Self {
		static INSTANCE: Lazy<GlobalRegistry> = Lazy::new(GlobalRegistry::new);
		&INSTANCE
	}

	/// Collects migrations from linkme's distributed_slice
	fn collect_compile_time_migrations(&self) -> Vec<Migration> {
		#[cfg(not(test))]
		{
			let mut migrations = Vec::new();
			for provider in MIGRATION_PROVIDERS {
				migrations.extend(provider());
			}
			migrations
		}
		#[cfg(test)]
		{
			// In test mode, do not access MIGRATION_PROVIDERS to avoid
			// "duplicate distributed_slice" errors. Tests should use
			// runtime registration via register() method instead.
			Vec::new()
		}
	}

	/// Merges compile-time and runtime migrations
	fn merged_migrations(&self) -> Vec<Migration> {
		let mut all_migrations = self.collect_compile_time_migrations();

		// Add runtime-registered migrations
		if let Ok(runtime) = self.runtime_migrations.read() {
			all_migrations.extend(runtime.clone());
		}

		all_migrations
	}
}

impl MigrationRegistry for GlobalRegistry {
	fn all_migrations(&self) -> Vec<Migration> {
		self.merged_migrations()
	}

	fn migrations_for_app(&self, app_label: &str) -> Vec<Migration> {
		self.merged_migrations()
			.into_iter()
			.filter(|m| m.app_label == app_label)
			.collect()
	}

	fn registered_app_labels(&self) -> Vec<String> {
		let migrations = self.merged_migrations();
		let mut labels: Vec<String> = migrations
			.iter()
			.map(|m| m.app_label.to_string())
			.collect::<HashSet<_>>()
			.into_iter()
			.collect();
		labels.sort();
		labels
	}

	fn register(&self, migration: Migration) -> Result<(), String> {
		self.runtime_migrations
			.write()
			.map_err(|e| format!("Failed to acquire write lock: {}", e))?
			.push(migration);
		Ok(())
	}

	fn clear(&self) {
		if let Ok(mut runtime) = self.runtime_migrations.write() {
			runtime.clear();
		}
	}
}

/// Returns a reference to the global registry instance
///
/// This is the primary entry point for production code.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_db::migrations::registry::{global_registry, MigrationRegistry};
/// let registry = global_registry();
/// let all_migrations = registry.all_migrations();
/// ```
pub fn global_registry() -> &'static GlobalRegistry {
	GlobalRegistry::instance()
}

#[cfg(test)]
mod tests {
	use super::*;
	use serial_test::serial;

	#[test]
	#[serial(global_registry)]
	fn test_global_registry_singleton() {
		let registry1 = GlobalRegistry::instance();
		let registry2 = GlobalRegistry::instance();

		// Verify they point to the same instance
		assert!(std::ptr::eq(registry1, registry2));
	}

	#[test]
	#[serial(global_registry)]
	fn test_runtime_registration() {
		let registry = GlobalRegistry::instance();

		// Clear before test
		registry.clear();

		let migration = Migration {
			app_label: "test_app".to_string(),
			name: "0001_initial".to_string(),
			operations: vec![],
			dependencies: vec![],
			replaces: vec![],
			atomic: true,
			initial: None,
			state_only: false,
			database_only: false,
			swappable_dependencies: vec![],
			optional_dependencies: vec![],
		};

		// Register migration
		registry.register(migration.clone()).unwrap();

		// Verify registration
		let migrations = registry.all_migrations();
		assert!(
			migrations
				.iter()
				.any(|m| m.app_label == "test_app" && m.name == "0001_initial"),
			"Runtime-registered migration should be present"
		);

		// Cleanup
		registry.clear();
	}

	#[test]
	#[serial(global_registry)]
	fn test_clear_runtime_migrations() {
		let registry = GlobalRegistry::instance();

		// Clear before test
		registry.clear();

		let migration = Migration {
			app_label: "test_app".to_string(),
			name: "0001_initial".to_string(),
			operations: vec![],
			dependencies: vec![],
			replaces: vec![],
			atomic: true,
			initial: None,
			state_only: false,
			database_only: false,
			swappable_dependencies: vec![],
			optional_dependencies: vec![],
		};

		registry.register(migration).unwrap();
		assert!(
			!registry.all_migrations().is_empty(),
			"Registry should have migrations before clear"
		);

		// Clear and verify
		registry.clear();
		let runtime_only = registry.runtime_migrations.read().unwrap().clone();
		assert!(
			runtime_only.is_empty(),
			"Runtime migrations should be empty after clear"
		);
	}

	#[test]
	#[serial(global_registry)]
	fn test_migrations_for_app_filtering() {
		let registry = GlobalRegistry::instance();
		registry.clear();

		// Register migrations for different apps
		registry
			.register(Migration {
				app_label: "polls".to_string(),
				name: "0001_initial".to_string(),
				operations: vec![],
				dependencies: vec![],
				replaces: vec![],
				atomic: true,
				initial: None,
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
				replaces: vec![],
				atomic: true,
				initial: None,
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
				replaces: vec![],
				atomic: true,
				initial: None,
				state_only: false,
				database_only: false,
				swappable_dependencies: vec![],
				optional_dependencies: vec![],
			})
			.unwrap();

		// Test filtering
		let polls_migrations = registry.migrations_for_app("polls");
		assert_eq!(polls_migrations.len(), 2, "Should have 2 polls migrations");
		assert!(
			polls_migrations.iter().all(|m| m.app_label == "polls"),
			"All migrations should be from polls app"
		);

		let users_migrations = registry.migrations_for_app("users");
		assert_eq!(users_migrations.len(), 1, "Should have 1 users migration");

		// Cleanup
		registry.clear();
	}

	#[test]
	#[serial(global_registry)]
	fn test_migrations_for_nonexistent_app_returns_empty() {
		let registry = GlobalRegistry::instance();
		registry.clear();

		// Register some migrations
		registry
			.register(Migration {
				app_label: "polls".to_string(),
				name: "0001_initial".to_string(),
				operations: vec![],
				dependencies: vec![],
				replaces: vec![],
				atomic: true,
				initial: None,
				state_only: false,
				database_only: false,
				swappable_dependencies: vec![],
				optional_dependencies: vec![],
			})
			.unwrap();

		// Query for non-existent app
		let migrations = registry.migrations_for_app("nonexistent_app_12345");
		assert!(
			migrations.is_empty(),
			"Expected empty result for non-existent app"
		);

		// Cleanup
		registry.clear();
	}

	#[test]
	#[serial(global_registry)]
	fn test_registered_app_labels_no_duplicates() {
		let registry = GlobalRegistry::instance();
		registry.clear();

		// Register multiple migrations for same apps
		registry
			.register(Migration {
				app_label: "polls".to_string(),
				name: "0001_initial".to_string(),
				operations: vec![],
				dependencies: vec![],
				replaces: vec![],
				atomic: true,
				initial: None,
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
				replaces: vec![],
				atomic: true,
				initial: None,
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
				replaces: vec![],
				atomic: true,
				initial: None,
				state_only: false,
				database_only: false,
				swappable_dependencies: vec![],
				optional_dependencies: vec![],
			})
			.unwrap();

		// Get labels
		let labels = registry.registered_app_labels();

		// Should be sorted and deduplicated
		assert_eq!(labels, vec!["polls", "users"]);
		assert_eq!(labels.len(), 2, "Should have exactly 2 unique app labels");

		// Cleanup
		registry.clear();
	}
}
