#![cfg(feature = "sqlite")]

use async_trait::async_trait;
use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	ColumnDefinition, FieldType, Migration, MigrationCatalog, MigrationDirection, MigrationError,
	MigrationKey, MigrationSource, Operation, ProjectState, Result, SqlDialect, plan_migration_sql,
	plan_migration_sql_with_states,
};

struct TestSource {
	migrations: Vec<Migration>,
}

#[async_trait]
impl MigrationSource for TestSource {
	async fn all_migrations(&self) -> Result<Vec<Migration>> {
		Ok(self.migrations.clone())
	}
}

fn create_table(app: &str, name: &str, table: &str) -> Migration {
	Migration::new(name, app).add_operation(Operation::CreateTable {
		name: table.to_string(),
		columns: vec![ColumnDefinition::new("id", FieldType::Integer)],
		constraints: Vec::new(),
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	})
}

fn add_column(
	app: &str,
	name: &str,
	dependencies: &[(&str, &str)],
	table: &str,
	column: &str,
) -> Migration {
	let mut migration = Migration::new(name, app).add_operation(Operation::AddColumn {
		table: table.to_string(),
		column: ColumnDefinition::new(column, FieldType::Text),
		mysql_options: None,
	});
	migration.dependencies = dependencies
		.iter()
		.map(|(dependency_app, dependency_name)| {
			(
				(*dependency_app).to_string(),
				(*dependency_name).to_string(),
			)
		})
		.collect();
	migration
}

#[tokio::test]
async fn state_before_replays_only_target_ancestors() {
	let initial = create_table("catalog", "0001_initial", "books");
	let title = add_column(
		"catalog",
		"0002_title",
		&[("catalog", "0001_initial")],
		"books",
		"title",
	);
	let unrelated = create_table("audit", "0001_initial", "audit_entries");
	let catalog = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![title, unrelated, initial],
	})
	.await
	.unwrap();

	let state = catalog
		.state_before(&MigrationKey::new("catalog", "0002_title"))
		.unwrap();

	let books = state
		.models
		.get(&("catalog".to_string(), "books".to_string()))
		.expect("the ancestor should create books");
	assert_eq!(books.fields.keys().collect::<Vec<_>>(), vec!["id"]);
	assert_eq!(state.models.len(), 1);
}

#[tokio::test]
async fn state_after_includes_the_target_migration() {
	let initial = create_table("catalog", "0001_initial", "books");
	let title = add_column(
		"catalog",
		"0002_title",
		&[("catalog", "0001_initial")],
		"books",
		"title",
	);
	let catalog = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![title, initial],
	})
	.await
	.unwrap();

	let state = catalog
		.state_after(&MigrationKey::new("catalog", "0002_title"))
		.unwrap();

	let books = state
		.models
		.get(&("catalog".to_string(), "books".to_string()))
		.expect("the target should retain books");
	assert_eq!(books.fields.keys().collect::<Vec<_>>(), vec!["id", "title"]);
}

#[tokio::test]
async fn merge_state_replays_both_dependency_branches_once() {
	let initial = create_table("catalog", "0001_initial", "books");
	let title = add_column(
		"catalog",
		"0002_title",
		&[("catalog", "0001_initial")],
		"books",
		"title",
	);
	let summary = add_column(
		"catalog",
		"0002_summary",
		&[("catalog", "0001_initial")],
		"books",
		"summary",
	);
	let published = add_column(
		"catalog",
		"0003_merge",
		&[("catalog", "0002_title"), ("catalog", "0002_summary")],
		"books",
		"published",
	);
	let catalog = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![published, summary, title, initial],
	})
	.await
	.unwrap();

	let before = catalog
		.state_before(&MigrationKey::new("catalog", "0003_merge"))
		.unwrap();
	let before_fields = &before
		.models
		.get(&("catalog".to_string(), "books".to_string()))
		.unwrap()
		.fields;
	assert_eq!(
		before_fields.keys().collect::<Vec<_>>(),
		vec!["id", "summary", "title"]
	);

	let after = catalog
		.state_after(&MigrationKey::new("catalog", "0003_merge"))
		.unwrap();
	let after_fields = &after
		.models
		.get(&("catalog".to_string(), "books".to_string()))
		.unwrap()
		.fields;
	assert_eq!(
		after_fields.keys().collect::<Vec<_>>(),
		vec!["id", "published", "summary", "title"]
	);
}

#[tokio::test]
async fn database_only_migration_does_not_change_reconstructed_project_state() {
	let mut migration = create_table("catalog", "0001_database_only", "books");
	migration.database_only = true;
	let catalog = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![migration],
	})
	.await
	.unwrap();

	let state = catalog
		.state_after(&MigrationKey::new("catalog", "0001_database_only"))
		.unwrap();

	assert!(state.models.is_empty());
}

