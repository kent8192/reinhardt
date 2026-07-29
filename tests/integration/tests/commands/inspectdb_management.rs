//! End-to-end `inspectdb` coverage for PostgreSQL, MySQL, and SQLite.
//!
//! These tests exercise the command adapter, backend introspection, canonical
//! rendering, and atomic directory publication together against real databases.

use reinhardt_commands::{
	BaseCommand, CommandContext, CommandError, InspectDbCommand, InspectDbWriter,
};
use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	FieldType, GeneratedOutput, GeneratedStorage, InspectDbOptions, MigrationError,
	inspect_database,
};
use reinhardt_query::prelude::{
	Alias, ColumnDef, Expr, ExprTrait, ForeignKey, ForeignKeyAction, MySqlQueryBuilder,
	PostgresQueryBuilder, Query, QueryStatementBuilder, SchemaExpr, SqliteQueryBuilder,
};
use reinhardt_test::fixtures::{mysql_container, postgres_container};
use rstest::*;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{MySqlPool, PgPool, SqlitePool};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use testcontainers::{ContainerAsync, GenericImage};

#[derive(Default)]
struct CapturedOutput {
	stdout: Mutex<Vec<String>>,
	stderr: Mutex<Vec<String>>,
}

impl InspectDbWriter for CapturedOutput {
	fn write_stdout(&self, content: &str) -> io::Result<()> {
		self.stdout
			.lock()
			.expect("stdout capture lock should not be poisoned")
			.push(content.to_string());
		Ok(())
	}

	fn write_stderr(&self, content: &str) -> io::Result<()> {
		self.stderr
			.lock()
			.expect("stderr capture lock should not be poisoned")
			.push(content.to_string());
		Ok(())
	}
}

#[derive(Default)]
struct RollbackFaultOutput {
	stdout: Mutex<Vec<String>>,
	stderr: Mutex<Vec<String>>,
	installed_before_failure: Mutex<bool>,
}

impl InspectDbWriter for RollbackFaultOutput {
	fn write_stdout(&self, content: &str) -> io::Result<()> {
		self.stdout
			.lock()
			.expect("stdout capture lock should not be poisoned")
			.push(content.to_string());
		Ok(())
	}

	fn write_stderr(&self, content: &str) -> io::Result<()> {
		self.stderr
			.lock()
			.expect("stderr capture lock should not be poisoned")
			.push(content.to_string());
		Ok(())
	}

	fn publish_generated_files(
		&self,
		output: &GeneratedOutput,
		force: bool,
	) -> Result<(), MigrationError> {
		assert!(force, "the rollback scenario replaces an existing file");
		let replacement = output
			.files
			.iter()
			.find(|file| file.path.ends_with("accounts.rs"))
			.expect("accounts output should be generated");
		let new_file = output
			.files
			.iter()
			.find(|file| file.path.ends_with("models.rs"))
			.expect("module output should be generated");
		let original = fs::read(&replacement.path)?;

		fs::write(&replacement.path, replacement.content.as_bytes())?;
		fs::write(&new_file.path, new_file.content.as_bytes())?;
		assert_ne!(
			fs::read(&replacement.path)?,
			original,
			"replacement must be installed before the injected failure",
		);
		assert!(
			new_file.path.is_file(),
			"new output must be installed before the injected failure",
		);
		*self
			.installed_before_failure
			.lock()
			.expect("publication state lock should not be poisoned") = true;

		fs::write(&replacement.path, &original)?;
		fs::remove_file(&new_file.path)?;
		Err(MigrationError::IoError(io::Error::other(
			"injected directory publication failure after install",
		)))
	}
}

struct SqliteInspectDbFixture {
	_temp: TempDir,
	_pool: SqlitePool,
	url: String,
}

