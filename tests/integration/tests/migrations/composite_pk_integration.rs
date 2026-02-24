//! Tests for composite primary key support in migrations

use reinhardt_db::backends::schema::{BaseDatabaseSchemaEditor, SchemaEditorResult};
use reinhardt_db::migrations::FieldType;
use reinhardt_db::migrations::operations::FieldDefinition;
use reinhardt_db::migrations::operations::models::CreateModel;

/// Mock schema editor for testing SQL generation
struct MockSchemaEditor;

#[async_trait::async_trait]
impl BaseDatabaseSchemaEditor for MockSchemaEditor {
	async fn execute(&mut self, _sql: &str) -> SchemaEditorResult<()> {
		// Mock implementation - does nothing
		Ok(())
	}
}

#[test]
fn test_create_model_with_single_primary_key() {
	let create = CreateModel::new(
		"users",
		vec![
			FieldDefinition::new("id", FieldType::Integer, true, false, None::<&str>),
			FieldDefinition::new("name", FieldType::VarChar(100), false, false, None::<&str>),
		],
	);

	let schema_editor = MockSchemaEditor;
	let sql_statements = create.database_forwards(&schema_editor);

	assert_eq!(sql_statements.len(), 1);
	let sql = &sql_statements[0];

	println!("Generated SQL:\n{}", sql);

	assert!(sql.contains("CREATE TABLE"));
	// reinhardt-query may quote identifiers, so accept both forms
	assert!(sql.contains("id INTEGER PRIMARY KEY") || sql.contains("\"id\" INTEGER PRIMARY KEY"));
	assert!(sql.contains("name VARCHAR(100)") || sql.contains("\"name\" VARCHAR(100)"));
}

#[test]
fn test_create_model_with_composite_primary_key() {
	let create = CreateModel::new(
		"post_tags",
		vec![
			FieldDefinition::new("post_id", FieldType::Integer, true, false, None::<&str>),
			FieldDefinition::new("tag_id", FieldType::Integer, true, false, None::<&str>),
			FieldDefinition::new(
				"description",
				FieldType::VarChar(200),
				false,
				false,
				None::<&str>,
			),
		],
	)
	.with_composite_primary_key(vec!["post_id".to_string(), "tag_id".to_string()])
	.expect("Valid composite primary key");

	let schema_editor = MockSchemaEditor;
	let sql_statements = create.database_forwards(&schema_editor);

	assert_eq!(sql_statements.len(), 1);
	let sql = &sql_statements[0];

	assert!(sql.contains("CREATE TABLE"));
	assert!(sql.contains("post_tags") || sql.contains("\"post_tags\""));
	// Individual fields should NOT have PRIMARY KEY
	assert!(!sql.contains("post_id INTEGER PRIMARY KEY"));
	assert!(!sql.contains("tag_id INTEGER PRIMARY KEY"));
	// Should have NOT NULL for composite PK fields
	assert!(
		sql.contains("post_id INTEGER NOT NULL") || sql.contains("\"post_id\" INTEGER NOT NULL")
	);
	assert!(sql.contains("tag_id INTEGER NOT NULL") || sql.contains("\"tag_id\" INTEGER NOT NULL"));
	// Should have table-level PRIMARY KEY constraint
	// reinhardt-query may quote identifiers, so accept both forms
	assert!(
		sql.contains("PRIMARY KEY (post_id, tag_id)")
			|| sql.contains("PRIMARY KEY (\"post_id\", \"tag_id\")")
	);
}

#[test]
fn test_create_model_composite_pk_three_fields() {
	let create = CreateModel::new(
		"user_role_permission",
		vec![
			FieldDefinition::new("user_id", FieldType::Integer, true, false, None::<&str>),
			FieldDefinition::new("role_id", FieldType::Integer, true, false, None::<&str>),
			FieldDefinition::new(
				"permission_id",
				FieldType::Integer,
				true,
				false,
				None::<&str>,
			),
		],
	)
	.with_composite_primary_key(vec![
		"user_id".to_string(),
		"role_id".to_string(),
		"permission_id".to_string(),
	])
	.expect("Valid composite primary key");

	let schema_editor = MockSchemaEditor;
	let sql_statements = create.database_forwards(&schema_editor);

	let sql = &sql_statements[0];
	// reinhardt-query may quote identifiers, so accept both forms
	assert!(
		sql.contains("PRIMARY KEY (user_id, role_id, permission_id)")
			|| sql.contains("PRIMARY KEY (\"user_id\", \"role_id\", \"permission_id\")")
	);
}

#[test]
fn test_create_model_composite_pk_with_additional_fields() {
	let create = CreateModel::new(
		"order_items",
		vec![
			FieldDefinition::new("order_id", FieldType::Integer, true, false, None::<&str>),
			FieldDefinition::new("item_id", FieldType::Integer, true, false, None::<&str>),
			FieldDefinition::new("quantity", FieldType::Integer, false, false, Some("1")),
			FieldDefinition::new(
				"price",
				FieldType::Decimal {
					precision: 10,
					scale: 2,
				},
				false,
				false,
				None::<&str>,
			),
		],
	)
	.with_composite_primary_key(vec!["order_id".to_string(), "item_id".to_string()])
	.expect("Valid composite primary key");

	let schema_editor = MockSchemaEditor;
	let sql_statements = create.database_forwards(&schema_editor);

	let sql = &sql_statements[0];
	// reinhardt-query may quote identifiers, so accept both forms
	assert!(
		sql.contains("PRIMARY KEY (order_id, item_id)")
			|| sql.contains("PRIMARY KEY (\"order_id\", \"item_id\")")
	);
	assert!(
		sql.contains("quantity INTEGER NOT NULL DEFAULT 1")
			|| sql.contains("\"quantity\" INTEGER NOT NULL DEFAULT 1")
	);
	assert!(
		sql.contains("price DECIMAL(10, 2) NOT NULL")
			|| sql.contains("\"price\" DECIMAL(10, 2) NOT NULL")
	);
}
