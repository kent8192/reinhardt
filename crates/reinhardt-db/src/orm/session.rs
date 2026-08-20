// Copyright 2024-2025 the reinhardt-db authors
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not use
// this file except in compliance with the License. You may obtain a copy of the
// License at
//
// Unless required by applicable law or agreed to in writing, software distributed under
// the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. See the License for the specific language governing
// permissions and limitations under the License.

//! ORM Session - SQLAlchemy-style database session with identity map and unit of work pattern
//!
//! This module provides a Session object that manages database operations with automatic
//! object tracking, identity mapping, and transaction management.

use super::transaction::Transaction;
use crate::orm::inspection::FieldInfo;
use crate::orm::model::Model;
use crate::orm::query::{OrmQuery, QuerySet};
use crate::orm::query_types::{DbBackend, QueryStatement};
use reinhardt_query::value::Value as RValue;
use reinhardt_query::{
	Alias, Expr, ExprTrait, MySqlQueryBuilder, PostgresQueryBuilder, Query as RQuery,
	QueryStatementBuilder, SelectStatement, SimpleExpr, SqliteQueryBuilder,
};
use serde_json::Value;
use sqlx::{AnyPool, Row};
use std::any::TypeId;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

/// Session error types
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
	/// Database error occurred
	DatabaseError(String),
	/// Object not found in session
	ObjectNotFound(String),
	/// Transaction error
	TransactionError(String),
	/// Serialization/deserialization error
	SerializationError(String),
	/// Invalid state
	InvalidState(String),
	/// Flush operation error
	FlushError(String),
}

impl std::fmt::Display for SessionError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::DatabaseError(msg) => write!(f, "Database error: {}", msg),
			Self::ObjectNotFound(msg) => write!(f, "Object not found: {}", msg),
			Self::TransactionError(msg) => write!(f, "Transaction error: {}", msg),
			Self::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
			Self::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
			Self::FlushError(msg) => write!(f, "Flush error: {}", msg),
		}
	}
}

impl std::error::Error for SessionError {}

/// Identity map entry storing tracked objects
struct IdentityEntry {
	/// The serialized object data
	data: Value,
	/// Field metadata used to preserve database-specific query bindings
	field_metadata: Vec<FieldInfo>,
	/// Type ID for runtime type checking
	type_id: TypeId,
	/// Whether the object has been modified
	// Allow dead_code: dirty tracking flag set internally, read by future flush/commit logic
	#[allow(dead_code)]
	is_dirty: bool,
	/// Whether the object should be inserted instead of updated during flush.
	is_new: bool,
}

#[derive(Clone)]
struct PendingDelete {
	table_name: String,
	primary_key_values: Vec<(String, Value, Option<String>)>,
}

/// SQLAlchemy-style ORM session with identity map and unit of work
///
/// # Examples
///
/// ```no_run
/// use reinhardt_db::orm::session::Session;
/// use reinhardt_db::orm::query_types::DbBackend;
/// use sqlx::AnyPool;
/// use std::sync::Arc;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let pool = AnyPool::connect("sqlite::memory:").await?;
/// let session = Session::new(Arc::new(pool), DbBackend::Sqlite).await?;
///
/// // Session is ready for use
/// # Ok(())
/// # }
/// ```
pub struct Session {
	/// Connection pool
	// Allow dead_code: pool stored for session-scoped query execution and transaction management
	#[allow(dead_code)]
	pool: Arc<AnyPool>,
	/// Database backend type
	db_backend: DbBackend,
	/// Active transaction (if any)
	transaction: Option<Transaction>,
	/// Identity map: tracks objects by type and primary key
	identity_map: HashMap<String, IdentityEntry>,
	/// Set of object keys that have been modified
	dirty_objects: HashSet<String>,
	/// Set of object keys marked for deletion
	deleted_objects: HashMap<String, PendingDelete>,
	/// Whether session is closed
	is_closed: bool,
	/// Counter for generating temporary keys for new objects
	new_object_counter: usize,
	/// Generated IDs from the last flush operation (table_name, generated_id)
	last_generated_ids: Vec<(String, i64)>,
}

