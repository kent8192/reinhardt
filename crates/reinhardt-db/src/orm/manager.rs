use super::connection::{
	DatabaseBackend, DatabaseConnection, DatabaseConnectionLease, OrmExecutor, QueryRow, QueryValue,
};
use super::field_codec::{DatabaseArrayType, database_value_to_query_value};
use super::inspection::FieldInfo;
use super::query::RelationLoadInput;
use super::{DatabaseValue, FieldCodecError, Model, QuerySet};
use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error};
use reinhardt_query::prelude::{
	Alias, ColumnRef, DeleteStatement, Expr, ExprTrait, Func, InsertStatement, MySqlQueryBuilder,
	PostgresQueryBuilder, Query, QueryBuilder, SelectStatement, SqliteQueryBuilder,
	UpdateStatement, Values,
};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

fn find_field_info<'a>(field_metadata: &'a [FieldInfo], field_name: &str) -> Option<&'a FieldInfo> {
	field_metadata.iter().find(|field| field.name == field_name)
}

fn field_codec_error(error: FieldCodecError) -> Error {
	let kind = match &error {
		FieldCodecError::TypeMismatch { .. } | FieldCodecError::InvalidEnumValue { .. } => {
			DatabaseErrorKind::Type
		}
		FieldCodecError::Serialization(_) => DatabaseErrorKind::Serialization,
	};
	let message = error.to_string();
	Error::database_with_source(kind, message, error)
}

fn executor_field_codec_error(error: FieldCodecError) -> crate::backends::error::DatabaseError {
	crate::backends::error::DatabaseError::new(
		crate::backends::error::DatabaseErrorKind::Serialization,
		error.to_string(),
	)
}

fn executor_error(error: Error) -> crate::backends::error::DatabaseError {
	crate::backends::error::into_database_error(error)
}

/// Build SQL with values from an INSERT statement based on database backend
fn build_insert_sql(stmt: &InsertStatement, backend: DatabaseBackend) -> (String, Values) {
	match backend {
		DatabaseBackend::Postgres => PostgresQueryBuilder.build_insert(stmt),
		DatabaseBackend::MySql => MySqlQueryBuilder.build_insert(stmt),
		DatabaseBackend::Sqlite => SqliteQueryBuilder.build_insert(stmt),
	}
}

/// Build SQL with values from an UPDATE statement based on database backend
fn build_update_sql(stmt: &UpdateStatement, backend: DatabaseBackend) -> (String, Values) {
	match backend {
		DatabaseBackend::Postgres => PostgresQueryBuilder.build_update(stmt),
		DatabaseBackend::MySql => MySqlQueryBuilder.build_update(stmt),
		DatabaseBackend::Sqlite => SqliteQueryBuilder.build_update(stmt),
	}
}

/// Build SQL with values from a SELECT statement based on database backend
fn build_select_sql(stmt: &SelectStatement, backend: DatabaseBackend) -> (String, Values) {
	match backend {
		DatabaseBackend::Postgres => PostgresQueryBuilder.build_select(stmt),
		DatabaseBackend::MySql => MySqlQueryBuilder.build_select(stmt),
		DatabaseBackend::Sqlite => SqliteQueryBuilder.build_select(stmt),
	}
}

/// Convert a SELECT statement to SQL string based on database backend
fn select_to_string(stmt: &SelectStatement, backend: DatabaseBackend) -> String {
	build_select_sql(stmt, backend).0
}

/// Convert an INSERT statement to SQL string based on database backend
fn insert_to_string(stmt: &InsertStatement, backend: DatabaseBackend) -> String {
	build_insert_sql(stmt, backend).0
}

/// Build SQL with values from a DELETE statement based on database backend
fn build_delete_sql(stmt: &DeleteStatement, backend: DatabaseBackend) -> (String, Values) {
	match backend {
		DatabaseBackend::Postgres => PostgresQueryBuilder.build_delete(stmt),
		DatabaseBackend::MySql => MySqlQueryBuilder.build_delete(stmt),
		DatabaseBackend::Sqlite => SqliteQueryBuilder.build_delete(stmt),
	}
}

fn database_value_sql_literal(
	value: DatabaseValue,
	backend: DatabaseBackend,
) -> Result<String, FieldCodecError> {
	if let DatabaseValue::Array {
		element_type,
		values,
	} = &value
		&& backend == DatabaseBackend::Postgres
		&& values.is_empty()
	{
		let element_type = match element_type {
			DatabaseArrayType::String => "text",
			DatabaseArrayType::I32 => "integer",
			DatabaseArrayType::I64 => "bigint",
			DatabaseArrayType::F32 => "real",
			DatabaseArrayType::F64 => "double precision",
			DatabaseArrayType::Bool => "boolean",
			DatabaseArrayType::Uuid => "uuid",
		};
		return Ok(format!("ARRAY[]::{element_type}[]"));
	}

	if backend == DatabaseBackend::Postgres || !matches!(&value, DatabaseValue::Array { .. }) {
		return Ok(database_value_to_query_value(value).to_sql_literal());
	}

	let json = value.into_json_value()?;
	Ok(format!("'{}'", json.to_string().replace('\'', "''")))
}

fn quote_identifier(identifier: &str, backend: DatabaseBackend) -> String {
	let quote = if backend == DatabaseBackend::MySql {
		'`'
	} else {
		'"'
	};
	format!("{quote}{identifier}{quote}")
}

struct DefaultDatabase {
	lease: DatabaseConnectionLease,
	handle: DatabaseConnection,
}

/// Global database connection state
static DB: once_cell::sync::OnceCell<Arc<RwLock<Option<DefaultDatabase>>>> =
	once_cell::sync::OnceCell::new();

/// Initialize the global database connection
///
/// # Arguments
///
/// * `url` - Database connection URL
///
/// # Examples
///
/// ```no_run
/// # async fn example() {
/// use reinhardt_db::orm::manager::init_database;
///
/// init_database("postgres://localhost/mydb").await.unwrap();
/// # }
/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
/// ```
pub async fn init_database(url: &str) -> reinhardt_core::exception::Result<()> {
	init_database_with_pool_size(url, None).await
}

/// Initialize the global database connection with a specific pool size
///
/// If the global connection is already initialized, this function returns
/// successfully without opening another connection.
///
/// # Arguments
///
/// * `url` - Database connection URL
/// * `pool_size` - Maximum number of connections in the pool (None = use default)
///
/// # Examples
///
/// ```no_run
/// # async fn example() {
/// use reinhardt_db::orm::manager::init_database_with_pool_size;
///
/// // Use larger pool for high-concurrency tests
/// init_database_with_pool_size("postgres://localhost/mydb", Some(50)).await.unwrap();
/// # }
/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
/// ```
pub async fn init_database_with_pool_size(
	url: &str,
	pool_size: Option<u32>,
) -> reinhardt_core::exception::Result<()> {
	if let Some(db_cell) = DB.get()
		&& db_cell.read().await.is_some()
	{
		return Ok(());
	}

	let owner = super::engine::connect_backend_with_pool_size(url, pool_size).await?;
	let lease = DatabaseConnectionLease::register(owner)?;
	let database = DefaultDatabase {
		handle: lease.handle(),
		lease,
	};

	if let Some(db_cell) = DB.get() {
		let mut guard = db_cell.write().await;
		if guard.is_none() {
			*guard = Some(database);
		}
	} else {
		DB.get_or_init(|| Arc::new(RwLock::new(Some(database))));
	}

	Ok(())
}

/// Reinitialize the global database connection (for testing)
///
/// This function replaces the existing database connection with a new one.
/// Useful for test scenarios where each test needs a fresh connection pool.
///
/// # Arguments
///
/// * `url` - Database connection URL
///
/// # Examples
///
/// ```no_run
/// # async fn example() {
/// use reinhardt_db::orm::manager::reinitialize_database;
///
/// reinitialize_database("postgres://localhost/mydb").await.unwrap();
/// # }
/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
/// ```
pub async fn reinitialize_database(url: &str) -> reinhardt_core::exception::Result<()> {
	reinitialize_database_with_pool_size(url, None).await
}

/// Reinitialize the global database connection with a specific pool size (for testing)
///
/// # Arguments
///
/// * `url` - Database connection URL
/// * `pool_size` - Maximum number of connections in the pool (None = use default)
///
/// # Examples
///
/// ```no_run
/// # async fn example() {
/// use reinhardt_db::orm::manager::reinitialize_database_with_pool_size;
///
/// // Use larger pool for concurrent tests
/// reinitialize_database_with_pool_size("postgres://localhost/mydb", Some(30)).await.unwrap();
/// # }
/// # tokio::runtime::Runtime::new().unwrap().block_on(example());
/// ```
pub async fn reinitialize_database_with_pool_size(
	url: &str,
	pool_size: Option<u32>,
) -> reinhardt_core::exception::Result<()> {
	let owner = super::engine::connect_backend_with_pool_size(url, pool_size).await?;
	let lease = DatabaseConnectionLease::register(owner)?;
	let database = DefaultDatabase {
		handle: lease.handle(),
		lease,
	};

	if let Some(db_cell) = DB.get() {
		// Replace existing connection
		let mut guard = db_cell.write().await;
		*guard = Some(database);
	} else {
		// First time initialization
		DB.get_or_init(|| Arc::new(RwLock::new(Some(database))));
	}

	Ok(())
}

/// Replace the global ORM database connection and return the previous value.
///
/// This is intended for test fixtures that need to mutate global ORM state while
/// preserving RAII cleanup semantics. Passing `None` clears the global connection.
#[doc(hidden)]
pub async fn replace_database_connection_for_testing(
	lease: Option<DatabaseConnectionLease>,
) -> Option<DatabaseConnectionLease> {
	let db = DB.get_or_init(|| Arc::new(RwLock::new(None)));
	let mut guard = db.write().await;
	let database = lease.map(|lease| DefaultDatabase {
		handle: lease.handle(),
		lease,
	});
	std::mem::replace(&mut *guard, database).map(|database| database.lease)
}

/// Get a reference to the global database connection
pub async fn get_connection() -> reinhardt_core::exception::Result<DatabaseConnection> {
	let db = DB.get().ok_or_else(|| {
		Error::from(DatabaseError::new(
			DatabaseErrorKind::Configuration,
			"Database not initialized",
		))
	})?;
	let guard = db.read().await;
	guard
		.as_ref()
		.map(|database| database.handle)
		.ok_or_else(|| {
			Error::from(DatabaseError::new(
				DatabaseErrorKind::Connection,
				"Database connection not available",
			))
		})
}

/// Returns a lease that retains the global ORM database registration.
#[doc(hidden)]
pub async fn get_connection_lease() -> reinhardt_core::exception::Result<DatabaseConnectionLease> {
	let db = DB.get().ok_or_else(|| {
		Error::from(DatabaseError::new(
			DatabaseErrorKind::Configuration,
			"Database not initialized",
		))
	})?;
	let guard = db.read().await;
	guard
		.as_ref()
		.map(|database| database.lease.clone())
		.ok_or_else(|| {
			Error::from(DatabaseError::new(
				DatabaseErrorKind::Connection,
				"Database connection not available",
			))
		})
}

/// Returns a coherent lease-and-handle snapshot of the global ORM registration.
#[doc(hidden)]
pub async fn get_connection_registration()
-> reinhardt_core::exception::Result<(DatabaseConnectionLease, DatabaseConnection)> {
	let db = DB.get().ok_or_else(|| {
		Error::from(DatabaseError::new(
			DatabaseErrorKind::Configuration,
			"Database not initialized",
		))
	})?;
	let guard = db.read().await;
	guard
		.as_ref()
		.map(|database| (database.lease.clone(), database.handle))
		.ok_or_else(|| {
			Error::from(DatabaseError::new(
				DatabaseErrorKind::Connection,
				"Database connection not available",
			))
		})
}

/// Model manager (similar to Django's Manager)
/// Provides an interface for database operations
pub struct Manager<M: Model> {
	_marker: PhantomData<M>,
}

impl<M: Model> Manager<M> {
	/// Creates a new instance.
	pub fn new() -> Self {
		Self {
			_marker: PhantomData,
		}
	}

	fn executor_backend(executor: &dyn super::connection::TransactionExecutor) -> DatabaseBackend {
		match executor.backend() {
			crate::backends::types::DatabaseType::Postgres => DatabaseBackend::Postgres,
			crate::backends::types::DatabaseType::Mysql => DatabaseBackend::MySql,
			crate::backends::types::DatabaseType::Sqlite => DatabaseBackend::Sqlite,
		}
	}

	fn decode_executor_row(
		row: crate::backends::types::Row,
	) -> Result<M, crate::backends::error::DatabaseError> {
		super::connection::QueryRow::from_backend_row(row)
			.deserialize_model::<M>()
			.map_err(executor_field_codec_error)
	}

	/// Preserve the model-declared primary-key storage type when binding executor queries.
	fn primary_key_query_value(pk: &M::PrimaryKey) -> reinhardt_query::value::Value {
		let pk_string = pk.to_string();
		let field_type = M::field_metadata()
			.into_iter()
			.find(|field| field.name == M::primary_key_field())
			.map(|field| field.field_type);

		match field_type
			.as_deref()
			.and_then(|value| value.rsplit('.').next())
		{
			Some("AutoField")
			| Some("IntegerField")
			| Some("BigAutoField")
			| Some("BigIntegerField") => pk_string.parse::<i64>().map_or_else(
				|_| reinhardt_query::value::Value::String(Some(Box::new(pk_string.clone()))),
				|value| reinhardt_query::value::Value::BigInt(Some(value)),
			),
			Some("UuidField") => Uuid::parse_str(&pk_string).map_or_else(
				|_| reinhardt_query::value::Value::String(Some(Box::new(pk_string.clone()))),
				|value| reinhardt_query::value::Value::Uuid(Some(Box::new(value))),
			),
			// Manual models often do not provide field metadata. Preserve the
			// numeric storage type for those models when the primary key is
			// represented by an integer.
			_ => pk_string.parse::<i64>().map_or_else(
				|_| reinhardt_query::value::Value::String(Some(Box::new(pk_string))),
				|value| reinhardt_query::value::Value::BigInt(Some(value)),
			),
		}
	}

	fn is_generated_field(field: &str) -> bool {
		M::generated_field_names().contains(&field)
	}

	fn field_column<'a>(field_metadata: &'a [FieldInfo], field_name: &'a str) -> &'a str {
		find_field_info(field_metadata, field_name)
			.map(FieldInfo::db_column_name)
			.unwrap_or_else(|| {
				if field_name == M::primary_key_field() {
					M::primary_key_column()
				} else {
					field_name
				}
			})
	}

	fn normalize_primary_key_aliases(
		mut fields: HashMap<String, String>,
	) -> reinhardt_core::exception::Result<HashMap<String, String>> {
		let primary_key_field = M::primary_key_field();
		let primary_key_column = M::primary_key_column();
		if primary_key_field == primary_key_column {
			return Ok(fields);
		}

		let logical_value = fields.get(primary_key_field).cloned();
		let physical_value = fields.get(primary_key_column).cloned();
		match (logical_value, physical_value) {
			(Some(logical_value), Some(physical_value)) if logical_value != physical_value => {
				return Err(Error::Validation(format!(
					"Conflicting values for primary key aliases `{primary_key_field}` and `{primary_key_column}`"
				)));
			}
			(Some(_), Some(_)) => {
				fields.remove(primary_key_column);
			}
			(None, Some(physical_value)) => {
				fields.remove(primary_key_column);
				fields.insert(primary_key_field.to_string(), physical_value);
			}
			_ => {}
		}
		Ok(fields)
	}

