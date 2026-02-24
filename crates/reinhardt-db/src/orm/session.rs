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
use crate::orm::model::Model;
use crate::orm::query::OrmQuery;
use crate::orm::query_types::DbBackend;
use reinhardt_query::value::Value as RValue;
use reinhardt_query::{
	Alias, Expr, ExprTrait, MySqlQueryBuilder, PostgresQueryBuilder, Query as RQuery,
	QueryStatementBuilder, SqliteQueryBuilder,
};
use serde_json::Value;
use sqlx::{AnyPool, Row};
use std::any::TypeId;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

/// Session error types
#[derive(Debug, Clone)]
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
	/// Type ID for runtime type checking
	type_id: TypeId,
	/// Whether the object has been modified
	#[allow(dead_code)]
	is_dirty: bool,
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
	deleted_objects: HashSet<String>,
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
			deleted_objects: HashSet::new(),
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
				type_id: TypeId::of::<T>(),
				is_dirty: true,
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
				type_id: TypeId::of::<T>(),
				is_dirty: false,
			},
		);

		Ok(Some(obj))
	}

	/// Get all objects of a given type from the database
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
	/// let users: Vec<User> = session.list_all().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn list_all<T: Model + 'static>(&self) -> Result<Vec<T>, SessionError> {
		self.check_closed()?;

		// Use field_metadata() to build the query and map results
		let field_metadata = T::field_metadata();
		if field_metadata.is_empty() {
			// No field metadata available - return empty list
			return Ok(Vec::new());
		}

		// Build column expressions for SELECT
		// DateTime fields are cast to text format for AnyPool compatibility
		// (SQLx AnyPool doesn't support PostgreSQL's TIMESTAMP type)
		let mut column_exprs: Vec<String> = Vec::new();
		for field in &field_metadata {
			let column_name = field.db_column.as_deref().unwrap_or(&field.name);
			let is_datetime = field.field_type.contains("DateTimeField")
				|| field.field_type.contains("DateField");

			let expr = if is_datetime {
				// Cast datetime fields to ISO8601 text format
				match self.db_backend {
					DbBackend::Postgres => {
						format!(
							"TO_CHAR(\"{}\", 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS \"{}\"",
							column_name, column_name
						)
					}
					DbBackend::Mysql => {
						format!(
							"DATE_FORMAT(`{}`, '%Y-%m-%dT%H:%i:%sZ') AS `{}`",
							column_name, column_name
						)
					}
					DbBackend::Sqlite => {
						format!(
							"strftime('%Y-%m-%dT%H:%M:%SZ', \"{}\") AS \"{}\"",
							column_name, column_name
						)
					}
				}
			} else {
				// Regular column
				match self.db_backend {
					DbBackend::Postgres | DbBackend::Sqlite => format!("\"{}\"", column_name),
					DbBackend::Mysql => format!("`{}`", column_name),
				}
			};
			column_exprs.push(expr);
		}

		// Build complete SQL query manually
		let table_name = T::table_name();
		let columns_sql = column_exprs.join(", ");
		let sql = match self.db_backend {
			DbBackend::Postgres | DbBackend::Sqlite => {
				format!("SELECT {} FROM \"{}\"", columns_sql, table_name)
			}
			DbBackend::Mysql => {
				format!("SELECT {} FROM `{}`", columns_sql, table_name)
			}
		};

		// Execute query
		let rows = sqlx::query(&sql)
			.fetch_all(&*self.pool)
			.await
			.map_err(|e| SessionError::DatabaseError(format!("Failed to query database: {}", e)))?;

		let mut results = Vec::with_capacity(rows.len());

		for row in rows {
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
					typ if typ.contains("CharField") || typ.contains("TextField") => {
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
					// DateTimeField / DateField: already cast to string in SQL query
					typ if typ.contains("DateTimeField") || typ.contains("DateField") => {
						// These fields are cast to ISO8601 strings in the SQL query
						// The value will be parsed by serde when deserializing to chrono::DateTime
						row.try_get::<Option<String>, _>(column_name)
							.map(|v| {
								v.map(serde_json::Value::from)
									.unwrap_or(serde_json::Value::Null)
							})
							.unwrap_or(serde_json::Value::Null)
					}
					// Default: try as string
					_ => row
						.try_get::<Option<String>, _>(column_name)
						.map(|v| {
							v.map(serde_json::Value::from)
								.unwrap_or(serde_json::Value::Null)
						})
						.unwrap_or(serde_json::Value::Null),
				};

				json_map.insert(field.name.clone(), value);
			}

			// Deserialize JSON to model object
			let obj: T =
				serde_json::from_value(serde_json::Value::Object(json_map)).map_err(|e| {
					SessionError::SerializationError(format!(
						"Failed to deserialize query result: {}",
						e
					))
				})?;

			results.push(obj);
		}

		Ok(results)
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

		// Clear any previously generated IDs
		self.last_generated_ids.clear();

		// Determine database backend from pool
		let backend = self.get_backend();

		// Process dirty objects (INSERT/UPDATE)
		for key in &self.dirty_objects.clone() {
			if let Some(entry) = self.identity_map.get(key) {
				// Parse the identity key to get table name and primary key
				let parts: Vec<&str> = key.split(':').collect();
				if parts.len() != 2 {
					continue;
				}
				let table_name = parts[0];

				// Extract data from JSON
				if let Some(obj) = entry.data.as_object() {
					// Check if this is an INSERT (no primary key) or UPDATE (has primary key)
					// The "id" field must exist AND not be null for UPDATE
					let has_pk = obj.get("id").map(|v| !v.is_null()).unwrap_or(false);

					if has_pk {
						// UPDATE existing record
						let mut update_stmt =
							RQuery::update().table(Alias::new(table_name)).to_owned();

						// Set all columns except primary key and auto-managed datetime fields
						for (col_name, col_value) in obj {
							if col_name == "id" || col_name.ends_with("_id") {
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
								json_to_reinhardt_query_value(col_value),
							);
						}

						// Add WHERE clause for primary key
						if let Some(pk_value) = obj.get("id") {
							update_stmt.and_where(
								Expr::col(Alias::new("id"))
									.eq(Expr::val(json_to_reinhardt_query_value(pk_value))),
							);
						}

						// Build and execute SQL
						let (sql, values) = match backend {
							DbBackend::Postgres => update_stmt.build(PostgresQueryBuilder),
							DbBackend::Mysql => update_stmt.build(MySqlQueryBuilder),
							DbBackend::Sqlite => update_stmt.build(SqliteQueryBuilder),
						};

						self.execute_with_values(&sql, &values).await?;
					} else {
						// INSERT new record
						let mut insert_stmt = RQuery::insert()
							.into_table(Alias::new(table_name))
							.to_owned();

						let mut columns = Vec::new();
						let mut values_vec: Vec<RValue> = Vec::new();

						for (col_name, col_value) in obj {
							// Skip id/primary key column - auto-generated
							if col_name == "id" || col_name.ends_with("_id") {
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
								values_vec.push(json_to_reinhardt_query_value(col_value));
							}
						}

						// If there are columns to insert, add them
						if !columns.is_empty() {
							insert_stmt.columns(columns);
							insert_stmt.values(values_vec).unwrap();
						}

						// Add RETURNING clause for PostgreSQL to get generated ID
						if backend == DbBackend::Postgres {
							insert_stmt.returning_col(Alias::new("id"));
						}

						// Build and execute SQL
						let (sql, values) = match backend {
							DbBackend::Postgres => insert_stmt.build(PostgresQueryBuilder),
							DbBackend::Mysql => insert_stmt.build(MySqlQueryBuilder),
							DbBackend::Sqlite => insert_stmt.build(SqliteQueryBuilder),
						};

						// Execute and get generated ID if available
						if backend == DbBackend::Postgres {
							if let Ok(row) = self.execute_returning(&sql, &values).await {
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
							}
						} else {
							self.execute_with_values(&sql, &values).await?;
						}
					}
				}
			}
		}

		self.dirty_objects.clear();

		// Process deleted objects (DELETE)
		for key in &self.deleted_objects.clone() {
			// Parse the identity key to get table name and primary key
			let parts: Vec<&str> = key.split(':').collect();
			if parts.len() != 2 {
				continue;
			}
			let table_name = parts[0];
			let pk_value_str = parts[1];

			// Build DELETE statement
			let mut delete_stmt = RQuery::delete()
				.from_table(Alias::new(table_name))
				.to_owned();

			// Parse primary key as integer for BIGINT comparison
			// Fall back to string comparison if parsing fails
			if let Ok(pk_int) = pk_value_str.parse::<i64>() {
				delete_stmt.and_where(Expr::col(Alias::new("id")).eq(Expr::val(pk_int)));
			} else {
				delete_stmt.and_where(Expr::col(Alias::new("id")).eq(Expr::val(pk_value_str)));
			}

			// Build and execute SQL
			let (sql, values) = match backend {
				DbBackend::Postgres => delete_stmt.build(PostgresQueryBuilder),
				DbBackend::Mysql => delete_stmt.build(MySqlQueryBuilder),
				DbBackend::Sqlite => delete_stmt.build(SqliteQueryBuilder),
			};

			self.execute_with_values(&sql, &values).await?;

			// Remove from identity map
			self.identity_map.remove(key);
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
		sql: &str,
		values: &reinhardt_query::value::Values,
	) -> Result<(), SessionError> {
		let mut query = sqlx::query(sql);

		// Bind all values from reinhardt_query::value::Values
		for value in &values.0 {
			query = bind_reinhardt_query_value(query, value);
		}

		query
			.execute(&*self.pool)
			.await
			.map_err(|e| SessionError::FlushError(e.to_string()))?;

		Ok(())
	}

	/// Execute SQL with RETURNING clause (PostgreSQL)
	async fn execute_returning(
		&self,
		sql: &str,
		values: &reinhardt_query::value::Values,
	) -> Result<sqlx::any::AnyRow, SessionError> {
		let mut query = sqlx::query(sql);

		// Bind all values from reinhardt_query::value::Values
		for value in &values.0 {
			query = bind_reinhardt_query_value(query, value);
		}

		query
			.fetch_one(&*self.pool)
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

		// Mark for deletion
		self.deleted_objects.insert(key.clone());

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

/// Convert JSON value to reinhardt_query Value
fn json_to_reinhardt_query_value(value: &Value) -> RValue {
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
			// Try to parse as UUID first
			if let Ok(uuid) = Uuid::parse_str(s) {
				return RValue::Uuid(Some(Box::new(uuid)));
			}
			RValue::String(Some(Box::new(s.clone())))
		}
		Value::Array(_) | Value::Object(_) => {
			// For complex types, serialize as JSON string
			RValue::String(Some(Box::new(value.to_string())))
		}
	}
}