#[fixture]
async fn sqlite_inspectdb_fixture() -> SqliteInspectDbFixture {
	let temp = tempfile::Builder::new()
		.prefix("reinhardt-inspectdb-integration-")
		.tempdir_in("/tmp")
		.expect("SQLite fixture directory should be created");
	let database_path = temp.path().join("inspectdb.sqlite3");
	let url = format!("sqlite:///{}", database_path.display());
	let pool = SqlitePoolOptions::new()
		.max_connections(1)
		.connect_with(
			SqliteConnectOptions::new()
				.filename(&database_path)
				.create_if_missing(true),
		)
		.await
		.expect("SQLite fixture should connect");
	create_sqlite_schema(&pool).await;

	SqliteInspectDbFixture {
		_temp: temp,
		_pool: pool,
		url,
	}
}

async fn create_postgres_schema(pool: &PgPool) {
	let accounts = Query::create_table()
		.table(Alias::new("accounts"))
		.col(
			ColumnDef::new(Alias::new("id"))
				.integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(Alias::new("name"))
				.string_len(64)
				.not_null(true),
		)
		.col(
			ColumnDef::new(Alias::new("active"))
				.boolean()
				.not_null(true),
		)
		.col(
			ColumnDef::new(Alias::new("created_at"))
				.timestamp_with_time_zone()
				.not_null(true),
		)
		.to_string(PostgresQueryBuilder::new());
	let audit_log = Query::create_table()
		.table(Alias::new("audit_log"))
		.col(
			ColumnDef::new(Alias::new("id"))
				.integer()
				.not_null(true)
				.primary_key(true),
		)
		.to_string(PostgresQueryBuilder::new());
	let view = active_accounts_view().to_string(PostgresQueryBuilder::new());

	for statement in [&accounts, &audit_log, &view] {
		sqlx::query(statement)
			.execute(pool)
			.await
			.expect("generated PostgreSQL schema statement should execute");
	}
}

