use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;
pub fn migration() -> Migration {
	Migration {
		app_label: "clusters".to_string(),
		name: "0004_replace_user_id_with_organization_id".to_string(),
		operations: vec![
			Operation::DropColumn {
				table: "clusters".to_string(),
				column: "user_id".to_string(),
			},
			Operation::AddColumn {
				table: "clusters".to_string(),
				column: ColumnDefinition {
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
			Operation::AddConstraint {
				table: "clusters".to_string(),
				constraint_sql: "CONSTRAINT clusters_organization_id_fk FOREIGN KEY (organization_id) REFERENCES organizations(id) ON DELETE CASCADE".to_string(),
			},
		],
		dependencies: vec![
			("clusters".to_string(), "0003_add_token_hash".to_string()),
			("organizations".to_string(), "0001_initial".to_string()),
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
