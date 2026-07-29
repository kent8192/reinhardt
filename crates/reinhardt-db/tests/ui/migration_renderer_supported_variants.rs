use reinhardt_db::migrations::dependency::{
	DependencyCondition, OptionalDependency, SwappableDependency,
};
use reinhardt_db::migrations::prelude::*;
use reinhardt_db::migrations::{FieldType, IndexType};

fn migration() -> Migration {
	Migration {
		app_label: "accounts".to_string(),
		name: "0001_supported".to_string(),
		operations: vec![
			Operation::CreateTable {
				name: "accounts".to_string(),
				columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
				constraints: vec![Constraint::Check {
					name: "id_positive".to_string(),
					expression: "id > 0".to_string(),
				}],
				without_rowid: None,
				interleave_in_parent: None,
				partition: None,
			},
			Operation::CreateIndex {
				table: "accounts".to_string(),
				columns: vec!["id".to_string()],
				unique: true,
				index_type: Some(IndexType::BTree),
				where_clause: None,
				concurrently: false,
				expressions: None,
				mysql_options: None,
				operator_class: None,
			},
			Operation::RunSQL {
				sql: "SELECT 1".to_string(),
				reverse_sql: None,
			},
		],
		dependencies: vec![("auth".to_string(), "0001_initial".to_string())],
		replaces: vec![("accounts".to_string(), "0001_old".to_string())],
		atomic: true,
		initial: Some(false),
		state_only: false,
		database_only: false,
		swappable_dependencies: vec![SwappableDependency::new(
			"AUTH_USER_MODEL",
			"auth",
			"User",
			"0001_initial",
		)],
		optional_dependencies: vec![OptionalDependency::new(
			"audit",
			"0001_initial",
			DependencyCondition::FeatureEnabled("audit".to_string()),
		)],
	}
}

fn main() {
	let migration = migration();
	assert_eq!(migration.operations.len(), 3);
}