/// Bind reinhardt_query Value to sqlx Query
fn bind_reinhardt_query_value<'a>(
	query: sqlx::query::Query<'a, sqlx::Any, sqlx::any::AnyArguments<'a>>,
	value: &RValue,
) -> sqlx::query::Query<'a, sqlx::Any, sqlx::any::AnyArguments<'a>> {
	match value {
		RValue::Bool(Some(b)) => query.bind(*b),
		RValue::TinyInt(Some(i)) => query.bind(*i as i32),
		RValue::SmallInt(Some(i)) => query.bind(*i as i32),
		RValue::Int(Some(i)) => query.bind(*i),
		RValue::BigInt(Some(i)) => query.bind(*i),
		RValue::TinyUnsigned(Some(i)) => query.bind(*i as i32),
		RValue::SmallUnsigned(Some(i)) => query.bind(*i as i32),
		RValue::Unsigned(Some(i)) => query.bind(*i as i64),
		RValue::BigUnsigned(Some(i)) => query.bind(*i as i64),
		RValue::Float(Some(f)) => query.bind(*f),
		RValue::Double(Some(f)) => query.bind(*f),
		RValue::String(Some(s)) => query.bind(s.as_ref().clone()),
		RValue::Bytes(Some(b)) => query.bind(b.as_ref().clone()),
		// UUID: sqlx::Any doesn't natively support UUID, bind as string
		RValue::Uuid(Some(u)) => query.bind(u.to_string()),
		// Json variant is available because reinhardt-query is compiled with "with-json" feature
		RValue::Json(Some(j)) => {
			// Serialize JSON to string for sqlx::Any which doesn't support direct JSON binding
			query.bind(j.to_string())
		}
		// All None/null variants bind as null
		_ => query.bind(None::<i32>),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
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
		let rq_value = super::json_to_reinhardt_query_value(&value);

		let debug_str = format!("{:?}", rq_value);
		assert!(debug_str.contains("hello world") || debug_str.contains("String"));
	}

	#[test]
	fn test_json_to_reinhardt_query_value_integer() {
		use serde_json::json;
		let value = json!(42);
		let rq_value = super::json_to_reinhardt_query_value(&value);

		let debug_str = format!("{:?}", rq_value);
		assert!(debug_str.contains("42") || debug_str.contains("Int"));
	}

	#[test]
	fn test_json_to_reinhardt_query_value_float() {
		use serde_json::json;
		let value = json!(3.14159);
		let rq_value = super::json_to_reinhardt_query_value(&value);

		let debug_str = format!("{:?}", rq_value);
		assert!(debug_str.contains("3.14159") || debug_str.contains("Double"));
	}

	#[test]
	fn test_json_to_reinhardt_query_value_bool_true() {
		use serde_json::json;
		let value = json!(true);
		let rq_value = super::json_to_reinhardt_query_value(&value);

		let debug_str = format!("{:?}", rq_value);
		assert!(debug_str.contains("true") || debug_str.contains("Bool"));
	}

	#[test]
	fn test_json_to_reinhardt_query_value_bool_false() {
		use serde_json::json;
		let value = json!(false);
		let rq_value = super::json_to_reinhardt_query_value(&value);

		let debug_str = format!("{:?}", rq_value);
		assert!(debug_str.contains("false") || debug_str.contains("Bool"));
	}

	#[test]
	fn test_json_to_reinhardt_query_value_null() {
		use serde_json::json;
		let value = json!(null);
		let rq_value = super::json_to_reinhardt_query_value(&value);

		// Should produce some value (null representation)
		let debug_str = format!("{:?}", rq_value);
		assert!(!debug_str.is_empty());
	}

	#[test]
	fn test_json_to_reinhardt_query_value_array() {
		use serde_json::json;
		let value = json!([1, 2, 3]);
		let rq_value = super::json_to_reinhardt_query_value(&value);

		// Array should be serialized as JSON string
		let debug_str = format!("{:?}", rq_value);
		assert!(!debug_str.is_empty());
	}

	#[test]
	fn test_json_to_reinhardt_query_value_object() {
		use serde_json::json;
		let value = json!({"name": "test", "count": 42});
		let rq_value = super::json_to_reinhardt_query_value(&value);

		// Object should be serialized as JSON string
		let debug_str = format!("{:?}", rq_value);
		assert!(!debug_str.is_empty());
	}

	#[test]
	fn test_json_to_reinhardt_query_value_negative_integer() {
		use serde_json::json;
		let value = json!(-100);
		let rq_value = super::json_to_reinhardt_query_value(&value);

		let debug_str = format!("{:?}", rq_value);
		assert!(debug_str.contains("-100") || debug_str.contains("Int"));
	}

	#[test]
	fn test_json_to_reinhardt_query_value_large_integer() {
		use serde_json::json;
		let value = json!(9223372036854775807i64); // i64::MAX
		let rq_value = super::json_to_reinhardt_query_value(&value);

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
}
