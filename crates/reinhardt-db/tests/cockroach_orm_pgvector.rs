#![cfg(feature = "pgvector")]
// The model macro emits the framework's native cfg gate into this standalone
// integration-test crate, where Cargo does not declare that cfg name.
#![allow(unexpected_cfgs)]

use std::any::Any;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use reinhardt_core::exception::{DatabaseErrorKind, Error};
use reinhardt_core::macros::model;
use reinhardt_db::{
	backends::{
		DatabaseConnection as BackendsConnection,
		backend::DatabaseBackend as BackendsDatabaseBackend,
		error::Result as BackendResult,
		types::{DatabaseType, QueryResult, QueryValue, Row, TransactionExecutor},
	},
	orm::{
		DatabaseConnectionLease, DatabaseValue, Model, Vector,
		manager::replace_database_connection_for_testing,
		query::{FieldAssignment, Filter, FilterOperator, FilterValue, QuerySet, UpdateValue},
		transaction::IsolationLevel,
	},
};
use serde::{Deserialize, Serialize};

#[model(
	app_label = "cockroach_orm_pgvector",
	table_name = "cockroach_orm_documents"
)]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct CockroachDocument {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 64)]
	name: String,
	embedding: Vector<3>,
}

#[derive(Default)]
struct Recorder {
	sql: Mutex<Vec<String>>,
}

impl Recorder {
	fn record(&self, sql: &str) {
		self.sql
			.lock()
			.expect("SQL recorder lock should remain available")
			.push(sql.to_owned());
	}

	fn statements(&self) -> Vec<String> {
		self.sql
			.lock()
			.expect("SQL recorder lock should remain available")
			.clone()
	}
}

struct RecordingPostgresBackend {
	recorder: Arc<Recorder>,
}

struct RecordingPostgresTransaction {
	recorder: Arc<Recorder>,
}

#[async_trait]
impl BackendsDatabaseBackend for RecordingPostgresBackend {
	fn database_type(&self) -> DatabaseType {
		DatabaseType::Postgres
	}

	fn placeholder(&self, index: usize) -> String {
		format!("${index}")
	}

	fn supports_returning(&self) -> bool {
		true
	}

	fn supports_on_conflict(&self) -> bool {
		true
	}

	async fn execute(&self, sql: &str, _params: Vec<QueryValue>) -> BackendResult<QueryResult> {
		self.recorder.record(sql);
		Ok(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
	}

	async fn fetch_one(&self, sql: &str, _params: Vec<QueryValue>) -> BackendResult<Row> {
		self.recorder.record(sql);
		Ok(Row::default())
	}

	async fn fetch_all(&self, sql: &str, _params: Vec<QueryValue>) -> BackendResult<Vec<Row>> {
		self.recorder.record(sql);
		Ok(Vec::new())
	}

	async fn fetch_optional(
		&self,
		sql: &str,
		_params: Vec<QueryValue>,
	) -> BackendResult<Option<Row>> {
		self.recorder.record(sql);
		Ok(None)
	}

	async fn begin(&self) -> BackendResult<Box<dyn TransactionExecutor>> {
		Ok(Box::new(RecordingPostgresTransaction {
			recorder: self.recorder.clone(),
		}))
	}

	fn as_any(&self) -> &dyn Any {
		self
	}
}

#[async_trait]
impl TransactionExecutor for RecordingPostgresTransaction {
	fn backend(&self) -> DatabaseType {
		DatabaseType::Postgres
	}

	async fn execute(&mut self, sql: &str, _params: Vec<QueryValue>) -> BackendResult<QueryResult> {
		self.recorder.record(sql);
		Ok(QueryResult {
			rows_affected: 1,
			last_insert_id: None,
		})
	}

	async fn fetch_one(&mut self, sql: &str, _params: Vec<QueryValue>) -> BackendResult<Row> {
		self.recorder.record(sql);
		Ok(Row::default())
	}

