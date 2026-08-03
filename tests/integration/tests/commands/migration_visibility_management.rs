//! Backend integration coverage for migration visibility commands.

use async_trait::async_trait;
use reinhardt_commands::{
	BaseCommand, CommandContext, MigrationVisibilityWriter, ShowMigrationsCommand,
	SqlMigrateCommand,
};
use reinhardt_db::backends::{DatabaseConnection, DatabaseType};
use reinhardt_db::migrations::{
	ColumnDefinition, DatabaseMigrationExecutor, DatabaseMigrationRecorder, FieldType,
	FilesystemRepository, Migration, MigrationCatalog, MigrationDirection, MigrationKey,
	MigrationRenderOptions, MigrationSource, Operation, PlannedStatement, ProjectState, Result,
	SqlDialect, plan_migration_sql,
};
use reinhardt_test::fixtures::{mysql_container, postgres_container};
use rstest::*;
use sqlx::{MySqlPool, PgPool};
use std::io;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use testcontainers::{ContainerAsync, GenericImage};

#[derive(Default)]
struct RecordingWriter {
	outputs: Mutex<Vec<String>>,
}

impl MigrationVisibilityWriter for RecordingWriter {
	fn write_stdout(&self, content: &str) -> io::Result<()> {
		self.outputs
			.lock()
			.expect("migration visibility output lock should not be poisoned")
			.push(content.to_string());
		Ok(())
	}
}

impl RecordingWriter {
	fn take(&self) -> Vec<String> {
		std::mem::take(
			&mut *self
				.outputs
				.lock()
				.expect("migration visibility output lock should not be poisoned"),
		)
	}
}

struct TestSource {
	migrations: Vec<Migration>,
}

#[async_trait]
impl MigrationSource for TestSource {
	async fn all_migrations(&self) -> Result<Vec<Migration>> {
		Ok(self.migrations.clone())
	}
}

fn table(name: &str, columns: Vec<ColumnDefinition>) -> Operation {
	Operation::CreateTable {
		name: name.to_string(),
		columns,
		constraints: Vec::new(),
		without_rowid: None,
		interleave_in_parent: None,
		partition: None,
	}
}

fn representative_migration() -> Migration {
	let mut migration = Migration::new("0001_visibility", "catalog");
	migration.operations = vec![
		table(
			"visibility_items",
			vec![ColumnDefinition::new("id", FieldType::Integer)],
		),
		Operation::AddColumn {
			table: "visibility_items".to_string(),
			column: ColumnDefinition::new("title", FieldType::Text),
			mysql_options: None,
		},
		Operation::DropTable {
			name: "visibility_archive".to_string(),
		},
	];
	migration
}

fn dialect(connection: &DatabaseConnection) -> SqlDialect {
	match connection.database_type() {
		DatabaseType::Postgres => SqlDialect::Postgres,
		DatabaseType::Mysql => SqlDialect::Mysql,
		DatabaseType::Sqlite => SqlDialect::Sqlite,
	}
}

async fn table_exists(connection: &DatabaseConnection, table_name: &str) -> bool {
	match connection.database_type() {
		DatabaseType::Postgres => {
			let pool = connection
				.into_postgres()
				.expect("PostgreSQL fixture should expose its pool");
			sqlx::query_scalar::<_, bool>(
				"SELECT EXISTS (
					SELECT 1 FROM information_schema.tables
					WHERE table_schema = 'public' AND table_name = $1
				)",
			)
			.bind(table_name)
			.fetch_one(&pool)
			.await
			.expect("PostgreSQL table existence query should succeed")
		}
		DatabaseType::Mysql => {
			let pool = connection
				.into_mysql()
				.expect("MySQL fixture should expose its pool");
			sqlx::query_scalar::<_, i64>(
				"SELECT COUNT(*) FROM information_schema.tables
				 WHERE table_schema = DATABASE() AND table_name = ?",
			)
			.bind(table_name)
			.fetch_one(&pool)
			.await
			.expect("MySQL table existence query should succeed")
				> 0
		}
		DatabaseType::Sqlite => connection
			.fetch_optional(
				"SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?",
				vec![table_name.into()],
			)
			.await
			.expect("SQLite table existence query should succeed")
			.is_some(),
	}
}