async fn create_mysql_schema(pool: &MySqlPool) {
	let accounts = Query::create_table()
		.table(Alias::new("accounts"))
		.col(
			ColumnDef::new(Alias::new("id"))
				.integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(Alias::new("name"))
				.string_len(64)
				.not_null(true),
		)
		.col(
			ColumnDef::new(Alias::new("active"))
				.boolean()
				.not_null(true),
		)
		.col(
			ColumnDef::new(Alias::new("created_at"))
				.date_time()
				.not_null(true),
		)
		.col(
			ColumnDef::new(Alias::new("status"))
				.string_len(16)
				.not_null(true)
				.default("enabled".into()),
		)
		.col(
			ColumnDef::new(Alias::new("amount"))
				.decimal(12, 4)
				.not_null(true),
		)
		.col(
			ColumnDef::new(Alias::new("amount_copy"))
				.decimal(12, 4)
				.generated_stored(SchemaExpr::col("amount")),
		)
		.to_string(MySqlQueryBuilder::new());
	let mut audit_log = Query::create_table();
	audit_log
		.table(Alias::new("audit_log"))
		.col(
			ColumnDef::new(Alias::new("id"))
				.integer()
				.not_null(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(Alias::new("account_id"))
				.integer()
				.not_null(true),
		);
	let mut account_fk = ForeignKey::create();
	account_fk
		.name(Alias::new("fk_audit_log_account"))
		.from_tbl(Alias::new("audit_log"))
		.from_col(Alias::new("account_id"))
		.to_tbl(Alias::new("accounts"))
		.to_col(Alias::new("id"))
		.on_delete(ForeignKeyAction::Cascade)
		.on_update(ForeignKeyAction::Restrict);
	audit_log.foreign_key_from_builder(&mut account_fk);
	let audit_log = audit_log.to_string(MySqlQueryBuilder::new());
	let view = active_accounts_view().to_string(MySqlQueryBuilder::new());

	for statement in [&accounts, &audit_log] {
		sqlx::query(statement)
			.execute(pool)
			.await
			.expect("generated MySQL schema statement should execute");
	}

	let secondary_index = Query::create_index()
		.name(Alias::new("idx_accounts_status"))
		.table(Alias::new("accounts"))
		.col(Alias::new("status"))
		.to_string(MySqlQueryBuilder::new());
	let unique_index = Query::create_index()
		.name(Alias::new("uq_accounts_name"))
		.table(Alias::new("accounts"))
		.col(Alias::new("name"))
		.unique()
		.to_string(MySqlQueryBuilder::new());
	for statement in [&secondary_index, &unique_index, &view] {
		sqlx::query(statement)
			.execute(pool)
			.await
			.expect("generated MySQL schema metadata statement should execute");
	}
}

async fn create_sqlite_schema(pool: &SqlitePool) {
	let accounts = Query::create_table()
		.table(Alias::new("accounts"))
		.col(
			ColumnDef::new(Alias::new("id"))
				.integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(Alias::new("name"))
				.string_len(64)
				.not_null(true),
		)
		.col(
			ColumnDef::new(Alias::new("active"))
				.boolean()
				.not_null(true),
		)
		.col(
			ColumnDef::new(Alias::new("created_at"))
				.date_time()
				.not_null(true),
		)
		.to_string(SqliteQueryBuilder::new());
	let audit_log = Query::create_table()
		.table(Alias::new("audit_log"))
		.col(
			ColumnDef::new(Alias::new("id"))
				.integer()
				.not_null(true)
				.primary_key(true),
		)
		.to_string(SqliteQueryBuilder::new());
	let view = active_accounts_view().to_string(SqliteQueryBuilder::new());

	for statement in [&accounts, &audit_log, &view] {
		sqlx::query(statement)
			.execute(pool)
			.await
			.expect("generated SQLite schema statement should execute");
	}
}

fn active_accounts_view() -> reinhardt_query::query::CreateViewStatement {
	let mut select = Query::select();
	select
		.columns([
			Alias::new("id"),
			Alias::new("name"),
			Alias::new("active"),
			Alias::new("created_at"),
		])
		.from(Alias::new("accounts"))
		.and_where(Expr::col(Alias::new("active")).eq(true));
	let mut view = Query::create_view();
	view.name(Alias::new("active_accounts")).as_select(select);
	view
}

async fn run_inspectdb(url: &str, tables: &[&str], include_views: bool) -> (String, Vec<String>) {
	let output = Arc::new(CapturedOutput::default());
	let command = InspectDbCommand::with_writer(output.clone());
	let mut context =
		CommandContext::new(tables.iter().map(|table| (*table).to_string()).collect());
	context.set_option("database".to_string(), "default".to_string());
	context.set_option("database-url".to_string(), url.to_string());
	if include_views {
		context.set_option("include-views".to_string(), "true".to_string());
	}

	command
		.execute(&context)
		.await
		.expect("inspectdb should generate models");

	let stdout = output
		.stdout
		.lock()
		.expect("stdout capture lock should not be poisoned")
		.clone();
	let stderr = output
		.stderr
		.lock()
		.expect("stderr capture lock should not be poisoned")
		.clone();
	assert_eq!(stdout.len(), 1, "stdout mode must use one complete write");
	(stdout.into_iter().next().expect("one stdout write"), stderr)
}

fn struct_fields(source: &str) -> BTreeMap<String, BTreeMap<String, String>> {
	let syntax = syn::parse_file(source).expect("generated stdout should be parseable Rust");
	syntax
		.items
		.into_iter()
		.filter_map(|item| match item {
			syn::Item::Struct(item) => Some((
				item.ident.to_string(),
				item.fields
					.into_iter()
					.map(|field| {
						(
							field
								.ident
								.expect("generated model fields should be named")
								.to_string(),
							type_name(&field.ty),
						)
					})
					.collect(),
			)),
			_ => None,
		})
		.collect()
}

fn type_name(ty: &syn::Type) -> String {
	match ty {
		syn::Type::Path(path) => path
			.path
			.segments
			.iter()
			.map(|segment| {
				let arguments = match &segment.arguments {
					syn::PathArguments::None => String::new(),
					syn::PathArguments::AngleBracketed(arguments) => {
						let arguments = arguments
							.args
							.iter()
							.map(|argument| match argument {
								syn::GenericArgument::Type(ty) => type_name(ty),
								_ => panic!("generated field type should use type arguments only"),
							})
							.collect::<Vec<_>>()
							.join(", ");
						format!("<{arguments}>")
					}
					syn::PathArguments::Parenthesized(_) => {
						panic!("generated field type should not use parenthesized arguments")
					}
				};
				format!("{}{arguments}", segment.ident)
			})
			.collect::<Vec<_>>()
			.join("::"),
		syn::Type::Reference(reference) => format!("&{}", type_name(&reference.elem)),
		_ => panic!("generated field should use a path type"),
	}
}

fn expected_account_fields(
	active: &str,
	created_at: &str,
	additional: &[(&str, &str)],
) -> BTreeMap<String, String> {
	let mut fields = BTreeMap::from([
		("active".to_string(), active.to_string()),
		("created_at".to_string(), created_at.to_string()),
		("id".to_string(), "i32".to_string()),
		("name".to_string(), "String".to_string()),
	]);
	fields.extend(
		additional
			.iter()
			.map(|(name, ty)| ((*name).to_string(), (*ty).to_string())),
	);
	fields
}

async fn assert_backend_contract(
	url: &str,
	backend: &str,
	active: &str,
	created_at: &str,
	additional: &[(&str, &str)],
) {
	let (first, first_stderr) = run_inspectdb(url, &["accounts"], false).await;
	let (second, second_stderr) = run_inspectdb(url, &["accounts"], false).await;

	assert_eq!(first, second, "identical schemas must render identically");
	assert_eq!(
		struct_fields(&first),
		BTreeMap::from([(
			"Accounts".to_string(),
			expected_account_fields(active, created_at, additional),
		)]),
		"exact table selection must exclude other tables and views",
	);
	let expected_stderr = vec![
		format!("Inspecting database schema ({backend})..."),
		"Found 1 schema objects".to_string(),
		"Generated models module".to_string(),
	];
	assert_eq!(first_stderr, expected_stderr);
	assert_eq!(second_stderr, expected_stderr);
	assert!(
		first_stderr.iter().all(|line| !line.contains(url)),
		"stderr must not expose the database URL",
	);

	let (with_views, _) = run_inspectdb(url, &[], true).await;
	assert_eq!(
		struct_fields(&with_views)
			.keys()
			.cloned()
			.collect::<Vec<_>>(),
		vec![
			"Accounts".to_string(),
			"ActiveAccounts".to_string(),
			"AuditLog".to_string(),
		],
		"`--include-views` must add the view while retaining tables",
	);
}

#[rstest]
#[tokio::test]
async fn postgres_inspectdb_selects_exact_tables_and_includes_views(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, pool, _port, url) = postgres_container.await;
	create_postgres_schema(pool.as_ref()).await;

	assert_backend_contract(
		&url,
		"Postgres",
		"bool",
		"chrono::DateTime<chrono::Utc>",
		&[],
	)
	.await;
}

