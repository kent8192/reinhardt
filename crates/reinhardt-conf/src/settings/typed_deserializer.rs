//! Type-aware deserializer adapter.
//!
//! Wraps `&serde_json::Value` and, at the visitor boundary, coerces
//! `Value::String` into the target Rust type when the visitor expects a
//! non-String type. Coercion strategies are per-shape:
//!
//! | Visitor expectation | Coercion                               |
//! |---------------------|----------------------------------------|
//! | scalar (bool/int/   | `FromStr`                              |
//! |   float/char)       |                                        |
//! | `Option<T>`         | empty string -> None, else recurse     |
//! | `Vec<T>`            | parse as JSON array, recurse per item  |
//! | `Map<K, V>`         | parse as JSON object, recurse per item |
//! | `Vec<u8>` (bytes)   | base64 (STANDARD)                      |
//! | enum                | delegate to `serde_json`; failures     |
//! |                     |   wrap as `CoercionError::Parse`       |
//! | struct / tuple /    | `CoercionError::UnsupportedShape`      |
//! |   tuple struct      |                                        |
//!
//! The wrapper never re-runs interpolation: the resolved string is
//! consumed exactly once.

use super::interpolation::KeyPath;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::de::{DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::forward_to_deserialize_any;

/// Short, human-readable name for a `serde_json::Value` variant.
///
/// Used in `CoercionError::Parse` source messages to disambiguate
/// "expected JSON array, got <kind>" failures.
fn json_kind_name(v: &serde_json::Value) -> &'static str {
	match v {
		serde_json::Value::Null => "null",
		serde_json::Value::Bool(_) => "bool",
		serde_json::Value::Number(_) => "number",
		serde_json::Value::String(_) => "string",
		serde_json::Value::Array(_) => "array",
		serde_json::Value::Object(_) => "object",
	}
}

/// JSON container shape accepted by typed string coercion.
#[derive(Clone, Copy)]
pub(crate) enum JsonContainerShape {
	/// A JSON object.
	Object,
	/// A JSON array.
	Array,
}

/// Parse a JSON container literal produced by an interpolated string.
///
/// This is shared by typed deserialization and value-free contract validation
/// so both paths accept the same array and object string forms.
pub(crate) fn coerce_json_container(
	value: &serde_json::Value,
	shape: JsonContainerShape,
) -> Option<serde_json::Value> {
	let parsed = serde_json::from_str(value.as_str()?).ok()?;
	match (shape, &parsed) {
		(JsonContainerShape::Object, serde_json::Value::Object(_))
		| (JsonContainerShape::Array, serde_json::Value::Array(_)) => Some(parsed),
		_ => None,
	}
}

/// Implement a `deserialize_<scalar>` method that coerces `Value::String`
/// into the target scalar type via `FromStr`. For non-string inputs, it
/// delegates to `serde_json::Value`'s deserializer and re-wraps any
/// resulting error as `CoercionError::Parse` so callers always see a
/// uniform error shape carrying `key_path` / `target_type`.
///
/// `$err_ty` is the concrete error type returned by `<$ty as FromStr>::Err`
/// (e.g. `std::str::ParseBoolError`, `std::num::ParseIntError`,
/// `std::num::ParseFloatError`). It is required because `s.parse()` infers
/// `Err = <$ty as FromStr>::Err`, and the closure's `e:` annotation must
/// match for `Box::new(e)` to coerce into `Box<dyn Error + Send + Sync>`.
macro_rules! impl_scalar_coerce {
	($method:ident, $visit:ident, $ty:ty, $err_ty:ty, $type_name:expr) => {
		fn $method<V>(self, visitor: V) -> Result<V::Value, Self::Error>
		where
			V: Visitor<'de>,
		{
			match self.value {
				serde_json::Value::String(s) => {
					let parsed: $ty = s.parse().map_err(|e: $err_ty| CoercionError::Parse {
						target_type: $type_name.to_string(),
						value: s.clone(),
						key_path: self.key_path.to_string(),
						source: Box::new(e),
					})?;
					visitor.$visit(parsed)
				}
				other => other
					.clone()
					.$method(visitor)
					.map_err(|e| CoercionError::Parse {
						target_type: $type_name.to_string(),
						value: other.to_string(),
						key_path: self.key_path.to_string(),
						source: Box::new(e),
					}),
			}
		}
	};
}

