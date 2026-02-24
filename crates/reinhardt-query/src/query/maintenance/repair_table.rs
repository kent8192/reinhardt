//! REPAIR TABLE statement builder
//!
//! This module provides the `RepairTableStatement` type for building SQL REPAIR TABLE queries.
//! **MySQL-only**: This statement is specific to MySQL and MariaDB.

use crate::{
	backend::QueryBuilder,
	types::{DynIden, IntoIden, RepairTableOption},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// REPAIR TABLE statement builder
///
/// This struct provides a fluent API for constructing REPAIR TABLE queries.
/// REPAIR TABLE repairs a possibly corrupted table.
///
/// **MySQL-only**: Other backends will panic with a helpful error message.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // REPAIR TABLE users
/// let query = Query::repair_table()
///     .table("users");
///
/// // REPAIR TABLE users QUICK
/// let query = Query::repair_table()
///     .table("users")
///     .quick();
///
/// // REPAIR TABLE users EXTENDED USE_FRM
/// let query = Query::repair_table()
///     .table("users")
///     .extended()
///     .use_frm();
/// ```
#[derive(Debug, Clone, Default)]
pub struct RepairTableStatement {
	pub(crate) tables: Vec<DynIden>,
	pub(crate) no_write_to_binlog: bool,
	pub(crate) local: bool,
	pub(crate) quick: bool,
	pub(crate) extended: bool,
	pub(crate) use_frm: bool,
}

impl RepairTableStatement {
	/// Create a new REPAIR TABLE statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::repair_table();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Take the ownership of data in the current [`RepairTableStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			tables: std::mem::take(&mut self.tables),
			no_write_to_binlog: std::mem::take(&mut self.no_write_to_binlog),
			local: std::mem::take(&mut self.local),
			quick: std::mem::take(&mut self.quick),
			extended: std::mem::take(&mut self.extended),
			use_frm: std::mem::take(&mut self.use_frm),
		}
	}

	/// Add a table to repair
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::repair_table()
	///     .table("users");
	/// ```
	pub fn table<T>(&mut self, table: T) -> &mut Self
	where
		T: IntoIden,
	{
		self.tables.push(table.into_iden());
		self
	}

	/// Set NO_WRITE_TO_BINLOG option
	///
	/// Suppresses binary logging for this operation.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::repair_table()
	///     .table("users")
	///     .no_write_to_binlog();
	/// ```
	pub fn no_write_to_binlog(&mut self) -> &mut Self {
		self.no_write_to_binlog = true;
		self
	}

	/// Set LOCAL option
	///
	/// Suppresses binary logging for this operation (same as NO_WRITE_TO_BINLOG).
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::repair_table()
	///     .table("users")
	///     .local();
	/// ```
	pub fn local(&mut self) -> &mut Self {
		self.local = true;
		self
	}

	/// Set QUICK option
	///
	/// Tries to repair only the index file, not the data file.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::repair_table()
	///     .table("users")
	///     .quick();
	/// ```
	pub fn quick(&mut self) -> &mut Self {
		self.quick = true;
		self
	}

	/// Set EXTENDED option
	///
	/// Creates the index row by row instead of creating one index at a time with sorting.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::repair_table()
	///     .table("users")
	///     .extended();
	/// ```
	pub fn extended(&mut self) -> &mut Self {
		self.extended = true;
		self
	}

	/// Set USE_FRM option
	///
	/// Uses the table definition from the .frm file to recreate the index file.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::repair_table()
	///     .table("users")
	///     .use_frm();
	/// ```
	pub fn use_frm(&mut self) -> &mut Self {
		self.use_frm = true;
		self
	}

	/// Set options from RepairTableOption
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::RepairTableOption;
	///
	/// let opt = RepairTableOption::new().quick(true).use_frm(true);
	/// let query = Query::repair_table()
	///     .table("users")
	///     .options(opt);
	/// ```
	pub fn options(&mut self, opt: RepairTableOption) -> &mut Self {
		self.no_write_to_binlog = opt.no_write_to_binlog;
		self.local = opt.local;
		self.quick = opt.quick;
		self.extended = opt.extended;
		self.use_frm = opt.use_frm;
		self
	}
}

impl QueryStatementBuilder for RepairTableStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_repair_table(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_repair_table(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_repair_table(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_repair_table(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for RepairTableStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_repair_table_new() {
		let stmt = RepairTableStatement::new();
		assert!(stmt.tables.is_empty());
		assert!(!stmt.no_write_to_binlog);
		assert!(!stmt.local);
		assert!(!stmt.quick);
		assert!(!stmt.extended);
		assert!(!stmt.use_frm);
	}

	#[rstest]
	fn test_repair_table_with_table() {
		let mut stmt = RepairTableStatement::new();
		stmt.table("users");
		assert_eq!(stmt.tables.len(), 1);
		assert_eq!(stmt.tables[0].to_string(), "users");
	}

	#[rstest]
	fn test_repair_table_with_multiple_tables() {
		let mut stmt = RepairTableStatement::new();
		stmt.table("users").table("posts");
		assert_eq!(stmt.tables.len(), 2);
		assert_eq!(stmt.tables[0].to_string(), "users");
		assert_eq!(stmt.tables[1].to_string(), "posts");
	}

	#[rstest]
	fn test_repair_table_quick() {
		let mut stmt = RepairTableStatement::new();
		stmt.quick();
		assert!(stmt.quick);
		assert!(!stmt.extended);
		assert!(!stmt.use_frm);
	}

	#[rstest]
	fn test_repair_table_extended() {
		let mut stmt = RepairTableStatement::new();
		stmt.extended();
		assert!(!stmt.quick);
		assert!(stmt.extended);
		assert!(!stmt.use_frm);
	}

	#[rstest]
	fn test_repair_table_use_frm() {
		let mut stmt = RepairTableStatement::new();
		stmt.use_frm();
		assert!(!stmt.quick);
		assert!(!stmt.extended);
		assert!(stmt.use_frm);
	}

	#[rstest]
	fn test_repair_table_with_option() {
		let opt = RepairTableOption::new().quick(true).use_frm(true);
		let mut stmt = RepairTableStatement::new();
		stmt.table("users").options(opt);
		assert_eq!(stmt.tables.len(), 1);
		assert!(stmt.quick);
		assert!(!stmt.extended);
		assert!(stmt.use_frm);
	}

	#[rstest]
	fn test_repair_table_take() {
		let mut stmt = RepairTableStatement::new();
		stmt.table("users").quick().use_frm();

		let taken = stmt.take();
		assert_eq!(taken.tables.len(), 1);
		assert!(taken.quick);
		assert!(taken.use_frm);

		// Original should be reset
		assert!(stmt.tables.is_empty());
		assert!(!stmt.quick);
		assert!(!stmt.use_frm);
	}
}
