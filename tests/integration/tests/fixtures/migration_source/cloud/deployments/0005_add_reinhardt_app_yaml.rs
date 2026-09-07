use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;

pub fn migration() -> Migration {
	Migration {
		app_label: "deployments".to_string(),
		name: "0005_add_reinhardt_app_yaml".to_string(),
		operations: vec![Operation::AddColumn {
			table: "deployments".to_string(),
			column: ColumnDefinition {
				name: "reinhardt_app_yaml".to_string(),
				type_definition: FieldType::VarChar(65535u32),
				not_null: false,
				unique: false,
				primary_key: false,
				auto_increment: false,
				default: None,
			},
			mysql_options: None,
		}],
		dependencies: vec![(
			"deployments".to_string(),
			"0004_replace_user_id_with_organization_id".to_string(),
		)],
		atomic: true,
		replaces: vec![],
		initial: Some(false),
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}