async fn column_exists(
	connection: &DatabaseConnection,
	table_name: &str,
	column_name: &str,
) -> bool {
	match connection.database_type() {
		DatabaseType::Postgres => {
			let pool = connection
				.into_postgres()
				.expect("PostgreSQL fixture should expose its pool");
			sqlx::query_scalar::<_, bool>(
				"SELECT EXISTS (
					SELECT 1 FROM information_schema.columns
					WHERE table_schema = 'public' AND table_name = $1 AND column_name = $2
				)",
			)
			.bind(table_name)
			.bind(column_name)
			.fetch_one(&pool)
			.await
			.expect("PostgreSQL column existence query should succeed")
		}
		DatabaseType::Mysql => {
			let pool = connection
				.into_mysql()
				.expect("MySQL fixture should expose its pool");
			sqlx::query_scalar::<_, i64>(
				"SELECT COUNT(*) FROM information_schema.columns
				 WHERE table_schema = DATABASE() AND table_name = ? AND column_name = ?",
			)
			.bind(table_name)
			.bind(column_name)
			.fetch_one(&pool)
			.await
			.expect("MySQL column existence query should succeed")
				> 0
		}
		DatabaseType::Sqlite => {
			let pool = connection
				.into_sqlite()
				.expect("SQLite fixture should expose its pool");
			sqlx::query_scalar::<_, String>(&format!(
				"SELECT name FROM pragma_table_info('{table_name}') WHERE name = ?"
			))
			.bind(column_name)
			.fetch_optional(&pool)
			.await
			.expect("SQLite column existence query should succeed")
			.is_some()
		}
	}
}

async fn assert_collected_sql_matches_executor(
	connection: DatabaseConnection,
	expected_create_table: &str,
) {
	connection
		.execute("CREATE TABLE visibility_archive (id INTEGER)", Vec::new())
		.await
		.expect("pre-existing archive table should be installed");
	assert!(!table_exists(&connection, "visibility_items").await);
	assert!(!column_exists(&connection, "visibility_items", "title").await);
	assert!(table_exists(&connection, "visibility_archive").await);
	let migration = representative_migration();
	let plan = plan_migration_sql(
		&connection,
		&migration,
		&ProjectState::new(),
		MigrationDirection::Forward,
	)
	.await
	.expect("representative migration should be collectable");
	let sql_dialect = dialect(&connection);
	let collected = plan.render(sql_dialect);
	let executable = plan
		.statements
		.iter()
		.filter_map(|statement| match statement {
			PlannedStatement::Sql(sql) => {
				Some(sql.trim().trim_end_matches(';').trim_end().to_string())
			}
			PlannedStatement::Comment(_) => None,
		})
		.collect::<Vec<_>>();
	let rendered = collected
		.split(";\n")
		.map(str::trim)
		.filter(|statement| {
			!statement.is_empty() && *statement != "BEGIN" && *statement != "COMMIT"
		})
		.map(ToString::to_string)
		.collect::<Vec<_>>();
	let expected = match sql_dialect {
		SqlDialect::Mysql => vec![
			expected_create_table.to_string(),
			"ALTER TABLE `visibility_items` ADD COLUMN `title` TEXT".to_string(),
			"DROP TABLE `visibility_archive`".to_string(),
		],
		_ => vec![
			expected_create_table.to_string(),
			"ALTER TABLE visibility_items ADD COLUMN title TEXT".to_string(),
			"DROP TABLE visibility_archive".to_string(),
		],
	};

	assert_eq!(executable, expected);
	assert_eq!(rendered, executable);

	let mut executor = DatabaseMigrationExecutor::new(connection.clone());
	let result = executor
		.apply_migrations(std::slice::from_ref(&migration))
		.await
		.expect("executor should consume the representative SQL plan");
	assert_eq!(result.applied, vec!["catalog.0001_visibility".to_string()]);
	assert!(table_exists(&connection, "visibility_items").await);
	assert!(column_exists(&connection, "visibility_items", "title").await);
	assert!(!table_exists(&connection, "visibility_archive").await);
}

