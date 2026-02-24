//! Sequence type definitions
//!
//! This module provides types for sequence-related DDL operations:
//!
//! - [`SequenceDef`]: Sequence definition for CREATE SEQUENCE
//! - [`SequenceOption`]: Options for ALTER SEQUENCE operations

use crate::types::{DynIden, IntoIden};

/// Sequence definition for CREATE SEQUENCE
///
/// This struct represents a sequence definition, including its name
/// and various options like increment, min/max values, start value, cache, cycle, and ownership.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::sequence::SequenceDef;
///
/// // CREATE SEQUENCE my_seq
/// let seq = SequenceDef::new("my_seq");
///
/// // CREATE SEQUENCE my_seq INCREMENT BY 5
/// let seq = SequenceDef::new("my_seq")
///     .increment(5);
///
/// // CREATE SEQUENCE my_seq START WITH 100 MINVALUE 1 MAXVALUE 1000
/// let seq = SequenceDef::new("my_seq")
///     .start(100)
///     .min_value(Some(1))
///     .max_value(Some(1000));
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SequenceDef {
	pub(crate) name: DynIden,
	pub(crate) if_not_exists: bool,
	pub(crate) increment: Option<i64>,
	pub(crate) min_value: Option<Option<i64>>,
	pub(crate) max_value: Option<Option<i64>>,
	pub(crate) start: Option<i64>,
	pub(crate) cache: Option<i64>,
	pub(crate) cycle: Option<bool>,
	pub(crate) owned_by: Option<OwnedBy>,
}

/// Ownership specification for sequences
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum OwnedBy {
	/// OWNED BY table.column
	Column { table: DynIden, column: DynIden },
	/// OWNED BY NONE
	None,
}

/// Sequence option for ALTER SEQUENCE operations
///
/// This enum represents various options that can be modified using ALTER SEQUENCE.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::sequence::SequenceOption;
///
/// // RESTART
/// let opt = SequenceOption::Restart(None);
///
/// // RESTART WITH 100
/// let opt = SequenceOption::Restart(Some(100));
///
/// // INCREMENT BY 5
/// let opt = SequenceOption::IncrementBy(5);
/// ```
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SequenceOption {
	/// RESTART [WITH value]
	Restart(Option<i64>),
	/// INCREMENT BY value
	IncrementBy(i64),
	/// MINVALUE value
	MinValue(i64),
	/// NO MINVALUE
	NoMinValue,
	/// MAXVALUE value
	MaxValue(i64),
	/// NO MAXVALUE
	NoMaxValue,
	/// CACHE value
	Cache(i64),
	/// CYCLE
	Cycle,
	/// NO CYCLE
	NoCycle,
	/// OWNED BY table.column or OWNED BY NONE
	OwnedBy(OwnedBy),
}

impl SequenceDef {
	/// Create a new sequence definition
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::sequence::SequenceDef;
	///
	/// let seq = SequenceDef::new("my_seq");
	/// ```
	pub fn new<N: IntoIden>(name: N) -> Self {
		Self {
			name: name.into_iden(),
			if_not_exists: false,
			increment: None,
			min_value: None,
			max_value: None,
			start: None,
			cache: None,
			cycle: None,
			owned_by: None,
		}
	}

	/// Set IF NOT EXISTS clause
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::sequence::SequenceDef;
	///
	/// let seq = SequenceDef::new("my_seq")
	///     .if_not_exists(true);
	/// ```
	pub fn if_not_exists(mut self, if_not_exists: bool) -> Self {
		self.if_not_exists = if_not_exists;
		self
	}

	/// Set INCREMENT BY value
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::sequence::SequenceDef;
	///
	/// let seq = SequenceDef::new("my_seq")
	///     .increment(5);
	/// ```
	pub fn increment(mut self, increment: i64) -> Self {
		self.increment = Some(increment);
		self
	}

	/// Set MINVALUE
	///
	/// Use `None` for NO MINVALUE, or `Some(value)` for specific minimum.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::sequence::SequenceDef;
	///
	/// // MINVALUE 1
	/// let seq = SequenceDef::new("my_seq")
	///     .min_value(Some(1));
	///
	/// // NO MINVALUE
	/// let seq = SequenceDef::new("my_seq")
	///     .min_value(None);
	/// ```
	pub fn min_value(mut self, min_value: Option<i64>) -> Self {
		self.min_value = Some(min_value);
		self
	}

	/// Set MAXVALUE
	///
	/// Use `None` for NO MAXVALUE, or `Some(value)` for specific maximum.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::sequence::SequenceDef;
	///
	/// // MAXVALUE 1000
	/// let seq = SequenceDef::new("my_seq")
	///     .max_value(Some(1000));
	///
	/// // NO MAXVALUE
	/// let seq = SequenceDef::new("my_seq")
	///     .max_value(None);
	/// ```
	pub fn max_value(mut self, max_value: Option<i64>) -> Self {
		self.max_value = Some(max_value);
		self
	}

	/// Set START WITH value
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::sequence::SequenceDef;
	///
	/// let seq = SequenceDef::new("my_seq")
	///     .start(100);
	/// ```
	pub fn start(mut self, start: i64) -> Self {
		self.start = Some(start);
		self
	}

