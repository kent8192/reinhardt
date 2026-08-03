#![cfg(feature = "sqlite")]

use std::sync::Arc;

use async_trait::async_trait;
use reinhardt_db::backends::{
	DatabaseBackend, DatabaseConnection, DatabaseError, DatabaseErrorKind, DatabaseType,
	QueryResult, QueryValue, Row, types::TransactionExecutor,
};
use reinhardt_db::migrations::{
	ColumnDefinition, DatabaseMigrationExecutor, FieldType, Migration, MigrationDirection,
	MigrationError, Operation, PlannedStatement, ProjectState, plan_migration_sql,
};

async fn sqlite_connection() -> DatabaseConnection {
	DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("connect to SQLite")
}

struct PlanningBackend(DatabaseType);

#[async_trait]
impl DatabaseBackend for PlanningBackend {
	fn database_type(&self) -> DatabaseType {
		self.0
	}

	fn placeholder(&self, index: usize) -> String {
		match self.0 {
			DatabaseType::Postgres => format!("${index}"),
			DatabaseType::Mysql | DatabaseType::Sqlite => "?".to_string(),
		}
	}

	fn supports_returning(&self) -> bool {
		!matches!(self.0, DatabaseType::Mysql)
	}

	fn supports_on_conflict(&self) -> bool {
		true
	}

