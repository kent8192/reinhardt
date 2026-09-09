use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		app_label: "auth".to_string(),
		name: "0006_add_social_account_token_metadata".to_string(),
		operations: vec![
			Operation::AddColumn {
				table: "auth_social_accounts".to_string(),
				column: ColumnDefinition {
					name: "encrypted_access_token".to_string(),
					type_definition: FieldType::VarChar(4096u32),
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
			Operation::AddColumn {
				table: "auth_social_accounts".to_string(),
				column: ColumnDefinition {
					name: "token_expires_at".to_string(),
					type_definition: FieldType::TimestampTz,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
			Operation::AddColumn {
				table: "auth_social_accounts".to_string(),
				column: ColumnDefinition {
					name: "scopes".to_string(),
					type_definition: FieldType::VarChar(2048u32),
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
		],
		dependencies: vec![("auth".to_string(), "0005_add_social_accounts".to_string())],
		atomic: true,
		replaces: vec![],
		initial: Some(false),
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}
