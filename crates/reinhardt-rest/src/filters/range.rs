//! Range filters for date and numeric fields
//!
//! Provides filtering capabilities for fields within a specified range.

use std::fmt::Debug;

/// Range filter for numeric and date fields
///
/// Supports filtering by exact range (inclusive/exclusive bounds),
/// greater than, less than, and between operations.
///
/// # Type Parameters
///
/// * `T` - The type of the field being filtered
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::RangeFilter;
///
/// // Numeric range
/// let price_filter: RangeFilter<i32> = RangeFilter::new("price")
///     .gte(100)
///     .lte(500);
///
/// // Date range (example with string representation)
/// let date_filter: RangeFilter<String> = RangeFilter::new("created_at")
///     .gte("2024-01-01".to_string())
///     .lt("2024-12-31".to_string());
/// // Verify filters are created successfully
/// assert!(price_filter.has_bounds());
/// assert!(date_filter.has_bounds());
/// ```
#[derive(Debug, Clone)]
pub struct RangeFilter<T> {
	/// The name of the field to filter
	pub field_name: String,
	/// Greater than or equal to (inclusive lower bound)
	pub gte: Option<T>,
	/// Greater than (exclusive lower bound)
	pub gt: Option<T>,
	/// Less than or equal to (inclusive upper bound)
	pub lte: Option<T>,
	/// Less than (exclusive upper bound)
	pub lt: Option<T>,
}

impl<T> RangeFilter<T>
where
	T: Clone + Debug,
{
	/// Create a new range filter
	///
	/// # Arguments
	///
	/// * `field_name` - The name of the field to filter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::RangeFilter;
	///
	/// let filter: RangeFilter<i32> = RangeFilter::new("age");
	/// // Verify the filter is created successfully
	/// assert_eq!(filter.field_name(), "age");
	/// ```
	pub fn new(field_name: impl Into<String>) -> Self {
		Self {
			field_name: field_name.into(),
			gte: None,
			gt: None,
			lte: None,
			lt: None,
		}
	}

	/// Set the greater than or equal to (inclusive lower bound)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::RangeFilter;
	///
	/// let filter: RangeFilter<i32> = RangeFilter::new("price")
	///     .gte(100);
	/// // Verify the lower bound is set correctly
	/// assert_eq!(filter.gte, Some(100));
	/// ```
	pub fn gte(mut self, value: T) -> Self {
		self.gte = Some(value);
		self
	}

	/// Set the greater than (exclusive lower bound)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::RangeFilter;
	///
	/// let filter: RangeFilter<i32> = RangeFilter::new("price")
	///     .gt(100);
	/// // Verify the exclusive lower bound is set correctly
	/// assert_eq!(filter.gt, Some(100));
	/// ```
	pub fn gt(mut self, value: T) -> Self {
		self.gt = Some(value);
		self
	}

	/// Set the less than or equal to (inclusive upper bound)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::RangeFilter;
	///
	/// let filter: RangeFilter<i32> = RangeFilter::new("price")
	///     .lte(500);
	/// // Verify the inclusive upper bound is set correctly
	/// assert_eq!(filter.lte, Some(500));
	/// ```
	pub fn lte(mut self, value: T) -> Self {
		self.lte = Some(value);
		self
	}

	/// Set the less than (exclusive upper bound)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::RangeFilter;
	///
	/// let filter: RangeFilter<i32> = RangeFilter::new("price")
	///     .lt(500);
	/// // Verify the exclusive upper bound is set correctly
	/// assert_eq!(filter.lt, Some(500));
	/// ```
	pub fn lt(mut self, value: T) -> Self {
		self.lt = Some(value);
		self
	}

	/// Set both bounds for an inclusive range [min, max]
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::RangeFilter;
	///
	/// let filter: RangeFilter<i32> = RangeFilter::new("price")
	///     .between(100, 500);
	/// // Verify both bounds are set correctly
	/// assert_eq!(filter.gte, Some(100));
	/// assert_eq!(filter.lte, Some(500));
	/// ```
	pub fn between(mut self, min: T, max: T) -> Self {
		self.gte = Some(min);
		self.lte = Some(max);
		self
	}

	/// Get the field name
	pub fn field_name(&self) -> &str {
		&self.field_name
	}

	/// Check if this filter has any bounds set
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::RangeFilter;
	///
	/// let empty: RangeFilter<i32> = RangeFilter::new("price");
	/// assert!(!empty.has_bounds());
	///
	/// let filtered: RangeFilter<i32> = RangeFilter::new("price").gte(100);
	/// assert!(filtered.has_bounds());
	/// ```
	pub fn has_bounds(&self) -> bool {
		self.gte.is_some() || self.gt.is_some() || self.lte.is_some() || self.lt.is_some()
	}
}

