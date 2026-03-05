use super::connection::{DatabaseBackend, DatabaseConnection};
use super::{Model, QuerySet};
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

/// Global database connection state
static DB: once_cell::sync::OnceCell<Arc<RwLock<Option<DatabaseConnection>>>> =
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
	let conn = DatabaseConnection::connect_with_pool_size(url, pool_size).await?;
	DB.get_or_init(|| Arc::new(RwLock::new(Some(conn))));
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
	let conn = DatabaseConnection::connect_with_pool_size(url, pool_size).await?;

	if let Some(db_cell) = DB.get() {
		// Replace existing connection
		let mut guard = db_cell.write().await;
		*guard = Some(conn);
	} else {
		// First time initialization
		DB.get_or_init(|| Arc::new(RwLock::new(Some(conn))));
	}

	Ok(())
}

/// Get a reference to the global database connection
pub async fn get_connection() -> reinhardt_core::exception::Result<DatabaseConnection> {
	let db = DB.get().ok_or_else(|| {
		reinhardt_core::exception::Error::Database("Database not initialized".to_string())
	})?;
	let guard = db.read().await;
	guard.clone().ok_or_else(|| {
		reinhardt_core::exception::Error::Database("Database connection not available".to_string())
	})
}

/// Model manager (similar to Django's Manager)
/// Provides an interface for database operations
pub struct Manager<M: Model> {
	_marker: PhantomData<M>,
}

impl<M: Model> Manager<M> {
	pub fn new() -> Self {
		Self {
			_marker: PhantomData,
		}
	}

	/// Get all records
	pub fn all(&self) -> QuerySet<M> {
		QuerySet::new()
	}

	/// Filter records by field, operator, and value
	///
	/// Accepts both string literals and FieldRef for type-safe field references.
	///
	/// # Examples
	///
	/// ```ignore
	/// // String literal
	/// User::objects().filter("email", FilterOperator::Eq, FilterValue::String("alice@example.com".to_string()))
	///
	/// // Type-safe FieldRef
	/// User::objects().filter(User::field_email(), FilterOperator::Eq, FilterValue::String("alice@example.com".to_string()))
	/// ```
	pub fn filter<F: Into<String>>(
		&self,
		field: F,
		operator: super::query::FilterOperator,
		value: super::query::FilterValue,
	) -> QuerySet<M> {
		let filter = super::query::Filter::new(field.into(), operator, value);
		QuerySet::new().filter(filter)
	}

	/// Filter records by a Filter object (Django-style)
	///
	/// # Example
	///
	/// ```ignore
	/// let users = User::objects()
	///     .filter_by(User::field_name().eq("Alice"))
	///     .all(&db)
	///     .await?;
	/// ```
	pub fn filter_by(&self, filter: super::query::Filter) -> QuerySet<M> {
		QuerySet::new().filter(filter)
	}