	async fn execute(
		&self,
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> reinhardt_db::backends::Result<QueryResult> {
		Err(DatabaseError::new(
			DatabaseErrorKind::Unsupported,
			"planning backend does not execute SQL",
		)
		.into())
	}

	async fn fetch_one(
		&self,
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> reinhardt_db::backends::Result<Row> {
		Err(DatabaseError::new(
			DatabaseErrorKind::Unsupported,
			"planning backend does not fetch rows",
		)
		.into())
	}

	async fn fetch_all(
		&self,
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> reinhardt_db::backends::Result<Vec<Row>> {
		Err(DatabaseError::new(
			DatabaseErrorKind::Unsupported,
			"planning backend does not fetch rows",
		)
		.into())
	}

	async fn fetch_optional(
		&self,
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> reinhardt_db::backends::Result<Option<Row>> {
		Err(DatabaseError::new(
			DatabaseErrorKind::Unsupported,
			"planning backend does not fetch rows",
		)
		.into())
	}

	async fn begin(&self) -> reinhardt_db::backends::Result<Box<dyn TransactionExecutor>> {
		Err(DatabaseError::new(
			DatabaseErrorKind::Unsupported,
			"planning backend does not begin transactions",
		)
		.into())
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}

fn sql(statements: &[PlannedStatement]) -> Vec<&str> {
	statements
		.iter()
		.filter_map(|statement| match statement {
			PlannedStatement::Sql(sql) => Some(sql.as_str()),
			PlannedStatement::Comment(_) => None,
		})
		.collect()
}

#[tokio::test]
async fn forward_plan_preserves_operation_and_statement_order() {
	let connection = sqlite_connection().await;
	let mut migration = Migration::new("0001_initial", "catalog");
	migration.operations = vec![
		Operation::RunSQL {
			sql: "CREATE TABLE \"events\" (\"id\" INTEGER); INSERT INTO \"events\" VALUES (1);"
				.to_string(),
			reverse_sql: Some("DROP TABLE \"events\";".to_string()),
		},
		Operation::RenameTable {
			old_name: "event log".to_string(),
			new_name: "audit log".to_string(),
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

	assert!(plan.atomic);
	assert_eq!(
		sql(&plan.statements),
		vec![
			"CREATE TABLE \"events\" (\"id\" INTEGER)",
			"INSERT INTO \"events\" VALUES (1)",
			"ALTER TABLE \"event log\" RENAME TO \"audit log\""
		]
	);
}

#[tokio::test]
async fn planner_uses_backend_specific_identifier_quoting() {
	let mut migration = Migration::new("0001_quoted", "catalog");
	migration.operations.push(Operation::CreateTable {
		name: "order history".to_string(),
		columns: vec![ColumnDefinition::new("select", FieldType::Integer)],
		constraints: Vec::new(),
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	});

	let postgres = plan_migration_sql(
		&DatabaseConnection::new(Arc::new(PlanningBackend(DatabaseType::Postgres))),
		&migration,
		&ProjectState::new(),
		MigrationDirection::Forward,
	)
	.await
	.unwrap();
	let mysql = plan_migration_sql(
		&DatabaseConnection::new(Arc::new(PlanningBackend(DatabaseType::Mysql))),
		&migration,
		&ProjectState::new(),
		MigrationDirection::Forward,
	)
	.await
	.unwrap();

	assert_eq!(
		sql(&postgres.statements),
		vec!["CREATE TABLE \"order history\" (\n  \"select\" INTEGER\n)"]
	);
	assert_eq!(
		sql(&mysql.statements),
		vec!["CREATE TABLE `order history` (\n  `select` INTEGER\n)"]
	);
}

#[tokio::test]
async fn backward_plan_reverses_operation_order_and_each_statement_is_separate() {
	let connection = sqlite_connection().await;
	let mut migration = Migration::new("0002_change_name", "catalog");
	migration.operations = vec![
		Operation::RunSQL {
			sql: "SELECT 1".to_string(),
			reverse_sql: Some("SELECT 2; SELECT 3;".to_string()),
		},
		Operation::RenameTable {
			old_name: "books".to_string(),
			new_name: "catalog_books".to_string(),
		},
	];

	let plan = plan_migration_sql(
		&connection,
		&migration,
		&ProjectState::new(),
		MigrationDirection::Backward,
	)
	.await
	.unwrap();

	assert_eq!(
		sql(&plan.statements),
		vec![
			"ALTER TABLE catalog_books RENAME TO books",
			"SELECT 2",
			"SELECT 3",
		]
	);
}

#[tokio::test]
async fn run_rust_is_a_comment_and_non_atomic_metadata_is_preserved() {
	let connection = sqlite_connection().await;
	let mut migration = Migration::new("0003_seed", "catalog").atomic(false);
	migration.operations.push(Operation::RunRust {
		code: "seed_catalog();".to_string(),
		reverse_code: Some("clear_catalog();".to_string()),
	});

	let plan = plan_migration_sql(
		&connection,
		&migration,
		&ProjectState::new(),
		MigrationDirection::Forward,
	)
	.await
	.unwrap();

	assert!(!plan.atomic);
	assert_eq!(
		plan.statements,
		vec![PlannedStatement::Comment(
			"RunRust: seed_catalog();".to_string()
		)]
	);
}

#[tokio::test]
async fn backward_plan_rejects_irreversible_operation() {
	let connection = sqlite_connection().await;
	let mut migration = Migration::new("0004_irreversible", "catalog");
	migration.operations.push(Operation::RunSQL {
		sql: "DELETE FROM \"events\"".to_string(),
		reverse_sql: None,
	});

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
		"Irreversible migration: catalog.0004_irreversible contains an irreversible RunSQL operation"
	);
}

#[tokio::test]
async fn sqlite_recreation_is_planned_without_executing_ddl() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"books\" (\"id\" INTEGER PRIMARY KEY, \"title\" TEXT NOT NULL, \"obsolete\" TEXT)",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0005_drop_obsolete", "catalog");
	migration.operations.push(Operation::DropColumn {
		table: "books".to_string(),
		column: "obsolete".to_string(),
		old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
	});

	let plan = plan_migration_sql(
		&connection,
		&migration,
		&ProjectState::new(),
		MigrationDirection::Forward,
	)
	.await
	.unwrap();

	assert_eq!(
		sql(&plan.statements),
		vec![
			"CREATE TABLE \"books_new\" (\n  id INTEGER NOT NULL PRIMARY KEY,\n  title TEXT NOT NULL\n);",
			"INSERT INTO \"books_new\" (\"id\", \"title\") SELECT \"id\", \"title\" FROM \"books\";",
			"DROP TABLE \"books\";",
			"ALTER TABLE \"books_new\" RENAME TO \"books\";",
		]
	);
	assert_eq!(
		plan.render(reinhardt_db::migrations::SqlDialect::Sqlite),
		"PRAGMA foreign_keys = OFF;\nBEGIN;\nCREATE TABLE \"books_new\" (\n  id INTEGER NOT NULL PRIMARY KEY,\n  title TEXT NOT NULL\n);\nINSERT INTO \"books_new\" (\"id\", \"title\") SELECT \"id\", \"title\" FROM \"books\";\nDROP TABLE \"books\";\nALTER TABLE \"books_new\" RENAME TO \"books\";\nCOMMIT;\nPRAGMA foreign_key_check;\nPRAGMA foreign_keys = ON;\n"
	);
	let columns = connection
		.fetch_all("PRAGMA table_info(\"books\")", vec![])
		.await
		.unwrap();
	assert_eq!(columns.len(), 3);
}

#[tokio::test]
async fn sqlite_recreation_includes_schema_changes_planned_earlier_in_the_migration() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"books\" (\"id\" INTEGER PRIMARY KEY, \"obsolete\" TEXT)",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0006_add_and_drop", "catalog");
	migration.operations = vec![
		Operation::AddColumn {
			table: "books".to_string(),
			column: ColumnDefinition::new("title", FieldType::Text),
			mysql_options: None,
		},
		Operation::DropColumn {
			table: "books".to_string(),
			column: "obsolete".to_string(),
			old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
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
		sql(&plan.statements),
		vec![
			"ALTER TABLE books ADD COLUMN title TEXT",
			"CREATE TABLE \"books_new\" (\n  id INTEGER NOT NULL PRIMARY KEY,\n  title TEXT\n);",
			"INSERT INTO \"books_new\" (\"id\", \"title\") SELECT \"id\", \"title\" FROM \"books\";",
			"DROP TABLE \"books\";",
			"ALTER TABLE \"books_new\" RENAME TO \"books\";",
		]
	);

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());
	executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.unwrap();
	let columns = connection
		.fetch_all("PRAGMA table_info(\"books\")", vec![])
		.await
		.unwrap();
	let names = columns
		.iter()
		.map(|row| row.get::<String>("name").unwrap())
		.collect::<Vec<_>>();
	assert_eq!(names, vec!["id", "title"]);
}

#[tokio::test]
async fn sqlite_recreation_follows_a_renamed_table_in_the_same_migration() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"books\" (\"id\" INTEGER PRIMARY KEY, \"obsolete\" TEXT)",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0007_rename_and_drop", "catalog");
	migration.operations = vec![
		Operation::RenameTable {
			old_name: "books".to_string(),
			new_name: "library_books".to_string(),
		},
		Operation::DropColumn {
			table: "library_books".to_string(),
			column: "obsolete".to_string(),
			old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
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
		sql(&plan.statements),
		vec![
			"ALTER TABLE books RENAME TO library_books",
			"CREATE TABLE \"library_books_new\" (\n  id INTEGER NOT NULL PRIMARY KEY\n);",
			"INSERT INTO \"library_books_new\" (\"id\") SELECT \"id\" FROM \"library_books\";",
			"DROP TABLE \"library_books\";",
			"ALTER TABLE \"library_books_new\" RENAME TO \"library_books\";",
		]
	);
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());
	executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.unwrap();
	let columns = connection
		.fetch_all("PRAGMA table_info(\"library_books\")", vec![])
		.await
		.unwrap();
	assert_eq!(columns.len(), 1);
	assert_eq!(columns[0].get::<String>("name").unwrap(), "id");
}

#[tokio::test]
async fn sqlite_existing_create_table_policy_drives_later_recreation() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"books\" (\"id\" INTEGER PRIMARY KEY, \"legacy\" TEXT)",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0008_existing_create", "catalog");
	migration.operations = vec![
		Operation::CreateTable {
			name: "books".to_string(),
			columns: vec![
				ColumnDefinition::new("id", FieldType::Integer),
				ColumnDefinition::new("planned_only", FieldType::Text),
			],
			constraints: Vec::new(),
			without_rowid: None,
			interleave_in_parent: None,
			partition: None,
		},
		Operation::DropColumn {
			table: "books".to_string(),
			column: "legacy".to_string(),
			old_definition: Some(ColumnDefinition::new("legacy", FieldType::Text)),
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

	assert!(
		sql(&plan.statements)[1].contains("id INTEGER NOT NULL PRIMARY KEY"),
		"{:?}",
		plan.statements
	);
	assert!(!sql(&plan.statements)[1].contains("planned_only"));
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());
	executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.unwrap();
	let columns = connection
		.fetch_all("PRAGMA table_info(\"books\")", vec![])
		.await
		.unwrap();
	let names = columns
		.iter()
		.map(|row| row.get::<String>("name").unwrap())
		.collect::<Vec<_>>();
	assert_eq!(names, vec!["id"]);
}

#[tokio::test]
async fn sqlite_recreation_preserves_an_index_created_earlier_in_the_migration() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"books\" (\"id\" INTEGER PRIMARY KEY, \"title\" TEXT, \"obsolete\" TEXT)",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0009_index_and_drop", "catalog");
	migration.operations = vec![
		Operation::CreateIndex {
			table: "books".to_string(),
			columns: vec!["title".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: None,
			mysql_options: None,
			operator_class: None,
		},
		Operation::DropColumn {
			table: "books".to_string(),
			column: "obsolete".to_string(),
			old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
		},
	];

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());
	executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.unwrap();

	let index = connection
		.fetch_optional(
			"SELECT sql FROM sqlite_master WHERE type = 'index' AND name = 'idx_books_title'",
			vec![],
		)
		.await
		.unwrap();
	assert!(index.is_some());
}

#[tokio::test]
async fn sqlite_recreation_rejects_prior_opaque_run_sql() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"books\" (\"id\" INTEGER PRIMARY KEY, \"obsolete\" TEXT)",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0010_opaque_schema", "catalog");
	migration.operations = vec![
		Operation::RunSQL {
			sql: "ALTER TABLE \"books\" ADD COLUMN \"opaque\" TEXT".to_string(),
			reverse_sql: Some("ALTER TABLE \"books\" DROP COLUMN \"opaque\"".to_string()),
		},
		Operation::DropColumn {
			table: "books".to_string(),
			column: "obsolete".to_string(),
			old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
		},
	];

	let error = plan_migration_sql(
		&connection,
		&migration,
		&ProjectState::new(),
		MigrationDirection::Forward,
	)
	.await
	.unwrap_err();

	assert_eq!(
		error.to_string(),
		"Invalid migration: cannot safely plan SQLite recreation after opaque RunSQL in catalog.0010_opaque_schema"
	);
}

#[tokio::test]
async fn sqlite_backward_recreation_uses_prior_add_and_rename_state() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"books\" (\"id\" INTEGER PRIMARY KEY, \"new_title\" TEXT, \"added\" TEXT)",
			vec![],
		)
		.await
		.unwrap();
	connection
		.execute(
			"CREATE INDEX \"idx_books_new_title\" ON \"books\" (\"new_title\")",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0011_backward_chain", "catalog");
	migration.operations = vec![
		Operation::AddColumn {
			table: "books".to_string(),
			column: ColumnDefinition::new("added", FieldType::Text),
			mysql_options: None,
		},
		Operation::RenameColumn {
			table: "books".to_string(),
			old_name: "old_title".to_string(),
			new_name: "new_title".to_string(),
		},
		Operation::DropColumn {
			table: "books".to_string(),
			column: "obsolete".to_string(),
			old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
		},
	];

	let plan = plan_migration_sql(
		&connection,
		&migration,
		&ProjectState::new(),
		MigrationDirection::Backward,
	)
	.await
	.unwrap();
	let planned_sql = sql(&plan.statements);

	assert_eq!(planned_sql[0], "ALTER TABLE books ADD COLUMN obsolete TEXT");
	assert_eq!(
		planned_sql[1],
		"ALTER TABLE books RENAME COLUMN new_title TO old_title"
	);
	assert!(planned_sql[2].contains("old_title TEXT"));
	assert!(planned_sql[2].contains("obsolete TEXT"));
	assert!(!planned_sql[2].contains("new_title"));
	assert_eq!(
		planned_sql.last().copied(),
		Some("CREATE INDEX \"idx_books_new_title\" ON \"books\" (\"old_title\");")
	);
}

