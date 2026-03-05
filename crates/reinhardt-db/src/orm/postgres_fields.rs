//! PostgreSQL-specific field types
//!
//! This module provides PostgreSQL-specific field types inspired by Django's
//! `django/contrib/postgres/fields/`.
//!
//! # Available Field Types
//!
//! - **ArrayField**: Store arrays of values
//! - **JSONBField**: Store JSON data efficiently with indexing support
//! - **HStoreField**: Store key-value pairs
//! - **RangeFields**: Integer, Date, DateTime ranges
//! - **CITextField**: Case-insensitive text field
//!
//! # Example
//!
//! ```rust
//! use reinhardt_db::orm::{ArrayField, JSONBField};
//!
//! // Array field storing tags
//! let tags_field = ArrayField::<String>::new("VARCHAR(50)");
//! assert_eq!(tags_field.base_type(), "VARCHAR(50)");
//!
//! // JSONB field for metadata
//! let metadata_field = JSONBField::new();
//! assert_eq!(metadata_field.sql_type(), "JSONB");
//! ```

use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

/// PostgreSQL Array field
///
/// Stores an array of values of a specific type.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::ArrayField;
///
/// // Array of integers
/// let scores = ArrayField::<i32>::new("INTEGER");
/// assert_eq!(scores.base_type(), "INTEGER");
///
/// // Array of strings with max length
/// let tags = ArrayField::<String>::new("VARCHAR(50)");
/// assert_eq!(tags.base_type(), "VARCHAR(50)");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrayField<T> {
	base_type: String,
	size: Option<usize>,
	default: Option<Vec<T>>,
	_phantom: PhantomData<T>,
}

impl<T> ArrayField<T> {
	/// Create a new ArrayField
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::ArrayField;
	///
	/// let field = ArrayField::<i32>::new("INTEGER");
	/// assert_eq!(field.base_type(), "INTEGER");
	/// ```
	pub fn new(base_type: impl Into<String>) -> Self {
		Self {
			base_type: base_type.into(),
			size: None,
			default: None,
			_phantom: PhantomData,
		}
	}

	/// Set a fixed size for the array
	pub fn with_size(mut self, size: usize) -> Self {
		self.size = Some(size);
		self
	}

	/// Set a default value
	pub fn with_default(mut self, default: Vec<T>) -> Self {
		self.default = Some(default);
		self
	}

	/// Get the base type
	pub fn base_type(&self) -> &str {
		&self.base_type
	}

	/// Get the size constraint if set
	pub fn size(&self) -> Option<usize> {
		self.size
	}

	/// Generate SQL type definition
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::ArrayField;
	///
	/// let field = ArrayField::<i32>::new("INTEGER");
	/// assert_eq!(field.sql_type(), "INTEGER[]");
	///
	/// let sized_field = ArrayField::<i32>::new("INTEGER").with_size(10);
	/// assert_eq!(sized_field.sql_type(), "INTEGER[10]");
	/// ```
	pub fn sql_type(&self) -> String {
		if let Some(size) = self.size {
			format!("{}[{}]", self.base_type, size)
		} else {
			format!("{}[]", self.base_type)
		}
	}
}

/// PostgreSQL JSONB field
///
/// Stores JSON data in binary format with indexing support.
/// More efficient than JSON type for querying.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::JSONBField;
///
/// let metadata = JSONBField::new();
/// assert_eq!(metadata.sql_type(), "JSONB");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JSONBField {
	default: Option<serde_json::Value>,
}

impl JSONBField {
	/// Create a new JSONBField
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::JSONBField;
	///
	/// let field = JSONBField::new();
	/// assert_eq!(field.sql_type(), "JSONB");
	/// ```
	pub fn new() -> Self {
		Self { default: None }
	}

	/// Set a default JSON value
	pub fn with_default(mut self, default: serde_json::Value) -> Self {
		self.default = Some(default);
		self
	}

	/// Generate SQL type definition
	pub fn sql_type(&self) -> &'static str {
		"JSONB"
	}

	/// Get default value
	pub fn default(&self) -> Option<&serde_json::Value> {
		self.default.as_ref()
	}
}

impl Default for JSONBField {
	fn default() -> Self {
		Self::new()
	}
}

/// PostgreSQL HStore field
///
/// Stores key-value pairs. Requires the `hstore` extension.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::HStoreField;
///
/// let attributes = HStoreField::new();
/// assert_eq!(attributes.sql_type(), "HSTORE");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HStoreField {
	default: Option<std::collections::HashMap<String, String>>,
}

impl HStoreField {
	/// Create a new HStoreField
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_db::orm::HStoreField;
	///
	/// let field = HStoreField::new();
	/// assert_eq!(field.sql_type(), "HSTORE");
	/// ```
	pub fn new() -> Self {
		Self { default: None }
	}

	/// Set a default value
	pub fn with_default(mut self, default: std::collections::HashMap<String, String>) -> Self {
		self.default = Some(default);
		self
	}

	/// Generate SQL type definition
	pub fn sql_type(&self) -> &'static str {
		"HSTORE"
	}
}

impl Default for HStoreField {
	fn default() -> Self {
		Self::new()
	}
}

/// PostgreSQL Integer Range field
///
/// Stores a range of integers (INT4RANGE).
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::IntegerRangeField;
///
/// let age_range = IntegerRangeField::new();
/// assert_eq!(age_range.sql_type(), "INT4RANGE");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegerRangeField {
	default: Option<(Option<i32>, Option<i32>)>,
}

impl IntegerRangeField {
	/// Create a new IntegerRangeField
	pub fn new() -> Self {
		Self { default: None }
	}

