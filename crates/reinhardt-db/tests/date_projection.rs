use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Result};
#[cfg(feature = "sqlite")]
use reinhardt_db::backends::DatabaseConnection as BackendsConnection;
use reinhardt_db::backends::types::{
	DatabaseType, QueryResult, QueryValue, Row, TransactionExecutor,
};
#[cfg(feature = "sqlite")]
use reinhardt_db::orm::DatabaseConnectionLease;
use reinhardt_db::orm::{
	DateProjectionOrder, DateTimeTruncKind, DateTruncKind, FieldRef, FieldSelector, Model,
	OrmExecutor,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ProjectionEvent {
	id: Option<i64>,
	event_date: Option<NaiveDate>,
	occurred_at: DateTime<Utc>,
}

#[derive(Clone)]
struct ProjectionEventFields;

impl FieldSelector for ProjectionEventFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

impl Model for ProjectionEvent {
	type PrimaryKey = i64;
	type Fields = ProjectionEventFields;
	type Objects = reinhardt_db::orm::Manager<Self>;

	fn table_name() -> &'static str {
		"projection_events"
	}

	fn new_fields() -> Self::Fields {
		ProjectionEventFields
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}
}

impl ProjectionEvent {
	const fn field_event_date() -> FieldRef<Self, Option<NaiveDate>> {
		FieldRef::new("event_date")
	}

	const fn field_occurred_at() -> FieldRef<Self, DateTime<Utc>> {
		FieldRef::new("occurred_at")
	}
}

struct RecordingExecutor {
	backend: reinhardt_db::orm::DatabaseBackend,
	rows: Vec<Row>,
	sql: Option<String>,
	params: Vec<QueryValue>,
}

impl RecordingExecutor {
	fn new(backend: reinhardt_db::orm::DatabaseBackend, rows: Vec<Row>) -> Self {
		Self {
			backend,
			rows,
			sql: None,
			params: Vec::new(),
		}
	}

	fn unused() -> Result<QueryResult> {
		Err(DatabaseError::new(DatabaseErrorKind::Query, "unexpected executor operation").into())
	}

	fn unused_row() -> Result<Row> {
		Err(DatabaseError::new(DatabaseErrorKind::Query, "unexpected executor operation").into())
	}
}

#[async_trait]
impl OrmExecutor for RecordingExecutor {
	fn backend(&self) -> reinhardt_db::orm::DatabaseBackend {
		self.backend
	}

