/// Window functions similar to Django's window functions
use serde::{Deserialize, Serialize};

/// Window frame type
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FrameType {
	/// Range variant.
	Range,
	/// Rows variant.
	Rows,
	/// Groups variant.
	Groups,
}

impl FrameType {
	/// Convert frame type to SQL keyword
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::FrameType;
	///
	/// assert_eq!(FrameType::Range.to_sql(), "RANGE");
	/// assert_eq!(FrameType::Rows.to_sql(), "ROWS");
	/// assert_eq!(FrameType::Groups.to_sql(), "GROUPS");
	/// ```
	pub fn to_sql(&self) -> &'static str {
		match self {
			FrameType::Range => "RANGE",
			FrameType::Rows => "ROWS",
			FrameType::Groups => "GROUPS",
		}
	}
}

/// Frame boundary
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FrameBoundary {
	/// UnboundedPreceding variant.
	UnboundedPreceding,
	/// Preceding variant.
	Preceding(i64),
	/// CurrentRow variant.
	CurrentRow,
	/// Following variant.
	Following(i64),
	/// UnboundedFollowing variant.
	UnboundedFollowing,
}

impl FrameBoundary {
	/// Convert frame boundary to SQL expression
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::FrameBoundary;
	///
	/// assert_eq!(FrameBoundary::UnboundedPreceding.to_sql(), "UNBOUNDED PRECEDING");
	/// assert_eq!(FrameBoundary::Preceding(5).to_sql(), "5 PRECEDING");
	/// assert_eq!(FrameBoundary::CurrentRow.to_sql(), "CURRENT ROW");
	/// assert_eq!(FrameBoundary::Following(3).to_sql(), "3 FOLLOWING");
	/// assert_eq!(FrameBoundary::UnboundedFollowing.to_sql(), "UNBOUNDED FOLLOWING");
	/// ```
	pub fn to_sql(&self) -> String {
		match self {
			FrameBoundary::UnboundedPreceding => "UNBOUNDED PRECEDING".to_string(),
			FrameBoundary::Preceding(n) => format!("{} PRECEDING", n),
			FrameBoundary::CurrentRow => "CURRENT ROW".to_string(),
			FrameBoundary::Following(n) => format!("{} FOLLOWING", n),
			FrameBoundary::UnboundedFollowing => "UNBOUNDED FOLLOWING".to_string(),
		}
	}
}

/// Window frame specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
	/// The frame type.
	pub frame_type: FrameType,
	/// The start.
	pub start: FrameBoundary,
	/// The end.
	pub end: Option<FrameBoundary>,
}

impl Frame {
	/// Create a ROWS frame specification
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{Frame, FrameBoundary};
	///
	/// let frame = Frame::rows(
	///     FrameBoundary::UnboundedPreceding,
	///     Some(FrameBoundary::CurrentRow)
	/// );
	/// assert_eq!(frame.to_sql(), "ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW");
	/// ```
	pub fn rows(start: FrameBoundary, end: Option<FrameBoundary>) -> Self {
		Self {
			frame_type: FrameType::Rows,
			start,
			end,
		}
	}
	/// Create a RANGE frame specification
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{Frame, FrameBoundary};
	///
	/// let frame = Frame::range(
	///     FrameBoundary::Preceding(3),
	///     Some(FrameBoundary::Following(3))
	/// );
	/// assert_eq!(frame.to_sql(), "RANGE BETWEEN 3 PRECEDING AND 3 FOLLOWING");
	/// ```
	pub fn range(start: FrameBoundary, end: Option<FrameBoundary>) -> Self {
		Self {
			frame_type: FrameType::Range,
			start,
			end,
		}
	}
	/// Create a GROUPS frame specification
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{Frame, FrameBoundary};
	///
	/// let frame = Frame::groups(
	///     FrameBoundary::CurrentRow,
	///     Some(FrameBoundary::UnboundedFollowing)
	/// );
	/// assert_eq!(frame.to_sql(), "GROUPS BETWEEN CURRENT ROW AND UNBOUNDED FOLLOWING");
	/// ```
	pub fn groups(start: FrameBoundary, end: Option<FrameBoundary>) -> Self {
		Self {
			frame_type: FrameType::Groups,
			start,
			end,
		}
	}
	/// Convert frame to SQL BETWEEN clause
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{Frame, FrameBoundary};
	///
	/// let frame = Frame::rows(FrameBoundary::Preceding(1), None);
	/// assert_eq!(frame.to_sql(), "ROWS BETWEEN 1 PRECEDING AND CURRENT ROW");
	/// ```
	pub fn to_sql(&self) -> String {
		let mut sql = format!(
			"{} BETWEEN {}",
			self.frame_type.to_sql(),
			self.start.to_sql()
		);

		if let Some(ref end) = self.end {
			sql.push_str(&format!(" AND {}", end.to_sql()));
		} else {
			sql.push_str(" AND CURRENT ROW");
		}

		sql
	}
}

