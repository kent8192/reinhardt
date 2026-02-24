//! Django-style accessor for ManyToMany relationships.
//!
//! This module provides the ManyToManyAccessor type, which implements
//! Django-style API for managing many-to-many relationships:
//! - `add()` - Add a relationship
//! - `remove()` - Remove a relationship
//! - `all()` - Get all related records
//! - `clear()` - Remove all relationships
//! - `set()` - Replace all relationships

use super::Manager;
use super::connection::{DatabaseBackend, DatabaseConnection};
use super::relationship::RelationshipType;
use crate::orm::Model;
use reinhardt_query::prelude::{
	Alias, BinOper, ColumnRef, DeleteStatement, Expr, Func, InsertStatement, MySqlQueryBuilder,
	PostgresQueryBuilder, Query, QueryBuilder, SelectStatement, SqliteQueryBuilder, Values,
};
use serde::{Serialize, de::DeserializeOwned};
use std::marker::PhantomData;

/// Build SELECT SQL using the appropriate QueryBuilder for the given backend.
fn build_select_sql(stmt: &SelectStatement, backend: DatabaseBackend) -> (String, Values) {
	match backend {
		DatabaseBackend::Postgres => PostgresQueryBuilder.build_select(stmt),
		DatabaseBackend::MySql => MySqlQueryBuilder.build_select(stmt),
		DatabaseBackend::Sqlite => SqliteQueryBuilder.build_select(stmt),
	}
}

/// Build INSERT SQL using the appropriate QueryBuilder for the given backend.
fn build_insert_sql(stmt: &InsertStatement, backend: DatabaseBackend) -> (String, Values) {
	match backend {
		DatabaseBackend::Postgres => PostgresQueryBuilder.build_insert(stmt),
		DatabaseBackend::MySql => MySqlQueryBuilder.build_insert(stmt),
		DatabaseBackend::Sqlite => SqliteQueryBuilder.build_insert(stmt),
	}
}

/// Build DELETE SQL using the appropriate QueryBuilder for the given backend.
fn build_delete_sql(stmt: &DeleteStatement, backend: DatabaseBackend) -> (String, Values) {
	match backend {
		DatabaseBackend::Postgres => PostgresQueryBuilder.build_delete(stmt),
		DatabaseBackend::MySql => MySqlQueryBuilder.build_delete(stmt),
		DatabaseBackend::Sqlite => SqliteQueryBuilder.build_delete(stmt),
	}
}

/// Django-style accessor for ManyToMany relationships.
///
/// This type provides methods to manage many-to-many relationships
/// using an intermediate/through table.
///
/// # Type Parameters
///
/// - `S`: Source model type (the model that owns the ManyToMany field)
/// - `T`: Target model type (the related model)
///
/// # Examples
///
/// ```rust,ignore
/// # #[tokio::main]
/// # async fn main() {
/// use reinhardt_db::orm::{Model, ManyToManyAccessor};
///
/// let user = User::find_by_id(&db, user_id).await?;
/// let accessor = ManyToManyAccessor::new(&user, "groups", db.clone());
///
/// // Add a relationship
/// accessor.add(&group).await?;
///
/// // Get all related records
/// let groups = accessor.all().await?;
///
/// // Remove a relationship
/// accessor.remove(&group).await?;
///
/// // Clear all relationships
/// accessor.clear().await?;
///
/// # }
/// ```
pub struct ManyToManyAccessor<S, T>
where
	S: Model,
	T: Model + Serialize + DeserializeOwned,
{
	source_id: S::PrimaryKey,
	through_table: String,
	source_field: String,
	target_field: String,
	db: DatabaseConnection,
	limit: Option<usize>,
	offset: Option<usize>,
	_phantom_source: PhantomData<S>,
	_phantom_target: PhantomData<T>,
}