/// Date range filter
///
/// Specialized range filter for date/datetime fields with common date operations.
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::DateRangeFilter;
///
/// let filter = DateRangeFilter::new("created_at")
///     .after("2024-01-01")
///     .before("2024-12-31");
/// // Verify date range is configured correctly
/// assert_eq!(filter.inner().gte, Some("2024-01-01".to_string()));
/// assert_eq!(filter.inner().lte, Some("2024-12-31".to_string()));
/// ```
#[derive(Debug, Clone)]
pub struct DateRangeFilter {
	/// The underlying range filter
	range: RangeFilter<String>,
}

impl DateRangeFilter {
	/// Create a new date range filter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::DateRangeFilter;
	///
	/// let filter = DateRangeFilter::new("created_at");
	/// assert_eq!(filter.field_name(), "created_at");
	/// ```
	pub fn new(field_name: impl Into<String>) -> Self {
		Self {
			range: RangeFilter::new(field_name),
		}
	}

	/// Set the after date (inclusive)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::DateRangeFilter;
	///
	/// let filter = DateRangeFilter::new("created_at")
	///     .after("2024-01-01");
	/// ```
	pub fn after(mut self, date: impl Into<String>) -> Self {
		self.range = self.range.gte(date.into());
		self
	}

	/// Set the before date (inclusive)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::DateRangeFilter;
	///
	/// let filter = DateRangeFilter::new("created_at")
	///     .before("2024-12-31");
	/// ```
	pub fn before(mut self, date: impl Into<String>) -> Self {
		self.range = self.range.lte(date.into());
		self
	}

	/// Set a date range
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::DateRangeFilter;
	///
	/// let filter = DateRangeFilter::new("created_at")
	///     .range("2024-01-01", "2024-12-31");
	/// ```
	pub fn range(mut self, start: impl Into<String>, end: impl Into<String>) -> Self {
		self.range = self.range.between(start.into(), end.into());
		self
	}

	/// Get the field name
	pub fn field_name(&self) -> &str {
		self.range.field_name()
	}

	/// Get the underlying range filter
	pub fn inner(&self) -> &RangeFilter<String> {
		&self.range
	}
}

/// Numeric range filter
///
/// Specialized range filter for numeric fields with common numeric operations.
///
/// # Type Parameters
///
/// * `T` - The numeric type
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::NumericRangeFilter;
///
/// let filter: NumericRangeFilter<i32> = NumericRangeFilter::new("price")
///     .min(100)
///     .max(500);
/// // Verify numeric range is configured correctly
/// assert_eq!(filter.inner().gte, Some(100));
/// assert_eq!(filter.inner().lte, Some(500));
/// ```
#[derive(Debug, Clone)]
pub struct NumericRangeFilter<T> {
	/// The underlying range filter
	range: RangeFilter<T>,
}

