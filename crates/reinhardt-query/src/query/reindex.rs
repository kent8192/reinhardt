//! REINDEX statement builder
//!
//! This module provides the `ReindexStatement` type for building SQL REINDEX queries.
//!
//! # Backend Support
//!
//! - **PostgreSQL**: Full support (INDEX, TABLE, SCHEMA, DATABASE, SYSTEM with options)
//! - **MySQL**: NOT SUPPORTED (use OPTIMIZE TABLE)
//! - **SQLite**: Basic support (INDEX only, no options)

use crate::types::{DynIden, IntoIden};

use super::traits::{QueryBuilderTrait, QueryStatementBuilder};

/// REINDEX target type
///
/// Specifies what to reindex: individual index, table, schema, database, or entire system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReindexTarget {
	/// Reindex a specific index
	Index,
	/// Reindex all indexes in a table
	Table,
	/// Reindex all indexes in a schema (PostgreSQL only)
	Schema,
	/// Reindex all indexes in a database (PostgreSQL only)
	Database,
	/// Reindex all system catalogs (PostgreSQL only)
	System,
}

/// REINDEX statement builder
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt_query::prelude::*;
///
/// // PostgreSQL: Reindex a single index
/// let stmt = Query::reindex()
///     .index("idx_users_email");
///
/// // PostgreSQL: Reindex all indexes in a table
/// let stmt = Query::reindex()
///     .table("users");
///
/// // PostgreSQL: Reindex with options
/// let stmt = Query::reindex()
///     .index("idx_users_email")
///     .concurrently()
///     .verbose();
///
/// // SQLite: Reindex an index
/// let stmt = Query::reindex()
///     .index("idx_users_email");
/// ```
#[derive(Debug, Clone)]
pub struct ReindexStatement {
	pub(crate) target: Option<ReindexTarget>,
	pub(crate) name: Option<DynIden>,
	pub(crate) concurrently: bool,
	pub(crate) verbose: bool,
	pub(crate) tablespace: Option<DynIden>,
}

impl ReindexStatement {
	/// Create a new REINDEX statement
	pub fn new() -> Self {
		Self {
			target: None,
			name: None,
			concurrently: false,
			verbose: false,
			tablespace: None,
		}
	}

	/// Take the statement, leaving a default value in its place
	pub fn take(&mut self) -> Self {
		Self {
			target: self.target.take(),
			name: self.name.take(),
			concurrently: self.concurrently,
			verbose: self.verbose,
			tablespace: self.tablespace.take(),
		}
	}

	/// Reindex a specific index
	pub fn index<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.target = Some(ReindexTarget::Index);
		self.name = Some(name.into_iden());
		self
	}

	/// Reindex all indexes in a table
	pub fn table<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.target = Some(ReindexTarget::Table);
		self.name = Some(name.into_iden());
		self
	}

	/// Reindex all indexes in a schema (PostgreSQL only)
	pub fn schema<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.target = Some(ReindexTarget::Schema);
		self.name = Some(name.into_iden());
		self
	}

	/// Reindex all indexes in a database (PostgreSQL only)
	pub fn database<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.target = Some(ReindexTarget::Database);
		self.name = Some(name.into_iden());
		self
	}

	/// Reindex all system catalogs (PostgreSQL only)
	pub fn system<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.target = Some(ReindexTarget::System);
		self.name = Some(name.into_iden());
		self
	}

	/// Enable concurrent reindexing (PostgreSQL only)
	///
	/// This allows other operations to proceed while the index is being rebuilt.
	pub fn concurrently(&mut self) -> &mut Self {
		self.concurrently = true;
		self
	}

	/// Enable verbose output (PostgreSQL only)
	pub fn verbose(&mut self) -> &mut Self {
		self.verbose = true;
		self
	}

	/// Set the tablespace for the rebuilt index (PostgreSQL only)
	pub fn tablespace<T>(&mut self, tablespace: T) -> &mut Self
	where
		T: IntoIden,
	{
		self.tablespace = Some(tablespace.into_iden());
		self
	}
}

impl Default for ReindexStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for ReindexStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		use std::any::Any;

		if let Some(postgres) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			use crate::backend::QueryBuilder;
			postgres.build_reindex(self)
		} else if let Some(_mysql) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			panic!(
				"MySQL does not support REINDEX. Use OPTIMIZE TABLE or DROP/CREATE INDEX instead."
			);
		} else if let Some(sqlite) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			use crate::backend::QueryBuilder;
			sqlite.build_reindex(self)
		} else {
			panic!("Unsupported query builder type");
		}
	}
}
