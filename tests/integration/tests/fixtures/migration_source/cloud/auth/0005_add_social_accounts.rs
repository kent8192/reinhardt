use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;
pub fn migration() -> Migration {
	Migration {
		app_label: "auth".to_string(),
		name: "0005_add_social_accounts".to_string(),
		operations: vec![
			Operation::CreateTable {
				name: "auth_social_accounts".to_string(),
				columns: vec![
					ColumnDefinition {
						name: "id".to_string(),
						type_definition: FieldType::Uuid,
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
						name: "provider".to_string(),
						type_definition: FieldType::VarChar(32u32),
						not_null: true,
						unique: false,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
					ColumnDefinition {
						name: "provider_user_id".to_string(),
						type_definition: FieldType::VarChar(255u32),
						not_null: true,
						unique: false,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
					ColumnDefinition {
						name: "provider_username".to_string(),
						type_definition: FieldType::VarChar(255u32),
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
					ColumnDefinition {
						name: "updated_at".to_string(),
						type_definition: FieldType::TimestampTz,
						not_null: true,
						unique: false,
						primary_key: false,
						auto_increment: false,
						default: None,
					},
				],
				// `(provider, provider_user_id)` is globally unique so the same
				// external identity cannot be claimed by two local users.
				constraints: vec![Constraint::Unique {
					name: "auth_social_account_provider_uid_uniq".to_string(),
					columns: vec!["provider".to_string(), "provider_user_id".to_string()],
				}],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			// Supporting index for the unlink and "list user's linked
			// providers" lookups, both of which filter by `user_id`.
			Operation::CreateIndex {
				table: "auth_social_accounts".to_string(),
				columns: vec!["user_id".to_string()],
				unique: false,
				index_type: None,
				where_clause: None,
				concurrently: false,
				expressions: None,
				mysql_options: None,
				operator_class: None,
			},
		],
		dependencies: vec![(
			"auth".to_string(),
			"0004_add_email_verification_tokens".to_string(),
		)],
		atomic: true,
		replaces: vec![],
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}