#[tokio::test]
async fn sqlite_backward_recreation_rejects_prior_reverse_run_sql() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"books\" (\"id\" INTEGER PRIMARY KEY, \"added\" TEXT, \"opaque\" TEXT)",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0012_backward_opaque", "catalog");
	migration.operations = vec![
		Operation::AddColumn {
			table: "books".to_string(),
			column: ColumnDefinition::new("added", FieldType::Text),
			mysql_options: None,
		},
		Operation::RunSQL {
			sql: "ALTER TABLE \"books\" ADD COLUMN \"opaque\" TEXT".to_string(),
			reverse_sql: Some("ALTER TABLE \"books\" DROP COLUMN \"opaque\"".to_string()),
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

	assert_eq!(
		error.to_string(),
		"Invalid migration: cannot safely plan SQLite recreation after opaque RunSQL in catalog.0012_backward_opaque"
	);
}

#[tokio::test]
async fn sqlite_table_rename_updates_plain_index_and_self_foreign_key() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"nodes\" (\"id\" INTEGER PRIMARY KEY, \"parent_id\" INTEGER, \"obsolete\" TEXT, CONSTRAINT \"nodes_parent_fk\" FOREIGN KEY (\"parent_id\") REFERENCES \"nodes\" (\"id\"))",
			vec![],
		)
		.await
		.unwrap();
	connection
		.execute(
			"CREATE INDEX \"idx_nodes_parent_id\" ON \"nodes\" (\"parent_id\")",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0013_rename_dependencies", "catalog");
	migration.operations = vec![
		Operation::RenameTable {
			old_name: "nodes".to_string(),
			new_name: "tree_nodes".to_string(),
		},
		Operation::DropColumn {
			table: "tree_nodes".to_string(),
			column: "obsolete".to_string(),
			old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
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
	let planned_sql = sql(&plan.statements);

	assert!(
		planned_sql[1].contains("REFERENCES tree_nodes(\"id\")"),
		"{planned_sql:?}"
	);
	assert_eq!(
		planned_sql.last().copied(),
		Some("CREATE INDEX \"idx_nodes_parent_id\" ON \"tree_nodes\" (\"parent_id\");")
	);

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());
	executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.unwrap();
	let foreign_keys = connection
		.fetch_all("PRAGMA foreign_key_list(\"tree_nodes\")", vec![])
		.await
		.unwrap();
	assert_eq!(
		foreign_keys[0].get::<String>("table").unwrap(),
		"tree_nodes"
	);
	let index_sql = connection
		.fetch_one(
			"SELECT sql FROM sqlite_master WHERE type = 'index' AND name = 'idx_nodes_parent_id'",
			vec![],
		)
		.await
		.unwrap()
		.get::<String>("sql")
		.unwrap();
	assert_eq!(
		index_sql,
		"CREATE INDEX \"idx_nodes_parent_id\" ON \"tree_nodes\" (\"parent_id\")"
	);
}

#[tokio::test]
async fn sqlite_column_rename_rejects_partial_index_metadata() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"books\" (\"id\" INTEGER PRIMARY KEY, \"title\" TEXT, \"obsolete\" TEXT)",
			vec![],
		)
		.await
		.unwrap();
	connection
		.execute(
			"CREATE INDEX \"idx_books_live_title\" ON \"books\" (\"title\") WHERE \"title\" IS NOT NULL",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0014_partial_rename", "catalog");
	migration.operations = vec![
		Operation::RenameColumn {
			table: "books".to_string(),
			old_name: "title".to_string(),
			new_name: "name".to_string(),
		},
		Operation::DropColumn {
			table: "books".to_string(),
			column: "obsolete".to_string(),
			old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
		},
	];

	let error = plan_migration_sql(
		&connection,
		&migration,
		&ProjectState::new(),
		MigrationDirection::Forward,
	)
	.await
	.unwrap_err();

	assert!(matches!(error, MigrationError::InvalidMigration(_)));
	assert!(error.to_string().contains("partial or expression index"));
}

