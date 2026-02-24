//! VACUUM statement builder
//!
//! This module provides the `VacuumStatement` type for building SQL VACUUM queries.

use crate::{
	backend::QueryBuilder,
	types::{DynIden, IntoIden, VacuumOption},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// VACUUM statement builder
///
/// This struct provides a fluent API for constructing VACUUM queries.
/// VACUUM reclaims storage and optionally analyzes a database.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // VACUUM (all tables)
/// let query = Query::vacuum();
///
/// // VACUUM users
/// let query = Query::vacuum()
///     .table("users");
///
/// // VACUUM FULL users
/// let query = Query::vacuum()
///     .table("users")
///     .full();
///
/// // VACUUM FULL ANALYZE users
/// let query = Query::vacuum()
///     .table("users")
///     .full()
///     .analyze();
/// ```
#[derive(Debug, Clone, Default)]
pub struct VacuumStatement {
	pub(crate) tables: Vec<DynIden>,
	pub(crate) full: bool,
	pub(crate) freeze: bool,
	pub(crate) verbose: bool,
	pub(crate) analyze: bool,
}

impl VacuumStatement {
	/// Create a new VACUUM statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::vacuum();
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Take the ownership of data in the current [`VacuumStatement`]
	pub fn take(&mut self) -> Self {
		Self {
			tables: std::mem::take(&mut self.tables),
			full: std::mem::take(&mut self.full),
			freeze: std::mem::take(&mut self.freeze),
			verbose: std::mem::take(&mut self.verbose),
			analyze: std::mem::take(&mut self.analyze),
		}
	}

	/// Add a table to vacuum
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::vacuum()
	///     .table("users");
	/// ```
	pub fn table<T>(&mut self, table: T) -> &mut Self
	where
		T: IntoIden,
	{
		self.tables.push(table.into_iden());
		self
	}

	/// Set FULL option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::vacuum()
	///     .full();
	/// ```
	pub fn full(&mut self) -> &mut Self {
		self.full = true;
		self
	}

	/// Set FREEZE option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::vacuum()
	///     .freeze();
	/// ```
	pub fn freeze(&mut self) -> &mut Self {
		self.freeze = true;
		self
	}

	/// Set VERBOSE option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::vacuum()
	///     .verbose();
	/// ```
	pub fn verbose(&mut self) -> &mut Self {
		self.verbose = true;
		self
	}

	/// Set ANALYZE option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::vacuum()
	///     .analyze();
	/// ```
	pub fn analyze(&mut self) -> &mut Self {
		self.analyze = true;
		self
	}

	/// Set options from VacuumOption
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	/// use reinhardt_query::types::VacuumOption;
	///
	/// let opt = VacuumOption::new().full(true).analyze(true);
	/// let query = Query::vacuum()
	///     .table("users")
	///     .options(opt);
	/// ```
	pub fn options(&mut self, opt: VacuumOption) -> &mut Self {
		self.full = opt.full;
		self.freeze = opt.freeze;
		self.verbose = opt.verbose;
		self.analyze = opt.analyze;
		self
	}
}

impl QueryStatementBuilder for VacuumStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_vacuum(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_vacuum(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_vacuum(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::CockroachDBQueryBuilder>()
		{
			return builder.build_vacuum(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for VacuumStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_vacuum_new() {
		let stmt = VacuumStatement::new();
		assert!(stmt.tables.is_empty());
		assert!(!stmt.full);
		assert!(!stmt.freeze);
		assert!(!stmt.verbose);
		assert!(!stmt.analyze);
	}

	#[rstest]
	fn test_vacuum_with_table() {
		let mut stmt = VacuumStatement::new();
		stmt.table("users");
		assert_eq!(stmt.tables.len(), 1);
		assert_eq!(stmt.tables[0].to_string(), "users");
	}

	#[rstest]
	fn test_vacuum_with_multiple_tables() {
		let mut stmt = VacuumStatement::new();
		stmt.table("users").table("posts");
		assert_eq!(stmt.tables.len(), 2);
		assert_eq!(stmt.tables[0].to_string(), "users");
		assert_eq!(stmt.tables[1].to_string(), "posts");
	}

	#[rstest]
	fn test_vacuum_full() {
		let mut stmt = VacuumStatement::new();
		stmt.full();
		assert!(stmt.full);
		assert!(!stmt.freeze);
		assert!(!stmt.verbose);
		assert!(!stmt.analyze);
	}

	#[rstest]
	fn test_vacuum_freeze() {
		let mut stmt = VacuumStatement::new();
		stmt.freeze();
		assert!(!stmt.full);
		assert!(stmt.freeze);
		assert!(!stmt.verbose);
		assert!(!stmt.analyze);
	}

	#[rstest]
	fn test_vacuum_verbose() {
		let mut stmt = VacuumStatement::new();
		stmt.verbose();
		assert!(!stmt.full);
		assert!(!stmt.freeze);
		assert!(stmt.verbose);
		assert!(!stmt.analyze);
	}

	#[rstest]
	fn test_vacuum_analyze() {
		let mut stmt = VacuumStatement::new();
		stmt.analyze();
		assert!(!stmt.full);
		assert!(!stmt.freeze);
		assert!(!stmt.verbose);
		assert!(stmt.analyze);
	}

	#[rstest]
	fn test_vacuum_combined_options() {
		let mut stmt = VacuumStatement::new();
		stmt.table("users").full().freeze().verbose().analyze();
		assert_eq!(stmt.tables.len(), 1);
		assert_eq!(stmt.tables[0].to_string(), "users");
		assert!(stmt.full);
		assert!(stmt.freeze);
		assert!(stmt.verbose);
		assert!(stmt.analyze);
	}

	#[rstest]
	fn test_vacuum_with_vacuum_option() {
		let opt = VacuumOption::new().full(true).analyze(true);
		let mut stmt = VacuumStatement::new();
		stmt.table("users").options(opt);
		assert_eq!(stmt.tables.len(), 1);
		assert!(stmt.full);
		assert!(!stmt.freeze);
		assert!(!stmt.verbose);
		assert!(stmt.analyze);
	}

	#[rstest]
	fn test_vacuum_take() {
		let mut stmt = VacuumStatement::new();
		stmt.table("users").full().analyze();

		let taken = stmt.take();
		assert_eq!(taken.tables.len(), 1);
		assert!(taken.full);
		assert!(taken.analyze);

		// Original should be reset
		assert!(stmt.tables.is_empty());
		assert!(!stmt.full);
		assert!(!stmt.analyze);
	}
}
