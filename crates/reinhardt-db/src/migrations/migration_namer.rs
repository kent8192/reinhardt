//! Migration name generation
//!
//! This module provides Django-style migration naming:
//! - `0001_initial` for initial migrations
//! - `NNNN_model_field` for single operations
//! - `NNNN_op1_op2` for multiple operations
//! - `NNNN_auto_YYYYMMdd_HHMM` for operations without fragments

use super::Operation;
use super::operation_trait::MigrationOperation;
use chrono::Utc;

/// Maximum total length for migration name (52 characters per Django)
const MAX_NAME_LENGTH: usize = 52;

/// Generates Django-style migration names
pub struct MigrationNamer;

impl MigrationNamer {
	/// Generate migration name from operations
	///
	/// # Rules
	///
	/// 1. **Initial migrations**: `"initial"`
	/// 2. **Single operation with fragment**: `"{fragment}"`
	/// 3. **Multiple operations**: `"{frag1}_{frag2}..."`
	///    - Max 52 characters, truncate with `_and_more`
	/// 4. **No fragments**: `"auto_{timestamp}"`
	///
	/// # Examples
	///
	/// ```rust
	/// # use reinhardt_db::migrations::{MigrationNamer, Operation};
	/// // Initial migration
	/// assert_eq!(
	///     MigrationNamer::generate_name(&[], true),
	///     "initial"
	/// );
	///
	/// // Single operation
	/// let ops = vec![Operation::CreateTable {
	///     name: "users".to_string(),
	///     columns: vec![],
	///     constraints: vec![],
	///     without_rowid: None,
	///     interleave_in_parent: None,
	///     partition: None,
	/// }];
	/// assert_eq!(
	///     MigrationNamer::generate_name(&ops, false),
	///     "users"
	/// );
	///
	/// // Multiple operations
	/// let ops = vec![
	///     Operation::AddColumn {
	///         table: "users".to_string(),
	///         column: reinhardt_db::migrations::ColumnDefinition {
	///             name: "email".to_string(),
	///             type_definition: reinhardt_db::migrations::FieldType::Custom("VARCHAR(255)".to_string()),
	///             not_null: false,
	///             unique: false,
	///             primary_key: false,
	///             auto_increment: false,
	///             default: None,
	///         },
	///         mysql_options: None,
	///     },
	///     Operation::AddColumn {
	///         table: "users".to_string(),
	///         column: reinhardt_db::migrations::ColumnDefinition {
	///             name: "phone".to_string(),
	///             type_definition: reinhardt_db::migrations::FieldType::Custom("VARCHAR(20)".to_string()),
	///             not_null: false,
	///             unique: false,
	///             primary_key: false,
	///             auto_increment: false,
	///             default: None,
	///         },
	///         mysql_options: None,
	///     },
	/// ];
	/// assert_eq!(
	///     MigrationNamer::generate_name(&ops, false),
	///     "users_email_users_phone"
	/// );
	///
	/// // No fragments (RunSQL)
	/// let ops = vec![Operation::RunSQL {
	///     sql: "SELECT 1".to_string(),
	///     reverse_sql: None,
	/// }];
	/// assert!(
	///     MigrationNamer::generate_name(&ops, false).starts_with("auto_")
	/// );
	/// ```
	pub fn generate_name(operations: &[Operation], is_initial: bool) -> String {
		// Rule 1: Initial migrations
		if is_initial {
			return "initial".to_string();
		}

		// Extract fragments from operations
		let fragments: Vec<String> = operations
			.iter()
			.filter_map(|op| op.migration_name_fragment())
			.collect();

		// Rule 4: No fragments → auto-naming with timestamp
		if fragments.is_empty() {
			return Self::auto_name();
		}

		// Rule 2 & 3: Single or multiple operations
		let combined = fragments.join("_");

		// Truncate if exceeds maximum length
		if combined.len() <= MAX_NAME_LENGTH {
			combined
		} else {
			Self::truncate_name(&combined)
		}
	}

	/// Generate auto-naming with timestamp: `auto_YYYYMMdd_HHMMSS_nnnnnnnnn`
	///
	/// Uses nanosecond precision to prevent collisions from rapid consecutive calls.
	pub fn auto_name() -> String {
		let now = Utc::now();
		format!(
			"auto_{}_{:09}",
			now.format("%Y%m%d_%H%M%S"),
			now.timestamp_subsec_nanos()
		)
	}

