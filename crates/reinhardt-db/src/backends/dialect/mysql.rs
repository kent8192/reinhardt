//! MySQL dialect implementation

use async_trait::async_trait;
use sqlx::{Column, MySql, MySqlPool, Row as SqlxRow, Transaction, mysql::MySqlRow};
use std::sync::Arc;

use crate::backends::{
	backend::DatabaseBackend,
	error::Result,
	types::{
		DatabaseType, IsolationLevel, QueryResult, QueryValue, Row, Savepoint, TransactionExecutor,
	},
};

/// MySQL database backend
pub struct MySqlBackend {
	pool: Arc<MySqlPool>,
}

impl MySqlBackend {
	pub fn new(pool: MySqlPool) -> Self {
		Self {
			pool: Arc::new(pool),
		}
	}

	pub fn pool(&self) -> &MySqlPool {
		&self.pool
	}

	fn bind_value<'q>(
		query: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
		value: &'q QueryValue,
	) -> sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments> {
		match value {
			QueryValue::Null => query.bind(None::<i32>),
			QueryValue::Bool(b) => query.bind(b),
			QueryValue::Int(i) => query.bind(i),
			QueryValue::Float(f) => query.bind(f),
			QueryValue::String(s) => query.bind(s),
			QueryValue::Bytes(b) => query.bind(b),
			QueryValue::Timestamp(dt) => query.bind(dt),
			// MySQL stores UUIDs as BINARY(16) or CHAR(36); we bind as string
			QueryValue::Uuid(u) => query.bind(u.to_string()),
			QueryValue::Now => {
				// MySQL uses NOW() function, which should be part of SQL string
				// For binding, we use current UTC time
				query.bind(chrono::Utc::now())
			}
		}
	}

	fn convert_row(mysql_row: MySqlRow) -> Result<Row> {
		let mut row = Row::new();
		for column in mysql_row.columns() {
			let column_name = column.name();
			if let Ok(value) = mysql_row.try_get::<bool, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Bool(value));
			} else if let Ok(value) = mysql_row.try_get::<i64, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Int(value));
			} else if let Ok(value) = mysql_row.try_get::<i32, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Int(value as i64));
			} else if let Ok(value) = mysql_row.try_get::<f64, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Float(value));
			} else if let Ok(value) = mysql_row.try_get::<String, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::String(value));
			} else if let Ok(value) = mysql_row.try_get::<Vec<u8>, _>(column_name) {
				// MySQL 8.0 information_schema returns binary collation columns that
				// sqlx reports as LONGBLOB. Attempt UTF-8 conversion to recover string data.
				match String::from_utf8(value.clone()) {
					Ok(s) => row.insert(column_name.to_string(), QueryValue::String(s)),
					Err(_) => row.insert(column_name.to_string(), QueryValue::Bytes(value)),
				};
			} else if let Ok(value) = mysql_row.try_get::<chrono::NaiveDateTime, _>(column_name) {
				// MySQL TIMESTAMP/DATETIME without timezone
				row.insert(
					column_name.to_string(),
					QueryValue::Timestamp(chrono::DateTime::from_naive_utc_and_offset(
						value,
						chrono::Utc,
					)),
				);
			} else if let Ok(value) =
				mysql_row.try_get::<chrono::DateTime<chrono::Utc>, _>(column_name)
			{
				row.insert(column_name.to_string(), QueryValue::Timestamp(value));
			} else if mysql_row.try_get::<Option<i32>, _>(column_name).is_ok() {
				row.insert(column_name.to_string(), QueryValue::Null);
			}
		}
		Ok(row)
	}
}

#[async_trait]
impl DatabaseBackend for MySqlBackend {
	fn database_type(&self) -> DatabaseType {
		DatabaseType::Mysql
	}

	fn placeholder(&self, _index: usize) -> String {
		"?".to_string()
	}

	fn supports_returning(&self) -> bool {
		false
	}

	fn supports_on_conflict(&self) -> bool {
		false
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
		let mysql_row = query.fetch_one(self.pool.as_ref()).await?;
		Self::convert_row(mysql_row)
	}

	async fn fetch_all(&self, sql: &str, params: Vec<QueryValue>) -> Result<Vec<Row>> {
		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let mysql_rows = query.fetch_all(self.pool.as_ref()).await?;
		mysql_rows.into_iter().map(Self::convert_row).collect()
	}

