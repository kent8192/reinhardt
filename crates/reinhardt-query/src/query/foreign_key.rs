//! Foreign key statement builders.
//!
//! This module provides [`ForeignKey`] as an entry point and
//! [`ForeignKeyCreateStatement`] for building foreign key constraints
//! compatible with the reinhardt-query builder pattern.

use crate::types::{DynIden, ForeignKeyAction, IntoIden, IntoTableRef, TableRef};

/// Entry point for foreign key operations.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// let mut fk = ForeignKey::create();
/// fk.from_tbl(Alias::new("posts"))
///     .from_col(Alias::new("user_id"))
///     .to_tbl(Alias::new("users"))
///     .to_col(Alias::new("id"));
/// ```
#[derive(Debug, Clone)]
pub struct ForeignKey;

impl ForeignKey {
	/// Create a new foreign key CREATE statement builder.
	pub fn create() -> ForeignKeyCreateStatement {
		ForeignKeyCreateStatement::new()
	}
}

/// Builder for a CREATE FOREIGN KEY constraint.
///
/// This builder collects foreign key metadata (source table/columns,
/// referenced table/columns, and referential actions) for use in
/// CREATE TABLE statements.
#[derive(Debug, Clone, Default)]
pub struct ForeignKeyCreateStatement {
	pub(crate) name: Option<DynIden>,
	pub(crate) from_tbl: Option<TableRef>,
	pub(crate) from_cols: Vec<DynIden>,
	pub(crate) to_tbl: Option<TableRef>,
	pub(crate) to_cols: Vec<DynIden>,
	pub(crate) on_delete: Option<ForeignKeyAction>,
	pub(crate) on_update: Option<ForeignKeyAction>,
}

impl ForeignKeyCreateStatement {
	/// Create a new empty foreign key builder.
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the constraint name.
	pub fn name<T>(&mut self, name: T) -> &mut Self
	where
		T: IntoIden,
	{
		self.name = Some(name.into_iden());
		self
	}

	/// Set the source (referencing) table.
	pub fn from_tbl<T>(&mut self, tbl: T) -> &mut Self
	where
		T: IntoTableRef,
	{
		self.from_tbl = Some(tbl.into_table_ref());
		self
	}

	/// Add a source (referencing) column.
	pub fn from_col<C>(&mut self, col: C) -> &mut Self
	where
		C: IntoIden,
	{
		self.from_cols.push(col.into_iden());
		self
	}

	/// Set the target (referenced) table.
	pub fn to_tbl<T>(&mut self, tbl: T) -> &mut Self
	where
		T: IntoTableRef,
	{
		self.to_tbl = Some(tbl.into_table_ref());
		self
	}

	/// Add a target (referenced) column.
	pub fn to_col<C>(&mut self, col: C) -> &mut Self
	where
		C: IntoIden,
	{
		self.to_cols.push(col.into_iden());
		self
	}

	/// Set the ON DELETE action.
	pub fn on_delete(&mut self, action: ForeignKeyAction) -> &mut Self {
		self.on_delete = Some(action);
		self
	}

	/// Set the ON UPDATE action.
	pub fn on_update(&mut self, action: ForeignKeyAction) -> &mut Self {
		self.on_update = Some(action);
		self
	}
}
