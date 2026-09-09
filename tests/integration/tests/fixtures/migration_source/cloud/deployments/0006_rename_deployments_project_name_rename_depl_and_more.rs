use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;
pub(super) fn migration() -> Migration {
	Migration {
		app_label: "deployments".to_string(),
		name: "0006_rename_deployments_project_name_rename_depl_and_more".to_string(),
		operations: vec![
			Operation::RenameColumn {
				table: "deployments".to_string(),
				old_name: "app_name".to_string(),
				new_name: "project_name".to_string(),
			},
			Operation::RenameColumn {
				table: "deployments".to_string(),
				old_name: "reinhardt_app_yaml".to_string(),
				new_name: "project_yaml".to_string(),
			},
			Operation::AlterColumn {
				table: "deployments".to_string(),
				column: "cluster_id".to_string(),
				old_definition: None,
				new_definition: ColumnDefinition {
					name: "cluster_id".to_string(),
					type_definition: FieldType::BigInteger,
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
			Operation::AlterColumn {
				table: "deployments".to_string(),
				column: "organization_id".to_string(),
				old_definition: None,
				new_definition: ColumnDefinition {
					name: "organization_id".to_string(),
					type_definition: FieldType::BigInteger,
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: None,
				},
				mysql_options: None,
			},
			Operation::AlterColumn {
				table: "deployments".to_string(),
				column: "status".to_string(),
				old_definition: None,
				new_definition: ColumnDefinition {
					name: "status".to_string(),
					type_definition: FieldType::VarChar(50u32),
					not_null: true,
					unique: false,
					primary_key: false,
					auto_increment: false,
					default: Some("'pending'".to_string()),
				},
				mysql_options: None,
			},
		],
		dependencies: vec![(
			"deployments".to_string(),
			"0005_add_reinhardt_app_yaml".to_string(),
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