#[rstest]
#[tokio::test]
async fn mysql_inspectdb_selects_exact_tables_and_includes_views(
	#[future] mysql_container: (ContainerAsync<GenericImage>, Arc<MySqlPool>, u16, String),
) {
	let (_container, pool, _port, url) = mysql_container.await;
	create_mysql_schema(pool.as_ref()).await;
	let connection = DatabaseConnection::connect_mysql(&url)
		.await
		.expect("MySQL inspectdb connection should succeed");
	let schema = inspect_database(&connection, &InspectDbOptions::default())
		.await
		.expect("MySQL schema discovery should succeed");
	let mut schema_objects = schema.tables.keys().cloned().collect::<Vec<_>>();
	schema_objects.sort();
	assert_eq!(
		schema_objects,
		vec!["accounts".to_string(), "audit_log".to_string()],
		"MySQL catalog discovery must preserve exact table identifiers",
	);
	let accounts = &schema.tables["accounts"];
	assert_eq!(
		accounts.indexes["idx_accounts_status"],
		reinhardt_db::migrations::IndexInfo {
			name: "idx_accounts_status".to_string(),
			columns: vec!["status".to_string()],
			unique: false,
			access_method: Some("BTREE".to_string()),
			index_type: None,
			expressions: None,
			operator_class: None,
			operator_class_is_default: false,
		},
	);
	assert_eq!(
		accounts.indexes["uq_accounts_name"],
		reinhardt_db::migrations::IndexInfo {
			name: "uq_accounts_name".to_string(),
			columns: vec!["name".to_string()],
			unique: true,
			access_method: Some("BTREE".to_string()),
			index_type: None,
			expressions: None,
			operator_class: None,
			operator_class_is_default: false,
		},
	);
	assert_eq!(
		accounts.unique_constraints,
		vec![reinhardt_db::migrations::UniqueConstraintInfo {
			name: "uq_accounts_name".to_string(),
			columns: vec!["name".to_string()],
		}],
	);
	assert_eq!(
		accounts.columns["status"].default,
		Some("enabled".to_string()),
	);
	assert_eq!(
		accounts.columns["amount"].column_type,
		FieldType::Decimal {
			precision: 12,
			scale: 4,
		},
	);
	let generated = accounts.columns["amount_copy"]
		.generated
		.as_ref()
		.expect("generated-column metadata should be preserved");
	assert_eq!(generated.storage, GeneratedStorage::Stored);
	assert_eq!(generated.raw_sql.as_deref(), Some("`amount`"));
	let foreign_keys = &schema.tables["audit_log"].foreign_keys;
	assert_eq!(foreign_keys.len(), 1);
	assert_eq!(foreign_keys[0].name, "fk_audit_log_account");
	assert_eq!(foreign_keys[0].columns, vec!["account_id".to_string()]);
	assert_eq!(foreign_keys[0].referenced_table, "accounts");
	assert_eq!(foreign_keys[0].referenced_columns, vec!["id".to_string()]);
	assert_eq!(foreign_keys[0].on_delete.as_deref(), Some("CASCADE"));
	assert_eq!(foreign_keys[0].on_update.as_deref(), Some("RESTRICT"));

	assert_backend_contract(
		&url,
		"Mysql",
		"bool",
		"chrono::NaiveDateTime",
		&[
			("amount", "rust_decimal::Decimal"),
			("amount_copy", "Option<rust_decimal::Decimal>"),
			("status", "String"),
		],
	)
	.await;
}