impl Session {
	/// Create a new session with the given connection pool and database backend
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::session::Session;
	/// use reinhardt_db::orm::query_types::DbBackend;
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = AnyPool::connect("sqlite::memory:").await?;
	/// let session = Session::new(Arc::new(pool), DbBackend::Sqlite).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn new(pool: Arc<AnyPool>, db_backend: DbBackend) -> Result<Self, SessionError> {
		Ok(Self {
			pool,
			db_backend,
			transaction: None,
			identity_map: HashMap::new(),
			dirty_objects: HashSet::new(),
			deleted_objects: HashMap::new(),
			is_closed: false,
			new_object_counter: 0,
			last_generated_ids: Vec::new(),
		})
	}

	/// Add an object to the session for tracking
	///
	/// Objects with a primary key will be tracked for UPDATE operations.
	/// Objects without a primary key (None) will be tracked for INSERT operations.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::session::Session;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	/// use reinhardt_db::orm::query_types::DbBackend;
	///
	/// #[derive(Serialize, Deserialize, Clone)]
	/// struct User {
	///     id: Option<i64>,
	///     name: String,
	/// }
	///
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// impl Model for User {
	///     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	///     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// }
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = AnyPool::connect("sqlite::memory:").await?;
	/// let mut session = Session::new(Arc::new(pool), DbBackend::Sqlite).await?;
	///
	/// // Add existing object with PK (for UPDATE)
	/// let user = User { id: Some(1), name: "Alice".to_string() };
	/// session.add(user).await?;
	///
	/// // Add new object without PK (for INSERT)
	/// let new_user = User { id: None, name: "Bob".to_string() };
	/// session.add(new_user).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn add<T: Model + 'static>(&mut self, obj: T) -> Result<(), SessionError> {
		let is_new = obj.primary_key().is_none();
		self.add_with_state(obj, is_new).await
	}

	/// Add an object as a new row, including objects with an assigned natural key.
	pub async fn add_new<T: Model + 'static>(&mut self, obj: T) -> Result<(), SessionError> {
		self.add_with_state(obj, true).await
	}

	async fn add_with_state<T: Model + 'static>(
		&mut self,
		obj: T,
		is_new: bool,
	) -> Result<(), SessionError> {
		self.check_closed()?;

		// Generate key based on whether object has a primary key
		let key = match obj.primary_key() {
			Some(pk) => {
				// Existing object with PK - use standard key format
				format!("{}:{}", T::table_name(), pk)
			}
			None => {
				// New object without PK - use temporary key format
				let counter = self.new_object_counter;
				self.new_object_counter += 1;
				format!("{}:__new__{}", T::table_name(), counter)
			}
		};

		let data = serde_json::to_value(&obj)
			.map_err(|e| SessionError::SerializationError(e.to_string()))?;

		self.identity_map.insert(
			key.clone(),
			IdentityEntry {
				data,
				field_metadata: T::field_metadata(),
				type_id: TypeId::of::<T>(),
				is_dirty: true,
				is_new,
			},
		);

		self.dirty_objects.insert(key);

		Ok(())
	}

	/// Get an object by primary key
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::session::Session;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	/// use reinhardt_db::orm::query_types::DbBackend;
	///
	/// #[derive(Serialize, Deserialize, Clone)]
	/// struct User {
	///     id: Option<i64>,
	///     name: String,
	/// }
	///
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// impl Model for User {
	///     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	///     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// }
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = AnyPool::connect("sqlite::memory:").await?;
	/// let mut session = Session::new(Arc::new(pool), DbBackend::Sqlite).await?;
	///
	/// let user: Option<User> = session.get(1).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn get<T: Model + 'static>(
		&mut self,
		id: T::PrimaryKey,
	) -> Result<Option<T>, SessionError> {
		self.check_closed()?;

		let key = format!("{}:{}", T::table_name(), id);

		// Check identity map first
		if let Some(entry) = self.identity_map.get(&key) {
			if entry.type_id != TypeId::of::<T>() {
				return Err(SessionError::InvalidState(
					"Type mismatch in identity map".to_string(),
				));
			}

			let obj: T = serde_json::from_value(entry.data.clone())
				.map_err(|e| SessionError::SerializationError(e.to_string()))?;

			return Ok(Some(obj));
		}

		// Query database if not in identity map
		// Use field_metadata() to build the query and map results
		let field_metadata = T::field_metadata();
		if field_metadata.is_empty() {
			// No field metadata available - model might not use derive(Model) macro
			// Return None as we cannot query without field information
			return Ok(None);
		}

		// Build SELECT query using reinhardt_query
		let pk_field = T::primary_key_field();
		let mut select_query = RQuery::select();
		select_query.from(Alias::new(T::table_name()));

		// Add all fields to SELECT
		for field in &field_metadata {
			let column_name = field.db_column.as_deref().unwrap_or(&field.name);
			select_query.column(Alias::new(column_name));
		}

		// Add WHERE clause for primary key
		select_query.and_where(Expr::col(Alias::new(pk_field)).eq(id.to_string()));

		// Build SQL query based on backend
		let sql = match self.db_backend {
			DbBackend::Postgres => select_query.to_string(PostgresQueryBuilder),
			DbBackend::Mysql => select_query.to_string(MySqlQueryBuilder),
			DbBackend::Sqlite => select_query.to_string(SqliteQueryBuilder),
		};

		// Execute query
		let row = match sqlx::query(&sql).fetch_optional(&*self.pool).await {
			Ok(Some(row)) => row,
			Ok(None) => return Ok(None),
			Err(e) => {
				return Err(SessionError::DatabaseError(format!(
					"Failed to query database: {}",
					e
				)));
			}
		};

		// Build JSON object from row data
		let mut json_map = serde_json::Map::new();
		for field in &field_metadata {
			let column_name = field.db_column.as_deref().unwrap_or(&field.name);

			// Extract value from row based on field type
			let value: serde_json::Value = match field.field_type.as_str() {
				typ if typ.contains("IntegerField") => {
					if field.nullable {
						row.try_get::<Option<i32>, _>(column_name)
							.map(|v| {
								v.map(serde_json::Value::from)
									.unwrap_or(serde_json::Value::Null)
							})
							.unwrap_or(serde_json::Value::Null)
					} else {
						row.try_get::<i32, _>(column_name)
							.map(serde_json::Value::from)
							.unwrap_or(serde_json::Value::Null)
					}
				}
				typ if typ.contains("BigIntegerField") => {
					if field.nullable {
						row.try_get::<Option<i64>, _>(column_name)
							.map(|v| {
								v.map(serde_json::Value::from)
									.unwrap_or(serde_json::Value::Null)
							})
							.unwrap_or(serde_json::Value::Null)
					} else {
						row.try_get::<i64, _>(column_name)
							.map(serde_json::Value::from)
							.unwrap_or(serde_json::Value::Null)
					}
				}
				typ if typ.contains("CharField") => {
					if field.nullable {
						row.try_get::<Option<String>, _>(column_name)
							.map(|v| {
								v.map(serde_json::Value::from)
									.unwrap_or(serde_json::Value::Null)
							})
							.unwrap_or(serde_json::Value::Null)
					} else {
						row.try_get::<String, _>(column_name)
							.map(serde_json::Value::from)
							.unwrap_or(serde_json::Value::Null)
					}
				}
				typ if typ.contains("BooleanField") => {
					if field.nullable {
						row.try_get::<Option<bool>, _>(column_name)
							.map(|v| {
								v.map(serde_json::Value::from)
									.unwrap_or(serde_json::Value::Null)
							})
							.unwrap_or(serde_json::Value::Null)
					} else {
						row.try_get::<bool, _>(column_name)
							.map(serde_json::Value::from)
							.unwrap_or(serde_json::Value::Null)
					}
				}
				typ if typ.contains("FloatField") => {
					if field.nullable {
						row.try_get::<Option<f64>, _>(column_name)
							.map(|v| {
								v.map(serde_json::Value::from)
									.unwrap_or(serde_json::Value::Null)
							})
							.unwrap_or(serde_json::Value::Null)
					} else {
						row.try_get::<f64, _>(column_name)
							.map(serde_json::Value::from)
							.unwrap_or(serde_json::Value::Null)
					}
				}
				// Add more type mappings as needed
				_ => serde_json::Value::Null,
			};

			json_map.insert(field.name.clone(), value);
		}

		// Deserialize JSON to model object
		let obj: T = serde_json::from_value(serde_json::Value::Object(json_map)).map_err(|e| {
			SessionError::SerializationError(format!("Failed to deserialize query result: {}", e))
		})?;

		// Add to identity map
		let obj_data = serde_json::to_value(&obj)
			.map_err(|e| SessionError::SerializationError(e.to_string()))?;

		self.identity_map.insert(
			key.clone(),
			IdentityEntry {
				data: obj_data,
				field_metadata: field_metadata.clone(),
				type_id: TypeId::of::<T>(),
				is_dirty: false,
				is_new: false,
			},
		);

		Ok(Some(obj))
	}

	/// Execute a model-shaped queryset using this session's configured pool.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::session::Session;
	/// use reinhardt_db::orm::{Model, QuerySet};
	/// use serde::{Serialize, Deserialize};
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	/// use reinhardt_db::orm::query_types::DbBackend;
	///
	/// #[derive(Serialize, Deserialize, Clone)]
	/// struct User {
	///     id: Option<i64>,
	///     name: String,
	/// }
	///
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// impl Model for User {
	///     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	///     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// }
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = AnyPool::connect("postgres://localhost/test").await?;
	/// let mut session = Session::new(Arc::new(pool), DbBackend::Postgres).await?;
	///
	/// let queryset = QuerySet::<User>::new();
	/// let users: Vec<User> = session.list(&queryset).await?;
	/// # Ok(())
	/// # }
	/// ```
	///
	/// Executes a model-shaped [`QuerySet`] using this session's configured pool
	/// and backend. Bound filter parameters are passed to the driver instead of
	/// interpolated into SQL. Filtering, ordering, distinct, limits, and offsets
	/// are supported; projections, deferred fields, annotations, relations,
	/// joins, grouping, CTEs, and alternate query sources return a
	/// `SessionError::DatabaseError` because they do not produce whole model rows.
	/// On the main line, `sqlx::Any` does not support array filter parameters, so
	/// array-backed filters also return `SessionError::InvalidState`.
	pub async fn list<T>(&self, queryset: &QuerySet<T>) -> Result<Vec<T>, SessionError>
	where
		T: Model + serde::de::DeserializeOwned + 'static,
	{
		let mut connection = self
			.pool
			.acquire()
			.await
			.map_err(|error| SessionError::DatabaseError(error.to_string()))?;
		self.list_with_connection(queryset, &mut *connection).await
	}

	/// Execute a model-shaped [`QuerySet`] through a caller-owned connection.
	pub async fn list_with_connection<T>(
		&self,
		queryset: &QuerySet<T>,
		connection: &mut sqlx::AnyConnection,
	) -> Result<Vec<T>, SessionError>
	where
		T: Model + serde::de::DeserializeOwned + 'static,
	{
		self.list_with_connection_inner(queryset, connection, false)
			.await
	}

	/// Execute a model-shaped [`QuerySet`] and lock matching rows until the
	/// caller-owned transaction completes.
	pub async fn list_with_connection_for_update<T>(
		&self,
		queryset: &QuerySet<T>,
		connection: &mut sqlx::AnyConnection,
	) -> Result<Vec<T>, SessionError>
	where
		T: Model + serde::de::DeserializeOwned + 'static,
	{
		self.list_with_connection_inner(queryset, connection, true)
			.await
	}

	async fn list_with_connection_inner<T>(
		&self,
		queryset: &QuerySet<T>,
		connection: &mut sqlx::AnyConnection,
		lock_rows: bool,
	) -> Result<Vec<T>, SessionError>
	where
		T: Model + serde::de::DeserializeOwned + 'static,
	{
		self.check_closed()?;
		if T::field_metadata().is_empty() {
			return Ok(Vec::new());
		}

		let mut statement = queryset
			.build_full_model_select_statement()
			.map_err(|error| SessionError::DatabaseError(error.to_string()))?;
		let fields = apply_any_model_projection::<T>(&mut statement, self.db_backend)?;
		if lock_rows && matches!(self.db_backend, DbBackend::Postgres | DbBackend::Mysql) {
			statement.clear_distinct();
		}
		let (mut sql, values) = QueryStatement::Select(statement).build(self.db_backend);
		if lock_rows && matches!(self.db_backend, DbBackend::Postgres | DbBackend::Mysql) {
			let root_table = T::table_name().replace('"', "\"\"");
			sql.push_str(" FOR UPDATE");
			if self.db_backend == DbBackend::Postgres {
				sql.push_str(" OF \"");
				sql.push_str(&root_table);
				sql.push('"');
			}
		}
		let sql = sql_with_postgres_parameter_casts(self.db_backend, &sql, &values)?;
		let mut query = sqlx::query(sql.as_ref());
		for value in &values.0 {
			query = bind_reinhardt_query_value(query, value)?;
		}
		let rows = query
			.fetch_all(&mut *connection)
			.await
			.map_err(|error| SessionError::DatabaseError(error.to_string()))?;
		rows.iter()
			.map(|row| deserialize_any_row::<T>(row, &fields))
			.collect()
	}

	/// Execute an unfiltered model query using this session's configured pool.
	pub async fn list_all<T>(&self) -> Result<Vec<T>, SessionError>
	where
		T: Model + serde::de::DeserializeOwned + 'static,
	{
		if T::field_metadata().is_empty() {
			return Ok(Vec::new());
		}
		self.list(&QuerySet::<T>::new()).await
	}

	/// Create a query for the given model type
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::session::Session;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	/// use reinhardt_db::orm::query_types::DbBackend;
	///
	/// #[derive(Serialize, Deserialize, Clone)]
	/// struct User {
	///     id: Option<i64>,
	///     name: String,
	/// }
	///
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// impl Model for User {
	///     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	///     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// }
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = AnyPool::connect("sqlite::memory:").await?;
	/// let session = Session::new(Arc::new(pool), DbBackend::Sqlite).await?;
	///
	/// let query = session.query::<User>();
	/// # Ok(())
	/// # }
	/// ```
	pub fn query<T: Model>(&self) -> OrmQuery {
		OrmQuery::new()
	}

	/// Flush all pending changes to the database
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::session::Session;
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	/// use reinhardt_db::orm::query_types::DbBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = AnyPool::connect("sqlite::memory:").await?;
	/// let mut session = Session::new(Arc::new(pool), DbBackend::Sqlite).await?;
	///
	/// // Add/modify objects...
	/// session.flush().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn flush(&mut self) -> Result<(), SessionError> {
		self.check_closed()?;
		let mut connection = self
			.pool
			.acquire()
			.await
			.map_err(|error| SessionError::DatabaseError(error.to_string()))?;
		self.flush_with_connection(&mut *connection).await
	}

	/// Flush tracked changes through a caller-owned connection.
	pub async fn flush_with_connection(
		&mut self,
		connection: &mut sqlx::AnyConnection,
	) -> Result<(), SessionError> {
		self.check_closed()?;

		// Clear any previously generated IDs
		self.last_generated_ids.clear();

		// Determine database backend from pool
		let backend = self.get_backend();

		// Process dirty objects (INSERT/UPDATE)
		for key in &self.dirty_objects.clone() {
			if let Some(entry) = self.identity_map.get(key) {
				// Parse the identity key to get table name and primary key
				let Some((table_name, _primary_key)) = key.split_once(':') else {
					continue;
				};

				// Extract data from JSON
				if let Some(obj) = entry.data.as_object() {
					let primary_key_fields: Vec<&FieldInfo> = entry
						.field_metadata
						.iter()
						.filter(|field| field.primary_key)
						.collect();
					if !entry.is_new {
						// UPDATE existing record
						let mut update_stmt =
							RQuery::update().table(Alias::new(table_name)).to_owned();

						// Set all columns except primary key and auto-managed datetime fields
						for (col_name, col_value) in obj {
							if col_name == "id"
								|| col_name.ends_with("_id")
								|| primary_key_fields.iter().any(|field| {
									field.name == *col_name || field.db_column_name() == col_name
								}) {
								continue; // Skip primary key columns
							}
							// Skip null values to avoid type inference issues
							// (e.g., NULL being bound as integer for timestamp columns)
							if col_value.is_null() {
								continue;
							}
							// Skip datetime fields that are typically auto-managed
							// These fields are returned as ISO8601 strings from list_all() and
							// cannot be directly inserted into TIMESTAMP columns
							if col_name == "created_at"
								|| col_name == "updated_at"
								|| col_name.ends_with("_date")
								|| col_name.ends_with("_time")
								|| col_name.ends_with("_at")
							{
								continue;
							}
							update_stmt.value(
								Alias::new(col_name),
								json_to_reinhardt_query_value(
									col_value,
									entry
										.field_metadata
										.iter()
										.find(|field| {
											field.name == *col_name
												|| field.db_column_name() == col_name
										})
										.map(|field| field.field_type.as_str()),
								),
							);
						}

						// Add every primary-key component to the update predicate.
						if primary_key_fields.is_empty() {
							let pk_value = obj
								.get("id")
								.filter(|value| !value.is_null())
								.ok_or_else(|| {
									SessionError::InvalidState(
										"Object has no non-null primary key field `id`".to_owned(),
									)
								})?;
							update_stmt.and_where(
								Expr::col(Alias::new("id")).eq(Expr::val(
									json_to_reinhardt_query_value(
										pk_value,
										entry
											.field_metadata
											.iter()
											.find(|field| field.name == "id")
											.map(|field| field.field_type.as_str()),
									),
								)),
							);
						} else {
							for field in &primary_key_fields {
								let pk_value = obj
									.get(&field.name)
									.or_else(|| obj.get(field.db_column_name()))
									.filter(|value| !value.is_null())
									.ok_or_else(|| {
										SessionError::InvalidState(format!(
											"Object has no non-null primary key field `{}`",
											field.name
										))
									})?;
								update_stmt.and_where(
									Expr::col(Alias::new(field.db_column_name())).eq(Expr::val(
										json_to_reinhardt_query_value(
											pk_value,
											Some(field.field_type.as_str()),
										),
									)),
								);
							}
						}

						// Build and execute SQL
						let (sql, values) = match backend {
							DbBackend::Postgres => update_stmt.build(PostgresQueryBuilder),
							DbBackend::Mysql => update_stmt.build(MySqlQueryBuilder),
							DbBackend::Sqlite => update_stmt.build(SqliteQueryBuilder),
						};

						self.execute_with_values(connection, &sql, &values).await?;
					} else {
						// INSERT new record
						let mut insert_stmt = RQuery::insert()
							.into_table(Alias::new(table_name))
							.to_owned();

						let mut columns = Vec::new();
						let mut values_vec: Vec<RValue> = Vec::new();

						for (col_name, col_value) in obj {
							let is_primary_key = primary_key_fields.iter().any(|field| {
								field.name == *col_name || field.db_column_name() == col_name
							});
							let is_assigned_primary_key = is_primary_key && !col_value.is_null();
							// Skip generated keys and relation-managed foreign keys, but retain
							// explicitly assigned values for every declared primary-key column.
							if (col_name == "id" && !is_assigned_primary_key)
								|| (col_name.ends_with("_id") && !is_primary_key)
							{
								continue;
							}
							// Skip null datetime fields to let database DEFAULT apply
							// (e.g., created_at, updated_at with DEFAULT CURRENT_TIMESTAMP)
							if col_value.is_null()
								&& (col_name == "created_at"
									|| col_name == "updated_at" || col_name.ends_with("_date")
									|| col_name.ends_with("_time")
									|| col_name.ends_with("_at"))
							{
								continue;
							}
							columns.push(Alias::new(col_name));
							// For NULL values, use RValue::Int(None) to represent SQL NULL
							if col_value.is_null() {
								values_vec.push(RValue::Int(None));
							} else {
								values_vec.push(json_to_reinhardt_query_value(
									col_value,
									entry
										.field_metadata
										.iter()
										.find(|field| {
											field.name == *col_name
												|| field.db_column_name() == col_name
										})
										.map(|field| field.field_type.as_str()),
								));
							}
						}

						// If there are columns to insert, add them
						if !columns.is_empty() {
							insert_stmt.columns(columns);
							insert_stmt.values(values_vec).map_err(|e| {
								SessionError::FlushError(format!(
									"Failed to build INSERT values: {}",
									e
								))
							})?;
						}

						let returns_generated_id = backend == DbBackend::Postgres
							&& primary_key_fields
								.iter()
								.any(|field| field.db_column_name() == "id")
							&& obj.get("id").is_none_or(Value::is_null);

						// Add RETURNING only when PostgreSQL generates the conventional id key.
						if returns_generated_id {
							insert_stmt.returning_col(Alias::new("id"));
						}

						// Build and execute SQL
						let (sql, values) = match backend {
							DbBackend::Postgres => insert_stmt.build(PostgresQueryBuilder),
							DbBackend::Mysql => insert_stmt.build(MySqlQueryBuilder),
							DbBackend::Sqlite => insert_stmt.build(SqliteQueryBuilder),
						};

						// Execute and get generated ID if available
						if returns_generated_id {
							let row = self.execute_returning(connection, &sql, &values).await?;

							// Extract the generated ID
							let generated_id: i64 = row.try_get("id").map_err(|e| {
								SessionError::FlushError(format!("Failed to extract ID: {}", e))
							})?;

							// Track the generated ID for retrieval after flush
							self.last_generated_ids
								.push((table_name.to_string(), generated_id));

							// Update the identity map
							self.update_identity_map_with_generated_id(
								key,
								table_name,
								generated_id,
							)?;
						} else {
							self.execute_with_values(connection, &sql, &values).await?;
						}
					}
				}
			}
		}

		self.dirty_objects.clear();

		// Process deleted objects (DELETE)
		for (key, pending) in self.deleted_objects.clone() {
			let mut delete_stmt = RQuery::delete()
				.from_table(Alias::new(&pending.table_name))
				.to_owned();

			for (column_name, value, field_type) in pending.primary_key_values {
				delete_stmt.and_where(Expr::col(Alias::new(&column_name)).eq(Expr::val(
					json_to_reinhardt_query_value(&value, field_type.as_deref()),
				)));
			}

			// Build and execute SQL
			let (sql, values) = match backend {
				DbBackend::Postgres => delete_stmt.build(PostgresQueryBuilder),
				DbBackend::Mysql => delete_stmt.build(MySqlQueryBuilder),
				DbBackend::Sqlite => delete_stmt.build(SqliteQueryBuilder),
			};

			self.execute_with_values(connection, &sql, &values).await?;

			// Remove from identity map
			self.identity_map.remove(&key);
		}

		self.deleted_objects.clear();

		Ok(())
	}

	/// Update identity map with generated ID from RETURNING clause
	///
	/// This method is called after executing an INSERT with RETURNING clause
	/// to update the identity map entry with the generated primary key value.
	///
	/// # Arguments
	///
	/// * `old_key` - The current identity key (e.g., "table_name:null")
	/// * `table_name` - The name of the table
	/// * `generated_id` - The generated primary key value from the database
	fn update_identity_map_with_generated_id(
		&mut self,
		old_key: &str,
		table_name: &str,
		generated_id: i64,
	) -> Result<(), SessionError> {
		if let Some(mut entry) = self.identity_map.remove(old_key) {
			// JSON update
			if let Some(obj) = entry.data.as_object_mut() {
				obj.insert("id".to_string(), serde_json::Value::from(generated_id));
			}

			entry.is_dirty = false;
			entry.is_new = false;
			let new_key = format!("{}:{}", table_name, generated_id);
			self.identity_map.insert(new_key, entry);
			self.dirty_objects.remove(old_key);

			Ok(())
		} else {
			Err(SessionError::InvalidState(
				"Entry not found in identity map".to_string(),
			))
		}
	}

	/// Get database backend type from pool
	fn get_backend(&self) -> DbBackend {
		// Return the backend type that was provided during Session creation
		self.db_backend
	}

	/// Get the IDs generated during the last flush operation
	///
	/// Returns a slice of (table_name, generated_id) tuples for all objects
	/// that were inserted with auto-generated primary keys during the last flush.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::session::Session;
	/// use reinhardt_db::orm::query_types::DbBackend;
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = AnyPool::connect("postgres://localhost/test").await?;
	/// let mut session = Session::new(Arc::new(pool), DbBackend::Postgres).await?;
	///
	/// // ... add objects and flush ...
	///
	/// // Get the generated IDs
	/// for (table_name, id) in session.get_generated_ids() {
	///     println!("Generated ID {} for table {}", id, table_name);
	/// }
	/// # Ok(())
	/// # }
	/// ```
	pub fn get_generated_ids(&self) -> &[(String, i64)] {
		&self.last_generated_ids
	}

	/// Execute SQL with reinhardt_query values
	async fn execute_with_values(
		&self,
		connection: &mut sqlx::AnyConnection,
		sql: &str,
		values: &reinhardt_query::value::Values,
	) -> Result<(), SessionError> {
		let sql = sql_with_postgres_parameter_casts(self.db_backend, sql, values)?;
		let mut query = sqlx::query(sql.as_ref());

		// Bind all values from reinhardt_query::value::Values
		for value in &values.0 {
			query = bind_reinhardt_query_value(query, value)?;
		}

		query
			.execute(&mut *connection)
			.await
			.map_err(|e| SessionError::FlushError(e.to_string()))?;

		Ok(())
	}

	/// Execute SQL with RETURNING clause (PostgreSQL)
	async fn execute_returning(
		&self,
		connection: &mut sqlx::AnyConnection,
		sql: &str,
		values: &reinhardt_query::value::Values,
	) -> Result<sqlx::any::AnyRow, SessionError> {
		let sql = sql_with_postgres_parameter_casts(self.db_backend, sql, values)?;
		let mut query = sqlx::query(sql.as_ref());

		// Bind all values from reinhardt_query::value::Values
		for value in &values.0 {
			query = bind_reinhardt_query_value(query, value)?;
		}

		query
			.fetch_one(&mut *connection)
			.await
			.map_err(|e| SessionError::FlushError(e.to_string()))
	}

	/// Commit the current transaction and flush changes
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::session::Session;
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	/// use reinhardt_db::orm::query_types::DbBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = AnyPool::connect("sqlite::memory:").await?;
	/// let mut session = Session::new(Arc::new(pool), DbBackend::Sqlite).await?;
	///
	/// // Add/modify objects...
	/// session.commit().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn commit(&mut self) -> Result<(), SessionError> {
		self.check_closed()?;

		// Flush pending changes
		self.flush().await?;

		// Commit transaction if active
		if let Some(mut tx) = self.transaction.take() {
			tx.commit().map_err(SessionError::TransactionError)?;
		}

		Ok(())
	}

	/// Rollback the current transaction
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::session::Session;
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	/// use reinhardt_db::orm::query_types::DbBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = AnyPool::connect("sqlite::memory:").await?;
	/// let mut session = Session::new(Arc::new(pool), DbBackend::Sqlite).await?;
	///
	/// // Operations...
	/// session.rollback().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn rollback(&mut self) -> Result<(), SessionError> {
		self.check_closed()?;

		// Clear dirty and deleted objects
		self.dirty_objects.clear();
		self.deleted_objects.clear();

		// Rollback transaction if active
		if let Some(mut tx) = self.transaction.take() {
			tx.rollback().map_err(SessionError::TransactionError)?;
		}

		Ok(())
	}

	/// Close the session
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::session::Session;
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	/// use reinhardt_db::orm::query_types::DbBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = AnyPool::connect("sqlite::memory:").await?;
	/// let mut session = Session::new(Arc::new(pool), DbBackend::Sqlite).await?;
	///
	/// session.close().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn close(mut self) -> Result<(), SessionError> {
		if self.is_closed {
			return Ok(());
		}

		// Rollback any pending transaction
		if self.transaction.is_some() {
			self.rollback().await?;
		}

		self.is_closed = true;
		Ok(())
	}

	/// Begin a new transaction
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::session::Session;
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	/// use reinhardt_db::orm::query_types::DbBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = AnyPool::connect("sqlite::memory:").await?;
	/// let mut session = Session::new(Arc::new(pool), DbBackend::Sqlite).await?;
	///
	/// session.begin().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn begin(&mut self) -> Result<(), SessionError> {
		self.check_closed()?;

		if self.transaction.is_some() {
			return Err(SessionError::TransactionError(
				"Transaction already active".to_string(),
			));
		}

		let mut tx = Transaction::new();
		tx.begin().map_err(SessionError::TransactionError)?;

		self.transaction = Some(tx);

		Ok(())
	}

	/// Delete an object from the session
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::session::Session;
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	/// use reinhardt_db::orm::query_types::DbBackend;
	///
	/// #[derive(Serialize, Deserialize, Clone)]
	/// struct User {
	///     id: Option<i64>,
	///     name: String,
	/// }
	///
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// #
	/// impl Model for User {
	///     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	///     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	///     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	///     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// }
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = AnyPool::connect("sqlite::memory:").await?;
	/// let mut session = Session::new(Arc::new(pool), DbBackend::Sqlite).await?;
	///
	/// let user = User { id: Some(1), name: "Alice".to_string() };
	/// session.delete(user).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn delete<T: Model + 'static>(&mut self, obj: T) -> Result<(), SessionError> {
		self.check_closed()?;

		let pk = obj
			.primary_key()
			.ok_or_else(|| SessionError::InvalidState("Object has no primary key".to_string()))?;

		let key = format!("{}:{}", T::table_name(), pk);
		let data = serde_json::to_value(&obj)
			.map_err(|error| SessionError::SerializationError(error.to_string()))?;
		let metadata = T::field_metadata();
		let primary_key_fields: Vec<&FieldInfo> =
			metadata.iter().filter(|field| field.primary_key).collect();
		let mut primary_key_values = Vec::new();
		if primary_key_fields.is_empty() {
			let field_name = T::primary_key_field();
			let value = data
				.get(field_name)
				.cloned()
				.filter(|value| !value.is_null())
				.ok_or_else(|| {
					SessionError::InvalidState(format!(
						"Object has no non-null primary key field `{field_name}`"
					))
				})?;
			let field_type = metadata
				.iter()
				.find(|field| field.name == field_name)
				.map(|field| field.field_type.clone());
			let column_name = metadata
				.iter()
				.find(|field| field.name == field_name)
				.map(|field| field.db_column_name().to_owned())
				.unwrap_or_else(|| field_name.to_owned());
			primary_key_values.push((column_name, value, field_type));
		} else {
			for field in primary_key_fields {
				let value = data
					.get(&field.name)
					.or_else(|| data.get(field.db_column_name()))
					.cloned()
					.filter(|value| !value.is_null())
					.ok_or_else(|| {
						SessionError::InvalidState(format!(
							"Object has no non-null primary key field `{}`",
							field.name
						))
					})?;
				primary_key_values.push((
					field.db_column_name().to_owned(),
					value,
					Some(field.field_type.clone()),
				));
			}
		}

		// Mark for deletion
		self.deleted_objects.insert(
			key.clone(),
			PendingDelete {
				table_name: T::table_name().to_owned(),
				primary_key_values,
			},
		);

		// Remove from dirty set if present
		self.dirty_objects.remove(&key);

		Ok(())
	}

	/// Check if the session is closed
	fn check_closed(&self) -> Result<(), SessionError> {
		if self.is_closed {
			Err(SessionError::InvalidState("Session is closed".to_string()))
		} else {
			Ok(())
		}
	}

	/// Get the number of objects in the identity map
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::session::Session;
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	/// use reinhardt_db::orm::query_types::DbBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = AnyPool::connect("sqlite::memory:").await?;
	/// let session = Session::new(Arc::new(pool), DbBackend::Sqlite).await?;
	///
	/// let count = session.identity_count();
	/// # Ok(())
	/// # }
	/// ```
	pub fn identity_count(&self) -> usize {
		self.identity_map.len()
	}

	/// Get the number of dirty objects
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::session::Session;
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	/// use reinhardt_db::orm::query_types::DbBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = AnyPool::connect("sqlite::memory:").await?;
	/// let session = Session::new(Arc::new(pool), DbBackend::Sqlite).await?;
	///
	/// let count = session.dirty_count();
	/// # Ok(())
	/// # }
	/// ```
	pub fn dirty_count(&self) -> usize {
		self.dirty_objects.len()
	}

	/// Check if session has active transaction
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::session::Session;
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	/// use reinhardt_db::orm::query_types::DbBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = AnyPool::connect("sqlite::memory:").await?;
	/// let session = Session::new(Arc::new(pool), DbBackend::Sqlite).await?;
	///
	/// let has_tx = session.has_transaction();
	/// # Ok(())
	/// # }
	/// ```
	pub fn has_transaction(&self) -> bool {
		self.transaction.is_some()
	}

	/// Check if session is closed
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_db::orm::session::Session;
	/// use sqlx::AnyPool;
	/// use std::sync::Arc;
	/// use reinhardt_db::orm::query_types::DbBackend;
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let pool = AnyPool::connect("sqlite::memory:").await?;
	/// let session = Session::new(Arc::new(pool), DbBackend::Sqlite).await?;
	///
	/// let closed = session.is_closed();
	/// # Ok(())
	/// # }
	/// ```
	pub fn is_closed(&self) -> bool {
		self.is_closed
	}
}