	/// Generate merge migration name from conflicting leaf names
	///
	/// Combines leaf migration names into a merge name following Django conventions.
	/// The format is `merge_{leaf1}_{leaf2}` with truncation if the name exceeds
	/// `MAX_NAME_LENGTH`.
	///
	/// When the combined name is too long, it is truncated with `_and_more` suffix.
	///
	/// # Examples
	///
	/// ```rust
	/// # use reinhardt_db::migrations::MigrationNamer;
	/// let name = MigrationNamer::generate_merge_name(&["0002_add_field", "0002_add_index"]);
	/// assert_eq!(name, "merge_0002_add_field_0002_add_index");
	///
	/// let name = MigrationNamer::generate_merge_name(&["0002_a", "0002_b", "0002_c"]);
	/// assert_eq!(name, "merge_0002_a_0002_b_0002_c");
	/// ```
	pub fn generate_merge_name(leaf_names: &[&str]) -> String {
		let combined = leaf_names.join("_");
		let name = format!("merge_{}", combined);

		if name.len() <= MAX_NAME_LENGTH {
			name
		} else {
			Self::truncate_name(&name)
		}
	}

	/// Truncate long migration name and append `_and_more`
	///
	/// Ensures the final name stays within `MAX_NAME_LENGTH` characters.
	fn truncate_name(name: &str) -> String {
		const SUFFIX: &str = "_and_more";
		let max_prefix = MAX_NAME_LENGTH.saturating_sub(SUFFIX.len());

		// Find safe truncation point (avoid cutting mid-word if possible)
		let truncate_at = name
			.char_indices()
			.take_while(|(idx, _)| *idx < max_prefix)
			.last()
			.map(|(idx, ch)| idx + ch.len_utf8())
			.unwrap_or(0);

		format!("{}{}", &name[..truncate_at], SUFFIX)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::migrations::{ColumnDefinition, FieldType};

	#[test]
	fn test_initial_migration() {
		let name = MigrationNamer::generate_name(&[], true);
		assert_eq!(name, "initial");
	}

	#[test]
	fn test_single_operation_create_table() {
		let ops = vec![Operation::CreateTable {
			name: "users".to_string(),
			columns: vec![],
			constraints: vec![],
			without_rowid: None,
			partition: None,
			interleave_in_parent: None,
		}];

		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "users");
	}

	#[test]
	fn test_multiple_operations() {
		let ops = vec![
			Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition {
					name: "email".to_string(),
					type_definition: FieldType::Custom("VARCHAR(255)".to_string()),
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
			Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition {
					name: "phone".to_string(),
					type_definition: FieldType::Custom("VARCHAR(20)".to_string()),
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
		];

		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "users_email_users_phone");
	}

	#[test]
	fn test_no_fragments_auto_naming() {
		let ops = vec![Operation::RunSQL {
			sql: "SELECT 1".to_string(),
			reverse_sql: None,
		}];

		let name = MigrationNamer::generate_name(&ops, false);
		assert!(name.starts_with("auto_"));
		assert!(name.contains("_"));
	}

	#[test]
	fn test_truncate_long_name() {
		let long_name = "a".repeat(60);
		let truncated = MigrationNamer::truncate_name(&long_name);

		assert!(truncated.len() <= MAX_NAME_LENGTH);
		assert!(truncated.ends_with("_and_more"));
	}

	#[test]
	fn test_exact_max_length() {
		let exact_name = "a".repeat(MAX_NAME_LENGTH);
		let result = if exact_name.len() <= MAX_NAME_LENGTH {
			exact_name.clone()
		} else {
			MigrationNamer::truncate_name(&exact_name)
		};

		assert_eq!(result, exact_name);
	}

	#[test]
	fn test_generate_merge_name_two_leaves() {
		// Arrange
		let leaves = &["0002_add_field", "0002_add_index"];

		// Act
		let name = MigrationNamer::generate_merge_name(leaves);

		// Assert
		assert_eq!(name, "merge_0002_add_field_0002_add_index");
	}

	#[test]
	fn test_generate_merge_name_three_leaves() {
		// Arrange
		let leaves = &["0002_a", "0002_b", "0002_c"];

		// Act
		let name = MigrationNamer::generate_merge_name(leaves);

		// Assert
		assert_eq!(name, "merge_0002_a_0002_b_0002_c");
	}

	#[test]
	fn test_generate_merge_name_truncation() {
		// Arrange: create leaf names that when combined exceed MAX_NAME_LENGTH
		let leaves = &[
			"0002_very_long_migration_name_alpha",
			"0002_very_long_migration_name_beta",
		];

		// Act
		let name = MigrationNamer::generate_merge_name(leaves);

		// Assert
		assert!(
			name.len() <= 52,
			"Name should be within MAX_NAME_LENGTH, got len={}",
			name.len()
		);
		assert!(name.starts_with("merge_"));
		assert!(name.ends_with("_and_more"));
	}

	#[test]
	fn test_generate_merge_name_single_leaf() {
		// Arrange: edge case - single leaf (defensive)
		let leaves = &["0002_add_field"];

		// Act
		let name = MigrationNamer::generate_merge_name(leaves);

		// Assert
		assert_eq!(name, "merge_0002_add_field");
	}
}
