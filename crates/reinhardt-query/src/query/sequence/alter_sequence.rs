//! ALTER SEQUENCE statement builder
//!
//! This module provides the `AlterSequenceStatement` type for building SQL ALTER SEQUENCE queries.

use crate::{
	backend::QueryBuilder,
	types::{
		DynIden, IntoIden,
		sequence::{OwnedBy, SequenceOption},
	},
};

use crate::query::traits::{QueryBuilderTrait, QueryStatementBuilder, QueryStatementWriter};

/// ALTER SEQUENCE statement builder
///
/// This struct provides a fluent API for constructing ALTER SEQUENCE queries.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::prelude::*;
///
/// // ALTER SEQUENCE my_seq RESTART
/// let query = Query::alter_sequence()
///     .name("my_seq")
///     .restart(None);
///
/// // ALTER SEQUENCE my_seq RESTART WITH 100
/// let query = Query::alter_sequence()
///     .name("my_seq")
///     .restart(Some(100));
///
/// // ALTER SEQUENCE my_seq INCREMENT BY 5 MINVALUE 1 MAXVALUE 1000
/// let query = Query::alter_sequence()
///     .name("my_seq")
///     .increment_by(5)
///     .min_value(1)
///     .max_value(1000);
///
/// // ALTER SEQUENCE my_seq OWNED BY my_table.id
/// let query = Query::alter_sequence()
///     .name("my_seq")
///     .owned_by_column("my_table", "id");
/// ```
#[derive(Debug, Clone)]
pub struct AlterSequenceStatement {
	pub(crate) name: DynIden,
	pub(crate) options: Vec<SequenceOption>,
}

impl AlterSequenceStatement {
	/// Create a new ALTER SEQUENCE statement
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_sequence();
	/// ```
	pub fn new() -> Self {
		Self {
			name: "".into_iden(),
			options: Vec::new(),
		}
	}

	/// Take the ownership of data in the current [`AlterSequenceStatement`]
	pub fn take(&mut self) -> Self {
		let taken = Self {
			name: self.name.clone(),
			options: self.options.clone(),
		};
		// Reset self to empty state
		self.name = "".into_iden();
		self.options.clear();
		taken
	}

	/// Set the sequence name
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_sequence()
	///     .name("my_seq");
	/// ```
	pub fn name<N>(&mut self, name: N) -> &mut Self
	where
		N: IntoIden,
	{
		self.name = name.into_iden();
		self
	}

	/// Add RESTART option
	///
	/// Use `None` for RESTART (without value) or `Some(value)` for RESTART WITH value.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// // RESTART
	/// let query = Query::alter_sequence()
	///     .name("my_seq")
	///     .restart(None);
	///
	/// // RESTART WITH 100
	/// let query = Query::alter_sequence()
	///     .name("my_seq")
	///     .restart(Some(100));
	/// ```
	pub fn restart(&mut self, value: Option<i64>) -> &mut Self {
		self.options.push(SequenceOption::Restart(value));
		self
	}

	/// Add INCREMENT BY option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_sequence()
	///     .name("my_seq")
	///     .increment_by(5);
	/// ```
	pub fn increment_by(&mut self, increment: i64) -> &mut Self {
		self.options.push(SequenceOption::IncrementBy(increment));
		self
	}

	/// Add MINVALUE option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_sequence()
	///     .name("my_seq")
	///     .min_value(1);
	/// ```
	pub fn min_value(&mut self, value: i64) -> &mut Self {
		self.options.push(SequenceOption::MinValue(value));
		self
	}

	/// Add NO MINVALUE option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_sequence()
	///     .name("my_seq")
	///     .no_min_value();
	/// ```
	pub fn no_min_value(&mut self) -> &mut Self {
		self.options.push(SequenceOption::NoMinValue);
		self
	}

	/// Add MAXVALUE option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_sequence()
	///     .name("my_seq")
	///     .max_value(1000);
	/// ```
	pub fn max_value(&mut self, value: i64) -> &mut Self {
		self.options.push(SequenceOption::MaxValue(value));
		self
	}

