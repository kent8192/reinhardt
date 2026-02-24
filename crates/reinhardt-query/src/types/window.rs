//! Window function types and structures.
//!
//! This module defines types for SQL window functions, including:
//! - Frame types (RANGE, ROWS, GROUPS)
//! - Frame boundaries (UNBOUNDED PRECEDING, CURRENT ROW, etc.)
//! - Frame clauses
//! - Window specifications (PARTITION BY, ORDER BY, frame clauses)

use crate::{expr::SimpleExpr, types::order::OrderExpr};

/// Frame type for window functions.
///
/// Defines how the frame is calculated:
/// - `Range`: Frame based on value range
/// - `Rows`: Frame based on physical row positions
/// - `Groups`: Frame based on peer groups (PostgreSQL only)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameType {
	/// RANGE frame type.
	Range,
	/// ROWS frame type.
	Rows,
	/// GROUPS frame type (PostgreSQL only).
	///
	/// Using this with MySQL or SQLite will cause a panic during SQL generation.
	Groups,
}

/// Frame boundary for window functions.
///
/// Defines the start or end boundary of a window frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
	/// UNBOUNDED PRECEDING - beginning of partition.
	UnboundedPreceding,
	/// N PRECEDING - N rows/values before current row.
	Preceding(i64),
	/// CURRENT ROW - the current row.
	CurrentRow,
	/// N FOLLOWING - N rows/values after current row.
	Following(i64),
	/// UNBOUNDED FOLLOWING - end of partition.
	UnboundedFollowing,
}

/// Frame clause for window functions.
///
/// Defines the frame boundaries for a window function.
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::types::{FrameClause, FrameType, Frame};
///
/// // ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
/// let frame = FrameClause {
///     frame_type: FrameType::Rows,
///     start: Frame::UnboundedPreceding,
///     end: Some(Frame::CurrentRow),
/// };
///
/// // RANGE BETWEEN 1 PRECEDING AND 1 FOLLOWING
/// let frame2 = FrameClause {
///     frame_type: FrameType::Range,
///     start: Frame::Preceding(1),
///     end: Some(Frame::Following(1)),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct FrameClause {
	/// Frame type (RANGE, ROWS, or GROUPS).
	pub frame_type: FrameType,
	/// Start boundary of the frame.
	pub start: Frame,
	/// End boundary of the frame (if `None`, only start is specified).
	pub end: Option<Frame>,
}

/// Window specification for window functions.
///
/// Defines a complete window specification including:
/// - `PARTITION BY` clause
/// - `ORDER BY` clause
/// - Frame clause (optional)
///
/// # Examples
///
/// ```rust
/// use reinhardt_query::{
///     types::{WindowStatement, FrameClause, FrameType, Frame, OrderExpr, Order},
///     expr::Expr,
/// };
///
/// // PARTITION BY department_id ORDER BY salary DESC
/// let window = WindowStatement {
///     partition_by: vec![Expr::col("department_id").into_simple_expr()],
///     order_by: vec![],
///     frame: None,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct WindowStatement {
	/// PARTITION BY expressions.
	pub partition_by: Vec<SimpleExpr>,
	/// ORDER BY expressions.
	pub order_by: Vec<OrderExpr>,
	/// Optional frame clause.
	pub frame: Option<FrameClause>,
}

impl WindowStatement {
	/// Creates a new empty window specification.
	pub fn new() -> Self {
		Self {
			partition_by: Vec::new(),
			order_by: Vec::new(),
			frame: None,
		}
	}
}

impl Default for WindowStatement {
	fn default() -> Self {
		Self::new()
	}
}
