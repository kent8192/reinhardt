//! Migration squashing
//!
//! This module provides functionality to combine multiple migrations into a single migration,
//! inspired by Django's `squashmigrations` command.
//!
//! # Example
//!
//! ```rust
//! use reinhardt_db::migrations::squash::{MigrationSquasher, SquashOptions};
//! use reinhardt_db::migrations::Migration;
//!
//! // Create migrations to squash
//! let migration1 = Migration::new("0001_initial", "myapp");
//! let migration2 = Migration::new("0002_add_field", "myapp")
//!     .add_dependency("myapp", "0001_initial");
//! let migration3 = Migration::new("0003_alter_field", "myapp")
//!     .add_dependency("myapp", "0002_add_field");
//!
//! let migrations = vec![migration1, migration2, migration3];
//!
//! // Squash them into a single migration
//! let squasher = MigrationSquasher::new();
//! let options = SquashOptions::default();
//! let squashed = squasher.squash(&migrations, "0001_squashed_0003", options).unwrap();
//!
//! assert_eq!(squashed.name, "0001_squashed_0003");
//! assert_eq!(squashed.replaces.len(), 3);
//! ```

use super::{Migration, MigrationError, Operation, Result};
use std::collections::HashSet;

/// Options for migration squashing
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::squash::SquashOptions;
///
/// let options = SquashOptions::default();
/// assert!(options.optimize);
/// assert!(!options.no_optimize);
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct SquashOptions {
	/// Enable operation optimization (remove redundant operations)
	pub optimize: bool,
	/// Disable optimization (keep all operations)
	pub no_optimize: bool,
}

impl Default for SquashOptions {
	fn default() -> Self {
		Self {
			optimize: true,
			no_optimize: false,
		}
	}
}

/// Migration squasher
///
/// Combines multiple sequential migrations into a single migration.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::migrations::squash::MigrationSquasher;
/// use reinhardt_db::migrations::Migration;
///
/// let squasher = MigrationSquasher::new();
/// let migrations = vec![Migration::new("0001_initial", "myapp")];
/// let squashed = squasher.squash(&migrations, "0001_squashed", Default::default()).unwrap();
/// ```
pub struct MigrationSquasher {
	_private: (),
}

impl MigrationSquasher {
	/// Create a new migration squasher
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::squash::MigrationSquasher;
	///
	/// let squasher = MigrationSquasher::new();
	/// ```
	pub fn new() -> Self {
		Self { _private: () }
	}

	/// Squash multiple migrations into one
	///
	/// # Arguments
	///
	/// * `migrations` - List of migrations to squash (must be sequential)
	/// * `squashed_name` - Name for the squashed migration
	/// * `options` - Squashing options
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::squash::{MigrationSquasher, SquashOptions};
	/// use reinhardt_db::migrations::Migration;
	///
	/// let migration1 = Migration::new("0001_initial", "myapp");
	/// let migration2 = Migration::new("0002_add_field", "myapp");
	/// let migrations = vec![migration1, migration2];
	///
	/// let squasher = MigrationSquasher::new();
	/// let squashed = squasher.squash(&migrations, "0001_squashed_0002", SquashOptions::default()).unwrap();
	///
	/// assert_eq!(squashed.name, "0001_squashed_0002");
	/// ```
	pub fn squash(
		&self,
		migrations: &[Migration],
		squashed_name: impl Into<String>,
		options: SquashOptions,
	) -> Result<Migration> {
		if migrations.is_empty() {
			return Err(MigrationError::InvalidMigration(
				"Cannot squash empty migration list".to_string(),
			));
		}

		// Validate all migrations belong to the same app
		let app_label = &migrations[0].app_label;
		if !migrations.iter().all(|m| m.app_label == *app_label) {
			return Err(MigrationError::InvalidMigration(
				"All migrations must belong to the same app".to_string(),
			));
		}

		// Collect all operations
		let mut operations = Vec::new();
		for migration in migrations {
			operations.extend(migration.operations.clone());
		}

		// Optimize operations if enabled
		if options.optimize && !options.no_optimize {
			operations = self.optimize_operations(operations);
		}

		// Create squashed migration
		let mut squashed = Migration::new(squashed_name, app_label.clone());
		squashed.operations = operations;

		// Record which migrations this replaces
		for migration in migrations {
			squashed
				.replaces
				.push((migration.app_label.clone(), migration.name.clone()));
		}

		// Collect dependencies from first migration (external dependencies only)
		if let Some(first) = migrations.first() {
			for (dep_app, dep_name) in &first.dependencies {
				// Only include dependencies outside the squashed range
				if *dep_app != *app_label
					|| !migrations
						.iter()
						.any(|m| m.app_label == *dep_app && m.name == *dep_name)
				{
					squashed
						.dependencies
						.push((dep_app.clone(), dep_name.clone()));
				}
			}
		}

		Ok(squashed)
	}

