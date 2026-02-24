//! Core Value enum definition.

use super::ArrayType;

/// Core value representation for SQL parameters.
///
/// This enum represents all possible SQL value types. The enum is designed
/// to be size-optimized: larger types are boxed to maintain a consistent
/// enum size of approximately one pointer width.
///
/// ## Null Values
///
/// All variants use `Option<T>` to represent nullable values. A `None` value
/// will be rendered as SQL `NULL`.
///
/// ## Variant Naming Convention
///
/// Variant names align with common SQL type names:
///
/// | Variant | SQL Type | Rust Type |
/// |---------|----------|-----------|
/// | `Value::Bool` | BOOLEAN | `bool` |
/// | `Value::TinyInt` | TINYINT | `i8` |
/// | `Value::SmallInt` | SMALLINT | `i16` |
/// | `Value::Int` | INTEGER | `i32` |
/// | `Value::BigInt` | BIGINT | `i64` |
/// | `Value::TinyUnsigned` | TINYINT UNSIGNED | `u8` |
/// | `Value::SmallUnsigned` | SMALLINT UNSIGNED | `u16` |
/// | `Value::Unsigned` | INTEGER UNSIGNED | `u32` |
/// | `Value::BigUnsigned` | BIGINT UNSIGNED | `u64` |
/// | `Value::Float` | FLOAT | `f32` |
/// | `Value::Double` | DOUBLE | `f64` |
/// | `Value::Char` | CHAR | `char` |
/// | `Value::String` | VARCHAR/TEXT | `String` |
/// | `Value::Bytes` | BLOB/BINARY | `Vec<u8>` |
///
/// ## Example
///
/// ```rust
/// use reinhardt_query::Value;
///
/// // Integer types
/// let int_val = Value::Int(Some(42));
/// let null_int = Value::Int(None);
/// let bigint_val = Value::BigInt(Some(9223372036854775807i64));
///
/// // String type (boxed for size optimization)
/// let string_val = Value::String(Some(Box::new("hello".to_string())));
///
/// // Check for null
/// assert!(!int_val.is_null());
/// assert!(null_int.is_null());
///
/// // Convert to SQL literal
/// assert_eq!(int_val.to_sql_literal(), "42");
/// assert_eq!(null_int.to_sql_literal(), "NULL");
/// ```
///
/// ## Feature-Gated Types
///
/// Additional types are available with feature flags:
///
/// - `with-chrono`: Date/time types (`Value::ChronoDate`, `Value::ChronoTime`, etc.)
/// - `with-uuid`: UUID type (`Value::Uuid`)
/// - `with-json`: JSON type (`Value::Json`)
/// - `with-rust_decimal`: Decimal type (`Value::Decimal`)
/// - `with-bigdecimal`: BigDecimal type (`Value::BigDecimal`)
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
	// -------------------------------------------------------------------------
	// Primitive types (inline, not boxed)
	// -------------------------------------------------------------------------
	/// Boolean value
	Bool(Option<bool>),
	/// 8-bit signed integer
	TinyInt(Option<i8>),
	/// 16-bit signed integer
	SmallInt(Option<i16>),
	/// 32-bit signed integer
	Int(Option<i32>),
	/// 64-bit signed integer
	BigInt(Option<i64>),
	/// 8-bit unsigned integer
	TinyUnsigned(Option<u8>),
	/// 16-bit unsigned integer
	SmallUnsigned(Option<u16>),
	/// 32-bit unsigned integer
	Unsigned(Option<u32>),
	/// 64-bit unsigned integer
	BigUnsigned(Option<u64>),
	/// 32-bit floating point
	Float(Option<f32>),
	/// 64-bit floating point
	Double(Option<f64>),
	/// Single character
	Char(Option<char>),

	// -------------------------------------------------------------------------
	// Heap-allocated types (boxed for size optimization)
	// -------------------------------------------------------------------------
	/// String value (boxed)
	String(Option<Box<String>>),
	/// Binary data (boxed)
	Bytes(Option<Box<Vec<u8>>>),

	// -------------------------------------------------------------------------
	// Feature-gated types: chrono
	// -------------------------------------------------------------------------
	/// Chrono NaiveDate
	#[cfg(feature = "with-chrono")]
	ChronoDate(Option<Box<chrono::NaiveDate>>),
	/// Chrono NaiveTime
	#[cfg(feature = "with-chrono")]
	ChronoTime(Option<Box<chrono::NaiveTime>>),
	/// Chrono NaiveDateTime
	#[cfg(feature = "with-chrono")]
	ChronoDateTime(Option<Box<chrono::NaiveDateTime>>),
	/// Chrono DateTime with UTC timezone
	#[cfg(feature = "with-chrono")]
	ChronoDateTimeUtc(Option<Box<chrono::DateTime<chrono::Utc>>>),
	/// Chrono DateTime with Local timezone
	#[cfg(feature = "with-chrono")]
	ChronoDateTimeLocal(Option<Box<chrono::DateTime<chrono::Local>>>),
	/// Chrono DateTime with fixed offset timezone
	#[cfg(feature = "with-chrono")]
	ChronoDateTimeWithTimeZone(Option<Box<chrono::DateTime<chrono::FixedOffset>>>),

	// -------------------------------------------------------------------------
	// Feature-gated types: uuid
	// -------------------------------------------------------------------------
	/// UUID value
	#[cfg(feature = "with-uuid")]
	Uuid(Option<Box<uuid::Uuid>>),

	// -------------------------------------------------------------------------
	// Feature-gated types: json
	// -------------------------------------------------------------------------
	/// JSON value
	#[cfg(feature = "with-json")]
	Json(Option<Box<serde_json::Value>>),

	// -------------------------------------------------------------------------
	// Feature-gated types: decimal
	// -------------------------------------------------------------------------
	/// Rust Decimal value
	#[cfg(feature = "with-rust_decimal")]
	Decimal(Option<Box<rust_decimal::Decimal>>),

	/// BigDecimal value
	#[cfg(feature = "with-bigdecimal")]
	BigDecimal(Option<Box<bigdecimal::BigDecimal>>),

	// -------------------------------------------------------------------------
	// Array type
	// -------------------------------------------------------------------------
	/// Array of values with element type information
	Array(ArrayType, Option<Box<Vec<Value>>>),
}

