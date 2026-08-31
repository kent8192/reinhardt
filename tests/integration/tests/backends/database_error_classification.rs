//! Portable database error classification across supported SQL backends.

use reinhardt_core::exception::Error;
use reinhardt_db::DatabaseErrorKind;
use reinhardt_db::{
	backends::DatabaseConnection as BackendsConnection,
	orm::connection::{DatabaseBackend, DatabaseConnection, DatabaseConnectionLease},
};
use reinhardt_query::prelude::{
	ColumnDef, Expr, ExprTrait, Iden, IntoIden, MySqlQueryBuilder, PostgresQueryBuilder, Query,
	QueryStatementWriter, SqliteQueryBuilder, Value,
};
#[cfg(feature = "postgres")]
use reinhardt_test::fixtures::postgres_container;
use rstest::*;
#[cfg(feature = "postgres")]
use sqlx::PgPool;
#[cfg(feature = "postgres")]
use std::sync::Arc;
#[cfg(feature = "postgres")]
use testcontainers::{ContainerAsync, GenericImage};

#[derive(Debug, Clone, Copy, Iden)]
enum ErrorKindParents {
	Table,
	Id,
}

#[derive(Debug, Clone, Copy, Iden)]
enum ErrorKindRecords {
	Table,
	Id,
	UniqueValue,
	ParentId,
	RequiredValue,
	Quantity,
}

const PORTABLE_CONSTRAINT_KINDS: [DatabaseErrorKind; 4] = [
	DatabaseErrorKind::UniqueViolation,
	DatabaseErrorKind::ForeignKeyViolation,
	DatabaseErrorKind::NotNullViolation,
	DatabaseErrorKind::CheckViolation,
];

fn sql_for_backend(statement: &impl QueryStatementWriter, backend: DatabaseBackend) -> String {
	match backend {
		DatabaseBackend::Postgres => statement.to_string(PostgresQueryBuilder),
		DatabaseBackend::MySql => statement.to_string(MySqlQueryBuilder),
		DatabaseBackend::Sqlite => statement.to_string(SqliteQueryBuilder),
	}
}

fn parent_insert_sql(backend: DatabaseBackend) -> String {
	let mut statement = Query::insert();
	statement
		.into_table(ErrorKindParents::Table.into_iden())
		.columns([ErrorKindParents::Id])
		.values_panic([Value::BigInt(Some(1))]);

	sql_for_backend(&statement, backend)
}

fn record_insert_sql(
	backend: DatabaseBackend,
	id: i64,
	unique_value: &str,
	parent_id: i64,
	required_value: Option<&str>,
	quantity: i32,
) -> String {
	let mut statement = Query::insert();
	statement
		.into_table(ErrorKindRecords::Table.into_iden())
		.columns([
			ErrorKindRecords::Id,
			ErrorKindRecords::UniqueValue,
			ErrorKindRecords::ParentId,
			ErrorKindRecords::RequiredValue,
			ErrorKindRecords::Quantity,
		])
		.values_panic([
			Value::BigInt(Some(id)),
			Value::String(Some(Box::new(unique_value.to_owned()))),
			Value::BigInt(Some(parent_id)),
			Value::String(required_value.map(|value| Box::new(value.to_owned()))),
			Value::Int(Some(quantity)),
		]);

	sql_for_backend(&statement, backend)
}

async fn execute_constraint_error(connection: &DatabaseConnection, sql: &str) -> Error {
	connection
		.execute(sql, vec![])
		.await
		.expect_err("the invalid statement must fail")
}

