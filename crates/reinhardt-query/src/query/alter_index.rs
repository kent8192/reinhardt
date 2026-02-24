//! ALTER INDEX statement builder (PostgreSQL and MySQL)
//!
//! This module provides the `AlterIndexStatement` type for building SQL ALTER INDEX queries.
//!
//! # Backend Support
//!
//! - **PostgreSQL**: Full support (RENAME TO, SET TABLESPACE)
//! - **MySQL**: Partial support via ALTER TABLE (RENAME INDEX only, requires table name)
//! - **SQLite**: NOT SUPPORTED (must drop and recreate index)

use crate::types::{DynIden, IntoIden};

use super::traits::{QueryBuilderTrait, QueryStatementBuilder};

/// ALTER INDEX statement builder
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// // PostgreSQL: Rename index
/// let stmt = Query::alter_index()
///     .name("idx_users_email")
///     .rename_to("idx_users_email_new");
///
/// // PostgreSQL: Set tablespace
/// let stmt = Query::alter_index()
///     .name("idx_users_email")
///     .set_tablespace("fast_ssd");
///
/// // MySQL: Rename index (requires table name)
/// let stmt = Query::alter_index()
///     .table("users")
///     .name("idx_email")
///     .rename_to("idx_email_new");
/// ```
#[derive(Debug, Clone)]
pub struct AlterIndexStatement {
	pub(crate) name: Option<DynIden>,
	pub(crate) rename_to: Option<DynIden>,
	pub(crate) set_tablespace: Option<DynIden>,
	pub(crate) table: Option<DynIden>, // MySQL requires table name
}

impl AlterIndexStatement {
	/// Create a new ALTER INDEX statement
	pub fn new() -> Self {
		Self {
			name: None,
			rename_to: None,
			set_tablespace: None,
			table: None,
		}
	}

	/// Take the statement, leaving a default value in its place
	pub fn take(&mut self) -> Self {
		Self {
			name: self.name.take(),
			rename_to: self.rename_to.take(),
			set_tablespace: self.set_tablespace.take(),
			table: self.table.take(),
		}
	}

	/// Set the index name to alter
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.name = Some(name.into_iden());
		self
	}

	/// Rename the index to a new name
	pub fn rename_to<N>(&mut self, new_name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.rename_to = Some(new_name.into_iden());
		self
	}

	/// Set the tablespace for the index (PostgreSQL only)
	pub fn set_tablespace<T>(&mut self, tablespace: T) -> &mut Self
	where
		T: IntoIden,
	{
		self.set_tablespace = Some(tablespace.into_iden());
		self
	}

	/// Set the table name (required for MySQL)
	pub fn table<T>(&mut self, table: T) -> &mut Self
	where
		T: IntoIden,
	{
		self.table = Some(table.into_iden());
		self
	}
}

impl Default for AlterIndexStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for AlterIndexStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		use std::any::Any;

		if let Some(postgres) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			use crate::backend::QueryBuilder;
			postgres.build_alter_index(self)
		} else if let Some(mysql) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			use crate::backend::QueryBuilder;
			mysql.build_alter_index(self)
		} else if let Some(_sqlite) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			panic!("SQLite does not support ALTER INDEX. Drop and recreate the index instead.");
		} else {
			panic!("Unsupported query builder type");
		}
	}
}
