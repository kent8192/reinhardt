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
use crate::m2m_naming::{default_m2m_columns, default_through_table};
use crate::orm::Model;
use reinhardt_query::prelude::{
	Alias, BinOper, ColumnRef, DeleteStatement, Expr, Func, InsertStatement, MySqlQueryBuilder,
	PostgresQueryBuilder, Query, QueryBuilder, SelectStatement, SqliteQueryBuilder, Values,
};
use serde::{Serialize, de::DeserializeOwned};
use std::marker::PhantomData;
use std::time::Instant;

/// Build SELECT SQL using the appropriate QueryBuilder for the given backend.
fn build_select_sql(stmt: &SelectStatement, backend: DatabaseBackend) -> (String, Values) {
	match backend {
		DatabaseBackend::Postgres => PostgresQueryBuilder.build_select(stmt),
		DatabaseBackend::MySql => MySqlQueryBuilder.build_select(stmt),
		DatabaseBackend::Sqlite => SqliteQueryBuilder.build_select(stmt),
	}
}

fn value_samples(values: &Values) -> Vec<String> {
	values.iter().map(|value| value.to_sql_literal()).collect()
}

fn primary_key_value<M: Model>(primary_key: M::PrimaryKey) -> reinhardt_query::value::Value {
	let filter_value = M::primary_key_filter_value(primary_key);
	super::query::QuerySet::<M>::filter_value_to_sea_value(&filter_value)
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

		// Get through table name and FK column names from metadata, falling
		// back to the canonical convention defined in `crate::m2m_naming`
		// (single source of truth shared with the migration autodetector;
		// see issues #4659 and #4665). `default_m2m_columns` applies the
		// `from_/to_` prefix only for self-referential M2M, matching what
		// `MigrationAutodetector::create_intermediate_table_for_m2m` emits.
		let through_table = rel_info
			.as_ref()
			.and_then(|r| r.through_table.clone())
			.unwrap_or_else(|| default_through_table(S::table_name(), field_name));

		let source_id = source
			.primary_key()
			.expect("Source model must have primary key")
			.clone();

		let (default_source_field, default_target_field) =
			default_m2m_columns(S::table_name(), T::table_name());
		let source_field = rel_info
			.as_ref()
			.and_then(|r| r.source_field.clone())
			.unwrap_or(default_source_field);

		let target_field = rel_info
			.as_ref()
			.and_then(|r| r.target_field.clone())
			.unwrap_or(default_target_field);

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
				Expr::val(primary_key_value::<S>(self.source_id.clone())),
				Expr::val(primary_key_value::<T>(target_id)),
			])
			.to_owned();

		let (sql, values) = build_insert_sql(&query, self.db.backend());

		self.db
			.execute(&sql, super::execution::convert_values(values))
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
			.and_where(Expr::col(Alias::new(&self.source_field)).binary(
				BinOper::Equal,
				Expr::val(primary_key_value::<S>(self.source_id.clone())),
			))
			.and_where(
				Expr::col(Alias::new(&self.target_field))
					.binary(BinOper::Equal, Expr::val(primary_key_value::<T>(target_id))),
			)
			.to_owned();

		let (sql, values) = build_delete_sql(&query, self.db.backend());

		self.db
			.execute(&sql, super::execution::convert_values(values))
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
			.expr_as(
				Func::count(Expr::asterisk().into_simple_expr()),
				Alias::new("count"),
			)
			.and_where(Expr::col(Alias::new(&self.source_field)).binary(
				BinOper::Equal,
				Expr::val(primary_key_value::<S>(self.source_id.clone())),
			));

		let query = query.to_owned();
		let (sql, values) = build_select_sql(&query, self.db.backend());
		let params = value_samples(&values);
		let started_at = Instant::now();
		let query_result = self
			.db
			.query(&sql, super::execution::convert_values(values))
			.await;
		let duration = started_at.elapsed();
		let rows = match query_result {
			Ok(rows) => {
				super::instrumentation::instrumentation()
					.orm_query_end_with_params(&sql, &params, duration)
					.await;
				rows
			}
			Err(error) => return Err(error.to_string()),
		};

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
				.binary(
					BinOper::Equal,
					Expr::val(primary_key_value::<S>(self.source_id.clone())),
				),
			);

		// Apply LIMIT/OFFSET
		if let Some(limit) = self.limit {
			query.limit(limit as u64);
		}
		if let Some(offset) = self.offset {
			query.offset(offset as u64);
		}

		let query = query.to_owned();
		let (sql, values) = build_select_sql(&query, self.db.backend());
		let params = value_samples(&values);
		let started_at = Instant::now();
		let query_result = self
			.db
			.query(&sql, super::execution::convert_values(values))
			.await;
		let duration = started_at.elapsed();
		let rows = match query_result {
			Ok(rows) => {
				super::instrumentation::instrumentation()
					.orm_query_end_with_params(&sql, &params, duration)
					.await;
				rows
			}
			Err(error) => return Err(error.to_string()),
		};

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
			.and_where(Expr::col(Alias::new(&self.source_field)).binary(
				BinOper::Equal,
				Expr::val(primary_key_value::<S>(self.source_id.clone())),
			))
			.to_owned();

		let (sql, values) = build_delete_sql(&query, self.db.backend());

		self.db
			.execute(&sql, super::execution::convert_values(values))
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
			.and_where(Expr::col(Alias::new(&self.source_field)).binary(
				BinOper::Equal,
				Expr::val(primary_key_value::<S>(self.source_id.clone())),
			))
			.to_owned();
		let (clear_sql, clear_values) = build_delete_sql(&clear_query, backend);
		tx.execute(&clear_sql, super::execution::convert_values(clear_values))
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
					Expr::val(primary_key_value::<S>(self.source_id.clone())),
					Expr::val(primary_key_value::<T>(target_id)),
				])
				.to_owned();

			let (insert_sql, insert_values) = build_insert_sql(&insert_query, backend);
			tx.execute(&insert_sql, super::execution::convert_values(insert_values))
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

		// Resolve through-table and FK column names through the same
		// metadata-aware path as `new()`, routing the fallbacks through
		// `crate::m2m_naming` (single source of truth shared with the
		// migration autodetector; see issues #4659, #4665). The helpers
		// apply `from_/to_` prefixes for self-referential M2M, matching
		// `MigrationAutodetector::create_intermediate_table_for_m2m`.
		let rel_info = S::relationship_metadata()
			.into_iter()
			.find(|r| r.name == field_name && r.relationship_type == RelationshipType::ManyToMany);

		let through_table = rel_info
			.as_ref()
			.and_then(|r| r.through_table.clone())
			.unwrap_or_else(|| default_through_table(S::table_name(), field_name));

		let (default_source_field, default_target_field) =
			default_m2m_columns(S::table_name(), T::table_name());
		let source_field = rel_info
			.as_ref()
			.and_then(|r| r.source_field.clone())
			.unwrap_or(default_source_field);
		let target_field = rel_info
			.as_ref()
			.and_then(|r| r.target_field.clone())
			.unwrap_or(default_target_field);

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
					.binary(BinOper::Equal, Expr::val(primary_key_value::<T>(target_id))),
			)
			.to_owned();

		let (sql, values) = build_select_sql(&query, db.backend());
		let params = value_samples(&values);
		let started_at = Instant::now();
		let query_result = db
			.query(&sql, super::execution::convert_values(values))
			.await;
		let duration = started_at.elapsed();
		let rows = match query_result {
			Ok(rows) => {
				super::instrumentation::instrumentation()
					.orm_query_end_with_params(&sql, &params, duration)
					.await;
				rows
			}
			Err(error) => return Err(error.to_string()),
		};

		rows.into_iter()
			.map(|row| serde_json::from_value(row.data).map_err(|e| e.to_string()))
			.collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::orm::model::FieldSelector;
	use reinhardt_query::prelude::QueryStatementBuilder;

	/// Regression test for #4659: the runtime accessor's default
	/// through-table name MUST agree with the name `makemigrations`
	/// synthesizes. The autodetector uses
	/// `format!("{}_{}", source_model.table_name, field_name)`; the
	/// accessor must do the same. Previously it prepended `S::app_label()`
	/// as a separate segment, producing names like `auth_users_groups`
	/// instead of `users_groups` (or `dm_dm_room_members` instead of
	/// `dm_room_members`), so runtime M2M queries targeted a table that
	/// `makemigrations` never created.
	#[test]
	fn default_through_table_matches_autodetector_convention() {
		// Arrange / Act: TestUser has table_name = "users". The accessor now
		// routes through `crate::m2m_naming::default_through_table`, so this
		// regression test exercises the same helper the autodetector uses.
		let through = default_through_table(TestUser::table_name(), "members");

		// Assert
		assert_eq!(through, "users_members");
		assert!(
			!through.starts_with("auth_"),
			"app_label must NOT be prepended; that would double-count it \
			 when table_name already carries the prefix (e.g. \"dm_room\"). \
			 See #4659 for the breakage this causes."
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

	#[test]
	fn postgres_builder_preserves_integer_primary_key_values() {
		let query = Query::insert()
			.into_table(Alias::new("auth_users_groups"))
			.columns([Alias::new("users_id"), Alias::new("groups_id")])
			.values_panic([
				Expr::val(primary_key_value::<TestUser>(42)),
				Expr::val(primary_key_value::<TestGroup>(7)),
			])
			.to_owned();

		let (_, values) = build_insert_sql(&query, DatabaseBackend::Postgres);

		assert_eq!(
			values.0,
			vec![
				reinhardt_query::value::Value::BigInt(Some(42)),
				reinhardt_query::value::Value::BigInt(Some(7)),
			]
		);
	}

	#[test]
	fn postgres_builder_preserves_uuid_primary_key_value() {
		let id = uuid::Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000")
			.expect("UUID literal should be valid");
		let query = Query::delete()
			.from_table(Alias::new("groups_members"))
			.and_where(Expr::col(Alias::new("groups_id")).binary(
				BinOper::Equal,
				Expr::val(primary_key_value::<TestUuidGroup>(id)),
			))
			.to_owned();

		let (_, values) = build_delete_sql(&query, DatabaseBackend::Postgres);

		assert!(matches!(
			values.0.as_slice(),
			[reinhardt_query::value::Value::Uuid(Some(value))] if **value == id
		));
	}

	#[cfg(feature = "sqlite")]
	#[tokio::test]
	async fn sqlite_accessor_executes_all_relationship_queries_with_bound_values() {
		let db = DatabaseConnection::connect_sqlite("sqlite::memory:")
			.await
			.expect("in-memory SQLite connection should be available");
		for statement in [
			"CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT NOT NULL)",
			"CREATE TABLE groups (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
			"CREATE TABLE users_groups (users_id INTEGER NOT NULL, groups_id INTEGER NOT NULL, PRIMARY KEY (users_id, groups_id))",
			"INSERT INTO users (id, username) VALUES (1, 'ada')",
			"INSERT INTO groups (id, name) VALUES (1, 'readers'), (2, 'writers')",
		] {
			db.execute(statement, Vec::new())
				.await
				.expect("SQLite relationship table should be created");
		}

		let user = TestUser {
			id: 1,
			username: "ada".to_string(),
		};
		let groups = [
			TestGroup {
				id: 1,
				name: "readers".to_string(),
			},
			TestGroup {
				id: 2,
				name: "writers".to_string(),
			},
		];
		let accessor = ManyToManyAccessor::<TestUser, TestGroup>::new(&user, "groups", db.clone());

		accessor
			.add(&groups[0])
			.await
			.expect("relationship should be inserted");
		assert_eq!(
			accessor
				.count()
				.await
				.expect("relationship count should load"),
			1
		);
		let related = accessor.all().await.expect("related groups should load");
		assert_eq!(related.len(), 1);
		assert_eq!(related[0].id, groups[0].id);

		accessor
			.remove(&groups[0])
			.await
			.expect("relationship should be removed");
		assert_eq!(
			accessor
				.count()
				.await
				.expect("relationship count should load"),
			0
		);

		accessor
			.set(&groups)
			.await
			.expect("relationship set should be committed");
		assert_eq!(
			accessor
				.count()
				.await
				.expect("relationship count should load"),
			2
		);

		let related_users = ManyToManyAccessor::<TestUser, TestGroup>::filter_by_target(
			&TestUser::objects(),
			"groups",
			&groups[1],
			db.clone(),
		)
		.await
		.expect("source models should be filtered by target");
		assert_eq!(related_users.len(), 1);
		assert_eq!(related_users[0].id, user.id);

		accessor
			.clear()
			.await
			.expect("relationships should be cleared");
		assert_eq!(
			accessor
				.count()
				.await
				.expect("relationship count should load"),
			0
		);
	}

	#[test]
	fn uuid_model_contract_is_usable_by_the_accessor() {
		let first_id = uuid::Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000")
			.expect("UUID literal should be valid");
		let second_id = uuid::Uuid::parse_str("223e4567-e89b-12d3-a456-426614174000")
			.expect("UUID literal should be valid");
		let mut group = TestUuidGroup { id: first_id };

		assert_eq!(TestUuidGroup::table_name(), "uuid_groups");
		assert_eq!(
			TestUuidGroup::new_fields().with_alias("groups"),
			TestUuidGroupFields
		);
		assert_eq!(group.primary_key(), Some(first_id));
		group.set_primary_key(second_id);
		assert_eq!(group.primary_key(), Some(second_id));
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
		type Objects = Manager<Self>;

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
		type Objects = Manager<Self>;

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

	#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
	struct TestUuidGroup {
		id: uuid::Uuid,
	}

	#[derive(Clone, Debug, PartialEq, Eq)]
	struct TestUuidGroupFields;

	impl crate::orm::model::FieldSelector for TestUuidGroupFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for TestUuidGroup {
		type PrimaryKey = uuid::Uuid;
		type Fields = TestUuidGroupFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"uuid_groups"
		}

		fn new_fields() -> Self::Fields {
			TestUuidGroupFields
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			Some(self.id)
		}

		fn primary_key_filter_value(pk: Self::PrimaryKey) -> crate::orm::query::FilterValue {
			crate::orm::query::FilterValue::Uuid(pk)
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = value;
		}
	}
}