	async fn fetch_optional(&self, sql: &str, params: Vec<QueryValue>) -> Result<Option<Row>> {
		let mut query = sqlx::query(sql);
		for param in &params {
			query = Self::bind_value(query, param);
		}
		let mysql_row = query.fetch_optional(self.pool.as_ref()).await?;
		mysql_row.map(Self::convert_row).transpose()
	}

	async fn begin(&self) -> Result<Box<dyn TransactionExecutor>> {
		let tx = self.pool.begin().await?;
		Ok(Box::new(MySqlTransactionExecutor::new(tx)))
	}

	async fn begin_with_isolation(
		&self,
		isolation_level: IsolationLevel,
	) -> Result<Box<dyn TransactionExecutor>> {
		// MySQL requires SET TRANSACTION before starting the transaction
		// First set the isolation level for the next transaction
		let set_sql = format!(
			"SET TRANSACTION ISOLATION LEVEL {}",
			isolation_level.to_sql(DatabaseType::Mysql)
		);
		sqlx::query(&set_sql).execute(self.pool.as_ref()).await?;

		// Then start the transaction
		let tx = self.pool.begin().await?;
		Ok(Box::new(MySqlTransactionExecutor::new(tx)))
	}

	fn as_any(&self) -> &dyn std::any::Any {
		self
	}
}

/// MySQL transaction executor
pub struct MySqlTransactionExecutor {
	tx: Option<Transaction<'static, MySql>>,
}

impl MySqlTransactionExecutor {
	pub fn new(tx: Transaction<'static, MySql>) -> Self {
		Self { tx: Some(tx) }
	}

	fn bind_value<'q>(
		query: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
		value: &'q QueryValue,
	) -> sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments> {
		match value {
			QueryValue::Null => query.bind(None::<i32>),
			QueryValue::Bool(b) => query.bind(b),
			QueryValue::Int(i) => query.bind(i),
			QueryValue::Float(f) => query.bind(f),
			QueryValue::String(s) => query.bind(s),
			QueryValue::Bytes(b) => query.bind(b),
			QueryValue::Timestamp(dt) => query.bind(dt),
			// MySQL stores UUIDs as BINARY(16) or CHAR(36); we bind as string
			QueryValue::Uuid(u) => query.bind(u.to_string()),
			QueryValue::Now => query.bind(chrono::Utc::now()),
		}
	}

	fn convert_row(mysql_row: MySqlRow) -> Result<Row> {
		let mut row = Row::new();
		for column in mysql_row.columns() {
			let column_name = column.name();
			if let Ok(value) = mysql_row.try_get::<bool, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Bool(value));
			} else if let Ok(value) = mysql_row.try_get::<i64, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Int(value));
			} else if let Ok(value) = mysql_row.try_get::<i32, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Int(value as i64));
			} else if let Ok(value) = mysql_row.try_get::<f64, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::Float(value));
			} else if let Ok(value) = mysql_row.try_get::<String, _>(column_name) {
				row.insert(column_name.to_string(), QueryValue::String(value));
			} else if let Ok(value) = mysql_row.try_get::<Vec<u8>, _>(column_name) {
				// MySQL 8.0 information_schema returns binary collation columns that
				// sqlx reports as LONGBLOB. Attempt UTF-8 conversion to recover string data.
				match String::from_utf8(value.clone()) {
					Ok(s) => row.insert(column_name.to_string(), QueryValue::String(s)),
					Err(_) => row.insert(column_name.to_string(), QueryValue::Bytes(value)),
				};
			} else if let Ok(value) = mysql_row.try_get::<chrono::NaiveDateTime, _>(column_name) {
				row.insert(
					column_name.to_string(),
					QueryValue::Timestamp(chrono::DateTime::from_naive_utc_and_offset(
						value,
						chrono::Utc,
					)),
				);
			} else if let Ok(value) =
				mysql_row.try_get::<chrono::DateTime<chrono::Utc>, _>(column_name)
			{
				row.insert(column_name.to_string(), QueryValue::Timestamp(value));
			} else if mysql_row.try_get::<Option<i32>, _>(column_name).is_ok() {
				row.insert(column_name.to_string(), QueryValue::Null);
			}
		}
		Ok(row)
	}
}

#[async_trait]
impl TransactionExecutor for MySqlTransactionExecutor {
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