	async fn execute(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<QueryResult> {
		Self::unused()
	}

	async fn fetch_one(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<Row> {
		Self::unused_row()
	}

	async fn fetch_all(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>> {
		self.sql = Some(sql.to_string());
		self.params = params;
		Ok(self.rows.clone())
	}

	async fn fetch_optional(
		&mut self,
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> Result<Option<Row>> {
		Ok(None)
	}
}

#[async_trait]
impl TransactionExecutor for RecordingExecutor {
	fn backend(&self) -> DatabaseType {
		match self.backend {
			reinhardt_db::orm::DatabaseBackend::Postgres => DatabaseType::Postgres,
			reinhardt_db::orm::DatabaseBackend::MySql => DatabaseType::Mysql,
			reinhardt_db::orm::DatabaseBackend::Sqlite => DatabaseType::Sqlite,
		}
	}

	async fn execute(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<QueryResult> {
		Self::unused()
	}

	async fn fetch_one(&mut self, _sql: &str, _params: Vec<QueryValue>) -> Result<Row> {
		Self::unused_row()
	}

	async fn fetch_all(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>> {
		self.sql = Some(sql.to_string());
		self.params = params;
		Ok(self.rows.clone())
	}

	async fn fetch_optional(
		&mut self,
		_sql: &str,
		_params: Vec<QueryValue>,
	) -> Result<Option<Row>> {
		Ok(None)
	}

	async fn commit(self: Box<Self>) -> Result<()> {
		Ok(())
	}

	async fn rollback(self: Box<Self>) -> Result<()> {
		Ok(())
	}
}

fn value_row(value: QueryValue) -> Row {
	let mut row = Row::new();
	row.insert("value".to_string(), value);
	row
}

#[tokio::test]
async fn dates_with_db_decodes_iso_week_boundaries_and_orders_in_sql() {
	let rows = vec![
		value_row(QueryValue::String("2024-12-30".to_string())),
		value_row(QueryValue::String("2025-01-06".to_string())),
	];
	let mut executor = RecordingExecutor::new(reinhardt_db::orm::DatabaseBackend::Sqlite, rows);

	let values = ProjectionEvent::objects()
		.dates_with_db(
			&mut executor,
			ProjectionEvent::field_event_date(),
			DateTruncKind::Week,
			DateProjectionOrder::Asc,
		)
		.await
		.unwrap();

	assert_eq!(
		values,
		vec![
			NaiveDate::from_ymd_opt(2024, 12, 30).unwrap(),
			NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(),
		]
	);
	assert_eq!(
		executor.sql.unwrap(),
		concat!(
			"SELECT DISTINCT DATE(\"event_date\", '-' || ((CAST(strftime('%w', ",
			"\"event_date\") AS INTEGER) + 6) % 7) || ' days') AS \"value\" ",
			"FROM \"projection_events\" WHERE \"event_date\" IS NOT NULL ORDER BY ",
			"\"value\" ASC"
		)
	);
	assert_eq!(executor.params, Vec::<QueryValue>::new());
}

#[tokio::test]
async fn datetimes_with_db_returns_named_zone_values_across_dst_gap_and_fold() {
	let rows = vec![
		value_row(QueryValue::Timestamp(
			Utc.with_ymd_and_hms(2024, 3, 10, 7, 0, 0).unwrap(),
		)),
		value_row(QueryValue::Timestamp(
			Utc.with_ymd_and_hms(2024, 11, 3, 6, 0, 0).unwrap(),
		)),
	];
	let mut executor = RecordingExecutor::new(reinhardt_db::orm::DatabaseBackend::Postgres, rows);

	let values = ProjectionEvent::objects()
		.datetimes_with_db(
			&mut executor,
			ProjectionEvent::field_occurred_at(),
			DateTimeTruncKind::Hour,
			DateProjectionOrder::Asc,
			Some(chrono_tz::America::New_York),
		)
		.await
		.unwrap();

	assert_eq!(values[0].to_rfc3339(), "2024-03-10T03:00:00-04:00");
	assert_eq!(values[1].to_rfc3339(), "2024-11-03T01:00:00-05:00");
	assert_eq!(
		executor.params,
		vec![
			QueryValue::String("America/New_York".to_string()),
			QueryValue::String("America/New_York".to_string()),
		]
	);
}

#[tokio::test]
async fn sqlite_named_time_zone_returns_capability_error_without_querying() {
	let mut executor =
		RecordingExecutor::new(reinhardt_db::orm::DatabaseBackend::Sqlite, Vec::new());

	let error = ProjectionEvent::objects()
		.datetimes_with_db(
			&mut executor,
			ProjectionEvent::field_occurred_at(),
			DateTimeTruncKind::Day,
			DateProjectionOrder::Asc,
			Some(chrono_tz::Asia::Tokyo),
		)
		.await
		.unwrap_err();

	let database_error = match error {
		reinhardt_core::exception::Error::Database(error) => error,
		reinhardt_core::exception::Error::DatabaseWithSource { database_error, .. } => {
			database_error
		}
		other => panic!("expected database error, got {other:?}"),
	};
	assert_eq!(database_error.kind(), DatabaseErrorKind::Unsupported);
	assert_eq!(
		database_error.message(),
		"named time-zone conversion is not supported by the SQLite backend"
	);
	assert_eq!(executor.sql, None);
}

#[tokio::test]
async fn dates_with_executor_uses_the_caller_owned_transaction() {
	let rows = vec![value_row(QueryValue::String("2026-01-01".to_string()))];
	let mut executor = RecordingExecutor::new(reinhardt_db::orm::DatabaseBackend::Postgres, rows);

	let values = ProjectionEvent::objects()
		.dates_with_executor(
			&mut executor,
			ProjectionEvent::field_event_date(),
			DateTruncKind::Year,
			DateProjectionOrder::Desc,
		)
		.await
		.unwrap();

	assert_eq!(values, vec![NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()]);
	assert_eq!(
		executor.sql.unwrap(),
		concat!(
			"SELECT DISTINCT DATE_TRUNC('year', \"event_date\")::date AS \"value\" ",
			"FROM \"projection_events\" WHERE \"event_date\" IS NOT NULL ORDER BY ",
			"\"value\" DESC"
		)
	);
}

#[tokio::test]
#[cfg(feature = "sqlite")]
async fn sqlite_executes_distinct_null_excluding_date_and_datetime_projections() {
	let owner = BackendsConnection::connect_sqlite("sqlite::memory:")
		.await
		.unwrap();
	let lease = DatabaseConnectionLease::register(owner).unwrap();
	let mut connection = lease.handle();
	OrmExecutor::execute(
		&mut connection,
		concat!(
			"CREATE TABLE projection_events (",
			"id INTEGER PRIMARY KEY, event_date DATE, occurred_at DATETIME NOT NULL)"
		),
		Vec::new(),
	)
	.await
	.unwrap();
	OrmExecutor::execute(
		&mut connection,
		concat!(
			"INSERT INTO projection_events (id, event_date, occurred_at) VALUES ",
			"(1, '2024-12-31', '2024-03-10 07:30:45'), ",
			"(2, '2025-01-01', '2024-03-10 07:45:00'), ",
			"(3, NULL, '2024-03-10 08:01:00')"
		),
		Vec::new(),
	)
	.await
	.unwrap();

	let weeks = ProjectionEvent::objects()
		.dates_with_db(
			&mut connection,
			ProjectionEvent::field_event_date(),
			DateTruncKind::Week,
			DateProjectionOrder::Asc,
		)
		.await
		.unwrap();
	let hours = ProjectionEvent::objects()
		.datetimes_with_db(
			&mut connection,
			ProjectionEvent::field_occurred_at(),
			DateTimeTruncKind::Hour,
			DateProjectionOrder::Asc,
			None,
		)
		.await
		.unwrap();

	assert_eq!(weeks, vec![NaiveDate::from_ymd_opt(2024, 12, 30).unwrap()]);
	assert_eq!(
		hours,
		vec![
			Utc.with_ymd_and_hms(2024, 3, 10, 7, 0, 0)
				.unwrap()
				.with_timezone(&chrono_tz::Tz::UTC),
			Utc.with_ymd_and_hms(2024, 3, 10, 8, 0, 0)
				.unwrap()
				.with_timezone(&chrono_tz::Tz::UTC),
		]
	);
}