	fn returning_columns_from_object(
		obj: &std::collections::BTreeMap<String, DatabaseValue>,
	) -> Vec<Alias> {
		let primary_key = M::primary_key_field();
		let mut columns: Vec<&str> = obj.keys().map(String::as_str).collect();
		columns.sort_unstable();
		if let Some(index) = columns.iter().position(|column| *column == primary_key) {
			let pk = columns.remove(index);
			columns.insert(0, pk);
		}
		let field_metadata = M::field_metadata();
		columns
			.into_iter()
			.map(|column| Alias::new(Self::field_column(&field_metadata, column)))
			.collect()
	}

	fn build_update_statement_from_object(
		obj: &std::collections::BTreeMap<String, DatabaseValue>,
		_field_is_none: impl Fn(&str) -> bool,
	) -> Result<reinhardt_query::prelude::UpdateStatement, FieldCodecError> {
		Self::build_update_statement_from_object_with_returning(obj, true)
	}

	fn build_update_statement_from_object_with_returning(
		obj: &std::collections::BTreeMap<String, DatabaseValue>,
		include_returning: bool,
	) -> Result<reinhardt_query::prelude::UpdateStatement, FieldCodecError> {
		let mut stmt = Query::update();
		stmt.table(Alias::new(M::table_name()));
		let field_metadata = M::field_metadata();

		let mut has_values = false;
		for (k, v) in obj.iter().filter(|(k, _)| {
			let key = k.as_str();
			key != M::primary_key_field() && !Self::is_generated_field(key)
		}) {
			let column_name = Self::field_column(&field_metadata, k);
			if matches!(v, DatabaseValue::Null) {
				stmt.value_expr(Alias::new(column_name), Expr::cust("NULL"));
			} else {
				stmt.value(
					Alias::new(column_name),
					database_value_to_query_value(v.clone()),
				);
			}
			has_values = true;
		}

		if !has_values {
			let primary_key = M::primary_key_field();
			let primary_key_column = Self::field_column(&field_metadata, primary_key);
			stmt.value_expr(
				Alias::new(primary_key_column),
				Expr::col(Alias::new(primary_key_column)),
			);
		}

		let pk_value = obj
			.get(M::primary_key_field())
			.filter(|value| !matches!(value, DatabaseValue::Null))
			.cloned()
			.ok_or_else(|| {
				FieldCodecError::Serialization(format!(
					"encoded {} fields must contain a non-null primary key '{}'",
					M::table_name(),
					M::primary_key_field()
				))
			})?;
		let primary_key_column = Self::field_column(&field_metadata, M::primary_key_field());
		stmt.and_where(
			Expr::col(Alias::new(primary_key_column)).eq(database_value_to_query_value(pk_value)),
		);

		if include_returning {
			stmt.returning(Self::returning_columns_from_object(obj));
		}
		Ok(stmt)
	}

	fn build_insert_statement_from_object(
		obj: &std::collections::BTreeMap<String, DatabaseValue>,
		_field_is_none: impl Fn(&str) -> bool,
	) -> reinhardt_core::exception::Result<InsertStatement> {
		let mut stmt = Query::insert();
		stmt.into_table(Alias::new(M::table_name()));

		let pk_field = M::primary_key_field();
		let field_metadata = M::field_metadata();
		let (fields, values): (Vec<_>, Vec<_>) = obj
			.iter()
			.filter(|(k, v)| {
				let key = k.as_str();
				if Self::is_generated_field(key) {
					return false;
				}
				if key == pk_field {
					if matches!(v, DatabaseValue::Null) {
						return false;
					}
					if M::primary_key_uses_zero_sentinel()
						&& matches!(v, DatabaseValue::I32(0) | DatabaseValue::I64(0))
					{
						return false;
					}
				}
				if matches!(v, DatabaseValue::Null)
					&& (key == "created_at"
						|| key == "updated_at"
						|| key.ends_with("_date")
						|| key.ends_with("_time")
						|| key.ends_with("_at"))
				{
					return false;
				}
				true
			})
			.map(|(k, v)| {
				let value = database_value_to_query_value(v.clone());
				(Alias::new(Self::field_column(&field_metadata, k)), value)
			})
			.unzip();

		if fields.is_empty() {
			return Err(Error::from(DatabaseError::new(
				DatabaseErrorKind::Query,
				format!(
					"Cannot create {} because no writable fields remain after filtering generated and defaulted columns",
					M::table_name()
				),
			)));
		}

		stmt.columns(fields);
		stmt.values_panic(values);

		Ok(stmt)
	}

	/// Get all records
	pub fn all(&self) -> QuerySet<M> {
		QuerySet::new()
	}

	/// Filter records by a typed filter expression.
	///
	/// Accepts typed and untyped inputs through
	/// [`QueryFilterInput`](super::query::QueryFilterInput).
	/// The intended call style is the fluent builder produced by the
	/// `#[model]`-generated field accessors (`FieldRef::eq()` / `.gt()` / ...)
	/// or a composite condition built with `.and()`, `.or()`, and `.not()`.
	///
	/// # Examples
	///
	/// ```ignore
	/// // Typed builder (recommended):
	/// User::objects()
	///     .filter(User::field_email().eq("alice@example.com"))
	///     .all()
	///     .await?;
	///
	/// // Raw Filter (when the field name is dynamic):
	/// use reinhardt_db::orm::{Filter, FilterOperator, FilterValue};
	/// User::objects()
	///     .filter(Filter::new("email", FilterOperator::Eq, FilterValue::String("alice@example.com".to_string())))
	///     .all()
	///     .await?;
	/// ```
	pub fn filter(&self, filter: impl super::query::QueryFilterInput<M>) -> QuerySet<M> {
		QuerySet::new().filter(filter)
	}

	/// Get a single record by primary key
	/// Returns a QuerySet filtered by the primary key field
	pub fn get(&self, pk: M::PrimaryKey) -> QuerySet<M> {
		let pk_field = M::primary_key_column();
		let pk_str = pk.to_string();

		// Try to parse as i64 first (common for primary keys), fallback to string
		let pk_value = if let Ok(int_value) = pk_str.parse::<i64>() {
			super::query::FilterValue::Integer(int_value)
		} else {
			super::query::FilterValue::String(pk_str)
		};

		let filter = super::query::Filter::new(
			pk_field.to_string(),
			super::query::FilterOperator::Eq,
			pk_value,
		);
		QuerySet::new().filter(filter)
	}

	/// Set LIMIT clause
	///
	/// Limits the number of records returned by the QuerySet.
	/// Corresponds to Django's `QuerySet[:n]`.
	///
	/// # Examples
	///
	/// ```ignore
	/// let users = User::objects().limit(10).all().await?;
	/// ```
	pub fn limit(&self, limit: usize) -> QuerySet<M> {
		QuerySet::new().limit(limit)
	}

	/// Set ORDER BY clause
	///
	/// Sorts the QuerySet results by the specified fields.
	/// Use "-" prefix for descending order (e.g., "-created_at").
	/// Corresponds to Django's QuerySet.order_by().
	///
	/// # Examples
	///
	/// ```ignore
	/// // Ascending by name
	/// let users = User::objects().order_by(&["name"]).all().await?;
	///
	/// // Descending by created_at
	/// let users = User::objects().order_by(&["-created_at"]).all().await?;
	///
	/// // Multiple fields
	/// let users = User::objects().order_by(&["department", "-salary"]).all().await?;
	/// ```
	pub fn order_by(&self, fields: &[&str]) -> QuerySet<M> {
		QuerySet::new().order_by(fields)
	}

	/// Add annotation to QuerySet
	///
	/// Adds a computed field to each record using SQL expressions or aggregations.
	/// Corresponds to Django's QuerySet.annotate().
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_db::orm::annotation::{Annotation, AnnotationValue};
	/// use reinhardt_db::orm::aggregation::Aggregate;
	///
	/// let users = User::objects()
	///     .annotate(Annotation::new("total_orders",
	///         AnnotationValue::Aggregate(Aggregate::count(Some("orders")))))
	///     .all()
	///     .await?;
	/// ```
	pub fn annotate(&self, annotation: super::annotation::Annotation) -> QuerySet<M> {
		QuerySet::new().annotate(annotation)
	}

	/// Defer loading of specified fields
	///
	/// Excludes the specified fields from the initial query, loading them only when accessed.
	/// Corresponds to Django's QuerySet.defer().
	///
	/// # Examples
	///
	/// ```ignore
	/// let users = User::objects().defer(&["bio", "profile_picture"]).all().await?;
	/// ```
	pub fn defer(&self, fields: &[&str]) -> QuerySet<M> {
		QuerySet::new().defer(fields)
	}

	/// Load only specified fields
	///
	/// Loads only the specified fields, excluding all others.
	/// Corresponds to Django's QuerySet.only().
	///
	/// # Examples
	///
	/// ```ignore
	/// let users = User::objects().only(&["id", "username"]).all().await?;
	/// ```
	pub fn only(&self, fields: &[&str]) -> QuerySet<M> {
		QuerySet::new().only(fields)
	}

	/// Select specific fields (values)
	///
	/// Returns records with only the specified fields.
	/// Corresponds to Django's QuerySet.values().
	///
	/// # Examples
	///
	/// ```ignore
	/// let user_data = User::objects().values(&["id", "username", "email"]).all().await?;
	/// ```
	pub fn values(&self, fields: &[&str]) -> QuerySet<M> {
		QuerySet::new().values(fields)
	}

	/// Eager load related objects using JOIN
	///
	/// Performs SQL JOINs to load related objects in a single query.
	/// Corresponds to Django's QuerySet.select_related().
	///
	/// # Examples
	///
	/// ```ignore
	/// let posts = Post::objects().select_related(&["author", "category"]).all().await?;
	/// ```
	pub fn select_related<I>(&self, fields: I) -> QuerySet<M>
	where
		I: RelationLoadInput<M>,
	{
		QuerySet::new().select_related(fields)
	}

	/// Set OFFSET clause
	///
	/// Skips the specified number of records before returning results.
	/// Corresponds to Django's QuerySet slicing `[offset:]`.
	///
	/// # Examples
	///
	/// ```ignore
	/// let users = User::objects().offset(20).all().await?;
	/// ```
	pub fn offset(&self, offset: usize) -> QuerySet<M> {
		QuerySet::new().offset(offset)
	}

	/// Paginate results (LIMIT + OFFSET)
	///
	/// Convenience method that combines LIMIT and OFFSET for pagination.
	/// Corresponds to Django's Paginator.
	///
	/// # Examples
	///
	/// ```ignore
	/// let users = User::objects().paginate(3, 10).all().await?;  // page 3, 10 items per page
	/// ```
	pub fn paginate(&self, page: usize, page_size: usize) -> QuerySet<M> {
		QuerySet::new().paginate(page, page_size)
	}

	/// Prefetch related objects using separate queries
	///
	/// Performs separate queries to load related objects, reducing N+1 queries.
	/// Corresponds to Django's QuerySet.prefetch_related().
	///
	/// # Examples
	///
	/// ```ignore
	/// let posts = Post::objects().prefetch_related(&["comments", "tags"]).all().await?;
	/// ```
	pub fn prefetch_related<I>(&self, fields: I) -> QuerySet<M>
	where
		I: RelationLoadInput<M>,
	{
		QuerySet::new().prefetch_related(fields)
	}

	/// Select specific fields (values_list)
	///
	/// Alias for `values()`. Returns records with only the specified fields.
	/// Corresponds to Django's QuerySet.values_list().
	///
	/// # Examples
	///
	/// ```ignore
	/// let user_data = User::objects().values_list(&["id", "username"]).all().await?;
	/// ```
	pub fn values_list(&self, fields: &[&str]) -> QuerySet<M> {
		QuerySet::new().values_list(fields)
	}

	/// Filter by array overlap (PostgreSQL)
	///
	/// Filters rows where the array field overlaps with the provided values.
	/// Uses the `&&` operator in PostgreSQL.
	///
	/// # Examples
	///
	/// ```ignore
	/// let posts = Post::objects().filter_array_overlap("tags", &["rust", "web"]).all().await?;
	/// ```
	pub fn filter_array_overlap(&self, field: &str, values: &[&str]) -> QuerySet<M> {
		QuerySet::new().filter_array_overlap(field, values)
	}

	/// Filter by array contains (PostgreSQL)
	///
	/// Filters rows where the array field contains all provided values.
	/// Uses the `@>` operator in PostgreSQL.
	///
	/// # Examples
	///
	/// ```ignore
	/// let posts = Post::objects().filter_array_contains("tags", &["rust", "web"]).all().await?;
	/// ```
	pub fn filter_array_contains(&self, field: &str, values: &[&str]) -> QuerySet<M> {
		QuerySet::new().filter_array_contains(field, values)
	}

	/// Filter by JSONB contains (PostgreSQL)
	///
	/// Filters rows where the JSONB field contains the provided JSON.
	/// Uses the `@>` operator in PostgreSQL.
	///
	/// # Examples
	///
	/// ```ignore
	/// let users = User::objects().filter_jsonb_contains("metadata", r#"{"role": "admin"}"#).all().await?;
	/// ```
	pub fn filter_jsonb_contains(&self, field: &str, json: &str) -> QuerySet<M> {
		QuerySet::new().filter_jsonb_contains(field, json)
	}

	/// Filter by JSONB key exists (PostgreSQL)
	///
	/// Filters rows where the JSONB field has the specified key.
	/// Uses the `?` operator in PostgreSQL.
	///
	/// # Examples
	///
	/// ```ignore
	/// let users = User::objects().filter_jsonb_key_exists("metadata", "email").all().await?;
	/// ```
	pub fn filter_jsonb_key_exists(&self, field: &str, key: &str) -> QuerySet<M> {
		QuerySet::new().filter_jsonb_key_exists(field, key)
	}

	/// Filter by range contains (PostgreSQL)
	///
	/// Filters rows where the range field contains the provided value.
	/// Uses the `@>` operator in PostgreSQL.
	///
	/// # Examples
	///
	/// ```ignore
	/// let events = Event::objects().filter_range_contains("date_range", "2024-01-15").all().await?;
	/// ```
	pub fn filter_range_contains(&self, field: &str, value: &str) -> QuerySet<M> {
		QuerySet::new().filter_range_contains(field, value)
	}