#[tokio::test]
async fn sqlite_column_rename_rejects_expression_index_metadata() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"books\" (\"id\" INTEGER PRIMARY KEY, \"title\" TEXT, \"obsolete\" TEXT)",
			vec![],
		)
		.await
		.unwrap();
	connection
		.execute(
			"CREATE INDEX \"idx_books_lower_title\" ON \"books\" (lower(\"title\"))",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0015_expression_rename", "catalog");
	migration.operations = vec![
		Operation::RenameColumn {
			table: "books".to_string(),
			old_name: "title".to_string(),
			new_name: "name".to_string(),
		},
		Operation::DropColumn {
			table: "books".to_string(),
			column: "obsolete".to_string(),
			old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
		},
	];

	let error = plan_migration_sql(
		&connection,
		&migration,
		&ProjectState::new(),
		MigrationDirection::Forward,
	)
	.await
	.unwrap_err();

	assert!(matches!(error, MigrationError::InvalidMigration(_)));
	assert!(error.to_string().contains("partial or expression index"));
}

#[tokio::test]
async fn sqlite_discriminator_column_is_visible_to_later_recreation() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"animals\" (\"id\" INTEGER PRIMARY KEY, \"obsolete\" TEXT)",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0016_discriminator", "catalog");
	migration.operations = vec![
		Operation::AddDiscriminatorColumn {
			table: "animals".to_string(),
			column_name: "kind".to_string(),
			default_value: "animal".to_string(),
		},
		Operation::DropColumn {
			table: "animals".to_string(),
			column: "obsolete".to_string(),
			old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
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

	assert!(sql(&plan.statements)[1].contains("kind VARCHAR(50) DEFAULT 'animal'"));
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());
	executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.unwrap();
	let columns = connection
		.fetch_all("PRAGMA table_info(\"animals\")", vec![])
		.await
		.unwrap();
	let names = columns
		.iter()
		.map(|row| row.get::<String>("name").unwrap())
		.collect::<Vec<_>>();
	assert_eq!(names, vec!["id", "kind"]);
}

