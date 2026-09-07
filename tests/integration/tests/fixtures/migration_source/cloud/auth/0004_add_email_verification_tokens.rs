use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;
pub fn migration() -> Migration {
	Migration {
		app_label: "auth".to_string(),
		name: "0004_add_email_verification_tokens".to_string(),
		operations: vec![
			Operation::CreateTable {
				name: "auth_email_verification_tokens".to_string(),
				columns: vec![
					ColumnDefinition {
						name: "id".to_string(),
						type_definition: FieldType::BigInteger,
						not_null: true,
						unique: false,
						primary_key: true,
						auto_increment: true,
						default: None,
					},
					ColumnDefinition {
						name: "user_id".to_string(),
						type_definition: FieldType::Uuid,
						not_null: true,
						unique: false,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
					ColumnDefinition {
						name: "pending_email".to_string(),
						type_definition: FieldType::VarChar(254u32),
						not_null: true,
						unique: false,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
					ColumnDefinition {
						name: "token_hash".to_string(),
						type_definition: FieldType::VarChar(64u32),
						not_null: true,
						unique: true,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
					ColumnDefinition {
						name: "expires_at".to_string(),
						type_definition: FieldType::TimestampTz,
						not_null: true,
						unique: false,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
					ColumnDefinition {
						name: "consumed_at".to_string(),
						type_definition: FieldType::TimestampTz,
						not_null: false,
						unique: false,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
					ColumnDefinition {
						name: "created_at".to_string(),
						type_definition: FieldType::TimestampTz,
						not_null: true,
						unique: false,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
				],
				constraints: vec![Constraint::Unique {
					name: "auth_evt_token_hash_uniq".to_string(),
					columns: vec!["token_hash".to_string()],
				}],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			// Partial index to accelerate the per-user "is there an active
			// pending token?" lookup performed when issuing a new token
			// (which invalidates priors). The lookup by `token_hash` is
			// already covered by the column-level UNIQUE constraint, so no
			// additional index is needed for verify-time lookups.
			//
			// We deliberately do NOT add a UNIQUE on (user_id, token_hash):
			// `token_hash` is already globally unique, so a second uniqueness
			// rule would be redundant. A non-unique partial index over
			// unconsumed rows is what actually matches the query pattern.
			Operation::CreateIndex {
				table: "auth_email_verification_tokens".to_string(),
				columns: vec!["user_id".to_string()],
				unique: false,
				index_type: None,
				where_clause: Some("consumed_at IS NULL".to_string()),
				concurrently: false,
				expressions: None,
				mysql_options: None,
				operator_class: None,
			},
		],
		dependencies: vec![("auth".to_string(), "0003_initial".to_string())],
		atomic: true,
		replaces: vec![],
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}
