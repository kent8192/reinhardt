//! PostgreSQL dialect implementation

use async_trait::async_trait;
use sqlx::{Column, PgPool, Postgres, Transaction, postgres::PgRow};
use std::sync::Arc;
use uuid::Uuid;

use crate::backends::{
	backend::DatabaseBackend,
	error::Result,
	types::{
		DatabaseType, IsolationLevel, QueryResult, QueryValue, Row, Savepoint, TransactionExecutor,
	},
};

/// PostgreSQL database backend
pub struct PostgresBackend {
	pool: Arc<PgPool>,
}

impl PostgresBackend {
	pub fn new(pool: PgPool) -> Self {
		Self {
			pool: Arc::new(pool),
		}
	}

	pub fn pool(&self) -> &PgPool {
		&self.pool
	}

	fn bind_value<'q>(
		query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
		value: &'q QueryValue,
	) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
		match value {
			QueryValue::Null => query.bind(None::<i32>),
			QueryValue::Bool(b) => query.bind(b),
			QueryValue::Int(i) => query.bind(i),
			QueryValue::Float(f) => query.bind(f),
			QueryValue::String(s) => query.bind(s),
			QueryValue::Bytes(b) => query.bind(b),
			QueryValue::Timestamp(dt) => query.bind(dt),
			QueryValue::Uuid(u) => query.bind(u),
			QueryValue::Now => {
				// PostgreSQL uses NOW() function, which should be part of SQL string
				// For binding, we use current UTC time
				query.bind(chrono::Utc::now())
			}
		}
	}

	fn convert_row(pg_row: PgRow) -> Result<Row> {
		Self::convert_row_internal(pg_row)
	}
}

#[async_trait]
impl DatabaseBackend for PostgresBackend {
	fn database_type(&self) -> DatabaseType {
		DatabaseType::Postgres
	}

	fn placeholder(&self, index: usize) -> String {
		format!("${}", index)
	}

	fn supports_returning(&self) -> bool {
		true
	}

	fn supports_on_conflict(&self) -> bool {
		true
	}

	async fn execute(&self, sql: &str, params: Vec<QueryValue>) -> Result<QueryResult> {
		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let result = query.execute(self.pool.as_ref()).await?;
		Ok(QueryResult {
			rows_affected: result.rows_affected(),
		})
	}

	async fn fetch_one(&self, sql: &str, params: Vec<QueryValue>) -> Result<Row> {
		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let row = query.fetch_one(self.pool.as_ref()).await?;
		Self::convert_row(row)
	}

	async fn fetch_all(&self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>> {
		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let rows = query.fetch_all(self.pool.as_ref()).await?;
		rows.into_iter().map(Self::convert_row).collect()
	}

	async fn fetch_optional(&self, sql: &str, params: Vec<QueryValue>) -> Result<Option<Row>> {
		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let row = query.fetch_optional(self.pool.as_ref()).await?;
		row.map(Self::convert_row).transpose()
	}

	async fn begin(&self) -> Result<Box<dyn TransactionExecutor>> {
		let tx = self.pool.begin().await?;
		Ok(Box::new(PgTransactionExecutor::new(tx)))
	}

	async fn begin_with_isolation(
		&self,
		isolation_level: IsolationLevel,
	) -> Result<Box<dyn TransactionExecutor>> {
		// PostgreSQL supports setting isolation level at transaction start
		let mut tx = self.pool.begin().await?;

		// Set the isolation level using PostgreSQL's SET TRANSACTION command
		let sql = format!(
			"SET TRANSACTION ISOLATION LEVEL {}",
			isolation_level.to_sql(DatabaseType::Postgres)
		);
		sqlx::query(&sql).execute(&mut *tx).await?;

		Ok(Box::new(PgTransactionExecutor::new(tx)))
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}

/// PostgreSQL transaction executor
///
/// This struct wraps a SQLx `Transaction` to ensure all queries
/// within a transaction run on the same physical database connection.
pub struct PgTransactionExecutor {
	tx: Option<Transaction<'static, Postgres>>,
}

impl PgTransactionExecutor {
	pub fn new(tx: Transaction<'static, Postgres>) -> Self {
		Self { tx: Some(tx) }
	}

	fn bind_value<'q>(
		query: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
		value: &'q QueryValue,
	) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
		match value {
			QueryValue::Null => query.bind(None::<i32>),
			QueryValue::Bool(b) => query.bind(b),
			QueryValue::Int(i) => query.bind(i),
			QueryValue::Float(f) => query.bind(f),
			QueryValue::String(s) => query.bind(s),
			QueryValue::Bytes(b) => query.bind(b),
			QueryValue::Timestamp(dt) => query.bind(dt),
			QueryValue::Uuid(u) => query.bind(u),
			QueryValue::Now => query.bind(chrono::Utc::now()),
		}
	}

