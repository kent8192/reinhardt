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
	/// The format is `merge_{leaf1}_{leaf2}` (sorted alphabetically) with truncation if the name exceeds
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
		let mut sorted_names: Vec<&str> = leaf_names.to_vec();
		sorted_names.sort();
		let combined = sorted_names.join("_");
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

	// ================================================================
	// Operation fragment tests (issue #3198 coverage expansion)
	// ================================================================

	#[test]
	fn test_non_initial_with_empty_operations() {
		// Edge case: is_initial=false with no operations should trigger auto-naming
		let name = MigrationNamer::generate_name(&[], false);
		assert!(
			name.starts_with("auto_"),
			"Non-initial with empty ops should get auto name, got '{}'",
			name
		);
	}

	#[test]
	fn test_drop_table_fragment() {
		let ops = vec![Operation::DropTable {
			name: "Users".to_string(),
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "delete_users");
	}

	#[test]
	fn test_drop_column_fragment() {
		let ops = vec![Operation::DropColumn {
			table: "Users".to_string(),
			column: "Email".to_string(),
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "remove_users_email");
	}

	#[test]
	fn test_alter_column_fragment() {
		let ops = vec![Operation::AlterColumn {
			table: "Users".to_string(),
			column: "Age".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition {
				name: "age".to_string(),
				type_definition: FieldType::Integer,
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
			},
			mysql_options: None,
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "alter_users_age");
	}

	#[test]
	fn test_rename_table_fragment() {
		let ops = vec![Operation::RenameTable {
			old_name: "Users".to_string(),
			new_name: "Accounts".to_string(),
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "rename_users_to_accounts");
	}

	#[test]
	fn test_rename_column_fragment() {
		let ops = vec![Operation::RenameColumn {
			table: "Users".to_string(),
			old_name: "created_at".to_string(),
			new_name: "date_joined".to_string(),
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "rename_users_date_joined");
	}

	#[test]
	fn test_add_constraint_fragment() {
		let ops = vec![Operation::AddConstraint {
			table: "Orders".to_string(),
			constraint_sql: "CHECK (amount > 0)".to_string(),
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "add_constraint_orders");
	}

	#[test]
	fn test_drop_constraint_fragment() {
		let ops = vec![Operation::DropConstraint {
			table: "orders".to_string(),
			constraint_name: "CK_Amount".to_string(),
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "drop_constraint_ck_amount");
	}

	#[test]
	fn test_drop_index_fragment() {
		let ops = vec![Operation::DropIndex {
			table: "Users".to_string(),
			columns: vec!["email".to_string()],
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "drop_index_users");
	}

	#[test]
	fn test_create_unique_index_fragment() {
		let ops = vec![Operation::CreateIndex {
			table: "Users".to_string(),
			columns: vec!["email".to_string()],
			unique: true,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "create_unique_index_users");
	}

	#[test]
	fn test_create_non_unique_index_fragment() {
		let ops = vec![Operation::CreateIndex {
			table: "Users".to_string(),
			columns: vec!["email".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "create_index_users");
	}

	#[test]
	fn test_run_rust_triggers_auto_naming() {
		let ops = vec![Operation::RunRust {
			code: "fn run() {}".to_string(),
			reverse_code: None,
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert!(
			name.starts_with("auto_"),
			"RunRust should trigger auto-naming, got '{}'",
			name
		);
	}

	#[test]
	fn test_alter_table_comment_fragment() {
		let ops = vec![Operation::AlterTableComment {
			table: "Users".to_string(),
			comment: Some("User accounts".to_string()),
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "alter_comment_users");
	}

	#[test]
	fn test_create_schema_fragment() {
		let ops = vec![Operation::CreateSchema {
			name: "Tenant_A".to_string(),
			if_not_exists: true,
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "create_schema_tenant_a");
	}

	#[test]
	fn test_drop_schema_fragment() {
		let ops = vec![Operation::DropSchema {
			name: "Old_Schema".to_string(),
			cascade: true,
			if_exists: true,
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "drop_schema_old_schema");
	}

	#[test]
	fn test_create_extension_fragment() {
		let ops = vec![Operation::CreateExtension {
			name: "uuid-ossp".to_string(),
			if_not_exists: true,
			schema: None,
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "create_extension_uuid-ossp");
	}

	#[test]
	fn test_move_model_fragment() {
		let ops = vec![Operation::MoveModel {
			model_name: "UserProfile".to_string(),
			from_app: "Auth".to_string(),
			to_app: "Accounts".to_string(),
			rename_table: false,
			old_table_name: None,
			new_table_name: None,
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "move_auth_userprofile_accounts_userprofile");
	}

	#[test]
	fn test_create_inherited_table_fragment() {
		let ops = vec![Operation::CreateInheritedTable {
			name: "AdminUser".to_string(),
			columns: vec![],
			base_table: "users".to_string(),
			join_column: "user_id".to_string(),
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "create_inherited_adminuser");
	}

	#[test]
	fn test_add_discriminator_column_fragment() {
		let ops = vec![Operation::AddDiscriminatorColumn {
			table: "Users".to_string(),
			column_name: "user_type".to_string(),
			default_value: "standard".to_string(),
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "add_discriminator_users");
	}

	#[test]
	fn test_alter_unique_together_fragment() {
		let ops = vec![Operation::AlterUniqueTogether {
			table: "Orders".to_string(),
			unique_together: vec![vec!["user_id".to_string(), "product_id".to_string()]],
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "alter_unique_orders");
	}

	#[test]
	fn test_alter_model_options_fragment() {
		let ops = vec![Operation::AlterModelOptions {
			table: "Products".to_string(),
			options: std::collections::HashMap::new(),
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "alter_options_products");
	}

	#[test]
	fn test_bulk_load_fragment() {
		use crate::migrations::operations::{BulkLoadFormat, BulkLoadOptions, BulkLoadSource};
		let ops = vec![Operation::BulkLoad {
			table: "Events".to_string(),
			source: BulkLoadSource::Stdin,
			format: BulkLoadFormat::Csv,
			options: BulkLoadOptions::default(),
		}];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "bulk_load_events");
	}

	// ================================================================
	// Mixed operations and edge cases
	// ================================================================

	#[test]
	fn test_mixed_fragment_and_no_fragment_operations() {
		// Operations with fragments should be used; RunSQL (no fragment) is ignored
		let ops = vec![
			Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition {
					name: "email".to_string(),
					type_definition: FieldType::VarChar(255),
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
			Operation::RunSQL {
				sql: "UPDATE users SET email = ''".to_string(),
				reverse_sql: None,
			},
		];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(
			name, "users_email",
			"RunSQL (no fragment) should be filtered out, leaving only the AddColumn fragment"
		);
	}

	#[test]
	fn test_all_no_fragment_operations_trigger_auto_naming() {
		let ops = vec![
			Operation::RunSQL {
				sql: "SELECT 1".to_string(),
				reverse_sql: None,
			},
			Operation::RunRust {
				code: "fn run() {}".to_string(),
				reverse_code: None,
			},
		];
		let name = MigrationNamer::generate_name(&ops, false);
		assert!(
			name.starts_with("auto_"),
			"All no-fragment ops should trigger auto-naming, got '{}'",
			name
		);
	}

	#[test]
	fn test_is_initial_true_ignores_operations_entirely() {
		// Even with descriptive operations, is_initial=true should return "initial"
		let ops = vec![
			Operation::AddColumn {
				table: "users".to_string(),
				column: ColumnDefinition {
					name: "email".to_string(),
					type_definition: FieldType::VarChar(255),
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
			Operation::DropTable {
				name: "old_table".to_string(),
			},
		];
		let name = MigrationNamer::generate_name(&ops, true);
		assert_eq!(
			name, "initial",
			"is_initial=true must always return 'initial' regardless of operations"
		);
	}

	#[test]
	fn test_multiple_different_operation_types_combined() {
		let ops = vec![
			Operation::CreateTable {
				name: "posts".to_string(),
				columns: vec![],
				constraints: vec![],
				without_rowid: None,
				partition: None,
				interleave_in_parent: None,
			},
			Operation::DropTable {
				name: "old_posts".to_string(),
			},
		];
		let name = MigrationNamer::generate_name(&ops, false);
		assert_eq!(name, "posts_delete_old_posts");
	}

	// ================================================================
	// Merge name tests (continued from existing)
	// ================================================================

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
			name.len() <= MAX_NAME_LENGTH,
			"Name should be within MAX_NAME_LENGTH ({}), got len={}",
			MAX_NAME_LENGTH,
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

	#[test]
	fn test_generate_merge_name_unsorted_input_produces_deterministic_output() {
		// Arrange: intentionally unsorted input
		let unsorted = &["0002_b", "0002_a"];
		let sorted = &["0002_a", "0002_b"];

		// Act
		let name_from_unsorted = MigrationNamer::generate_merge_name(unsorted);
		let name_from_sorted = MigrationNamer::generate_merge_name(sorted);

		// Assert: both produce the same deterministic output
		assert_eq!(name_from_unsorted, "merge_0002_a_0002_b");
		assert_eq!(name_from_unsorted, name_from_sorted);
	}

	#[test]
	fn test_generate_merge_name_empty_slice() {
		// Arrange
		let leaves: &[&str] = &[];

		// Act
		let name = MigrationNamer::generate_merge_name(leaves);

		// Assert: should return "merge_" without panic
		assert_eq!(name, "merge_");
	}

	#[test]
	fn test_generate_merge_name_boundary_52_and_53_chars() {
		// Arrange: "merge_" is 6 chars, MAX_NAME_LENGTH is 52
		// Need leaf content of exactly 46 chars for boundary (52 total)
		let leaf_46 = "a".repeat(46);
		let leaf_47 = "a".repeat(47);

		// Act: exactly 52 chars (6 + 46) - no truncation
		let name_52 = MigrationNamer::generate_merge_name(&[&leaf_46]);

		// Assert: no truncation at exactly MAX_NAME_LENGTH
		assert_eq!(name_52.len(), 52);
		assert_eq!(name_52, format!("merge_{}", leaf_46));
		assert!(!name_52.ends_with("_and_more"));

		// Act: 53 chars (6 + 47) - truncation
		let name_53 = MigrationNamer::generate_merge_name(&[&leaf_47]);

		// Assert: truncated with _and_more suffix
		assert!(
			name_53.len() <= MAX_NAME_LENGTH,
			"Name should be within MAX_NAME_LENGTH ({}), got len={}",
			MAX_NAME_LENGTH,
			name_53.len()
		);
		assert!(name_53.ends_with("_and_more"));
	}
}