	/// Filter by IN subquery
	///
	/// Filters rows where the field value is in the result of a subquery.
	///
	/// # Examples
	///
	/// ```ignore
	/// let authors = Author::objects()
	///     .filter_in_subquery("id", |subq: QuerySet<Book>| {
	///         subq.filter(Filter::new("price", FilterOperator::Gt, FilterValue::Int(1500)))
	///             .values(&["author_id"])
	///     })?
	///     .all()
	///     .await?;
	/// ```
	pub fn filter_in_subquery<R: super::Model, F>(
		&self,
		field: &str,
		subquery_fn: F,
	) -> reinhardt_core::exception::Result<QuerySet<M>>
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		QuerySet::new().filter_in_subquery(field, subquery_fn)
	}

	/// Filter by NOT IN subquery
	///
	/// Filters rows where the field value is not in the result of a subquery.
	///
	/// # Examples
	///
	/// ```ignore
	/// let authors = Author::objects()
	///     .filter_not_in_subquery("id", |subq: QuerySet<Book>| {
	///         subq.filter(Filter::new("status", FilterOperator::Eq, FilterValue::String("archived".into())))
	///             .values(&["author_id"])
	///     })?
	///     .all()
	///     .await?;
	/// ```
	pub fn filter_not_in_subquery<R: super::Model, F>(
		&self,
		field: &str,
		subquery_fn: F,
	) -> reinhardt_core::exception::Result<QuerySet<M>>
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		QuerySet::new().filter_not_in_subquery(field, subquery_fn)
	}

	/// Filter by EXISTS subquery
	///
	/// Filters rows where the subquery returns at least one row.
	///
	/// # Examples
	///
	/// ```ignore
	/// let authors = Author::objects()
	///     .filter_exists(|subq: QuerySet<Book>| {
	///         subq.filter(Filter::new("author_id", FilterOperator::Eq, FilterValue::FieldRef(F::new("authors.id"))))
	///     })?
	///     .all()
	///     .await?;
	/// ```
	pub fn filter_exists<R: super::Model, F>(
		&self,
		subquery_fn: F,
	) -> reinhardt_core::exception::Result<QuerySet<M>>
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		QuerySet::new().filter_exists(subquery_fn)
	}

	/// Filter by NOT EXISTS subquery
	///
	/// Filters rows where the subquery returns no rows.
	///
	/// # Examples
	///
	/// ```ignore
	/// let authors = Author::objects()
	///     .filter_not_exists(|subq: QuerySet<Book>| {
	///         subq.filter(Filter::new("author_id", FilterOperator::Eq, FilterValue::FieldRef(F::new("authors.id"))))
	///     })?
	///     .all()
	///     .await?;
	/// ```
	pub fn filter_not_exists<R: super::Model, F>(
		&self,
		subquery_fn: F,
	) -> reinhardt_core::exception::Result<QuerySet<M>>
	where
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		QuerySet::new().filter_not_exists(subquery_fn)
	}

	/// Add Common Table Expression (WITH clause)
	///
	/// Adds a CTE that can be referenced in the main query.
	///
	/// # Examples
	///
	/// ```ignore
	/// let high_earners = CTE::new("high_earners", "SELECT * FROM employees WHERE salary > 100000");
	/// let results = Employee::objects()
	///     .with_cte(high_earners)
	///     .all()
	///     .await?;
	/// ```
	pub fn with_cte(&self, cte: super::cte::CTE) -> QuerySet<M> {
		QuerySet::new().with_cte(cte)
	}

	/// Full-text search (PostgreSQL)
	///
	/// Filters rows using PostgreSQL's full-text search.
	///
	/// # Examples
	///
	/// ```ignore
	/// let articles = Article::objects()
	///     .full_text_search("search_vector", "rust programming")
	///     .all()
	///     .await?;
	/// ```
	pub fn full_text_search(&self, field: &str, query: &str) -> QuerySet<M> {
		QuerySet::new().full_text_search(field, query)
	}

	/// Annotate with subquery
	///
	/// Adds a scalar subquery to the SELECT clause.
	///
	/// # Examples
	///
	/// ```ignore
	/// let authors = Author::objects()
	///     .annotate_subquery::<Book, _>("book_count", |subq| {
	///         subq.filter("author_id", FilterOperator::Eq, FilterValue::OuterRef(OuterRef::new("authors.id")))
	///             .values(&["COUNT(*)"])
	///     })?
	///     .all()
	///     .await?;
	/// ```
	pub fn annotate_subquery<R, F>(
		&self,
		name: &str,
		builder: F,
	) -> reinhardt_core::exception::Result<QuerySet<M>>
	where
		R: super::Model + 'static,
		F: FnOnce(QuerySet<R>) -> QuerySet<R>,
	{
		QuerySet::new().annotate_subquery(name, builder)
	}

	/// Get a record by composite primary key
	///
	/// Retrieves a single object using all fields of a composite primary key.
	///
	/// # Examples
	///
	/// ```ignore
	/// let mut pk_values = HashMap::new();
	/// pk_values.insert("post_id".to_string(), PkValue::Int(1));
	/// pk_values.insert("tag_id".to_string(), PkValue::Int(5));
	/// let post_tag = PostTag::objects().get_composite(&pk_values).await?;
	/// ```
	pub async fn get_composite(
		&self,
		pk_values: &std::collections::HashMap<String, super::composite_pk::PkValue>,
	) -> reinhardt_core::exception::Result<M>
	where
		M: Clone + serde::de::DeserializeOwned,
	{
		QuerySet::new().get_composite(pk_values).await
	}

	/// Create a new record using reinhardt-query for SQL injection protection
	pub async fn create(&self, model: &M) -> reinhardt_core::exception::Result<M> {
		let mut conn = get_connection().await?;
		self.create_with_conn(&mut conn, model).await
	}

	async fn create_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
		model: &M,
	) -> Result<M, crate::backends::error::DatabaseError> {
		let obj = model
			.encode_database_fields()
			.map_err(executor_field_codec_error)?;
		let mut stmt =
			Self::build_insert_statement_from_object(&obj, |field| model.field_is_none(field))
				.map_err(executor_error)?;
		let backend = Self::executor_backend(executor);

		if backend != DatabaseBackend::MySql {
			stmt.returning(Self::returning_columns_from_object(&obj));
		}
		let (sql, values) = build_insert_sql(&stmt, backend);
		let params = values
			.0
			.into_iter()
			.map(Self::sea_value_to_query_value)
			.collect();

		if backend == DatabaseBackend::MySql {
			let explicit_primary_key = obj
				.get(M::primary_key_field())
				.filter(|value| {
					!matches!(
						value,
						DatabaseValue::Null | DatabaseValue::I32(0) | DatabaseValue::I64(0)
					)
				})
				.cloned();
			if explicit_primary_key.is_none() {
				executor
					.fetch_one("SELECT LAST_INSERT_ID(0) AS generated_id", Vec::new())
					.await
					.map_err(executor_error)?;
			}
			executor
				.execute(&sql, params)
				.await
				.map_err(executor_error)?;

			let primary_key_value = if let Some(primary_key) = explicit_primary_key {
				database_value_to_query_value(primary_key)
			} else {
				let row = executor
					.fetch_one(
						"SELECT CAST(LAST_INSERT_ID() AS SIGNED) AS generated_id",
						Vec::new(),
					)
					.await
					.map_err(executor_error)?;
				let generated_id = row.get::<i64>("generated_id")?;
				if generated_id <= 0 {
					return Err(crate::backends::error::DatabaseError::new(
						crate::backends::error::DatabaseErrorKind::Unsupported,
						"MySQL executor inserts without an explicit primary key require an auto-increment integer primary key",
					));
				}
				reinhardt_query::value::Value::BigInt(Some(generated_id))
			};

			let mut select = Query::select();
			select.from(Alias::new(M::table_name()));
			select.column(ColumnRef::Asterisk);
			let field_metadata = M::field_metadata();
			let primary_key_column = Self::field_column(&field_metadata, M::primary_key_field());
			select.and_where(Expr::col(Alias::new(primary_key_column)).eq(primary_key_value));
			let (select_sql, select_values) = build_select_sql(&select, backend);
			let select_params = select_values
				.0
				.into_iter()
				.map(Self::sea_value_to_query_value)
				.collect();
			let row = executor
				.fetch_one(&select_sql, select_params)
				.await
				.map_err(executor_error)?;
			return Self::decode_executor_row(row);
		}

		let row = executor
			.fetch_one(&sql, params)
			.await
			.map_err(executor_error)?;
		Self::decode_executor_row(row)
	}

	/// Insert a model through a caller-owned transaction executor, regardless of
	/// whether its primary key is already populated.
	pub async fn insert_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
		model: &M,
	) -> Result<M, crate::backends::error::DatabaseError> {
		self.create_with_executor(executor, model).await
	}

	/// Save a model through a caller-owned transaction executor.
	pub async fn save_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
		model: &M,
	) -> Result<M, crate::backends::error::DatabaseError> {
		if model.primary_key().is_some() {
			self.update_with_executor(executor, model).await
		} else {
			self.create_with_executor(executor, model).await
		}
	}

	/// Create a new record through a caller-owned ORM executor.
	///
	/// Pass the transaction supplied by [`DatabaseConnection::atomic`] when the
	/// write must participate in a closure-scoped transaction.
	///
	/// # Arguments
	///
	/// * `conn` - The mutable ORM executor to use
	/// * `model` - The model to create
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::{Manager, Model};
	/// # async fn example<M: Model>(manager: Manager<M>, model: &M) -> reinhardt_core::exception::Result<()> {
	/// use reinhardt_db::orm::manager::get_connection;
	///
	/// let conn = get_connection().await?;
	/// let _created = conn
	///     .atomic(async |transaction| {
	///         manager.create_with_conn(transaction, model).await
	///     })
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn create_with_conn<E>(
		&self,
		conn: &mut E,
		model: &M,
	) -> reinhardt_core::exception::Result<M>
	where
		E: OrmExecutor,
	{
		let obj = model.encode_database_fields().map_err(field_codec_error)?;
		let mut stmt =
			Self::build_insert_statement_from_object(&obj, |field| model.field_is_none(field))?;
		let backend = conn.backend();
		if backend != DatabaseBackend::MySql {
			stmt.returning(Self::returning_columns_from_object(&obj));
		}
		let (sql, values) = build_insert_sql(&stmt, backend);
		let params = values
			.0
			.into_iter()
			.map(Self::sea_value_to_query_value)
			.collect();

		if backend == DatabaseBackend::MySql {
			let explicit_primary_key = obj
				.get(M::primary_key_field())
				.filter(|value| {
					!matches!(
						value,
						DatabaseValue::Null | DatabaseValue::I32(0) | DatabaseValue::I64(0)
					)
				})
				.cloned();
			let result = conn.execute(&sql, params).await?;
			let primary_key = explicit_primary_key
				.map(database_value_to_query_value)
				.or_else(|| {
					result
						.last_insert_id
						.and_then(|id| i64::try_from(id).ok())
						.filter(|id| *id > 0)
						.map(|id| reinhardt_query::value::Value::BigInt(Some(id)))
				})
				.ok_or_else(|| {
					Error::from(DatabaseError::new(
						DatabaseErrorKind::Unsupported,
						"MySQL insert did not return a generated primary key",
					))
				})?;
			let field_metadata = M::field_metadata();
			let primary_key_column = Self::field_column(&field_metadata, M::primary_key_field());
			let mut select = Query::select();
			select
				.from(Alias::new(M::table_name()))
				.column(ColumnRef::Asterisk)
				.and_where(Expr::col(Alias::new(primary_key_column)).eq(primary_key));
			let (select_sql, select_values) = build_select_sql(&select, backend);
			let select_params = select_values
				.0
				.into_iter()
				.map(Self::sea_value_to_query_value)
				.collect();
			let row = conn.fetch_one(&select_sql, select_params).await?;
			return QueryRow::from_backend_row(row)
				.deserialize_model::<M>()
				.map_err(field_codec_error);
		}

		let row = conn.fetch_one(&sql, params).await?;
		QueryRow::from_backend_row(row)
			.deserialize_model::<M>()
			.map_err(field_codec_error)
	}

	pub(crate) fn json_to_sea_value_for_field(
		value: &serde_json::Value,
		field_info: Option<&FieldInfo>,
		field_is_none: bool,
	) -> reinhardt_query::value::Value {
		if field_info
			.map(|field| super::json::is_json_field_type(&field.field_type))
			.unwrap_or(false)
		{
			if field_is_none {
				reinhardt_query::value::Value::Json(None)
			} else {
				reinhardt_query::value::Value::Json(Some(Box::new(value.clone())))
			}
		} else {
			Self::json_to_sea_value(value)
		}
	}

	/// Convert serde_json::Value to reinhardt_query::value::Value for parameter binding
	pub(crate) fn json_to_sea_value(v: &serde_json::Value) -> reinhardt_query::value::Value {
		match v {
			serde_json::Value::Null => reinhardt_query::value::Value::Int(None),
			serde_json::Value::Bool(b) => reinhardt_query::value::Value::Bool(Some(*b)),
			serde_json::Value::Number(n) => {
				if let Some(i) = n.as_i64() {
					reinhardt_query::value::Value::BigInt(Some(i))
				} else if let Some(f) = n.as_f64() {
					reinhardt_query::value::Value::Double(Some(f))
				} else {
					reinhardt_query::value::Value::Int(None)
				}
			}
			serde_json::Value::String(s) => {
				// 1. Try to parse as UUID (format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)
				//    UUIDs are often serialized as strings via serde
				if let Ok(uuid) = Uuid::parse_str(s) {
					return reinhardt_query::value::Value::Uuid(Some(Box::new(uuid)));
				}

				// 2. Try to parse as ISO 8601 datetime (chrono::DateTime<Utc>)
				// This handles timestamps serialized by serde_json from chrono::DateTime

				// 2.1 Try RFC3339 strict format first (e.g., "2024-01-01T00:00:00+00:00")
				if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
					return reinhardt_query::value::Value::ChronoDateTimeUtc(Some(Box::new(
						dt.with_timezone(&chrono::Utc),
					)));
				}

				// 2.2 Try chrono's FromStr trait for DateTime<Utc>
				//    This handles formats like "2024-01-01T00:00:00Z" with optional subseconds
				if let Ok(dt) = s.parse::<chrono::DateTime<chrono::Utc>>() {
					return reinhardt_query::value::Value::ChronoDateTimeUtc(Some(Box::new(dt)));
				}

				// 2.3 Try parsing with FixedOffset timezone then convert to UTC
				//    Handles formats like "2024-01-01T00:00:00.123456789+00:00"
				if let Ok(dt) = s.parse::<chrono::DateTime<chrono::FixedOffset>>() {
					return reinhardt_query::value::Value::ChronoDateTimeUtc(Some(Box::new(
						dt.with_timezone(&chrono::Utc),
					)));
				}

				// Fallback: treat as regular string (non-datetime, non-UUID values)
				reinhardt_query::value::Value::String(Some(Box::new(s.clone())))
			}
			serde_json::Value::Array(arr) => {
				// Convert JSON array to reinhardt_query::value::Value array
				// Array(ArrayType, Option<Box<Vec<Value>>>)
				let values: Vec<reinhardt_query::value::Value> =
					arr.iter().map(|v| Self::json_to_sea_value(v)).collect();
				reinhardt_query::value::Value::Array(
					reinhardt_query::value::ArrayType::String,
					Some(Box::new(values)),
				)
			}
			serde_json::Value::Object(_obj) => {
				// Use reinhardt-query's Json type for PostgreSQL JSONB/JSON columns
				// Json expects Box<serde_json::Value>
				reinhardt_query::value::Value::Json(Some(Box::new(v.clone())))
			}
		}
	}

	/// Convert reinhardt_query::value::Value to QueryValue for database parameter binding
	pub(crate) fn sea_value_to_query_value(
		v: reinhardt_query::value::Value,
	) -> super::connection::QueryValue {
		use super::connection::QueryValue;

		match v {
			reinhardt_query::value::Value::Bool(Some(b)) => QueryValue::Bool(b),
			reinhardt_query::value::Value::Bool(None) => QueryValue::Null,

			reinhardt_query::value::Value::TinyInt(Some(i)) => QueryValue::Int(i as i64),
			reinhardt_query::value::Value::TinyInt(None) => QueryValue::Null,
			reinhardt_query::value::Value::SmallInt(Some(i)) => QueryValue::Int(i as i64),
			reinhardt_query::value::Value::SmallInt(None) => QueryValue::Null,
			reinhardt_query::value::Value::Int(Some(i)) => QueryValue::Int(i as i64),
			reinhardt_query::value::Value::Int(None) => QueryValue::Null,
			reinhardt_query::value::Value::BigInt(Some(i)) => QueryValue::Int(i),
			reinhardt_query::value::Value::BigInt(None) => QueryValue::Null,

			reinhardt_query::value::Value::TinyUnsigned(Some(u)) => QueryValue::Int(u as i64),
			reinhardt_query::value::Value::TinyUnsigned(None) => QueryValue::Null,
			reinhardt_query::value::Value::SmallUnsigned(Some(u)) => QueryValue::Int(u as i64),
			reinhardt_query::value::Value::SmallUnsigned(None) => QueryValue::Null,
			reinhardt_query::value::Value::Unsigned(Some(u)) => QueryValue::Int(u as i64),
			reinhardt_query::value::Value::Unsigned(None) => QueryValue::Null,
			reinhardt_query::value::Value::BigUnsigned(Some(u)) => QueryValue::Int(u as i64),
			reinhardt_query::value::Value::BigUnsigned(None) => QueryValue::Null,

			reinhardt_query::value::Value::Float(Some(f)) => QueryValue::Float(f as f64),
			reinhardt_query::value::Value::Float(None) => QueryValue::Null,
			reinhardt_query::value::Value::Double(Some(f)) => QueryValue::Float(f),
			reinhardt_query::value::Value::Double(None) => QueryValue::Null,
			// QueryValue has no dedicated decimal variant. Preserve the exact
			// decimal spelling as text so the backend can coerce it to DECIMAL
			// without losing precision through an intermediate float.
			reinhardt_query::value::Value::Decimal(Some(value)) => {
				QueryValue::String(value.to_string())
			}
			reinhardt_query::value::Value::Decimal(None) => QueryValue::Null,

			reinhardt_query::value::Value::String(Some(s)) => QueryValue::String((*s).clone()),
			reinhardt_query::value::Value::String(None) => QueryValue::Null,

			reinhardt_query::value::Value::Bytes(Some(b)) => QueryValue::Bytes((*b).clone()),
			reinhardt_query::value::Value::Bytes(None) => QueryValue::Null,

			// Timestamp handling
			// ChronoDateTime contains NaiveDateTime, convert to UTC
			reinhardt_query::value::Value::ChronoDateTime(Some(dt)) => {
				QueryValue::Timestamp(dt.and_utc())
			}
			reinhardt_query::value::Value::ChronoDateTime(None) => QueryValue::Null,
			reinhardt_query::value::Value::ChronoDateTimeUtc(Some(dt)) => {
				QueryValue::Timestamp(*dt)
			}
			reinhardt_query::value::Value::ChronoDateTimeUtc(None) => QueryValue::Null,
			reinhardt_query::value::Value::ChronoDate(Some(date)) => {
				QueryValue::String(date.to_string())
			}
			reinhardt_query::value::Value::ChronoDate(None) => QueryValue::Null,
			reinhardt_query::value::Value::ChronoTime(Some(time)) => {
				QueryValue::String(time.to_string())
			}
			reinhardt_query::value::Value::ChronoTime(None) => QueryValue::Null,

			// UUID handling
			reinhardt_query::value::Value::Uuid(Some(u)) => QueryValue::Uuid(*u),
			reinhardt_query::value::Value::Uuid(None) => QueryValue::Null,

			// JSON types - serialize to string
			reinhardt_query::value::Value::Json(json) => QueryValue::Json(json),
			reinhardt_query::value::Value::Array(array_type, Some(values)) => {
				use reinhardt_query::value::Value as SeaValue;

				match array_type {
					reinhardt_query::value::ArrayType::String => QueryValue::StringArray(
						values
							.iter()
							.filter_map(|value| match value {
								SeaValue::String(Some(value)) => Some((**value).clone()),
								_ => None,
							})
							.collect(),
					),
					reinhardt_query::value::ArrayType::Int => QueryValue::IntArray(
						values
							.iter()
							.filter_map(|value| match value {
								SeaValue::Int(Some(value)) => Some(*value),
								_ => None,
							})
							.collect(),
					),
					reinhardt_query::value::ArrayType::BigInt => QueryValue::BigIntArray(
						values
							.iter()
							.filter_map(|value| match value {
								SeaValue::BigInt(Some(value)) => Some(*value),
								_ => None,
							})
							.collect(),
					),
					reinhardt_query::value::ArrayType::Bool => QueryValue::BoolArray(
						values
							.iter()
							.filter_map(|value| match value {
								SeaValue::Bool(Some(value)) => Some(*value),
								_ => None,
							})
							.collect(),
					),
					reinhardt_query::value::ArrayType::Float => QueryValue::FloatArray(
						values
							.iter()
							.filter_map(|value| match value {
								SeaValue::Float(Some(value)) => Some(*value),
								_ => None,
							})
							.collect(),
					),
					reinhardt_query::value::ArrayType::Double => QueryValue::DoubleArray(
						values
							.iter()
							.filter_map(|value| match value {
								SeaValue::Double(Some(value)) => Some(*value),
								_ => None,
							})
							.collect(),
					),
					reinhardt_query::value::ArrayType::Uuid => QueryValue::UuidArray(
						values
							.iter()
							.filter_map(|value| match value {
								SeaValue::Uuid(Some(value)) => Some(**value),
								_ => None,
							})
							.collect(),
					),
					_ => QueryValue::Json(Some(Box::new(super::execution::array_values_to_json(
						&values,
					)))),
				}
			}
			reinhardt_query::value::Value::Array(_, None) => QueryValue::Null,

			// For complex types or unsupported types, convert to null
			// This is a safe fallback that won't cause runtime errors
			_ => QueryValue::Null,
		}
	}

	fn query_value_to_sea_value(value: QueryValue) -> reinhardt_query::value::Value {
		match value {
			QueryValue::Null => reinhardt_query::value::Value::Int(None),
			QueryValue::Bool(value) => reinhardt_query::value::Value::Bool(Some(value)),
			QueryValue::Int(value) => reinhardt_query::value::Value::BigInt(Some(value)),
			QueryValue::Float(value) => reinhardt_query::value::Value::Double(Some(value)),
			QueryValue::String(value) => {
				reinhardt_query::value::Value::String(Some(Box::new(value)))
			}
			QueryValue::Bytes(value) => reinhardt_query::value::Value::Bytes(Some(Box::new(value))),
			QueryValue::Timestamp(value) => {
				reinhardt_query::value::Value::ChronoDateTimeUtc(Some(Box::new(value)))
			}
			QueryValue::Uuid(value) => reinhardt_query::value::Value::Uuid(Some(Box::new(value))),
			QueryValue::Json(value) => reinhardt_query::value::Value::Json(value),
			QueryValue::Vector(values) => {
				reinhardt_query::value::Value::Vector(Some(Box::new(values)))
			}
			QueryValue::StringArray(values) => {
				reinhardt_query::value::Value::Json(Some(Box::new(serde_json::Value::Array(
					values.into_iter().map(serde_json::Value::String).collect(),
				))))
			}
			QueryValue::IntArray(values) => reinhardt_query::value::Value::Json(Some(Box::new(
				serde_json::Value::Array(values.into_iter().map(serde_json::Value::from).collect()),
			))),
			QueryValue::BigIntArray(values) => reinhardt_query::value::Value::Json(Some(Box::new(
				serde_json::Value::Array(values.into_iter().map(serde_json::Value::from).collect()),
			))),
			QueryValue::BoolArray(values) => reinhardt_query::value::Value::Json(Some(Box::new(
				serde_json::Value::Array(values.into_iter().map(serde_json::Value::from).collect()),
			))),
			QueryValue::FloatArray(values) => reinhardt_query::value::Value::Json(Some(Box::new(
				serde_json::Value::Array(values.into_iter().map(serde_json::Value::from).collect()),
			))),
			QueryValue::DoubleArray(values) => reinhardt_query::value::Value::Json(Some(Box::new(
				serde_json::Value::Array(values.into_iter().map(serde_json::Value::from).collect()),
			))),
			QueryValue::UuidArray(values) => {
				reinhardt_query::value::Value::Json(Some(Box::new(serde_json::Value::Array(
					values
						.into_iter()
						.map(|value| serde_json::Value::String(value.to_string()))
						.collect(),
				))))
			}
			QueryValue::Now => reinhardt_query::value::Value::Int(None),
		}
	}

	/// Serialize a JSON value to SQL-compatible string representation
	// Allow dead_code: internal helper for JSON-to-SQL serialization in manager operations
	#[allow(dead_code)]
	fn serialize_value(v: &serde_json::Value) -> String {
		match v {
			serde_json::Value::Null => "NULL".to_string(),
			serde_json::Value::Bool(b) => b.to_string().to_uppercase(),
			serde_json::Value::Number(n) => n.to_string(),
			serde_json::Value::String(s) => {
				// Escape single quotes and wrap in quotes
				format!("'{}'", s.replace('\'', "''"))
			}
			serde_json::Value::Array(arr) => {
				// Convert to PostgreSQL array syntax: ARRAY['a', 'b', 'c']
				let items: Vec<String> = arr.iter().map(Self::serialize_value).collect();
				format!("ARRAY[{}]", items.join(", "))
			}
			serde_json::Value::Object(obj) => {
				// Convert to JSON string for JSONB columns
				let json_str = serde_json::to_string(obj).unwrap_or_else(|_| "{}".to_string());
				format!("'{}'::jsonb", json_str.replace('\'', "''"))
			}
		}
	}

	/// Update an existing record using reinhardt-query for SQL injection protection
	pub async fn update(&self, model: &M) -> reinhardt_core::exception::Result<M> {
		let mut conn = get_connection().await?;
		self.update_with_conn(&mut conn, model).await
	}

	async fn update_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
		model: &M,
	) -> Result<M, crate::backends::error::DatabaseError> {
		let pk = model.primary_key().ok_or_else(|| {
			crate::backends::error::DatabaseError::new(
				crate::backends::error::DatabaseErrorKind::Query,
				"Model must have primary key",
			)
		})?;
		let obj = model
			.encode_database_fields()
			.map_err(executor_field_codec_error)?;
		let backend = Self::executor_backend(executor);
		let stmt = Self::build_update_statement_from_object_with_returning(
			&obj,
			backend != DatabaseBackend::MySql,
		)
		.map_err(executor_field_codec_error)?;
		let context = super::execution::pgvector_context_for_update(&stmt);
		let (sql, values) = build_update_sql(&stmt, backend);
		let params = values
			.0
			.into_iter()
			.map(Self::sea_value_to_query_value)
			.collect();

		if backend == DatabaseBackend::MySql {
			executor
				.execute_with_context(&sql, params, context)
				.await
				.map_err(executor_error)?;
			let mut select = Query::select();
			select.from(Alias::new(M::table_name()));
			select.column(ColumnRef::Asterisk);
			let field_metadata = M::field_metadata();
			let primary_key_column = Self::field_column(&field_metadata, M::primary_key_field());
			select.and_where(
				Expr::col(Alias::new(primary_key_column)).eq(Self::primary_key_query_value(&pk)),
			);
			let (select_sql, select_values) = build_select_sql(&select, backend);
			let select_params = select_values
				.0
				.into_iter()
				.map(Self::sea_value_to_query_value)
				.collect();
			let row = executor
				.fetch_one(&select_sql, select_params)
				.await
				.map_err(executor_error)?;
			return Self::decode_executor_row(row);
		}

		let row = executor
			.fetch_one_with_context(&sql, params, context)
			.await
			.map_err(executor_error)?;
		Self::decode_executor_row(row)
	}

	/// Update an existing record through a caller-owned ORM executor.
	///
	/// Pass the transaction supplied by [`DatabaseConnection::atomic`] when the
	/// write must participate in a closure-scoped transaction.
	///
	/// # Arguments
	///
	/// * `conn` - The mutable ORM executor to use
	/// * `model` - The model to update (must have primary key set)
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::{Manager, Model};
	/// # async fn example<M: Model>(manager: Manager<M>, model: &M) -> reinhardt_core::exception::Result<()> {
	/// use reinhardt_db::orm::manager::get_connection;
	///
	/// let conn = get_connection().await?;
	/// let _updated = conn
	///     .atomic(async |transaction| {
	///         manager.update_with_conn(transaction, model).await
	///     })
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn update_with_conn<E>(
		&self,
		conn: &mut E,
		model: &M,
	) -> reinhardt_core::exception::Result<M>
	where
		E: OrmExecutor,
	{
		model.primary_key().ok_or_else(|| {
			Error::from(DatabaseError::new(
				DatabaseErrorKind::Query,
				"Model must have primary key",
			))
		})?;

		let obj = model.encode_database_fields().map_err(field_codec_error)?;
		let backend = conn.backend();
		let stmt = if backend == DatabaseBackend::MySql {
			Self::build_update_statement_from_object_with_returning(&obj, false)
		} else {
			Self::build_update_statement_from_object(&obj, |field| model.field_is_none(field))
		}
		.map_err(field_codec_error)?;

		let context = super::execution::pgvector_context_for_update(&stmt);
		let (sql, values) = build_update_sql(&stmt, backend);
		let values: Vec<_> = values
			.0
			.into_iter()
			.map(Self::sea_value_to_query_value)
			.collect();

		let pk = model.primary_key().ok_or_else(|| {
			Error::from(DatabaseError::new(
				DatabaseErrorKind::Query,
				"Model must have primary key",
			))
		})?;
		if backend == DatabaseBackend::MySql {
			conn.execute_with_context(&sql, values, context).await?;
			let field_metadata = M::field_metadata();
			let primary_key_column = Self::field_column(&field_metadata, M::primary_key_field());
			let mut select = Query::select();
			select
				.from(Alias::new(M::table_name()))
				.column(ColumnRef::Asterisk)
				.and_where(
					Expr::col(Alias::new(primary_key_column))
						.eq(Self::primary_key_query_value(&pk)),
				);
			let (select_sql, select_values) = build_select_sql(&select, backend);
			let select_params = select_values
				.0
				.into_iter()
				.map(Self::sea_value_to_query_value)
				.collect();
			let row = conn.fetch_one(&select_sql, select_params).await?;
			return QueryRow::from_backend_row(row)
				.deserialize_model::<M>()
				.map_err(field_codec_error);
		}

		let row = conn.fetch_one_with_context(&sql, values, context).await?;
		QueryRow::from_backend_row(row)
			.deserialize_model::<M>()
			.map_err(field_codec_error)
	}

	/// Delete a record using reinhardt-query for SQL injection protection
	pub async fn delete(&self, pk: M::PrimaryKey) -> reinhardt_core::exception::Result<()> {
		let mut conn = get_connection().await?;
		self.delete_with_conn(&mut conn, pk).await
	}

	/// Delete a model by primary key through a caller-owned transaction executor.
	pub async fn delete_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
		pk: M::PrimaryKey,
	) -> Result<(), crate::backends::error::DatabaseError> {
		let mut stmt = Query::delete();
		let field_metadata = M::field_metadata();
		let primary_key_column = Self::field_column(&field_metadata, M::primary_key_field());
		stmt.from_table(Alias::new(M::table_name())).and_where(
			Expr::col(Alias::new(primary_key_column)).eq(Self::primary_key_query_value(&pk)),
		);
		let (sql, values) = build_delete_sql(&stmt, Self::executor_backend(executor));
		let params = values
			.0
			.into_iter()
			.map(Self::sea_value_to_query_value)
			.collect();
		executor
			.execute(&sql, params)
			.await
			.map_err(executor_error)?;
		Ok(())
	}

	/// Delete a record through a caller-owned ORM executor.
	///
	/// Pass the transaction supplied by [`DatabaseConnection::atomic`] when the
	/// deletion must participate in a closure-scoped transaction.
	///
	/// # Arguments
	///
	/// * `conn` - The mutable ORM executor to use
	/// * `pk` - The primary key of the record to delete
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::{Manager, Model};
	/// # async fn example<M: Model>(manager: Manager<M>, pk: M::PrimaryKey) -> reinhardt_core::exception::Result<()> {
	/// use reinhardt_db::orm::manager::get_connection;
	///
	/// let conn = get_connection().await?;
	/// conn.atomic(async |transaction| {
	///     manager.delete_with_conn(transaction, pk).await
	/// })
	/// .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn delete_with_conn<E>(
		&self,
		conn: &mut E,
		pk: M::PrimaryKey,
	) -> reinhardt_core::exception::Result<()>
	where
		E: OrmExecutor,
	{
		// Build reinhardt-query DELETE statement
		let mut stmt = Query::delete();

		let pk_value = Self::primary_key_query_value(&pk);
		let field_metadata = M::field_metadata();
		let primary_key_column = Self::field_column(&field_metadata, M::primary_key_field());

		stmt.from_table(Alias::new(M::table_name()))
			.and_where(Expr::col(Alias::new(primary_key_column)).eq(pk_value));

		let (sql, values) = build_delete_sql(&stmt, conn.backend());
		let values: Vec<_> = values
			.0
			.into_iter()
			.map(Self::sea_value_to_query_value)
			.collect();

		conn.execute(&sql, values).await?;
		Ok(())
	}

	/// Count records using reinhardt-query
	pub async fn count(&self) -> reinhardt_core::exception::Result<i64> {
		let mut conn = get_connection().await?;
		self.count_with_conn(&mut conn).await
	}

	/// Count records through a caller-owned ORM executor.
	///
	/// Pass the transaction supplied by [`DatabaseConnection::atomic`] to observe
	/// writes that have not yet been committed.
	///
	/// # Arguments
	///
	/// * `conn` - The mutable ORM executor to use
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::{Manager, Model};
	/// # async fn example<M: Model>(manager: Manager<M>) -> reinhardt_core::exception::Result<()> {
	/// use reinhardt_db::orm::manager::get_connection;
	///
	/// let conn = get_connection().await?;
	/// let _count = conn
	///     .atomic(async |transaction| manager.count_with_conn(transaction).await)
	///     .await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn count_with_conn<E>(&self, conn: &mut E) -> reinhardt_core::exception::Result<i64>
	where
		E: OrmExecutor,
	{
		// Build reinhardt-query SELECT COUNT(*) statement with explicit alias
		let stmt = Query::select()
			.from(Alias::new(M::table_name()))
			.expr_as(Func::count(Expr::asterisk().into()), Alias::new("count"))
			.to_owned();

		let (sql, values) = build_select_sql(&stmt, conn.backend());
		let values: Vec<_> = values
			.0
			.into_iter()
			.map(Self::sea_value_to_query_value)
			.collect();

		let row = QueryRow::from_backend_row(conn.fetch_one(&sql, values).await?);
		row.get::<i64>("count").ok_or_else(|| {
			Error::from(DatabaseError::new(
				DatabaseErrorKind::Query,
				"Failed to get count",
			))
		})
	}

	/// Bulk create multiple records using reinhardt-query (similar to Django's bulk_create())
	pub fn bulk_create_query(&self, models: &[M]) -> Option<InsertStatement> {
		self.try_bulk_create_query(models).ok().flatten()
	}

	fn try_bulk_create_query(
		&self,
		models: &[M],
	) -> Result<Option<InsertStatement>, FieldCodecError> {
		if models.is_empty() {
			return Ok(None);
		}

		let database_values: Vec<std::collections::BTreeMap<String, DatabaseValue>> = models
			.iter()
			.map(Model::encode_database_fields)
			.collect::<Result<_, _>>()?;

		if database_values.is_empty() {
			return Ok(None);
		}

		let first_obj = &database_values[0];

		let primary_key = M::primary_key_field();
		let field_names: Vec<String> = first_obj
			.iter()
			.filter_map(|(name, value)| {
				if Self::is_generated_field(name.as_str())
					|| (name == primary_key
						&& (matches!(value, DatabaseValue::Null)
							|| (M::primary_key_uses_zero_sentinel()
								&& matches!(value, DatabaseValue::I32(0) | DatabaseValue::I64(0)))))
				{
					None
				} else {
					Some(name.clone())
				}
			})
			.collect();
		let field_metadata = M::field_metadata();
		let fields: Vec<_> = field_names
			.iter()
			.map(|name| Alias::new(Self::field_column(&field_metadata, name)))
			.collect();
		if fields.is_empty() {
			return Ok(None);
		}

		// Build reinhardt-query INSERT statement
		let mut stmt = Query::insert();
		stmt.into_table(Alias::new(M::table_name())).columns(fields);

		// Add value rows for each model
		for obj in &database_values {
			let values: Vec<reinhardt_query::value::Value> = field_names
				.iter()
				.map(|field| {
					obj.get(field.as_str())
						.cloned()
						.map(database_value_to_query_value)
							// Use untyped NULL for missing fields
							.unwrap_or(reinhardt_query::value::Value::Int(None))
				})
				.collect();
			stmt.values_panic(values);
		}

		Ok(Some(stmt.to_owned()))
	}

	/// Generate bulk create SQL (convenience method)
	///
	/// # Arguments
	///
	/// * `models` - Models to insert
	/// * `backend` - Database backend to generate SQL for
	pub fn bulk_create_sql(&self, models: &[M], backend: DatabaseBackend) -> String {
		if let Some(stmt) = self.bulk_create_query(models) {
			insert_to_string(&stmt, backend)
		} else {
			String::new()
		}
	}

	/// Generate UPDATE query for QuerySet
	pub fn update_queryset(
		&self,
		queryset: &QuerySet<M>,
		updates: &[(&str, &str)],
	) -> reinhardt_core::exception::Result<(String, Vec<String>)> {
		use crate::orm::query::UpdateValue;
		use std::collections::HashMap;

		// Convert &[(&str, &str)] to HashMap<String, UpdateValue>
		let updates_map: HashMap<String, UpdateValue> = updates
			.iter()
			.map(|(key, value)| (key.to_string(), UpdateValue::String(value.to_string())))
			.collect();

		queryset.update_sql(&updates_map)
	}

	/// Generate DELETE query for QuerySet
	pub fn delete_queryset(
		&self,
		queryset: &QuerySet<M>,
	) -> reinhardt_core::exception::Result<(String, Vec<String>)> {
		queryset.delete_sql()
	}

	/// Get or create a record (Django's get_or_create)
	/// Returns (model, created) where created is true if a new record was created
	///
	/// Django equivalent:
	/// ```python
	/// obj, created = Model.objects.get_or_create(
	///     field1=value1,
	///     defaults={'field2': value2}
	/// )
	/// ```
	pub async fn get_or_create(
		&self,
		lookup_fields: HashMap<String, String>,
		defaults: Option<HashMap<String, String>>,
	) -> reinhardt_core::exception::Result<(M, bool)> {
		let conn = get_connection().await?;

		// Try to find existing record
		let (select_sql, _) = self.get_or_create_sql(
			&lookup_fields,
			&defaults.clone().unwrap_or_default(),
			conn.backend(),
		)?;

		if let Ok(Some(row)) = conn.query_optional(&select_sql, vec![]).await {
			// row.data is already serde_json::Value::Object so deserialize directly
			let model: M = row.deserialize_model().map_err(field_codec_error)?;
			return Ok((model, false));
		}

		// Record not found, create new one
		let mut all_fields = lookup_fields.clone();
		if let Some(defs) = defaults {
			all_fields.extend(defs);
		}

		let fields: Vec<String> = all_fields.keys().cloned().collect();
		let values: Vec<String> = all_fields.values().map(|v| format!("'{}'", v)).collect();

		let insert_sql = format!(
			"INSERT INTO {} ({}) VALUES ({}) RETURNING *",
			M::table_name(),
			fields.join(", "),
			values.join(", ")
		);

		let row = conn.query_one(&insert_sql, vec![]).await?;
		// row.data is already serde_json::Value::Object so deserialize directly
		let model: M = row.deserialize_model().map_err(field_codec_error)?;

		Ok((model, true))
	}

	/// Retrieve a record through a caller-owned executor, or insert it when missing.
	pub async fn get_or_create_with_conn<E>(
		&self,
		conn: &mut E,
		lookup_fields: HashMap<String, String>,
		defaults: Option<HashMap<String, String>>,
	) -> reinhardt_core::exception::Result<(M, bool)>
	where
		E: OrmExecutor,
	{
		let lookup_fields = Self::normalize_primary_key_aliases(lookup_fields)?;
		let defaults = Self::normalize_primary_key_aliases(defaults.unwrap_or_default())?;
		let mut insert_fields = lookup_fields.clone();
		insert_fields.extend(defaults.clone());
		let (select_stmt, mut insert_stmt) =
			self.get_or_create_queries_from_normalized_fields(&lookup_fields, &defaults);
		let (select_sql, select_values) = build_select_sql(&select_stmt, conn.backend());
		let select_params = select_values
			.0
			.into_iter()
			.map(Self::sea_value_to_query_value)
			.collect();

		if let Some(row) = conn.fetch_optional(&select_sql, select_params).await? {
			let model = QueryRow::from_backend_row(row)
				.deserialize_model::<M>()
				.map_err(field_codec_error)?;
			return Ok((model, false));
		}

		let backend = conn.backend();
		if backend != DatabaseBackend::MySql {
			insert_stmt.returning(Self::returning_columns_from_object(
				&insert_fields
					.iter()
					.map(|(key, value)| (key.clone(), DatabaseValue::String(value.clone())))
					.collect(),
			));
		}
		let (insert_sql, insert_values) = build_insert_sql(&insert_stmt, backend);
		let insert_params = insert_values
			.0
			.into_iter()
			.map(Self::sea_value_to_query_value)
			.collect();

		if backend == DatabaseBackend::MySql {
			let result = conn.execute(&insert_sql, insert_params).await?;
			let primary_key = insert_fields
				.get(M::primary_key_field())
				.map(|value| QueryValue::String(value.clone()))
				.or_else(|| {
					result
						.last_insert_id
						.and_then(|id| i64::try_from(id).ok())
						.map(QueryValue::Int)
				})
				.ok_or_else(|| {
					Error::from(DatabaseError::new(
						DatabaseErrorKind::Query,
						"MySQL get_or_create did not return a primary key",
					))
				})?;
			let field_metadata = M::field_metadata();
			let primary_key_column = Self::field_column(&field_metadata, M::primary_key_field());
			let mut reload = Query::select();
			reload
				.from(Alias::new(M::table_name()))
				.column(ColumnRef::Asterisk)
				.and_where(
					Expr::col(Alias::new(primary_key_column))
						.eq(Self::query_value_to_sea_value(primary_key)),
				);
			let (reload_sql, reload_values) = build_select_sql(&reload, backend);
			let reload_params = reload_values
				.0
				.into_iter()
				.map(Self::sea_value_to_query_value)
				.collect();
			let row = conn.fetch_one(&reload_sql, reload_params).await?;
			let model = QueryRow::from_backend_row(row)
				.deserialize_model::<M>()
				.map_err(field_codec_error)?;
			return Ok((model, true));
		}

		let row = conn.fetch_one(&insert_sql, insert_params).await?;
		let model = QueryRow::from_backend_row(row)
			.deserialize_model::<M>()
			.map_err(field_codec_error)?;
		Ok((model, true))
	}

	/// Bulk create multiple records efficiently (Django's bulk_create)
	/// Inserts multiple records in a single query for performance
	///
	/// Django equivalent:
	/// ```python
	/// Model.objects.bulk_create([
	///     Model(field1=value1),
	///     Model(field2=value2),
	/// ])
	/// ```
	///
	/// Options:
	/// - batch_size: Split into multiple batches if needed
	/// - ignore_conflicts: Skip records that would violate constraints
	/// - update_conflicts: Update existing records instead of failing
	pub async fn bulk_create(
		&self,
		models: Vec<M>,
		batch_size: Option<usize>,
		ignore_conflicts: bool,
		_update_conflicts: bool,
	) -> reinhardt_core::exception::Result<Vec<M>> {
		if models.is_empty() {
			return Ok(vec![]);
		}

		let conn = get_connection().await?;
		let batch_size = batch_size.unwrap_or(models.len());
		let mut results = Vec::new();

		for chunk in models.chunks(batch_size) {
			let Some(mut statement) = self
				.try_bulk_create_query(chunk)
				.map_err(field_codec_error)?
			else {
				continue;
			};
			if !ignore_conflicts {
				statement.returning_all();
			}
			let (sql, values) = build_insert_sql(&statement, conn.backend());
			let sql = if ignore_conflicts {
				match conn.backend() {
					DatabaseBackend::Postgres => format!("{sql} ON CONFLICT DO NOTHING"),
					DatabaseBackend::MySql => sql.replacen("INSERT INTO", "INSERT IGNORE INTO", 1),
					DatabaseBackend::Sqlite => {
						sql.replacen("INSERT INTO", "INSERT OR IGNORE INTO", 1)
					}
				}
			} else {
				sql
			};
			let values = values
				.0
				.into_iter()
				.map(Self::sea_value_to_query_value)
				.collect();

			if ignore_conflicts {
				conn.execute(&sql, values).await?;
				// Note: Can't get RETURNING with DO NOTHING, skip results
				// Return empty vec for ignored conflicts
			} else {
				let rows = conn.query(&sql, values).await?;
				for row in rows {
					let model: M = row.deserialize_model().map_err(field_codec_error)?;
					results.push(model);
				}
			}
		}

		Ok(results)
	}

	/// Bulk-insert models through a caller-owned executor.
	pub async fn bulk_create_with_conn<E>(
		&self,
		conn: &mut E,
		models: Vec<M>,
		batch_size: Option<usize>,
		ignore_conflicts: bool,
		_update_conflicts: bool,
	) -> reinhardt_core::exception::Result<Vec<M>>
	where
		E: OrmExecutor,
	{
		if models.is_empty() {
			return Ok(Vec::new());
		}

		let batch_size = batch_size.unwrap_or(models.len());
		let mut results = Vec::new();
		for chunk in models.chunks(batch_size) {
			let Some(mut statement) = self
				.try_bulk_create_query(chunk)
				.map_err(field_codec_error)?
			else {
				continue;
			};
			let backend = conn.backend();
			if !ignore_conflicts && backend != DatabaseBackend::MySql {
				statement.returning_all();
			}
			let (sql, values) = build_insert_sql(&statement, backend);
			let sql = if ignore_conflicts {
				match backend {
					DatabaseBackend::Postgres => format!("{sql} ON CONFLICT DO NOTHING"),
					DatabaseBackend::MySql => sql.replacen("INSERT INTO", "INSERT IGNORE INTO", 1),
					DatabaseBackend::Sqlite => {
						sql.replacen("INSERT INTO", "INSERT OR IGNORE INTO", 1)
					}
				}
			} else {
				sql
			};
			let values = values
				.0
				.into_iter()
				.map(Self::sea_value_to_query_value)
				.collect();

			if ignore_conflicts || backend == DatabaseBackend::MySql {
				conn.execute(&sql, values).await?;
				if !ignore_conflicts {
					results.extend(chunk.iter().cloned());
				}
				continue;
			}

			for row in conn.fetch_all(&sql, values).await? {
				results.push(
					QueryRow::from_backend_row(row)
						.deserialize_model::<M>()
						.map_err(field_codec_error)?,
				);
			}
		}
		Ok(results)
	}

	/// Bulk update multiple records efficiently (Django's bulk_update)
	/// Updates specified fields for multiple records in optimized queries
	///
	/// Django equivalent:
	/// ```python
	/// Model.objects.bulk_update(
	///     [obj1, obj2, obj3],
	///     ['field1', 'field2'],
	///     batch_size=100
	/// )
	/// ```
	pub async fn bulk_update(
		&self,
		models: Vec<M>,
		fields: Vec<String>,
		batch_size: Option<usize>,
	) -> reinhardt_core::exception::Result<usize> {
		if models.is_empty() || fields.is_empty() {
			return Ok(0);
		}

		let conn = get_connection().await?;
		let batch_size = batch_size.unwrap_or(models.len());
		let mut total_updated = 0;

		for chunk in models.chunks(batch_size) {
			// Build updates structure
			let updates: Vec<(DatabaseValue, HashMap<String, DatabaseValue>)> = chunk
				.iter()
				.map(|model| {
					model.primary_key().ok_or_else(|| {
						Error::from(DatabaseError::new(
							DatabaseErrorKind::Type,
							"Bulk update model must have primary key",
						))
					})?;
					let obj = model.encode_database_fields().map_err(field_codec_error)?;
					let pk = obj
						.get(M::primary_key_field())
						.filter(|value| !matches!(value, DatabaseValue::Null))
						.cloned()
						.ok_or_else(|| {
							Error::from(DatabaseError::new(
								DatabaseErrorKind::Type,
								format!(
									"Encoded bulk update model must contain primary key '{}'",
									M::primary_key_field()
								),
							))
						})?;

					let mut field_map = HashMap::new();
					for field in fields
						.iter()
						.filter(|field| !Self::is_generated_field(field.as_str()))
					{
						if let Some(val) = obj.get(field) {
							field_map.insert(field.clone(), val.clone());
						}
					}

					Ok((pk, field_map))
				})
				.collect::<reinhardt_core::exception::Result<_>>()?;

			if !updates.is_empty() {
				let sql = self
					.bulk_update_database_values_sql_detailed(&updates, &fields, conn.backend())
					.map_err(field_codec_error)?;
				if sql.is_empty() {
					continue;
				}
				let rows_affected = conn.execute(&sql, vec![]).await?;
				total_updated += rows_affected as usize;
			}
		}

		Ok(total_updated)
	}

	/// Bulk-update models through a caller-owned executor.
	pub async fn bulk_update_with_conn<E>(
		&self,
		conn: &mut E,
		models: Vec<M>,
		fields: Vec<String>,
		batch_size: Option<usize>,
	) -> reinhardt_core::exception::Result<usize>
	where
		E: OrmExecutor,
	{
		if models.is_empty() || fields.is_empty() {
			return Ok(0);
		}

		let batch_size = batch_size.unwrap_or(models.len());
		let mut total_updated = 0;
		for chunk in models.chunks(batch_size) {
			let updates: Vec<(DatabaseValue, HashMap<String, DatabaseValue>)> = chunk
				.iter()
				.map(|model| {
					model.primary_key().ok_or_else(|| {
						Error::from(DatabaseError::new(
							DatabaseErrorKind::Type,
							"Bulk update model must have primary key",
						))
					})?;
					let obj = model.encode_database_fields().map_err(field_codec_error)?;
					let pk = obj
						.get(M::primary_key_field())
						.filter(|value| !matches!(value, DatabaseValue::Null))
						.cloned()
						.ok_or_else(|| {
							Error::from(DatabaseError::new(
								DatabaseErrorKind::Type,
								format!(
									"Encoded bulk update model must contain primary key '{}'",
									M::primary_key_field()
								),
							))
						})?;
					let field_map = fields
						.iter()
						.filter(|field| !Self::is_generated_field(field.as_str()))
						.filter_map(|field| {
							obj.get(field).cloned().map(|value| (field.clone(), value))
						})
						.collect();
					Ok((pk, field_map))
				})
				.collect::<reinhardt_core::exception::Result<_>>()?;

			if updates.is_empty() {
				continue;
			}
			let sql = self
				.bulk_update_database_values_sql_detailed(&updates, &fields, conn.backend())
				.map_err(field_codec_error)?;
			if sql.is_empty() {
				continue;
			}
			total_updated += conn.execute(&sql, Vec::new()).await?.rows_affected as usize;
		}
		Ok(total_updated)
	}

	/// Get or create - SQL generation using reinhardt-query (for testing)
	pub fn get_or_create_queries(
		&self,
		lookup_fields: &HashMap<String, String>,
		defaults: &HashMap<String, String>,
	) -> reinhardt_core::exception::Result<(SelectStatement, InsertStatement)> {
		let lookup_fields = Self::normalize_primary_key_aliases(lookup_fields.clone())?;
		let defaults = Self::normalize_primary_key_aliases(defaults.clone())?;
		Ok(self.get_or_create_queries_from_normalized_fields(&lookup_fields, &defaults))
	}

	fn get_or_create_queries_from_normalized_fields(
		&self,
		lookup_fields: &HashMap<String, String>,
		defaults: &HashMap<String, String>,
	) -> (SelectStatement, InsertStatement) {
		let field_metadata = M::field_metadata();
		let mut select_stmt = Query::select();
		select_stmt
			.from(Alias::new(M::table_name()))
			.column(ColumnRef::Asterisk);

		for (k, v) in lookup_fields.iter() {
			let column = Self::field_column(&field_metadata, k);
			select_stmt.and_where(Expr::col(Alias::new(column)).eq(v.as_str()));
		}

		// Generate INSERT query with reinhardt-query
		let mut insert_fields = lookup_fields.clone();
		insert_fields.extend(defaults.clone());

		let mut insert_stmt = Query::insert();
		insert_stmt.into_table(Alias::new(M::table_name()));

		let columns: Vec<_> = insert_fields
			.keys()
			.map(|k| Alias::new(Self::field_column(&field_metadata, k)))
			.collect();
		let values: Vec<reinhardt_query::prelude::Expr> = insert_fields
			.values()
			.map(|v| Expr::val(v.clone()))
			.collect();

		insert_stmt.columns(columns);
		insert_stmt.values_panic(values);

		(select_stmt.to_owned(), insert_stmt.to_owned())
	}

	/// Get or create - SQL generation (convenience method for testing)
	///
	/// # Arguments
	///
	/// * `lookup_fields` - Fields to lookup
	/// * `defaults` - Default values for creation
	/// * `backend` - Database backend to generate SQL for
	pub fn get_or_create_sql(
		&self,
		lookup_fields: &HashMap<String, String>,
		defaults: &HashMap<String, String>,
		backend: DatabaseBackend,
	) -> reinhardt_core::exception::Result<(String, String)> {
		let (select_stmt, insert_stmt) = self.get_or_create_queries(lookup_fields, defaults)?;
		Ok((
			select_to_string(&select_stmt, backend),
			insert_to_string(&insert_stmt, backend),
		))
	}

	/// Bulk create - SQL generation only (for testing)
	pub fn bulk_create_sql_detailed(
		&self,
		field_names: &[String],
		value_rows: &[Vec<serde_json::Value>],
		ignore_conflicts: bool,
	) -> String {
		if value_rows.is_empty() {
			return String::new();
		}

		let writable_indexes: Vec<_> = field_names
			.iter()
			.enumerate()
			.filter_map(|(index, field)| {
				if Self::is_generated_field(field) {
					None
				} else {
					Some(index)
				}
			})
			.collect();
		let writable_field_names: Vec<_> = writable_indexes
			.iter()
			.map(|index| field_names[*index].clone())
			.collect();
		if writable_field_names.is_empty() {
			return String::new();
		}

		let values_clause: Vec<String> = value_rows
			.iter()
			.map(|row| {
				let values = writable_indexes
					.iter()
					.filter_map(|index| row.get(*index))
					.map(|v| match v {
						serde_json::Value::Null => "NULL".to_string(),
						serde_json::Value::Number(n) => n.to_string(),
						serde_json::Value::String(s) => {
							// SQL injection prevention: Escape single quotes
							format!("'{}'", s.replace("'", "''"))
						}
						serde_json::Value::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
						serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
							// Treat arrays and objects as JSON strings
							format!("'{}'", v.to_string().replace("'", "''"))
						}
					})
					.collect::<Vec<_>>()
					.join(", ");
				format!("({})", values)
			})
			.collect();

		let mut sql = format!(
			"INSERT INTO {} ({}) VALUES {}",
			M::table_name(),
			writable_field_names.join(", "),
			values_clause.join(", ")
		);

		if ignore_conflicts {
			sql.push_str(" ON CONFLICT DO NOTHING");
		}

		sql
	}

	/// Bulk update SQL generation using CASE expressions
	///
	/// Generates raw SQL because reinhardt-query's `UpdateStatement` does not support
	/// expression-based SET values (e.g., CASE WHEN ... END).
	fn bulk_update_database_values_sql_detailed(
		&self,
		updates: &[(DatabaseValue, HashMap<String, DatabaseValue>)],
		fields: &[String],
		backend: DatabaseBackend,
	) -> Result<String, FieldCodecError> {
		if updates.is_empty() || fields.is_empty() {
			return Ok(String::new());
		}

		let table_name = M::table_name();
		let field_metadata = M::field_metadata();
		let primary_key_column = Self::field_column(&field_metadata, M::primary_key_field());
		let mut set_clauses = Vec::new();

		for field in fields
			.iter()
			.filter(|field| !Self::is_generated_field(field.as_str()))
		{
			let mut when_clauses = Vec::new();
			for (pk, field_map) in updates {
				if let Some(value) = field_map.get(field) {
					when_clauses.push(format!(
						"WHEN {} = {} THEN {}",
						quote_identifier(primary_key_column, backend),
						database_value_sql_literal(pk.clone(), backend)?,
						database_value_sql_literal(value.clone(), backend)?
					));
				}
			}
			if !when_clauses.is_empty() {
				let column_name = Self::field_column(&field_metadata, field);
				set_clauses.push(format!(
					"{} = CASE {} END",
					quote_identifier(column_name, backend),
					when_clauses.join(" ")
				));
			}
		}

		if set_clauses.is_empty() {
			return Ok(String::new());
		}
		let ids = updates
			.iter()
			.map(|(pk, _)| database_value_sql_literal(pk.clone(), backend))
			.collect::<Result<Vec<_>, _>>()?
			.join(", ");
		Ok(format!(
			"UPDATE {} SET {} WHERE {} IN ({})",
			quote_identifier(table_name, backend),
			set_clauses.join(", "),
			quote_identifier(primary_key_column, backend),
			ids
		))
	}

	/// Generates bulk-update SQL from legacy JSON input values.
	///
	/// Model writes use the canonical database-value path; this method remains available for
	/// callers that explicitly construct JSON update data.
	pub fn bulk_update_sql_detailed(
		&self,
		updates: &[(M::PrimaryKey, HashMap<String, serde_json::Value>)],
		fields: &[String],
		_backend: DatabaseBackend,
	) -> String
	where
		M::PrimaryKey: std::fmt::Display + Clone,
	{
		if updates.is_empty() || fields.is_empty() {
			return String::new();
		}

		let table_name = M::table_name();
		let field_metadata = M::field_metadata();
		let primary_key_column = Self::field_column(&field_metadata, M::primary_key_field());
		let mut set_clauses = Vec::new();

		for field in fields
			.iter()
			.filter(|field| !Self::is_generated_field(field.as_str()))
		{
			let mut when_clauses = Vec::new();

			for (pk, field_map) in updates.iter() {
				if let Some(value) = field_map.get(field) {
					let val_str = match value {
						serde_json::Value::Null => "NULL".to_string(),
						serde_json::Value::Bool(b) => b.to_string().to_uppercase(),
						serde_json::Value::Number(n) => n.to_string(),
						serde_json::Value::String(s) => format!("'{}'", s.replace('\'', "''")),
						serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
							format!("'{}'", value.to_string().replace('\'', "''"))
						}
					};
					when_clauses.push(format!(
						"WHEN \"{}\" = '{}' THEN {}",
						primary_key_column,
						pk.to_string().replace('\'', "''"),
						val_str
					));
				}
			}

			if !when_clauses.is_empty() {
				let column_name = Self::field_column(&field_metadata, field);
				set_clauses.push(format!(
					"\"{}\" = CASE {} END",
					column_name,
					when_clauses.join(" ")
				));
			}
		}

		if set_clauses.is_empty() {
			return String::new();
		}

		let ids: Vec<String> = updates
			.iter()
			.map(|(pk, _)| format!("'{}'", pk.to_string().replace('\'', "''")))
			.collect();

		format!(
			"UPDATE \"{}\" SET {} WHERE \"{}\" IN ({})",
			table_name,
			set_clauses.join(", "),
			primary_key_column,
			ids.join(", ")
		)
	}
}