	/// Optimize operations by removing redundant ones
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::migrations::squash::MigrationSquasher;
	/// use reinhardt_db::migrations::{Operation, ColumnDefinition, FieldType};
	///
	/// let squasher = MigrationSquasher::new();
	///
	/// // Create table then drop it - both can be removed
	/// let ops = vec![
	///     Operation::CreateTable {
	///         name: "temp".to_string(),
	///         columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
	///         constraints: vec![],
	///         without_rowid: None,
	///         interleave_in_parent: None,
	///         partition: None,
	///     },
	///     Operation::DropTable {
	///         name: "temp".to_string(),
	///     },
	/// ];
	///
	/// let optimized = squasher.optimize_operations(ops);
	/// assert_eq!(optimized.len(), 0);
	/// ```
	pub fn optimize_operations(&self, operations: Vec<Operation>) -> Vec<Operation> {
		let mut optimized = Vec::new();
		let mut created_tables = HashSet::new();
		let mut dropped_tables = HashSet::new();

		for operation in operations {
			let should_push = match &operation {
				Operation::CreateTable { name, .. } => {
					// Skip if this table will be dropped later
					if !dropped_tables.contains(name) {
						created_tables.insert(name.clone());
						true
					} else {
						false
					}
				}
				Operation::DropTable { name } => {
					// If table was just created, remove both operations
					if created_tables.contains(name) {
						optimized.retain(
							|op| !matches!(op, Operation::CreateTable { name: table_name, .. } if table_name == name),
						);
						created_tables.remove(name);
						false
					} else {
						dropped_tables.insert(name.clone());
						true
					}
				}
				Operation::AddColumn { table, .. } => {
					// Skip if table was dropped
					!dropped_tables.contains(table)
				}
				Operation::DropColumn { table, column } => {
					// Remove corresponding AddColumn if exists
					let had_add = optimized.iter().any(|op| {
						matches!(op, Operation::AddColumn { table: t, column: c, .. } if t == table && c.name == *column)
					});

					if had_add {
						optimized.retain(|op| {
							!matches!(op, Operation::AddColumn { table: t, column: c, .. } if t == table && c.name == *column)
						});
						false
					} else {
						!dropped_tables.contains(table)
					}
				}
				Operation::AlterColumn { table, .. } => {
					// Skip if table was dropped
					!dropped_tables.contains(table)
				}
				Operation::RenameTable { old_name, .. } => {
					// Skip if table was dropped
					!dropped_tables.contains(old_name)
				}
				Operation::RenameColumn { table, .. } => {
					// Skip if table was dropped
					!dropped_tables.contains(table)
				}
				_ => true,
			};

			if should_push {
				optimized.push(operation);
			}
		}

		optimized
	}
}

impl Default for MigrationSquasher {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::migrations::{ColumnDefinition, FieldType};