	/// Set CACHE value
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::sequence::SequenceDef;
	///
	/// let seq = SequenceDef::new("my_seq")
	///     .cache(20);
	/// ```
	pub fn cache(mut self, cache: i64) -> Self {
		self.cache = Some(cache);
		self
	}

	/// Set CYCLE or NO CYCLE
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::sequence::SequenceDef;
	///
	/// // CYCLE
	/// let seq = SequenceDef::new("my_seq")
	///     .cycle(true);
	///
	/// // NO CYCLE
	/// let seq = SequenceDef::new("my_seq")
	///     .cycle(false);
	/// ```
	pub fn cycle(mut self, cycle: bool) -> Self {
		self.cycle = Some(cycle);
		self
	}

	/// Set OWNED BY table.column
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::sequence::SequenceDef;
	///
	/// let seq = SequenceDef::new("my_seq")
	///     .owned_by_column("my_table", "id");
	/// ```
	pub fn owned_by_column<T: IntoIden, C: IntoIden>(mut self, table: T, column: C) -> Self {
		self.owned_by = Some(OwnedBy::Column {
			table: table.into_iden(),
			column: column.into_iden(),
		});
		self
	}

	/// Set OWNED BY NONE
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_query::types::sequence::SequenceDef;
	///
	/// let seq = SequenceDef::new("my_seq")
	///     .owned_by_none();
	/// ```
	pub fn owned_by_none(mut self) -> Self {
		self.owned_by = Some(OwnedBy::None);
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn test_sequence_def_basic() {
		let seq = SequenceDef::new("my_seq");
		assert_eq!(seq.name.to_string(), "my_seq");
		assert!(!seq.if_not_exists);
		assert!(seq.increment.is_none());
		assert!(seq.min_value.is_none());
		assert!(seq.max_value.is_none());
		assert!(seq.start.is_none());
		assert!(seq.cache.is_none());
		assert!(seq.cycle.is_none());
		assert!(seq.owned_by.is_none());
	}

	#[rstest]
	fn test_sequence_def_if_not_exists() {
		let seq = SequenceDef::new("my_seq").if_not_exists(true);
		assert_eq!(seq.name.to_string(), "my_seq");
		assert!(seq.if_not_exists);
	}

	#[rstest]
	fn test_sequence_def_increment() {
		let seq = SequenceDef::new("my_seq").increment(5);
		assert_eq!(seq.increment, Some(5));
	}

	#[rstest]
	fn test_sequence_def_min_max_values() {
		let seq = SequenceDef::new("my_seq")
			.min_value(Some(1))
			.max_value(Some(1000));
		assert_eq!(seq.min_value, Some(Some(1)));
		assert_eq!(seq.max_value, Some(Some(1000)));
	}

	#[rstest]
	fn test_sequence_def_no_min_max_values() {
		let seq = SequenceDef::new("my_seq").min_value(None).max_value(None);
		assert_eq!(seq.min_value, Some(None));
		assert_eq!(seq.max_value, Some(None));
	}

	#[rstest]
	fn test_sequence_def_start() {
		let seq = SequenceDef::new("my_seq").start(100);
		assert_eq!(seq.start, Some(100));
	}

	#[rstest]
	fn test_sequence_def_cache() {
		let seq = SequenceDef::new("my_seq").cache(20);
		assert_eq!(seq.cache, Some(20));
	}

	#[rstest]
	fn test_sequence_def_cycle() {
		let seq = SequenceDef::new("my_seq").cycle(true);
		assert_eq!(seq.cycle, Some(true));
	}

	#[rstest]
	fn test_sequence_def_no_cycle() {
		let seq = SequenceDef::new("my_seq").cycle(false);
		assert_eq!(seq.cycle, Some(false));
	}

	#[rstest]
	fn test_sequence_def_owned_by_column() {
		let seq = SequenceDef::new("my_seq").owned_by_column("my_table", "id");
		match seq.owned_by {
			Some(OwnedBy::Column { table, column }) => {
				assert_eq!(table.to_string(), "my_table");
				assert_eq!(column.to_string(), "id");
			}
			_ => panic!("Expected OwnedBy::Column"),
		}
	}

	#[rstest]
	fn test_sequence_def_owned_by_none() {
		let seq = SequenceDef::new("my_seq").owned_by_none();
		assert!(matches!(seq.owned_by, Some(OwnedBy::None)));
	}

	#[rstest]
	fn test_sequence_def_all_options() {
		let seq = SequenceDef::new("my_seq")
			.if_not_exists(true)
			.increment(5)
			.min_value(Some(1))
			.max_value(Some(1000))
			.start(100)
			.cache(20)
			.cycle(true)
			.owned_by_column("my_table", "id");

		assert_eq!(seq.name.to_string(), "my_seq");
		assert!(seq.if_not_exists);
		assert_eq!(seq.increment, Some(5));
		assert_eq!(seq.min_value, Some(Some(1)));
		assert_eq!(seq.max_value, Some(Some(1000)));
		assert_eq!(seq.start, Some(100));
		assert_eq!(seq.cache, Some(20));
		assert_eq!(seq.cycle, Some(true));
		match seq.owned_by {
			Some(OwnedBy::Column { table, column }) => {
				assert_eq!(table.to_string(), "my_table");
				assert_eq!(column.to_string(), "id");
			}
			_ => panic!("Expected OwnedBy::Column"),
		}
	}
}
