//! Local Migration Registry
//!
//! Test-specific registry using dynamic runtime registration.
//! Provides complete isolation for unit tests without linkme's distributed_slice.

use super::traits::MigrationRegistry;
use crate::migrations::Migration;
use std::collections::HashSet;
use std::sync::RwLock;

/// Local migration registry for testing
///
/// This registry stores migrations entirely in memory and does not use
/// linkme's distributed_slice. This provides complete isolation between
/// test cases and avoids the "duplicate distributed_slice" errors.
///
/// # Examples
///
/// ```rust,no_run
/// use reinhardt_db::migrations::registry::{LocalRegistry, MigrationRegistry};
/// use reinhardt_db::migrations::Migration;
///
/// let migration = Migration::new("0001_initial", "test");
/// let registry = LocalRegistry::new();
/// registry.register(migration).unwrap();
/// let migrations = registry.all_migrations();
/// ```
pub struct LocalRegistry {
	migrations: RwLock<Vec<Migration>>,
}

impl LocalRegistry {
	/// Creates a new empty local registry
	pub fn new() -> Self {
		Self {
			migrations: RwLock::new(Vec::new()),
		}
	}
}

impl Default for LocalRegistry {
	fn default() -> Self {
		Self::new()
	}
}

impl MigrationRegistry for LocalRegistry {
	fn all_migrations(&self) -> Vec<Migration> {
		self.migrations
			.read()
			.map(|m| m.clone())
			.unwrap_or_default()
	}

	fn migrations_for_app(&self, app_label: &str) -> Vec<Migration> {
		self.migrations
			.read()
			.map(|migrations| {
				migrations
					.iter()
					.filter(|m| m.app_label == app_label)
					.cloned()
					.collect()
			})
			.unwrap_or_default()
	}

	fn registered_app_labels(&self) -> Vec<String> {
		self.migrations
			.read()
			.map(|migrations| {
				let mut labels: Vec<String> = migrations
					.iter()
					.map(|m| m.app_label.to_string())
					.collect::<HashSet<_>>()
					.into_iter()
					.collect();
				labels.sort();
				labels
			})
			.unwrap_or_default()
	}

	fn register(&self, migration: Migration) -> Result<(), String> {
		self.migrations
			.write()
			.map_err(|e| format!("Failed to acquire write lock: {}", e))?
			.push(migration);
		Ok(())
	}

	fn clear(&self) {
		if let Ok(mut migrations) = self.migrations.write() {
			migrations.clear();
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_new_registry_is_empty() {
		let registry = LocalRegistry::new();
		assert!(registry.all_migrations().is_empty());
		assert!(registry.registered_app_labels().is_empty());
	}

	#[test]
	fn test_register_and_retrieve() {
		let registry = LocalRegistry::new();

		let migration = Migration {
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
		};

		registry.register(migration.clone()).unwrap();

		let all = registry.all_migrations();
		assert_eq!(all.len(), 1);
		assert_eq!(all[0].app_label, "polls");
		assert_eq!(all[0].name, "0001_initial");
	}

	#[test]
	fn test_migrations_for_app_filtering() {
		let registry = LocalRegistry::new();

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

		let polls_migrations = registry.migrations_for_app("polls");
		assert_eq!(polls_migrations.len(), 2);
		assert!(polls_migrations.iter().all(|m| m.app_label == "polls"));

		let users_migrations = registry.migrations_for_app("users");
		assert_eq!(users_migrations.len(), 1);
		assert_eq!(users_migrations[0].app_label, "users");
	}

	#[test]
	fn test_registered_app_labels() {
		let registry = LocalRegistry::new();

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

		let labels = registry.registered_app_labels();
		assert_eq!(labels.len(), 2);
		assert_eq!(labels, vec!["polls", "users"]);
	}

	#[test]
	fn test_clear() {
		let registry = LocalRegistry::new();

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

		assert!(!registry.all_migrations().is_empty());

		registry.clear();
		assert!(registry.all_migrations().is_empty());
		assert!(registry.registered_app_labels().is_empty());
	}

	#[test]
	fn test_multiple_registries_isolated() {
		let registry1 = LocalRegistry::new();
		let registry2 = LocalRegistry::new();

		registry1
			.register(Migration {
				app_label: "app1".to_string(),
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

		registry2
			.register(Migration {
				app_label: "app2".to_string(),
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

		// Each registry should only contain its own migrations
		assert_eq!(registry1.all_migrations().len(), 1);
		assert_eq!(registry1.all_migrations()[0].app_label, "app1");

		assert_eq!(registry2.all_migrations().len(), 1);
		assert_eq!(registry2.all_migrations()[0].app_label, "app2");
	}

	#[test]
	fn test_migrations_for_nonexistent_app_returns_empty() {
		let registry = LocalRegistry::new();

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

		// Query for non-existent app should return empty
		let migrations = registry.migrations_for_app("nonexistent_app_12345");
		assert!(
			migrations.is_empty(),
			"Expected empty result for non-existent app"
		);
	}
}
