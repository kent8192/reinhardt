//! Join types for SQL queries.
//!
//! This module provides types for JOIN operations:
//!
//! - [`JoinType`]: The type of join (INNER, LEFT, RIGHT, etc.)
//! - [`JoinOn`]: Join condition specification
//! - [`JoinExpr`]: Complete join expression

use super::{iden::DynIden, table_ref::TableRef};
use crate::expr::{Condition, IntoCondition};

/// SQL JOIN types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JoinType {
	/// INNER JOIN - returns rows when there is a match in both tables
	Join,
	/// INNER JOIN (explicit)
	InnerJoin,
	/// LEFT JOIN - returns all rows from the left table
	LeftJoin,
	/// RIGHT JOIN - returns all rows from the right table
	RightJoin,
	/// FULL OUTER JOIN - returns rows when there is a match in one of the tables
	FullOuterJoin,
	/// CROSS JOIN - cartesian product of both tables
	CrossJoin,
}

impl JoinType {
	/// Returns the SQL representation of this join type.
	#[must_use]
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Join => "JOIN",
			Self::InnerJoin => "INNER JOIN",
			Self::LeftJoin => "LEFT JOIN",
			Self::RightJoin => "RIGHT JOIN",
			Self::FullOuterJoin => "FULL OUTER JOIN",
			Self::CrossJoin => "CROSS JOIN",
		}
	}
}

/// Join condition specification.
///
/// Represents the ON or USING clause of a JOIN.
#[derive(Debug, Clone)]
pub enum JoinOn {
	/// ON condition with two column references (simple case)
	Columns(ColumnPair),
	/// ON condition with complex expression
	Condition(Condition),
	/// USING (column_list) clause
	Using(Vec<DynIden>),
}

/// A pair of columns for join conditions.
#[derive(Debug, Clone)]
pub struct ColumnPair {
	/// Left column (from the main table)
	pub left: ColumnSpec,
	/// Right column (from the joined table)
	pub right: ColumnSpec,
}

/// Column specification that can be simple or qualified.
#[derive(Debug, Clone)]
pub enum ColumnSpec {
	/// Simple column name
	Column(DynIden),
	/// Table-qualified column (table.column)
	TableColumn(DynIden, DynIden),
}

impl ColumnSpec {
	/// Create a simple column specification.
	pub fn column<I: super::iden::IntoIden>(column: I) -> Self {
		Self::Column(column.into_iden())
	}

	/// Create a table-qualified column specification.
	pub fn table_column<T: super::iden::IntoIden, C: super::iden::IntoIden>(
		table: T,
		column: C,
	) -> Self {
		Self::TableColumn(table.into_iden(), column.into_iden())
	}
}

/// A complete join expression.
///
/// This represents a complete JOIN clause including the join type,
/// target table, and join condition.
#[derive(Debug, Clone)]
pub struct JoinExpr {
	/// The type of join
	pub join: JoinType,
	/// The table to join
	pub table: TableRef,
	/// The join condition
	pub on: Option<JoinOn>,
}

impl JoinExpr {
	/// Create a new join expression with an INNER JOIN.
	pub fn new(table: TableRef) -> Self {
		Self {
			join: JoinType::InnerJoin,
			table,
			on: None,
		}
	}

	/// Set the join type.
	#[must_use]
	pub fn join_type(mut self, join: JoinType) -> Self {
		self.join = join;
		self
	}

	/// Set the join condition.
	#[must_use]
	pub fn on(mut self, condition: JoinOn) -> Self {
		self.on = Some(condition);
		self
	}

	/// Create a join condition on two columns.
	#[must_use]
	pub fn on_columns(mut self, left: ColumnSpec, right: ColumnSpec) -> Self {
		self.on = Some(JoinOn::Columns(ColumnPair { left, right }));
		self
	}

	/// Create a join condition with a complex expression.
	#[must_use]
	pub fn on_condition<C: IntoCondition>(mut self, condition: C) -> Self {
		self.on = Some(JoinOn::Condition(condition.into_condition()));
		self
	}

	/// Create a USING clause with column names.
	#[must_use]
	pub fn using_columns<I, C>(mut self, columns: I) -> Self
	where
		I: IntoIterator<Item = C>,
		C: super::iden::IntoIden,
	{
		let cols: Vec<DynIden> = columns.into_iter().map(|c| c.into_iden()).collect();
		self.on = Some(JoinOn::Using(cols));
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::types::iden::IntoIden;
	use rstest::rstest;

	#[rstest]
	#[case(JoinType::Join, "JOIN")]
	#[case(JoinType::InnerJoin, "INNER JOIN")]
	#[case(JoinType::LeftJoin, "LEFT JOIN")]
	#[case(JoinType::RightJoin, "RIGHT JOIN")]
	#[case(JoinType::FullOuterJoin, "FULL OUTER JOIN")]
	#[case(JoinType::CrossJoin, "CROSS JOIN")]
	fn test_join_type_as_str(#[case] join_type: JoinType, #[case] expected: &str) {
		assert_eq!(join_type.as_str(), expected);
	}

	#[rstest]
	fn test_column_spec_simple() {
		let _spec = ColumnSpec::column("id");
	}

	#[rstest]
	fn test_column_spec_qualified() {
		let _spec = ColumnSpec::table_column("users", "id");
	}

	#[rstest]
	fn test_join_expr_builder() {
		use crate::types::Alias;

		let table = TableRef::Table(Alias::new("posts").into_iden());
		let join = JoinExpr::new(table)
			.join_type(JoinType::LeftJoin)
			.on_columns(
				ColumnSpec::table_column("users", "id"),
				ColumnSpec::table_column("posts", "user_id"),
			);

		assert_eq!(join.join, JoinType::LeftJoin);
		assert!(join.on.is_some());
	}
}