	/// Add NO MAXVALUE option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_sequence()
	///     .name("my_seq")
	///     .no_max_value();
	/// ```
	pub fn no_max_value(&mut self) -> &mut Self {
		self.options.push(SequenceOption::NoMaxValue);
		self
	}

	/// Add CACHE option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_sequence()
	///     .name("my_seq")
	///     .cache(20);
	/// ```
	pub fn cache(&mut self, value: i64) -> &mut Self {
		self.options.push(SequenceOption::Cache(value));
		self
	}

	/// Add CYCLE option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_sequence()
	///     .name("my_seq")
	///     .cycle();
	/// ```
	pub fn cycle(&mut self) -> &mut Self {
		self.options.push(SequenceOption::Cycle);
		self
	}

	/// Add NO CYCLE option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_sequence()
	///     .name("my_seq")
	///     .no_cycle();
	/// ```
	pub fn no_cycle(&mut self) -> &mut Self {
		self.options.push(SequenceOption::NoCycle);
		self
	}

	/// Add OWNED BY table.column option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_sequence()
	///     .name("my_seq")
	///     .owned_by_column("my_table", "id");
	/// ```
	pub fn owned_by_column<T: IntoIden, C: IntoIden>(&mut self, table: T, column: C) -> &mut Self {
		self.options.push(SequenceOption::OwnedBy(OwnedBy::Column {
			table: table.into_iden(),
			column: column.into_iden(),
		}));
		self
	}

	/// Add OWNED BY NONE option
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::prelude::*;
	///
	/// let query = Query::alter_sequence()
	///     .name("my_seq")
	///     .owned_by_none();
	/// ```
	pub fn owned_by_none(&mut self) -> &mut Self {
		self.options.push(SequenceOption::OwnedBy(OwnedBy::None));
		self
	}
}

impl Default for AlterSequenceStatement {
	fn default() -> Self {
		Self::new()
	}
}