#[rstest]
#[tokio::test]
async fn sqlite_inspectdb_selects_exact_tables_and_includes_views(
	#[future] sqlite_inspectdb_fixture: SqliteInspectDbFixture,
) {
	let fixture = sqlite_inspectdb_fixture.await;

	assert_backend_contract(&fixture.url, "Sqlite", "i32", "String", &[]).await;
}

#[rstest]
#[tokio::test]
async fn directory_rejection_preserves_existing_output_and_creates_no_partial_files(
	#[future] sqlite_inspectdb_fixture: SqliteInspectDbFixture,
) {
	let fixture = sqlite_inspectdb_fixture.await;
	let output_directory = fixture._temp.path().join("models");
	fs::create_dir_all(output_directory.join("models"))
		.expect("output directory should be created");
	let accounts_path = output_directory.join("models/accounts.rs");
	fs::write(&accounts_path, b"original bytes").expect("existing output should be created");
	let output = Arc::new(CapturedOutput::default());
	let command = InspectDbCommand::with_writer(output.clone());
	let mut context = CommandContext::new(vec!["accounts".to_string()]);
	context.set_option("database".to_string(), "default".to_string());
	context.set_option("database-url".to_string(), fixture.url.clone());
	context.set_option(
		"output".to_string(),
		output_directory.to_string_lossy().into_owned(),
	);

	let error = command
		.execute(&context)
		.await
		.expect_err("existing output must reject the complete generated file set");

	assert!(
		matches!(error, CommandError::ExecutionError(_)),
		"directory preflight should return an execution error",
	);
	assert_eq!(
		fs::read(&accounts_path).expect("existing output should remain readable"),
		b"original bytes",
	);
	assert_eq!(
		fs::read_dir(&output_directory)
			.expect("output directory should remain readable")
			.map(|entry| {
				entry
					.expect("output entry should be readable")
					.file_name()
					.to_string_lossy()
					.into_owned()
			})
			.collect::<Vec<_>>(),
		vec!["models".to_string()],
	);
	assert!(!output_directory.join("models.rs").exists());
	assert!(!output_directory.join("mod.rs").exists());
	assert!(!output_directory.join("models/mod.rs").exists());
	assert_eq!(
		output
			.stdout
			.lock()
			.expect("stdout capture lock should not be poisoned")
			.as_slice(),
		&[] as &[String],
	);
}

