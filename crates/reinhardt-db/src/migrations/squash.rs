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

use super::dependency::{DependencyResolutionContext, DependencyResolver, MigrationDependency};
use super::{Migration, MigrationError, Operation, Result, SquashRange};
use reinhardt_query::prelude::SchemaExpr;
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

/// The migration and operation counts produced by a range squash.
#[derive(Debug)]
pub struct SquashResult {
	/// The combined migration.
	pub migration: Migration,
	/// Number of operations before optimization.
	pub original_operation_count: usize,
	/// Number of operations after optimization.
	pub optimized_operation_count: usize,
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

	/// Squash a validated migration range.
	///
	/// External dependencies retain the range's stable order. Replacement and
	/// conditional dependency metadata is deduplicated in source order.
	/// Source migrations must agree on atomicity, and the initial marker comes
	/// from the first migration.
	///
	/// # Errors
	///
	/// Returns an error for an empty range, when source migrations belong to
	/// different apps, or when `atomic`, `state_only`, or `database_only`
	/// differs between source migrations. Mixed whole-migration execution modes
	/// cannot be represented safely after combining their operations.
	pub fn squash_range(
		&self,
		range: &SquashRange,
		name: impl Into<String>,
		optimize: bool,
	) -> Result<SquashResult> {
		self.squash_range_with_context(
			range,
			name,
			optimize,
			&DependencyResolutionContext::default(),
		)
	}

	/// Squash a validated migration range using the active dependency context.
	///
	/// Swappable dependencies that resolve to selected migrations are omitted,
	/// preventing the replacement from depending on itself after graph rewrite.
	pub fn squash_range_with_context(
		&self,
		range: &SquashRange,
		name: impl Into<String>,
		optimize: bool,
		context: &DependencyResolutionContext,
	) -> Result<SquashResult> {
		let first = range.migrations.first().ok_or_else(|| {
			MigrationError::InvalidMigration("Cannot squash empty migration range".to_string())
		})?;
		if range
			.migrations
			.iter()
			.any(|migration| migration.app_label != first.app_label)
		{
			return Err(MigrationError::InvalidMigration(
				"All migrations in squash range must belong to the same app".to_string(),
			));
		}
		let state_only = first.state_only;
		let database_only = first.database_only;

		if range
			.migrations
			.iter()
			.any(|migration| migration.state_only != state_only)
		{
			return Err(MigrationError::InvalidMigration(
				"Cannot squash migrations with mixed state_only flags".to_string(),
			));
		}
		if range
			.migrations
			.iter()
			.any(|migration| migration.database_only != database_only)
		{
			return Err(MigrationError::InvalidMigration(
				"Cannot squash migrations with mixed database_only flags".to_string(),
			));
		}
		if range
			.migrations
			.iter()
			.any(|migration| migration.atomic != first.atomic)
		{
			return Err(MigrationError::InvalidMigration(
				"Cannot squash migrations with mixed atomic flags".to_string(),
			));
		}

		let mut operations = Vec::new();
		let mut replaces = Vec::new();
		let mut swappable_dependencies = Vec::new();
		let mut optional_dependencies = Vec::new();
		let dependency_resolver = DependencyResolver::new(context);
		let selected: HashSet<_> = range
			.migrations
			.iter()
			.map(|migration| (migration.app_label.as_str(), migration.name.as_str()))
			.collect();
		let mut dependencies = Vec::new();
		for migration in &range.migrations {
			operations.extend(migration.operations.clone());
			for dependency in &migration.dependencies {
				let normalized = range.normalize_dependency(&dependency.0, &dependency.1)?;
				let normalized = (normalized.app_label, normalized.name);
				if !selected.contains(&(normalized.0.as_str(), normalized.1.as_str()))
					&& !dependencies.contains(&normalized)
				{
					dependencies.push(normalized);
				}
			}
			for replacement in &migration.replaces {
				if !replaces.contains(replacement) {
					replaces.push(replacement.clone());
				}
			}
			let identity = (migration.app_label.clone(), migration.name.clone());
			if !replaces.contains(&identity) {
				replaces.push(identity);
			}
			for dependency in &migration.swappable_dependencies {
				let target = dependency_resolver
					.resolve(&MigrationDependency::Swappable(dependency.clone()))
					.expect("swappable dependencies always resolve to a target");
				let normalized = range.normalize_dependency(&target.0, &target.1)?;
				if !selected.contains(&(normalized.app_label.as_str(), normalized.name.as_str()))
					&& !swappable_dependencies.contains(dependency)
				{
					swappable_dependencies.push(dependency.clone());
				}
			}
			for dependency in &migration.optional_dependencies {
				let normalized = range
					.normalize_dependency(&dependency.app_label, &dependency.migration_name)?;
				if !selected.contains(&(normalized.app_label.as_str(), normalized.name.as_str()))
					&& !optional_dependencies.contains(dependency)
				{
					optional_dependencies.push(dependency.clone());
				}
			}
		}
		let original_operation_count = operations.len();
		if optimize {
			operations = self.optimize_operations(operations);
		}

		let mut migration = Migration::new(name, first.app_label.clone());
		migration.operations = operations;
		migration.dependencies = dependencies;
		migration.replaces = replaces;
		migration.atomic = first.atomic;
		migration.initial = first.initial;
		migration.state_only = state_only;
		migration.database_only = database_only;
		migration.swappable_dependencies = swappable_dependencies;
		migration.optional_dependencies = optional_dependencies;

		Ok(SquashResult {
			optimized_operation_count: migration.operations.len(),
			migration,
			original_operation_count,
		})
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

		// Build a HashSet of squashed migration identities for O(1) lookup
		let squashed_set: HashSet<(&str, &str)> = migrations
			.iter()
			.map(|m| (m.app_label.as_str(), m.name.as_str()))
			.collect();

		// Collect dependencies from all migrations (external dependencies only)
		let mut seen_deps: HashSet<(&str, &str)> = HashSet::new();
		for migration in migrations {
			for (dep_app, dep_name) in &migration.dependencies {
				// Only include dependencies outside the squashed range
				if *dep_app != *app_label
					|| !squashed_set.contains(&(dep_app.as_str(), dep_name.as_str()))
				{
					// Avoid duplicate dependencies via HashSet lookup
					if seen_deps.insert((dep_app.as_str(), dep_name.as_str())) {
						squashed
							.dependencies
							.push((dep_app.clone(), dep_name.clone()));
					}
				}
			}
		}

		Ok(squashed)
	}