fn apply_any_model_projection<T: Model>(
	statement: &mut SelectStatement,
	backend: DbBackend,
) -> Result<Vec<FieldInfo>, SessionError> {
	let fields = T::field_metadata();
	if fields.is_empty() {
		return Ok(fields);
	}

	statement.clear_selects();
	for field in &fields {
		let column_name = field.db_column.as_deref().unwrap_or(&field.name);
		let column = Expr::col(Alias::new(column_name));
		let quoted_column = match backend {
			DbBackend::Mysql => format!("`{}`", column_name.replace('`', "``")),
			DbBackend::Postgres | DbBackend::Sqlite => {
				format!("\"{}\"", column_name.replace('"', "\"\""))
			}
		};
		let field_type = field.field_type.as_str();
		let expression: SimpleExpr = if is_temporal_field_type(field_type) {
			Expr::cust(temporal_select_column_sql(backend, column_name, field_type))
				.into_simple_expr()
		} else if is_array_field(field) && backend == DbBackend::Postgres {
			Expr::cust(format!("array_to_json({quoted_column})::text")).into_simple_expr()
		} else if is_hstore_field(field) && backend == DbBackend::Postgres {
			Expr::cust(format!("hstore_to_json({quoted_column})::text")).into_simple_expr()
		} else if field_type.contains("UuidField")
			|| field_type.contains("UUIDField")
			|| field_type.contains("TimeField")
			|| field_type.contains("JsonField")
			|| field_type.contains("JSONField")
			|| field_type.contains("JSONBField")
			|| field_type.contains("DecimalField")
			|| is_structured_field(field)
		{
			let text_type = if backend == DbBackend::Mysql {
				"CHAR"
			} else {
				"TEXT"
			};
			Expr::cust(format!("CAST({quoted_column} AS {text_type})")).into_simple_expr()
		} else if field_type.contains("BooleanField") {
			match backend {
				DbBackend::Postgres => column.into_simple_expr(),
				DbBackend::Mysql => {
					Expr::cust(format!("CAST({quoted_column} AS SIGNED)")).into_simple_expr()
				}
				DbBackend::Sqlite => {
					Expr::cust(format!("CAST({quoted_column} AS INTEGER)")).into_simple_expr()
				}
			}
		} else {
			column.into_simple_expr()
		};
		statement.expr_as(expression, Alias::new(column_name));
	}

	Ok(fields)
}