/// Window specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Window {
	/// The partition by.
	pub partition_by: Vec<String>,
	/// The order by.
	pub order_by: Vec<String>,
	/// The frame.
	pub frame: Option<Frame>,
}

impl Window {
	/// Create a new empty window specification
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::Window;
	///
	/// let window = Window::new();
	/// assert_eq!(window.to_sql(), "");
	/// ```
	pub fn new() -> Self {
		Self {
			partition_by: Vec::new(),
			order_by: Vec::new(),
			frame: None,
		}
	}
	/// Add a PARTITION BY clause
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::Window;
	///
	/// let window = Window::new().partition_by("department");
	/// assert_eq!(window.to_sql(), "PARTITION BY department");
	/// ```
	pub fn partition_by(mut self, field: impl Into<String>) -> Self {
		self.partition_by.push(field.into());
		self
	}
	/// Add an ORDER BY clause
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::Window;
	///
	/// let window = Window::new().order_by("salary DESC");
	/// assert_eq!(window.to_sql(), "ORDER BY salary DESC");
	/// ```
	pub fn order_by(mut self, field: impl Into<String>) -> Self {
		self.order_by.push(field.into());
		self
	}
	/// Add a frame specification
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{Window, Frame, FrameBoundary};
	///
	/// let frame = Frame::rows(
	///     FrameBoundary::UnboundedPreceding,
	///     Some(FrameBoundary::CurrentRow)
	/// );
	/// let window = Window::new().order_by("date").frame(frame);
	/// assert_eq!(window.to_sql(), "ORDER BY date ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW");
	/// ```
	pub fn frame(mut self, frame: Frame) -> Self {
		self.frame = Some(frame);
		self
	}
	/// Convert window specification to SQL
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::Window;
	///
	/// let window = Window::new()
	///     .partition_by("department")
	///     .order_by("salary DESC");
	/// assert_eq!(window.to_sql(), "PARTITION BY department ORDER BY salary DESC");
	/// ```
	pub fn to_sql(&self) -> String {
		let mut parts = Vec::new();

		if !self.partition_by.is_empty() {
			parts.push(format!("PARTITION BY {}", self.partition_by.join(", ")));
		}

		if !self.order_by.is_empty() {
			parts.push(format!("ORDER BY {}", self.order_by.join(", ")));
		}

		if let Some(ref frame) = self.frame {
			parts.push(frame.to_sql());
		}

		parts.join(" ")
	}
}

impl Default for Window {
	fn default() -> Self {
		Self::new()
	}
}

/// Base trait for window functions
pub trait WindowFunction {
	/// Converts the window function to SQL using the given window definition.
	fn to_sql(&self, window: &Window) -> String;
}

/// ROW_NUMBER window function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowNumber;

impl RowNumber {
	/// Create a ROW_NUMBER window function
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{RowNumber, Window, WindowFunction};
	///
	/// let window = Window::new()
	///     .partition_by("department")
	///     .order_by("hire_date");
	/// let row_num = RowNumber::new();
	/// assert_eq!(
	///     row_num.to_sql(&window),
	///     "ROW_NUMBER() OVER (PARTITION BY department ORDER BY hire_date)"
	/// );
	/// ```
	pub fn new() -> Self {
		Self
	}
}

impl Default for RowNumber {
	fn default() -> Self {
		Self::new()
	}
}

impl WindowFunction for RowNumber {
	fn to_sql(&self, window: &Window) -> String {
		format!("ROW_NUMBER() OVER ({})", window.to_sql())
	}
}

/// RANK window function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rank;

impl Rank {
	/// Create a RANK window function
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{Rank, Window, WindowFunction};
	///
	/// let window = Window::new()
	///     .partition_by("department")
	///     .order_by("salary DESC");
	/// let rank = Rank::new();
	/// assert_eq!(
	///     rank.to_sql(&window),
	///     "RANK() OVER (PARTITION BY department ORDER BY salary DESC)"
	/// );
	/// ```
	pub fn new() -> Self {
		Self
	}
}

