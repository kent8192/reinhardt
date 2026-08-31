// reinhardt-migration-source: 1
use reinhardt::db::migrations::FieldType;
use reinhardt::db::migrations::prelude::*;
pub(super) fn migration() -> Migration {
	Migration::new("0001_initial".to_string(), "auth".to_string())
		.add_operation(Operation::CreateTable {
			name: "auth_permission".to_string(),
			columns: vec![
				ColumnDefinition::new("app_label".to_string(), FieldType::VarChar(100u32))
					.with_not_null(true)
					.with_unique(false)
					.with_primary_key(false)
					.with_auto_increment(false)
					.with_default(None)
					.with_generated(None)
					.with_domain_option(None),
				ColumnDefinition::new("codename".to_string(), FieldType::VarChar(100u32))
					.with_not_null(true)
					.with_unique(false)
					.with_primary_key(false)
					.with_auto_increment(false)
					.with_default(None)
					.with_generated(None)
					.with_domain_option(None),
				ColumnDefinition::new("id".to_string(), FieldType::Uuid)
					.with_not_null(true)
					.with_unique(false)
					.with_primary_key(true)
					.with_auto_increment(false)
					.with_default(None)
					.with_generated(None)
					.with_domain_option(None),
				ColumnDefinition::new("name".to_string(), FieldType::VarChar(255u32))
					.with_not_null(true)
					.with_unique(false)
					.with_primary_key(false)
					.with_auto_increment(false)
					.with_default(None)
					.with_generated(None)
					.with_domain_option(None),
			],
			constraints: vec![],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		})
		.add_operation(Operation::CreateTable {
			name: "auth_group".to_string(),
			columns: vec![
				ColumnDefinition::new("description".to_string(), FieldType::VarChar(500u32))
					.with_not_null(false)
					.with_unique(false)
					.with_primary_key(false)
					.with_auto_increment(false)
					.with_default(None)
					.with_generated(None)
					.with_domain_option(None),
				ColumnDefinition::new("id".to_string(), FieldType::Uuid)
					.with_not_null(true)
					.with_unique(false)
					.with_primary_key(true)
					.with_auto_increment(false)
					.with_default(None)
					.with_generated(None)
					.with_domain_option(None),
				ColumnDefinition::new("name".to_string(), FieldType::VarChar(150u32))
					.with_not_null(true)
					.with_unique(true)
					.with_primary_key(false)
					.with_auto_increment(false)
					.with_default(None)
					.with_generated(None)
					.with_domain_option(None),
			],
			constraints: vec![Constraint::Unique {
				name: "auth_group_name_uniq".to_string(),
				columns: vec!["name".to_string()],
			}],
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		})
		.atomic(true)
		.with_initial(Some(true))
		.state_only(false)
		.database_only(false)
}