impl Value {
	/// Returns `true` if this value is null.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::Value;
	///
	/// assert!(Value::Int(None).is_null());
	/// assert!(!Value::Int(Some(42)).is_null());
	/// ```
	#[must_use]
	pub fn is_null(&self) -> bool {
		match self {
			Self::Bool(v) => v.is_none(),
			Self::TinyInt(v) => v.is_none(),
			Self::SmallInt(v) => v.is_none(),
			Self::Int(v) => v.is_none(),
			Self::BigInt(v) => v.is_none(),
			Self::TinyUnsigned(v) => v.is_none(),
			Self::SmallUnsigned(v) => v.is_none(),
			Self::Unsigned(v) => v.is_none(),
			Self::BigUnsigned(v) => v.is_none(),
			Self::Float(v) => v.is_none(),
			Self::Double(v) => v.is_none(),
			Self::Char(v) => v.is_none(),
			Self::String(v) => v.is_none(),
			Self::Bytes(v) => v.is_none(),
			#[cfg(feature = "with-chrono")]
			Self::ChronoDate(v) => v.is_none(),
			#[cfg(feature = "with-chrono")]
			Self::ChronoTime(v) => v.is_none(),
			#[cfg(feature = "with-chrono")]
			Self::ChronoDateTime(v) => v.is_none(),
			#[cfg(feature = "with-chrono")]
			Self::ChronoDateTimeUtc(v) => v.is_none(),
			#[cfg(feature = "with-chrono")]
			Self::ChronoDateTimeLocal(v) => v.is_none(),
			#[cfg(feature = "with-chrono")]
			Self::ChronoDateTimeWithTimeZone(v) => v.is_none(),
			#[cfg(feature = "with-uuid")]
			Self::Uuid(v) => v.is_none(),
			#[cfg(feature = "with-json")]
			Self::Json(v) => v.is_none(),
			#[cfg(feature = "with-rust_decimal")]
			Self::Decimal(v) => v.is_none(),
			#[cfg(feature = "with-bigdecimal")]
			Self::BigDecimal(v) => v.is_none(),
			Self::Array(_, v) => v.is_none(),
		}
	}
}

