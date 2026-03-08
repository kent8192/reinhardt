//! Lookup type and value definitions

use crate::orm::Model;
use chrono::Timelike;
use serde::{Deserialize, Serialize};

/// Lookup type - defines how to compare the field value
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LookupType {
	// Equality
	/// Exact variant.
	Exact,  // =
	/// IExact variant.
	IExact, // ILIKE (case-insensitive)
	/// Ne variant.
	Ne,     // !=

	// Pattern matching
	/// Contains variant.
	Contains,    // LIKE '%x%'
	/// IContains variant.
	IContains,   // ILIKE '%x%'
	/// StartsWith variant.
	StartsWith,  // LIKE 'x%'
	/// IStartsWith variant.
	IStartsWith, // ILIKE 'x%'
	/// EndsWith variant.
	EndsWith,    // LIKE '%x'
	/// IEndsWith variant.
	IEndsWith,   // ILIKE '%x'
	/// Regex variant.
	Regex,       // ~ (PostgreSQL)
	/// IRegex variant.
	IRegex,      // ~* (PostgreSQL)

	// Comparison
	/// Gt variant.
	Gt,    // >
	/// Gte variant.
	Gte,   // >=
	/// Lt variant.
	Lt,    // <
	/// Lte variant.
	Lte,   // <=
	/// Range variant.
	Range, // BETWEEN

	// Set operations
	/// In variant.
	In,    // IN
	/// NotIn variant.
	NotIn, // NOT IN

	// NULL checks
	/// IsNull variant.
	IsNull,    // IS NULL
	/// IsNotNull variant.
	IsNotNull, // IS NOT NULL
}

/// Lookup value - the value to compare against
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LookupValue {
	/// String variant.
	String(String),
	/// Int variant.
	Int(i64),
	/// Float variant.
	Float(f64),
	/// Bool variant.
	Bool(bool),
	/// Array variant.
	Array(Vec<LookupValue>),
	/// Range variant.
	Range(Box<LookupValue>, Box<LookupValue>),
	/// Null variant.
	Null,
}

impl From<String> for LookupValue {
	fn from(s: String) -> Self {
		LookupValue::String(s)
	}
}

impl From<&str> for LookupValue {
	fn from(s: &str) -> Self {
		LookupValue::String(s.to_string())
	}
}

impl From<i32> for LookupValue {
	fn from(i: i32) -> Self {
		LookupValue::Int(i as i64)
	}
}

impl From<i64> for LookupValue {
	fn from(i: i64) -> Self {
		LookupValue::Int(i)
	}
}

impl From<f32> for LookupValue {
	fn from(f: f32) -> Self {
		LookupValue::Float(f as f64)
	}
}

impl From<f64> for LookupValue {
	fn from(f: f64) -> Self {
		LookupValue::Float(f)
	}
}

impl From<bool> for LookupValue {
	fn from(b: bool) -> Self {
		LookupValue::Bool(b)
	}
}

impl From<()> for LookupValue {
	fn from(_: ()) -> Self {
		LookupValue::Null
	}
}

// DateTime and Date conversions
impl From<super::traits::DateTime> for LookupValue {
	fn from(dt: super::traits::DateTime) -> Self {
		LookupValue::Int(dt.timestamp)
	}
}

impl From<super::traits::Date> for LookupValue {
	fn from(date: super::traits::Date) -> Self {
		// Encode as days since epoch or similar
		let days = date.year * 10000 + (date.month as i32) * 100 + (date.day as i32);
		LookupValue::Int(days as i64)
	}
}

// chrono integration
impl From<chrono::NaiveDateTime> for LookupValue {
	fn from(dt: chrono::NaiveDateTime) -> Self {
		LookupValue::Int(dt.and_utc().timestamp())
	}
}

impl From<chrono::NaiveDate> for LookupValue {
	fn from(date: chrono::NaiveDate) -> Self {
		// Convert to timestamp at midnight UTC
		LookupValue::Int(
			date.and_hms_opt(0, 0, 0)
				.map(|dt| dt.and_utc().timestamp())
				.unwrap_or(0),
		)
	}
}

impl From<chrono::NaiveTime> for LookupValue {
	fn from(time: chrono::NaiveTime) -> Self {
		// Store as seconds since midnight
		LookupValue::Int(time.num_seconds_from_midnight() as i64)
	}
}

impl<Tz: chrono::TimeZone> From<chrono::DateTime<Tz>> for LookupValue {
	fn from(dt: chrono::DateTime<Tz>) -> Self {
		LookupValue::Int(dt.timestamp())
	}
}

impl<T: Into<LookupValue>> From<(T, T)> for LookupValue {
	fn from((start, end): (T, T)) -> Self {
		LookupValue::Range(Box::new(start.into()), Box::new(end.into()))
	}
}

/// A complete lookup specification ready to be compiled to SQL
///
/// # Breaking Change
///
/// The type of `field_path` has been changed from `Vec<&'static str>` to `Vec<String>`.
/// This allows support for dynamic table aliases.
#[derive(Debug, Clone)]
pub struct Lookup<M: Model> {
	pub(crate) field_path: Vec<String>,
	pub(crate) lookup_type: LookupType,
	pub(crate) value: LookupValue,
	pub(crate) _phantom: std::marker::PhantomData<M>,
}

impl<M: Model> Lookup<M> {
	/// Create a new lookup for field filtering in QuerySets
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_db::orm::query_fields::{Lookup, LookupType, LookupValue};
	/// # use reinhardt_db::orm::Model;
	/// # use serde::{Serialize, Deserialize};
	/// # #[derive(Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     fn app_label() -> &'static str { "app" }
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn primary_key_field() -> &'static str { "id" }
	/// # }
	///
	/// let lookup = Lookup::<User>::new(
	///     vec!["name".to_string()],
	///     LookupType::Exact,
	///     LookupValue::String("Alice".to_string())
	/// );
	/// // Represents: WHERE name = 'Alice'
	/// assert_eq!(lookup.field_path(), &["name".to_string()]);
	/// assert_eq!(*lookup.lookup_type(), LookupType::Exact);
	/// ```
	pub fn new(field_path: Vec<String>, lookup_type: LookupType, value: LookupValue) -> Self {
		Self {
			field_path,
			lookup_type,
			value,
			_phantom: std::marker::PhantomData,
		}
	}
	/// Get the field path
	///
	pub fn field_path(&self) -> &[String] {
		&self.field_path
	}
	/// Get the lookup type
	///
	pub fn lookup_type(&self) -> &LookupType {
		&self.lookup_type
	}
	/// Get the lookup value
	///
	pub fn value(&self) -> &LookupValue {
		&self.value
	}
}
