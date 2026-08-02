use reinhardt_db::{
	backends::DatabaseConnection,
	migrations::{
		ColumnInfo, FieldType, InspectDbOptions, IntrospectConfig, TableInfo,
		generate_models_canonical, inspect_database, introspection::DatabaseSchema,
		render_models_module,
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

#[rstest::rstest]
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

#[rstest::rstest]
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

#[rstest::rstest]
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

#[rstest::rstest]
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

#[rstest::rstest]
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

#[rstest::rstest]
fn canonical_stdout_and_directory_output_are_fully_repeatable() {
	let mut first_tables = HashMap::new();
	first_tables.insert("zebra".to_string(), table("zebra", &["zeta", "alpha"]));
	first_tables.insert("alpha".to_string(), table("alpha", &["omega", "beta"]));
	let mut second_tables = HashMap::new();
	second_tables.insert("alpha".to_string(), table("alpha", &["beta", "omega"]));
	second_tables.insert("zebra".to_string(), table("zebra", &["alpha", "zeta"]));
	let first_schema = DatabaseSchema {
		tables: first_tables,
	};
	let second_schema = DatabaseSchema {
		tables: second_tables,
	};
	let mut config = IntrospectConfig::default();
	config.output.directory = "/tmp/reinhardt-inspectdb-rendering".into();

	let first_stdout =
		render_models_module(&config, &first_schema).expect("first stdout rendering succeeds");
	let second_stdout =
		render_models_module(&config, &second_schema).expect("second stdout rendering succeeds");
	let first_directory =
		generate_models_canonical(&config, &first_schema).expect("first directory rendering");
	let second_directory =
		generate_models_canonical(&config, &second_schema).expect("second directory rendering");
	let first_files: Vec<_> = first_directory
		.files
		.into_iter()
		.map(|file| (file.path, file.content))
		.collect();
	let second_files: Vec<_> = second_directory
		.files
		.into_iter()
		.map(|file| (file.path, file.content))
		.collect();

	assert_eq!(first_stdout, second_stdout);
	assert_eq!(first_files, second_files);
	assert_eq!(
		first_files
			.iter()
			.map(|(path, _)| path.as_path())
			.collect::<Vec<_>>(),
		vec![
			std::path::Path::new("/tmp/reinhardt-inspectdb-rendering/models/alpha.rs"),
			std::path::Path::new("/tmp/reinhardt-inspectdb-rendering/models/zebra.rs"),
			std::path::Path::new("/tmp/reinhardt-inspectdb-rendering/models.rs"),
		],
	);
	assert!(
		first_files
			.iter()
			.all(|(path, _)| path.file_name().is_none_or(|name| name != "mod.rs")),
	);
	let models_module = first_files
		.iter()
		.find(|(path, _)| path.ends_with("models.rs"))
		.map(|(_, content)| content)
		.expect("models.rs should be present");
	syn::parse_file(models_module).expect("models.rs should be parseable");
	assert!(models_module.contains("pub mod alpha;"));
	assert!(models_module.contains("pub mod zebra;"));
	assert!(!first_stdout.contains("Generated at:"));
	assert!(first_stdout.contains("Generated by `reinhardt inspectdb`"));
	assert!(first_stdout.contains("cargo run --bin manage inspectdb"));
	for (_, content) in first_files {
		assert!(!content.contains("Generated at:"));
		assert!(content.contains("Generated by `reinhardt inspectdb`"));
		assert!(content.contains("cargo run --bin manage inspectdb"));
	}
}
