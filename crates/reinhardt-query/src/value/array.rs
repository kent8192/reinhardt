//! Array type definitions.

/// Represents the element type of an array value.
///
/// This enum is used to track the type of elements in a SQL array,
/// which is important for proper SQL generation in backends that
/// support typed arrays (e.g., PostgreSQL).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ArrayType {
	/// Boolean array
	Bool,
	/// 8-bit signed integer array
	TinyInt,
	/// 16-bit signed integer array
	SmallInt,
	/// 32-bit signed integer array
	Int,
	/// 64-bit signed integer array
	BigInt,
	/// 8-bit unsigned integer array
	TinyUnsigned,
	/// 16-bit unsigned integer array
	SmallUnsigned,
	/// 32-bit unsigned integer array
	Unsigned,
	/// 64-bit unsigned integer array
	BigUnsigned,
	/// 32-bit floating point array
	Float,
	/// 64-bit floating point array
	Double,
	/// String array
	String,
	/// Character array
	Char,
	/// Binary data array
	Bytes,

	/// Chrono date array
	#[cfg(feature = "with-chrono")]
	ChronoDate,
	/// Chrono time array
	#[cfg(feature = "with-chrono")]
	ChronoTime,
	/// Chrono datetime array (naive)
	#[cfg(feature = "with-chrono")]
	ChronoDateTime,
	/// Chrono datetime array (UTC)
	#[cfg(feature = "with-chrono")]
	ChronoDateTimeUtc,
	/// Chrono datetime array (Local)
	#[cfg(feature = "with-chrono")]
	ChronoDateTimeLocal,
	/// Chrono datetime array (with timezone)
	#[cfg(feature = "with-chrono")]
	ChronoDateTimeWithTimeZone,

	/// UUID array
	#[cfg(feature = "with-uuid")]
	Uuid,

	/// JSON array
	#[cfg(feature = "with-json")]
	Json,

	/// Rust Decimal array
	#[cfg(feature = "with-rust_decimal")]
	Decimal,

	/// BigDecimal array
	#[cfg(feature = "with-bigdecimal")]
	BigDecimal,
}