#[tokio::test]
async fn sqlite_parent_table_rename_updates_lazily_loaded_child_foreign_key() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"parents\" (\"id\" INTEGER PRIMARY KEY)",
			vec![],
		)
		.await
		.unwrap();
	connection
		.execute(
			"CREATE TABLE \"children\" (\"id\" INTEGER PRIMARY KEY, \"parent_id\" INTEGER, \"obsolete\" TEXT, FOREIGN KEY (\"parent_id\") REFERENCES \"parents\" (\"id\"))",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0017_external_table_fk", "catalog");
	migration.operations = vec![
		Operation::RenameTable {
			old_name: "parents".to_string(),
			new_name: "guardians".to_string(),
		},
		Operation::DropColumn {
			table: "children".to_string(),
			column: "obsolete".to_string(),
			old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
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

	assert!(
		sql(&plan.statements)[1].contains("REFERENCES guardians(\"id\")"),
		"{:?}",
		plan.statements
	);
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());
	executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.unwrap();
	let foreign_keys = connection
		.fetch_all("PRAGMA foreign_key_list(\"children\")", vec![])
		.await
		.unwrap();
	assert_eq!(foreign_keys[0].get::<String>("table").unwrap(), "guardians");
}

#[tokio::test]
async fn sqlite_parent_table_rename_rejects_lazily_loaded_child_trigger() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"parents\" (\"id\" INTEGER PRIMARY KEY)",
			vec![],
		)
		.await
		.unwrap();
	connection
		.execute(
			"CREATE TABLE \"children\" (\"id\" INTEGER PRIMARY KEY, \"parent_id\" INTEGER, \"obsolete\" TEXT)",
			vec![],
		)
		.await
		.unwrap();
	connection
		.execute(
			"CREATE TRIGGER \"children_touch_parent\" AFTER INSERT ON \"children\" BEGIN UPDATE \"parents\" SET \"id\" = \"id\" WHERE \"id\" = NEW.\"parent_id\"; END",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0018_external_trigger", "catalog");
	migration.operations = vec![
		Operation::RenameTable {
			old_name: "parents".to_string(),
			new_name: "guardians".to_string(),
		},
		Operation::DropColumn {
			table: "children".to_string(),
			column: "obsolete".to_string(),
			old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
		},
	];

	let error = plan_migration_sql(
		&connection,
		&migration,
		&ProjectState::new(),
		MigrationDirection::Forward,
	)
	.await
	.unwrap_err();

	assert!(matches!(error, MigrationError::InvalidMigration(_)));
	assert!(
		error
			.to_string()
			.contains("raw trigger references renamed SQLite identifier 'parents'"),
		"{error}"
	);
}

