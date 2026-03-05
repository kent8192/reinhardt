//! # Query Execution
//!
//! SQLAlchemy-inspired query execution methods.
//!
//! This module provides execution methods similar to SQLAlchemy's Query class

use crate::backends::types::QueryValue;
use crate::orm::Model;
use reinhardt_query::prelude::{
	Alias, ColumnRef, Expr, ExprTrait, Func, Query, QueryStatementBuilder, SelectStatement,
};
use rust_decimal::prelude::ToPrimitive;
use std::marker::PhantomData;

/// Query execution result types
#[derive(Debug)]
pub enum ExecutionResult<T> {
	/// Single result
	One(T),
	/// Optional single result
	OneOrNone(Option<T>),
	/// Multiple results
	All(Vec<T>),
	/// Scalar value (for aggregates)
	Scalar(String),
	/// No result (for mutations)
	None,
}

/// Errors that can occur during query execution
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
	/// Database error
	#[error("Database error: {0}")]
	Database(#[from] crate::backends::DatabaseError),

	/// No result found (for .one())
	#[error("No result found")]
	NoResultFound,

	/// Multiple results found (for .one() and .one_or_none())
	#[error("Multiple results found (expected 1, got {0})")]
	MultipleResultsFound(usize),

	/// Deserialization error
	#[error("Failed to deserialize result: {0}")]
	Deserialization(#[from] serde_json::Error),

	/// Query building error
	#[error("Query building error: {0}")]
	QueryBuild(String),

	/// Generic error from anyhow
	#[error("Generic error: {0}")]
	Generic(#[from] anyhow::Error),
}

/// Convert reinhardt_query Value to QueryValue for parameter binding
fn convert_value_to_query_value(value: reinhardt_query::value::Value) -> QueryValue {
	use reinhardt_query::value::Value as SV;

	match value {
		// Null values
		SV::Bool(None)
		| SV::TinyInt(None)
		| SV::SmallInt(None)
		| SV::Int(None)
		| SV::BigInt(None)
		| SV::TinyUnsigned(None)
		| SV::SmallUnsigned(None)
		| SV::Unsigned(None)
		| SV::BigUnsigned(None)
		| SV::Float(None)
		| SV::Double(None)
		| SV::String(None)
		| SV::Char(None)
		| SV::Bytes(None)
		| SV::ChronoDateTimeUtc(None)
		| SV::ChronoDateTimeLocal(None)
		| SV::ChronoDateTimeWithTimeZone(None)
		| SV::ChronoDate(None)
		| SV::ChronoTime(None)
		| SV::ChronoDateTime(None)
		| SV::Json(None)
		| SV::Decimal(None)
		| SV::BigDecimal(None)
		| SV::Uuid(None) => QueryValue::Null,

		// Boolean
		SV::Bool(Some(b)) => QueryValue::Bool(b),

		// Signed integers (convert all to i64)
		SV::TinyInt(Some(v)) => QueryValue::Int(v as i64),
		SV::SmallInt(Some(v)) => QueryValue::Int(v as i64),
		SV::Int(Some(v)) => QueryValue::Int(v as i64),
		SV::BigInt(Some(v)) => QueryValue::Int(v),

		// Unsigned integers (convert to i64 with checked conversion for large values)
		SV::TinyUnsigned(Some(v)) => QueryValue::Int(v as i64),
		SV::SmallUnsigned(Some(v)) => QueryValue::Int(v as i64),
		SV::Unsigned(Some(v)) => QueryValue::Int(v as i64),
		SV::BigUnsigned(Some(v)) => QueryValue::Int(i64::try_from(v).unwrap_or_else(|_| {
			tracing::warn!(
				value = v,
				"BigUnsigned value {} exceeds i64::MAX, clamping to i64::MAX",
				v
			);
			i64::MAX
		})),

		// Floating point
		SV::Float(Some(v)) => QueryValue::Float(v as f64),
		SV::Double(Some(v)) => QueryValue::Float(v),

		// String and char
		SV::String(Some(s)) => QueryValue::String(s.to_string()),
		SV::Char(Some(c)) => QueryValue::String(c.to_string()),

		// Bytes
		SV::Bytes(Some(b)) => QueryValue::Bytes(b.to_vec()),

		// Chrono datetime types
		SV::ChronoDateTimeUtc(Some(dt)) => QueryValue::Timestamp(*dt),

		// For other datetime types, convert to UTC if possible
		SV::ChronoDateTimeLocal(Some(dt)) => {
			QueryValue::Timestamp((*dt).with_timezone(&chrono::Utc))
		}
		SV::ChronoDateTimeWithTimeZone(Some(dt)) => {
			QueryValue::Timestamp((*dt).with_timezone(&chrono::Utc))
		}

		// Other datetime types that cannot be easily converted
		SV::ChronoDate(_) | SV::ChronoTime(_) | SV::ChronoDateTime(_) => {
			// Convert to string representation as fallback
			QueryValue::String(format!("{:?}", value))
		}

		// JSON - convert to string
		SV::Json(_) => QueryValue::String(format!("{:?}", value)),

		// Decimal - convert to f64 with fallback through string parsing
		SV::Decimal(Some(d)) => {
			let f = d.to_f64().unwrap_or_else(|| {
				tracing::warn!(
					decimal = %d,
					"Decimal cannot be directly represented as f64, falling back to string parsing"
				);
				d.to_string().parse::<f64>().unwrap_or(0.0)
			});
			QueryValue::Float(f)
		}
		SV::BigDecimal(Some(d)) => {
			let f = d.to_string().parse::<f64>().unwrap_or_else(|_| {
				tracing::warn!(
					big_decimal = %d,
					"BigDecimal cannot be represented as f64"
				);
				0.0
			});
			QueryValue::Float(f)
		}

		// UUID
		SV::Uuid(Some(u)) => QueryValue::Uuid(*u),

		// Arrays - convert to string
		// For reinhardt-query 1.0.0-rc.29+: Array(ArrayType, Option<Box<Vec<Value>>>)
		SV::Array(_, arr) => QueryValue::String(format!("{:?}", arr)),
	}
}

/// Convert reinhardt_query Values (`Vec<Value>`) to `Vec<QueryValue>`
pub fn convert_values(values: reinhardt_query::prelude::Values) -> Vec<QueryValue> {
	values
		.0
		.into_iter()
		.map(convert_value_to_query_value)
		.collect()
}

/// Query execution methods with both sync builders and async execution
#[async_trait::async_trait]
pub trait QueryExecution<T: Model>
where
	T: Send + Sync,
	T::PrimaryKey: Send + Sync,
{
	/// Get a single result by primary key (async execution)
	/// Corresponds to SQLAlchemy's .get()
	async fn get_async(
		&self,
		db: &super::connection::DatabaseConnection,
		pk: &T::PrimaryKey,
	) -> Result<T, ExecutionError>
	where
		T: for<'de> serde::Deserialize<'de>;

	/// Get a single result by primary key (statement builder)
	/// Returns a SelectStatement for manual execution
	fn get(&self, pk: &T::PrimaryKey) -> SelectStatement;

	/// Get all results (async execution)
	/// Corresponds to SQLAlchemy's .all()
	async fn all_async(
		&self,
		db: &super::connection::DatabaseConnection,
	) -> Result<Vec<T>, ExecutionError>
	where
		T: for<'de> serde::Deserialize<'de>;

	/// Get all results (statement builder)
	/// Returns a SelectStatement for manual execution
	fn all(&self) -> SelectStatement;

	/// Get first result or None (async execution)
	/// Corresponds to SQLAlchemy's .first()
	async fn first_async(
		&self,
		db: &super::connection::DatabaseConnection,
	) -> Result<Option<T>, ExecutionError>
	where
		T: for<'de> serde::Deserialize<'de>;

	/// Get first result or None (statement builder)
	/// Returns a SelectStatement for manual execution
	fn first(&self) -> SelectStatement;

	/// Get exactly one result, raise if 0 or >1 (async execution)
	/// Corresponds to SQLAlchemy's .one()
	async fn one_async(
		&self,
		db: &super::connection::DatabaseConnection,
	) -> Result<T, ExecutionError>
	where
		T: for<'de> serde::Deserialize<'de>;

	/// Get exactly one result (statement builder)
	/// Returns a SelectStatement for manual execution
	fn one(&self) -> SelectStatement;

	/// Get one result or None, raise if >1 (async execution)
	/// Corresponds to SQLAlchemy's .one_or_none()
	async fn one_or_none_async(
		&self,
		db: &super::connection::DatabaseConnection,
	) -> Result<Option<T>, ExecutionError>
	where
		T: for<'de> serde::Deserialize<'de>;

	/// Get one result or None (statement builder)
	/// Returns a SelectStatement for manual execution
	fn one_or_none(&self) -> SelectStatement;

	/// Get scalar value (first column of first row) (async execution)
	/// Corresponds to SQLAlchemy's .scalar()
	async fn scalar_async<S>(
		&self,
		db: &super::connection::DatabaseConnection,
	) -> Result<Option<S>, ExecutionError>
	where
		S: for<'de> serde::Deserialize<'de>;

	/// Get scalar value (statement builder)
	/// Returns a SelectStatement for manual execution
	fn scalar(&self) -> SelectStatement;

	/// Count results (async execution)
	/// Corresponds to SQLAlchemy's .count()
	async fn count_async(
		&self,
		db: &super::connection::DatabaseConnection,
	) -> Result<i64, ExecutionError>;

	/// Count results (statement builder)
	/// Returns a SelectStatement for manual execution
	fn count(&self) -> SelectStatement;

	/// Check if any results exist (async execution)
	/// Corresponds to SQLAlchemy's .exists()
	async fn exists_async(
		&self,
		db: &super::connection::DatabaseConnection,
	) -> Result<bool, ExecutionError>;

	/// Check if any results exist (statement builder)
	/// Returns a SelectStatement for manual execution
	fn exists(&self) -> SelectStatement;
}

/// Execution context for SELECT queries
pub struct SelectExecution<T: Model> {
	stmt: SelectStatement,
	_phantom: PhantomData<T>,
}

impl<T: Model> SelectExecution<T> {
	/// Create a new query execution context with the given SelectStatement
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_db::orm::execution::SelectExecution;
	/// use reinhardt_db::orm::Model;
	/// use reinhardt_query::prelude::{QueryStatementBuilder, Alias, Query};
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct User {
	///     id: Option<i64>,
	///     name: String,
	/// }
	///
	/// #[derive(Clone)]
	/// struct UserFields;
	/// impl reinhardt_db::orm::FieldSelector for UserFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
	/// }
	///
	/// impl Model for User {
	///     type PrimaryKey = i64;
	///     type Fields = UserFields;
	///     fn app_label() -> &'static str { "app" }
	///     fn table_name() -> &'static str { "users" }
	///     fn new_fields() -> Self::Fields { UserFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	///     fn primary_key_field() -> &'static str { "id" }
	/// }
	///
	/// let stmt = Query::select().from(Alias::new("users")).to_owned();
	/// let exec = SelectExecution::<User>::new(stmt);
	/// ```
	pub fn new(stmt: SelectStatement) -> Self {
		Self {
			stmt,
			_phantom: PhantomData,
		}
	}
	/// Get a reference to the underlying SelectStatement
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_db::orm::execution::SelectExecution;
	/// use reinhardt_db::orm::Model;
	/// use reinhardt_query::prelude::{QueryStatementBuilder, Alias, Expr, Query};
	/// use serde::{Serialize, Deserialize};
	///
	/// #[derive(Debug, Clone, Serialize, Deserialize)]
	/// struct User {
	///     id: Option<i64>,
	///     name: String,
	/// }
	///
	/// #[derive(Clone)]
	/// struct UserFields;
	/// impl reinhardt_db::orm::FieldSelector for UserFields {
	///     fn with_alias(self, _alias: &str) -> Self { self }
	/// }
	///
	/// impl Model for User {
	///     type PrimaryKey = i64;
	///     type Fields = UserFields;
	///     fn app_label() -> &'static str { "app" }
	///     fn table_name() -> &'static str { "users" }
	///     fn new_fields() -> Self::Fields { UserFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	///     fn primary_key_field() -> &'static str { "id" }
	/// }
	///
	/// let stmt = Query::select()
	///     .from(Alias::new("users"))
	///     .and_where(Expr::col(Alias::new("active")).eq(true))
	///     .to_owned();
	/// let exec = SelectExecution::<User>::new(stmt);
	/// ```
	pub fn statement(&self) -> &SelectStatement {
		&self.stmt
	}
}

#[async_trait::async_trait]
impl<T: Model> QueryExecution<T> for SelectExecution<T>
where
	T::PrimaryKey: Into<reinhardt_query::value::Value> + Clone + Send + Sync,
	T: Send + Sync,
{
	fn get(&self, pk: &T::PrimaryKey) -> SelectStatement {
		Query::select()
			.from(Alias::new(T::table_name()))
			.column(ColumnRef::Asterisk)
			.and_where(
				Expr::col(Alias::new(T::primary_key_field())).eq(Expr::val(pk.clone().into())),
			)
			.limit(1)
			.to_owned()
	}

	fn all(&self) -> SelectStatement {
		self.stmt.clone()
	}

	fn first(&self) -> SelectStatement {
		let mut stmt = self.stmt.clone();
		stmt.limit(1);
		stmt
	}

	fn one(&self) -> SelectStatement {
		// Sets LIMIT 2 to detect multiple results
		// The execution layer should:
		// - Error if 0 results are returned (NoResultFound)
		// - Error if 2+ results are returned (MultipleResultsFound)
		// - Return the single result if exactly 1 is found
		let mut stmt = self.stmt.clone();
		stmt.limit(2);
		stmt
	}

	fn one_or_none(&self) -> SelectStatement {
		// Sets LIMIT 2 to detect multiple results
		// The execution layer should:
		// - Return None if 0 results
		// - Error if 2+ results are returned (MultipleResultsFound)
		// - Return Some(result) if exactly 1 is found
		let mut stmt = self.stmt.clone();
		stmt.limit(2);
		stmt
	}

	fn scalar(&self) -> SelectStatement {
		let mut stmt = self.stmt.clone();
		stmt.limit(1);
		stmt
	}

	fn count(&self) -> SelectStatement {
		// Use the original statement as a subquery and count all rows from it
		// This preserves all WHERE, JOIN, and other conditions
		Query::select()
			.expr(Func::count(Expr::asterisk().into_simple_expr()))
			.from_subquery(self.stmt.clone(), Alias::new("subquery"))
			.to_owned()
	}

	fn exists(&self) -> SelectStatement {
		Query::select()
			.expr(Expr::exists(self.stmt.clone()))
			.to_owned()
	}

	async fn get_async(
		&self,
		db: &super::connection::DatabaseConnection,
		pk: &T::PrimaryKey,
	) -> Result<T, ExecutionError>
	where
		T: for<'de> serde::Deserialize<'de>,
	{
		let stmt = self.get(pk);
		let (sql, values) = stmt.build_any(&reinhardt_query::prelude::PostgresQueryBuilder);

		let query_values = convert_values(values);
		let row = db.query_one(&sql, query_values).await?;
		let json = serde_json::to_value(&row)?;
		let result = serde_json::from_value(json)?;
		Ok(result)
	}

	async fn all_async(
		&self,
		db: &super::connection::DatabaseConnection,
	) -> Result<Vec<T>, ExecutionError>
	where
		T: for<'de> serde::Deserialize<'de>,
	{
		let stmt = self.all();
		let (sql, values) = stmt.build_any(&reinhardt_query::prelude::PostgresQueryBuilder);

		let query_values = convert_values(values);
		let rows = db.query(&sql, query_values).await?;
		let mut results = Vec::with_capacity(rows.len());
		for row in rows {
			let json = serde_json::to_value(&row)?;
			let result = serde_json::from_value(json)?;
			results.push(result);
		}
		Ok(results)
	}

	async fn first_async(
		&self,
		db: &super::connection::DatabaseConnection,
	) -> Result<Option<T>, ExecutionError>
	where
		T: for<'de> serde::Deserialize<'de>,
	{
		let stmt = self.first();
		let (sql, values) = stmt.build_any(&reinhardt_query::prelude::PostgresQueryBuilder);

		let query_values = convert_values(values);
		let rows = db.query(&sql, query_values).await?;
		match rows.first() {
			Some(row) => {
				let json = serde_json::to_value(row)?;
				let result = serde_json::from_value(json)?;
				Ok(Some(result))
			}
			None => Ok(None),
		}
	}

	async fn one_async(
		&self,
		db: &super::connection::DatabaseConnection,
	) -> Result<T, ExecutionError>
	where
		T: for<'de> serde::Deserialize<'de>,
	{
		let stmt = self.one();
		let (sql, values) = stmt.build_any(&reinhardt_query::prelude::PostgresQueryBuilder);

		let query_values = convert_values(values);
		let rows = db.query(&sql, query_values).await?;
		match rows.len() {
			0 => Err(ExecutionError::NoResultFound),
			1 => {
				let json = serde_json::to_value(&rows[0])?;
				let result = serde_json::from_value(json)?;
				Ok(result)
			}
			n => Err(ExecutionError::MultipleResultsFound(n)),
		}
	}

	async fn one_or_none_async(
		&self,
		db: &super::connection::DatabaseConnection,
	) -> Result<Option<T>, ExecutionError>
	where
		T: for<'de> serde::Deserialize<'de>,
	{
		let stmt = self.one_or_none();
		let (sql, values) = stmt.build_any(&reinhardt_query::prelude::PostgresQueryBuilder);

		let query_values = convert_values(values);
		let rows = db.query(&sql, query_values).await?;
		match rows.len() {
			0 => Ok(None),
			1 => {
				let json = serde_json::to_value(&rows[0])?;
				let result = serde_json::from_value(json)?;
				Ok(Some(result))
			}
			n => Err(ExecutionError::MultipleResultsFound(n)),
		}
	}

	async fn scalar_async<S>(
		&self,
		db: &super::connection::DatabaseConnection,
	) -> Result<Option<S>, ExecutionError>
	where
		S: for<'de> serde::Deserialize<'de>,
	{
		let stmt = self.scalar();
		let (sql, values) = stmt.build_any(&reinhardt_query::prelude::PostgresQueryBuilder);

		let query_values = convert_values(values);
		let rows = db.query(&sql, query_values).await?;
		match rows.first() {
			Some(row) => {
				// Get the first column value
				let json = serde_json::to_value(row)?;
				if let Some(obj) = json.as_object()
					&& let Some((_, value)) = obj.iter().next()
				{
					let result = serde_json::from_value(value.clone())?;
					return Ok(Some(result));
				}
				Ok(None)
			}
			None => Ok(None),
		}
	}

	async fn count_async(
		&self,
		db: &super::connection::DatabaseConnection,
	) -> Result<i64, ExecutionError> {
		let stmt = self.count();
		let (sql, values) = stmt.build_any(&reinhardt_query::prelude::PostgresQueryBuilder);

		let query_values = convert_values(values);
		let row = db.query_one(&sql, query_values).await?;
		let json = serde_json::to_value(&row)?;

		// Extract count from the result (usually the first column)
		if let Some(obj) = json.as_object()
			&& let Some((_, value)) = obj.iter().next()
		{
			let count: i64 = serde_json::from_value(value.clone())?;
			return Ok(count);
		}

		Err(ExecutionError::QueryBuild(
			"Count query returned unexpected format".to_string(),
		))
	}

	async fn exists_async(
		&self,
		db: &super::connection::DatabaseConnection,
	) -> Result<bool, ExecutionError> {
		let stmt = self.exists();
		let (sql, values) = stmt.build_any(&reinhardt_query::prelude::PostgresQueryBuilder);

		let query_values = convert_values(values);
		let row = db.query_one(&sql, query_values).await?;
		let json = serde_json::to_value(&row)?;

		// Extract exists from the result (usually the first column)
		if let Some(obj) = json.as_object()
			&& let Some((_, value)) = obj.iter().next()
		{
			let exists: bool = serde_json::from_value(value.clone())?;
			return Ok(exists);
		}

		Err(ExecutionError::QueryBuild(
			"Exists query returned unexpected format".to_string(),
		))
	}
}

/// Loading options for relationships
/// Corresponds to SQLAlchemy's loader options
#[derive(Debug, Clone)]
pub enum LoadOption {
	/// Eager load with JOIN
	/// Corresponds to joinedload()
	JoinedLoad(String),

	/// Eager load with separate SELECT
	/// Corresponds to selectinload()
	SelectInLoad(String),

	/// Lazy load on access
	/// Corresponds to lazyload()
	LazyLoad(String),

	/// Don't load at all
	/// Corresponds to noload()
	NoLoad(String),

	/// Raise error if accessed
	/// Corresponds to raiseload()
	RaiseLoad(String),

	/// Defer column loading
	/// Corresponds to defer()
	Defer(String),

	/// Undefer column loading
	/// Corresponds to undefer()
	Undefer(String),

	/// Load only specified columns
	/// Corresponds to load_only()
	LoadOnly(Vec<String>),
}

impl LoadOption {
	/// Convert load option to SQL comment for debugging
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::execution::LoadOption;
	///
	/// let option = LoadOption::JoinedLoad("profile".to_string());
	/// assert_eq!(option.to_sql_comment(), "/* joinedload(profile) */");
	///
	/// let option = LoadOption::Defer("password".to_string());
	/// assert_eq!(option.to_sql_comment(), "/* defer(password) */");
	///
	/// let option = LoadOption::LoadOnly(vec!["id".to_string(), "name".to_string()]);
	/// assert_eq!(option.to_sql_comment(), "/* load_only(id, name) */");
	/// ```
	pub fn to_sql_comment(&self) -> String {
		match self {
			LoadOption::JoinedLoad(rel) => format!("/* joinedload({}) */", rel),
			LoadOption::SelectInLoad(rel) => format!("/* selectinload({}) */", rel),
			LoadOption::LazyLoad(rel) => format!("/* lazyload({}) */", rel),
			LoadOption::NoLoad(rel) => format!("/* noload({}) */", rel),
			LoadOption::RaiseLoad(rel) => format!("/* raiseload({}) */", rel),
			LoadOption::Defer(col) => format!("/* defer({}) */", col),
			LoadOption::Undefer(col) => format!("/* undefer({}) */", col),
			LoadOption::LoadOnly(cols) => format!("/* load_only({}) */", cols.join(", ")),
		}
	}
}

/// Query options container
#[non_exhaustive]
pub struct QueryOptions {
	pub load_options: Vec<LoadOption>,
}

impl QueryOptions {
	/// Create a new empty query options container
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::execution::QueryOptions;
	///
	/// let options = QueryOptions::new();
	/// assert_eq!(options.to_sql_comments(), "");
	/// ```
	pub fn new() -> Self {
		Self {
			load_options: Vec::new(),
		}
	}
	/// Add a load option to the query
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::execution::{QueryOptions, LoadOption};
	///
	/// let options = QueryOptions::new()
	///     .add_option(LoadOption::JoinedLoad("profile".to_string()))
	///     .add_option(LoadOption::Defer("password".to_string()));
	///
	/// let comments = options.to_sql_comments();
	/// assert!(comments.contains("joinedload(profile)"));
	/// assert!(comments.contains("defer(password)"));
	/// ```
	pub fn add_option(mut self, option: LoadOption) -> Self {
		self.load_options.push(option);
		self
	}
	/// Convert all options to SQL comments
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::execution::{QueryOptions, LoadOption};
	///
	/// let options = QueryOptions::new()
	///     .add_option(LoadOption::SelectInLoad("posts".to_string()));
	///
	/// assert!(options.to_sql_comments().contains("selectinload(posts)"));
	/// ```
	pub fn to_sql_comments(&self) -> String {
		if self.load_options.is_empty() {
			String::new()
		} else {
			format!(
				" {}",
				self.load_options
					.iter()
					.map(|o| o.to_sql_comment())
					.collect::<Vec<_>>()
					.join(" ")
			)
		}
	}
}

impl Default for QueryOptions {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_core::validators::TableName;
	use rstest::rstest;
	use serde::{Deserialize, Serialize};

	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct User {
		id: Option<i64>,
		name: String,
	}

	#[derive(Clone)]
	struct UserFields;
	impl crate::orm::model::FieldSelector for UserFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	const USER_TABLE: TableName = TableName::new_const("users");

	impl Model for User {
		type PrimaryKey = i64;
		type Fields = UserFields;

		fn table_name() -> &'static str {
			USER_TABLE.as_str()
		}

		fn new_fields() -> Self::Fields {
			UserFields
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}
	}

	#[test]
	fn test_execution_get() {
		use reinhardt_query::prelude::{Alias, PostgresQueryBuilder, Query, QueryStatementBuilder};

		let stmt = Query::select()
			.from(Alias::new("users"))
			.column(ColumnRef::Asterisk)
			.to_owned();
		let exec = SelectExecution::<User>::new(stmt);
		let result_stmt = exec.get(&123);
		let sql = result_stmt.to_string(PostgresQueryBuilder);
		assert!(sql.contains("WHERE"));
		assert!(sql.contains("LIMIT"));
	}

	#[test]
	fn test_all() {
		use reinhardt_query::prelude::{Alias, PostgresQueryBuilder, Query, QueryStatementBuilder};

		let stmt = Query::select()
			.from(Alias::new("users"))
			.column(ColumnRef::Asterisk)
			.to_owned();
		let exec = SelectExecution::<User>::new(stmt);
		let result_stmt = exec.all();
		let sql = result_stmt.to_string(PostgresQueryBuilder);
		assert!(sql.contains("SELECT"));
		assert!(sql.contains("users"));
	}

	#[test]
	fn test_first() {
		use reinhardt_query::prelude::{
			Alias, Expr, PostgresQueryBuilder, Query, QueryStatementBuilder,
		};

		let stmt = Query::select()
			.from(Alias::new("users"))
			.column(ColumnRef::Asterisk)
			.and_where(Expr::col(Alias::new("active")).eq(true))
			.to_owned();
		let exec = SelectExecution::<User>::new(stmt);
		let result_stmt = exec.first();
		let sql = result_stmt.to_string(PostgresQueryBuilder);
		assert!(sql.contains("LIMIT"));
	}

	#[test]
	fn test_execution_count() {
		use reinhardt_query::prelude::{
			Alias, Expr, PostgresQueryBuilder, Query, QueryStatementBuilder,
		};

		let stmt = Query::select()
			.from(Alias::new("users"))
			.column(ColumnRef::Asterisk)
			.and_where(Expr::col(Alias::new("active")).eq(true))
			.to_owned();
		let exec = SelectExecution::<User>::new(stmt);
		let result_stmt = exec.count();
		let sql = result_stmt.to_string(PostgresQueryBuilder);
		assert!(sql.contains("COUNT"));
	}

	#[test]
	fn test_execution_exists() {
		use reinhardt_query::prelude::{
			Alias, Expr, PostgresQueryBuilder, Query, QueryStatementBuilder,
		};

		let stmt = Query::select()
			.from(Alias::new("users"))
			.column(ColumnRef::Asterisk)
			.and_where(Expr::col(Alias::new("name")).eq("Alice"))
			.to_owned();
		let exec = SelectExecution::<User>::new(stmt);
		let result_stmt = exec.exists();
		let sql = result_stmt.to_string(PostgresQueryBuilder);
		assert!(sql.contains("EXISTS"));
	}

	#[test]
	fn test_load_options() {
		let options = QueryOptions::new()
			.add_option(LoadOption::JoinedLoad("profile".to_string()))
			.add_option(LoadOption::Defer("password".to_string()));

		let comments = options.to_sql_comments();
		assert!(comments.contains("joinedload(profile)"));
		assert!(comments.contains("defer(password)"));
	}

	#[test]
	fn test_load_only() {
		let option = LoadOption::LoadOnly(vec!["id".to_string(), "name".to_string()]);
		let comment = option.to_sql_comment();
		assert!(comment.contains("load_only(id, name)"));
	}

	#[rstest]
	#[case::zero(0u64, 0i64)]
	#[case::one(1u64, 1i64)]
	#[case::i64_max(i64::MAX as u64, i64::MAX)]
	#[test]
	fn test_big_unsigned_to_query_value_within_range(#[case] input: u64, #[case] expected: i64) {
		// Arrange
		let value = reinhardt_query::value::Value::BigUnsigned(Some(input));

		// Act
		let result = convert_value_to_query_value(value);

		// Assert
		assert!(matches!(result, QueryValue::Int(v) if v == expected));
	}

	#[rstest]
	#[case::i64_max_plus_one(i64::MAX as u64 + 1)]
	#[case::u64_max(u64::MAX)]
	#[test]
	fn test_big_unsigned_overflow_clamps_to_i64_max(#[case] input: u64) {
		// Arrange
		let value = reinhardt_query::value::Value::BigUnsigned(Some(input));

		// Act
		let result = convert_value_to_query_value(value);

		// Assert: Should clamp to i64::MAX instead of wrapping to negative
		assert!(matches!(result, QueryValue::Int(v) if v == i64::MAX));
	}

	#[rstest]
	#[test]
	fn test_big_unsigned_none_converts_to_null() {
		// Arrange
		let value = reinhardt_query::value::Value::BigUnsigned(None);

		// Act
		let result = convert_value_to_query_value(value);

		// Assert
		assert!(matches!(result, QueryValue::Null));
	}
}
