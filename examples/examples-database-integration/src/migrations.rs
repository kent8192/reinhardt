//! Migrations for database integration example
//!
//! Provides migrations for users and todos tables using the MigrationProvider trait.

use reinhardt::db::migrations::{
	ColumnDefinition, FieldType, Migration, MigrationProvider, Operation,
};

/// Migration provider for the database integration example
///
/// Provides migrations for:
/// - users table (users app)
/// - todos table (todos app)
pub struct ExampleMigrations;

impl MigrationProvider for ExampleMigrations {
	fn migrations() -> Vec<Migration> {
		vec![
			Migration {
				app_label: "users".to_string(),
				name: "0001_initial".to_string(),
				operations: vec![Operation::CreateTable {
					name: "users".to_string(),
					columns: vec![
						ColumnDefinition {
							name: "id".to_string(),
							type_definition: FieldType::BigInteger,
							not_null: true,
							unique: false,
							primary_key: true,
							auto_increment: true,
							default: None,
						},
						ColumnDefinition {
							name: "name".to_string(),
							type_definition: FieldType::VarChar(255),
							not_null: true,
							unique: false,
							primary_key: false,
							auto_increment: false,
							default: None,
						},
						ColumnDefinition {
							name: "email".to_string(),
							type_definition: FieldType::VarChar(255),
							not_null: true,
							unique: true,
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
					],
					constraints: vec![],
					without_rowid: None,
					interleave_in_parent: None,
					partition: None,
				}],
				dependencies: vec![],
				replaces: vec![],
				atomic: true,
				initial: Some(true),
				state_only: false,
				database_only: false,
				optional_dependencies: vec![],
				swappable_dependencies: vec![],
			},
			Migration {
				app_label: "todos".to_string(),
				name: "0001_initial".to_string(),
				operations: vec![Operation::CreateTable {
					name: "todos".to_string(),
					columns: vec![
						ColumnDefinition {
							name: "id".to_string(),
							type_definition: FieldType::BigInteger,
							not_null: true,
							unique: false,
							primary_key: true,
							auto_increment: true,
							default: None,
						},
						ColumnDefinition {
							name: "title".to_string(),
							type_definition: FieldType::VarChar(255),
							not_null: true,
							unique: false,
							primary_key: false,
							auto_increment: false,
							default: None,
						},
						ColumnDefinition {
							name: "description".to_string(),
							type_definition: FieldType::Text,
							not_null: false,
							unique: false,
							primary_key: false,
							auto_increment: false,
							default: None,
						},
						ColumnDefinition {
							name: "completed".to_string(),
							type_definition: FieldType::Boolean,
							not_null: true,
							unique: false,
							primary_key: false,
							auto_increment: false,
							default: Some("false".to_string()),
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
					constraints: vec![],
					without_rowid: None,
					interleave_in_parent: None,
					partition: None,
				}],
				dependencies: vec![],
				replaces: vec![],
				atomic: true,
				initial: Some(true),
				state_only: false,
				database_only: false,
				optional_dependencies: vec![],
				swappable_dependencies: vec![],
			},
		]
	}
}