#[rstest]
#[tokio::test]
async fn postgres_collected_sql_matches_executor_visible_plan(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;
	let connection = DatabaseConnection::connect_postgres(&url)
		.await
		.expect("PostgreSQL visibility fixture should connect");

	assert_collected_sql_matches_executor(
		connection,
		"CREATE TABLE visibility_items (\n  id INTEGER\n)",
	)
	.await;
}

#[rstest]
#[tokio::test]
async fn mysql_collected_sql_matches_executor_visible_plan(
	#[future] mysql_container: (ContainerAsync<GenericImage>, Arc<MySqlPool>, u16, String),
) {
	let (_container, _pool, _port, url) = mysql_container.await;
	let connection = DatabaseConnection::connect_mysql(&url)
		.await
		.expect("MySQL visibility fixture should connect");

	assert_collected_sql_matches_executor(
		connection,
		"CREATE TABLE `visibility_items` (\n  `id` INTEGER\n)",
	)
	.await;
}

#[tokio::test]
async fn sqlite_collected_sql_matches_executor_visible_plan() {
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("SQLite visibility fixture should connect");

	assert_collected_sql_matches_executor(
		connection,
		"CREATE TABLE visibility_items (\n  id INTEGER\n)",
	)
	.await;
}

fn write_migration(repository: &FilesystemRepository, migration: &Migration) {
	let source = repository
		.render(
			migration,
			MigrationRenderOptions {
				include_header: false,
			},
		)
		.expect("visibility migration fixture should render");
	repository
		.create_new_source(&migration.app_label, &migration.name, &source)
		.expect("visibility migration fixture should be created");
}

#[tokio::test]
async fn showmigrations_keeps_missing_history_read_only_and_filters_cross_app_dependencies() {
	let project = TempDir::new().expect("visibility fixture directory should be created");
	let repository = FilesystemRepository::new(project.path().join("migrations"));
	let auth = Migration::new("0001_initial", "auth");
	let mut polls = Migration::new("0001_initial", "polls");
	polls.dependencies = vec![("auth".to_string(), "0001_initial".to_string())];
	write_migration(&repository, &auth);
	write_migration(&repository, &polls);
	let database = project.path().join("visibility.sqlite3");
	let url = format!("sqlite:///{}", database.display());
	let writer = Arc::new(RecordingWriter::default());
	let command = ShowMigrationsCommand::with_writer(writer.clone());
	let mut context = CommandContext::new(vec!["polls".to_string()]);
	context.set_option("database-url".to_string(), url.clone());
	context.set_option(
		"migrations-dir".to_string(),
		project
			.path()
			.join("migrations")
			.to_string_lossy()
			.into_owned(),
	);
	context.set_option("plan".to_string(), "true".to_string());

	command
		.execute(&context)
		.await
		.expect("missing history should be treated as an empty snapshot");

	assert_eq!(
		writer.take(),
		vec!["[ ] auth.0001_initial\n[ ] polls.0001_initial\n".to_string()],
	);
	let connection = DatabaseConnection::connect_sqlite(&url)
		.await
		.expect("SQLite visibility fixture should reconnect");
	assert!(!table_exists(&connection, "reinhardt_migrations").await);

	let recorder = DatabaseMigrationRecorder::new(connection);
	recorder
		.ensure_schema_table()
		.await
		.expect("visibility recorder table should be created explicitly by the test");
	recorder
		.record_applied("auth", "0001_initial")
		.await
		.expect("fixture applied timestamp should be recorded");
	let recorded = recorder
		.get_applied_migrations_if_present()
		.await
		.expect("fixture applied timestamp should remain readable");
	let applied_at = recorded[0].applied.format("%Y-%m-%d %H:%M:%S");
	context.options.remove("plan");
	context.set_verbosity(2);

	command
		.execute(&context)
		.await
		.expect("timestamped visibility output should succeed");

	let output = writer.take();
	assert_eq!(
		output,
		vec![format!(
			"auth\n [X] 0001_initial (applied at {applied_at})\npolls\n [ ] 0001_initial\n"
		)],
	);
}

