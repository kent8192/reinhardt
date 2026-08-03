#![cfg(all(feature = "postgres", feature = "mysql", feature = "sqlite"))]

use std::time::Duration;

use async_trait::async_trait;
use reinhardt_db::backends::DatabaseConnection;
use reinhardt_db::migrations::{
	DatabaseMigrationRecorder, Migration, MigrationCatalog, MigrationError, MigrationKey,
	MigrationSource, Result,
};
use serial_test::serial;
use tempfile::TempDir;
use testcontainers::{
	ContainerAsync, GenericImage, ImageExt,
	core::{IntoContainerPort, WaitFor},
	runners::AsyncRunner,
};

const MAX_CONNECT_RETRIES: u32 = 7;

struct TestSource {
	migrations: Vec<Migration>,
}

#[async_trait]
impl MigrationSource for TestSource {
	async fn all_migrations(&self) -> Result<Vec<Migration>> {
		Ok(self.migrations.clone())
	}
}

fn migration(app: &str, name: &str, dependencies: &[(&str, &str)]) -> Migration {
	let mut migration = Migration::new(name, app);
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

async fn connect_with_retry(url: &str) -> DatabaseConnection {
	for attempt in 0..=MAX_CONNECT_RETRIES {
		match DatabaseConnection::connect(url).await {
			Ok(connection) => return connection,
			Err(error) if attempt < MAX_CONNECT_RETRIES => {
				eprintln!(
					"database connection attempt {} of {} failed: {error}",
					attempt + 1,
					MAX_CONNECT_RETRIES + 1,
				);
				tokio::time::sleep(Duration::from_millis(200 * 2_u64.pow(attempt + 1))).await;
			}
			Err(error) => panic!(
				"database connection failed after {} attempts: {error}",
				MAX_CONNECT_RETRIES + 1,
			),
		}
	}

	unreachable!("the final database connection attempt either returns or panics")
}

async fn sqlite_connection() -> (TempDir, DatabaseConnection) {
	let directory = tempfile::Builder::new()
		.prefix("reinhardt-migration-visibility-")
		.tempdir_in("/tmp")
		.expect("SQLite temporary directory should be created under /tmp");
	let database_path = directory.path().join("visibility.sqlite");
	let connection =
		DatabaseConnection::connect_sqlite(&format!("sqlite:///{}", database_path.display()))
			.await
			.unwrap();
	(directory, connection)
}

async fn postgres_connection() -> (ContainerAsync<GenericImage>, DatabaseConnection, u16) {
	let container = GenericImage::new("postgres", "16-alpine")
		.with_exposed_port(5432.tcp())
		.with_wait_for(WaitFor::message_on_stderr(
			"database system is ready to accept connections",
		))
		.with_startup_timeout(Duration::from_secs(120))
		.with_env_var("POSTGRES_HOST_AUTH_METHOD", "trust")
		.start()
		.await
		.expect("PostgreSQL container should start");
	let port = container
		.get_host_port_ipv4(5432)
		.await
		.expect("PostgreSQL port should be available");
	let connection = connect_with_retry(&format!(
		"postgres://postgres@127.0.0.1:{port}/postgres?sslmode=disable"
	))
	.await;
	(container, connection, port)
}

async fn mysql_connection() -> (ContainerAsync<GenericImage>, DatabaseConnection) {
	let container = GenericImage::new("mysql", "8.0")
		.with_exposed_port(3306.tcp())
		.with_wait_for(WaitFor::message_on_stderr(
			"port: 3306  MySQL Community Server",
		))
		.with_startup_timeout(Duration::from_secs(120))
		.with_env_var("MYSQL_ROOT_PASSWORD", "test")
		.with_env_var("MYSQL_DATABASE", "migration_visibility")
		.start()
		.await
		.expect("MySQL container should start");
	let port = container
		.get_host_port_ipv4(3306)
		.await
		.expect("MySQL port should be available");
	let connection = connect_with_retry(&format!(
		"mysql://root:test@127.0.0.1:{port}/migration_visibility"
	))
	.await;
	(container, connection)
}

#[tokio::test]
async fn sqlite_absent_recorder_table_is_an_empty_applied_set() {
	let (_directory, connection) = sqlite_connection().await;
	let recorder = DatabaseMigrationRecorder::new(connection);

	let records = recorder.get_applied_migrations_if_present().await.unwrap();

	assert_eq!(records, Vec::new());
}

#[tokio::test]
#[serial(migration_visibility_containers)]
async fn postgres_absent_recorder_table_is_an_empty_applied_set() {
	let (_container, connection, _port) = postgres_connection().await;
	let recorder = DatabaseMigrationRecorder::new(connection);

	let records = recorder.get_applied_migrations_if_present().await.unwrap();

	assert_eq!(records, Vec::new());
}

#[tokio::test]
#[serial(migration_visibility_containers)]
async fn mysql_absent_recorder_table_is_an_empty_applied_set() {
	let (_container, connection) = mysql_connection().await;
	let recorder = DatabaseMigrationRecorder::new(connection);

	let records = recorder.get_applied_migrations_if_present().await.unwrap();

	assert_eq!(records, Vec::new());
}

#[tokio::test]
#[serial(migration_visibility_containers)]
async fn postgres_permission_errors_remain_errors() {
	let (_container, connection, port) = postgres_connection().await;
	let recorder = DatabaseMigrationRecorder::new(connection.clone());
	recorder.ensure_schema_table().await.unwrap();
	connection
		.execute("CREATE ROLE visibility_reader LOGIN", vec![])
		.await
		.unwrap();
	let reader = connect_with_retry(&format!(
		"postgres://visibility_reader@127.0.0.1:{port}/postgres?sslmode=disable"
	))
	.await;
	let recorder = DatabaseMigrationRecorder::new(reader);

	let error = recorder
		.get_applied_migrations_if_present()
		.await
		.unwrap_err();

	let MigrationError::DatabaseError(error) = error else {
		panic!("expected a database error");
	};
	assert_eq!(error.code(), Some("42501"));
}

#[tokio::test]
#[serial(migration_visibility_containers)]
async fn postgres_missing_relation_inside_recorder_view_remains_an_error() {
	let (_container, connection, _port) = postgres_connection().await;
	connection
		.execute(
			"CREATE FUNCTION migration_visibility_records()
			 RETURNS TABLE (app TEXT, name TEXT, applied TIMESTAMPTZ)
			 LANGUAGE plpgsql
			 AS $$
			 BEGIN
			     RETURN QUERY EXECUTE
			         'SELECT app, name, applied FROM visibility_missing_dependency';
			 END
			 $$",
			vec![],
		)
		.await
		.unwrap();
	connection
		.execute(
			"CREATE VIEW reinhardt_migrations AS
			 SELECT * FROM migration_visibility_records()",
			vec![],
		)
		.await
		.unwrap();
	let recorder = DatabaseMigrationRecorder::new(connection);

	let error = recorder
		.get_applied_migrations_if_present()
		.await
		.unwrap_err();

	let MigrationError::DatabaseError(error) = error else {
		panic!("expected a database error");
	};
	assert_eq!(error.code(), Some("42P01"));
	assert_eq!(
		error.message(),
		"relation \"visibility_missing_dependency\" does not exist"
	);
}

#[tokio::test]
#[serial(migration_visibility_containers)]
async fn mysql_missing_table_inside_recorder_view_remains_an_error() {
	let (_container, connection) = mysql_connection().await;
	let pool = connection
		.into_mysql()
		.expect("the fixture should expose a MySQL pool");
	sqlx::raw_sql(
		"CREATE FUNCTION migration_visibility_timestamp()
			 RETURNS DATETIME
			 READS SQL DATA
			 RETURN (
			     SELECT applied
			     FROM visibility_missing_dependency
			     LIMIT 1
			 )",
	)
	.execute(&pool)
	.await
	.unwrap();
	connection
		.execute(
			"CREATE VIEW reinhardt_migrations AS
			 SELECT
			     'blog' AS app,
			     '0001_initial' AS name,
			     migration_visibility_timestamp() AS applied",
			vec![],
		)
		.await
		.unwrap();
	let recorder = DatabaseMigrationRecorder::new(connection);

	let error = recorder
		.get_applied_migrations_if_present()
		.await
		.unwrap_err();

	let MigrationError::DatabaseError(error) = error else {
		panic!("expected a database error");
	};
	assert_eq!(error.code(), Some("HY000"));
	assert_eq!(
		error.message(),
		"View 'migration_visibility.reinhardt_migrations' references invalid table(s) or column(s) \
		 or function(s) or definer/invoker of view lack rights to use them"
	);
}

#[tokio::test]
async fn malformed_recorder_schema_remains_an_error() {
	let (_directory, connection) = sqlite_connection().await;
	connection
		.execute(
			"CREATE VIEW reinhardt_migrations AS
			 SELECT missing_visibility_function() AS app,
			        '0001_initial' AS name,
			        CURRENT_TIMESTAMP AS applied",
			vec![],
		)
		.await
		.unwrap();
	let recorder = DatabaseMigrationRecorder::new(connection);

	let error = recorder
		.get_applied_migrations_if_present()
		.await
		.unwrap_err();

	assert!(matches!(error, MigrationError::DatabaseError(_)));
}

#[tokio::test]
async fn recorder_type_errors_remain_errors() {
	let (_directory, connection) = sqlite_connection().await;
	connection
		.execute(
			"CREATE TABLE reinhardt_migrations (
				app TEXT NOT NULL,
				name TEXT NOT NULL,
				applied TEXT NOT NULL
			)",
			vec![],
		)
		.await
		.unwrap();
	connection
		.execute(
			"INSERT INTO reinhardt_migrations (app, name, applied)
			 VALUES ('blog', '0001_initial', 'not-a-timestamp')",
			vec![],
		)
		.await
		.unwrap();
	let recorder = DatabaseMigrationRecorder::new(connection);

	let error = recorder
		.get_applied_migrations_if_present()
		.await
		.unwrap_err();

	let MigrationError::DatabaseError(error) = error else {
		panic!("expected a database error");
	};
	assert_eq!(
		error.kind(),
		reinhardt_db::backends::DatabaseErrorKind::Type
	);
}