fn is_array_field(field: &FieldInfo) -> bool {
	field.field_type.contains("ArrayField") && !field.field_type.contains("BinaryField")
}

fn is_hstore_field(field: &FieldInfo) -> bool {
	field.field_type.contains("HStoreField")
}

fn is_json_or_array_field(field: &FieldInfo) -> bool {
	(field.field_type.contains("JsonField")
		|| field.field_type.contains("JSONField")
		|| field.field_type.contains("JSONBField")
		|| is_array_field(field))
		&& !field.field_type.contains("BinaryField")
}

fn is_structured_field(field: &FieldInfo) -> bool {
	is_json_or_array_field(field) || is_hstore_field(field)
}

fn is_temporal_field_type(field_type: &str) -> bool {
	field_type.contains("DateTimeField")
		|| field_type.contains("DateField")
		|| field_type.contains("TimeField")
}

fn temporal_select_column_sql(backend: DbBackend, column_name: &str, field_type: &str) -> String {
	let quoted_column = match backend {
		DbBackend::Mysql => format!("`{}`", column_name.replace('`', "``")),
		DbBackend::Postgres | DbBackend::Sqlite => {
			format!("\"{}\"", column_name.replace('"', "\"\""))
		}
	};

	match backend {
		DbBackend::Postgres if field_type.contains("DateTimeField") => {
			format!(
				"TO_CHAR(({quoted_column} AT TIME ZONE 'UTC'), 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')"
			)
		}
		DbBackend::Postgres if field_type.contains("DateField") => {
			format!("TO_CHAR({quoted_column}, 'YYYY-MM-DD')")
		}
		DbBackend::Postgres => format!("TO_CHAR({quoted_column}, 'HH24:MI:SS.US')"),
		DbBackend::Mysql if field_type.contains("DateTimeField") => {
			format!("DATE_FORMAT({quoted_column}, '%Y-%m-%dT%H:%i:%s.%fZ')")
		}
		DbBackend::Mysql if field_type.contains("DateField") => {
			format!("DATE_FORMAT({quoted_column}, '%Y-%m-%d')")
		}
		DbBackend::Mysql => format!("TIME_FORMAT({quoted_column}, '%H:%i:%s.%f')"),
		DbBackend::Sqlite if field_type.contains("DateTimeField") => format!(
			"CASE WHEN instr({quoted_column}, 'T') > 0 THEN CASE WHEN substr({quoted_column}, -1) = 'Z' OR substr({quoted_column}, -6, 1) IN ('+', '-') THEN {quoted_column} ELSE {quoted_column} || 'Z' END WHEN instr({quoted_column}, '.') > 0 THEN replace({quoted_column}, ' ', 'T') || 'Z' ELSE replace({quoted_column}, ' ', 'T') || '.000Z' END"
		),
		DbBackend::Sqlite => quoted_column,
	}
}

fn temporal_row_value<F>(
	row: &sqlx::any::AnyRow,
	column_name: &str,
	serialization_error: F,
) -> Result<Option<Value>, SessionError>
where
	F: Fn(String) -> SessionError,
{
	row.try_get::<Option<String>, _>(column_name)
		.map(|value| value.map(Value::from))
		.map_err(|error| serialization_error(error.to_string()))
}

fn deserialize_any_row<T>(row: &sqlx::any::AnyRow, fields: &[FieldInfo]) -> Result<T, SessionError>
where
	T: Model + serde::de::DeserializeOwned,
{
	let mut json_map = serde_json::Map::new();
	for field in fields {
		let column_name = field.db_column.as_deref().unwrap_or(&field.name);
		let serialization_error = |detail: String| {
			SessionError::SerializationError(format!(
				"table `{}`, field `{}`, column `{}`: {detail}",
				T::table_name(),
				field.name,
				column_name
			))
		};
		let field_type = field.field_type.as_str();
		let value = if field_type.contains("BigAutoField") || field_type.contains("BigIntegerField")
		{
			row.try_get::<Option<i64>, _>(column_name)
				.map(|value| value.map(Value::from))
				.map_err(|error| serialization_error(error.to_string()))?
		} else if field_type.contains("AutoField") || field_type.contains("IntegerField") {
			row.try_get::<Option<i32>, _>(column_name)
				.map(|value| value.map(Value::from))
				.map_err(|error| serialization_error(error.to_string()))?
		} else if field_type.contains("FloatField") {
			row.try_get::<Option<f64>, _>(column_name)
				.map(|value| value.map(Value::from))
				.or_else(|_| {
					row.try_get::<Option<f32>, _>(column_name)
						.map(|value| value.map(|value| Value::from(f64::from(value))))
				})
				.map_err(|error| serialization_error(error.to_string()))?
		} else if field_type.contains("BooleanField") {
			backend_bool_value(row, column_name, field, serialization_error)?.map(Value::Bool)
		} else if is_temporal_field_type(field_type) {
			temporal_row_value(row, column_name, serialization_error)?
		} else if field_type.contains("BinaryField") {
			row.try_get::<Option<Vec<u8>>, _>(column_name)
				.map(|value| value.map(Value::from))
				.map_err(|error| serialization_error(error.to_string()))?
		} else if is_structured_field(field) {
			let value = row
				.try_get::<Option<String>, _>(column_name)
				.map_err(|error| serialization_error(error.to_string()))?;
			value
				.map(|value| {
					serde_json::from_str(&value)
						.map_err(|error| serialization_error(error.to_string()))
				})
				.transpose()?
		} else {
			row.try_get::<Option<String>, _>(column_name)
				.map(|value| value.map(Value::from))
				.map_err(|error| serialization_error(error.to_string()))?
		};

		let value = match value {
			Some(value) => value,
			None if field.nullable => Value::Null,
			None => return Err(serialization_error("unexpected SQL NULL".to_owned())),
		};
		json_map.insert(field.name.clone(), value);
	}

	serde_json::from_value(Value::Object(json_map)).map_err(|error| {
		let field_context = fields
			.iter()
			.map(|field| {
				format!(
					"field `{}`, column `{}`",
					field.name,
					field.db_column_name()
				)
			})
			.collect::<Vec<_>>()
			.join("; ");
		SessionError::SerializationError(format!(
			"table `{}`, {field_context}: failed to deserialize query result: {error}",
			T::table_name(),
		))
	})
}

fn backend_bool_value<F>(
	row: &sqlx::any::AnyRow,
	column_name: &str,
	field: &FieldInfo,
	serialization_error: F,
) -> Result<Option<bool>, SessionError>
where
	F: Fn(String) -> SessionError,
{
	match row.try_get::<Option<i64>, _>(column_name) {
		Ok(Some(0)) => Ok(Some(false)),
		Ok(Some(1)) => Ok(Some(true)),
		Ok(Some(value)) => Err(serialization_error(format!(
			"boolean integer must be 0 or 1, got {value}"
		))),
		Ok(None) => Ok(None),
		Err(integer_error) => row
			.try_get::<Option<bool>, _>(column_name)
			.map_err(|bool_error| {
				serialization_error(format!(
					"cannot decode boolean field {}: integer: {integer_error}; boolean fallback: {bool_error}",
					field.name
				))
			}),
	}
}

/// Convert JSON value to reinhardt_query Value
fn json_to_reinhardt_query_value(value: &Value, field_type: Option<&str>) -> RValue {
	if field_type.is_some_and(|field_type| {
		field_type.contains("JsonField")
			|| field_type.contains("JSONField")
			|| field_type.contains("JsonbField")
			|| field_type.contains("JSONBField")
	}) && !value.is_null()
	{
		return RValue::Json(Some(Box::new(value.clone())));
	}

	match value {
		Value::Null => RValue::Int(None),
		Value::Bool(b) => RValue::Bool(Some(*b)),
		Value::Number(n) => {
			if let Some(i) = n.as_i64() {
				RValue::BigInt(Some(i))
			} else if let Some(f) = n.as_f64() {
				RValue::Double(Some(f))
			} else {
				RValue::Int(None)
			}
		}
		Value::String(s) => {
			if field_type.is_some_and(|field_type| field_type.contains("DecimalField"))
				&& let Ok(decimal) = s.parse::<rust_decimal::Decimal>()
			{
				return RValue::Decimal(Some(Box::new(decimal)));
			}
			if field_type.is_some_and(|field_type| {
				field_type.contains("UuidField") || field_type.contains("UUIDField")
			}) && let Ok(uuid) = Uuid::parse_str(s)
			{
				return RValue::Uuid(Some(Box::new(uuid)));
			}
			if field_type.is_some_and(|field_type| field_type.contains("DateTimeField"))
				&& let Ok(timestamp) = chrono::DateTime::parse_from_rfc3339(s)
			{
				return RValue::ChronoDateTimeUtc(Some(Box::new(
					timestamp.with_timezone(&chrono::Utc),
				)));
			}
			if field_type.is_some_and(|field_type| field_type.contains("DateField"))
				&& let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
			{
				return RValue::ChronoDate(Some(Box::new(date)));
			}
			if field_type.is_some_and(|field_type| field_type.contains("TimeField"))
				&& let Ok(time) = chrono::NaiveTime::parse_from_str(s, "%H:%M:%S%.f")
			{
				return RValue::ChronoTime(Some(Box::new(time)));
			}
			RValue::String(Some(Box::new(s.clone())))
		}
		Value::Array(values)
			if field_type.is_some_and(|field_type| field_type.contains("BinaryField")) =>
		{
			let bytes = values
				.iter()
				.map(|value| value.as_u64().and_then(|value| u8::try_from(value).ok()))
				.collect::<Option<Vec<_>>>();
			bytes.map_or_else(
				|| RValue::String(Some(Box::new(value.to_string()))),
				|bytes| RValue::Bytes(Some(Box::new(bytes))),
			)
		}
		Value::Array(_) | Value::Object(_) => {
			// For complex types, serialize as JSON string
			RValue::String(Some(Box::new(value.to_string())))
		}
	}
}

