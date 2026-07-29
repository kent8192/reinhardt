use reinhardt_db::{
	backends::DatabaseConnection,
	migrations::{
		ColumnInfo, FieldType, InspectDbOptions, IntrospectConfig, TableInfo, inspect_database,
		introspection::DatabaseSchema, render_models_module,
	},
};
use std::collections::HashMap;

fn table(name: &str, columns: &[&str]) -> TableInfo {
	let columns = columns
		.iter()
		.map(|name| {
			(
				(*name).to_string(),
				ColumnInfo {
					name: (*name).to_string(),
					column_type: FieldType::Text,
					nullable: false,
					default: None,
					auto_increment: false,
					generated: None,
				},
			)
		})
		.collect();

	TableInfo {
		name: name.to_string(),
		columns,
		indexes: HashMap::new(),
		primary_key: Vec::new(),
		foreign_keys: Vec::new(),
		unique_constraints: Vec::new(),
		check_constraints: Vec::new(),
	}
}

async fn sqlite_connection() -> DatabaseConnection {
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("SQLite connection should be available");
	connection
		.execute(
			"CREATE TABLE zebra (id INTEGER PRIMARY KEY, label TEXT NOT NULL)",
			vec![],
		)
		.await
		.expect("zebra table should be created");
	connection
		.execute(
			"CREATE TABLE alpha (id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
			vec![],
		)
		.await
		.expect("alpha table should be created");
	connection
		.execute(
			"CREATE VIEW alpha_view AS SELECT id, value FROM alpha",
			vec![],
		)
		.await
		.expect("alpha_view should be created");
	connection
}

#[tokio::test]
async fn inspect_database_filters_exact_requested_tables_and_rejects_unknown_names() {
	let connection = sqlite_connection().await;
	let schema = inspect_database(
		&connection,
		&InspectDbOptions {
			tables: vec!["zebra".to_string(), "alpha".to_string()],
			include_views: false,
			include_partitions: false,
		},
	)
	.await
	.expect("requested tables should be inspected");

	assert_eq!(schema.tables.len(), 2);
	assert!(schema.tables.contains_key("zebra"));
	assert!(schema.tables.contains_key("alpha"));

	let error = inspect_database(
		&connection,
		&InspectDbOptions {
			tables: vec!["alpha.*".to_string()],
			include_views: false,
			include_partitions: false,
		},
	)
	.await
	.expect_err("table names must be exact rather than patterns");
	assert_eq!(
		error.to_string(),
		"Introspection error: Requested table not found: alpha.*"
	);
}

#[tokio::test]
async fn inspect_database_includes_views_only_when_requested() {
	let connection = sqlite_connection().await;
	let without_views = inspect_database(&connection, &InspectDbOptions::default())
		.await
		.expect("tables should be inspected");
	assert!(!without_views.tables.contains_key("alpha_view"));

	let with_views = inspect_database(
		&connection,
		&InspectDbOptions {
			include_views: true,
			..InspectDbOptions::default()
		},
	)
	.await
	.expect("views should be inspected when requested");
	assert!(with_views.tables.contains_key("alpha_view"));
}

#[tokio::test]
async fn inspect_database_rejects_partitions_for_non_postgres_backends() {
	let connection = sqlite_connection().await;
	let error = inspect_database(
		&connection,
		&InspectDbOptions {
			include_partitions: true,
			..InspectDbOptions::default()
		},
	)
	.await
	.expect_err("SQLite does not support PostgreSQL partitions");
	assert_eq!(
		error.to_string(),
		"Introspection error: include_partitions is only supported for PostgreSQL"
	);
}

#[test]
fn render_models_module_is_stable_and_parses_as_one_rust_module() {
	let mut tables = HashMap::new();
	tables.insert("zebra".to_string(), table("zebra", &["zeta", "alpha"]));
	tables.insert("alpha".to_string(), table("alpha", &["omega", "beta"]));
	let schema = DatabaseSchema { tables };
	let mut config = IntrospectConfig::default();
	config.output.single_file = true;
	config.imports.additional = vec![
		"std::collections::BTreeSet".to_string(),
		"std::collections::BTreeMap".to_string(),
	];

	let source = render_models_module(&config, &schema).expect("schema should render");
	assert!(
		syn::parse_file(&source).is_ok(),
		"rendered source must parse"
	);
	assert!(source.find("pub struct Alpha").unwrap() < source.find("pub struct Zebra").unwrap());
	assert!(source.find("pub beta:").unwrap() < source.find("pub omega:").unwrap());
	assert!(source.find("pub alpha:").unwrap() < source.find("pub zeta:").unwrap());
	assert!(
		source.find("use std::collections::BTreeMap;").unwrap()
			< source.find("use std::collections::BTreeSet;").unwrap()
	);
}

#[test]
fn render_models_module_never_emits_database_credentials() {
	let mut tables = HashMap::new();
	tables.insert("accounts".to_string(), table("accounts", &["id"]));
	let schema = DatabaseSchema { tables };
	let config = IntrospectConfig::default().with_database_url(
		"postgres://inspect_user:database-password@localhost/accounts?api_key=query-secret",
	);

	let source = render_models_module(&config, &schema).expect("schema should render");

	assert!(!source.contains("database-password"));
	assert!(!source.contains("query-secret"));
	assert!(!source.contains("inspect_user"));
	assert!(!source.contains("localhost/accounts"));
}
