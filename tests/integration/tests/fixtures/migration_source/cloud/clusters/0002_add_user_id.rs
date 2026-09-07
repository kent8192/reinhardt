use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;
pub fn migration() -> Migration {
	Migration {
		app_label: "clusters".to_string(),
		name: "0002_add_user_id".to_string(),
		operations: vec![Operation::AddColumn {
			table: "clusters".to_string(),
			column: ColumnDefinition {
				name: "user_id".to_string(),
				type_definition: FieldType::Uuid,
				not_null: true,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
			},
			mysql_options: None,
		}],
		dependencies: vec![
			("clusters".to_string(), "0001_initial".to_string()),
			("auth".to_string(), "0001_initial".to_string()),
		],
		atomic: true,
		replaces: vec![],
		initial: Some(false),
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}