#[tokio::test]
async fn snapshot_filters_apps_with_transitive_cross_app_dependencies() {
	let catalog = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![
			migration("auth", "0001_initial", &[]),
			migration("blog", "0001_initial", &[("auth", "0001_initial")]),
			migration("blog", "0002_posts", &[("blog", "0001_initial")]),
			migration("reports", "0001_initial", &[]),
		],
	})
	.await
	.unwrap();
	let (_directory, connection) = sqlite_connection().await;
	let recorder = DatabaseMigrationRecorder::new(connection);
	recorder.ensure_schema_table().await.unwrap();
	recorder
		.record_applied("auth", "0001_initial")
		.await
		.unwrap();
	let applied = recorder.get_applied_migrations().await.unwrap();

	let snapshot = catalog
		.snapshot(&recorder, &["blog".to_string()])
		.await
		.unwrap();

	let ordered_keys: Vec<MigrationKey> = snapshot
		.ordered
		.iter()
		.map(|migration| MigrationKey::new(&migration.app_label, &migration.name))
		.collect();
	assert_eq!(
		ordered_keys,
		vec![
			MigrationKey::new("auth", "0001_initial"),
			MigrationKey::new("blog", "0001_initial"),
			MigrationKey::new("blog", "0002_posts"),
		]
	);
	assert_eq!(
		snapshot
			.applied
			.get(&MigrationKey::new("auth", "0001_initial")),
		Some(&applied[0].applied)
	);
	assert_eq!(snapshot.applied.len(), 1);
}