#[tokio::test]
async fn rendered_sql_uses_effective_atomic_ddl_and_consistent_terminators() {
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.unwrap();
	let mut migration = Migration::new("0001_initial", "catalog");
	migration.operations = vec![
		Operation::RunSQL {
			sql: "CREATE TABLE books (id INTEGER);".to_string(),
			reverse_sql: Some("DROP TABLE books;".to_string()),
		},
		Operation::RunRust {
			code: "seed_books();".to_string(),
			reverse_code: Some("clear_books();".to_string()),
		},
	];
	let plan = plan_migration_sql(
		&connection,
		&migration,
		&ProjectState::new(),
		MigrationDirection::Forward,
	)
	.await
	.unwrap();

	assert_eq!(
		plan.render(SqlDialect::Postgres),
		"BEGIN;\nCREATE TABLE books (id INTEGER);\n-- RunRust: seed_books();\nCOMMIT;\n"
	);
	assert_eq!(
		plan.render(SqlDialect::Mysql),
		"CREATE TABLE books (id INTEGER);\n-- RunRust: seed_books();\n"
	);
	assert_eq!(
		plan.render(SqlDialect::Sqlite),
		"BEGIN;\nCREATE TABLE books (id INTEGER);\n-- RunRust: seed_books();\nCOMMIT;\n"
	);
}

#[tokio::test]
async fn non_atomic_plan_never_renders_transaction_wrappers() {
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.unwrap();
	let migration = Migration::new("0001_non_atomic", "catalog")
		.atomic(false)
		.add_operation(Operation::RunSQL {
			sql: "SELECT 1".to_string(),
			reverse_sql: Some("SELECT 2".to_string()),
		});
	let plan = plan_migration_sql(
		&connection,
		&migration,
		&ProjectState::new(),
		MigrationDirection::Forward,
	)
	.await
	.unwrap();

	assert_eq!(plan.render(SqlDialect::Postgres), "SELECT 1;\n");
}

#[tokio::test]
async fn backward_drop_table_uses_the_catalog_target_state() {
	let mut id = ColumnDefinition::new("id", FieldType::Integer);
	id.not_null = true;
	let initial = Migration::new("0001_initial", "catalog").add_operation(Operation::CreateTable {
		name: "books".to_string(),
		columns: vec![id],
		constraints: Vec::new(),
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	});
	let mut drop_books =
		Migration::new("0002_drop_books", "catalog").add_operation(Operation::DropTable {
			name: "books".to_string(),
		});
	drop_books.dependencies = vec![("catalog".to_string(), "0001_initial".to_string())];
	let catalog = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![drop_books.clone(), initial],
	})
	.await
	.unwrap();
	let state_after = catalog
		.state_after(&MigrationKey::new("catalog", "0002_drop_books"))
		.unwrap();
	let state_before = catalog
		.state_before(&MigrationKey::new("catalog", "0002_drop_books"))
		.unwrap();
	assert!(state_after.models.is_empty());
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.unwrap();

	let plan = plan_migration_sql_with_states(
		&connection,
		&drop_books,
		&state_before,
		&state_after,
		MigrationDirection::Backward,
	)
	.await
	.unwrap();

	assert_eq!(
		plan.render(SqlDialect::Mysql),
		"CREATE TABLE books (\n  id INTEGER NOT NULL\n);\n"
	);
}

#[tokio::test]
async fn state_after_only_drop_table_error_points_to_the_two_state_api() {
	let initial = create_table("catalog", "0001_initial", "books");
	let mut drop_books =
		Migration::new("0002_drop_books", "catalog").add_operation(Operation::DropTable {
			name: "books".to_string(),
		});
	drop_books.dependencies = vec![("catalog".to_string(), "0001_initial".to_string())];
	let catalog = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![drop_books.clone(), initial],
	})
	.await
	.unwrap();
	let state_after = catalog
		.state_after(&MigrationKey::new("catalog", "0002_drop_books"))
		.unwrap();
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.unwrap();

	let error = plan_migration_sql(
		&connection,
		&drop_books,
		&state_after,
		MigrationDirection::Backward,
	)
	.await
	.unwrap_err();

	assert!(matches!(error, MigrationError::IrreversibleError(_)));
	assert_eq!(
		error.to_string(),
		"Irreversible migration: catalog.0002_drop_books requires pre-operation project state; use plan_migration_sql_with_states"
	);
}