impl Default for Rank {
	fn default() -> Self {
		Self::new()
	}
}

impl WindowFunction for Rank {
	fn to_sql(&self, window: &Window) -> String {
		format!("RANK() OVER ({})", window.to_sql())
	}
}

/// DENSE_RANK window function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenseRank;

impl DenseRank {
	/// Create a DENSE_RANK window function
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{DenseRank, Window, WindowFunction};
	///
	/// let window = Window::new().order_by("score DESC");
	/// let dense_rank = DenseRank::new();
	/// assert_eq!(
	///     dense_rank.to_sql(&window),
	///     "DENSE_RANK() OVER (ORDER BY score DESC)"
	/// );
	/// ```
	pub fn new() -> Self {
		Self
	}
}

impl Default for DenseRank {
	fn default() -> Self {
		Self::new()
	}
}

impl WindowFunction for DenseRank {
	fn to_sql(&self, window: &Window) -> String {
		format!("DENSE_RANK() OVER ({})", window.to_sql())
	}
}

/// NTILE window function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NTile {
	/// The num buckets.
	pub num_buckets: i64,
}

impl NTile {
	/// Create an NTILE window function to divide rows into buckets
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{NTile, Window, WindowFunction};
	///
	/// let window = Window::new().order_by("salary");
	/// let ntile = NTile::new(4);
	/// assert_eq!(ntile.to_sql(&window), "NTILE(4) OVER (ORDER BY salary)");
	/// ```
	pub fn new(num_buckets: i64) -> Self {
		Self { num_buckets }
	}
}

impl WindowFunction for NTile {
	fn to_sql(&self, window: &Window) -> String {
		format!("NTILE({}) OVER ({})", self.num_buckets, window.to_sql())
	}
}

/// LEAD window function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lead {
	/// The expression.
	pub expression: String,
	/// The offset.
	pub offset: i64,
	/// The default.
	pub default: Option<String>,
}

impl Lead {
	/// Create a LEAD window function to access following rows
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{Lead, Window, WindowFunction};
	///
	/// let window = Window::new().order_by("date");
	/// let lead = Lead::new("value");
	/// assert_eq!(lead.to_sql(&window), "LEAD(value, 1) OVER (ORDER BY date)");
	/// ```
	pub fn new(expression: impl Into<String>) -> Self {
		Self {
			expression: expression.into(),
			offset: 1,
			default: None,
		}
	}
	/// Set the offset for how many rows to look ahead
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{Lead, Window, WindowFunction};
	///
	/// let window = Window::new().order_by("date");
	/// let lead = Lead::new("value").offset(2);
	/// assert_eq!(lead.to_sql(&window), "LEAD(value, 2) OVER (ORDER BY date)");
	/// ```
	pub fn offset(mut self, offset: i64) -> Self {
		self.offset = offset;
		self
	}
	/// Set the default value when no following row exists
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{Lead, Window, WindowFunction};
	///
	/// let window = Window::new().order_by("date");
	/// let lead = Lead::new("value").offset(2).default("0");
	/// assert_eq!(lead.to_sql(&window), "LEAD(value, 2, 0) OVER (ORDER BY date)");
	/// ```
	pub fn default(mut self, default: impl Into<String>) -> Self {
		self.default = Some(default.into());
		self
	}
}

impl WindowFunction for Lead {
	fn to_sql(&self, window: &Window) -> String {
		let mut args = vec![self.expression.clone(), self.offset.to_string()];
		if let Some(ref default) = self.default {
			args.push(default.clone());
		}
		format!("LEAD({}) OVER ({})", args.join(", "), window.to_sql())
	}
}

/// LAG window function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lag {
	/// The expression.
	pub expression: String,
	/// The offset.
	pub offset: i64,
	/// The default.
	pub default: Option<String>,
}

