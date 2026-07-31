//! [`DbValue`] — the one place a Rust type's mapping to a database column value
//! lives, in both directions.
//!
//! The ORM used to persist a model by serializing it to `serde_json::Value` and
//! then reconstructing SQL types from the JSON — which erased the Rust type (a
//! `Uuid`, a `DateTime`, and a `String` all became strings) and scattered the
//! "how do I store type X" knowledge across the write binder, the row decoder,
//! and the derive macro. `DbValue` replaces that: a type says once how it
//! becomes a [`QueryValue`] and how it comes back, and the generated model code
//! calls it per field. Adding a new field type (a `PointField`, say) is then a
//! single `impl DbValue` — no edits to the bind/decode/macro paths.

use reinhardt_core::exception::{Error, Result};

use crate::backends::types::QueryValue;

/// A Rust type that maps to one database column value.
pub trait DbValue: Sized {
	/// Convert to the value bound into the SQL statement.
	fn to_db_value(&self) -> QueryValue;
	/// Reconstruct from the value a row yields for this column.
	fn from_db_value(value: QueryValue) -> Result<Self>;
}

fn wrong_type(expected: &str, got: &QueryValue) -> Error {
	Error::Database(format!("expected {expected}, got {got:?}"))
}

macro_rules! int_impl {
	($ty:ty) => {
		impl DbValue for $ty {
			fn to_db_value(&self) -> QueryValue {
				QueryValue::Int(*self as i64)
			}
			fn from_db_value(value: QueryValue) -> Result<Self> {
				match value {
					QueryValue::Int(i) => Ok(i as $ty),
					other => Err(wrong_type("an integer", &other)),
				}
			}
		}
	};
}
int_impl!(i16);
int_impl!(i32);
int_impl!(i64);

macro_rules! float_impl {
	($ty:ty) => {
		impl DbValue for $ty {
			fn to_db_value(&self) -> QueryValue {
				QueryValue::Float(*self as f64)
			}
			fn from_db_value(value: QueryValue) -> Result<Self> {
				match value {
					QueryValue::Float(f) => Ok(f as $ty),
					// Integer columns can surface as Int; accept for a float field.
					QueryValue::Int(i) => Ok(i as $ty),
					other => Err(wrong_type("a number", &other)),
				}
			}
		}
	};
}
float_impl!(f32);
float_impl!(f64);

impl DbValue for bool {
	fn to_db_value(&self) -> QueryValue {
		QueryValue::Bool(*self)
	}
	fn from_db_value(value: QueryValue) -> Result<Self> {
		match value {
			QueryValue::Bool(b) => Ok(b),
			other => Err(wrong_type("a boolean", &other)),
		}
	}
}

impl DbValue for String {
	fn to_db_value(&self) -> QueryValue {
		QueryValue::String(self.clone())
	}
	fn from_db_value(value: QueryValue) -> Result<Self> {
		match value {
			QueryValue::String(s) => Ok(s),
			other => Err(wrong_type("a string", &other)),
		}
	}
}

impl DbValue for chrono::DateTime<chrono::Utc> {
	fn to_db_value(&self) -> QueryValue {
		QueryValue::Timestamp(*self)
	}
	fn from_db_value(value: QueryValue) -> Result<Self> {
		match value {
			QueryValue::Timestamp(t) => Ok(t),
			// A row projected through serde (e.g. a synthesized row) carries the
			// timestamp as an RFC3339 string; accept it too.
			QueryValue::String(s) => chrono::DateTime::parse_from_rfc3339(&s)
				.map(|dt| dt.with_timezone(&chrono::Utc))
				.map_err(|e| Error::Database(format!("invalid timestamp {s:?}: {e}"))),
			other => Err(wrong_type("a timestamp", &other)),
		}
	}
}

impl DbValue for uuid::Uuid {
	fn to_db_value(&self) -> QueryValue {
		QueryValue::Uuid(*self)
	}
	fn from_db_value(value: QueryValue) -> Result<Self> {
		match value {
			QueryValue::Uuid(u) => Ok(u),
			// A serde-projected row carries the uuid as a string; accept it too.
			QueryValue::String(s) => uuid::Uuid::parse_str(&s)
				.map_err(|e| Error::Database(format!("invalid uuid {s:?}: {e}"))),
			other => Err(wrong_type("a uuid", &other)),
		}
	}
}

impl DbValue for serde_json::Value {
	fn to_db_value(&self) -> QueryValue {
		QueryValue::Json(self.clone())
	}
	fn from_db_value(value: QueryValue) -> Result<Self> {
		match value {
			QueryValue::Json(v) => Ok(v),
			other => Err(wrong_type("json", &other)),
		}
	}
}

impl DbValue for Vec<u8> {
	fn to_db_value(&self) -> QueryValue {
		QueryValue::Bytes(self.clone())
	}
	fn from_db_value(value: QueryValue) -> Result<Self> {
		match value {
			QueryValue::Bytes(b) => Ok(b),
			other => Err(wrong_type("bytes", &other)),
		}
	}
}

impl DbValue for chrono::NaiveDateTime {
	fn to_db_value(&self) -> QueryValue {
		QueryValue::Timestamp(self.and_utc())
	}
	fn from_db_value(value: QueryValue) -> Result<Self> {
		match value {
			QueryValue::Timestamp(t) => Ok(t.naive_utc()),
			other => Err(wrong_type("a timestamp", &other)),
		}
	}
}

impl DbValue for rust_decimal::Decimal {
	// Mirrors the current decimal path (convert_row reads it as f64) — lossy at
	// f64. A non-lossy version would need a `QueryValue::Decimal` variant + a
	// convert_row branch; this keeps parity with today's behavior.
	fn to_db_value(&self) -> QueryValue {
		use rust_decimal::prelude::ToPrimitive;
		QueryValue::Float(self.to_f64().unwrap_or_default())
	}
	fn from_db_value(value: QueryValue) -> Result<Self> {
		use rust_decimal::prelude::FromPrimitive;
		match value {
			QueryValue::Float(f) => rust_decimal::Decimal::from_f64(f)
				.ok_or_else(|| Error::Database(format!("invalid decimal {f}"))),
			QueryValue::Int(i) => Ok(rust_decimal::Decimal::from(i)),
			other => Err(wrong_type("a number", &other)),
		}
	}
}

/// Nullable columns: `None` is SQL NULL; anything else defers to the inner type.
impl<T: DbValue> DbValue for Option<T> {
	fn to_db_value(&self) -> QueryValue {
		match self {
			Some(v) => v.to_db_value(),
			None => QueryValue::Null,
		}
	}
	fn from_db_value(value: QueryValue) -> Result<Self> {
		match value {
			QueryValue::Null => Ok(None),
			other => Ok(Some(T::from_db_value(other)?)),
		}
	}
}
