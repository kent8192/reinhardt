//! Type traits for field lookups
//!
//! These traits are used to constrain which methods are available
//! for different field types.

use super::lookup::LookupValue;
use serde::{Deserialize, Serialize};

/// Marker trait for types that can be compared (=, !=, <, >, <=, >=)
pub trait Comparable:
	Clone + Serialize + for<'de> Deserialize<'de> + Send + Sync + Into<LookupValue>
{
}

/// Marker trait for string types (String, &str)
pub trait StringType: Comparable {}

/// Marker trait for numeric types (i32, i64, f32, f64)
pub trait NumericType: Comparable {}

/// Marker trait for date/time types
pub trait DateTimeType: Comparable {}

// Implement Comparable for common types
impl Comparable for String {}
impl Comparable for i32 {}
impl Comparable for i64 {}
impl Comparable for f32 {}
impl Comparable for f64 {}
impl Comparable for bool {}

// Implement StringType
impl StringType for String {}

// Implement NumericType
impl NumericType for i32 {}
impl NumericType for i64 {}
impl NumericType for f32 {}
impl NumericType for f64 {}

// DateTime type (legacy - prefer chrono types)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
/// Represents a date time.
pub struct DateTime {
	/// The timestamp.
	pub timestamp: i64,
}

impl Comparable for DateTime {}
impl DateTimeType for DateTime {}

// Date type (legacy - prefer chrono types)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
/// Represents a date.
pub struct Date {
	/// The year.
	pub year: i32,
	/// The month.
	pub month: u8,
	/// The day.
	pub day: u8,
}

impl Comparable for Date {}

// chrono integration
//
// These implementations allow chrono types to be used directly in QuerySet
// filters and lookups. All chrono datetime types are converted to Unix
// timestamps for comparison operations.

impl Comparable for chrono::NaiveDateTime {}
impl DateTimeType for chrono::NaiveDateTime {}

impl Comparable for chrono::NaiveDate {}
impl DateTimeType for chrono::NaiveDate {}

impl Comparable for chrono::NaiveTime {}

impl Comparable for chrono::DateTime<chrono::Utc> {}
impl DateTimeType for chrono::DateTime<chrono::Utc> {}

impl Comparable for chrono::DateTime<chrono::FixedOffset> {}
impl DateTimeType for chrono::DateTime<chrono::FixedOffset> {}

impl Comparable for chrono::DateTime<chrono::Local> {}
impl DateTimeType for chrono::DateTime<chrono::Local> {}