/// Coercion failure surfaced from `TypedSettingsDeserializer`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CoercionError {
	/// Failed to parse a string into the visitor's expected scalar /
	/// bytes / collection-element shape.
	#[error(
		"failed to coerce value `{value}` into {target_type} at \
		key `{key_path}`: {source}"
	)]
	Parse {
		/// `Visitor::expecting()` output for the destination type.
		target_type: String,
		/// Original string value that failed to coerce.
		value: String,
		/// Dot/bracket-separated TOML key path.
		key_path: String,
		/// Underlying parse error.
		#[source]
		source: Box<dyn std::error::Error + Send + Sync>,
	},

	/// The visitor expected a shape (struct, tuple, non-unit enum) that
	/// has no defined string -> T coercion.
	#[error(
		"cannot coerce string into {target} at key `{key_path}`: \
		use field-by-field interpolation instead"
	)]
	UnsupportedShape {
		/// Description of the unsupported destination shape.
		target: &'static str,
		/// Dot/bracket-separated TOML key path.
		key_path: String,
	},

	/// Pass-through for serde-internal errors.
	#[error(transparent)]
	Serde(#[from] serde::de::value::Error),
}

impl serde::de::Error for CoercionError {
	fn custom<T: std::fmt::Display>(msg: T) -> Self {
		CoercionError::Serde(serde::de::Error::custom(msg))
	}
}

/// Type-aware adapter around `&serde_json::Value`.
///
/// Phase 2 skeleton: forwards every `deserialize_*` to `serde_json`'s
/// own deserializer. Subsequent phases override individual visitor
/// callbacks to coerce `Value::String` into the target Rust type.
///
/// The struct's lifetime `'a` is the borrow lifetime of the wrapped
/// value. It is decoupled from the serde `'de` deserialization
/// lifetime so that `TypedSeqAccess` can own its element storage and
/// hand out children that borrow from it for shorter than `'de`.
pub struct TypedSettingsDeserializer<'a> {
	value: &'a serde_json::Value,
	key_path: KeyPath,
}

impl<'a> TypedSettingsDeserializer<'a> {
	/// Wrap a borrowed `serde_json::Value`. The returned deserializer
	/// coerces string values when the visitor expects a non-String
	/// target.
	pub fn new(value: &'a serde_json::Value) -> Self {
		Self {
			value,
			key_path: KeyPath::default(),
		}
	}
}

impl<'de, 'a> Deserializer<'de> for TypedSettingsDeserializer<'a> {
	type Error = CoercionError;

	// Phase 3a + 3b + 3c + 3d + 4 (#4226): bool, integers, floats, char,
	// enum, `Option<T>`, bytes, and sequences are explicitly overridden
	// below. Scalars use `FromStr`; bytes use base64 STANDARD; sequences
	// recurse element-by-element so per-element scalar coercion fires.
	// `map`/`struct` are also overridden so that per-field dispatch flows
	// through this deserializer (otherwise the scalar overrides would be
	// unreachable when the top-level value is an object). Remaining shapes
	// still forward to `deserialize_any`.
	forward_to_deserialize_any! {
		str string unit unit_struct newtype_struct identifier ignored_any
	}

	fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: Visitor<'de>,
	{
		// Delegate to serde_json's deserializer; map its error into ours.
		self.value
			.clone()
			.deserialize_any(visitor)
			.map_err(|e| CoercionError::Parse {
				target_type: "any".to_string(),
				value: self.value.to_string(),
				key_path: self.key_path.to_string(),
				source: Box::new(e),
			})
	}