impl QueryStatementBuilder for AlterSequenceStatement {
	fn build_any(&self, query_builder: &dyn QueryBuilderTrait) -> (String, crate::value::Values) {
		// Downcast to concrete QueryBuilder type
		use std::any::Any;
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::PostgresQueryBuilder>()
		{
			return builder.build_alter_sequence(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::MySqlQueryBuilder>()
		{
			return builder.build_alter_sequence(self);
		}
		if let Some(builder) =
			(query_builder as &dyn Any).downcast_ref::<crate::backend::SqliteQueryBuilder>()
		{
			return builder.build_alter_sequence(self);
		}
		panic!("Unsupported query builder type");
	}
}

impl QueryStatementWriter for AlterSequenceStatement {}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_alter_sequence_new() {
		let stmt = AlterSequenceStatement::new();
		assert!(stmt.name.to_string().is_empty());
		assert!(stmt.options.is_empty());
	}

	#[rstest]
	fn test_alter_sequence_with_name() {
		let mut stmt = AlterSequenceStatement::new();
		stmt.name("my_seq");
		assert_eq!(stmt.name.to_string(), "my_seq");
	}

	#[rstest]
	fn test_alter_sequence_restart_without_value() {
		let mut stmt = AlterSequenceStatement::new();
		stmt.name("my_seq").restart(None);
		assert_eq!(stmt.options.len(), 1);
		assert!(matches!(stmt.options[0], SequenceOption::Restart(None)));
	}

	#[rstest]
	fn test_alter_sequence_restart_with_value() {
		let mut stmt = AlterSequenceStatement::new();
		stmt.name("my_seq").restart(Some(100));
		assert_eq!(stmt.options.len(), 1);
		assert!(matches!(
			stmt.options[0],
			SequenceOption::Restart(Some(100))
		));
	}

	#[rstest]
	fn test_alter_sequence_increment_by() {
		let mut stmt = AlterSequenceStatement::new();
		stmt.name("my_seq").increment_by(5);
		assert_eq!(stmt.options.len(), 1);
		assert!(matches!(stmt.options[0], SequenceOption::IncrementBy(5)));
	}

	#[rstest]
	fn test_alter_sequence_min_value() {
		let mut stmt = AlterSequenceStatement::new();
		stmt.name("my_seq").min_value(1);
		assert_eq!(stmt.options.len(), 1);
		assert!(matches!(stmt.options[0], SequenceOption::MinValue(1)));
	}

	#[rstest]
	fn test_alter_sequence_no_min_value() {
		let mut stmt = AlterSequenceStatement::new();
		stmt.name("my_seq").no_min_value();
		assert_eq!(stmt.options.len(), 1);
		assert!(matches!(stmt.options[0], SequenceOption::NoMinValue));
	}

	#[rstest]
	fn test_alter_sequence_max_value() {
		let mut stmt = AlterSequenceStatement::new();
		stmt.name("my_seq").max_value(1000);
		assert_eq!(stmt.options.len(), 1);
		assert!(matches!(stmt.options[0], SequenceOption::MaxValue(1000)));
	}

	#[rstest]
	fn test_alter_sequence_no_max_value() {
		let mut stmt = AlterSequenceStatement::new();
		stmt.name("my_seq").no_max_value();
		assert_eq!(stmt.options.len(), 1);
		assert!(matches!(stmt.options[0], SequenceOption::NoMaxValue));
	}

	#[rstest]
	fn test_alter_sequence_cache() {
		let mut stmt = AlterSequenceStatement::new();
		stmt.name("my_seq").cache(20);
		assert_eq!(stmt.options.len(), 1);
		assert!(matches!(stmt.options[0], SequenceOption::Cache(20)));
	}

	#[rstest]
	fn test_alter_sequence_cycle() {
		let mut stmt = AlterSequenceStatement::new();
		stmt.name("my_seq").cycle();
		assert_eq!(stmt.options.len(), 1);
		assert!(matches!(stmt.options[0], SequenceOption::Cycle));
	}

	#[rstest]
	fn test_alter_sequence_no_cycle() {
		let mut stmt = AlterSequenceStatement::new();
		stmt.name("my_seq").no_cycle();
		assert_eq!(stmt.options.len(), 1);
		assert!(matches!(stmt.options[0], SequenceOption::NoCycle));
	}

	#[rstest]
	fn test_alter_sequence_owned_by_column() {
		let mut stmt = AlterSequenceStatement::new();
		stmt.name("my_seq").owned_by_column("my_table", "id");
		assert_eq!(stmt.options.len(), 1);
		match &stmt.options[0] {
			SequenceOption::OwnedBy(OwnedBy::Column { table, column }) => {
				assert_eq!(table.to_string(), "my_table");
				assert_eq!(column.to_string(), "id");
			}
			_ => panic!("Expected OwnedBy::Column"),
		}
	}

	#[rstest]
	fn test_alter_sequence_owned_by_none() {
		let mut stmt = AlterSequenceStatement::new();
		stmt.name("my_seq").owned_by_none();
		assert_eq!(stmt.options.len(), 1);
		assert!(matches!(
			stmt.options[0],
			SequenceOption::OwnedBy(OwnedBy::None)
		));
	}

	#[rstest]
	fn test_alter_sequence_multiple_options() {
		let mut stmt = AlterSequenceStatement::new();
		stmt.name("my_seq")
			.increment_by(5)
			.min_value(1)
			.max_value(1000)
			.cache(20)
			.cycle();
		assert_eq!(stmt.options.len(), 5);
	}

	#[rstest]
	fn test_alter_sequence_take() {
		let mut stmt = AlterSequenceStatement::new();
		stmt.name("my_seq").increment_by(5);
		let taken = stmt.take();
		assert!(stmt.name.to_string().is_empty());
		assert!(stmt.options.is_empty());
		assert_eq!(taken.name.to_string(), "my_seq");
		assert_eq!(taken.options.len(), 1);
	}
}