/// Bind a reinhardt_query Value to a SQLx Any query without lossy conversions.
fn bind_reinhardt_query_value<'q>(
	query: sqlx::query::Query<'q, sqlx::Any, sqlx::any::AnyArguments<'q>>,
	value: &RValue,
) -> Result<sqlx::query::Query<'q, sqlx::Any, sqlx::any::AnyArguments<'q>>, SessionError> {
	let query = match value {
		RValue::Bool(Some(value)) => query.bind(*value),
		RValue::Bool(None) => query.bind(None::<bool>),
		RValue::TinyInt(Some(value)) => query.bind(i32::from(*value)),
		RValue::TinyInt(None) => query.bind(None::<i32>),
		RValue::SmallInt(Some(value)) => query.bind(i32::from(*value)),
		RValue::SmallInt(None) => query.bind(None::<i32>),
		RValue::Int(Some(value)) => query.bind(*value),
		RValue::Int(None) => query.bind(None::<i32>),
		RValue::BigInt(Some(value)) => query.bind(*value),
		RValue::BigInt(None) => query.bind(None::<i64>),
		RValue::TinyUnsigned(Some(value)) => query.bind(i64::from(*value)),
		RValue::TinyUnsigned(None) => query.bind(None::<i64>),
		RValue::SmallUnsigned(Some(value)) => query.bind(i64::from(*value)),
		RValue::SmallUnsigned(None) => query.bind(None::<i64>),
		RValue::Unsigned(Some(value)) => query.bind(i64::from(*value)),
		RValue::Unsigned(None) => query.bind(None::<i64>),
		RValue::BigUnsigned(Some(value)) => {
			let value = i64::try_from(*value).map_err(|_| {
				SessionError::InvalidState(format!(
					"unsigned query parameter {value} exceeds the supported i64 range"
				))
			})?;
			query.bind(value)
		}
		RValue::BigUnsigned(None) => query.bind(None::<i64>),
		RValue::Float(Some(value)) => query.bind(*value),
		RValue::Float(None) => query.bind(None::<f32>),
		RValue::Double(Some(value)) => query.bind(*value),
		RValue::Double(None) => query.bind(None::<f64>),
		RValue::Char(Some(value)) => query.bind(value.to_string()),
		RValue::Char(None) => query.bind(None::<String>),
		RValue::String(Some(value)) => query.bind(value.as_ref().clone()),
		RValue::String(None) => query.bind(None::<String>),
		RValue::Bytes(Some(value)) => query.bind(value.as_ref().clone()),
		RValue::Bytes(None) => query.bind(None::<Vec<u8>>),
		RValue::ChronoDate(Some(value)) => query.bind(value.to_string()),
		RValue::ChronoDate(None) => query.bind(None::<String>),
		RValue::ChronoTime(Some(value)) => query.bind(value.to_string()),
		RValue::ChronoTime(None) => query.bind(None::<String>),
		RValue::ChronoDateTime(Some(value)) => query.bind(value.to_string()),
		RValue::ChronoDateTime(None) => query.bind(None::<String>),
		RValue::ChronoDateTimeUtc(Some(value)) => query.bind(value.to_rfc3339()),
		RValue::ChronoDateTimeUtc(None) => query.bind(None::<String>),
		RValue::ChronoDateTimeLocal(Some(value)) => query.bind(value.to_rfc3339()),
		RValue::ChronoDateTimeLocal(None) => query.bind(None::<String>),
		RValue::ChronoDateTimeWithTimeZone(Some(value)) => query.bind(value.to_rfc3339()),
		RValue::ChronoDateTimeWithTimeZone(None) => query.bind(None::<String>),
		RValue::Uuid(Some(value)) => query.bind(value.to_string()),
		RValue::Uuid(None) => query.bind(None::<String>),
		RValue::Json(Some(value)) => query.bind(value.to_string()),
		RValue::Json(None) => query.bind(None::<String>),
		RValue::Decimal(Some(value)) => query.bind(value.to_string()),
		RValue::Decimal(None) => query.bind(None::<String>),
		RValue::BigDecimal(Some(value)) => query.bind(value.to_string()),
		RValue::BigDecimal(None) => query.bind(None::<String>),
		RValue::Array(_, _) => {
			return Err(SessionError::InvalidState(
				"array query parameters are not supported by sqlx::Any".to_owned(),
			));
		}
	};

	Ok(query)
}

fn sql_with_postgres_parameter_casts<'a>(
	backend: DbBackend,
	sql: &'a str,
	values: &reinhardt_query::value::Values,
) -> Result<std::borrow::Cow<'a, str>, SessionError> {
	if backend != DbBackend::Postgres {
		return Ok(std::borrow::Cow::Borrowed(sql));
	}

	let bytes = sql.as_bytes();
	let mut output = String::with_capacity(sql.len());
	let mut placeholders = HashSet::new();
	let mut index = 0;
	let mut in_single_quote = false;
	let mut in_double_quote = false;

	while index < bytes.len() {
		match bytes[index] {
			b'\'' if !in_double_quote => {
				output.push('\'');
				if in_single_quote && bytes.get(index + 1) == Some(&b'\'') {
					output.push('\'');
					index += 2;
				} else {
					in_single_quote = !in_single_quote;
					index += 1;
				}
			}
			b'"' if !in_single_quote => {
				output.push('"');
				if in_double_quote && bytes.get(index + 1) == Some(&b'"') {
					output.push('"');
					index += 2;
				} else {
					in_double_quote = !in_double_quote;
					index += 1;
				}
			}
			b'$' if !in_single_quote && !in_double_quote => {
				let placeholder_start = index + 1;
				let mut placeholder_end = placeholder_start;
				while bytes.get(placeholder_end).is_some_and(u8::is_ascii_digit) {
					placeholder_end += 1;
				}

				if placeholder_end == placeholder_start {
					output.push('$');
					index += 1;
					continue;
				}

				let placeholder = &sql[placeholder_start..placeholder_end];
				let placeholder_index = placeholder.parse::<usize>().map_err(|_| {
					SessionError::InvalidState(format!(
						"invalid PostgreSQL query placeholder ${placeholder}"
					))
				})?;
				let value_index = placeholder_index.checked_sub(1).ok_or_else(|| {
					SessionError::InvalidState(
						"PostgreSQL query placeholders start at $1".to_owned(),
					)
				})?;
				let value = values.0.get(value_index).ok_or_else(|| {
					SessionError::InvalidState(format!(
						"PostgreSQL query placeholder ${placeholder_index} has no corresponding value"
					))
				})?;

				placeholders.insert(placeholder_index);
				output.push_str(&sql[index..placeholder_end]);
				if let Some(cast) = postgres_parameter_cast(value) {
					output.push_str("::");
					output.push_str(cast);
				}
				index = placeholder_end;
			}
			_ => {
				let character = sql[index..]
					.chars()
					.next()
					.expect("index always points to a valid UTF-8 boundary");
				output.push(character);
				index += character.len_utf8();
			}
		}
	}

	if placeholders.len() != values.0.len() {
		return Err(SessionError::InvalidState(format!(
			"PostgreSQL query has {} distinct placeholders for {} values",
			placeholders.len(),
			values.0.len()
		)));
	}

	Ok(std::borrow::Cow::Owned(output))
}

