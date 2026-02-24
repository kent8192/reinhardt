//! CREATE SEQUENCE statement builder
//!
//! This module provides the `CreateSequenceStatement` type for building SQL CREATE SEQUENCE queries.

use crate::{
	backend::QueryBuilder,
	types::{IntoIden, sequence::SequenceDef},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// CREATE SEQUENCE statement builder
///
/// This struct provides a fluent API for constructing CREATE SEQUENCE queries.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // CREATE SEQUENCE my_seq
/// let query = Query::create_sequence()
///     .name("my_seq");
///
/// // CREATE SEQUENCE IF NOT EXISTS my_seq INCREMENT BY 5
/// let query = Query::create_sequence()
///     .name("my_seq")
///     .if_not_exists()
///     .increment(5);
///
/// // CREATE SEQUENCE my_seq START WITH 100 MINVALUE 1 MAXVALUE 1000
/// let query = Query::create_sequence()
///     .name("my_seq")
///     .start(100)
///     .min_value(Some(1))
///     .max_value(Some(1000));
/// ```
#[derive(Debug, Clone)]
pub struct CreateSequenceStatement {
	pub(crate) sequence_def: SequenceDef,
}

impl CreateSequenceStatement {
	/// Create a new CREATE SEQUENCE statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_sequence();
	/// ```
	pub fn new() -> Self {
		// Start with empty name - will be set via .name()
		Self {
			sequence_def: SequenceDef::new(""),
		}
	}

	/// Take the ownership of data in the current [`CreateSequenceStatement`]
	pub fn take(&mut self) -> Self {
		let taken = Self {
			sequence_def: self.sequence_def.clone(),
		};
		// Reset self to empty state
		self.sequence_def = SequenceDef::new("");
		taken
	}

	/// Set the sequence name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_sequence()
	///     .name("my_seq");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.sequence_def.name = name.into_iden();
		self
	}

	/// Add IF NOT EXISTS clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_sequence()
	///     .name("my_seq")
	///     .if_not_exists();
	/// ```
	pub fn if_not_exists(&mut self) -> &mut Self {
		self.sequence_def.if_not_exists = true;
		self
	}

	/// Set INCREMENT BY value
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_sequence()
	///     .name("my_seq")
	///     .increment(5);
	/// ```
	pub fn increment(&mut self, increment: i64) -> &mut Self {
		self.sequence_def.increment = Some(increment);
		self
	}

	/// Set MINVALUE
	///
	/// Use `None` for NO MINVALUE, or `Some(value)` for specific minimum.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_sequence()
	///     .name("my_seq")
	///     .min_value(Some(1));
	/// ```
	pub fn min_value(&mut self, min_value: Option<i64>) -> &mut Self {
		self.sequence_def.min_value = Some(min_value);
		self
	}

	/// Set MAXVALUE
	///
	/// Use `None` for NO MAXVALUE, or `Some(value)` for specific maximum.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_sequence()
	///     .name("my_seq")
	///     .max_value(Some(1000));
	/// ```
	pub fn max_value(&mut self, max_value: Option<i64>) -> &mut Self {
		self.sequence_def.max_value = Some(max_value);
		self
	}

	/// Set START WITH value
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_sequence()
	///     .name("my_seq")
	///     .start(100);
	/// ```
	pub fn start(&mut self, start: i64) -> &mut Self {
		self.sequence_def.start = Some(start);
		self
	}

	/// Set CACHE value
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_sequence()
	///     .name("my_seq")
	///     .cache(20);
	/// ```
	pub fn cache(&mut self, cache: i64) -> &mut Self {
		self.sequence_def.cache = Some(cache);
		self
	}

	/// Set CYCLE or NO CYCLE
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_sequence()
	///     .name("my_seq")
	///     .cycle(true);
	/// ```
	pub fn cycle(&mut self, cycle: bool) -> &mut Self {
		self.sequence_def.cycle = Some(cycle);
		self
	}

	/// Set OWNED BY table.column
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_sequence()
	///     .name("my_seq")
	///     .owned_by_column("my_table", "id");
	/// ```
	pub fn owned_by_column<T: IntoIden, C: IntoIden>(&mut self, table: T, column: C) -> &mut Self {
		self.sequence_def = self.sequence_def.clone().owned_by_column(table, column);
		self
	}

	/// Set OWNED BY NONE
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::create_sequence()
	///     .name("my_seq")
	///     .owned_by_none();
	/// ```
	pub fn owned_by_none(&mut self) -> &mut Self {
		self.sequence_def = self.sequence_def.clone().owned_by_none();
		self
	}
}

impl Default for CreateSequenceStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for CreateSequenceStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_create_sequence(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_create_sequence(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_create_sequence(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for CreateSequenceStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_create_sequence_new() {
		let stmt = CreateSequenceStatement::new();
		assert!(stmt.sequence_def.name.to_string().is_empty());
		assert!(!stmt.sequence_def.if_not_exists);
		assert!(stmt.sequence_def.increment.is_none());
	}

	#[rstest]
	fn test_create_sequence_with_name() {
		let mut stmt = CreateSequenceStatement::new();
		stmt.name("my_seq");
		assert_eq!(stmt.sequence_def.name.to_string(), "my_seq");
	}

	#[rstest]
	fn test_create_sequence_if_not_exists() {
		let mut stmt = CreateSequenceStatement::new();
		stmt.name("my_seq").if_not_exists();
		assert!(stmt.sequence_def.if_not_exists);
	}

	#[rstest]
	fn test_create_sequence_increment() {
		let mut stmt = CreateSequenceStatement::new();
		stmt.name("my_seq").increment(5);
		assert_eq!(stmt.sequence_def.increment, Some(5));
	}

	#[rstest]
	fn test_create_sequence_min_max_values() {
		let mut stmt = CreateSequenceStatement::new();
		stmt.name("my_seq").min_value(Some(1)).max_value(Some(1000));
		assert_eq!(stmt.sequence_def.min_value, Some(Some(1)));
		assert_eq!(stmt.sequence_def.max_value, Some(Some(1000)));
	}

	#[rstest]
	fn test_create_sequence_start() {
		let mut stmt = CreateSequenceStatement::new();
		stmt.name("my_seq").start(100);
		assert_eq!(stmt.sequence_def.start, Some(100));
	}

	#[rstest]
	fn test_create_sequence_cache() {
		let mut stmt = CreateSequenceStatement::new();
		stmt.name("my_seq").cache(20);
		assert_eq!(stmt.sequence_def.cache, Some(20));
	}

	#[rstest]
	fn test_create_sequence_cycle() {
		let mut stmt = CreateSequenceStatement::new();
		stmt.name("my_seq").cycle(true);
		assert_eq!(stmt.sequence_def.cycle, Some(true));
	}

	#[rstest]
	fn test_create_sequence_take() {
		let mut stmt = CreateSequenceStatement::new();
		stmt.name("my_seq");
		let taken = stmt.take();
		assert!(stmt.sequence_def.name.to_string().is_empty());
		assert_eq!(taken.sequence_def.name.to_string(), "my_seq");
	}
}
