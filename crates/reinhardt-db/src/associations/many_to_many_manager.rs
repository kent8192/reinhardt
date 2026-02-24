//! ManyToMany relationship manager
//!
//! This module provides the `ManyToManyManager` type for managing many-to-many relationships
//! through junction tables. It abstracts CRUD operations on the intermediate table.

use crate::orm::{DatabaseConnection, QueryRow};
use reinhardt_core::exception::Result;
use reinhardt_query::prelude::{
	Alias, Expr, ExprTrait, Func, OnConflict, PostgresQueryBuilder, Query, QueryBuilder,
};
use std::fmt::Display;
use std::marker::PhantomData;

/// Manager for ManyToMany relationship operations
///
/// This type handles CRUD operations on junction tables for many-to-many relationships.
/// It uses `reinhardt_query` for type-safe SQL generation.
///
/// # Type Parameters
///
/// * `S` - Source model type
/// * `T` - Target model type
/// * `PK` - Primary key type (must implement Display for SQL conversion)
///
/// # Example
///
/// ```rust,ignore
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use reinhardt_db::associations::ManyToManyManager;
/// # use uuid::Uuid;
/// # let user_id = Uuid::new_v4();
/// # struct Database;
/// # let db = Database;
/// # struct Group { id: Uuid }
/// # let group = Group { id: Uuid::new_v4() };
///
/// let manager = ManyToManyManager::new(
///     user_id,
///     "auth_user_groups".to_string(),
///     "user_id".to_string(),
///     "group_id".to_string(),
/// );
///
/// // Add relationship
/// manager.add_with_db(&db, &group).await?;
///
/// // Check if exists
/// let exists = manager.contains_with_db(&db, &group).await?;
///
/// // Get all related items
/// let groups = manager.all_with_db(&db).await?;
///
/// // Remove relationship
/// manager.remove_with_db(&db, &group).await?;
///
/// // Clear all relationships
/// manager.clear_with_db(&db).await?;
///
/// # Ok(())
/// # }
/// ```
pub struct ManyToManyManager<S, T, PK> {
	source_pk: PK,
	through_table: String,
	source_field: String,
	target_field: String,
	_phantom_s: PhantomData<S>,
	_phantom_t: PhantomData<T>,
}