impl<S, T> ManyToManyAccessor<S, T>
where
	S: Model,
	T: Model + Serialize + DeserializeOwned,
{
	/// Create a new ManyToManyAccessor.
	///
	/// # Parameters
	///
	/// - `source`: The source model instance
	/// - `field_name`: The name of the ManyToMany field
	/// - `db`: Database connection
	///
	/// # Panics
	///
	/// Panics if:
	/// - The field_name does not correspond to a ManyToMany field
	/// - The source model has no primary key
	pub fn new(source: &S, field_name: &str, db: DatabaseConnection) -> Self {
		// Try to get through table info from model metadata
		let rel_info = S::relationship_metadata()
			.into_iter()
			.find(|r| r.name == field_name && r.relationship_type == RelationshipType::ManyToMany);

		// Get through table name from metadata or use Django naming convention
		let through_table = rel_info
			.as_ref()
			.and_then(|r| r.through_table.clone())
			.unwrap_or_else(|| {
				format!(
					"{}_{}_{}",
					S::app_label(),
					Self::table_name_lower(S::table_name()),
					field_name
				)
			});

		let source_id = source
			.primary_key()
			.expect("Source model must have primary key")
			.clone();

		// Get source/target field names from metadata or use default naming
		let source_field = rel_info
			.as_ref()
			.and_then(|r| r.source_field.clone())
			.unwrap_or_else(|| format!("{}_id", Self::table_name_lower(S::table_name())));

		let target_field = rel_info
			.as_ref()
			.and_then(|r| r.target_field.clone())
			.unwrap_or_else(|| format!("{}_id", Self::table_name_lower(T::table_name())));

		Self {
			source_id,
			through_table,
			source_field,
			target_field,
			db,
			limit: None,
			offset: None,
			_phantom_source: PhantomData,
			_phantom_target: PhantomData,
		}
	}

	/// Convert table name to lowercase for field naming.
	fn table_name_lower(table_name: &str) -> String {
		table_name.to_lowercase()
	}

	/// Add a relationship to the target model.
	///
	/// Creates a record in the intermediate table linking the source and target.
	///
	/// # Parameters
	///
	/// - `target`: The target model to add
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - The target model has no primary key
	/// - The database operation fails
	///
	/// # Examples
	///
	/// ```ignore
	/// accessor.add(&group).await?;
	/// ```
	pub async fn add(&self, target: &T) -> Result<(), String> {
		let target_id = target
			.primary_key()
			.ok_or_else(|| "Target model has no primary key".to_string())?;

		let query = Query::insert()
			.into_table(Alias::new(&self.through_table))
			.columns([
				Alias::new(&self.source_field),
				Alias::new(&self.target_field),
			])
			.values_panic([
				Expr::val(self.source_id.to_string()),
				Expr::val(target_id.to_string()),
			])
			.to_owned();

		let (sql, _values) = build_insert_sql(&query, self.db.backend());

		self.db
			.execute(&sql, vec![])
			.await
			.map_err(|e| e.to_string())?;

		Ok(())
	}

	/// Remove a relationship to the target model.
	///
	/// Deletes the record in the intermediate table linking the source and target.
	///
	/// # Parameters
	///
	/// - `target`: The target model to remove
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - The target model has no primary key
	/// - The database operation fails
	///
	/// # Examples
	///
	/// ```ignore
	/// accessor.remove(&group).await?;
	/// ```
	pub async fn remove(&self, target: &T) -> Result<(), String> {
		let target_id = target
			.primary_key()
			.ok_or_else(|| "Target model has no primary key".to_string())?;

		let query = Query::delete()
			.from_table(Alias::new(&self.through_table))
			.and_where(
				Expr::col(Alias::new(&self.source_field))
					.binary(BinOper::Equal, Expr::val(self.source_id.to_string())),
			)
			.and_where(
				Expr::col(Alias::new(&self.target_field))
					.binary(BinOper::Equal, Expr::val(target_id.to_string())),
			)
			.to_owned();

		let (sql, _values) = build_delete_sql(&query, self.db.backend());

		self.db
			.execute(&sql, vec![])
			.await
			.map_err(|e| e.to_string())?;

		Ok(())
	}

	/// Set LIMIT clause
	///
	/// Limits the number of records returned by the query.
	///
	/// # Examples
	///
	/// ```ignore
	/// let followers = accessor.limit(10).all().await?;
	/// ```
	pub fn limit(mut self, limit: usize) -> Self {
		self.limit = Some(limit);
		self
	}

	/// Set OFFSET clause
	///
	/// Skips the specified number of records before returning results.
	///
	/// # Examples
	///
	/// ```ignore
	/// let followers = accessor.offset(20).limit(10).all().await?;
	/// ```
	pub fn offset(mut self, offset: usize) -> Self {
		self.offset = Some(offset);
		self
	}

	/// Paginate results using page number and page size
	///
	/// Convenience method that calculates offset automatically.
	///
	/// # Examples
	///
	/// ```ignore
	/// // Page 3, 10 items per page (offset=20, limit=10)
	/// let followers = accessor.paginate(3, 10).all().await?;
	/// ```
	pub fn paginate(self, page: usize, page_size: usize) -> Self {
		let offset = page.saturating_sub(1) * page_size;
		self.offset(offset).limit(page_size)
	}

	/// Count total number of related items
	///
	/// Executes a COUNT(*) query to get the total number of related records
	/// without fetching them.
	///
	/// # Errors
	///
	/// Returns an error if the database operation fails.
	///
	/// # Examples
	///
	/// ```ignore
	/// let total_followers = accessor.count().await?;
	/// ```
	pub async fn count(&self) -> Result<usize, String> {
		let mut query = Query::select();
		query
			.from(Alias::new(&self.through_table))
			.expr(Func::count(Expr::asterisk().into_simple_expr()))
			.and_where(
				Expr::col(Alias::new(&self.source_field))
					.binary(BinOper::Equal, Expr::val(self.source_id.to_string())),
			);

		let query = query.to_owned();
		let (sql, _) = build_select_sql(&query, self.db.backend());
		let rows = self
			.db
			.query(&sql, vec![])
			.await
			.map_err(|e| e.to_string())?;

		if let Some(row) = rows.first()
			&& let Some(count_value) = row.data.get("count")
			&& let Some(count) = count_value.as_i64()
		{
			return Ok(count as usize);
		}

		Ok(0)
	}

	/// Get all related target models.
	///
	/// Queries the target table joined with the intermediate table to fetch all
	/// related records.
	///
	/// # Errors
	///
	/// Returns an error if the database operation fails.
	///
	/// # Examples
	///
	/// ```ignore
	/// let groups = accessor.all().await?;
	/// ```
	pub async fn all(&self) -> Result<Vec<T>, String> {
		let mut query = Query::select();
		query.from(Alias::new(T::table_name()));

		// Use explicit column selection instead of SELECT * to avoid conflicts
		// with intermediate table columns in JOIN queries.
		// When JOIN is used with SELECT *, all columns from both tables are returned,
		// which can cause type conflicts (e.g., intermediate table's INTEGER id vs
		// target table's UUID id).
		let field_metadata = T::field_metadata();
		if field_metadata.is_empty() {
			// Fallback: if no field metadata is available, select all from target table only
			query.column(ColumnRef::table_asterisk(Alias::new(T::table_name())));
		} else {
			// Explicitly select only target table columns
			for field in field_metadata {
				query.column((Alias::new(T::table_name()), Alias::new(&field.name)));
			}
		}

		query
			.inner_join(
				Alias::new(&self.through_table),
				Expr::col((Alias::new(T::table_name()), Alias::new("id"))).equals((
					Alias::new(&self.through_table),
					Alias::new(&self.target_field),
				)),
			)
			.and_where(
				Expr::col((
					Alias::new(&self.through_table),
					Alias::new(&self.source_field),
				))
				.binary(BinOper::Equal, Expr::val(self.source_id.to_string())),
			);

		// Apply LIMIT/OFFSET
		if let Some(limit) = self.limit {
			query.limit(limit as u64);
		}
		if let Some(offset) = self.offset {
			query.offset(offset as u64);
		}

		let query = query.to_owned();
		let (sql, _values) = build_select_sql(&query, self.db.backend());

		let rows = self
			.db
			.query(&sql, vec![])
			.await
			.map_err(|e| e.to_string())?;

		rows.into_iter()
			.map(|row| serde_json::from_value(row.data).map_err(|e| e.to_string()))
			.collect()
	}

	/// Remove all relationships.
	///
	/// Deletes all records in the intermediate table for this source instance.
	///
	/// # Errors
	///
	/// Returns an error if the database operation fails.
	///
	/// # Examples
	///
	/// ```ignore
	/// accessor.clear().await?;
	/// ```
	pub async fn clear(&self) -> Result<(), String> {
		let query = Query::delete()
			.from_table(Alias::new(&self.through_table))
			.and_where(
				Expr::col(Alias::new(&self.source_field))
					.binary(BinOper::Equal, Expr::val(self.source_id.to_string())),
			)
			.to_owned();

		let (sql, _values) = build_delete_sql(&query, self.db.backend());

		self.db
			.execute(&sql, vec![])
			.await
			.map_err(|e| e.to_string())?;

		Ok(())
	}

	/// Replace all relationships with a new set.
	///
	/// This is a transactional operation that:
	/// 1. Removes all existing relationships
	/// 2. Adds new relationships
	///
	/// # Parameters
	///
	/// - `targets`: The new set of target models
	///
	/// # Errors
	///
	/// Returns an error if the database operation fails.
	///
	/// # Examples
	///
	/// ```ignore
	/// accessor.set(&[group1, group2, group3]).await?;
	/// ```
	pub async fn set(&self, targets: &[T]) -> Result<(), String> {
		// Use transaction for atomicity
		let mut tx = self.db.begin().await.map_err(|e| e.to_string())?;
		let backend = self.db.backend();

		// Build and execute clear query within transaction
		let clear_query = Query::delete()
			.from_table(Alias::new(&self.through_table))
			.and_where(
				Expr::col(Alias::new(&self.source_field))
					.binary(BinOper::Equal, Expr::val(self.source_id.to_string())),
			)
			.to_owned();
		let (clear_sql, _) = build_delete_sql(&clear_query, backend);
		tx.execute(&clear_sql, vec![])
			.await
			.map_err(|e| e.to_string())?;

		// Add new relationships within transaction
		for target in targets {
			let target_id = target
				.primary_key()
				.ok_or_else(|| "Target model has no primary key".to_string())?;

			let insert_query = Query::insert()
				.into_table(Alias::new(&self.through_table))
				.columns([
					Alias::new(&self.source_field),
					Alias::new(&self.target_field),
				])
				.values_panic([
					Expr::val(self.source_id.to_string()),
					Expr::val(target_id.to_string()),
				])
				.to_owned();

			let (insert_sql, _) = build_insert_sql(&insert_query, backend);
			tx.execute(&insert_sql, vec![])
				.await
				.map_err(|e| e.to_string())?;
		}

		// Commit transaction
		tx.commit().await.map_err(|e| e.to_string())?;

		Ok(())
	}

	/// Filter source models by target model via many-to-many relationship
	///
	/// Returns all source model instances that have a relationship with the given target.
	/// This is more efficient than loading all source instances and checking relationships
	/// individually, as it uses a single JOIN query.
	///
	/// # Type Parameters
	///
	/// - `S`: Source model type (the model that owns the ManyToMany field)
	/// - `T`: Target model type (the related model)
	///
	/// # Arguments
	///
	/// - `source_manager`: Manager for the source model
	/// - `field_name`: Name of the ManyToMany field on the source model
	/// - `target`: The target model instance to filter by
	/// - `db`: Database connection
	///
	/// # Returns
	///
	/// All source model instances related to the target
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - The target model has no primary key
	/// - The database operation fails
	/// - The query results cannot be deserialized
	///
	/// # Examples
	///
	/// ```ignore
	/// // Find all rooms where a specific user is a member
	/// let user = User::find_by_id(&db, user_id).await?;
	/// let rooms = ManyToManyAccessor::<DMRoom, User>::filter_by_target(
	///     &DMRoom::objects(),
	///     "members",
	///     &user,
	///     db.clone()
	/// ).await?;
	/// ```
	///
	/// SQL equivalent:
	/// ```sql
	/// SELECT source_table.*
	/// FROM source_table
	/// INNER JOIN through_table ON source_table.id = through_table.source_id
	/// WHERE through_table.target_id = $1
	/// ```
	pub async fn filter_by_target(
		_source_manager: &Manager<S>,
		field_name: &str,
		target: &T,
		db: DatabaseConnection,
	) -> Result<Vec<S>, String> {
		let target_id = target
			.primary_key()
			.ok_or_else(|| "Target model has no primary key".to_string())?;

		// Calculate through table name (same logic as new())
		let through_table = format!("{}_{}", Self::table_name_lower(S::table_name()), field_name);

		let source_field = format!("{}_id", Self::table_name_lower(S::table_name()));
		let target_field = format!("{}_id", Self::table_name_lower(T::table_name()));

		// Build JOIN query using reinhardt-query
		let mut query = Query::select();
		query.from(Alias::new(S::table_name()));

		// Use explicit column selection instead of SELECT * to avoid conflicts
		// with intermediate table columns in JOIN queries.
		let field_metadata = S::field_metadata();
		if field_metadata.is_empty() {
			query.column(ColumnRef::table_asterisk(Alias::new(S::table_name())));
		} else {
			for field in field_metadata {
				query.column((Alias::new(S::table_name()), Alias::new(&field.name)));
			}
		}

		let query = query
			.inner_join(
				Alias::new(&through_table),
				Expr::col((Alias::new(S::table_name()), Alias::new("id")))
					.equals((Alias::new(&through_table), Alias::new(&source_field))),
			)
			.and_where(
				Expr::col((Alias::new(&through_table), Alias::new(&target_field)))
					.binary(BinOper::Equal, Expr::val(target_id.to_string())),
			)
			.to_owned();

		let (sql, _values) = build_select_sql(&query, db.backend());

		let rows = db.query(&sql, vec![]).await.map_err(|e| e.to_string())?;

		rows.into_iter()
			.map(|row| serde_json::from_value(row.data).map_err(|e| e.to_string()))
			.collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_query::prelude::QueryStatementBuilder;

	#[test]
	fn test_table_name_lower() {
		assert_eq!(
			ManyToManyAccessor::<TestUser, TestGroup>::table_name_lower("Users"),
			"users"
		);
		assert_eq!(
			ManyToManyAccessor::<TestUser, TestGroup>::table_name_lower("UserGroups"),
			"usergroups"
		);
	}

	#[test]
	fn test_sql_generation_add() {
		// Test that INSERT SQL is generated correctly
		let query = Query::insert()
			.into_table(Alias::new("auth_users_groups"))
			.columns([Alias::new("users_id"), Alias::new("groups_id")])
			.values_panic([Expr::val("1"), Expr::val("10")])
			.to_owned();

		let (sql, _) = query.build(PostgresQueryBuilder);
		assert!(sql.contains("INSERT INTO"));
		assert!(sql.contains("auth_users_groups"));
		assert!(sql.contains("users_id"));
		assert!(sql.contains("groups_id"));
	}

	#[test]
	fn test_sql_generation_remove() {
		// Test that DELETE SQL is generated correctly
		let query = Query::delete()
			.from_table(Alias::new("auth_users_groups"))
			.and_where(Expr::col(Alias::new("users_id")).binary(BinOper::Equal, Expr::val("1")))
			.and_where(Expr::col(Alias::new("groups_id")).binary(BinOper::Equal, Expr::val("10")))
			.to_owned();

		let (sql, _) = query.build(PostgresQueryBuilder);
		assert!(sql.contains("DELETE FROM"));
		assert!(sql.contains("auth_users_groups"));
		assert!(sql.contains("users_id"));
		assert!(sql.contains("groups_id"));
	}

	#[test]
	fn test_sql_generation_clear() {
		// Test that DELETE SQL for clear is generated correctly
		let query = Query::delete()
			.from_table(Alias::new("auth_users_groups"))
			.and_where(Expr::col(Alias::new("users_id")).binary(BinOper::Equal, Expr::val("1")))
			.to_owned();

		let (sql, _) = query.build(PostgresQueryBuilder);
		assert!(sql.contains("DELETE FROM"));
		assert!(sql.contains("auth_users_groups"));
		assert!(sql.contains("users_id"));
	}

	#[test]
	fn test_sql_generation_all() {
		// Test that SELECT SQL with JOIN is generated correctly
		let query = Query::select()
			.from(Alias::new("groups"))
			.column((Alias::new("groups"), Alias::new("*")))
			.inner_join(
				Alias::new("auth_users_groups"),
				Expr::col((Alias::new("groups"), Alias::new("id")))
					.equals((Alias::new("auth_users_groups"), Alias::new("groups_id"))),
			)
			.and_where(
				Expr::col((Alias::new("auth_users_groups"), Alias::new("users_id")))
					.binary(BinOper::Equal, Expr::val("1")),
			)
			.to_owned();

		let (sql, _) = query.build(PostgresQueryBuilder);
		assert!(sql.contains("SELECT"));
		assert!(sql.contains("INNER JOIN"));
		assert!(sql.contains("auth_users_groups"));
	}

	#[test]
	fn test_sql_generation_filter_by_target() {
		// Test that SELECT SQL with JOIN for filter_by_target is generated correctly
		let query = Query::select()
			.from(Alias::new("dm_room"))
			.column((Alias::new("dm_room"), Alias::new("*")))
			.inner_join(
				Alias::new("dm_room_members"),
				Expr::col((Alias::new("dm_room"), Alias::new("id")))
					.equals((Alias::new("dm_room_members"), Alias::new("dmroom_id"))),
			)
			.and_where(
				Expr::col((Alias::new("dm_room_members"), Alias::new("user_id")))
					.binary(BinOper::Equal, Expr::val("test-user-id")),
			)
			.to_owned();

		let (sql, _) = query.build(PostgresQueryBuilder);
		assert!(sql.contains("SELECT"));
		assert!(sql.contains("dm_room"));
		assert!(sql.contains("INNER JOIN"));
		assert!(sql.contains("dm_room_members"));
		assert!(sql.contains("user_id"));
		// Note: reinhardt-query uses parameterized queries, so the value may be in a parameter
		// instead of inline in the SQL string
	}

	// Test models for SQL generation tests
	#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	struct TestUser {
		id: i64,
		username: String,
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

		fn app_label() -> &'static str {
			"auth"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			Some(self.id)
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = value;
		}

		fn primary_key_field() -> &'static str {
			"id"
		}

		fn new_fields() -> Self::Fields {
			TestUserFields
		}
	}

	#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	struct TestGroup {
		id: i64,
		name: String,
	}

	#[derive(Clone)]
	struct TestGroupFields;
	impl crate::orm::model::FieldSelector for TestGroupFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for TestGroup {
		type PrimaryKey = i64;
		type Fields = TestGroupFields;

		fn table_name() -> &'static str {
			"groups"
		}

		fn app_label() -> &'static str {
			"auth"
		}

		fn new_fields() -> Self::Fields {
			TestGroupFields
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			Some(self.id)
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = value;
		}
	}
}