	/// Set default range (lower, upper)
	pub fn with_default(mut self, lower: Option<i32>, upper: Option<i32>) -> Self {
		self.default = Some((lower, upper));
		self
	}

	/// Generate SQL type definition
	pub fn sql_type(&self) -> &'static str {
		"INT4RANGE"
	}
}

impl Default for IntegerRangeField {
	fn default() -> Self {
		Self::new()
	}
}

/// PostgreSQL BigInteger Range field
///
/// Stores a range of big integers (INT8RANGE).
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::BigIntegerRangeField;
///
/// let range = BigIntegerRangeField::new();
/// assert_eq!(range.sql_type(), "INT8RANGE");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BigIntegerRangeField {
	default: Option<(Option<i64>, Option<i64>)>,
}

impl BigIntegerRangeField {
	pub fn new() -> Self {
		Self { default: None }
	}

	pub fn with_default(mut self, lower: Option<i64>, upper: Option<i64>) -> Self {
		self.default = Some((lower, upper));
		self
	}

	pub fn sql_type(&self) -> &'static str {
		"INT8RANGE"
	}
}

impl Default for BigIntegerRangeField {
	fn default() -> Self {
		Self::new()
	}
}

/// PostgreSQL Date Range field
///
/// Stores a range of dates (DATERANGE).
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::DateRangeField;
///
/// let period = DateRangeField::new();
/// assert_eq!(period.sql_type(), "DATERANGE");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRangeField {
	default: Option<(Option<NaiveDate>, Option<NaiveDate>)>,
}

impl DateRangeField {
	pub fn new() -> Self {
		Self { default: None }
	}

	pub fn with_default(mut self, lower: Option<NaiveDate>, upper: Option<NaiveDate>) -> Self {
		self.default = Some((lower, upper));
		self
	}

	pub fn sql_type(&self) -> &'static str {
		"DATERANGE"
	}
}

impl Default for DateRangeField {
	fn default() -> Self {
		Self::new()
	}
}

/// PostgreSQL DateTime Range field
///
/// Stores a range of timestamps (TSTZRANGE).
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::DateTimeRangeField;
///
/// let period = DateTimeRangeField::new();
/// assert_eq!(period.sql_type(), "TSTZRANGE");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateTimeRangeField {
	default: Option<(Option<NaiveDateTime>, Option<NaiveDateTime>)>,
}

impl DateTimeRangeField {
	pub fn new() -> Self {
		Self { default: None }
	}

	pub fn with_default(
		mut self,
		lower: Option<NaiveDateTime>,
		upper: Option<NaiveDateTime>,
	) -> Self {
		self.default = Some((lower, upper));
		self
	}

	pub fn sql_type(&self) -> &'static str {
		"TSTZRANGE"
	}
}

impl Default for DateTimeRangeField {
	fn default() -> Self {
		Self::new()
	}
}

/// Case-insensitive Text field
///
/// Uses PostgreSQL's CITEXT extension for case-insensitive text comparison.
///
/// # Example
///
/// ```rust
/// use reinhardt_db::orm::CITextField;
///
/// let email = CITextField::new();
/// assert_eq!(email.sql_type(), "CITEXT");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CITextField {
	max_length: Option<usize>,
	default: Option<String>,
}

impl CITextField {
	/// Create a new CITextField
	pub fn new() -> Self {
		Self {
			max_length: None,
			default: None,
		}
	}

	/// Set maximum length
	pub fn with_max_length(mut self, max_length: usize) -> Self {
		self.max_length = Some(max_length);
		self
	}

	/// Set default value
	pub fn with_default(mut self, default: impl Into<String>) -> Self {
		self.default = Some(default.into());
		self
	}

	/// Generate SQL type definition
	pub fn sql_type(&self) -> &'static str {
		"CITEXT"
	}
}

impl Default for CITextField {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_array_field_basic() {
		let field = ArrayField::<i32>::new("INTEGER");
		assert_eq!(field.base_type(), "INTEGER");
		assert_eq!(field.sql_type(), "INTEGER[]");
	}

	#[test]
	fn test_array_field_with_size() {
		let field = ArrayField::<String>::new("VARCHAR(50)").with_size(10);
		assert_eq!(field.size(), Some(10));
		assert_eq!(field.sql_type(), "VARCHAR(50)[10]");
	}

	#[test]
	fn test_jsonb_field() {
		let field = JSONBField::new();
		assert_eq!(field.sql_type(), "JSONB");
	}

	#[test]
	fn test_jsonb_field_with_default() {
		let default = serde_json::json!({"key": "value"});
		let field = JSONBField::new().with_default(default.clone());
		assert_eq!(field.default(), Some(&default));
	}

	#[test]
	fn test_hstore_field() {
		let field = HStoreField::new();
		assert_eq!(field.sql_type(), "HSTORE");
	}

	#[test]
	fn test_integer_range_field() {
		let field = IntegerRangeField::new();
		assert_eq!(field.sql_type(), "INT4RANGE");
	}

	#[test]
	fn test_biginteger_range_field() {
		let field = BigIntegerRangeField::new();
		assert_eq!(field.sql_type(), "INT8RANGE");
	}

	#[test]
	fn test_date_range_field() {
		let field = DateRangeField::new();
		assert_eq!(field.sql_type(), "DATERANGE");
	}

	#[test]
	fn test_datetime_range_field() {
		let field = DateTimeRangeField::new();
		assert_eq!(field.sql_type(), "TSTZRANGE");
	}

	#[test]
	fn test_citext_field() {
		let field = CITextField::new();
		assert_eq!(field.sql_type(), "CITEXT");
	}

	#[test]
	fn test_citext_field_with_max_length() {
		let field = CITextField::new().with_max_length(255);
		assert_eq!(field.sql_type(), "CITEXT");
	}
}