	impl_scalar_coerce!(
		deserialize_bool,
		visit_bool,
		bool,
		std::str::ParseBoolError,
		"bool"
	);
	impl_scalar_coerce!(deserialize_i8, visit_i8, i8, std::num::ParseIntError, "i8");
	impl_scalar_coerce!(
		deserialize_i16,
		visit_i16,
		i16,
		std::num::ParseIntError,
		"i16"
	);
	impl_scalar_coerce!(
		deserialize_i32,
		visit_i32,
		i32,
		std::num::ParseIntError,
		"i32"
	);
	impl_scalar_coerce!(
		deserialize_i64,
		visit_i64,
		i64,
		std::num::ParseIntError,
		"i64"
	);
	impl_scalar_coerce!(
		deserialize_i128,
		visit_i128,
		i128,
		std::num::ParseIntError,
		"i128"
	);
	impl_scalar_coerce!(deserialize_u8, visit_u8, u8, std::num::ParseIntError, "u8");
	impl_scalar_coerce!(
		deserialize_u16,
		visit_u16,
		u16,
		std::num::ParseIntError,
		"u16"
	);
	impl_scalar_coerce!(
		deserialize_u32,
		visit_u32,
		u32,
		std::num::ParseIntError,
		"u32"
	);
	impl_scalar_coerce!(
		deserialize_u64,
		visit_u64,
		u64,
		std::num::ParseIntError,
		"u64"
	);
	impl_scalar_coerce!(
		deserialize_u128,
		visit_u128,
		u128,
		std::num::ParseIntError,
		"u128"
	);
	impl_scalar_coerce!(
		deserialize_f32,
		visit_f32,
		f32,
		std::num::ParseFloatError,
		"f32"
	);
	impl_scalar_coerce!(
		deserialize_f64,
		visit_f64,
		f64,
		std::num::ParseFloatError,
		"f64"
	);

	fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: Visitor<'de>,
	{
		// `Value::Null` and the empty string both map to `None`. The empty-
		// string rule supports env-var defaults like `${VAR:-}` where an
		// optional field naturally produces `""`. For any other value, recurse
		// into `self` so that scalar coercion (Phase 3a/3b) fires for the
		// `Some(_)` payload's inner type.
		match self.value {
			serde_json::Value::Null => visitor.visit_none(),
			serde_json::Value::String(s) if s.is_empty() => visitor.visit_none(),
			_ => visitor.visit_some(self),
		}
	}

	fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: Visitor<'de>,
	{
		match self.value {
			serde_json::Value::String(s) => {
				let mut iter = s.chars();
				let c = iter.next().ok_or_else(|| CoercionError::Parse {
					target_type: "char".to_string(),
					value: s.clone(),
					key_path: self.key_path.to_string(),
					source: "expected exactly one char, got empty string".into(),
				})?;
				if iter.next().is_some() {
					return Err(CoercionError::Parse {
						target_type: "char".to_string(),
						value: s.clone(),
						key_path: self.key_path.to_string(),
						source: format!(
							"expected exactly one char, got {} chars",
							s.chars().count()
						)
						.into(),
					});
				}
				visitor.visit_char(c)
			}
			other => other
				.clone()
				.deserialize_char(visitor)
				.map_err(|e| CoercionError::Parse {
					target_type: "char".to_string(),
					value: other.to_string(),
					key_path: self.key_path.to_string(),
					source: Box::new(e),
				}),
		}
	}