impl Value {
	/// Convert this value to a SQL literal string suitable for inlining
	/// into a SQL statement.
	///
	/// This is used by `QueryStatementBuilder::to_string()` to produce
	/// SQL with values inlined (for debugging and non-parameterized use).
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::Value;
	///
	/// assert_eq!(Value::Int(Some(42)).to_sql_literal(), "42");
	/// assert_eq!(Value::Int(None).to_sql_literal(), "NULL");
	/// assert_eq!(
	///     Value::String(Some(Box::new("hello".to_string()))).to_sql_literal(),
	///     "'hello'"
	/// );
	/// assert_eq!(
	///     Value::String(Some(Box::new("it's".to_string()))).to_sql_literal(),
	///     "'it''s'"
	/// );
	/// ```
	#[must_use]
	pub fn to_sql_literal(&self) -> String {
		match self {
			Self::Bool(Some(v)) => {
				if *v {
					"TRUE".to_string()
				} else {
					"FALSE".to_string()
				}
			}
			Self::Bool(None) => "NULL".to_string(),
			Self::TinyInt(Some(v)) => v.to_string(),
			Self::TinyInt(None) => "NULL".to_string(),
			Self::SmallInt(Some(v)) => v.to_string(),
			Self::SmallInt(None) => "NULL".to_string(),
			Self::Int(Some(v)) => v.to_string(),
			Self::Int(None) => "NULL".to_string(),
			Self::BigInt(Some(v)) => v.to_string(),
			Self::BigInt(None) => "NULL".to_string(),
			Self::TinyUnsigned(Some(v)) => v.to_string(),
			Self::TinyUnsigned(None) => "NULL".to_string(),
			Self::SmallUnsigned(Some(v)) => v.to_string(),
			Self::SmallUnsigned(None) => "NULL".to_string(),
			Self::Unsigned(Some(v)) => v.to_string(),
			Self::Unsigned(None) => "NULL".to_string(),
			Self::BigUnsigned(Some(v)) => v.to_string(),
			Self::BigUnsigned(None) => "NULL".to_string(),
			Self::Float(Some(v)) => v.to_string(),
			Self::Float(None) => "NULL".to_string(),
			Self::Double(Some(v)) => v.to_string(),
			Self::Double(None) => "NULL".to_string(),
			Self::Char(Some(v)) => {
				// Escape single quotes by doubling them
				if *v == '\'' {
					"''''".to_string()
				} else {
					format!("'{}'", v)
				}
			}
			Self::Char(None) => "NULL".to_string(),
			Self::String(Some(v)) => {
				// Escape single quotes by doubling them
				format!("'{}'", v.replace('\'', "''"))
			}
			Self::String(None) => "NULL".to_string(),
			Self::Bytes(Some(v)) => {
				// Render as hex-encoded string with X prefix
				let hex: String = v.iter().map(|b| format!("{:02X}", b)).collect();
				format!("X'{}'", hex)
			}
			Self::Bytes(None) => "NULL".to_string(),
			#[cfg(feature = "with-chrono")]
			Self::ChronoDate(Some(v)) => format!("'{}'", v),
			#[cfg(feature = "with-chrono")]
			Self::ChronoDate(None) => "NULL".to_string(),
			#[cfg(feature = "with-chrono")]
			Self::ChronoTime(Some(v)) => format!("'{}'", v),
			#[cfg(feature = "with-chrono")]
			Self::ChronoTime(None) => "NULL".to_string(),
			#[cfg(feature = "with-chrono")]
			Self::ChronoDateTime(Some(v)) => format!("'{}'", v),
			#[cfg(feature = "with-chrono")]
			Self::ChronoDateTime(None) => "NULL".to_string(),
			#[cfg(feature = "with-chrono")]
			Self::ChronoDateTimeUtc(Some(v)) => format!("'{}'", v.to_rfc3339()),
			#[cfg(feature = "with-chrono")]
			Self::ChronoDateTimeUtc(None) => "NULL".to_string(),
			#[cfg(feature = "with-chrono")]
			Self::ChronoDateTimeLocal(Some(v)) => format!("'{}'", v.to_rfc3339()),
			#[cfg(feature = "with-chrono")]
			Self::ChronoDateTimeLocal(None) => "NULL".to_string(),
			#[cfg(feature = "with-chrono")]
			Self::ChronoDateTimeWithTimeZone(Some(v)) => format!("'{}'", v.to_rfc3339()),
			#[cfg(feature = "with-chrono")]
			Self::ChronoDateTimeWithTimeZone(None) => "NULL".to_string(),
			#[cfg(feature = "with-uuid")]
			Self::Uuid(Some(v)) => format!("'{}'", v),
			#[cfg(feature = "with-uuid")]
			Self::Uuid(None) => "NULL".to_string(),
			#[cfg(feature = "with-json")]
			Self::Json(Some(v)) => {
				let json_str = serde_json::to_string(v.as_ref()).unwrap_or_default();
				format!("'{}'", json_str.replace('\'', "''"))
			}
			#[cfg(feature = "with-json")]
			Self::Json(None) => "NULL".to_string(),
			#[cfg(feature = "with-rust_decimal")]
			Self::Decimal(Some(v)) => v.to_string(),
			#[cfg(feature = "with-rust_decimal")]
			Self::Decimal(None) => "NULL".to_string(),
			#[cfg(feature = "with-bigdecimal")]
			Self::BigDecimal(Some(v)) => v.to_string(),
			#[cfg(feature = "with-bigdecimal")]
			Self::BigDecimal(None) => "NULL".to_string(),
			Self::Array(_, Some(values)) => {
				let items: Vec<String> = values.iter().map(|v| v.to_sql_literal()).collect();
				format!("ARRAY[{}]", items.join(","))
			}
			Self::Array(_, None) => "NULL".to_string(),
		}
	}
}

impl Default for Value {
	/// Returns the default value, which is a null string.
	fn default() -> Self {
		Self::String(None)
	}
}