impl Lag {
	/// Create a LAG window function to access preceding rows
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{Lag, Window, WindowFunction};
	///
	/// let window = Window::new().order_by("date");
	/// let lag = Lag::new("value");
	/// assert_eq!(lag.to_sql(&window), "LAG(value, 1) OVER (ORDER BY date)");
	/// ```
	pub fn new(expression: impl Into<String>) -> Self {
		Self {
			expression: expression.into(),
			offset: 1,
			default: None,
		}
	}
	/// Set the offset for how many rows to look back
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{Lag, Window, WindowFunction};
	///
	/// let window = Window::new().order_by("date");
	/// let lag = Lag::new("value").offset(3);
	/// assert_eq!(lag.to_sql(&window), "LAG(value, 3) OVER (ORDER BY date)");
	/// ```
	pub fn offset(mut self, offset: i64) -> Self {
		self.offset = offset;
		self
	}
	/// Set the default value when no preceding row exists
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{Lag, Window, WindowFunction};
	///
	/// let window = Window::new().order_by("date");
	/// let lag = Lag::new("value").default("0");
	/// assert_eq!(lag.to_sql(&window), "LAG(value, 1, 0) OVER (ORDER BY date)");
	/// ```
	pub fn default(mut self, default: impl Into<String>) -> Self {
		self.default = Some(default.into());
		self
	}
}

impl WindowFunction for Lag {
	fn to_sql(&self, window: &Window) -> String {
		let mut args = vec![self.expression.clone(), self.offset.to_string()];
		if let Some(ref default) = self.default {
			args.push(default.clone());
		}
		format!("LAG({}) OVER ({})", args.join(", "), window.to_sql())
	}
}

/// FIRST_VALUE window function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirstValue {
	/// The expression.
	pub expression: String,
}

impl FirstValue {
	/// Create a FIRST_VALUE window function
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{FirstValue, Window, WindowFunction};
	///
	/// let window = Window::new()
	///     .partition_by("department")
	///     .order_by("salary DESC");
	/// let first_val = FirstValue::new("salary");
	/// assert_eq!(
	///     first_val.to_sql(&window),
	///     "FIRST_VALUE(salary) OVER (PARTITION BY department ORDER BY salary DESC)"
	/// );
	/// ```
	pub fn new(expression: impl Into<String>) -> Self {
		Self {
			expression: expression.into(),
		}
	}
}

impl WindowFunction for FirstValue {
	fn to_sql(&self, window: &Window) -> String {
		format!(
			"FIRST_VALUE({}) OVER ({})",
			self.expression,
			window.to_sql()
		)
	}
}

/// LAST_VALUE window function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastValue {
	/// The expression.
	pub expression: String,
}

impl LastValue {
	/// Create a LAST_VALUE window function
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{LastValue, Window, WindowFunction};
	///
	/// let window = Window::new()
	///     .partition_by("department")
	///     .order_by("salary DESC");
	/// let last_val = LastValue::new("salary");
	/// assert_eq!(
	///     last_val.to_sql(&window),
	///     "LAST_VALUE(salary) OVER (PARTITION BY department ORDER BY salary DESC)"
	/// );
	/// ```
	pub fn new(expression: impl Into<String>) -> Self {
		Self {
			expression: expression.into(),
		}
	}
}

impl WindowFunction for LastValue {
	fn to_sql(&self, window: &Window) -> String {
		format!("LAST_VALUE({}) OVER ({})", self.expression, window.to_sql())
	}
}

/// NTH_VALUE window function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NthValue {
	/// The expression.
	pub expression: String,
	/// The n.
	pub n: i64,
}

impl NthValue {
	/// Create an NTH_VALUE window function
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::window::{NthValue, Window, WindowFunction};
	///
	/// let window = Window::new().order_by("salary DESC");
	/// let nth_val = NthValue::new("salary", 2);
	/// assert_eq!(
	///     nth_val.to_sql(&window),
	///     "NTH_VALUE(salary, 2) OVER (ORDER BY salary DESC)"
	/// );
	/// ```
	pub fn new(expression: impl Into<String>, n: i64) -> Self {
		Self {
			expression: expression.into(),
			n,
		}
	}
}