	fn deserialize_enum<V>(
		self,
		name: &'static str,
		variants: &'static [&'static str],
		visitor: V,
	) -> Result<V::Value, Self::Error>
	where
		V: Visitor<'de>,
	{
		self.value
			.clone()
			.deserialize_enum(name, variants, visitor)
			.map_err(|e| CoercionError::Parse {
				target_type: format!("enum {name}"),
				value: self.value.to_string(),
				key_path: self.key_path.to_string(),
				source: Box::new(e),
			})
	}

	fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: Visitor<'de>,
	{
		// Accept both native `Value::Object(...)` and JSON-object literals
		// embedded in `Value::String(...)`. The string path supports
		// operator-friendly env-var defaults like
		// `${WEIGHTS:-{"a":1,"b":2}}`. Per-entry dispatch flows back through
		// `TypedSettingsDeserializer` so per-value scalar coercion fires
		// (e.g. `HashMap<String, i32>` from `{"a": "1"}` parses each string
		// value via the `i32` `FromStr` override).
		let object: serde_json::Map<String, serde_json::Value> = match self.value {
			serde_json::Value::Object(o) => o.clone(),
			serde_json::Value::String(s) => {
				if let Some(serde_json::Value::Object(object)) =
					coerce_json_container(self.value, JsonContainerShape::Object)
				{
					return visitor.visit_map(TypedMapAccess {
						entries: object.into_iter(),
						pending_value: None,
						pending_key: None,
						key_path: self.key_path,
					});
				}
				let parsed: serde_json::Value =
					serde_json::from_str(s).map_err(|e| CoercionError::Parse {
						target_type: "object".to_string(),
						value: s.clone(),
						key_path: self.key_path.to_string(),
						source: Box::new(e),
					})?;
				match parsed {
					serde_json::Value::Object(o) => o,
					other => {
						return Err(CoercionError::Parse {
							target_type: "object".to_string(),
							value: s.clone(),
							key_path: self.key_path.to_string(),
							source: format!(
								"expected JSON object after parsing string, got {}",
								json_kind_name(&other)
							)
							.into(),
						});
					}
				}
			}
			other => {
				return Err(CoercionError::Parse {
					target_type: "object".to_string(),
					value: other.to_string(),
					key_path: self.key_path.to_string(),
					source: format!("expected object, got {}", json_kind_name(other)).into(),
				});
			}
		};

		visitor.visit_map(TypedMapAccess {
			entries: object.into_iter(),
			pending_value: None,
			pending_key: None,
			key_path: self.key_path,
		})
	}

	fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: Visitor<'de>,
	{
		// Accept both native `Value::Array(...)` and JSON-array literals
		// embedded in `Value::String(...)`. The string path supports
		// operator-friendly env-var defaults like `${PORTS:-[5432, 5433]}`.
		// Element-wise dispatch flows back through
		// `TypedSettingsDeserializer` so per-element scalar coercion fires
		// (e.g. `Vec<u16>` from `["5432", "5433"]` parses each element via
		// the `u16` `FromStr` override).
		let array: Vec<serde_json::Value> = match self.value {
			serde_json::Value::Array(a) => a.clone(),
			serde_json::Value::String(s) => {
				if let Some(serde_json::Value::Array(array)) =
					coerce_json_container(self.value, JsonContainerShape::Array)
				{
					return visitor.visit_seq(TypedSeqAccess {
						values: array,
						index: 0,
						key_path: self.key_path,
					});
				}
				let parsed: serde_json::Value =
					serde_json::from_str(s).map_err(|e| CoercionError::Parse {
						target_type: "array".to_string(),
						value: s.clone(),
						key_path: self.key_path.to_string(),
						source: Box::new(e),
					})?;
				match parsed {
					serde_json::Value::Array(a) => a,
					other => {
						return Err(CoercionError::Parse {
							target_type: "array".to_string(),
							value: s.clone(),
							key_path: self.key_path.to_string(),
							source: format!(
								"expected JSON array after parsing string, got {}",
								json_kind_name(&other)
							)
							.into(),
						});
					}
				}
			}
			other => {
				return Err(CoercionError::Parse {
					target_type: "array".to_string(),
					value: other.to_string(),
					key_path: self.key_path.to_string(),
					source: format!("expected array, got {}", json_kind_name(other)).into(),
				});
			}
		};

		visitor.visit_seq(TypedSeqAccess {
			values: array,
			index: 0,
			key_path: self.key_path,
		})
	}

	fn deserialize_struct<V>(
		self,
		_name: &'static str,
		_fields: &'static [&'static str],
		visitor: V,
	) -> Result<V::Value, Self::Error>
	where
		V: Visitor<'de>,
	{
		// String -> struct has no defined coercion. Surface a clear
		// `UnsupportedShape` error directing callers to use field-by-field
		// interpolation instead, rather than letting an opaque serde
		// "expected map, got string" error escape via `deserialize_map`.
		if matches!(self.value, serde_json::Value::String(_)) {
			return Err(CoercionError::UnsupportedShape {
				target: "struct",
				key_path: self.key_path.to_string(),
			});
		}
		// Structs deserialize via `visit_map`; route through the same
		// adapter as `deserialize_map`.
		self.deserialize_map(visitor)
	}

	fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: Visitor<'de>,
	{
		// String -> tuple has no defined coercion. Same rationale as
		// `deserialize_struct`: emit `UnsupportedShape` to direct callers
		// to per-element interpolation. Native arrays still flow through
		// `serde_json`'s deserializer.
		if matches!(self.value, serde_json::Value::String(_)) {
			return Err(CoercionError::UnsupportedShape {
				target: "tuple",
				key_path: self.key_path.to_string(),
			});
		}
		self.value
			.clone()
			.deserialize_tuple(len, visitor)
			.map_err(|e| CoercionError::Parse {
				target_type: "tuple".to_string(),
				value: self.value.to_string(),
				key_path: self.key_path.to_string(),
				source: Box::new(e),
			})
	}

	fn deserialize_tuple_struct<V>(
		self,
		name: &'static str,
		len: usize,
		visitor: V,
	) -> Result<V::Value, Self::Error>
	where
		V: Visitor<'de>,
	{
		// String -> tuple struct has no defined coercion. Same rationale
		// as `deserialize_tuple`: emit `UnsupportedShape` instead of a
		// generic serde error.
		if matches!(self.value, serde_json::Value::String(_)) {
			return Err(CoercionError::UnsupportedShape {
				target: "tuple struct",
				key_path: self.key_path.to_string(),
			});
		}
		self.value
			.clone()
			.deserialize_tuple_struct(name, len, visitor)
			.map_err(|e| CoercionError::Parse {
				target_type: format!("tuple struct {name}"),
				value: self.value.to_string(),
				key_path: self.key_path.to_string(),
				source: Box::new(e),
			})
	}

	fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: Visitor<'de>,
	{
		// `forward_to_deserialize_any!` would route bytes through
		// `deserialize_any`, which for `Value::String` calls `visit_str` —
		// not `visit_bytes`. Override here so visitors that ask for bytes
		// (e.g. `serde_bytes`) receive base64-decoded bytes when the source
		// value is a JSON string. Native `Value::Array` of integers is
		// preserved by delegating to `serde_json`'s own deserializer.
		match self.value {
			serde_json::Value::String(s) => {
				let decoded = BASE64_STANDARD
					.decode(s)
					.map_err(|e| CoercionError::Parse {
						target_type: "bytes".to_string(),
						value: s.clone(),
						key_path: self.key_path.to_string(),
						source: Box::new(e),
					})?;
				visitor.visit_byte_buf(decoded)
			}
			other => other
				.clone()
				.deserialize_bytes(visitor)
				.map_err(|e| CoercionError::Parse {
					target_type: "bytes".to_string(),
					value: other.to_string(),
					key_path: self.key_path.to_string(),
					source: Box::new(e),
				}),
		}
	}

	fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: Visitor<'de>,
	{
		// Same coercion rules as `deserialize_bytes`; serde permits routing
		// `byte_buf` through `bytes` because the visitor is required to
		// accept either via `visit_bytes` / `visit_byte_buf`.
		self.deserialize_bytes(visitor)
	}
}