#[tokio::test]
async fn sqlite_parent_table_rename_ignores_unrelated_trigger_identifier() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"parents\" (\"id\" INTEGER PRIMARY KEY)",
			vec![],
		)
		.await
		.unwrap();
	connection
		.execute(
			"CREATE TABLE \"parents_archive\" (\"id\" INTEGER PRIMARY KEY)",
			vec![],
		)
		.await
		.unwrap();
	connection
		.execute(
			"CREATE TABLE \"children\" (\"id\" INTEGER PRIMARY KEY, \"parent_id\" INTEGER, \"obsolete\" TEXT)",
			vec![],
		)
		.await
		.unwrap();
	connection
		.execute(
			"CREATE TRIGGER \"children_archive_parent\" AFTER INSERT ON \"children\" BEGIN UPDATE \"parents_archive\" SET \"id\" = \"id\" WHERE \"id\" = NEW.\"parent_id\"; END",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0019_unrelated_external_trigger", "catalog");
	migration.operations = vec![
		Operation::RenameTable {
			old_name: "parents".to_string(),
			new_name: "guardians".to_string(),
		},
		Operation::DropColumn {
			table: "children".to_string(),
			column: "obsolete".to_string(),
			old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
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

	assert!(
		sql(&plan.statements)
			.last()
			.is_some_and(|statement| statement.contains("\"parents_archive\"")),
		"{:?}",
		plan.statements
	);
}

#[tokio::test]
async fn sqlite_parent_column_rename_updates_lazily_loaded_child_foreign_key() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"parents\" (\"code\" TEXT PRIMARY KEY)",
			vec![],
		)
		.await
		.unwrap();
	connection
		.execute(
			"CREATE TABLE \"children\" (\"id\" INTEGER PRIMARY KEY, \"parent_code\" TEXT, \"obsolete\" TEXT, FOREIGN KEY (\"parent_code\") REFERENCES \"parents\" (\"code\"))",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0018_external_column_fk", "catalog");
	migration.operations = vec![
		Operation::RenameColumn {
			table: "parents".to_string(),
			old_name: "code".to_string(),
			new_name: "slug".to_string(),
		},
		Operation::DropColumn {
			table: "children".to_string(),
			column: "obsolete".to_string(),
			old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
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

	assert!(
		sql(&plan.statements)[1].contains("REFERENCES parents(\"slug\")"),
		"{:?}",
		plan.statements
	);
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());
	executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.unwrap();
	let foreign_keys = connection
		.fetch_all("PRAGMA foreign_key_list(\"children\")", vec![])
		.await
		.unwrap();
	assert_eq!(foreign_keys[0].get::<String>("to").unwrap(), "slug");
}

#[tokio::test]
async fn sqlite_chained_parent_renames_update_lazily_loaded_child_foreign_key() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"parents\" (\"code\" TEXT PRIMARY KEY)",
			vec![],
		)
		.await
		.unwrap();
	connection
		.execute(
			"CREATE TABLE \"children\" (\"id\" INTEGER PRIMARY KEY, \"parent_code\" TEXT, \"obsolete\" TEXT, FOREIGN KEY (\"parent_code\") REFERENCES \"parents\" (\"code\"))",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0019_external_fk_chain", "catalog");
	migration.operations = vec![
		Operation::RenameTable {
			old_name: "parents".to_string(),
			new_name: "guardians".to_string(),
		},
		Operation::RenameColumn {
			table: "guardians".to_string(),
			old_name: "code".to_string(),
			new_name: "slug".to_string(),
		},
		Operation::RenameTable {
			old_name: "guardians".to_string(),
			new_name: "caretakers".to_string(),
		},
		Operation::DropColumn {
			table: "children".to_string(),
			column: "obsolete".to_string(),
			old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
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

	assert!(
		sql(&plan.statements)[3].contains("REFERENCES caretakers(\"slug\")"),
		"{:?}",
		plan.statements
	);
}

#[tokio::test]
async fn sqlite_column_rename_rejects_multiline_partial_index_metadata() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"books\" (\"id\" INTEGER PRIMARY KEY, \"title\" TEXT, \"obsolete\" TEXT)",
			vec![],
		)
		.await
		.unwrap();
	connection
		.execute(
			"CREATE INDEX \"idx_books_live_title\" ON \"books\" (\"title\")\nWHERE\t\"title\" IS NOT NULL",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0020_multiline_partial", "catalog");
	migration.operations = vec![
		Operation::RenameColumn {
			table: "books".to_string(),
			old_name: "title".to_string(),
			new_name: "name".to_string(),
		},
		Operation::DropColumn {
			table: "books".to_string(),
			column: "obsolete".to_string(),
			old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
		},
	];

	let error = plan_migration_sql(
		&connection,
		&migration,
		&ProjectState::new(),
		MigrationDirection::Forward,
	)
	.await
	.unwrap_err();

	assert!(matches!(error, MigrationError::InvalidMigration(_)));
	assert!(
		error
			.to_string()
			.contains("index metadata requiring raw SQL")
	);
}

#[tokio::test]
async fn sqlite_column_rename_rejects_typed_expression_index_with_columns() {
	let connection = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE \"books\" (\"id\" INTEGER PRIMARY KEY, \"title\" TEXT, \"obsolete\" TEXT)",
			vec![],
		)
		.await
		.unwrap();
	let mut migration = Migration::new("0021_typed_expression", "catalog");
	migration.operations = vec![
		Operation::CreateIndex {
			table: "books".to_string(),
			columns: vec!["title".to_string()],
			unique: false,
			index_type: None,
			where_clause: None,
			concurrently: false,
			expressions: Some(vec!["lower(\"title\")".to_string()]),
			mysql_options: None,
			operator_class: None,
		},
		Operation::RenameColumn {
			table: "books".to_string(),
			old_name: "title".to_string(),
			new_name: "name".to_string(),
		},
		Operation::DropColumn {
			table: "books".to_string(),
			column: "obsolete".to_string(),
			old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
		},
	];

	let error = plan_migration_sql(
		&connection,
		&migration,
		&ProjectState::new(),
		MigrationDirection::Forward,
	)
	.await
	.unwrap_err();

	assert!(matches!(error, MigrationError::InvalidMigration(_)));
	assert!(
		error
			.to_string()
			.contains("index metadata requiring raw SQL")
	);
}

#[tokio::test]
async fn executor_consumes_comment_plan_without_dispatching_it_as_sql() {
	let connection = sqlite_connection().await;
	let mut migration = Migration::new("0022_rust_only", "catalog");
	migration.operations.push(Operation::RunRust {
		code: "panic_if_executed_as_sql();".to_string(),
		reverse_code: Some("clear();".to_string()),
	});
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());

	let result = executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.unwrap();

	assert_eq!(result.applied, vec![migration.id()]);
	assert!(
		connection
			.fetch_optional(
				"SELECT name FROM sqlite_master WHERE name = 'panic_if_executed_as_sql'",
				vec![],
			)
			.await
			.unwrap()
			.is_none()
	);
}
