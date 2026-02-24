//! IntoValue trait and implementations.

use super::Value;

/// Trait for converting Rust types to SQL values.
///
/// This trait provides a uniform way to convert various Rust types
/// into the [`Value`] enum used by the query builder.
///
/// # Example
///
/// ```rust
/// use reinhardt_query::{IntoValue, Value};
///
/// let int_value: Value = 42i32.into_value();
/// let str_value: Value = "hello".into_value();
/// let null_value: Value = Option::<i32>::None.into_value();
/// ```
pub trait IntoValue {
	/// Converts this value into a [`Value`].
	fn into_value(self) -> Value;
}

// =============================================================================
// Implementations for primitive types
// =============================================================================

impl IntoValue for bool {
	fn into_value(self) -> Value {
		Value::Bool(Some(self))
	}
}

impl IntoValue for Option<bool> {
	fn into_value(self) -> Value {
		Value::Bool(self)
	}
}

impl IntoValue for i8 {
	fn into_value(self) -> Value {
		Value::TinyInt(Some(self))
	}
}

impl IntoValue for Option<i8> {
	fn into_value(self) -> Value {
		Value::TinyInt(self)
	}
}

impl IntoValue for i16 {
	fn into_value(self) -> Value {
		Value::SmallInt(Some(self))
	}
}

impl IntoValue for Option<i16> {
	fn into_value(self) -> Value {
		Value::SmallInt(self)
	}
}

impl IntoValue for i32 {
	fn into_value(self) -> Value {
		Value::Int(Some(self))
	}
}

impl IntoValue for Option<i32> {
	fn into_value(self) -> Value {
		Value::Int(self)
	}
}

impl IntoValue for i64 {
	fn into_value(self) -> Value {
		Value::BigInt(Some(self))
	}
}

impl IntoValue for Option<i64> {
	fn into_value(self) -> Value {
		Value::BigInt(self)
	}
}

impl IntoValue for u8 {
	fn into_value(self) -> Value {
		Value::TinyUnsigned(Some(self))
	}
}

impl IntoValue for Option<u8> {
	fn into_value(self) -> Value {
		Value::TinyUnsigned(self)
	}
}

impl IntoValue for u16 {
	fn into_value(self) -> Value {
		Value::SmallUnsigned(Some(self))
	}
}

impl IntoValue for Option<u16> {
	fn into_value(self) -> Value {
		Value::SmallUnsigned(self)
	}
}

impl IntoValue for u32 {
	fn into_value(self) -> Value {
		Value::Unsigned(Some(self))
	}
}

impl IntoValue for Option<u32> {
	fn into_value(self) -> Value {
		Value::Unsigned(self)
	}
}

impl IntoValue for u64 {
	fn into_value(self) -> Value {
		Value::BigUnsigned(Some(self))
	}
}

impl IntoValue for Option<u64> {
	fn into_value(self) -> Value {
		Value::BigUnsigned(self)
	}
}

impl IntoValue for f32 {
	fn into_value(self) -> Value {
		Value::Float(Some(self))
	}
}

impl IntoValue for Option<f32> {
	fn into_value(self) -> Value {
		Value::Float(self)
	}
}

impl IntoValue for f64 {
	fn into_value(self) -> Value {
		Value::Double(Some(self))
	}
}

impl IntoValue for Option<f64> {
	fn into_value(self) -> Value {
		Value::Double(self)
	}
}

impl IntoValue for char {
	fn into_value(self) -> Value {
		Value::Char(Some(self))
	}
}

impl IntoValue for Option<char> {
	fn into_value(self) -> Value {
		Value::Char(self)
	}
}

// =============================================================================
// Implementations for String types
// =============================================================================

impl IntoValue for String {
	fn into_value(self) -> Value {
		Value::String(Some(Box::new(self)))
	}
}

impl IntoValue for Option<String> {
	fn into_value(self) -> Value {
		Value::String(self.map(Box::new))
	}
}

impl IntoValue for &str {
	fn into_value(self) -> Value {
		Value::String(Some(Box::new(self.to_owned())))
	}
}

impl IntoValue for Option<&str> {
	fn into_value(self) -> Value {
		Value::String(self.map(|s| Box::new(s.to_owned())))
	}
}

impl IntoValue for &String {
	fn into_value(self) -> Value {
		Value::String(Some(Box::new(self.clone())))
	}
}

impl IntoValue for Box<String> {
	fn into_value(self) -> Value {
		Value::String(Some(self))
	}
}

impl IntoValue for Option<Box<String>> {
	fn into_value(self) -> Value {
		Value::String(self)
	}
}