	/// Optimize operations by removing proven-equivalent schema lifecycles.
	///
	/// Create/drop table and add/drop column pairs reduce only inside segments
	/// containing the five recognized table and column operations. Adjacent
	/// alters of the same column retain the first old definition and final new
	/// definition. Every other operation is a barrier, including data
	/// operations and operation variants added in the future.
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
		let mut optimized = Vec::with_capacity(operations.len());
		let mut segment = Vec::new();
		for operation in operations {
			if Self::is_reducible(&operation) {
				segment.push(operation);
			} else {
				optimized.extend(Self::optimize_segment(std::mem::take(&mut segment)));
				optimized.push(operation);
			}
		}
		optimized.extend(Self::optimize_segment(segment));

		optimized
	}

	fn is_reducible(operation: &Operation) -> bool {
		matches!(
			operation,
			Operation::CreateTable { .. }
				| Operation::DropTable { .. }
				| Operation::AddColumn { .. }
				| Operation::DropColumn { .. }
				| Operation::AlterColumn { .. }
		)
	}

	fn optimize_segment(segment: Vec<Operation>) -> Vec<Operation> {
		let mut optimized = Vec::with_capacity(segment.len());
		let mut previous_alter = None;

		for operation in segment {
			let current_alter = match &operation {
				Operation::AlterColumn { table, column, .. } => {
					Some((table.clone(), column.clone()))
				}
				_ => None,
			};
			match operation {
				Operation::DropTable { ref name } => {
					if let Some(create_index) =
						optimized.iter().rposition(
							|candidate| matches!(candidate, Operation::CreateTable { name: candidate_name, .. } if candidate_name == name),
						)
						&& optimized[create_index + 1..]
							.iter()
							.filter(|candidate| Self::operation_table(candidate) == Some(name))
							.all(|candidate| {
								matches!(
									candidate,
									Operation::AddColumn { .. }
										| Operation::DropColumn { .. }
										| Operation::AlterColumn { .. }
								)
							})
							&& !optimized[create_index + 1..]
								.iter()
								.any(|candidate| Self::operation_references_table(candidate, name))
					{
						optimized = optimized
							.into_iter()
							.enumerate()
							.filter_map(|(index, candidate)| {
								let remove_same_table =
									index > create_index
										&& Self::operation_table(&candidate) == Some(name);
								(index != create_index && !remove_same_table).then_some(candidate)
							})
							.collect();
					} else {
						optimized.push(operation);
					}
				}
				Operation::DropColumn {
					ref table,
					ref column,
					..
				} => {
					if let Some(add_index) = optimized.iter().rposition(|candidate| {
						matches!(
							candidate,
							Operation::AddColumn {
								table: candidate_table,
								column: candidate_column,
								..
							} if candidate_table == table && candidate_column.name == *column
						)
					}) && !optimized[add_index + 1..].iter().any(|candidate| {
						matches!(
							candidate,
							Operation::CreateTable { name, .. } | Operation::DropTable { name }
								if name == table
						) || matches!(
							candidate,
							Operation::AddColumn {
								table: candidate_table,
								column: candidate_column,
								..
							} if candidate_table == table && candidate_column.name == *column
						) || matches!(
							candidate,
							Operation::DropColumn {
								table: candidate_table,
								column: candidate_column,
								..
							} if candidate_table == table && candidate_column == column
						) || Self::operation_references_column(candidate, table, column)
					}) {
						optimized = optimized
							.into_iter()
							.enumerate()
							.filter_map(|(index, candidate)| {
								let remove_matching_alter = index > add_index
									&& matches!(
										&candidate,
										Operation::AlterColumn {
											table: candidate_table,
											column: candidate_column,
											..
										} if candidate_table == table && candidate_column == column
									);
								(index != add_index && !remove_matching_alter).then_some(candidate)
							})
							.collect();
					} else {
						optimized.push(operation);
					}
				}
				Operation::AlterColumn {
					table,
					column,
					old_definition,
					new_definition,
					mysql_options,
				} => {
					let follows_same_alter =
						previous_alter
							.as_ref()
							.is_some_and(|(previous_table, previous_column)| {
								previous_table == &table && previous_column == &column
							});
					if follows_same_alter
						&& let Some(Operation::AlterColumn {
						table: previous_table,
						column: previous_column,
						new_definition: previous_new_definition,
						mysql_options: previous_mysql_options,
						..
					}) = optimized.last_mut()
						&& *previous_table == table
						&& *previous_column == column
						&& *previous_new_definition == new_definition
					{
						*previous_new_definition = new_definition;
						*previous_mysql_options = mysql_options;
					} else {
						optimized.push(Operation::AlterColumn {
							table,
							column,
							old_definition,
							new_definition,
							mysql_options,
						});
					}
				}
				_ => optimized.push(operation),
			}
			previous_alter = current_alter;
		}

		optimized
	}

	fn operation_references_column(operation: &Operation, table: &str, column: &str) -> bool {
		let references_foreign_key = |definition: &crate::migrations::ColumnDefinition| {
			matches!(
				&definition.type_definition,
				crate::migrations::FieldType::ForeignKey { to_table, to_field, .. }
					if to_table == table && to_field == column
			)
		};
		match operation {
			Operation::CreateTable {
				columns,
				constraints,
				..
			} => {
				columns.iter().any(references_foreign_key)
					|| constraints.iter().any(|constraint| match constraint {
						crate::migrations::Constraint::ForeignKey {
							referenced_table,
							referenced_columns,
							..
						} => {
							referenced_table == table
								&& referenced_columns.iter().any(|referenced| referenced == column)
						}
						crate::migrations::Constraint::OneToOne {
							referenced_table,
							referenced_column,
							..
						} => referenced_table == table && referenced_column == column,
						_ => false,
					})
			}
			Operation::AddColumn {
				table: candidate_table,
				column: candidate_column,
				..
			} => {
				references_foreign_key(candidate_column)
					|| (candidate_table == table
						&& Self::column_references_column(candidate_column, column))
			}
			Operation::AlterColumn {
				table: candidate_table,
				old_definition,
				new_definition,
				..
			} => {
				references_foreign_key(new_definition)
					|| old_definition.as_ref().is_some_and(references_foreign_key)
					|| (candidate_table == table
						&& (Self::column_references_column(new_definition, column)
							|| old_definition.as_ref().is_some_and(|definition| {
								Self::column_references_column(definition, column)
							})))
			}
			_ => false,
		}
	}

	fn operation_references_table(operation: &Operation, table: &str) -> bool {
		let column_references_table = |definition: &crate::migrations::ColumnDefinition| {
			matches!(
				&definition.type_definition,
				crate::migrations::FieldType::ForeignKey { to_table, .. } if to_table == table
			)
		};
		match operation {
			Operation::CreateTable {
				columns,
				constraints,
				..
			} => {
				columns.iter().any(column_references_table)
					|| constraints.iter().any(|constraint| {
						matches!(
							constraint,
							crate::migrations::Constraint::ForeignKey { referenced_table, .. }
								| crate::migrations::Constraint::OneToOne { referenced_table, .. }
								if referenced_table == table
						)
					})
			}
			Operation::AddColumn { column, .. } => column_references_table(column),
			Operation::AlterColumn {
				old_definition,
				new_definition,
				..
			} => {
				column_references_table(new_definition)
					|| old_definition.as_ref().is_some_and(column_references_table)
			}
			_ => false,
		}
	}

	fn column_references_column(
		definition: &crate::migrations::ColumnDefinition,
		column: &str,
	) -> bool {
		let Some(generated) = definition.generated.as_ref() else {
			return false;
		};
		if let Some(expression) = generated.expr.as_deref() {
			return Self::schema_expr_references_column(expression, column);
		}
		generated
			.raw_sql
			.as_deref()
			.or(generated.expr_tokens.as_deref())
			.is_some_and(|expression| Self::expression_text_references_column(expression, column))
	}

	fn schema_expr_references_column(expression: &SchemaExpr, column: &str) -> bool {
		match expression {
			SchemaExpr::Column(identifier) => identifier.to_string() == column,
			SchemaExpr::Value(_) => false,
			SchemaExpr::Binary { left, right, .. } => {
				Self::schema_expr_references_column(left, column)
					|| Self::schema_expr_references_column(right, column)
			}
			SchemaExpr::Function { args, .. } => args
				.iter()
				.any(|argument| Self::schema_expr_references_column(argument, column)),
			SchemaExpr::Cast { expr, .. } => Self::schema_expr_references_column(expr, column),
			_ => false,
		}
	}

	fn expression_text_references_column(expression: &str, column: &str) -> bool {
		expression
			.split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
			.any(|token| token.eq_ignore_ascii_case(column))
	}

	fn operation_table(operation: &Operation) -> Option<&String> {
		match operation {
			Operation::CreateTable { name, .. } | Operation::DropTable { name } => Some(name),
			Operation::AddColumn { table, .. }
			| Operation::DropColumn { table, .. }
			| Operation::AlterColumn { table, .. } => Some(table),
			_ => None,
		}
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
	use crate::migrations::{
		ColumnDefinition, DependencyCondition, FieldType, ForeignKeyAction,
		GeneratedColumnDefinition, OptionalDependency,
	};
	use reinhardt_query::prelude::GeneratedStorage;

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
				old_definition: None,
			},
		];

		let optimized = squasher.optimize_operations(ops);
		assert_eq!(optimized.len(), 0);
	}

	#[test]
	fn test_optimize_add_drop_column_preserves_generated_column_dependency() {
		// Arrange
		let squasher = MigrationSquasher::new();
		let mut derived = ColumnDefinition::new("derived", FieldType::Integer);
		derived.generated = Some(GeneratedColumnDefinition::raw_sql(
			"temp + 1",
			GeneratedStorage::Stored,
		));
		let ops = vec![
			Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition::new("temp", FieldType::Integer),
				mysql_options: None,
			},
			Operation::AddColumn {
				table: "users".to_string(),
				column: derived.clone(),
				mysql_options: None,
			},
			Operation::AlterColumn {
				table: "users".to_string(),
				column: "derived".to_string(),
				old_definition: Some(derived),
				new_definition: ColumnDefinition::new("derived", FieldType::Integer),
				mysql_options: None,
			},
			Operation::DropColumn {
				table: "users".to_string(),
				column: "temp".to_string(),
				old_definition: None,
			},
		];

		// Act
		let optimized = squasher.optimize_operations(ops.clone());

		// Assert
		assert_eq!(optimized, ops);
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

	#[rstest::rstest]
	fn optimize_operations_preserves_foreign_key_references_to_transient_tables() {
		// Arrange
		let foreign_key = ColumnDefinition::new(
			"temp_id",
			FieldType::ForeignKey {
				to_table: "temp".to_string(),
				to_field: "id".to_string(),
				on_delete: ForeignKeyAction::NoAction,
			},
		);
		let migrations = vec![
			Migration::new("0001_initial", "app")
				.add_operation(Operation::CreateTable {
					name: "temp".to_string(),
					columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
					constraints: Vec::new(),
					without_rowid: None,
					interleave_in_parent: None,
					partition: None,
				})
				.add_operation(Operation::AddColumn {
					table: "accounts".to_string(),
					column: foreign_key.clone(),
					mysql_options: None,
				})
				.add_operation(Operation::AlterColumn {
					table: "accounts".to_string(),
					column: "temp_id".to_string(),
					old_definition: Some(foreign_key),
					new_definition: ColumnDefinition::new("temp_id", FieldType::Integer),
					mysql_options: None,
				})
				.add_operation(Operation::DropTable {
					name: "temp".to_string(),
				}),
		];

		// Act
		let squashed = MigrationSquasher::new()
			.squash(&migrations, "0001_squashed", SquashOptions::default())
			.unwrap();

		// Assert
		assert_eq!(squashed.operations.len(), 4);
	}

	#[test]
	fn optimize_operations_preserves_create_table_foreign_key_to_transient_column() {
		// Arrange
		let migrations = vec![
			Migration::new("0001_initial", "app")
				.add_operation(Operation::AddColumn {
					table: "accounts".to_string(),
					column: ColumnDefinition::new("temp", FieldType::Integer),
					mysql_options: None,
				})
				.add_operation(Operation::CreateTable {
					name: "audit".to_string(),
					columns: vec![ColumnDefinition::new(
						"account_temp",
						FieldType::ForeignKey {
							to_table: "accounts".to_string(),
							to_field: "temp".to_string(),
							on_delete: ForeignKeyAction::NoAction,
						},
					)],
					constraints: Vec::new(),
					without_rowid: None,
					interleave_in_parent: None,
					partition: None,
				})
				.add_operation(Operation::DropColumn {
					table: "accounts".to_string(),
					column: "temp".to_string(),
					old_definition: None,
				}),
		];

		// Act
		let squashed = MigrationSquasher::new()
			.squash(&migrations, "0001_squashed", SquashOptions::default())
			.unwrap();

		// Assert
		assert_eq!(squashed.operations.len(), 3);
	}

	#[test]
	fn test_squash_range_excludes_optional_dependencies_within_range() {
		// Arrange
		let mut first = Migration::new("0001_initial", "myapp");
		first.optional_dependencies.push(OptionalDependency::new(
			"myapp",
			"0002_add_field",
			DependencyCondition::AppInstalled("myapp".to_string()),
		));
		let second = Migration::new("0002_add_field", "myapp");
		let range = SquashRange {
			migrations: vec![first, second],
			external_dependencies: vec![],
			available_migrations: Vec::new(),
			replacement_owners: std::collections::HashMap::new(),
		};

		// Act
		let result = MigrationSquasher::new()
			.squash_range(&range, "0001_squashed_0002", false)
			.unwrap();

		// Assert
		assert!(result.migration.optional_dependencies.is_empty());
	}

	#[test]
	fn squash_range_excludes_selected_swappable_dependency_with_active_setting() {
		let mut first = Migration::new("0001_initial", "auth");
		first
			.swappable_dependencies
			.push(crate::migrations::dependency::SwappableDependency::new(
				"AUTH_USER_MODEL",
				"accounts",
				"User",
				"0002_user",
			));
		let second = Migration::new("0002_user", "auth");
		let range = SquashRange {
			migrations: vec![first, second],
			external_dependencies: vec![],
			available_migrations: Vec::new(),
			replacement_owners: std::collections::HashMap::new(),
		};
		let context =
			DependencyResolutionContext::new().with_setting("AUTH_USER_MODEL", "auth.User");

		let result = MigrationSquasher::new()
			.squash_range_with_context(&range, "0001_squashed_0002", false, &context)
			.unwrap();

		assert!(result.migration.swappable_dependencies.is_empty());
	}

	#[test]
	fn test_squash_range_rejects_mixed_atomicity() {
		let first = Migration::new("0001_initial", "myapp");
		let mut second = Migration::new("0002_non_atomic", "myapp");
		second.atomic = false;
		let range = SquashRange {
			migrations: vec![first, second],
			external_dependencies: vec![],
			available_migrations: Vec::new(),
			replacement_owners: std::collections::HashMap::new(),
		};

		let error = MigrationSquasher::new()
			.squash_range(&range, "0001_squashed_0002", false)
			.unwrap_err();

		assert_eq!(
			error.to_string(),
			"Invalid migration: Cannot squash migrations with mixed atomic flags"
		);
	}

	#[test]
	fn squash_range_normalizes_required_dependencies_owned_by_selected_replacement() {
		let first = Migration::new("0001_squashed_0002", "myapp");
		let second = Migration::new("0003_more", "myapp").add_dependency("myapp", "0002_old");
		let range = SquashRange {
			migrations: vec![first, second],
			external_dependencies: vec![],
			available_migrations: vec![
				crate::migrations::MigrationKey::new("myapp", "0001_squashed_0002"),
				crate::migrations::MigrationKey::new("myapp", "0003_more"),
			],
			replacement_owners: std::collections::HashMap::from([(
				crate::migrations::MigrationKey::new("myapp", "0002_old"),
				crate::migrations::MigrationKey::new("myapp", "0001_squashed_0002"),
			)]),
		};

		let result = MigrationSquasher::new()
			.squash_range(&range, "0001_squashed_0003", false)
			.unwrap();

		assert!(result.migration.dependencies.is_empty());
	}

	#[test]
	fn squash_range_normalizes_optional_dependency_before_retaining_metadata() {
		let mut first = Migration::new("0001_squashed_0002", "myapp");
		first.optional_dependencies.push(OptionalDependency::new(
			"myapp",
			"0002_old",
			DependencyCondition::AppInstalled("myapp".to_string()),
		));
		let range = SquashRange {
			migrations: vec![first],
			external_dependencies: vec![],
			available_migrations: vec![crate::migrations::MigrationKey::new(
				"myapp",
				"0001_squashed_0002",
			)],
			replacement_owners: std::collections::HashMap::from([(
				crate::migrations::MigrationKey::new("myapp", "0002_old"),
				crate::migrations::MigrationKey::new("myapp", "0001_squashed_0002"),
			)]),
		};

		let result = MigrationSquasher::new()
			.squash_range(&range, "0001_squashed_0002_again", false)
			.unwrap();

		assert!(result.migration.optional_dependencies.is_empty());
	}
}
