//! Order and ordering types for SQL queries.
//!
//! This module provides types for ORDER BY clauses:
//!
//! - [`Order`]: Ascending or descending order
//! - [`NullOrdering`]: NULL ordering (NULLS FIRST/LAST)
//! - [`OrderExpr`]: An expression with its ordering specification

use super::iden::DynIden;

/// Ordering direction for ORDER BY clauses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Order {
	/// Ascending order (ASC)
	#[default]
	Asc,
	/// Descending order (DESC)
	Desc,
}

impl Order {
	/// Returns the SQL representation of this order.
	#[must_use]
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Asc => "ASC",
			Self::Desc => "DESC",
		}
	}
}

/// NULL ordering specification.
///
/// Used to specify whether NULL values should appear first or last
/// in the ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NullOrdering {
	/// NULLS FIRST - NULL values appear before non-NULL values
	First,
	/// NULLS LAST - NULL values appear after non-NULL values
	Last,
}

impl NullOrdering {
	/// Returns the SQL representation of this null ordering.
	#[must_use]
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::First => "NULLS FIRST",
			Self::Last => "NULLS LAST",
		}
	}
}

/// An expression with its ordering specification.
///
/// This represents a single item in an ORDER BY clause.
#[derive(Debug, Clone)]
pub struct OrderExpr {
	/// The expression to order by (column or expression)
	pub expr: OrderExprKind,
	/// The ordering direction
	pub order: Order,
	/// Optional NULL ordering
	pub nulls: Option<NullOrdering>,
}

/// The kind of expression in an ORDER BY clause.
#[derive(Debug, Clone)]
pub enum OrderExprKind {
	/// A column identifier
	Column(DynIden),
	/// A qualified column (table.column)
	TableColumn(DynIden, DynIden),
	/// An expression (requires expr module)
	Expr(Box<crate::expr::SimpleExpr>),
}

impl OrderExpr {
	/// Create a new order expression for a column with the default order (ASC).
	pub fn new<I: super::iden::IntoIden>(column: I) -> Self {
		Self {
			expr: OrderExprKind::Column(column.into_iden()),
			order: Order::Asc,
			nulls: None,
		}
	}

	/// Create a new order expression for a qualified column.
	pub fn new_table_column<T: super::iden::IntoIden, C: super::iden::IntoIden>(
		table: T,
		column: C,
	) -> Self {
		Self {
			expr: OrderExprKind::TableColumn(table.into_iden(), column.into_iden()),
			order: Order::Asc,
			nulls: None,
		}
	}

	/// Set the ordering direction.
	#[must_use]
	pub fn order(mut self, order: Order) -> Self {
		self.order = order;
		self
	}

	/// Set NULL ordering.
	#[must_use]
	pub fn nulls(mut self, nulls: NullOrdering) -> Self {
		self.nulls = Some(nulls);
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_order_as_str() {
		assert_eq!(Order::Asc.as_str(), "ASC");
		assert_eq!(Order::Desc.as_str(), "DESC");
	}

	#[rstest]
	fn test_order_default() {
		assert_eq!(Order::default(), Order::Asc);
	}

	#[rstest]
	fn test_null_ordering_as_str() {
		assert_eq!(NullOrdering::First.as_str(), "NULLS FIRST");
		assert_eq!(NullOrdering::Last.as_str(), "NULLS LAST");
	}

	#[rstest]
	fn test_order_expr_builder() {
		let expr = OrderExpr::new("column_name")
			.order(Order::Desc)
			.nulls(NullOrdering::Last);

		assert_eq!(expr.order, Order::Desc);
		assert_eq!(expr.nulls, Some(NullOrdering::Last));
	}
}