	async fn fetch_all(&mut self, sql: &str, _params: Vec<QueryValue>) -> BackendResult<Vec<Row>> {
		self.recorder.record(sql);
		Ok(Vec::new())
	}

	async fn fetch_optional(
		&mut self,
		sql: &str,
		_params: Vec<QueryValue>,
	) -> BackendResult<Option<Row>> {
		self.recorder.record(sql);
		Ok(None)
	}

	async fn commit(self: Box<Self>) -> BackendResult<()> {
		Ok(())
	}

	async fn rollback(self: Box<Self>) -> BackendResult<()> {
		Ok(())
	}
}

fn vector(values: [f32; 3]) -> Vector<3> {
	Vector::try_from_slice(&values).expect("test vectors should be valid")
}

fn document(id: Option<i64>) -> CockroachDocument {
	CockroachDocument {
		id,
		name: "document".to_owned(),
		embedding: vector([1.0, 2.0, 3.0]),
	}
}

fn vector_query() -> QuerySet<CockroachDocument> {
	QuerySet::new().select_expr(
		"distance",
		CockroachDocument::new_fields()
			.embedding
			.cosine_distance(vector([3.0, 2.0, 1.0])),
	)
}

fn assert_cockroach_pgvector_error(error: &Error) {
	let database_error = error
		.database_error()
		.expect("query-build failure should retain structured database context");
	assert_eq!(database_error.kind(), DatabaseErrorKind::Unsupported);
	assert_eq!(
		database_error.message(),
		"pgvector distance operators is not supported by the CockroachDB backend"
	);
}

fn assert_cockroach_pgvector_value_error(error: &Error) {
	let database_error = error
		.database_error()
		.expect("query-build failure should retain structured database context");
	assert_eq!(database_error.kind(), DatabaseErrorKind::Unsupported);
	assert_eq!(
		database_error.message(),
		"pgvector values is not supported by the CockroachDB backend"
	);
}

fn connection_with_flavor(
	is_cockroachdb: bool,
	recorder: Arc<Recorder>,
) -> DatabaseConnectionLease {
	DatabaseConnectionLease::register(owner_with_flavor(is_cockroachdb, recorder))
		.expect("recording connection should register")
}

fn owner_with_flavor(is_cockroachdb: bool, recorder: Arc<Recorder>) -> BackendsConnection {
	let backend = Arc::new(RecordingPostgresBackend { recorder });
	BackendsConnection::new_with_flavor(backend, is_cockroachdb)
}

#[tokio::test]
async fn cockroach_connection_rejects_vector_select_insert_and_update_before_execution() {
	let recorder = Arc::new(Recorder::default());
	let lease = connection_with_flavor(true, recorder.clone());
	let mut connection = lease.handle();

	let select_error = match vector_query().rows_with_db(&mut connection).await {
		Ok(_) => panic!("CockroachDB should reject vector SELECT expressions"),
		Err(error) => error,
	};
	assert_cockroach_pgvector_error(&select_error);

	let insert_error = CockroachDocument::objects()
		.create_with_conn(&mut connection, &document(None))
		.await
		.expect_err("CockroachDB should reject vector INSERT values");
	assert_eq!(
		insert_error
			.database_error()
			.expect("insert failure should retain structured database context")
			.message(),
		"pgvector values is not supported by the CockroachDB backend"
	);

	let update_error = QuerySet::<CockroachDocument>::new()
		.filter(Filter::new("id", FilterOperator::Eq, FilterValue::Int(7)))
		.update_fields_with_conn(
			&mut connection,
			[FieldAssignment::new(
				"embedding",
				UpdateValue::Typed(Ok(DatabaseValue::Vector(vec![4.0, 5.0, 6.0]))),
			)],
		)
		.await
		.expect_err("CockroachDB should reject vector UPDATE values");
	assert_eq!(
		update_error
			.database_error()
			.expect("update failure should retain structured database context")
			.message(),
		"pgvector values is not supported by the CockroachDB backend"
	);

	assert_eq!(
		recorder.statements(),
		Vec::<String>::new(),
		"unsupported vector statements must fail before reaching the backend"
	);
}

#[tokio::test]
async fn cockroach_connection_keeps_postgres_compatible_orm_sql_working() {
	let recorder = Arc::new(Recorder::default());
	let lease = connection_with_flavor(true, recorder.clone());
	let mut connection = lease.handle();

	let rows = QuerySet::<CockroachDocument>::new()
		.values(&["id", "name"])
		.rows_with_db(&mut connection)
		.await
		.expect("ordinary CockroachDB SELECT should remain PostgreSQL-compatible");
	assert!(rows.is_empty());

	CockroachDocument::objects()
		.delete_with_conn(&mut connection, 7)
		.await
		.expect("ordinary CockroachDB DELETE should remain PostgreSQL-compatible");

	assert_eq!(
		recorder.statements(),
		vec![
			r#"SELECT "id", "name" FROM "cockroach_orm_documents""#.to_owned(),
			r#"DELETE FROM "cockroach_orm_documents" WHERE "id" = $1"#.to_owned(),
		]
	);
}

#[tokio::test]
async fn cockroach_flavor_survives_atomic_executor_construction() {
	let recorder = Arc::new(Recorder::default());
	let lease = connection_with_flavor(true, recorder.clone());
	let connection = lease.handle();

	let error = connection
		.atomic(async |transaction| {
			vector_query().rows_with_db(transaction).await?;
			Ok::<_, Error>(())
		})
		.await
		.expect_err("CockroachDB transaction should reject vector SELECT expressions");

	assert_cockroach_pgvector_error(&error);

	let isolation_error = connection
		.atomic_with_isolation(IsolationLevel::Serializable, async |transaction| {
			vector_query().rows_with_db(transaction).await?;
			Ok::<_, Error>(())
		})
		.await
		.expect_err("isolated CockroachDB transaction should reject vector SELECT expressions");
	assert_cockroach_pgvector_error(&isolation_error);

	assert_eq!(
		recorder.statements(),
		Vec::<String>::new(),
		"transactional vector statements must fail before reaching the backend"
	);
}

#[tokio::test]
async fn cockroach_flavor_survives_atomic_legacy_transaction_executor_dispatch() {
	let recorder = Arc::new(Recorder::default());
	let lease = connection_with_flavor(true, recorder.clone());
	let connection = lease.handle();

	connection
		.atomic(async |transaction| {
			let error = vector_query()
				.all_with_executor(transaction)
				.await
				.expect_err("legacy transaction dispatch should reject CockroachDB vectors");
			assert_eq!(
				error.kind(),
				reinhardt_db::backends::error::DatabaseErrorKind::Unsupported
			);
			assert_eq!(
				error.message(),
				"pgvector distance operators is not supported by the CockroachDB backend"
			);
			Ok::<_, Error>(())
		})
		.await
		.expect("the handled query-build error should allow the transaction to commit");

	assert_eq!(
		recorder.statements(),
		Vec::<String>::new(),
		"legacy transaction vector statements must fail before reaching the backend"
	);
}

#[tokio::test]
async fn cockroach_flavor_survives_public_transaction_executor_construction() {
	let recorder = Arc::new(Recorder::default());
	let owner = owner_with_flavor(true, recorder.clone());
	let mut executor = owner
		.begin()
		.await
		.expect("recording CockroachDB transaction should begin");

	let error = vector_query()
		.all_with_executor(executor.as_mut())
		.await
		.expect_err("public CockroachDB transaction should reject vector SELECT expressions");
	assert_eq!(
		error.kind(),
		reinhardt_db::backends::error::DatabaseErrorKind::Unsupported
	);
	assert_eq!(
		error.message(),
		"pgvector distance operators is not supported by the CockroachDB backend"
	);
	assert_eq!(
		recorder.statements(),
		Vec::<String>::new(),
		"public transaction vector statements must fail before reaching the backend"
	);

	executor
		.rollback()
		.await
		.expect("recording transaction should roll back");
}

#[tokio::test]
async fn registry_slot_reuse_does_not_leak_cockroach_flavor() {
	let cockroach_recorder = Arc::new(Recorder::default());
	let cockroach_lease = connection_with_flavor(true, cockroach_recorder);
	let expired_handle = cockroach_lease.handle();
	drop(cockroach_lease);

	let expired_error = match vector_query()
		.rows_with_db(&mut expired_handle.clone())
		.await
	{
		Ok(_) => panic!("dropping the lease should expire its copied handle"),
		Err(error) => error,
	};
	assert_eq!(
		expired_error
			.database_error()
			.expect("expired handle should retain structured database context")
			.kind(),
		DatabaseErrorKind::ConnectionHandleExpired
	);

	let postgres_recorder = Arc::new(Recorder::default());
	let postgres_lease = connection_with_flavor(false, postgres_recorder.clone());
	let mut postgres_connection = postgres_lease.handle();
	let rows = vector_query()
		.rows_with_db(&mut postgres_connection)
		.await
		.expect("a reused registry slot must use its new PostgreSQL flavor");
	assert!(rows.is_empty());
	assert_eq!(postgres_recorder.statements().len(), 1);
}

#[tokio::test]
async fn cockroach_bulk_update_with_conn_rejects_vector_values_before_execution() {
	let recorder = Arc::new(Recorder::default());
	let lease = connection_with_flavor(true, recorder.clone());
	let mut connection = lease.handle();

	let error = CockroachDocument::objects()
		.bulk_update_with_conn(
			&mut connection,
			vec![document(Some(7))],
			vec!["embedding".to_owned()],
			None,
		)
		.await
		.expect_err("CockroachDB should reject vector bulk updates");
	assert_cockroach_pgvector_value_error(&error);
	assert_eq!(recorder.statements(), Vec::<String>::new());
}

#[tokio::test]
#[serial_test::serial(cockroach_orm_database)]
async fn cockroach_global_bulk_update_rejects_vector_values_before_execution() {
	let recorder = Arc::new(Recorder::default());
	let lease = connection_with_flavor(true, recorder.clone());
	let previous = replace_database_connection_for_testing(Some(lease)).await;

	let result = CockroachDocument::objects()
		.bulk_update(vec![document(Some(7))], vec!["embedding".to_owned()], None)
		.await;

	let installed = replace_database_connection_for_testing(previous).await;
	drop(installed);

	let error = result.expect_err("CockroachDB should reject global vector bulk updates");
	assert_cockroach_pgvector_value_error(&error);
	assert_eq!(recorder.statements(), Vec::<String>::new());
}

#[tokio::test]
async fn cockroach_bulk_update_keeps_non_vector_fields_working() {
	let recorder = Arc::new(Recorder::default());
	let lease = connection_with_flavor(true, recorder.clone());
	let mut connection = lease.handle();

	let updated = CockroachDocument::objects()
		.bulk_update_with_conn(
			&mut connection,
			vec![document(Some(7))],
			vec!["name".to_owned()],
			None,
		)
		.await
		.expect("ordinary CockroachDB bulk updates should remain supported");

	assert_eq!(updated, 1);
	assert_eq!(recorder.statements().len(), 1);
}

#[tokio::test]
async fn postgres_bulk_update_keeps_vector_values_working() {
	let recorder = Arc::new(Recorder::default());
	let lease = connection_with_flavor(false, recorder.clone());
	let mut connection = lease.handle();

	let updated = CockroachDocument::objects()
		.bulk_update_with_conn(
			&mut connection,
			vec![document(Some(7))],
			vec!["embedding".to_owned()],
			None,
		)
		.await
		.expect("PostgreSQL should retain vector bulk update support");

	assert_eq!(updated, 1);
	assert_eq!(recorder.statements().len(), 1);
}