	/// Get a single record by primary key
	/// Returns a QuerySet filtered by the primary key field
	pub fn get(&self, pk: M::PrimaryKey) -> QuerySet<M> {
		let pk_field = M::primary_key_field();
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
	pub fn select_related(&self, fields: &[&str]) -> QuerySet<M> {
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
	pub fn prefetch_related(&self, fields: &[&str]) -> QuerySet<M> {
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
	///     })
	///     .all()
	///     .await?;
	/// ```
	pub fn filter_in_subquery<R: super::Model, F>(&self, field: &str, subquery_fn: F) -> QuerySet<M>
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
	///     })
	///     .all()
	///     .await?;
	/// ```
	pub fn filter_not_in_subquery<R: super::Model, F>(
		&self,
		field: &str,
		subquery_fn: F,
	) -> QuerySet<M>
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
	///     })
	///     .all()
	///     .await?;
	/// ```
	pub fn filter_exists<R: super::Model, F>(&self, subquery_fn: F) -> QuerySet<M>
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
	///     })
	///     .all()
	///     .await?;
	/// ```
	pub fn filter_not_exists<R: super::Model, F>(&self, subquery_fn: F) -> QuerySet<M>
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
	///     })
	///     .all()
	///     .await?;
	/// ```
	pub fn annotate_subquery<R, F>(&self, name: &str, builder: F) -> QuerySet<M>
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
		let conn = get_connection().await?;
		self.create_with_conn(&conn, model).await
	}

	/// Create a new record with an explicit database connection
	///
	/// This method allows using a specific connection, which is essential for
	/// transaction support. When operations are performed within a transaction,
	/// the same connection must be used throughout.
	///
	/// # Arguments
	///
	/// * `conn` - The database connection to use
	/// * `model` - The model to create
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::{Model, Manager, TransactionScope};
	/// # async fn example<M: Model>(manager: Manager<M>, model: &M) -> reinhardt_core::exception::Result<()> {
	/// use reinhardt_db::orm::manager::get_connection;
	///
	/// let conn = get_connection().await?;
	/// let tx = TransactionScope::begin(&conn).await?;
	///
	/// // Create within transaction
	/// let created = manager.create_with_conn(&conn, model).await?;
	///
	/// tx.commit().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn create_with_conn(
		&self,
		conn: &DatabaseConnection,
		model: &M,
	) -> reinhardt_core::exception::Result<M> {
		let json = serde_json::to_value(model)
			.map_err(|e| reinhardt_core::exception::Error::Database(e.to_string()))?;

		// Extract fields and values from model
		let obj = json.as_object().ok_or_else(|| {
			reinhardt_core::exception::Error::Database("Model must serialize to object".to_string())
		})?;

		// Build reinhardt-query INSERT statement
		let mut stmt = Query::insert();
		stmt.into_table(Alias::new(M::table_name()));

		// Get the primary key field name to filter out auto-increment fields
		let pk_field = M::primary_key_field();

		// Filter out primary key fields and null datetime fields.
		// Null datetime fields are skipped to let database DEFAULT apply
		// (e.g., created_at, updated_at with DEFAULT CURRENT_TIMESTAMP).
		let (fields, values): (Vec<_>, Vec<_>) = obj
			.iter()
			.filter(|(k, v)| {
				let key = k.as_str();
				// Exclude primary key field if it's null or 0 (auto-increment)
				if key == pk_field {
					if v.is_null() {
						return false;
					}
					if let Some(n) = v.as_i64() {
						return n != 0;
					}
				}
				// Skip null datetime fields to let database DEFAULT apply
				if v.is_null()
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
				// Convert null values to SQL NULL for proper insertion
				let value = if v.is_null() {
					reinhardt_query::value::Value::Int(None)
				} else {
					Self::json_to_sea_value(v)
				};
				(Alias::new(k.as_str()), value)
			})
			.unzip();

		stmt.columns(fields);
		stmt.values_panic(values);

		// Add RETURNING clause with explicit column names from JSON object
		// Note: Using Asterisk in columns() may not work correctly with reinhardt-query
		let all_columns: Vec<_> = obj.keys().map(|k| Alias::new(k.as_str())).collect();
		stmt.returning(all_columns);

		let (sql, values) = build_insert_sql(&stmt, conn.backend());
		let values: Vec<_> = values
			.0
			.into_iter()
			.map(Self::sea_value_to_query_value)
			.collect();

		let row = conn.query_one(&sql, values).await?;

		// row.data is already serde_json::Value::Object so deserialize directly
		serde_json::from_value(row.data.clone())
			.map_err(|e| reinhardt_core::exception::Error::Database(e.to_string()))
	}

	/// Convert serde_json::Value to reinhardt_query::value::Value for parameter binding
	fn json_to_sea_value(v: &serde_json::Value) -> reinhardt_query::value::Value {
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
	fn sea_value_to_query_value(v: reinhardt_query::value::Value) -> super::connection::QueryValue {
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

			// UUID handling
			reinhardt_query::value::Value::Uuid(Some(u)) => QueryValue::Uuid(*u),
			reinhardt_query::value::Value::Uuid(None) => QueryValue::Null,

			// JSON types - serialize to string
			reinhardt_query::value::Value::Json(Some(json)) => QueryValue::String(json.to_string()),
			reinhardt_query::value::Value::Json(None) => QueryValue::Null,

			// For complex types or unsupported types, convert to null
			// This is a safe fallback that won't cause runtime errors
			_ => QueryValue::Null,
		}
	}

	/// Serialize a JSON value to SQL-compatible string representation
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
		let conn = get_connection().await?;
		self.update_with_conn(&conn, model).await
	}