/// `MapAccess` adapter that re-wraps each value in a
/// `TypedSettingsDeserializer` so per-field type overrides apply, and
/// pushes the current key onto `KeyPath` so error messages identify the
/// failing field.
///
/// The struct owns its `serde_json::Map` iterator because the source may
/// be a parsed-from-string object whose backing storage does not outlive
/// `'de`. Children borrow from `self.pending_value` for the duration of
/// `next_value_seed`, which is shorter than (or equal to) `'de` — and
/// the child deserializer's struct lifetime is decoupled from `'de`, so
/// this is sound. Mirrors `TypedSeqAccess`.
struct TypedMapAccess {
	entries: serde_json::map::IntoIter,
	pending_key: Option<String>,
	pending_value: Option<serde_json::Value>,
	key_path: KeyPath,
}

impl<'de> MapAccess<'de> for TypedMapAccess {
	type Error = CoercionError;

	fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
	where
		K: DeserializeSeed<'de>,
	{
		match self.entries.next() {
			Some((k, v)) => {
				// Keys are always strings in JSON objects, but typed coercion
				// applies to their declared key type just as it does to values.
				let key_value = serde_json::Value::String(k.clone());
				let key = seed
					.deserialize(TypedSettingsDeserializer {
						value: &key_value,
						key_path: self.key_path.clone(),
					})
					.map_err(|e| CoercionError::Parse {
						target_type: "map_key".to_string(),
						value: k.clone(),
						key_path: self.key_path.to_string(),
						source: Box::new(e),
					})?;
				self.pending_key = Some(k);
				self.pending_value = Some(v);
				Ok(Some(key))
			}
			None => Ok(None),
		}
	}

	fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
	where
		V: DeserializeSeed<'de>,
	{
		let value = self
			.pending_value
			.take()
			.ok_or_else(|| CoercionError::Parse {
				target_type: "map_value".to_string(),
				value: String::new(),
				key_path: self.key_path.to_string(),
				source: "next_value_seed called before next_key_seed".into(),
			})?;
		let key = self.pending_key.take();

		// Build a child `KeyPath` that includes the current field name
		// so coercion errors identify the failing key.
		let mut child_path = self.key_path.clone();
		if let Some(k) = key.as_deref() {
			child_path.push_key(k);
		}
		let de = TypedSettingsDeserializer {
			value: &value,
			key_path: child_path,
		};
		seed.deserialize(de)
	}
}

/// `SeqAccess` adapter that re-wraps each element in a
/// `TypedSettingsDeserializer` so per-element type overrides apply,
/// and pushes the current index onto `KeyPath` so error messages
/// identify the failing element.
///
/// The struct owns its `Vec<Value>` because the source may be a parsed
/// JSON string whose backing storage does not outlive `'de`. Children
/// borrow from `self.values` for the duration of `next_element_seed`,
/// which is shorter than (or equal to) `'de` — but the child
/// deserializer's struct lifetime is decoupled from `'de`, so this is
/// sound.
struct TypedSeqAccess {
	values: Vec<serde_json::Value>,
	index: usize,
	key_path: KeyPath,
}

impl<'de> SeqAccess<'de> for TypedSeqAccess {
	type Error = CoercionError;

	fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
	where
		T: DeserializeSeed<'de>,
	{
		if self.index >= self.values.len() {
			return Ok(None);
		}
		let value = &self.values[self.index];
		let mut child_path = self.key_path.clone();
		child_path.push_index(self.index);
		self.index += 1;

		let de = TypedSettingsDeserializer {
			value,
			key_path: child_path,
		};
		seed.deserialize(de).map(Some)
	}

	fn size_hint(&self) -> Option<usize> {
		Some(self.values.len().saturating_sub(self.index))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serde::Deserialize;

	#[rstest]
	fn skeleton_passthrough_for_native_int() {
		// Arrange — native JSON int, no coerce needed.
		let v = serde_json::json!({ "port": 5432 });

		#[derive(Debug, Deserialize, PartialEq)]
		struct S {
			port: u16,
		}

		// Act
		let de = TypedSettingsDeserializer::new(&v);
		let got: S = S::deserialize(de).expect("should deserialize");

		// Assert
		assert_eq!(got, S { port: 5432 });
	}
}