impl IntoValue for std::borrow::Cow<'_, str> {
	fn into_value(self) -> Value {
		Value::String(Some(Box::new(self.into_owned())))
	}
}

// =============================================================================
// Implementations for Bytes types
// =============================================================================

impl IntoValue for Vec<u8> {
	fn into_value(self) -> Value {
		Value::Bytes(Some(Box::new(self)))
	}
}

impl IntoValue for Option<Vec<u8>> {
	fn into_value(self) -> Value {
		Value::Bytes(self.map(Box::new))
	}
}

impl IntoValue for &[u8] {
	fn into_value(self) -> Value {
		Value::Bytes(Some(Box::new(self.to_vec())))
	}
}

// =============================================================================
// Implementation for Value itself (identity)
// =============================================================================

impl IntoValue for Value {
	fn into_value(self) -> Value {
		self
	}
}

// =============================================================================
// Into<Value> implementations (delegate to IntoValue)
// =============================================================================

impl From<bool> for Value {
	fn from(v: bool) -> Self {
		v.into_value()
	}
}

impl From<i8> for Value {
	fn from(v: i8) -> Self {
		v.into_value()
	}
}

impl From<i16> for Value {
	fn from(v: i16) -> Self {
		v.into_value()
	}
}

impl From<i32> for Value {
	fn from(v: i32) -> Self {
		v.into_value()
	}
}

impl From<i64> for Value {
	fn from(v: i64) -> Self {
		v.into_value()
	}
}

impl From<u8> for Value {
	fn from(v: u8) -> Self {
		v.into_value()
	}
}

impl From<u16> for Value {
	fn from(v: u16) -> Self {
		v.into_value()
	}
}

impl From<u32> for Value {
	fn from(v: u32) -> Self {
		v.into_value()
	}
}

impl From<u64> for Value {
	fn from(v: u64) -> Self {
		v.into_value()
	}
}

impl From<f32> for Value {
	fn from(v: f32) -> Self {
		v.into_value()
	}
}

impl From<f64> for Value {
	fn from(v: f64) -> Self {
		v.into_value()
	}
}

impl From<char> for Value {
	fn from(v: char) -> Self {
		v.into_value()
	}
}

impl From<String> for Value {
	fn from(v: String) -> Self {
		v.into_value()
	}
}

impl From<&str> for Value {
	fn from(v: &str) -> Self {
		v.into_value()
	}
}

impl From<Vec<u8>> for Value {
	fn from(v: Vec<u8>) -> Self {
		v.into_value()
	}
}

impl From<&[u8]> for Value {
	fn from(v: &[u8]) -> Self {
		v.into_value()
	}
}

// =============================================================================
// Feature-gated implementations: chrono
// =============================================================================

#[cfg(feature = "with-chrono")]
impl IntoValue for chrono::NaiveDate {
	fn into_value(self) -> Value {
		Value::ChronoDate(Some(Box::new(self)))
	}
}

#[cfg(feature = "with-chrono")]
impl IntoValue for Option<chrono::NaiveDate> {
	fn into_value(self) -> Value {
		Value::ChronoDate(self.map(Box::new))
	}
}

#[cfg(feature = "with-chrono")]
impl IntoValue for chrono::NaiveTime {
	fn into_value(self) -> Value {
		Value::ChronoTime(Some(Box::new(self)))
	}
}

#[cfg(feature = "with-chrono")]
impl IntoValue for Option<chrono::NaiveTime> {
	fn into_value(self) -> Value {
		Value::ChronoTime(self.map(Box::new))
	}
}

#[cfg(feature = "with-chrono")]
impl IntoValue for chrono::NaiveDateTime {
	fn into_value(self) -> Value {
		Value::ChronoDateTime(Some(Box::new(self)))
	}
}

#[cfg(feature = "with-chrono")]
impl IntoValue for Option<chrono::NaiveDateTime> {
	fn into_value(self) -> Value {
		Value::ChronoDateTime(self.map(Box::new))
	}
}

#[cfg(feature = "with-chrono")]
impl IntoValue for chrono::DateTime<chrono::Utc> {
	fn into_value(self) -> Value {
		Value::ChronoDateTimeUtc(Some(Box::new(self)))
	}
}

#[cfg(feature = "with-chrono")]
impl IntoValue for Option<chrono::DateTime<chrono::Utc>> {
	fn into_value(self) -> Value {
		Value::ChronoDateTimeUtc(self.map(Box::new))
	}
}

#[cfg(feature = "with-chrono")]
impl IntoValue for chrono::DateTime<chrono::Local> {
	fn into_value(self) -> Value {
		Value::ChronoDateTimeLocal(Some(Box::new(self)))
	}
}