impl WindowFunction for NthValue {
	fn to_sql(&self, window: &Window) -> String {
		format!(
			"NTH_VALUE({}, {}) OVER ({})",
			self.expression,
			self.n,
			window.to_sql()
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_window_partition_by() {
		let window = Window::new().partition_by("department");
		assert_eq!(window.to_sql(), "PARTITION BY department");
	}

	#[test]
	fn test_window_order_by() {
		let window = Window::new().order_by("salary DESC");
		assert_eq!(window.to_sql(), "ORDER BY salary DESC");
	}

	#[test]
	fn test_window_partition_and_order() {
		let window = Window::new()
			.partition_by("department")
			.order_by("salary DESC");
		assert_eq!(
			window.to_sql(),
			"PARTITION BY department ORDER BY salary DESC"
		);
	}

	#[test]
	fn test_frame_rows() {
		let frame = Frame::rows(
			FrameBoundary::UnboundedPreceding,
			Some(FrameBoundary::CurrentRow),
		);
		assert_eq!(
			frame.to_sql(),
			"ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW"
		);
	}

	#[test]
	fn test_frame_range() {
		let frame = Frame::range(
			FrameBoundary::Preceding(3),
			Some(FrameBoundary::Following(3)),
		);
		assert_eq!(frame.to_sql(), "RANGE BETWEEN 3 PRECEDING AND 3 FOLLOWING");
	}

	#[test]
	fn test_row_number() {
		let window = Window::new()
			.partition_by("department")
			.order_by("hire_date");
		let row_num = RowNumber::new();
		assert_eq!(
			row_num.to_sql(&window),
			"ROW_NUMBER() OVER (PARTITION BY department ORDER BY hire_date)"
		);
	}

	#[test]
	fn test_rank() {
		let window = Window::new()
			.partition_by("department")
			.order_by("salary DESC");
		let rank = Rank::new();
		assert_eq!(
			rank.to_sql(&window),
			"RANK() OVER (PARTITION BY department ORDER BY salary DESC)"
		);
	}

	#[test]
	fn test_dense_rank() {
		let window = Window::new().order_by("score DESC");
		let dense_rank = DenseRank::new();
		assert_eq!(
			dense_rank.to_sql(&window),
			"DENSE_RANK() OVER (ORDER BY score DESC)"
		);
	}

	#[test]
	fn test_ntile() {
		let window = Window::new().order_by("salary");
		let ntile = NTile::new(4);
		assert_eq!(ntile.to_sql(&window), "NTILE(4) OVER (ORDER BY salary)");
	}

	#[test]
	fn test_lead() {
		let window = Window::new().order_by("date");
		let lead = Lead::new("value");
		assert_eq!(lead.to_sql(&window), "LEAD(value, 1) OVER (ORDER BY date)");
	}

	#[test]
	fn test_lead_with_offset_and_default() {
		let window = Window::new().order_by("date");
		let lead = Lead::new("value").offset(2).default("0");
		assert_eq!(
			lead.to_sql(&window),
			"LEAD(value, 2, 0) OVER (ORDER BY date)"
		);
	}

	#[test]
	fn test_lag() {
		let window = Window::new().order_by("date");
		let lag = Lag::new("value");
		assert_eq!(lag.to_sql(&window), "LAG(value, 1) OVER (ORDER BY date)");
	}

	#[test]
	fn test_lag_with_offset() {
		let window = Window::new().order_by("date");
		let lag = Lag::new("value").offset(3);
		assert_eq!(lag.to_sql(&window), "LAG(value, 3) OVER (ORDER BY date)");
	}

	#[test]
	fn test_first_value() {
		let window = Window::new()
			.partition_by("department")
			.order_by("salary DESC");
		let first_val = FirstValue::new("salary");
		assert_eq!(
			first_val.to_sql(&window),
			"FIRST_VALUE(salary) OVER (PARTITION BY department ORDER BY salary DESC)"
		);
	}

	#[test]
	fn test_last_value() {
		let window = Window::new()
			.partition_by("department")
			.order_by("salary DESC");
		let last_val = LastValue::new("salary");
		assert_eq!(
			last_val.to_sql(&window),
			"LAST_VALUE(salary) OVER (PARTITION BY department ORDER BY salary DESC)"
		);
	}

	#[test]
	fn test_nth_value() {
		let window = Window::new().order_by("salary DESC");
		let nth_val = NthValue::new("salary", 2);
		assert_eq!(
			nth_val.to_sql(&window),
			"NTH_VALUE(salary, 2) OVER (ORDER BY salary DESC)"
		);
	}

	#[test]
	fn test_window_with_frame() {
		let frame = Frame::rows(
			FrameBoundary::Preceding(1),
			Some(FrameBoundary::Following(1)),
		);
		let window = Window::new()
			.partition_by("department")
			.order_by("date")
			.frame(frame);
		assert_eq!(
			window.to_sql(),
			"PARTITION BY department ORDER BY date ROWS BETWEEN 1 PRECEDING AND 1 FOLLOWING"
		);
	}
}
