//! Serde helper functions for admin server types.
//!
//! Provides custom deserializers to bridge the gap between ORM row
//! representation and Rust struct types. In particular, the ORM stores
//! `Vec<String>` fields as TEXT columns whose values are JSON-encoded
//! strings (e.g., `"[]"`), whereas serde expects a native JSON array.

use serde::{Deserialize, Deserializer};

/// Deserializes a `Vec<String>` from either a JSON array or a JSON string
/// containing an array.
///
/// The ORM pipeline converts PostgreSQL TEXT columns to
/// `serde_json::Value::String(...)`.  When the column holds a
/// JSON-encoded array (e.g., `"[\"read\",\"write\"]"`), the default
/// `Vec<String>` deserializer fails because it receives a string, not
/// an array.  This function transparently handles both representations.
pub(crate) fn string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
	D: Deserializer<'de>,
{
	#[derive(Deserialize)]
	#[serde(untagged)]
	enum StringOrVec {
		Vec(Vec<String>),
		String(String),
	}

	match StringOrVec::deserialize(deserializer)? {
		StringOrVec::Vec(v) => Ok(v),
		StringOrVec::String(s) => serde_json::from_str(&s).map_err(serde::de::Error::custom),
	}
}