fn source_chain_contains_sqlx(error: &(dyn std::error::Error + 'static)) -> bool {
	std::iter::successors(Some(error), |current| current.source())
		.any(|source| source.downcast_ref::<sqlx::Error>().is_some())
}

fn assert_database_kind(error: Error, expected: DatabaseErrorKind) {
	assert_eq!(error.database_kind(), Some(expected));
}

async fn create_portable_schema(connection: &DatabaseConnection) {
	let backend = connection.backend();
	let mut parent_table = Query::create_table();
	parent_table.table(ErrorKindParents::Table.into_iden()).col(
		ColumnDef::new(ErrorKindParents::Id)
			.big_integer()
			.primary_key(true),
	);
	let mut record_table = Query::create_table();
	record_table
		.table(ErrorKindRecords::Table.into_iden())
		.col(
			ColumnDef::new(ErrorKindRecords::Id)
				.big_integer()
				.primary_key(true),
		)
		.col(
			ColumnDef::new(ErrorKindRecords::UniqueValue)
				.string_len(255)
				.not_null(true),
		)
		.col(
			ColumnDef::new(ErrorKindRecords::ParentId)
				.big_integer()
				.not_null(true),
		)
		.col(
			ColumnDef::new(ErrorKindRecords::RequiredValue)
				.string_len(255)
				.not_null(true),
		)
		.col(
			ColumnDef::new(ErrorKindRecords::Quantity)
				.integer()
				.not_null(true)
				.check(Expr::col(ErrorKindRecords::Quantity).gt(0)),
		)
		.unique([ErrorKindRecords::UniqueValue])
		.foreign_key(
			[ErrorKindRecords::ParentId],
			ErrorKindParents::Table.into_iden(),
			[ErrorKindParents::Id],
			None,
			None,
		);

	connection
		.execute(&sql_for_backend(&parent_table, backend), vec![])
		.await
		.expect("the parent table must be created");
	connection
		.execute(&sql_for_backend(&record_table, backend), vec![])
		.await
		.expect("the record table must be created");
	connection
		.execute(&parent_insert_sql(backend), vec![])
		.await
		.expect("the parent row must be inserted");
	connection
		.execute(
			&record_insert_sql(backend, 1, "duplicate", 1, Some("present"), 1),
			vec![],
		)
		.await
		.expect("the baseline row must be inserted");
}

async fn portable_constraint_errors(connection: &DatabaseConnection) -> Vec<Error> {
	let backend = connection.backend();
	let unique = record_insert_sql(backend, 2, "duplicate", 1, Some("present"), 1);
	let foreign_key = record_insert_sql(backend, 3, "foreign-key", 999, Some("present"), 1);
	let not_null = record_insert_sql(backend, 4, "not-null", 1, None, 1);
	let check = record_insert_sql(backend, 5, "check", 1, Some("present"), 0);

	vec![
		execute_constraint_error(connection, &unique).await,
		execute_constraint_error(connection, &foreign_key).await,
		execute_constraint_error(connection, &not_null).await,
		execute_constraint_error(connection, &check).await,
	]
}

fn assert_portable_constraint_errors(errors: Vec<Error>) {
	assert_eq!(
		errors
			.iter()
			.map(|error| error.database_kind())
			.collect::<Vec<_>>(),
		PORTABLE_CONSTRAINT_KINDS.map(Some)
	);
	for error in errors {
		let database_error = error
			.database_error()
			.expect("the execution failure must be classified as a database error");
		assert_eq!(database_error.constraint(), None);
		assert_eq!(database_error.table(), None);
		assert_eq!(database_error.columns(), []);
		assert!(source_chain_contains_sqlx(&error));
	}
}

#[cfg(feature = "postgres")]
#[rstest]
#[tokio::test]
async fn postgres_constraint_errors_have_portable_kinds(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	// Arrange
	let (_container, _pool, _port, url) = postgres_container.await;
	let owner = BackendsConnection::connect(&url)
		.await
		.expect("the PostgreSQL fixture must accept framework connections");
	let lease =
		DatabaseConnectionLease::register(owner).expect("connection registration must succeed");
	let connection = lease.handle();
	create_portable_schema(&connection).await;

	// Act
	let errors = portable_constraint_errors(&connection).await;

	// Assert
	assert_portable_constraint_errors(errors);
}

#[cfg(feature = "sqlite")]
#[rstest]
#[tokio::test]
async fn sqlite_constraint_errors_have_portable_kinds() {
	// Arrange
	let owner = BackendsConnection::connect("sqlite::memory:")
		.await
		.expect("the in-memory SQLite database must connect");
	let lease =
		DatabaseConnectionLease::register(owner).expect("connection registration must succeed");
	let connection = lease.handle();
	// reinhardt-query has no builder for this backend session directive.
	connection
		.execute("PRAGMA foreign_keys = ON", vec![])
		.await
		.expect("SQLite foreign-key enforcement must be enabled");
	create_portable_schema(&connection).await;

	// Act
	let errors = portable_constraint_errors(&connection).await;

	// Assert
	assert_portable_constraint_errors(errors);
}

#[cfg(feature = "mysql")]
#[rstest]
#[tokio::test]
async fn mysql_constraint_errors_have_portable_kinds() {
	use reinhardt_test::{MySqlContainer, TestDatabase};

	// Arrange
	let container = MySqlContainer::new().await;
	container
		.wait_ready()
		.await
		.expect("the MySQL container must become ready");
	let owner = BackendsConnection::connect(&container.connection_url())
		.await
		.expect("the MySQL fixture must accept framework connections");
	let lease =
		DatabaseConnectionLease::register(owner).expect("connection registration must succeed");
	let connection = lease.handle();
	create_portable_schema(&connection).await;

	// Act
	let errors = portable_constraint_errors(&connection).await;

	// Assert
	assert_portable_constraint_errors(errors);
}

#[cfg(feature = "postgres")]
#[rstest]
#[tokio::test]
async fn postgres_constraint_errors_retain_structured_metadata(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) {
	let (_container, _pool, _port, url) = postgres_container.await;
	let owner = BackendsConnection::connect(&url)
		.await
		.expect("the PostgreSQL fixture must accept framework connections");
	let lease =
		DatabaseConnectionLease::register(owner).expect("connection registration must succeed");
	let connection = lease.handle();
	for statement in [
		"CREATE TABLE records_parent (id BIGINT PRIMARY KEY)",
		"CREATE TABLE records (id BIGINT PRIMARY KEY, unique_value TEXT CONSTRAINT \"records.unique\" UNIQUE, parent_id BIGINT CONSTRAINT records_parent_fk REFERENCES records_parent(id), required_value TEXT NOT NULL, quantity INTEGER CONSTRAINT records_quantity_check CHECK (quantity > 0))",
		"INSERT INTO records_parent (id) VALUES (1)",
		"INSERT INTO records (id, unique_value, parent_id, required_value, quantity) VALUES (1, 'duplicate', 1, 'present', 1)",
	] {
		connection
			.execute(statement, vec![])
			.await
			.expect("the PostgreSQL metadata fixture must be created");
	}

	let errors = [
		execute_constraint_error(
			&connection,
			"INSERT INTO records (id, unique_value, parent_id, required_value, quantity) VALUES (2, 'duplicate', 1, 'present', 1)",
		)
		.await,
		execute_constraint_error(
			&connection,
			"INSERT INTO records (id, unique_value, parent_id, required_value, quantity) VALUES (3, 'foreign-key', 999, 'present', 1)",
		)
		.await,
		execute_constraint_error(
			&connection,
			"INSERT INTO records (id, unique_value, parent_id, required_value, quantity) VALUES (4, 'not-null', 1, NULL, 1)",
		)
		.await,
		execute_constraint_error(
			&connection,
			"INSERT INTO records (id, unique_value, parent_id, required_value, quantity) VALUES (5, 'check', 1, 'present', 0)",
		)
		.await,
	];
	let expected = [
		(
			DatabaseErrorKind::UniqueViolation,
			Some("records.unique"),
			None,
		),
		(
			DatabaseErrorKind::ForeignKeyViolation,
			Some("records_parent_fk"),
			None,
		),
		(
			DatabaseErrorKind::NotNullViolation,
			None,
			Some("required_value"),
		),
		(
			DatabaseErrorKind::CheckViolation,
			Some("records_quantity_check"),
			None,
		),
	];

	for (error, (kind, constraint, column)) in errors.into_iter().zip(expected) {
		assert_eq!(error.database_kind(), Some(kind));
		let database_error = error.database_error().expect("database error metadata");
		assert_eq!(database_error.table(), Some("records"));
		assert_eq!(database_error.constraint(), constraint);
		assert_eq!(
			database_error.columns(),
			column.into_iter().collect::<Vec<_>>()
		);
		assert!(source_chain_contains_sqlx(&error));
	}
}

#[cfg(feature = "postgres")]
#[rstest]
#[tokio::test]
async fn unavailable_postgres_endpoint_is_classified_as_timeout() {
	// Arrange
	let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
		.expect("a local ephemeral port must be available");
	let address = listener
		.local_addr()
		.expect("the bound listener must have a local address");
	drop(listener);
	let url = format!(
		"postgres://postgres@{}:{}/postgres?connect_timeout=1",
		address.ip(),
		address.port()
	);

	// Act
	let result = BackendsConnection::connect(&url).await;

	// Assert
	let Err(error) = result else {
		panic!("a closed local endpoint must time out through the framework pool");
	};
	assert_database_kind(error, DatabaseErrorKind::Timeout);
}