	fn convert_row(pg_row: PgRow) -> Result<Row> {
		PostgresBackend::convert_row_internal(pg_row)
	}
}

impl PostgresBackend {
	/// Internal row conversion method shared between backend and transaction executor
	pub(crate) fn convert_row_internal(pg_row: PgRow) -> Result<Row> {
		use rust_decimal::prelude::ToPrimitive;
		use sqlx::Row as SqlxRow;

		let mut row = Row::new();
		for column in pg_row.columns() {
			let column_name = column.name();

			if let Ok(value) = pg_row.try_get::<Uuid, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Uuid(value));
			} else if let Ok(value) = pg_row.try_get::<bool, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Bool(value));
			} else if let Ok(value) = pg_row.try_get::<i64, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Int(value));
			} else if let Ok(value) = pg_row.try_get::<i32, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Int(value as i64));
			} else if let Ok(value) = pg_row.try_get::<rust_decimal::Decimal, _>(column_name) {
				// Convert DECIMAL/NUMERIC to f64 for Float storage
				if let Some(f) = value.to_f64() {
					row.insert(column_name.to_string(), QueryValue::Float(f));
				}
			} else if let Ok(value) = pg_row.try_get::<f64, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Float(value));
			} else if let Ok(value) = pg_row.try_get::<String, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::String(value));
			} else if let Ok(value) = pg_row.try_get::<Vec<u8>, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Bytes(value));
			} else if let Ok(value) = pg_row.try_get::<chrono::NaiveDateTime, _>(column_name) {
				row.insert(
					column_name.to_string(),
					QueryValue::Timestamp(chrono::DateTime::from_naive_utc_and_offset(
						value,
						chrono::Utc,
					)),
				);
			} else if let Ok(value) =
				pg_row.try_get::<chrono::DateTime<chrono::Utc>, _>(column_name)
			{
				row.insert(column_name.to_string(), QueryValue::Timestamp(value));
			} else if pg_row.try_get::<Option<i32>, _>(column_name).is_ok() {
				row.insert(column_name.to_string(), QueryValue::Null);
			}
		}
		Ok(row)
	}
}

#[async_trait]
impl TransactionExecutor for PgTransactionExecutor {
	async fn execute(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<QueryResult> {
		let tx = self.tx.as_mut().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;

		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let result = query.execute(&mut **tx).await?;
		Ok(QueryResult {
			rows_affected: result.rows_affected(),
		})
	}

	async fn fetch_one(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Row> {
		let tx = self.tx.as_mut().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;

		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let row = query.fetch_one(&mut **tx).await?;
		Self::convert_row(row)
	}

	async fn fetch_all(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>> {
		let tx = self.tx.as_mut().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;

		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let rows = query.fetch_all(&mut **tx).await?;
		rows.into_iter().map(Self::convert_row).collect()
	}

	async fn fetch_optional(&mut self, sql: &str, params: Vec<QueryValue>) -> Result<Option<Row>> {
		let tx = self.tx.as_mut().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;

		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let row = query.fetch_optional(&mut **tx).await?;
		row.map(Self::convert_row).transpose()
	}

	async fn commit(mut self: Box<Self>) -> Result<()> {
		let tx = self.tx.take().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;
		tx.commit().await?;
		Ok(())
	}

	async fn rollback(mut self: Box<Self>) -> Result<()> {
		let tx = self.tx.take().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;
		tx.rollback().await?;
		Ok(())
	}

	async fn savepoint(&mut self, name: &str) -> Result<()> {
		let tx = self.tx.as_mut().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;

		let sp = Savepoint::new(name);
		sqlx::query(&sp.to_sql()).execute(&mut **tx).await?;
		Ok(())
	}

	async fn release_savepoint(&mut self, name: &str) -> Result<()> {
		let tx = self.tx.as_mut().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;

		let sp = Savepoint::new(name);
		sqlx::query(&sp.release_sql()).execute(&mut **tx).await?;
		Ok(())
	}

	async fn rollback_to_savepoint(&mut self, name: &str) -> Result<()> {
		let tx = self.tx.as_mut().ok_or_else(|| {
			crate::backends::error::DatabaseError::TransactionError(
				"Transaction already consumed".to_string(),
			)
		})?;

		let sp = Savepoint::new(name);
		sqlx::query(&sp.rollback_sql()).execute(&mut **tx).await?;
		Ok(())
	}
}