impl<S, T, PK> ManyToManyManager<S, T, PK>
where
	PK: Display + Clone,
{
	/// Create a new ManyToManyManager
	///
	/// # Arguments
	///
	/// * `source_pk` - Primary key of the source instance
	/// * `through_table` - Name of the junction table
	/// * `source_field` - Column name for the source foreign key in junction table
	/// * `target_field` - Column name for the target foreign key in junction table
	pub fn new(
		source_pk: PK,
		through_table: String,
		source_field: String,
		target_field: String,
	) -> Self {
		Self {
			source_pk,
			through_table,
			source_field,
			target_field,
			_phantom_s: PhantomData,
			_phantom_t: PhantomData,
		}
	}

	/// Add a target instance to the relationship
	///
	/// This inserts a row into the junction table. If the relationship already exists
	/// (UNIQUE constraint violation), the operation is ignored (ON CONFLICT DO NOTHING).
	///
	/// # Arguments
	///
	/// * `conn` - Database connection
	/// * `target_pk` - Primary key of the target instance
	///
	/// # Returns
	///
	/// * `Ok(())` on success
	/// * `Err` if database operation fails
	pub async fn add_with_db<TPK>(&self, conn: &DatabaseConnection, target_pk: TPK) -> Result<()>
	where
		TPK: Display,
	{
		let mut stmt = Query::insert();
		stmt.into_table(Alias::new(&self.through_table))
			.columns([
				Alias::new(&self.source_field),
				Alias::new(&self.target_field),
			])
			.values_panic([self.source_pk.to_string(), target_pk.to_string()])
			.on_conflict(
				OnConflict::columns([
					Alias::new(&self.source_field),
					Alias::new(&self.target_field),
				])
				.do_nothing()
				.to_owned(),
			);

		let pg = PostgresQueryBuilder::new();
		let (sql, values) = pg.build_insert(&stmt);
		let params = crate::orm::execution::convert_values(values);

		// Execute SQL
		conn.execute(&sql, params).await?;
		Ok(())
	}

	/// Remove a target instance from the relationship
	///
	/// This deletes the corresponding row from the junction table.
	///
	/// # Arguments
	///
	/// * `conn` - Database connection
	/// * `target_pk` - Primary key of the target instance
	///
	/// # Returns
	///
	/// * `Ok(())` on success
	/// * `Err` if database operation fails
	pub async fn remove_with_db<TPK>(&self, conn: &DatabaseConnection, target_pk: TPK) -> Result<()>
	where
		TPK: Display,
	{
		let mut stmt = Query::delete();
		stmt.from_table(Alias::new(&self.through_table))
			.and_where(Expr::col(Alias::new(&self.source_field)).eq(self.source_pk.to_string()))
			.and_where(Expr::col(Alias::new(&self.target_field)).eq(target_pk.to_string()));

		let pg = PostgresQueryBuilder::new();
		let (sql, _) = pg.build_delete(&stmt);

		// Execute SQL
		conn.execute(&sql, vec![]).await?;
		Ok(())
	}

	/// Check if a target instance is in the relationship
	///
	/// # Arguments
	///
	/// * `conn` - Database connection
	/// * `target_pk` - Primary key of the target instance
	///
	/// # Returns
	///
	/// * `Ok(true)` if the relationship exists
	/// * `Ok(false)` if the relationship does not exist
	/// * `Err` if database operation fails
	pub async fn contains_with_db<TPK>(
		&self,
		conn: &DatabaseConnection,
		target_pk: TPK,
	) -> Result<bool>
	where
		TPK: Display,
	{
		let mut stmt = Query::select();
		stmt.from(Alias::new(&self.through_table))
			.expr(Expr::asterisk())
			.and_where(Expr::col(Alias::new(&self.source_field)).eq(self.source_pk.to_string()))
			.and_where(Expr::col(Alias::new(&self.target_field)).eq(target_pk.to_string()));

		let pg = PostgresQueryBuilder::new();
		let (sql, _) = pg.build_select(&stmt);

		// Execute SQL
		let rows = conn.query(&sql, vec![]).await?;
		Ok(!rows.is_empty())
	}

	/// Get all target instances in the relationship
	///
	/// This performs a JOIN between the junction table and the target table.
	///
	/// # Arguments
	///
	/// * `conn` - Database connection
	/// * `target_table` - Name of the target table
	/// * `target_pk_field` - Name of the primary key column in target table
	///
	/// # Returns
	///
	/// * `Ok(Vec<T>)` with all related target instances
	/// * `Err` if database operation fails
	pub async fn all_with_db(
		&self,
		conn: &DatabaseConnection,
		target_table: &str,
		target_pk_field: &str,
	) -> Result<Vec<QueryRow>> {
		let mut stmt = Query::select();
		stmt.from(Alias::new(&self.through_table))
			.inner_join(
				Alias::new(target_table),
				Expr::col((Alias::new(&self.through_table), Alias::new(&self.target_field)))
					.equals((Alias::new(target_table), Alias::new(target_pk_field))),
			)
			// Select all columns from target table
			.expr(Expr::asterisk())
			.and_where(
				Expr::col((Alias::new(&self.through_table), Alias::new(&self.source_field)))
					.eq(self.source_pk.to_string()),
			);

		let pg = PostgresQueryBuilder::new();
		let (sql, _) = pg.build_select(&stmt);

		// Execute SQL
		conn.query(&sql, vec![])
			.await
			.map_err(|e| reinhardt_core::exception::Error::Database(e.to_string()))
	}

	/// Clear all relationships for the source instance
	///
	/// This deletes all rows from the junction table where the source matches.
	///
	/// # Arguments
	///
	/// * `conn` - Database connection
	///
	/// # Returns
	///
	/// * `Ok(())` on success
	/// * `Err` if database operation fails
	pub async fn clear_with_db(&self, conn: &DatabaseConnection) -> Result<()> {
		let mut stmt = Query::delete();
		stmt.from_table(Alias::new(&self.through_table))
			.and_where(Expr::col(Alias::new(&self.source_field)).eq(self.source_pk.to_string()));

		let pg = PostgresQueryBuilder::new();
		let (sql, _) = pg.build_delete(&stmt);

		// Execute SQL
		conn.execute(&sql, vec![]).await?;
		Ok(())
	}

	/// Count the number of relationships for the source instance
	///
	/// # Arguments
	///
	/// * `conn` - Database connection
	///
	/// # Returns
	///
	/// * `Ok(usize)` with the count
	/// * `Err` if database operation fails
	pub async fn count_with_db(&self, conn: &DatabaseConnection) -> Result<usize> {
		let mut stmt = Query::select();
		stmt.from(Alias::new(&self.through_table))
			.expr_as(
				Func::count(Expr::asterisk().into_simple_expr()),
				Alias::new("count"),
			)
			.and_where(Expr::col(Alias::new(&self.source_field)).eq(self.source_pk.to_string()));

		let pg = PostgresQueryBuilder::new();
		let (sql, _) = pg.build_select(&stmt);

		// Execute SQL
		let row = conn.query_one(&sql, vec![]).await?;

		// Get count value (retrieve by column name)
		let count_value = row
			.get::<i64>("count")
			.or_else(|| row.get::<i64>("COUNT"))
			.ok_or_else(|| {
				reinhardt_core::exception::Error::Database(
					"Failed to extract count value from query result".to_string(),
				)
			})?;

		Ok(count_value as usize)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_manager_creation() {
		let manager: ManyToManyManager<(), (), i64> = ManyToManyManager::new(
			42,
			"user_groups".to_string(),
			"user_id".to_string(),
			"group_id".to_string(),
		);

		assert_eq!(manager.through_table, "user_groups");
		assert_eq!(manager.source_field, "user_id");
		assert_eq!(manager.target_field, "group_id");
	}
}