impl<M: Model> Default for Manager<M> {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::{Manager, field_codec_error};
	use crate::backends::types::QueryValue;
	use crate::orm::Json;
	use crate::orm::Model;
	use crate::orm::connection::DatabaseBackend;
	use crate::orm::inspection::FieldInfo;
	use crate::orm::{FieldCodecError, FieldSelector};
	use serde::{Deserialize, Serialize};
	use std::collections::HashMap;

	#[cfg(feature = "pgvector")]
	#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
	struct VectorManagerModel {
		id: Option<i64>,
		embedding: crate::orm::Vector<3>,
	}

	#[cfg(feature = "pgvector")]
	#[derive(Debug, Clone)]
	struct VectorManagerModelFields;

	#[cfg(feature = "pgvector")]
	impl FieldSelector for VectorManagerModelFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	#[cfg(feature = "pgvector")]
	impl Model for VectorManagerModel {
		type PrimaryKey = i64;
		type Fields = VectorManagerModelFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"vector_manager_models"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn new_fields() -> Self::Fields {
			VectorManagerModelFields
		}

		fn encode_database_fields(
			&self,
		) -> Result<std::collections::BTreeMap<String, crate::orm::DatabaseValue>, FieldCodecError>
		{
			Ok(std::collections::BTreeMap::from([
				(
					"id".to_owned(),
					self.id
						.map(crate::orm::DatabaseValue::I64)
						.unwrap_or(crate::orm::DatabaseValue::Null),
				),
				(
					"embedding".to_owned(),
					crate::orm::DatabaseValue::Vector(self.embedding.as_slice().to_vec()),
				),
			]))
		}
	}