#[cfg(feature = "with-chrono")]
impl IntoValue for Option<chrono::DateTime<chrono::Local>> {
	fn into_value(self) -> Value {
		Value::ChronoDateTimeLocal(self.map(Box::new))
	}
}

#[cfg(feature = "with-chrono")]
impl IntoValue for chrono::DateTime<chrono::FixedOffset> {
	fn into_value(self) -> Value {
		Value::ChronoDateTimeWithTimeZone(Some(Box::new(self)))
	}
}

#[cfg(feature = "with-chrono")]
impl IntoValue for Option<chrono::DateTime<chrono::FixedOffset>> {
	fn into_value(self) -> Value {
		Value::ChronoDateTimeWithTimeZone(self.map(Box::new))
	}
}

#[cfg(feature = "with-chrono")]
impl From<chrono::NaiveDate> for Value {
	fn from(v: chrono::NaiveDate) -> Self {
		v.into_value()
	}
}

#[cfg(feature = "with-chrono")]
impl From<chrono::NaiveTime> for Value {
	fn from(v: chrono::NaiveTime) -> Self {
		v.into_value()
	}
}

#[cfg(feature = "with-chrono")]
impl From<chrono::NaiveDateTime> for Value {
	fn from(v: chrono::NaiveDateTime) -> Self {
		v.into_value()
	}
}

#[cfg(feature = "with-chrono")]
impl From<chrono::DateTime<chrono::Utc>> for Value {
	fn from(v: chrono::DateTime<chrono::Utc>) -> Self {
		v.into_value()
	}
}

#[cfg(feature = "with-chrono")]
impl From<chrono::DateTime<chrono::Local>> for Value {
	fn from(v: chrono::DateTime<chrono::Local>) -> Self {
		v.into_value()
	}
}

#[cfg(feature = "with-chrono")]
impl From<chrono::DateTime<chrono::FixedOffset>> for Value {
	fn from(v: chrono::DateTime<chrono::FixedOffset>) -> Self {
		v.into_value()
	}
}

// =============================================================================
// Feature-gated implementations: uuid
// =============================================================================

#[cfg(feature = "with-uuid")]
impl IntoValue for uuid::Uuid {
	fn into_value(self) -> Value {
		Value::Uuid(Some(Box::new(self)))
	}
}

#[cfg(feature = "with-uuid")]
impl IntoValue for Option<uuid::Uuid> {
	fn into_value(self) -> Value {
		Value::Uuid(self.map(Box::new))
	}
}

#[cfg(feature = "with-uuid")]
impl From<uuid::Uuid> for Value {
	fn from(v: uuid::Uuid) -> Self {
		v.into_value()
	}
}

// =============================================================================
// Feature-gated implementations: json
// =============================================================================

#[cfg(feature = "with-json")]
impl IntoValue for serde_json::Value {
	fn into_value(self) -> Value {
		Value::Json(Some(Box::new(self)))
	}
}

#[cfg(feature = "with-json")]
impl IntoValue for Option<serde_json::Value> {
	fn into_value(self) -> Value {
		Value::Json(self.map(Box::new))
	}
}

#[cfg(feature = "with-json")]
impl From<serde_json::Value> for Value {
	fn from(v: serde_json::Value) -> Self {
		v.into_value()
	}
}

// =============================================================================
// Feature-gated implementations: rust_decimal
// =============================================================================

#[cfg(feature = "with-rust_decimal")]
impl IntoValue for rust_decimal::Decimal {
	fn into_value(self) -> Value {
		Value::Decimal(Some(Box::new(self)))
	}
}

#[cfg(feature = "with-rust_decimal")]
impl IntoValue for Option<rust_decimal::Decimal> {
	fn into_value(self) -> Value {
		Value::Decimal(self.map(Box::new))
	}
}

#[cfg(feature = "with-rust_decimal")]
impl From<rust_decimal::Decimal> for Value {
	fn from(v: rust_decimal::Decimal) -> Self {
		v.into_value()
	}
}

// =============================================================================
// Feature-gated implementations: bigdecimal
// =============================================================================

#[cfg(feature = "with-bigdecimal")]
impl IntoValue for bigdecimal::BigDecimal {
	fn into_value(self) -> Value {
		Value::BigDecimal(Some(Box::new(self)))
	}
}

#[cfg(feature = "with-bigdecimal")]
impl IntoValue for Option<bigdecimal::BigDecimal> {
	fn into_value(self) -> Value {
		Value::BigDecimal(self.map(Box::new))
	}
}

#[cfg(feature = "with-bigdecimal")]
impl From<bigdecimal::BigDecimal> for Value {
	fn from(v: bigdecimal::BigDecimal) -> Self {
		v.into_value()
	}
}