	#[test]
	fn test_squash_basic() {
		let migration1 = Migration::new("0001_initial", "myapp");
		let migration2 =
			Migration::new("0002_add_field", "myapp").add_dependency("myapp", "0001_initial");

		let migrations = vec![migration1, migration2];

		let squasher = MigrationSquasher::new();
		let squashed = squasher
			.squash(&migrations, "0001_squashed_0002", SquashOptions::default())
			.unwrap();

		assert_eq!(squashed.name, "0001_squashed_0002");
		assert_eq!(squashed.app_label, "myapp");
		assert_eq!(squashed.replaces.len(), 2);
	}

	#[test]
	fn test_squash_empty_migrations() {
		let squasher = MigrationSquasher::new();
		let result = squasher.squash(&[], "squashed", SquashOptions::default());

		assert!(result.is_err());
	}

	#[test]
	fn test_squash_different_apps() {
		let migration1 = Migration::new("0001_initial", "app1");
		let migration2 = Migration::new("0002_add_field", "app2");

		let migrations = vec![migration1, migration2];

		let squasher = MigrationSquasher::new();
		let result = squasher.squash(&migrations, "squashed", SquashOptions::default());

		assert!(result.is_err());
	}

	#[test]
	fn test_optimize_create_drop_table() {
		let squasher = MigrationSquasher::new();

		let ops = vec![
			Operation::CreateTable {
				name: "temp".to_string(),
				columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
				constraints: vec![],
				without_rowid: None,
				partition: None,
				interleave_in_parent: None,
			},
			Operation::DropTable {
				name: "temp".to_string(),
			},
		];

		let optimized = squasher.optimize_operations(ops);
		assert_eq!(optimized.len(), 0);
	}

	#[test]
	fn test_optimize_add_drop_column() {
		let squasher = MigrationSquasher::new();

		let ops = vec![
			Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition::new("temp_field", FieldType::VarChar(100)),
				mysql_options: None,
			},
			Operation::DropColumn {
				table: "users".to_string(),
				column: "temp_field".to_string(),
			},
		];

		let optimized = squasher.optimize_operations(ops);
		assert_eq!(optimized.len(), 0);
	}

	#[test]
	fn test_optimize_no_optimization() {
		let squasher = MigrationSquasher::new();

		let ops = vec![
			Operation::CreateTable {
				name: "users".to_string(),
				columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
				constraints: vec![],
				without_rowid: None,
				partition: None,
				interleave_in_parent: None,
			},
			Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition::new("name", FieldType::VarChar(100)),
				mysql_options: None,
			},
		];

		let optimized = squasher.optimize_operations(ops.clone());
		assert_eq!(optimized.len(), ops.len());
	}

	#[test]
	fn test_squash_with_operations() {
		let migration1 =
			Migration::new("0001_initial", "myapp").add_operation(Operation::CreateTable {
				name: "users".to_string(),
				columns: vec![ColumnDefinition::new(
					"id",
					FieldType::Custom("INTEGER PRIMARY KEY".to_string()),
				)],
				constraints: vec![],
				without_rowid: None,
				partition: None,
				interleave_in_parent: None,
			});

		let migration2 = Migration::new("0002_add_field", "myapp")
			.add_dependency("myapp", "0001_initial")
			.add_operation(Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition::new("name", FieldType::VarChar(100)),
				mysql_options: None,
			});

		let migrations = vec![migration1, migration2];

		let squasher = MigrationSquasher::new();
		let squashed = squasher
			.squash(&migrations, "0001_squashed_0002", SquashOptions::default())
			.unwrap();

		assert_eq!(squashed.operations.len(), 2);
	}

	#[test]
	fn test_squash_external_dependencies() {
		let migration1 =
			Migration::new("0001_initial", "myapp").add_dependency("other_app", "0001_initial");

		let migration2 =
			Migration::new("0002_add_field", "myapp").add_dependency("myapp", "0001_initial");

		let migrations = vec![migration1, migration2];

		let squasher = MigrationSquasher::new();
		let squashed = squasher
			.squash(&migrations, "0001_squashed_0002", SquashOptions::default())
			.unwrap();

		// Should keep external dependency
		assert_eq!(squashed.dependencies.len(), 1);
		assert_eq!(squashed.dependencies[0].0, "other_app");
	}
}
