//! Comment type definitions
//!
//! This module provides types for comment-related DDL operations:
//!
//! - [`CommentTarget`]: Target object for COMMENT ON statements

use crate::types::DynIden;

/// Target object for COMMENT ON statement
///
/// This enum represents the various database objects that can have comments
/// attached to them in PostgreSQL.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::comment::CommentTarget;
/// use reinhardt_query::types::IntoIden;
///
/// // COMMENT ON TABLE "users"
/// let target = CommentTarget::Table("users".into_iden());
///
/// // COMMENT ON COLUMN "users"."email"
/// let target = CommentTarget::Column("users".into_iden(), "email".into_iden());
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum CommentTarget {
	/// COMMENT ON TABLE table_name
	Table(DynIden),
	/// COMMENT ON COLUMN table_name.column_name
	Column(DynIden, DynIden),
	/// COMMENT ON INDEX index_name
	Index(DynIden),
	/// COMMENT ON VIEW view_name
	View(DynIden),
	/// COMMENT ON MATERIALIZED VIEW view_name
	MaterializedView(DynIden),
	/// COMMENT ON SEQUENCE sequence_name
	Sequence(DynIden),
	/// COMMENT ON SCHEMA schema_name
	Schema(DynIden),
	/// COMMENT ON DATABASE database_name
	Database(DynIden),
	/// COMMENT ON FUNCTION function_name
	Function(DynIden),
	/// COMMENT ON TRIGGER trigger_name ON table_name
	Trigger(DynIden, DynIden),
	/// COMMENT ON TYPE type_name
	Type(DynIden),
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::IntoIden;
	use rstest::*;

	#[rstest]
	fn test_comment_target_table() {
		let target = CommentTarget::Table("users".into_iden());
		match target {
			CommentTarget::Table(table) => {
				assert_eq!(table.to_string(), "users");
			}
			_ => panic!("Expected CommentTarget::Table"),
		}
	}

	#[rstest]
	fn test_comment_target_column() {
		let target = CommentTarget::Column("users".into_iden(), "email".into_iden());
		match target {
			CommentTarget::Column(table, column) => {
				assert_eq!(table.to_string(), "users");
				assert_eq!(column.to_string(), "email");
			}
			_ => panic!("Expected CommentTarget::Column"),
		}
	}

	#[rstest]
	fn test_comment_target_index() {
		let target = CommentTarget::Index("idx_users_email".into_iden());
		match target {
			CommentTarget::Index(index) => {
				assert_eq!(index.to_string(), "idx_users_email");
			}
			_ => panic!("Expected CommentTarget::Index"),
		}
	}

	#[rstest]
	fn test_comment_target_view() {
		let target = CommentTarget::View("active_users".into_iden());
		match target {
			CommentTarget::View(view) => {
				assert_eq!(view.to_string(), "active_users");
			}
			_ => panic!("Expected CommentTarget::View"),
		}
	}

	#[rstest]
	fn test_comment_target_materialized_view() {
		let target = CommentTarget::MaterializedView("user_stats".into_iden());
		match target {
			CommentTarget::MaterializedView(view) => {
				assert_eq!(view.to_string(), "user_stats");
			}
			_ => panic!("Expected CommentTarget::MaterializedView"),
		}
	}

	#[rstest]
	fn test_comment_target_sequence() {
		let target = CommentTarget::Sequence("user_id_seq".into_iden());
		match target {
			CommentTarget::Sequence(seq) => {
				assert_eq!(seq.to_string(), "user_id_seq");
			}
			_ => panic!("Expected CommentTarget::Sequence"),
		}
	}

	#[rstest]
	fn test_comment_target_schema() {
		let target = CommentTarget::Schema("public".into_iden());
		match target {
			CommentTarget::Schema(schema) => {
				assert_eq!(schema.to_string(), "public");
			}
			_ => panic!("Expected CommentTarget::Schema"),
		}
	}

	#[rstest]
	fn test_comment_target_database() {
		let target = CommentTarget::Database("mydb".into_iden());
		match target {
			CommentTarget::Database(db) => {
				assert_eq!(db.to_string(), "mydb");
			}
			_ => panic!("Expected CommentTarget::Database"),
		}
	}

	#[rstest]
	fn test_comment_target_function() {
		let target = CommentTarget::Function("calculate_total".into_iden());
		match target {
			CommentTarget::Function(func) => {
				assert_eq!(func.to_string(), "calculate_total");
			}
			_ => panic!("Expected CommentTarget::Function"),
		}
	}

	#[rstest]
	fn test_comment_target_trigger() {
		let target = CommentTarget::Trigger("update_timestamp".into_iden(), "users".into_iden());
		match target {
			CommentTarget::Trigger(trigger, table) => {
				assert_eq!(trigger.to_string(), "update_timestamp");
				assert_eq!(table.to_string(), "users");
			}
			_ => panic!("Expected CommentTarget::Trigger"),
		}
	}

	#[rstest]
	fn test_comment_target_type() {
		let target = CommentTarget::Type("user_status".into_iden());
		match target {
			CommentTarget::Type(typ) => {
				assert_eq!(typ.to_string(), "user_status");
			}
			_ => panic!("Expected CommentTarget::Type"),
		}
	}
}