	#[cfg(feature = "pgvector")]
	fn vector_manager_row() -> crate::orm::Row {
		crate::orm::Row {
			data: HashMap::from([
				("id".to_owned(), QueryValue::Int(7)),
				(
					"embedding".to_owned(),
					QueryValue::Vector(vec![1.0, 2.0, 3.0]),
				),
			]),
		}
	}

	#[cfg(feature = "pgvector")]
	struct VectorManagerOrmExecutor {
		backend: DatabaseBackend,
		calls: Vec<(
			&'static str,
			Option<crate::backends::error::PgvectorOperationKind>,
		)>,
	}

	#[cfg(feature = "pgvector")]
	#[async_trait::async_trait]
	impl crate::orm::OrmExecutor for VectorManagerOrmExecutor {
		fn backend(&self) -> DatabaseBackend {
			self.backend
		}

		async fn execute(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> reinhardt_core::exception::Result<crate::orm::QueryResult> {
			self.calls.push(("execute", None));
			Ok(crate::orm::QueryResult {
				rows_affected: 1,
				last_insert_id: None,
			})
		}

		async fn execute_with_context(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
			context: Option<crate::backends::error::PgvectorOperationKind>,
		) -> reinhardt_core::exception::Result<crate::orm::QueryResult> {
			self.calls.push(("execute_with_context", context));
			Ok(crate::orm::QueryResult {
				rows_affected: 1,
				last_insert_id: None,
			})
		}

		async fn fetch_one(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> reinhardt_core::exception::Result<crate::orm::Row> {
			self.calls.push(("fetch_one", None));
			Ok(vector_manager_row())
		}

		async fn fetch_one_with_context(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
			context: Option<crate::backends::error::PgvectorOperationKind>,
		) -> reinhardt_core::exception::Result<crate::orm::Row> {
			self.calls.push(("fetch_one_with_context", context));
			Ok(vector_manager_row())
		}

		async fn fetch_all(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> reinhardt_core::exception::Result<Vec<crate::orm::Row>> {
			panic!("vector manager update test does not fetch multiple rows")
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> reinhardt_core::exception::Result<Option<crate::orm::Row>> {
			panic!("vector manager update test does not fetch optional rows")
		}
	}

	#[cfg(feature = "pgvector")]
	struct VectorManagerTransaction {
		backend: crate::backends::types::DatabaseType,
		calls: Vec<(
			&'static str,
			Option<crate::backends::error::PgvectorOperationKind>,
		)>,
	}

	#[cfg(feature = "pgvector")]
	#[async_trait::async_trait]
	impl crate::orm::connection::TransactionExecutor for VectorManagerTransaction {
		fn backend(&self) -> crate::backends::types::DatabaseType {
			self.backend
		}

		async fn execute(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> crate::backends::error::Result<crate::orm::QueryResult> {
			self.calls.push(("execute", None));
			Ok(crate::orm::QueryResult {
				rows_affected: 1,
				last_insert_id: None,
			})
		}

		async fn execute_with_context(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
			context: Option<crate::backends::error::PgvectorOperationKind>,
		) -> crate::backends::error::Result<crate::orm::QueryResult> {
			self.calls.push(("execute_with_context", context));
			Ok(crate::orm::QueryResult {
				rows_affected: 1,
				last_insert_id: None,
			})
		}

		async fn fetch_one(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> crate::backends::error::Result<crate::orm::Row> {
			self.calls.push(("fetch_one", None));
			Ok(vector_manager_row())
		}

		async fn fetch_one_with_context(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
			context: Option<crate::backends::error::PgvectorOperationKind>,
		) -> crate::backends::error::Result<crate::orm::Row> {
			self.calls.push(("fetch_one_with_context", context));
			Ok(vector_manager_row())
		}

		async fn fetch_all(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> crate::backends::error::Result<Vec<crate::orm::Row>> {
			panic!("vector manager update test does not fetch multiple rows")
		}

		async fn fetch_optional(
			&mut self,
			_sql: &str,
			_params: Vec<QueryValue>,
		) -> crate::backends::error::Result<Option<crate::orm::Row>> {
			panic!("vector manager update test does not fetch optional rows")
		}

		async fn commit(self: Box<Self>) -> crate::backends::error::Result<()> {
			Ok(())
		}

		async fn rollback(self: Box<Self>) -> crate::backends::error::Result<()> {
			Ok(())
		}
	}

	#[test]
	fn test_field_codec_error_preserves_typed_source() {
		let error = field_codec_error(FieldCodecError::Serialization(
			"rejected manager value".to_owned(),
		));

		assert_eq!(
			error.database_kind(),
			Some(reinhardt_core::exception::DatabaseErrorKind::Serialization)
		);
		assert_eq!(
			error.to_string(),
			"Database error: field serialization failed: rejected manager value"
		);
		let source = std::error::Error::source(&error)
			.expect("manager codec error should preserve its typed source");
		assert!(source.downcast_ref::<FieldCodecError>().is_some());
	}

	#[test]
	fn manager_preserves_vector_query_values() {
		let value =
			Manager::<JsonManagerModel>::query_value_to_sea_value(QueryValue::Vector(vec![
				1.0, 2.0, 3.0,
			]));

		assert_eq!(
			value,
			reinhardt_query::value::Value::Vector(Some(Box::new(vec![1.0, 2.0, 3.0])))
		);
	}

	#[cfg(feature = "pgvector")]
	#[rstest::rstest]
	#[case(DatabaseBackend::Postgres, "fetch_one_with_context")]
	#[case(DatabaseBackend::MySql, "execute_with_context")]
	#[tokio::test]
	async fn manager_update_with_conn_propagates_vector_statement_context(
		#[case] backend: DatabaseBackend,
		#[case] expected_method: &'static str,
	) {
		let model = VectorManagerModel {
			id: Some(7),
			embedding: crate::orm::Vector::try_from(vec![1.0, 2.0, 3.0]).unwrap(),
		};
		let mut executor = VectorManagerOrmExecutor {
			backend,
			calls: Vec::new(),
		};

		let updated = Manager::<VectorManagerModel>::new()
			.update_with_conn(&mut executor, &model)
			.await
			.expect("vector manager update should decode the returned model");

		assert_eq!(updated, model);
		assert_eq!(
			executor.calls.first(),
			Some(&(
				expected_method,
				Some(crate::backends::error::PgvectorOperationKind::VectorValue)
			))
		);
	}

	#[cfg(feature = "pgvector")]
	#[rstest::rstest]
	#[case(
		crate::backends::types::DatabaseType::Postgres,
		"fetch_one_with_context"
	)]
	#[case(crate::backends::types::DatabaseType::Mysql, "execute_with_context")]
	#[tokio::test]
	async fn manager_update_with_transaction_propagates_vector_statement_context(
		#[case] backend: crate::backends::types::DatabaseType,
		#[case] expected_method: &'static str,
	) {
		let model = VectorManagerModel {
			id: Some(7),
			embedding: crate::orm::Vector::try_from(vec![1.0, 2.0, 3.0]).unwrap(),
		};
		let mut executor = VectorManagerTransaction {
			backend,
			calls: Vec::new(),
		};

		let updated = Manager::<VectorManagerModel>::new()
			.save_with_executor(&mut executor, &model)
			.await
			.expect("vector manager transaction update should decode the returned model");

		assert_eq!(updated, model);
		assert_eq!(
			executor.calls.first(),
			Some(&(
				expected_method,
				Some(crate::backends::error::PgvectorOperationKind::VectorValue)
			))
		);
	}

	#[serial_test::serial(sqlx_drivers)]
	#[tokio::test]
	async fn init_database_skips_connection_when_already_initialized() {
		let owner = crate::orm::connection::BackendsConnection::connect_sqlite("sqlite::memory:")
			.await
			.unwrap();
		let lease = crate::orm::connection::DatabaseConnectionLease::register(owner).unwrap();
		let previous = super::replace_database_connection_for_testing(Some(lease)).await;

		let result = super::init_database("unsupported://must-not-connect").await;
		let backend = super::get_connection()
			.await
			.map(|connection| connection.backend());

		super::replace_database_connection_for_testing(previous).await;

		result.expect("repeated initialization should not reconnect");
		assert_eq!(backend.unwrap(), DatabaseBackend::Sqlite);
	}

	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct TestUser {
		id: Option<i64>,
		name: String,
		email: String,
	}

	impl TestUser {
		// Allow dead_code: test helper constructor for manager tests
		#[allow(dead_code)]
		fn new(name: String, email: String) -> Self {
			Self {
				id: None,
				name,
				email,
			}
		}
	}

	#[derive(Debug, Clone)]
	struct TestUserFields;

	impl FieldSelector for TestUserFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for TestUser {
		type PrimaryKey = i64;
		type Fields = TestUserFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"test_user"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn primary_key_field() -> &'static str {
			"id"
		}

		fn new_fields() -> Self::Fields {
			TestUserFields
		}
	}

	#[test]
	fn manager_binds_integer_arrays_natively() {
		let value =
			Manager::<TestUser>::sea_value_to_query_value(reinhardt_query::value::Value::Array(
				reinhardt_query::value::ArrayType::Int,
				Some(Box::new(vec![reinhardt_query::value::Value::Int(Some(7))])),
			));

		assert_eq!(value, crate::orm::connection::QueryValue::IntArray(vec![7]));
	}

	#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
	struct TestSettings {
		theme: String,
	}

	#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
	struct JsonManagerModel {
		id: Option<i64>,
		scalar_json: Json<String>,
		settings: Json<TestSettings>,
		optional_json: Option<Json<serde_json::Value>>,
	}

	#[derive(Debug, Clone)]
	struct JsonManagerModelFields;

	impl FieldSelector for JsonManagerModelFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for JsonManagerModel {
		type PrimaryKey = i64;
		type Fields = JsonManagerModelFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"json_manager_models"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn new_fields() -> Self::Fields {
			JsonManagerModelFields
		}

		fn field_metadata() -> Vec<FieldInfo> {
			vec![
				test_manager_field_info("id", "BigIntegerField", false, true),
				test_manager_field_info("scalar_json", "JsonField", false, false),
				test_manager_field_info("settings", "JsonField", false, false),
				test_manager_field_info("optional_json", "JsonField", true, false),
			]
		}

		fn field_is_none(&self, field_name: &str) -> bool {
			match field_name {
				"id" => self.id.is_none(),
				"optional_json" => self.optional_json.is_none(),
				_ => false,
			}
		}
	}

	fn test_manager_field_info(
		name: &str,
		field_type: &str,
		nullable: bool,
		primary_key: bool,
	) -> FieldInfo {
		FieldInfo {
			name: name.to_string(),
			field_type: field_type.to_string(),
			storage_kind: None,
			domain: None,
			nullable,
			primary_key,
			unique: false,
			blank: false,
			editable: true,
			default: None,
			db_default: None,
			db_column: None,
			choices: None,
			attributes: HashMap::new(),
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct GeneratedUser {
		id: Option<i64>,
		name: String,
		email: String,
		full_name: String,
	}

	#[derive(Debug, Clone)]
	struct GeneratedUserFields;

	impl FieldSelector for GeneratedUserFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for GeneratedUser {
		type PrimaryKey = i64;
		type Fields = GeneratedUserFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"generated_user"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn primary_key_field() -> &'static str {
			"id"
		}

		fn generated_field_names() -> &'static [&'static str] {
			&["full_name"]
		}

		fn new_fields() -> Self::Fields {
			GeneratedUserFields
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct GeneratedOnlyUser {
		id: Option<i64>,
		full_name: String,
	}

	#[derive(Debug, Clone)]
	struct GeneratedOnlyUserFields;

	impl FieldSelector for GeneratedOnlyUserFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for GeneratedOnlyUser {
		type PrimaryKey = i64;
		type Fields = GeneratedOnlyUserFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"generated_only_user"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn primary_key_field() -> &'static str {
			"id"
		}

		fn generated_field_names() -> &'static [&'static str] {
			&["full_name"]
		}

		fn new_fields() -> Self::Fields {
			GeneratedOnlyUserFields
		}
	}

	#[test]
	fn test_get_or_create_sql() {
		let manager = TestUser::objects();
		let mut lookup = HashMap::new();
		lookup.insert("email".to_string(), "test@example.com".to_string());

		let mut defaults = HashMap::new();
		defaults.insert("name".to_string(), "Test User".to_string());

		let (select_sql, insert_sql) = manager
			.get_or_create_sql(&lookup, &defaults, DatabaseBackend::Postgres)
			.unwrap();

		// reinhardt-query uses quoted identifiers and TestUser table is "test_user"
		assert!(select_sql.contains("SELECT") && select_sql.contains("FROM"));
		assert!(select_sql.contains("test_user"));
		assert!(select_sql.contains("email"));
		// reinhardt-query produces parameterized SQL with $1 placeholder instead of inline values
		assert!(select_sql.contains("$1"));
		assert!(insert_sql.contains("INSERT"));
		assert!(insert_sql.contains("test_user"));
		assert!(insert_sql.contains("email"));
		assert!(insert_sql.contains("name"));
	}

	#[test]
	fn test_bulk_create_sql() {
		use serde_json::json;
		let manager = TestUser::objects();
		let fields = vec!["name".to_string(), "email".to_string()];
		let values = vec![
			vec![json!("Alice"), json!("alice@example.com")],
			vec![json!("Bob"), json!("bob@example.com")],
		];

		let sql = manager.bulk_create_sql_detailed(&fields, &values, false);

		// reinhardt-query uses quoted identifiers and TestUser table is "test_user"
		assert!(sql.contains("INSERT"));
		assert!(sql.contains("test_user"));
		assert!(sql.contains("name"));
		assert!(sql.contains("email"));
		assert!(sql.contains("Alice"));
		assert!(sql.contains("alice@example.com"));
		assert!(sql.contains("Bob"));
		assert!(sql.contains("bob@example.com"));
	}

	#[test]
	fn test_bulk_create_sql_with_conflict() {
		use serde_json::json;
		let manager = TestUser::objects();
		let fields = vec!["name".to_string(), "email".to_string()];
		let values = vec![vec![json!("Alice"), json!("alice@example.com")]];

		let sql = manager.bulk_create_sql_detailed(&fields, &values, true);

		assert!(sql.contains("ON CONFLICT DO NOTHING"));
	}

	#[test]
	fn test_bulk_create_query_preserves_json_field_tags() {
		let manager = JsonManagerModel::objects();
		let model = JsonManagerModel {
			id: Some(1),
			scalar_json: Json::new("draft".to_string()),
			settings: Json::new(TestSettings {
				theme: "paper".to_string(),
			}),
			optional_json: Some(Json::new(serde_json::Value::Null)),
		};

		let stmt = manager.bulk_create_query(&[model]).unwrap();
		let (_, values) = super::build_insert_sql(&stmt, DatabaseBackend::Postgres);
		let json_value_count = values
			.0
			.iter()
			.filter(|value| matches!(value, reinhardt_query::value::Value::Json(_)))
			.count();

		assert_eq!(json_value_count, 3);
	}

	#[rstest::rstest]
	fn test_backend_json_string_scalar_preserves_native_json_provenance() {
		// Arrange
		let expected = JsonManagerModel {
			id: Some(1),
			scalar_json: Json::new("draft".to_string()),
			settings: Json::new(TestSettings {
				theme: "paper".to_string(),
			}),
			optional_json: None,
		};
		let mut backend_row = crate::backends::types::Row::new();
		backend_row.insert("id".to_string(), crate::backends::types::QueryValue::Int(1));
		backend_row.insert(
			"scalar_json".to_string(),
			crate::backends::types::QueryValue::Json(Some(Box::new(serde_json::Value::String(
				"draft".to_string(),
			)))),
		);
		backend_row.insert(
			"settings".to_string(),
			crate::backends::types::QueryValue::Json(Some(Box::new(serde_json::json!({
				"theme": "paper"
			})))),
		);
		backend_row.insert(
			"optional_json".to_string(),
			crate::backends::types::QueryValue::Json(None),
		);

		// Act
		let model = crate::orm::connection::QueryRow::from_backend_row(backend_row)
			.deserialize_model::<JsonManagerModel>()
			.unwrap();

		// Assert
		assert_eq!(model, expected);
	}

	#[serial_test::serial(sqlx_drivers)]
	#[tokio::test]
	async fn test_manager_create_roundtrips_typed_json_fields_on_sqlite() {
		let database_file = tempfile::NamedTempFile::new().unwrap();
		let database_url = format!("sqlite://{}", database_file.path().display());
		let owner = crate::orm::connection::BackendsConnection::connect_sqlite(&database_url)
			.await
			.unwrap();
		let lease = crate::orm::connection::DatabaseConnectionLease::register(owner).unwrap();
		let mut connection = lease.handle();
		connection
			.execute(
				"CREATE TABLE json_manager_models (\
				 id INTEGER PRIMARY KEY AUTOINCREMENT, \
				 scalar_json TEXT NOT NULL, \
				 settings TEXT NOT NULL, \
				 optional_json TEXT NULL)",
				vec![],
			)
			.await
			.unwrap();
		let model = JsonManagerModel {
			id: None,
			scalar_json: Json::new("draft".to_string()),
			settings: Json::new(TestSettings {
				theme: "paper".to_string(),
			}),
			optional_json: Some(Json::new(serde_json::Value::Null)),
		};

		let created = JsonManagerModel::objects()
			.create_with_conn(&mut connection, &model)
			.await
			.unwrap();

		assert_eq!(created.scalar_json.as_inner(), "draft");
		assert_eq!(created.settings.theme, "paper");
		assert_eq!(
			created.optional_json.unwrap().into_inner(),
			serde_json::Value::Null
		);
	}

	#[test]
	fn test_bulk_create_sql_detailed_omits_generated_fields() {
		use serde_json::json;
		let manager = GeneratedUser::objects();
		let fields = vec![
			"name".to_string(),
			"email".to_string(),
			"full_name".to_string(),
		];
		let values = vec![vec![
			json!("Alice"),
			json!("alice@example.com"),
			json!("Alice Smith"),
		]];

		let sql = manager.bulk_create_sql_detailed(&fields, &values, false);

		assert!(sql.contains("INSERT INTO generated_user"));
		assert!(sql.contains("name"));
		assert!(sql.contains("email"));
		assert!(!sql.contains("full_name"));
		assert!(!sql.contains("Alice Smith"));
	}

	#[test]
	fn test_bulk_create_sql_detailed_returns_empty_for_only_generated_fields() {
		use serde_json::json;
		let manager = GeneratedUser::objects();
		let fields = vec!["full_name".to_string()];
		let values = vec![vec![json!("Alice Smith")]];

		let sql = manager.bulk_create_sql_detailed(&fields, &values, false);

		assert!(sql.is_empty());
	}

	#[test]
	fn test_bulk_update_sql() {
		use serde_json::json;
		let manager = TestUser::objects();

		let mut updates = Vec::new();
		let mut user1_fields = HashMap::new();
		user1_fields.insert("name".to_string(), json!("Alice Updated"));
		user1_fields.insert("email".to_string(), json!("alice_new@example.com"));
		updates.push((1i64, user1_fields));

		let mut user2_fields = HashMap::new();
		user2_fields.insert("name".to_string(), json!("Bob Updated"));
		user2_fields.insert("email".to_string(), json!("bob_new@example.com"));
		updates.push((2i64, user2_fields));

		let fields = vec!["name".to_string(), "email".to_string()];
		let sql = manager.bulk_update_sql_detailed(&updates, &fields, DatabaseBackend::Postgres);

		// reinhardt-query uses quoted identifiers and TestUser table is "test_user"
		assert!(sql.contains("UPDATE"));
		assert!(sql.contains("test_user"));
		assert!(sql.contains("SET"));
		assert!(sql.contains("name"));
		assert!(sql.contains("CASE"));
		assert!(sql.contains("email"));
		assert!(sql.contains("Alice Updated"));
		assert!(sql.contains("Bob Updated"));
		assert!(sql.contains("WHERE"));
	}

	#[test]
	fn test_bulk_update_database_values_serializes_arrays_per_backend() {
		use crate::orm::{DatabaseArrayType, DatabaseValue};

		let manager = TestUser::objects();
		let mut field_values = HashMap::new();
		field_values.insert(
			"name".to_string(),
			DatabaseValue::Array {
				element_type: DatabaseArrayType::String,
				values: vec![
					DatabaseValue::String("alpha".to_string()),
					DatabaseValue::String("beta".to_string()),
				],
			},
		);
		let updates = vec![(DatabaseValue::I64(1), field_values)];
		let fields = vec!["name".to_string()];

		let sqlite_sql = manager
			.bulk_update_database_values_sql_detailed(&updates, &fields, DatabaseBackend::Sqlite)
			.expect("SQLite array SQL should render");
		assert!(sqlite_sql.contains("'[\"alpha\",\"beta\"]'"));
		assert!(!sqlite_sql.contains("ARRAY["));

		let postgres_sql = manager
			.bulk_update_database_values_sql_detailed(&updates, &fields, DatabaseBackend::Postgres)
			.expect("PostgreSQL array SQL should render");
		assert!(postgres_sql.contains("ARRAY["));
	}

	#[test]
	fn test_bulk_update_database_values_casts_empty_postgres_arrays() {
		use crate::orm::{DatabaseArrayType, DatabaseValue};

		let manager = TestUser::objects();
		let mut field_values = HashMap::new();
		field_values.insert(
			"name".to_string(),
			DatabaseValue::Array {
				element_type: DatabaseArrayType::String,
				values: vec![],
			},
		);
		let updates = vec![(DatabaseValue::I64(1), field_values)];
		let fields = vec!["name".to_string()];

		let sql = manager
			.bulk_update_database_values_sql_detailed(&updates, &fields, DatabaseBackend::Postgres)
			.expect("PostgreSQL empty array SQL should render");

		assert!(sql.contains("ARRAY[]::text[]"));
	}

	#[test]
	fn test_bulk_update_sql_detailed_omits_generated_fields() {
		use serde_json::json;
		let manager = GeneratedUser::objects();
		let mut updates = Vec::new();
		let mut fields_map = HashMap::new();
		fields_map.insert("name".to_string(), json!("Alice Updated"));
		fields_map.insert("full_name".to_string(), json!("Alice Smith"));
		updates.push((1i64, fields_map));
		let fields = vec!["name".to_string(), "full_name".to_string()];

		let sql = manager.bulk_update_sql_detailed(&updates, &fields, DatabaseBackend::Postgres);

		assert!(sql.contains("UPDATE \"generated_user\""));
		assert!(sql.contains("\"name\""));
		assert!(sql.contains("Alice Updated"));
		assert!(!sql.contains("full_name"));
		assert!(!sql.contains("Alice Smith"));
	}

	#[test]
	fn test_bulk_update_sql_detailed_returns_empty_for_only_generated_fields() {
		use serde_json::json;
		let manager = GeneratedUser::objects();
		let mut updates = Vec::new();
		let mut fields_map = HashMap::new();
		fields_map.insert("full_name".to_string(), json!("Alice Smith"));
		updates.push((1i64, fields_map));
		let fields = vec!["full_name".to_string()];

		let sql = manager.bulk_update_sql_detailed(&updates, &fields, DatabaseBackend::Postgres);

		assert!(sql.is_empty());
	}

	#[test]
	fn test_update_statement_uses_noop_set_for_generated_only_models() {
		let model = GeneratedOnlyUser {
			id: Some(7),
			full_name: "Alice Smith".to_string(),
		};
		let obj = model
			.encode_database_fields()
			.expect("model fields should encode");
		let stmt =
			Manager::<GeneratedOnlyUser>::build_update_statement_from_object(&obj, |_| false)
				.expect("encoded primary key should build an update statement");

		let (sql, params) = super::build_update_sql(&stmt, DatabaseBackend::Postgres);

		assert_eq!(
			sql,
			"UPDATE \"generated_only_user\" SET \"id\" = \"id\" WHERE \"id\" = $1 RETURNING \"id\", \"full_name\""
		);
		assert_eq!(params.len(), 1);
	}

	#[test]
	fn test_create_statement_rejects_generated_only_models() {
		let model = GeneratedOnlyUser {
			id: None,
			full_name: "Alice Smith".to_string(),
		};
		let obj = model
			.encode_database_fields()
			.expect("model fields should encode");

		let err = Manager::<GeneratedOnlyUser>::build_insert_statement_from_object(&obj, |_| false)
			.expect_err("generated-only create should fail before rendering empty INSERT");

		assert!(
			err.to_string().contains("no writable fields remain"),
			"unexpected error: {err}"
		);
	}

	#[test]
	fn test_bulk_create_empty() {
		use serde_json::Value;
		let manager = TestUser::objects();
		let fields: Vec<String> = vec![];
		let values: Vec<Vec<Value>> = vec![];

		let sql = manager.bulk_create_sql_detailed(&fields, &values, false);
		assert!(sql.is_empty());
	}

	#[test]
	fn test_bulk_update_empty() {
		use serde_json::Value;
		let manager = TestUser::objects();
		let updates: Vec<(i64, HashMap<String, Value>)> = vec![];
		let fields = vec!["name".to_string()];

		let sql = manager.bulk_update_sql_detailed(&updates, &fields, DatabaseBackend::Postgres);
		assert!(sql.is_empty());
	}

	// ──────────────────────────────────────────────────────────────
	// Additional manager tests
	// ──────────────────────────────────────────────────────────────

	#[test]
	fn test_manager_new() {
		let manager = super::Manager::<TestUser>::new();
		// Manager is just a phantom type wrapper, so this just ensures it compiles
		let _ = manager;
	}

	#[test]
	fn test_manager_default() {
		let manager = super::Manager::<TestUser>::default();
		// Default should work the same as new
		let _ = manager;
	}

	#[test]
	fn test_get_or_create_sql_empty_lookup() {
		let manager = TestUser::objects();
		let lookup: HashMap<String, String> = HashMap::new();
		let defaults: HashMap<String, String> = HashMap::new();

		let (select_sql, insert_sql) = manager
			.get_or_create_sql(&lookup, &defaults, DatabaseBackend::Postgres)
			.unwrap();

		// Empty lookup still produces valid SQL structure
		assert!(select_sql.contains("SELECT") || select_sql.contains("select"));
		assert!(insert_sql.contains("INSERT") || insert_sql.contains("insert"));
	}

	#[test]
	fn test_get_or_create_sql_with_multiple_lookups() {
		let manager = TestUser::objects();
		let mut lookup = HashMap::new();
		lookup.insert("email".to_string(), "test@example.com".to_string());
		lookup.insert("name".to_string(), "Test User".to_string());

		let defaults: HashMap<String, String> = HashMap::new();

		let (select_sql, _insert_sql) = manager
			.get_or_create_sql(&lookup, &defaults, DatabaseBackend::Postgres)
			.unwrap();

		// Should have both conditions in WHERE clause
		assert!(select_sql.contains("email"));
		assert!(select_sql.contains("name"));
	}

	#[test]
	fn test_bulk_create_sql_single_row() {
		use serde_json::json;
		let manager = TestUser::objects();
		let fields = vec!["name".to_string()];
		let values = vec![vec![json!("SingleUser")]];

		let sql = manager.bulk_create_sql_detailed(&fields, &values, false);

		assert!(sql.contains("INSERT"));
		assert!(sql.contains("test_user"));
		assert!(sql.contains("SingleUser"));
	}

	#[test]
	fn test_bulk_update_sql_single_field() {
		use serde_json::json;
		let manager = TestUser::objects();

		let mut updates = Vec::new();
		let mut user1_fields = HashMap::new();
		user1_fields.insert("name".to_string(), json!("Updated Name"));
		updates.push((1i64, user1_fields));

		let fields = vec!["name".to_string()];
		let sql = manager.bulk_update_sql_detailed(&updates, &fields, DatabaseBackend::Postgres);

		assert!(sql.contains("UPDATE"));
		assert!(sql.contains("name"));
		assert!(sql.contains("Updated Name"));
		assert!(!sql.contains("email"));
	}

	#[test]
	fn test_json_to_sea_value_string() {
		use serde_json::json;
		let value = json!("hello");
		let sea_value = super::Manager::<TestUser>::json_to_sea_value(&value);

		// reinhardt-query Value should contain the string
		let debug_str = format!("{:?}", sea_value);
		assert!(debug_str.contains("hello") || debug_str.contains("String"));
	}

	#[test]
	fn test_json_to_sea_value_integer() {
		use serde_json::json;
		let value = json!(42);
		let sea_value = super::Manager::<TestUser>::json_to_sea_value(&value);

		let debug_str = format!("{:?}", sea_value);
		assert!(debug_str.contains("42") || debug_str.contains("Int"));
	}

	#[test]
	fn test_json_to_sea_value_float() {
		use serde_json::json;
		let value = json!(1.5);
		let sea_value = super::Manager::<TestUser>::json_to_sea_value(&value);

		let debug_str = format!("{:?}", sea_value);
		assert!(debug_str.contains("1.5") || debug_str.contains("Double"));
	}

	#[test]
	fn test_json_to_sea_value_bool() {
		use serde_json::json;
		let value = json!(true);
		let sea_value = super::Manager::<TestUser>::json_to_sea_value(&value);

		let debug_str = format!("{:?}", sea_value);
		assert!(debug_str.contains("true") || debug_str.contains("Bool"));
	}

	#[test]
	fn test_json_to_sea_value_null() {
		use serde_json::json;
		let value = json!(null);
		let sea_value = super::Manager::<TestUser>::json_to_sea_value(&value);

		// Null should be represented somehow
		let debug_str = format!("{:?}", sea_value);
		assert!(!debug_str.is_empty());
	}

	#[test]
	fn test_json_to_sea_value_array() {
		use serde_json::json;
		let value = json!([1, 2, 3]);
		let sea_value = super::Manager::<TestUser>::json_to_sea_value(&value);

		// Array should be converted (typically to JSON string)
		let debug_str = format!("{:?}", sea_value);
		assert!(!debug_str.is_empty());
	}

	#[test]
	fn test_json_to_sea_value_object() {
		use serde_json::json;
		let value = json!({"key": "value"});
		let sea_value = super::Manager::<TestUser>::json_to_sea_value(&value);

		// Object should be converted (typically to JSON string)
		let debug_str = format!("{:?}", sea_value);
		assert!(!debug_str.is_empty());
	}

	#[test]
	fn test_serialize_value_string() {
		use serde_json::json;
		let value = json!("test_string");
		let serialized = super::Manager::<TestUser>::serialize_value(&value);

		// Should return the string representation
		assert!(serialized.contains("test_string"));
	}

	#[test]
	fn test_serialize_value_number() {
		use serde_json::json;
		let value = json!(123);
		let serialized = super::Manager::<TestUser>::serialize_value(&value);

		assert!(serialized.contains("123"));
	}
}