impl<T> NumericRangeFilter<T>
where
	T: Clone + Debug,
{
	/// Create a new numeric range filter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::NumericRangeFilter;
	///
	/// let filter: NumericRangeFilter<i32> = NumericRangeFilter::new("price");
	/// assert_eq!(filter.field_name(), "price");
	/// ```
	pub fn new(field_name: impl Into<String>) -> Self {
		Self {
			range: RangeFilter::new(field_name),
		}
	}

	/// Set the minimum value (inclusive)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::NumericRangeFilter;
	///
	/// let filter: NumericRangeFilter<i32> = NumericRangeFilter::new("price")
	///     .min(100);
	/// ```
	pub fn min(mut self, value: T) -> Self {
		self.range = self.range.gte(value);
		self
	}

	/// Set the maximum value (inclusive)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::NumericRangeFilter;
	///
	/// let filter: NumericRangeFilter<i32> = NumericRangeFilter::new("price")
	///     .max(500);
	/// ```
	pub fn max(mut self, value: T) -> Self {
		self.range = self.range.lte(value);
		self
	}

	/// Set a numeric range
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::NumericRangeFilter;
	///
	/// let filter: NumericRangeFilter<i32> = NumericRangeFilter::new("price")
	///     .range(100, 500);
	/// ```
	pub fn range(mut self, min: T, max: T) -> Self {
		self.range = self.range.between(min, max);
		self
	}

	/// Get the field name
	pub fn field_name(&self) -> &str {
		self.range.field_name()
	}

	/// Get the underlying range filter
	pub fn inner(&self) -> &RangeFilter<T> {
		&self.range
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_range_filter_creation() {
		let filter: RangeFilter<i32> = RangeFilter::new("age");
		assert_eq!(filter.field_name(), "age");
		assert!(!filter.has_bounds());
	}

	#[test]
	fn test_range_filter_gte() {
		let filter: RangeFilter<i32> = RangeFilter::new("price").gte(100);
		assert_eq!(filter.gte, Some(100));
		assert!(filter.has_bounds());
	}

	#[test]
	fn test_range_filter_gt() {
		let filter: RangeFilter<i32> = RangeFilter::new("price").gt(100);
		assert_eq!(filter.gt, Some(100));
		assert!(filter.has_bounds());
	}

	#[test]
	fn test_range_filter_lte() {
		let filter: RangeFilter<i32> = RangeFilter::new("price").lte(500);
		assert_eq!(filter.lte, Some(500));
		assert!(filter.has_bounds());
	}

	#[test]
	fn test_range_filter_lt() {
		let filter: RangeFilter<i32> = RangeFilter::new("price").lt(500);
		assert_eq!(filter.lt, Some(500));
		assert!(filter.has_bounds());
	}

	#[test]
	fn test_range_filter_between() {
		let filter: RangeFilter<i32> = RangeFilter::new("price").between(100, 500);
		assert_eq!(filter.gte, Some(100));
		assert_eq!(filter.lte, Some(500));
		assert!(filter.has_bounds());
	}

	#[test]
	fn test_range_filter_complex() {
		let filter: RangeFilter<i32> = RangeFilter::new("price").gt(100).lt(500);
		assert_eq!(filter.gt, Some(100));
		assert_eq!(filter.lt, Some(500));
		assert!(filter.has_bounds());
	}

	#[test]
	fn test_date_range_filter_creation() {
		let filter = DateRangeFilter::new("created_at");
		assert_eq!(filter.field_name(), "created_at");
	}

	#[test]
	fn test_date_range_filter_after() {
		let filter = DateRangeFilter::new("created_at").after("2024-01-01");
		assert_eq!(filter.inner().gte, Some("2024-01-01".to_string()));
	}

	#[test]
	fn test_date_range_filter_before() {
		let filter = DateRangeFilter::new("created_at").before("2024-12-31");
		assert_eq!(filter.inner().lte, Some("2024-12-31".to_string()));
	}

	#[test]
	fn test_date_range_filter_range() {
		let filter = DateRangeFilter::new("created_at").range("2024-01-01", "2024-12-31");
		assert_eq!(filter.inner().gte, Some("2024-01-01".to_string()));
		assert_eq!(filter.inner().lte, Some("2024-12-31".to_string()));
	}

	#[test]
	fn test_numeric_range_filter_creation() {
		let filter: NumericRangeFilter<i32> = NumericRangeFilter::new("quantity");
		assert_eq!(filter.field_name(), "quantity");
	}

	#[test]
	fn test_numeric_range_filter_min() {
		let filter: NumericRangeFilter<i32> = NumericRangeFilter::new("quantity").min(10);
		assert_eq!(filter.inner().gte, Some(10));
	}

	#[test]
	fn test_numeric_range_filter_max() {
		let filter: NumericRangeFilter<i32> = NumericRangeFilter::new("quantity").max(100);
		assert_eq!(filter.inner().lte, Some(100));
	}

	#[test]
	fn test_numeric_range_filter_range() {
		let filter: NumericRangeFilter<f64> = NumericRangeFilter::new("price").range(99.99, 999.99);
		assert_eq!(filter.inner().gte, Some(99.99));
		assert_eq!(filter.inner().lte, Some(999.99));
	}
}
