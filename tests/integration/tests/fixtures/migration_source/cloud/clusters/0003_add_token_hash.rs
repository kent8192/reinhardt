use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;
pub fn migration() -> Migration {
	Migration {
		app_label: "clusters".to_string(),
		name: "0003_add_token_hash".to_string(),
		operations: vec![
			Operation::AddColumn {
				table: "clusters".to_string(),
				column: ColumnDefinition {
					name: "token_hash".to_string(),
					type_definition: FieldType::VarChar(255u32),
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
			Operation::AddColumn {
				table: "clusters".to_string(),
				column: ColumnDefinition {
					name: "token_last_rotated_at".to_string(),
					type_definition: FieldType::TimestampTz,
					not_null: false,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
		],
		dependencies: vec![("clusters".to_string(), "0002_add_user_id".to_string())],
		atomic: true,
		replaces: vec![],
		initial: Some(false),
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}