fn postgres_parameter_cast(value: &RValue) -> Option<&'static str> {
	match value {
		RValue::Uuid(_) => Some("uuid"),
		RValue::ChronoDateTimeUtc(_)
		| RValue::ChronoDateTimeLocal(_)
		| RValue::ChronoDateTimeWithTimeZone(_) => Some("timestamptz"),
		RValue::ChronoDateTime(_) => Some("timestamp"),
		RValue::ChronoDate(_) => Some("date"),
		RValue::ChronoTime(_) => Some("time"),
		RValue::Json(_) => Some("jsonb"),
		RValue::Decimal(_) | RValue::BigDecimal(_) => Some("numeric"),
		_ => None,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::orm::Manager;
	use crate::orm::fields::{AutoField, BigIntegerField, CharField, Field, FloatField};
	use reinhardt_query::value::Values;
	use rstest::*;
	use serde::{Deserialize, Serialize};
	use serial_test::serial;
	use sqlx::Any;

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct TestUser {
		id: Option<i64>,
		name: String,
		email: String,
	}

	#[derive(Debug, Clone)]
	struct TestUserFields;

	impl crate::orm::model::FieldSelector for TestUserFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for TestUser {
		type PrimaryKey = i64;
		type Fields = TestUserFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"users"
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

		fn field_metadata() -> Vec<FieldInfo> {
			let mut id = BigIntegerField::new();
			id.base.primary_key = true;
			id.set_attributes_from_name("id");
			let mut name = CharField::new(255);
			name.set_attributes_from_name("name");
			let mut email = CharField::new(255);
			email.set_attributes_from_name("email");
			vec![
				FieldInfo::from_field(&id),
				FieldInfo::from_field(&name),
				FieldInfo::from_field(&email),
			]
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct FloatPrecisionModel {
		amount: f64,
	}

	impl Model for FloatPrecisionModel {
		type PrimaryKey = i64;
		type Fields = TestUserFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"float_precision_models"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			None
		}

		fn set_primary_key(&mut self, _value: Self::PrimaryKey) {}

		fn new_fields() -> Self::Fields {
			TestUserFields
		}

		fn field_metadata() -> Vec<FieldInfo> {
			let mut amount = FloatField::new();
			amount.set_attributes_from_name("amount");
			vec![FieldInfo::from_field(&amount)]
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct NaturalKeyRecord {
		record_key: String,
		name: String,
	}

	impl Model for NaturalKeyRecord {
		type PrimaryKey = String;
		type Fields = TestUserFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"natural_key_records"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			Some(self.record_key.clone())
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.record_key = value;
		}

		fn new_fields() -> Self::Fields {
			TestUserFields
		}

		fn field_metadata() -> Vec<FieldInfo> {
			let mut record_key = CharField::new(255);
			record_key.base.primary_key = true;
			record_key.set_attributes_from_name("record_key");
			let mut name = CharField::new(255);
			name.set_attributes_from_name("name");
			vec![
				FieldInfo::from_field(&record_key),
				FieldInfo::from_field(&name),
			]
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct AssignedIdRecord {
		id: i64,
		name: String,
	}

	impl Model for AssignedIdRecord {
		type PrimaryKey = i64;
		type Fields = TestUserFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"assigned_id_records"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			Some(self.id)
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = value;
		}

		fn new_fields() -> Self::Fields {
			TestUserFields
		}

		fn field_metadata() -> Vec<FieldInfo> {
			let mut id = BigIntegerField::new();
			id.base.primary_key = true;
			id.set_attributes_from_name("id");
			let mut id_info = FieldInfo::from_field(&id);
			id_info.primary_key = true;
			let mut name = CharField::new(255);
			name.set_attributes_from_name("name");
			vec![id_info, FieldInfo::from_field(&name)]
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct AssignedNaturalKeyRecord {
		organization_id: i64,
		name: String,
	}

	impl Model for AssignedNaturalKeyRecord {
		type PrimaryKey = i64;
		type Fields = TestUserFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"assigned_natural_key_records"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			Some(self.organization_id)
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.organization_id = value;
		}

		fn new_fields() -> Self::Fields {
			TestUserFields
		}

		fn field_metadata() -> Vec<FieldInfo> {
			let mut organization_id = BigIntegerField::new();
			organization_id.base.primary_key = true;
			organization_id.set_attributes_from_name("organization_id");
			let mut organization_info = FieldInfo::from_field(&organization_id);
			organization_info.primary_key = true;
			let mut name = CharField::new(255);
			name.set_attributes_from_name("name");
			vec![organization_info, FieldInfo::from_field(&name)]
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct HiddenPrimaryKeyUser {
		#[serde(skip_serializing)]
		id: Option<i64>,
		name: String,
	}

	impl Model for HiddenPrimaryKeyUser {
		type PrimaryKey = i64;
		type Fields = TestUserFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"hidden_primary_key_users"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn new_fields() -> Self::Fields {
			TestUserFields
		}

		fn field_metadata() -> Vec<FieldInfo> {
			let mut id = BigIntegerField::new();
			id.base.primary_key = true;
			id.set_attributes_from_name("id");
			let mut name = CharField::new(255);
			name.set_attributes_from_name("name");
			vec![FieldInfo::from_field(&id), FieldInfo::from_field(&name)]
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct ProjectionModel {
		id: Option<i64>,
	}

	impl Model for ProjectionModel {
		type PrimaryKey = i64;
		type Fields = TestUserFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"projection_models"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn new_fields() -> Self::Fields {
			TestUserFields
		}

		fn field_metadata() -> Vec<FieldInfo> {
			[
				("uuid_value", "UuidField"),
				("time_value", "TimeField"),
				("json_value", "JsonField"),
				("decimal_value", "DecimalField"),
				("array_value", "ArrayField"),
				("bool_value", "BooleanField"),
				("datetime_value", "DateTimeField"),
			]
			.into_iter()
			.map(|(name, field_type)| {
				let mut field = CharField::new(255);
				field.set_attributes_from_name(name);
				let mut info = FieldInfo::from_field(&field);
				info.field_type = format!("reinhardt.orm.models.{field_type}");
				info
			})
			.collect()
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct TemporalProjectionModel {
		id: Option<i64>,
		date_value: String,
		datetime_value: String,
	}

	impl Model for TemporalProjectionModel {
		type PrimaryKey = i64;
		type Fields = TestUserFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"temporal_projection_models"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn new_fields() -> Self::Fields {
			TestUserFields
		}

		fn field_metadata() -> Vec<FieldInfo> {
			[
				("id", "BigIntegerField"),
				("date_value", "DateField"),
				("datetime_value", "DateTimeField"),
			]
			.into_iter()
			.map(|(name, field_type)| {
				let mut field = CharField::new(255);
				field.set_attributes_from_name(name);
				let mut info = FieldInfo::from_field(&field);
				info.field_type = format!("reinhardt.orm.models.{field_type}");
				info
			})
			.collect()
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
	struct AutoFieldModel {
		id: Option<i32>,
		large_id: i64,
	}

	impl Model for AutoFieldModel {
		type PrimaryKey = i32;
		type Fields = TestUserFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"auto_field_models"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn new_fields() -> Self::Fields {
			TestUserFields
		}

		fn field_metadata() -> Vec<FieldInfo> {
			let mut id = AutoField::new();
			id.set_attributes_from_name("id");
			let mut large_id = BigIntegerField::new();
			large_id.set_attributes_from_name("large_id");
			large_id.base.db_column = Some("large_storage_id".to_owned());
			let mut large_id = FieldInfo::from_field(&large_id);
			large_id.field_type = "reinhardt.orm.models.BigAutoField".to_owned();
			vec![FieldInfo::from_field(&id), large_id]
		}
	}

	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct SerdeContextModel {
		id: Option<i64>,
		typed_value: i32,
	}

	impl Model for SerdeContextModel {
		type PrimaryKey = i64;
		type Fields = TestUserFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"serde_context_models"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn new_fields() -> Self::Fields {
			TestUserFields
		}

		fn field_metadata() -> Vec<FieldInfo> {
			let mut id = BigIntegerField::new();
			id.base.primary_key = true;
			id.set_attributes_from_name("id");
			let mut typed_value = CharField::new(255);
			typed_value.set_attributes_from_name("typed_value");
			typed_value.base.db_column = Some("stored_value".to_owned());
			vec![
				FieldInfo::from_field(&id),
				FieldInfo::from_field(&typed_value),
			]
		}
	}

	// Create test pool using SQLite in-memory database
	async fn create_test_pool() -> Arc<AnyPool> {
		use sqlx::pool::PoolOptions;

		// Initialize SQLx drivers (idempotent operation)
		sqlx::any::install_default_drivers();

		// Use shared in-memory database so all connections see the same data
		// The "mode=memory" and "cache=shared" ensure the database persists across connections
		let pool = PoolOptions::<Any>::new()
			.min_connections(1)
			.max_connections(5)
			.connect("sqlite:file:test_session_db?mode=memory&cache=shared")
			.await
			.expect("Failed to create test pool");

		// Create the users table for testing
		sqlx::query(
			"CREATE TABLE IF NOT EXISTS users (
				id INTEGER PRIMARY KEY,
				name TEXT NOT NULL,
				email TEXT NOT NULL
			)",
		)
		.execute(&pool)
		.await
		.expect("Failed to create users table");

		Arc::new(pool)
	}

	/// Initialize SQLx drivers (required for AnyPool)
	#[fixture]
	fn init_drivers() {
		sqlx::any::install_default_drivers();
	}

	#[rstest]
	#[case(
		DbBackend::Postgres,
		&[
			"CAST(\"uuid_value\" AS TEXT)",
			"array_to_json(\"array_value\")::text",
			"TO_CHAR(\"datetime_value\"",
		]
	)]
	#[case(
		DbBackend::Mysql,
		&[
			"CAST(`uuid_value` AS CHAR)",
			"CAST(`bool_value` AS SIGNED)",
			"DATE_FORMAT(`datetime_value`",
		]
	)]
	#[case(
			DbBackend::Sqlite,
			&[
				"CAST(\"uuid_value\" AS TEXT)",
				"CAST(\"bool_value\" AS INTEGER)",
				"CASE WHEN instr(\"datetime_value\", 'T') > 0",
			]
		)]
	fn any_model_projection_uses_backend_safe_text_and_bool_expressions(
		#[case] backend: DbBackend,
		#[case] expected_fragments: &[&str],
	) {
		let mut statement = RQuery::select()
			.column(Alias::new("ignored"))
			.from(Alias::new(ProjectionModel::table_name()))
			.to_owned();

		let fields =
			apply_any_model_projection::<ProjectionModel>(&mut statement, backend).unwrap();
		let sql = QueryStatement::Select(statement).to_string(backend);

		assert_eq!(fields.len(), 7);
		assert!(
			expected_fragments
				.iter()
				.all(|fragment| sql.contains(fragment))
		);
		assert!(!sql.contains("ignored"));
	}

	#[rstest]
	fn temporal_projection_preserves_date_and_datetime_precision() {
		assert_eq!(
			temporal_select_column_sql(DbBackend::Postgres, "date_value", "DateField"),
			"TO_CHAR(\"date_value\", 'YYYY-MM-DD')"
		);
		assert_eq!(
			temporal_select_column_sql(DbBackend::Postgres, "datetime_value", "DateTimeField"),
			"TO_CHAR((\"datetime_value\" AT TIME ZONE 'UTC'), 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')"
		);
		assert_eq!(
			temporal_select_column_sql(DbBackend::Mysql, "date_value", "DateField"),
			"DATE_FORMAT(`date_value`, '%Y-%m-%d')"
		);
		assert_eq!(
			temporal_select_column_sql(DbBackend::Mysql, "datetime_value", "DateTimeField"),
			"DATE_FORMAT(`datetime_value`, '%Y-%m-%dT%H:%i:%s.%fZ')"
		);
		assert_eq!(
			temporal_select_column_sql(DbBackend::Sqlite, "datetime_value", "DateTimeField"),
			"CASE WHEN instr(\"datetime_value\", 'T') > 0 THEN CASE WHEN substr(\"datetime_value\", -1) = 'Z' OR substr(\"datetime_value\", -6, 1) IN ('+', '-') THEN \"datetime_value\" ELSE \"datetime_value\" || 'Z' END WHEN instr(\"datetime_value\", '.') > 0 THEN replace(\"datetime_value\", ' ', 'T') || 'Z' ELSE replace(\"datetime_value\", ' ', 'T') || '.000Z' END"
		);
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn list_decodes_temporal_fields_without_losing_precision(_init_drivers: ()) {
		let pool = sqlx::pool::PoolOptions::<Any>::new()
			.max_connections(1)
			.connect("sqlite::memory:")
			.await
			.unwrap();
		sqlx::query(
			"CREATE TABLE temporal_projection_models (
				id INTEGER PRIMARY KEY,
				date_value TEXT NOT NULL,
				datetime_value TEXT NOT NULL
			)",
		)
		.execute(&pool)
		.await
		.unwrap();
		sqlx::query(
			"INSERT INTO temporal_projection_models
				(id, date_value, datetime_value) VALUES (?, ?, ?)",
		)
		.bind(1_i64)
		.bind("2026-08-19")
		.bind("2026-08-19T12:34:56.123456Z")
		.execute(&pool)
		.await
		.unwrap();

		let session = Session::new(Arc::new(pool), DbBackend::Sqlite)
			.await
			.unwrap();
		let rows = session
			.list(&QuerySet::<TemporalProjectionModel>::new())
			.await
			.unwrap();

		assert_eq!(
			rows,
			vec![TemporalProjectionModel {
				id: Some(1),
				date_value: "2026-08-19".to_owned(),
				datetime_value: "2026-08-19T12:34:56.123456Z".to_owned(),
			}]
		);
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn list_all_delegates_to_model_queryset(_init_drivers: ()) {
		let pool = sqlx::pool::PoolOptions::<Any>::new()
			.max_connections(1)
			.connect("sqlite::memory:")
			.await
			.unwrap();
		sqlx::query(
			"CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT NOT NULL)",
		)
		.execute(&pool)
		.await
		.unwrap();
		sqlx::query("INSERT INTO users (id, name, email) VALUES (?, ?, ?)")
			.bind(7_i64)
			.bind("Alice")
			.bind("alice@example.com")
			.execute(&pool)
			.await
			.unwrap();
		let session = Session::new(Arc::new(pool), DbBackend::Sqlite)
			.await
			.unwrap();

		let users = session.list_all::<TestUser>().await.unwrap();

		assert_eq!(
			users,
			vec![TestUser {
				id: Some(7),
				name: "Alice".to_owned(),
				email: "alice@example.com".to_owned(),
			}]
		);
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn list_all_decodes_auto_and_big_auto_fields_from_sqlite(_init_drivers: ()) {
		let pool = sqlx::pool::PoolOptions::<Any>::new()
			.max_connections(1)
			.connect("sqlite::memory:")
			.await
			.unwrap();
		sqlx::query(
			"CREATE TABLE auto_field_models (
				id INTEGER PRIMARY KEY,
				large_storage_id INTEGER NOT NULL
			)",
		)
		.execute(&pool)
		.await
		.unwrap();
		sqlx::query("INSERT INTO auto_field_models (id, large_storage_id) VALUES (?, ?)")
			.bind(7_i32)
			.bind(5_000_000_000_i64)
			.execute(&pool)
			.await
			.unwrap();
		let session = Session::new(Arc::new(pool), DbBackend::Sqlite)
			.await
			.unwrap();

		let rows = session.list_all::<AutoFieldModel>().await.unwrap();

		assert_eq!(
			rows,
			vec![AutoFieldModel {
				id: Some(7),
				large_id: 5_000_000_000,
			}]
		);
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn list_all_reports_field_alias_for_model_serde_failure(_init_drivers: ()) {
		let pool = sqlx::pool::PoolOptions::<Any>::new()
			.max_connections(1)
			.connect("sqlite::memory:")
			.await
			.unwrap();
		sqlx::query(
			"CREATE TABLE serde_context_models (
				id INTEGER PRIMARY KEY,
				stored_value TEXT NOT NULL
			)",
		)
		.execute(&pool)
		.await
		.unwrap();
		sqlx::query("INSERT INTO serde_context_models (id, stored_value) VALUES (?, ?)")
			.bind(1_i64)
			.bind("not-an-integer")
			.execute(&pool)
			.await
			.unwrap();
		let session = Session::new(Arc::new(pool), DbBackend::Sqlite)
			.await
			.unwrap();

		let error = session.list_all::<SerdeContextModel>().await.unwrap_err();

		assert!(matches!(
			error,
			SessionError::SerializationError(message)
				if message.contains("table `serde_context_models`")
					&& message.contains("field `typed_value`")
					&& message.contains("column `stored_value`")
		));
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn list_all_rejects_null_for_non_nullable_field(_init_drivers: ()) {
		let pool = sqlx::pool::PoolOptions::<Any>::new()
			.max_connections(1)
			.connect("sqlite::memory:")
			.await
			.unwrap();
		let row = sqlx::query("SELECT NULL AS id, 'Alice' AS name, 'alice@example.com' AS email")
			.fetch_one(&pool)
			.await
			.unwrap();

		let error = deserialize_any_row::<TestUser>(&row, &TestUser::field_metadata()).unwrap_err();

		assert!(matches!(
			error,
			SessionError::SerializationError(message)
				if message.contains("table `users`")
					&& message.contains("field `id`")
					&& message.contains("column `id`")
		));
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn sqlite_float_rows_preserve_f64_precision(_init_drivers: ()) {
		let pool = sqlx::pool::PoolOptions::<Any>::new()
			.max_connections(1)
			.connect("sqlite::memory:")
			.await
			.unwrap();
		let row = sqlx::query("SELECT CAST('0.12345678901234567' AS REAL) AS amount")
			.fetch_one(&pool)
			.await
			.unwrap();

		let model = deserialize_any_row::<FloatPrecisionModel>(
			&row,
			&FloatPrecisionModel::field_metadata(),
		)
		.unwrap();

		assert_eq!(model.amount, 0.12345678901234567_f64);
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn postgres_natural_key_insert_propagates_success(_init_drivers: ()) {
		let pool = sqlx::pool::PoolOptions::<Any>::new()
			.max_connections(1)
			.connect("sqlite::memory:")
			.await
			.unwrap();
		sqlx::query(
			"CREATE TABLE natural_key_records (record_key TEXT PRIMARY KEY, name TEXT NOT NULL)",
		)
		.execute(&pool)
		.await
		.unwrap();

		let pool = Arc::new(pool);
		let mut session = Session::new(pool.clone(), DbBackend::Postgres)
			.await
			.unwrap();
		session
			.add_new(NaturalKeyRecord {
				record_key: "natural-1".to_owned(),
				name: "Alice".to_owned(),
			})
			.await
			.unwrap();

		let mut connection = pool.acquire().await.unwrap();
		session
			.flush_with_connection(&mut *connection)
			.await
			.unwrap();
		let row = sqlx::query("SELECT name FROM natural_key_records WHERE record_key = $1")
			.bind("natural-1")
			.fetch_one(&mut *connection)
			.await
			.unwrap();

		let name: String = row.try_get("name").unwrap();
		assert_eq!(name, "Alice");
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn sqlite_insert_keeps_an_explicit_id_primary_key(_init_drivers: ()) {
		let pool = sqlx::pool::PoolOptions::<Any>::new()
			.max_connections(1)
			.connect("sqlite::memory:")
			.await
			.unwrap();
		sqlx::query(
			"CREATE TABLE assigned_id_records (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
		)
		.execute(&pool)
		.await
		.unwrap();

		let pool = Arc::new(pool);
		let mut session = Session::new(pool.clone(), DbBackend::Sqlite).await.unwrap();
		session
			.add_new(AssignedIdRecord {
				id: 42,
				name: "Assigned".to_owned(),
			})
			.await
			.unwrap();

		let mut connection = pool.acquire().await.unwrap();
		session
			.flush_with_connection(&mut *connection)
			.await
			.unwrap();
		let row = sqlx::query("SELECT name FROM assigned_id_records WHERE id = ?")
			.bind(42_i64)
			.fetch_one(&mut *connection)
			.await
			.unwrap();

		assert_eq!(row.try_get::<String, _>("name").unwrap(), "Assigned");
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn sqlite_insert_keeps_an_explicit_primary_key_ending_in_id(_init_drivers: ()) {
		let pool = sqlx::pool::PoolOptions::<Any>::new()
			.max_connections(1)
			.connect("sqlite::memory:")
			.await
			.unwrap();
		sqlx::query(
			"CREATE TABLE assigned_natural_key_records \
			 (organization_id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
		)
		.execute(&pool)
		.await
		.unwrap();

		let pool = Arc::new(pool);
		let mut session = Session::new(pool.clone(), DbBackend::Sqlite).await.unwrap();
		session
			.add_new(AssignedNaturalKeyRecord {
				organization_id: 7,
				name: "Assigned natural key".to_owned(),
			})
			.await
			.unwrap();

		let mut connection = pool.acquire().await.unwrap();
		session
			.flush_with_connection(&mut *connection)
			.await
			.unwrap();
		let row =
			sqlx::query("SELECT name FROM assigned_natural_key_records WHERE organization_id = ?")
				.bind(7_i64)
				.fetch_one(&mut *connection)
				.await
				.unwrap();

		assert_eq!(
			row.try_get::<String, _>("name").unwrap(),
			"Assigned natural key"
		);
	}

	#[tokio::test]

	async fn test_session_creation() {
		let pool = create_test_pool().await;
		let session = Session::new(pool, DbBackend::Sqlite).await;

		let session = session.unwrap();
		assert!(!session.is_closed());
		assert_eq!(session.identity_count(), 0);
		assert_eq!(session.dirty_count(), 0);
	}

	#[tokio::test]

	async fn test_session_add_object() {
		let pool = create_test_pool().await;
		let mut session = Session::new(pool, DbBackend::Sqlite).await.unwrap();

		let user = TestUser {
			id: Some(1),
			name: "Alice".to_string(),
			email: "alice@example.com".to_string(),
		};

		let result = session.add(user).await;
		assert!(result.is_ok());
		assert_eq!(session.identity_count(), 1);
		assert_eq!(session.dirty_count(), 1);
	}

	#[tokio::test]
	async fn test_session_add_new_tracks_assigned_key_as_insert() {
		let pool = create_test_pool().await;
		let mut session = Session::new(pool, DbBackend::Sqlite).await.unwrap();
		let user = TestUser {
			id: Some(2),
			name: "AssignedNewUser".to_owned(),
			email: "assigned@example.com".to_owned(),
		};

		session.add_new(user).await.unwrap();

		assert!(
			session
				.identity_map
				.get("users:2")
				.is_some_and(|entry| entry.is_new)
		);
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn flush_rejects_existing_objects_without_primary_key_data(_init_drivers: ()) {
		// Arrange
		let pool = create_test_pool().await;
		let mut session = Session::new(pool.clone(), DbBackend::Sqlite)
			.await
			.expect("session should initialize");
		session
			.add(HiddenPrimaryKeyUser {
				id: Some(7),
				name: "hidden".to_owned(),
			})
			.await
			.expect("object should be tracked");
		let mut connection = pool.acquire().await.expect("connection should acquire");

		// Act
		let result = session.flush_with_connection(&mut *connection).await;

		// Assert
		assert_eq!(
			result,
			Err(SessionError::InvalidState(
				"Object has no non-null primary key field `id`".to_owned(),
			))
		);
	}

	#[tokio::test]

	async fn test_session_get_from_identity_map() {
		let pool = create_test_pool().await;
		let mut session = Session::new(pool, DbBackend::Sqlite).await.unwrap();

		let user = TestUser {
			id: Some(1),
			name: "Bob".to_string(),
			email: "bob@example.com".to_string(),
		};

		session.add(user.clone()).await.unwrap();

		let retrieved: Option<TestUser> = session.get(1).await.unwrap();
		assert!(retrieved.is_some());
		assert_eq!(retrieved.unwrap(), user);
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn test_session_flush_clears_dirty(_init_drivers: ()) {
		let pool = create_test_pool().await;
		let mut session = Session::new(pool, DbBackend::Sqlite).await.unwrap();

		let user = TestUser {
			id: Some(1),
			name: "Charlie".to_string(),
			email: "charlie@example.com".to_string(),
		};

		session.add(user).await.unwrap();
		assert_eq!(session.dirty_count(), 1);

		session.flush().await.unwrap();
		assert_eq!(session.dirty_count(), 0);
		assert_eq!(session.identity_count(), 1);
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn test_session_delete_object(_init_drivers: ()) {
		let pool = create_test_pool().await;
		let mut session = Session::new(pool, DbBackend::Sqlite).await.unwrap();

		let user = TestUser {
			id: Some(1),
			name: "Dave".to_string(),
			email: "dave@example.com".to_string(),
		};

		session.add(user.clone()).await.unwrap();
		session.flush().await.unwrap();

		session.delete(user).await.unwrap();
		session.flush().await.unwrap();

		let retrieved: Option<TestUser> = session.get(1).await.unwrap();
		assert!(retrieved.is_none());
	}

	#[tokio::test]

	async fn test_session_transaction_begin() {
		let pool = create_test_pool().await;
		let mut session = Session::new(pool, DbBackend::Sqlite).await.unwrap();

		assert!(!session.has_transaction());

		session.begin().await.unwrap();
		assert!(session.has_transaction());
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn test_session_transaction_commit(_init_drivers: ()) {
		let pool = create_test_pool().await;
		let mut session = Session::new(pool, DbBackend::Sqlite).await.unwrap();

		session.begin().await.unwrap();

		let user = TestUser {
			id: Some(1),
			name: "Eve".to_string(),
			email: "eve@example.com".to_string(),
		};

		session.add(user).await.unwrap();
		session.commit().await.unwrap();

		assert!(!session.has_transaction());
		assert_eq!(session.dirty_count(), 0);
	}

	#[tokio::test]

	async fn test_session_transaction_rollback() {
		let pool = create_test_pool().await;
		let mut session = Session::new(pool, DbBackend::Sqlite).await.unwrap();

		session.begin().await.unwrap();

		let user = TestUser {
			id: Some(1),
			name: "Frank".to_string(),
			email: "frank@example.com".to_string(),
		};

		session.add(user).await.unwrap();
		assert_eq!(session.dirty_count(), 1);

		session.rollback().await.unwrap();

		assert!(!session.has_transaction());
		assert_eq!(session.dirty_count(), 0);
	}

	#[tokio::test]

	async fn test_session_close() {
		let pool = create_test_pool().await;
		let session = Session::new(pool, DbBackend::Sqlite).await.unwrap();

		assert!(!session.is_closed());

		session.close().await.unwrap();
	}

	#[tokio::test]

	async fn test_session_operations_after_close() {
		let pool = create_test_pool().await;
		let session = Session::new(pool, DbBackend::Sqlite).await.unwrap();

		let _user = TestUser {
			id: Some(1),
			name: "Grace".to_string(),
			email: "grace@example.com".to_string(),
		};

		session.close().await.unwrap();

		// Cannot use session after close since it consumes self
		// This test verifies the API design
	}

	#[tokio::test]

	async fn test_session_multiple_objects() {
		let pool = create_test_pool().await;
		let mut session = Session::new(pool, DbBackend::Sqlite).await.unwrap();

		for i in 1..=5 {
			let user = TestUser {
				id: Some(i),
				name: format!("User{}", i),
				email: format!("user{}@example.com", i),
			};
			session.add(user).await.unwrap();
		}

		assert_eq!(session.identity_count(), 5);
		assert_eq!(session.dirty_count(), 5);
	}

	#[tokio::test]

	async fn test_session_delete_removes_from_dirty() {
		let pool = create_test_pool().await;
		let mut session = Session::new(pool, DbBackend::Sqlite).await.unwrap();

		let user = TestUser {
			id: Some(1),
			name: "Henry".to_string(),
			email: "henry@example.com".to_string(),
		};

		session.add(user.clone()).await.unwrap();
		assert_eq!(session.dirty_count(), 1);

		session.delete(user).await.unwrap();
		assert_eq!(session.dirty_count(), 0);
	}

	#[tokio::test]

	async fn test_session_query_creation() {
		let pool = create_test_pool().await;
		let session = Session::new(pool, DbBackend::Sqlite).await.unwrap();

		let _query = session.query::<TestUser>();
	}

	#[tokio::test]

	async fn test_session_double_begin_fails() {
		let pool = create_test_pool().await;
		let mut session = Session::new(pool, DbBackend::Sqlite).await.unwrap();

		session.begin().await.unwrap();
		let result = session.begin().await;

		assert!(result.is_err());
	}

	#[tokio::test]
	async fn test_session_add_without_pk_succeeds() {
		let pool = create_test_pool().await;
		let mut session = Session::new(pool, DbBackend::Sqlite).await.unwrap();

		let user = TestUser {
			id: None,
			name: "NewUser".to_string(),
			email: "newuser@example.com".to_string(),
		};

		// Objects without PK can be added (for INSERT operations)
		let result = session.add(user).await;
		assert!(result.is_ok());
	}

	// ──────────────────────────────────────────────────────────────
	// Additional session tests - SessionError Display
	// ──────────────────────────────────────────────────────────────

	#[test]
	fn test_session_error_database_error_display() {
		let err = SessionError::DatabaseError("connection failed".to_string());
		assert_eq!(err.to_string(), "Database error: connection failed");
	}

	#[test]
	fn test_session_error_object_not_found_display() {
		let err = SessionError::ObjectNotFound("user:123".to_string());
		assert_eq!(err.to_string(), "Object not found: user:123");
	}

	#[test]
	fn test_session_error_transaction_error_display() {
		let err = SessionError::TransactionError("commit failed".to_string());
		assert_eq!(err.to_string(), "Transaction error: commit failed");
	}

	#[test]
	fn test_session_error_serialization_error_display() {
		let err = SessionError::SerializationError("invalid json".to_string());
		assert_eq!(err.to_string(), "Serialization error: invalid json");
	}

	#[test]
	fn test_session_error_invalid_state_display() {
		let err = SessionError::InvalidState("session closed".to_string());
		assert_eq!(err.to_string(), "Invalid state: session closed");
	}

	#[test]
	fn test_session_error_flush_error_display() {
		let err = SessionError::FlushError("failed to write".to_string());
		assert_eq!(err.to_string(), "Flush error: failed to write");
	}

	#[test]
	fn test_session_error_debug() {
		let err = SessionError::DatabaseError("test".to_string());
		let debug_str = format!("{:?}", err);
		assert!(debug_str.contains("DatabaseError"));
		assert!(debug_str.contains("test"));
	}

	#[test]
	fn test_session_error_clone() {
		let err = SessionError::ObjectNotFound("key".to_string());
		let cloned = err.clone();
		assert_eq!(err.to_string(), cloned.to_string());
	}

	#[test]
	fn test_session_error_is_std_error() {
		let err: Box<dyn std::error::Error> =
			Box::new(SessionError::DatabaseError("test".to_string()));
		assert!(err.to_string().contains("Database error"));
	}

	// ──────────────────────────────────────────────────────────────
	// json_to_reinhardt_query_value tests
	// ──────────────────────────────────────────────────────────────

	#[test]
	fn test_json_to_reinhardt_query_value_string() {
		use serde_json::json;
		let value = json!("hello world");
		let rq_value = super::json_to_reinhardt_query_value(&value, None);

		let debug_str = format!("{:?}", rq_value);
		assert!(debug_str.contains("hello world") || debug_str.contains("String"));
	}

	#[rstest]
	fn test_json_to_reinhardt_query_value_does_not_infer_uuid_from_text() {
		let value = serde_json::json!("67e55044-10b1-426f-9247-bb680e5fe0c8");
		let rq_value = super::json_to_reinhardt_query_value(&value, None);

		assert_eq!(
			rq_value,
			RValue::String(Some(Box::new(
				"67e55044-10b1-426f-9247-bb680e5fe0c8".to_owned(),
			)))
		);
	}

	#[test]
	fn test_json_to_reinhardt_query_value_integer() {
		use serde_json::json;
		let value = json!(42);
		let rq_value = super::json_to_reinhardt_query_value(&value, None);

		let debug_str = format!("{:?}", rq_value);
		assert!(debug_str.contains("42") || debug_str.contains("Int"));
	}

	#[test]
	fn test_json_to_reinhardt_query_value_float() {
		use serde_json::json;
		let value = json!(2.5);
		let rq_value = super::json_to_reinhardt_query_value(&value, None);

		let debug_str = format!("{:?}", rq_value);
		assert!(debug_str.contains("2.5") || debug_str.contains("Double"));
	}

	#[test]
	fn test_json_to_reinhardt_query_value_bool_true() {
		use serde_json::json;
		let value = json!(true);
		let rq_value = super::json_to_reinhardt_query_value(&value, None);

		let debug_str = format!("{:?}", rq_value);
		assert!(debug_str.contains("true") || debug_str.contains("Bool"));
	}

	#[test]
	fn test_json_to_reinhardt_query_value_bool_false() {
		use serde_json::json;
		let value = json!(false);
		let rq_value = super::json_to_reinhardt_query_value(&value, None);

		let debug_str = format!("{:?}", rq_value);
		assert!(debug_str.contains("false") || debug_str.contains("Bool"));
	}

	#[test]
	fn test_json_to_reinhardt_query_value_null() {
		use serde_json::json;
		let value = json!(null);
		let rq_value = super::json_to_reinhardt_query_value(&value, None);

		// Should produce some value (null representation)
		let debug_str = format!("{:?}", rq_value);
		assert!(!debug_str.is_empty());
	}

	#[test]
	fn test_json_to_reinhardt_query_value_array() {
		use serde_json::json;
		let value = json!([1, 2, 3]);
		let rq_value = super::json_to_reinhardt_query_value(&value, None);

		// Array should be serialized as JSON string
		let debug_str = format!("{:?}", rq_value);
		assert!(!debug_str.is_empty());
	}

	#[test]
	fn test_json_to_reinhardt_query_value_object() {
		use serde_json::json;
		let value = json!({"name": "test", "count": 42});
		let rq_value = super::json_to_reinhardt_query_value(&value, None);

		// Object should be serialized as JSON string
		let debug_str = format!("{:?}", rq_value);
		assert!(!debug_str.is_empty());
	}

	#[test]
	fn json_to_reinhardt_query_value_preserves_json_fields() {
		let value = serde_json::json!({"name": "test", "count": 42});

		assert_eq!(
			super::json_to_reinhardt_query_value(&value, Some("reinhardt.orm.models.JsonField"),),
			RValue::Json(Some(Box::new(value)))
		);
	}

	#[test]
	fn json_to_reinhardt_query_value_preserves_decimal_fields() {
		let value = serde_json::json!("9007199254740993.123456789");
		let expected = value.as_str().unwrap().parse().unwrap();

		assert_eq!(
			super::json_to_reinhardt_query_value(&value, Some("reinhardt.orm.models.DecimalField"),),
			RValue::Decimal(Some(Box::new(expected)))
		);
	}

	#[test]
	fn test_json_to_reinhardt_query_value_negative_integer() {
		use serde_json::json;
		let value = json!(-100);
		let rq_value = super::json_to_reinhardt_query_value(&value, None);

		let debug_str = format!("{:?}", rq_value);
		assert!(debug_str.contains("-100") || debug_str.contains("Int"));
	}

	#[test]
	fn test_json_to_reinhardt_query_value_large_integer() {
		use serde_json::json;
		let value = json!(9223372036854775807i64); // i64::MAX
		let rq_value = super::json_to_reinhardt_query_value(&value, None);

		// Should handle large integers
		let debug_str = format!("{:?}", rq_value);
		assert!(!debug_str.is_empty());
	}

	// ──────────────────────────────────────────────────────────────
	// DbBackend tests
	// ──────────────────────────────────────────────────────────────

	#[tokio::test]
	async fn test_session_get_backend() {
		let pool = create_test_pool().await;
		let session = Session::new(pool, DbBackend::Sqlite).await.unwrap();

		assert_eq!(session.get_backend(), DbBackend::Sqlite);
	}

	// ──────────────────────────────────────────────────────────────
	// bind_reinhardt_query_value tests
	// ──────────────────────────────────────────────────────────────

	#[test]
	fn bind_reinhardt_query_value_rejects_unsigned_overflow() {
		let result = bind_reinhardt_query_value(
			sqlx::query("SELECT ?"),
			&RValue::BigUnsigned(Some(u64::MAX)),
		);
		assert!(matches!(
			result,
			Err(SessionError::InvalidState(ref message))
				if message.contains("exceeds the supported i64 range")
		));
	}

	#[tokio::test]
	async fn bind_reinhardt_query_value_keeps_uuid_and_timestamp_non_null() {
		let pool = create_test_pool().await;
		let uuid = Uuid::parse_str("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap();
		let timestamp = chrono::DateTime::parse_from_rfc3339("2026-08-19T00:00:00Z")
			.unwrap()
			.with_timezone(&chrono::Utc);
		let mut query = sqlx::query("SELECT ? AS uuid_value, ? AS timestamp_value");
		query = bind_reinhardt_query_value(query, &RValue::Uuid(Some(Box::new(uuid)))).unwrap();
		query = bind_reinhardt_query_value(
			query,
			&RValue::ChronoDateTimeUtc(Some(Box::new(timestamp))),
		)
		.unwrap();
		let row = query.fetch_one(pool.as_ref()).await.unwrap();
		assert_eq!(
			row.try_get::<String, _>("uuid_value").unwrap(),
			uuid.to_string()
		);
		assert_eq!(
			row.try_get::<String, _>("timestamp_value").unwrap(),
			timestamp.to_rfc3339()
		);
	}

	#[test]
	fn postgres_uuid_and_utc_timestamp_casts_are_parameter_side() {
		let uuid = Uuid::parse_str("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap();
		let timestamp = chrono::DateTime::parse_from_rfc3339("2026-08-19T00:00:00Z")
			.unwrap()
			.with_timezone(&chrono::Utc);
		let sql = sql_with_postgres_parameter_casts(
			DbBackend::Postgres,
			r#"UPDATE \"items\" SET \"created_at\" = $2 WHERE \"id\" = $1"#,
			&Values(vec![
				RValue::Uuid(Some(Box::new(uuid))),
				RValue::ChronoDateTimeUtc(Some(Box::new(timestamp))),
			]),
		)
		.unwrap();
		assert_eq!(
			sql.as_ref(),
			r#"UPDATE \"items\" SET \"created_at\" = $2::timestamptz WHERE \"id\" = $1::uuid"#
		);
	}

	#[test]
	fn postgres_parameter_casts_reject_placeholder_value_mismatch() {
		let error = sql_with_postgres_parameter_casts(
			DbBackend::Postgres,
			"SELECT $1",
			&Values(Vec::new()),
		)
		.unwrap_err();
		assert!(matches!(error, SessionError::InvalidState(_)));
	}

	#[rstest]
	#[case(DbBackend::Mysql)]
	#[case(DbBackend::Sqlite)]
	fn non_postgres_parameter_sql_is_unchanged(#[case] backend: DbBackend) {
		let values = Values(vec![RValue::Uuid(None)]);
		let sql = sql_with_postgres_parameter_casts(backend, "SELECT ?", &values).unwrap();
		assert_eq!(sql.as_ref(), "SELECT ?");
	}

	#[rstest]
	fn test_insert_values_error_maps_to_flush_error() {
		// Arrange
		// Create an InsertStatement with 2 columns but provide 1 value to trigger mismatch error
		let mut insert_stmt = RQuery::insert()
			.into_table(Alias::new("test_table"))
			.to_owned();
		insert_stmt.columns(vec![Alias::new("col_a"), Alias::new("col_b")]);
		let mismatched_values = vec![RValue::String(Some(Box::new("only_one".to_string())))];

		// Act
		let result: Result<(), SessionError> = insert_stmt
			.values(mismatched_values)
			.map(|_| ())
			.map_err(|e| SessionError::FlushError(format!("Failed to build INSERT values: {}", e)));

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			matches!(err, SessionError::FlushError(ref msg) if msg.contains("Failed to build INSERT values"))
		);
		assert!(err.to_string().contains("Flush error:"));
	}

	#[rstest]
	#[serial(sqlx_drivers)]
	#[tokio::test]
	async fn test_session_flush_insert_new_object_without_pk(_init_drivers: ()) {
		// Arrange
		// Test flush with a new object (no primary key) to exercise the INSERT path
		let pool = create_test_pool().await;
		let mut session = Session::new(pool, DbBackend::Sqlite).await.unwrap();

		let user = TestUser {
			id: None,
			name: "NewUser".to_string(),
			email: "newuser@example.com".to_string(),
		};

		// Act
		session.add(user).await.unwrap();
		let flush_result = session.flush().await;

		// Assert
		assert!(flush_result.is_ok());
		assert_eq!(session.dirty_count(), 0);
	}
}
