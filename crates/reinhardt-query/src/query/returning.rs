//! RETURNING clause support
//!
//! This module provides the `ReturningClause` type for specifying columns to return
//! from INSERT, UPDATE, and DELETE statements (PostgreSQL, SQLite 3.35+).

use crate::types::{ColumnRef, IntoColumnRef};

/// RETURNING clause for INSERT/UPDATE/DELETE statements
///
/// Specifies which columns should be returned after the modification operation.
/// Supported by PostgreSQL and SQLite 3.35+.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// // Return all columns
/// let returning = ReturningClause::all();
///
/// // Return specific columns
/// let returning = ReturningClause::columns(["id", "created_at"]);
/// ```
#[derive(Debug, Clone, Default)]
pub enum ReturningClause {
	/// Return all columns (RETURNING *)
	#[default]
	All,
	/// Return specific columns
	Columns(Vec<ColumnRef>),
}

impl ReturningClause {
	/// Create a RETURNING * clause
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::insert()
	///     .into_table("users")
	///     .columns(["name", "email"])
	///     .values_panic(["Alice", "alice@example.com"])
	///     .returning_all();
	/// ```
	pub fn all() -> Self {
		Self::All
	}

	/// Create a RETURNING clause with specific columns
	///
	/// # Examples
	///
	/// ```rust,ignore
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::insert()
	///     .into_table("users")
	///     .columns(["name", "email"])
	///     .values_panic(["Alice", "alice@example.com"])
	///     .returning(["id", "created_at"]);
	/// ```
	pub fn columns<I, C>(cols: I) -> Self
	where
		I: IntoIterator<Item = C>,
		C: IntoColumnRef,
	{
		let columns: Vec<ColumnRef> = cols.into_iter().map(|c| c.into_column_ref()).collect();
		Self::Columns(columns)
	}

	/// Add a column to the RETURNING clause
	///
	/// If this is a `ReturningClause::All`, it will be converted to a specific column list.
	pub fn add_column<C>(&mut self, col: C)
	where
		C: IntoColumnRef,
	{
		match self {
			Self::All => {
				// Convert ALL to specific columns
				*self = Self::Columns(vec![col.into_column_ref()]);
			}
			Self::Columns(cols) => {
				cols.push(col.into_column_ref());
			}
		}
	}

	/// Check if this clause returns all columns
	pub fn is_all(&self) -> bool {
		matches!(self, Self::All)
	}

	/// Get the columns list if this is a specific column clause
	pub fn get_columns(&self) -> Option<&[ColumnRef]> {
		match self {
			Self::All => None,
			Self::Columns(cols) => Some(cols),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_returning_all() {
		let returning = ReturningClause::all();
		assert!(returning.is_all());
		assert!(returning.get_columns().is_none());
	}

	#[test]
	fn test_returning_columns() {
		let returning = ReturningClause::columns(["id", "name"]);
		assert!(!returning.is_all());
		let cols = returning.get_columns().unwrap();
		assert_eq!(cols.len(), 2);
	}

	#[test]
	fn test_add_column() {
		let mut returning = ReturningClause::all();
		returning.add_column("id");
		assert!(!returning.is_all());
		let cols = returning.get_columns().unwrap();
		assert_eq!(cols.len(), 1);
	}

	#[test]
	fn test_default() {
		let returning = ReturningClause::default();
		assert!(returning.is_all());
	}
}
