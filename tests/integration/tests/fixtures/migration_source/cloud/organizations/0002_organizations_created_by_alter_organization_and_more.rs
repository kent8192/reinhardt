use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;
pub fn migration() -> Migration {
	Migration {
		app_label: "organizations".to_string(),
		name: "0002_organizations_created_by_alter_organization_and_more".to_string(),
		operations: vec![
			Operation::AddColumn {
				table: "organizations".to_string(),
				column: ColumnDefinition {
					name: "created_by".to_string(),
					type_definition: FieldType::Uuid,
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
			Operation::AlterColumn {
				table: "organizations".to_string(),
				column: "id".to_string(),
				old_definition: None,
				new_definition: ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::BigInteger,
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: true,
					default: None,
				},
				mysql_options: None,
			},
			Operation::AlterColumn {
				table: "organization_memberships".to_string(),
				column: "id".to_string(),
				old_definition: None,
				new_definition: ColumnDefinition {
					name: "id".to_string(),
					type_definition: FieldType::BigInteger,
					not_null: true,
					unique: false,
					primary_key: true,
					auto_increment: true,
					default: None,
				},
				mysql_options: None,
			},
		],
		dependencies: vec![("organizations".to_string(), "0001_initial".to_string())],
		atomic: true,
		replaces: vec![],
		initial: None,
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![],
		optional_dependencies: vec![],
	}
}