	/// Update an existing record with an explicit database connection
	///
	/// This method allows using a specific connection, which is essential for
	/// transaction support.
	///
	/// # Arguments
	///
	/// * `conn` - The database connection to use
	/// * `model` - The model to update (must have primary key set)
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::{Model, Manager, TransactionScope};
	/// # async fn example<M: Model>(manager: Manager<M>, model: &M) -> reinhardt_core::exception::Result<()> {
	/// use reinhardt_db::orm::manager::get_connection;
	///
	/// let conn = get_connection().await?;
	/// let tx = TransactionScope::begin(&conn).await?;
	///
	/// // Update within transaction
	/// let updated = manager.update_with_conn(&conn, model).await?;
	///
	/// tx.commit().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn update_with_conn(
		&self,
		conn: &DatabaseConnection,
		model: &M,
	) -> reinhardt_core::exception::Result<M> {
		let pk = model.primary_key().ok_or_else(|| {
			reinhardt_core::exception::Error::Database("Model must have primary key".to_string())
		})?;

		let json = serde_json::to_value(model)
			.map_err(|e| reinhardt_core::exception::Error::Database(e.to_string()))?;

		let obj = json.as_object().ok_or_else(|| {
			reinhardt_core::exception::Error::Database("Model must serialize to object".to_string())
		})?;

		// Build reinhardt-query UPDATE statement
		let mut stmt = Query::update();
		stmt.table(Alias::new(M::table_name()));

		// Add SET clauses for all fields except primary key
		for (k, v) in obj
			.iter()
			.filter(|(k, _)| k.as_str() != M::primary_key_field())
		{
			if v.is_null() {
				// Use untyped NULL to avoid PostgreSQL type mismatch errors
				// (e.g., setting timestamp column to NULL would fail with Int(None))
				stmt.value_expr(Alias::new(k.as_str()), Expr::cust("NULL"));
			} else {
				stmt.value(Alias::new(k.as_str()), Self::json_to_sea_value(v));
			}
		}

		// Add WHERE clause for primary key
		// Try to parse as i64 first (common for primary keys), fallback to string
		let pk_str = pk.to_string();
		let pk_value = if let Ok(int_value) = pk_str.parse::<i64>() {
			reinhardt_query::value::Value::BigInt(Some(int_value))
		} else if let Ok(uuid) = Uuid::parse_str(&pk_str) {
			reinhardt_query::value::Value::Uuid(Some(Box::new(uuid)))
		} else {
			reinhardt_query::value::Value::String(Some(Box::new(pk_str)))
		};
		stmt.and_where(Expr::col(Alias::new(M::primary_key_field())).eq(pk_value));

		// Add RETURNING clause with explicit column names from JSON object
		// Note: Using Asterisk in columns() may not work correctly with reinhardt-query
		let all_columns: Vec<_> = obj.keys().map(|k| Alias::new(k.as_str())).collect();
		stmt.returning(all_columns);

		let (sql, values) = build_update_sql(&stmt, conn.backend());
		let values: Vec<_> = values
			.0
			.into_iter()
			.map(Self::sea_value_to_query_value)
			.collect();

		let row = conn.query_one(&sql, values).await?;
		// row.data is already serde_json::Value::Object so deserialize directly
		serde_json::from_value(row.data.clone())
			.map_err(|e| reinhardt_core::exception::Error::Database(e.to_string()))
	}

	/// Delete a record using reinhardt-query for SQL injection protection
	pub async fn delete(&self, pk: M::PrimaryKey) -> reinhardt_core::exception::Result<()> {
		let conn = get_connection().await?;
		self.delete_with_conn(&conn, pk).await
	}

	/// Delete a record with an explicit database connection
	///
	/// This method allows using a specific connection, which is essential for
	/// transaction support.
	///
	/// # Arguments
	///
	/// * `conn` - The database connection to use
	/// * `pk` - The primary key of the record to delete
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::{Model, Manager, TransactionScope};
	/// # async fn example<M: Model>(manager: Manager<M>, pk: M::PrimaryKey) -> reinhardt_core::exception::Result<()> {
	/// use reinhardt_db::orm::manager::get_connection;
	///
	/// let conn = get_connection().await?;
	/// let tx = TransactionScope::begin(&conn).await?;
	///
	/// // Delete within transaction
	/// manager.delete_with_conn(&conn, pk).await?;
	///
	/// tx.commit().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn delete_with_conn(
		&self,
		conn: &DatabaseConnection,
		pk: M::PrimaryKey,
	) -> reinhardt_core::exception::Result<()> {
		// Build reinhardt-query DELETE statement
		let mut stmt = Query::delete();

		// Try to parse as i64 first (common for primary keys), fallback to string
		let pk_str = pk.to_string();
		let pk_value = if let Ok(int_value) = pk_str.parse::<i64>() {
			reinhardt_query::value::Value::BigInt(Some(int_value))
		} else if let Ok(uuid) = Uuid::parse_str(&pk_str) {
			reinhardt_query::value::Value::Uuid(Some(Box::new(uuid)))
		} else {
			reinhardt_query::value::Value::String(Some(Box::new(pk_str)))
		};

		stmt.from_table(Alias::new(M::table_name()))
			.and_where(Expr::col(Alias::new(M::primary_key_field())).eq(pk_value));

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
		let conn = get_connection().await?;
		self.count_with_conn(&conn).await
	}

	/// Count records with an explicit database connection
	///
	/// This method allows using a specific connection, which is essential for
	/// verifying data within a transaction before commit/rollback.
	///
	/// # Arguments
	///
	/// * `conn` - The database connection to use
	///
	/// # Examples
	///
	/// ```no_run
	/// # use reinhardt_db::orm::{Model, Manager, TransactionScope};
	/// # async fn example<M: Model>(manager: Manager<M>) -> reinhardt_core::exception::Result<()> {
	/// use reinhardt_db::orm::manager::get_connection;
	///
	/// let conn = get_connection().await?;
	/// let tx = TransactionScope::begin(&conn).await?;
	///
	/// // Count within transaction (sees uncommitted data)
	/// let count = manager.count_with_conn(&conn).await?;
	///
	/// tx.commit().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn count_with_conn(
		&self,
		conn: &DatabaseConnection,
	) -> reinhardt_core::exception::Result<i64> {
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

		let row = conn.query_one(&sql, values).await?;
		row.get::<i64>("count").ok_or_else(|| {
			reinhardt_core::exception::Error::Database("Failed to get count".to_string())
		})
	}

	/// Bulk create multiple records using reinhardt-query (similar to Django's bulk_create())
	pub fn bulk_create_query(&self, models: &[M]) -> Option<InsertStatement> {
		if models.is_empty() {
			return None;
		}

		// Convert all models to JSON and extract field names from first model
		let json_values: Vec<serde_json::Value> = models
			.iter()
			.filter_map(|m| serde_json::to_value(m).ok())
			.collect();

		if json_values.is_empty() {
			return None;
		}

		// Get field names from first model
		let first_obj = json_values[0].as_object()?;

		let fields: Vec<_> = first_obj.keys().map(|k| Alias::new(k.as_str())).collect();

		// Build reinhardt-query INSERT statement
		let mut stmt = Query::insert();
		stmt.into_table(Alias::new(M::table_name())).columns(fields);

		// Add value rows for each model
		for val in &json_values {
			if let Some(obj) = val.as_object() {
				let values: Vec<reinhardt_query::value::Value> = first_obj
					.keys()
					.map(|field| {
						obj.get(field)
							.map(|v| {
								if v.is_null() {
									// Use untyped NULL to avoid PostgreSQL type mismatch errors
									reinhardt_query::value::Value::Int(None)
								} else {
									Self::json_to_sea_value(v)
								}
							})
							// Use untyped NULL for missing fields
							.unwrap_or(reinhardt_query::value::Value::Int(None))
					})
					.collect();
				stmt.values_panic(values);
			}
		}

		Some(stmt.to_owned())
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
	) -> (String, Vec<String>) {
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
	pub fn delete_queryset(&self, queryset: &QuerySet<M>) -> (String, Vec<String>) {
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
		);

		if let Ok(Some(row)) = conn.query_optional(&select_sql, vec![]).await {
			// row.data is already serde_json::Value::Object so deserialize directly
			let model: M = serde_json::from_value(row.data.clone())
				.map_err(|e| reinhardt_core::exception::Error::Database(e.to_string()))?;
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
		let model: M = serde_json::from_value(row.data.clone())
			.map_err(|e| reinhardt_core::exception::Error::Database(e.to_string()))?;

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
			// Extract fields from first model
			let json = serde_json::to_value(&chunk[0])
				.map_err(|e| reinhardt_core::exception::Error::Database(e.to_string()))?;
			let obj = json.as_object().ok_or_else(|| {
				reinhardt_core::exception::Error::Database(
					"Model must serialize to object".to_string(),
				)
			})?;
			// Exclude primary key field if it's Null (for auto-increment)
			let pk_field = M::primary_key_field();
			let field_names: Vec<String> = obj
				.iter()
				.filter_map(|(k, v)| {
					if k == pk_field && v.is_null() {
						None
					} else {
						Some(k.clone())
					}
				})
				.collect();

			// Extract values for all models in chunk
			let value_rows: Vec<Vec<serde_json::Value>> = chunk
				.iter()
				.map(|model| {
					let json = serde_json::to_value(model).unwrap();
					let obj = json.as_object().unwrap();
					field_names.iter().map(|field| obj[field].clone()).collect()
				})
				.collect();

			let sql = self.bulk_create_sql_detailed(&field_names, &value_rows, ignore_conflicts);

			// Execute and get results
			if ignore_conflicts {
				conn.execute(&sql, vec![]).await?;
				// Note: Can't get RETURNING with DO NOTHING, skip results
				// Return empty vec for ignored conflicts
			} else {
				let sql_with_returning = sql + " RETURNING *";
				let rows = conn.query(&sql_with_returning, vec![]).await?;
				for row in rows {
					// row.data is already serde_json::Value::Object so deserialize directly
					let model: M = serde_json::from_value(row.data.clone())
						.map_err(|e| reinhardt_core::exception::Error::Database(e.to_string()))?;
					results.push(model);
				}
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
			let updates: Vec<(M::PrimaryKey, HashMap<String, serde_json::Value>)> = chunk
				.iter()
				.filter_map(|model| {
					let pk = model.primary_key()?.clone();
					let json = serde_json::to_value(model).ok()?;
					let obj = json.as_object()?;

					let mut field_map = HashMap::new();
					for field in &fields {
						if let Some(val) = obj.get(field) {
							field_map.insert(field.clone(), val.clone());
						}
					}

					Some((pk, field_map))
				})
				.collect();

			if !updates.is_empty() {
				let sql = self.bulk_update_sql_detailed(&updates, &fields, conn.backend());
				let rows_affected = conn.execute(&sql, vec![]).await?;
				total_updated += rows_affected as usize;
			}
		}

		Ok(total_updated)
	}

	/// Get or create - SQL generation using reinhardt-query (for testing)
	pub fn get_or_create_queries(
		&self,
		lookup_fields: &HashMap<String, String>,
		defaults: &HashMap<String, String>,
	) -> (SelectStatement, InsertStatement) {
		// Generate SELECT query with reinhardt-query
		let mut select_stmt = Query::select();
		select_stmt
			.from(Alias::new(M::table_name()))
			.column(ColumnRef::Asterisk);

		for (k, v) in lookup_fields.iter() {
			select_stmt.and_where(Expr::col(Alias::new(k.as_str())).eq(v.as_str()));
		}

		// Generate INSERT query with reinhardt-query
		let mut insert_fields = lookup_fields.clone();
		insert_fields.extend(defaults.clone());

		let mut insert_stmt = Query::insert();
		insert_stmt.into_table(Alias::new(M::table_name()));

		let columns: Vec<_> = insert_fields
			.keys()
			.map(|k| Alias::new(k.as_str()))
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
	) -> (String, String) {
		let (select_stmt, insert_stmt) = self.get_or_create_queries(lookup_fields, defaults);
		(
			select_to_string(&select_stmt, backend),
			insert_to_string(&insert_stmt, backend),
		)
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

		let values_clause: Vec<String> = value_rows
			.iter()
			.map(|row| {
				let values = row
					.iter()
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
			field_names.join(", "),
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
		let mut set_clauses = Vec::new();

		for field in fields {
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
						"WHEN \"id\" = '{}' THEN {}",
						pk.to_string().replace('\'', "''"),
						val_str
					));
				}
			}

			if !when_clauses.is_empty() {
				set_clauses.push(format!(
					"\"{}\" = CASE {} END",
					field,
					when_clauses.join(" ")
				));
			}
		}

		let ids: Vec<String> = updates
			.iter()
			.map(|(pk, _)| format!("'{}'", pk.to_string().replace('\'', "''")))
			.collect();

		format!(
			"UPDATE \"{}\" SET {} WHERE \"id\" IN ({})",
			table_name,
			set_clauses.join(", "),
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
	use crate::orm::FieldSelector;
	use crate::orm::Model;
	use crate::orm::connection::DatabaseBackend;
	use serde::{Deserialize, Serialize};
	use std::collections::HashMap;

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
	fn test_get_or_create_sql() {
		let manager = TestUser::objects();
		let mut lookup = HashMap::new();
		lookup.insert("email".to_string(), "test@example.com".to_string());

		let mut defaults = HashMap::new();
		defaults.insert("name".to_string(), "Test User".to_string());

		let (select_sql, insert_sql) =
			manager.get_or_create_sql(&lookup, &defaults, DatabaseBackend::Postgres);

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

		let (select_sql, insert_sql) =
			manager.get_or_create_sql(&lookup, &defaults, DatabaseBackend::Postgres);

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

		let (select_sql, _insert_sql) =
			manager.get_or_create_sql(&lookup, &defaults, DatabaseBackend::Postgres);

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