#[rstest]
#[tokio::test]
async fn directory_adapter_propagates_publication_failure_after_strategy_rolls_back_state(
	#[future] sqlite_inspectdb_fixture: SqliteInspectDbFixture,
) {
	let fixture = sqlite_inspectdb_fixture.await;
	let output_directory = fixture._temp.path().join("models");
	fs::create_dir_all(output_directory.join("models"))
		.expect("output directory should be created");
	let accounts_path = output_directory.join("models/accounts.rs");
	fs::write(&accounts_path, b"original bytes").expect("existing output should be created");
	let output = Arc::new(RollbackFaultOutput::default());
	let command = InspectDbCommand::with_writer(output.clone());
	let mut context = CommandContext::new(vec!["accounts".to_string()]);
	context.set_option("database".to_string(), "default".to_string());
	context.set_option("database-url".to_string(), fixture.url.clone());
	context.set_option(
		"output".to_string(),
		output_directory.to_string_lossy().into_owned(),
	);
	context.set_option("force".to_string(), "true".to_string());

	let error = command
		.execute(&context)
		.await
		.expect_err("directory publication failure should reach the command caller");

	assert_eq!(
		error.to_string(),
		"Execution error: Generated file write failed: IO error: injected directory publication failure after install",
	);
	assert!(
		*output
			.installed_before_failure
			.lock()
			.expect("publication state lock should not be poisoned"),
		"the strategy must install output before failing",
	);
	assert_eq!(
		fs::read(&accounts_path).expect("original output should be restored"),
		b"original bytes",
	);
	assert_eq!(
		fs::read_dir(&output_directory)
			.expect("output directory should remain readable")
			.map(|entry| {
				entry
					.expect("output entry should be readable")
					.file_name()
					.to_string_lossy()
					.into_owned()
			})
			.collect::<Vec<_>>(),
		vec!["models".to_string()],
	);
	assert!(!output_directory.join("models.rs").exists());
	assert!(!output_directory.join("mod.rs").exists());
	assert!(!output_directory.join("models/mod.rs").exists());
	assert!(
		output
			.stdout
			.lock()
			.expect("stdout capture lock should not be poisoned")
			.is_empty(),
	);
	assert_eq!(
		output
			.stderr
			.lock()
			.expect("stderr capture lock should not be poisoned")
			.as_slice(),
		&[
			"Inspecting database schema (Sqlite)...",
			"Found 1 schema objects",
		],
	);
	assert!(
		output
			.stderr
			.lock()
			.expect("stderr capture lock should not be poisoned")
			.iter()
			.all(|line| !line.contains(&fixture.url)),
		"stderr must not expose the database URL",
	);
}