#[tokio::test]
async fn snapshot_keeps_originals_visible_during_partial_replacement_history() {
	let first = migration("blog", "0001_initial", &[]);
	let second = migration("blog", "0002_posts", &[("blog", "0001_initial")]);
	let mut replacement = migration("blog", "0001_squashed_0002_posts", &[]);
	replacement.replaces = vec![
		("blog".to_string(), "0001_initial".to_string()),
		("blog".to_string(), "0002_posts".to_string()),
	];
	let catalog = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![first, second, replacement],
	})
	.await
	.unwrap();
	let (_directory, connection) = sqlite_connection().await;
	let recorder = DatabaseMigrationRecorder::new(connection);
	recorder.ensure_schema_table().await.unwrap();
	recorder
		.record_applied("blog", "0001_initial")
		.await
		.unwrap();

	let snapshot = catalog.snapshot(&recorder, &[]).await.unwrap();
	let ordered_keys: Vec<_> = snapshot
		.ordered
		.iter()
		.map(|migration| MigrationKey::new(&migration.app_label, &migration.name))
		.collect();

	assert_eq!(
		ordered_keys,
		vec![
			MigrationKey::new("blog", "0001_initial"),
			MigrationKey::new("blog", "0002_posts"),
		]
	);
	assert!(
		snapshot
			.applied
			.contains_key(&MigrationKey::new("blog", "0001_initial"))
	);
	assert!(
		!snapshot
			.applied
			.contains_key(&MigrationKey::new("blog", "0002_posts"))
	);
}

#[tokio::test]
async fn snapshot_rejects_an_unknown_app() {
	let catalog = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![migration("blog", "0001_initial", &[])],
	})
	.await
	.unwrap();
	let (_directory, connection) = sqlite_connection().await;
	let recorder = DatabaseMigrationRecorder::new(connection);

	let error = catalog
		.snapshot(&recorder, &["missing".to_string()])
		.await
		.unwrap_err();

	assert_eq!(error.to_string(), "Migration not found: app missing");
}

#[tokio::test]
async fn strict_catalog_rejects_cycles_before_a_snapshot_can_be_built() {
	let error = MigrationCatalog::load_strict(&TestSource {
		migrations: vec![
			migration("auth", "0001_initial", &[("blog", "0001_initial")]),
			migration("blog", "0001_initial", &[("auth", "0001_initial")]),
		],
	})
	.await
	.unwrap_err();

	assert_eq!(
		error.to_string(),
		"Circular dependency detected: auth.0001_initial, blog.0001_initial"
	);
}
