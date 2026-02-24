//! Composite migration source
//!
//! Combines multiple migration sources into a single unified source.

use super::{Migration, MigrationError, MigrationSource, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Composite migration source that combines multiple sources
///
/// This source allows combining multiple migration sources (e.g., RegistrySource
/// and FilesystemSource) into a single source. Migrations are merged, with
/// later sources taking precedence over earlier ones in case of conflicts.
///
/// # Example
///
/// ```rust,no_run
/// use std::sync::Arc;
/// use reinhardt_db::migrations::{RegistrySource, FilesystemSource, CompositeSource};
/// let registry = Arc::new(RegistrySource::new());
/// let filesystem = Arc::new(FilesystemSource::new("./migrations"));
///
/// let composite = CompositeSource::new()
///     .add_source(registry)
///     .add_source(filesystem);
/// ```
pub struct CompositeSource {
	/// List of sources in order of precedence (first = lowest, last = highest)
	sources: Vec<Arc<dyn MigrationSource>>,
}

impl CompositeSource {
	/// Create a new empty CompositeSource
	pub fn new() -> Self {
		Self {
			sources: Vec::new(),
		}
	}

	/// Add a migration source
	///
	/// Sources are queried in the order they are added. Later sources
	/// take precedence over earlier ones when merging migrations.
	pub fn add_source(mut self, source: Arc<dyn MigrationSource>) -> Self {
		self.sources.push(source);
		self
	}

	/// Merge migrations from all sources
	///
	/// Migrations with the same (app_label, name) are deduplicated,
	/// with migrations from later sources taking precedence.
	///
	/// **Warning**: If migrations with the same key but different operations
	/// are detected, a warning is printed to stderr. This helps identify
	/// situations where `makemigrations` was run multiple times or where
	/// migration history is inconsistent.
	async fn merge_migrations(&self) -> Result<Vec<Migration>> {
		let mut merged: HashMap<(String, String), Migration> = HashMap::new();
		let mut conflicts: Vec<(String, String)> = Vec::new(); // (app_label, name)

		// Collect migrations from all sources
		for source in &self.sources {
			let migrations = source.all_migrations().await?;

			for migration in migrations {
				let key = (migration.app_label.to_string(), migration.name.to_string());

				// Check if migration with this key already exists
				if let Some(existing) = merged.get(&key) {
					// Compare operations to detect content differences
					if existing.operations != migration.operations {
						// Conflict detected: same key, different operations
						conflicts.push(key.clone());
					}
				}

				// Later sources take precedence
				merged.insert(key, migration);
			}
		}

		// Print warnings for detected conflicts
		if !conflicts.is_empty() {
			eprintln!("⚠️  Migration merge conflicts detected:");
			for (app, name) in &conflicts {
				eprintln!(
					"  - {}.{} has different operations from multiple sources",
					app, name
				);
			}
			eprintln!(
				"This may indicate that makemigrations was run multiple times \
				or that the migration history is inconsistent."
			);
		}

		// Convert to Vec
		Ok(merged.into_values().collect())
	}
}

impl Default for CompositeSource {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl MigrationSource for CompositeSource {
	async fn all_migrations(&self) -> Result<Vec<Migration>> {
		self.merge_migrations().await
	}

	async fn migrations_for_app(&self, app_label: &str) -> Result<Vec<Migration>> {
		let all = self.merge_migrations().await?;
		Ok(all
			.into_iter()
			.filter(|m| m.app_label == app_label)
			.collect())
	}

	async fn get_migration(&self, app_label: &str, name: &str) -> Result<Migration> {
		let migrations = self.migrations_for_app(app_label).await?;
		migrations
			.into_iter()
			.find(|m| m.name == name)
			.ok_or_else(|| MigrationError::NotFound(format!("{}.{}", app_label, name)))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::migrations::Migration;

	/// Test source that returns predefined migrations
	struct TestSource {
		migrations: Vec<Migration>,
	}

	#[async_trait]
	impl MigrationSource for TestSource {
		async fn all_migrations(&self) -> Result<Vec<Migration>> {
			Ok(self.migrations.clone())
		}
	}

	fn create_test_migration(app_label: &str, name: &str) -> Migration {
		Migration {
			app_label: app_label.to_string(),
			name: name.to_string(),
			operations: vec![],
			dependencies: vec![],
			atomic: true,
			initial: None,
			replaces: vec![],
			state_only: false,
			database_only: false,
			swappable_dependencies: vec![],
			optional_dependencies: vec![],
		}
	}

	#[tokio::test]
	async fn test_composite_source_new() {
		let composite = CompositeSource::new();
		assert_eq!(composite.sources.len(), 0);
	}

	#[tokio::test]
	async fn test_composite_source_add_source() {
		let source1 = Arc::new(TestSource {
			migrations: vec![create_test_migration("polls", "0001_initial")],
		});

		let composite = CompositeSource::new().add_source(source1);
		assert_eq!(composite.sources.len(), 1);
	}

	#[tokio::test]
	async fn test_composite_source_merge_migrations() {
		let source1 = Arc::new(TestSource {
			migrations: vec![
				create_test_migration("polls", "0001_initial"),
				create_test_migration("polls", "0002_add_field"),
			],
		});

		let source2 = Arc::new(TestSource {
			migrations: vec![
				create_test_migration("users", "0001_initial"),
				create_test_migration("users", "0002_add_email"),
			],
		});

		let composite = CompositeSource::new()
			.add_source(source1)
			.add_source(source2);

		let migrations = composite.all_migrations().await.unwrap();
		assert_eq!(migrations.len(), 4);
	}

	#[tokio::test]
	async fn test_composite_source_deduplicate() {
		// Source 1 has polls.0001_initial
		let source1 = Arc::new(TestSource {
			migrations: vec![create_test_migration("polls", "0001_initial")],
		});

		// Source 2 also has polls.0001_initial (should override source1)
		let source2 = Arc::new(TestSource {
			migrations: vec![{
				let mut m = create_test_migration("polls", "0001_initial");
				m.atomic = false; // Different value to verify override
				m
			}],
		});

		let composite = CompositeSource::new()
			.add_source(source1)
			.add_source(source2);

		let migrations = composite.all_migrations().await.unwrap();
		assert_eq!(migrations.len(), 1);

		// Verify source2's version is used (atomic = false)
		let migration = &migrations[0];
		assert_eq!(migration.app_label, "polls");
		assert_eq!(migration.name, "0001_initial");
		assert!(!migration.atomic); // source2's value
	}

	#[tokio::test]
	async fn test_composite_source_migrations_for_app() {
		let source1 = Arc::new(TestSource {
			migrations: vec![
				create_test_migration("polls", "0001_initial"),
				create_test_migration("polls", "0002_add_field"),
			],
		});

		let source2 = Arc::new(TestSource {
			migrations: vec![
				create_test_migration("users", "0001_initial"),
				create_test_migration("polls", "0003_alter_field"),
			],
		});

		let composite = CompositeSource::new()
			.add_source(source1)
			.add_source(source2);

		let polls_migrations = composite.migrations_for_app("polls").await.unwrap();
		assert_eq!(polls_migrations.len(), 3);
		assert!(polls_migrations.iter().all(|m| m.app_label == "polls"));
	}

	#[tokio::test]
	async fn test_composite_source_get_migration() {
		let source1 = Arc::new(TestSource {
			migrations: vec![create_test_migration("polls", "0001_initial")],
		});

		let source2 = Arc::new(TestSource {
			migrations: vec![create_test_migration("users", "0001_initial")],
		});

		let composite = CompositeSource::new()
			.add_source(source1)
			.add_source(source2);

		let migration = composite
			.get_migration("polls", "0001_initial")
			.await
			.unwrap();
		assert_eq!(migration.app_label, "polls");
		assert_eq!(migration.name, "0001_initial");
	}

	#[tokio::test]
	async fn test_composite_source_get_migration_not_found() {
		let source1 = Arc::new(TestSource {
			migrations: vec![create_test_migration("polls", "0001_initial")],
		});

		let composite = CompositeSource::new().add_source(source1);

		let result = composite.get_migration("polls", "0002_nonexistent").await;
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), MigrationError::NotFound(_)));
	}

	#[tokio::test]
	async fn test_composite_source_empty() {
		let composite = CompositeSource::new();
		let migrations = composite.all_migrations().await.unwrap();
		assert_eq!(migrations.len(), 0);
	}
}