#[tokio::test]
async fn sqlite_recreation_is_collected_and_consumed_by_execution() {
	let connection = DatabaseConnection::connect_sqlite("sqlite::memory:")
		.await
		.expect("SQLite recreation fixture should connect");
	let initial = Migration::new("0001_initial", "catalog").add_operation(table(
		"visibility_books",
		vec![
			ColumnDefinition::new("id", FieldType::Integer),
			ColumnDefinition::new("obsolete", FieldType::Text),
		],
	));
	let mut remove_obsolete = Migration::new("0002_remove_obsolete", "catalog");
	remove_obsolete.dependencies = vec![("catalog".to_string(), "0001_initial".to_string())];
	remove_obsolete.operations = vec![Operation::DropColumn {
		table: "visibility_books".to_string(),
		column: "obsolete".to_string(),
		old_definition: Some(ColumnDefinition::new("obsolete", FieldType::Text)),
	}];
	let catalog = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![initial.clone(), remove_obsolete.clone()],
	})
	.await
	.expect("SQLite recreation catalog should load");
	let mut executor = DatabaseMigrationExecutor::new(connection.clone());
	executor
		.apply_migrations(std::slice::from_ref(&initial))
		.await
		.expect("initial SQLite schema should be installed");
	let key = MigrationKey::new("catalog", "0002_remove_obsolete");
	let plan = reinhardt_db::migrations::plan_migration_sql_with_states(
		&connection,
		&remove_obsolete,
		&catalog
			.state_before(&key)
			.expect("state before recreation should load"),
		&catalog
			.state_after(&key)
			.expect("state after recreation should load"),
		MigrationDirection::Forward,
	)
	.await
	.expect("SQLite recreation SQL should collect");
	let collected = plan.render(SqlDialect::Sqlite);

	assert!(collected.contains("CREATE TABLE \"visibility_books_new\""));
	assert!(
		collected.contains(
			"INSERT INTO \"visibility_books_new\" (\"id\") SELECT \"id\" FROM \"visibility_books\";"
		),
		"{collected}",
	);

	executor
		.apply_migrations(std::slice::from_ref(&remove_obsolete))
		.await
		.expect("SQLite executor should consume recreation statements");
	assert!(column_exists(&connection, "visibility_books", "id").await);
	assert!(!column_exists(&connection, "visibility_books", "obsolete").await);
}

#[tokio::test]
async fn irreversible_rollback_produces_no_partial_sql_or_history_write() {
	let project = TempDir::new().expect("visibility fixture directory should be created");
	let repository = FilesystemRepository::new(project.path().join("migrations"));
	let migration = Migration::new("0001_irreversible", "audit").add_operation(Operation::RunSQL {
		sql: "SELECT 1".to_string(),
		reverse_sql: None,
	});
	write_migration(&repository, &migration);
	let database = project.path().join("visibility.sqlite3");
	let url = format!("sqlite:///{}", database.display());
	let writer = Arc::new(RecordingWriter::default());
	let command = SqlMigrateCommand::with_writer(writer.clone());
	let mut context = CommandContext::new(vec!["audit".to_string(), "0001".to_string()]);
	context.set_option("database-url".to_string(), url.clone());
	context.set_option(
		"migrations-dir".to_string(),
		project
			.path()
			.join("migrations")
			.to_string_lossy()
			.into_owned(),
	);
	context.set_option("backwards".to_string(), "true".to_string());

	let error = command
		.execute(&context)
		.await
		.expect_err("irreversible rollback should fail before stdout");

	assert!(
		error.to_string().contains("Irreversible migration"),
		"{error}"
	);
	assert!(writer.take().is_empty());
	let connection = DatabaseConnection::connect_sqlite(&url)
		.await
		.expect("SQLite visibility fixture should reconnect");
	assert!(!table_exists(&connection, "reinhardt_migrations").await);
}