#[tokio::test]
async fn backward_legacy_drop_column_uses_its_pre_operation_definition() {
	let mut initial = create_table("catalog", "0001_initial", "books");
	initial.operations = vec![Operation::CreateTable {
		name: "books".to_string(),
		columns: vec![
			ColumnDefinition::new("id", FieldType::Integer),
			ColumnDefinition::new("title", FieldType::Text),
		],
		constraints: Vec::new(),
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	}];
	let mut drop_title =
		Migration::new("0002_drop_title", "catalog").add_operation(Operation::DropColumn {
			table: "books".to_string(),
			column: "title".to_string(),
			old_definition: None,
		});
	drop_title.dependencies = vec![("catalog".to_string(), "0001_initial".to_string())];
	let catalog = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![drop_title.clone(), initial],
	})
	.await
	.unwrap();
	let state_after = catalog
		.state_after(&MigrationKey::new("catalog", "0002_drop_title"))
		.unwrap();
	let state_before = catalog
		.state_before(&MigrationKey::new("catalog", "0002_drop_title"))
		.unwrap();
	let books = state_after.find_model_by_table("books").unwrap();
	assert_eq!(books.fields.keys().collect::<Vec<_>>(), vec!["id"]);
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.unwrap();

	let plan = plan_migration_sql_with_states(
		&connection,
		&drop_title,
		&state_before,
		&state_after,
		MigrationDirection::Backward,
	)
	.await
	.unwrap();

	assert!(plan.render(SqlDialect::Mysql).contains("title TEXT"));
}

#[tokio::test]
async fn backward_legacy_alter_column_uses_its_pre_operation_definition() {
	let initial = create_table("catalog", "0001_initial", "books");
	let mut alter_id =
		Migration::new("0002_alter_id", "catalog").add_operation(Operation::AlterColumn {
			table: "books".to_string(),
			column: "id".to_string(),
			old_definition: None,
			new_definition: ColumnDefinition::new("id", FieldType::Text),
			mysql_options: None,
		});
	alter_id.dependencies = vec![("catalog".to_string(), "0001_initial".to_string())];
	let catalog = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![alter_id.clone(), initial],
	})
	.await
	.unwrap();
	let state_before = catalog
		.state_before(&MigrationKey::new("catalog", "0002_alter_id"))
		.unwrap();
	let state_after = catalog
		.state_after(&MigrationKey::new("catalog", "0002_alter_id"))
		.unwrap();
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.unwrap();
	connection
		.execute("CREATE TABLE books (id TEXT NOT NULL)", vec![])
		.await
		.unwrap();

	let plan = plan_migration_sql_with_states(
		&connection,
		&alter_id,
		&state_before,
		&state_after,
		MigrationDirection::Backward,
	)
	.await
	.unwrap();

	assert!(plan.render(SqlDialect::Sqlite).contains("id INTEGER"));
}

#[tokio::test]
async fn two_state_planner_rejects_a_mismatched_target_state() {
	let initial = create_table("catalog", "0001_initial", "books");
	let title = add_column(
		"catalog",
		"0002_title",
		&[("catalog", "0001_initial")],
		"books",
		"title",
	);
	let catalog = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![title.clone(), initial],
	})
	.await
	.unwrap();
	let state_before = catalog
		.state_before(&MigrationKey::new("catalog", "0002_title"))
		.unwrap();
	let mismatched_after = ProjectState::new();
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.unwrap();

	let error = plan_migration_sql_with_states(
		&connection,
		&title,
		&state_before,
		&mismatched_after,
		MigrationDirection::Backward,
	)
	.await
	.unwrap_err();

	assert!(matches!(error, MigrationError::InvalidMigration(_)));
	assert_eq!(
		error.to_string(),
		"Invalid migration: catalog.0002_title state_after does not match state_before replay"
	);
}

#[tokio::test]
async fn late_irreversible_rollback_returns_no_partial_plan_to_render() {
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.unwrap();
	let mut migration = Migration::new("0002_rollback", "catalog");
	migration.operations = vec![
		Operation::RunSQL {
			sql: "DELETE FROM books".to_string(),
			reverse_sql: None,
		},
		Operation::RunSQL {
			sql: "UPDATE books SET title = 'new'".to_string(),
			reverse_sql: Some("UPDATE books SET title = 'old'".to_string()),
		},
	];

	let error = plan_migration_sql(
		&connection,
		&migration,
		&ProjectState::new(),
		MigrationDirection::Backward,
	)
	.await
	.unwrap_err();

	assert!(matches!(error, MigrationError::IrreversibleError(_)));
	assert_eq!(
		error.to_string(),
		"Irreversible migration: catalog.0002_rollback contains an irreversible RunSQL operation"
	);
}

#[tokio::test]
async fn unknown_target_returns_an_error_instead_of_a_partial_state() {
	let catalog = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![create_table("catalog", "0001_initial", "books")],
	})
	.await
	.unwrap();

	let error = catalog
		.state_after(&MigrationKey::new("catalog", "9999_missing"))
		.unwrap_err();

	assert!(matches!(error, MigrationError::NotFound(_)));
	assert_eq!(
		error.to_string(),
		"Migration not found: catalog.9999_missing"
	);
}
