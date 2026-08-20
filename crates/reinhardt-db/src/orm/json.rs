//! Typed JSON field wrapper for model fields.

use super::model::Model;
use super::{DatabaseStorageKind, DatabaseValue, FieldCodecError};
#[cfg(feature = "pgvector")]
use crate::field_domain::MAX_DENSE_VECTOR_DIMENSIONS;
use base64::Engine;
use serde::de::value::{MapDeserializer, StringDeserializer};
use serde::de::{IntoDeserializer, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};
use std::collections::HashSet;
use std::ops::{Deref, DerefMut};

/// Newtype wrapper for JSON-backed model fields.
///
/// `Json<T>` serializes and deserializes exactly like `T`, while making the
/// model field type explicit enough for `#[model]` to emit JSON column
/// metadata.
#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Json<T>(pub T);

impl<T> Json<T> {
	/// Creates a typed JSON wrapper.
	pub const fn new(value: T) -> Self {
		Self(value)
	}

	/// Returns the wrapped value.
	pub fn into_inner(self) -> T {
		self.0
	}

	/// Borrows the wrapped value.
	pub const fn as_inner(&self) -> &T {
		&self.0
	}

	/// Mutably borrows the wrapped value.
	pub fn as_inner_mut(&mut self) -> &mut T {
		&mut self.0
	}

	/// Converts the wrapped value into a JSON value.
	pub fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error>
	where
		T: Serialize,
	{
		serde_json::to_value(&self.0)
	}

	/// Builds a typed JSON wrapper from a JSON value.
	pub fn from_json_value(value: serde_json::Value) -> Result<Self, serde_json::Error>
	where
		T: DeserializeOwned,
	{
		serde_json::from_value(value).map(Self)
	}
}

impl<T> Deref for Json<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl<T> DerefMut for Json<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl<T> From<T> for Json<T> {
	fn from(value: T) -> Self {
		Self(value)
	}
}

impl<T> Serialize for Json<T>
where
	T: Serialize,
{
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		self.0.serialize(serializer)
	}
}

impl<'de, T> Deserialize<'de> for Json<T>
where
	T: Deserialize<'de>,
{
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: Deserializer<'de>,
	{
		T::deserialize(deserializer).map(Self)
	}
}

pub(crate) fn is_json_field_type(field_type: &str) -> bool {
	field_type.contains("JsonField")
		|| field_type.contains("JSONField")
		|| field_type.contains("JSONBField")
		|| field_type.contains("HStoreField")
}

pub(crate) fn deserialize_model_row<M: Model>(
	data: serde_json::Value,
	mut json_null_fields: HashSet<String>,
	mut native_json_fields: HashSet<String>,
) -> Result<M, FieldCodecError> {
	let serde_json::Value::Object(mut fields) = data else {
		return serde_json::from_value(data)
			.map_err(|error| FieldCodecError::Serialization(error.to_string()));
	};

	for field in M::field_metadata() {
		let column_name = field.db_column_name().to_string();
		let source_name = if fields.contains_key(&field.name) {
			field.name.clone()
		} else {
			column_name
		};
		let Some(stored_value) = fields.remove(&source_name) else {
			continue;
		};

		let is_json = field.storage_kind != Some(DatabaseStorageKind::Bytes)
			&& (is_json_field_type(&field.field_type) || field.field_type.contains("ArrayField"));
		let was_native_json_null = is_json && json_null_fields.remove(&source_name);
		let was_native_json = is_json && native_json_fields.remove(&source_name);
		let was_json_text = is_json && stored_value.is_string() && !was_native_json;
		let parsed_value = match stored_value {
			serde_json::Value::String(text) if is_json && !was_native_json => {
				serde_json::from_str(&text).map_err(|error| {
					FieldCodecError::Serialization(format!(
						"Failed to hydrate JSON field {}.{} from column '{}': {}",
						M::table_name(),
						field.name,
						field.db_column_name(),
						error
					))
				})?
			}
			value => value,
		};

		let preserves_json_null = was_native_json_null || was_json_text;
		let database_value = if parsed_value.is_null() && !preserves_json_null {
			DatabaseValue::Null
		} else {
			database_value_from_json(parsed_value, field.storage_kind)?
		};
		let decoded_value = M::decode_database_field(&field.name, database_value)?;
		if field.nullable && decoded_value.is_null() && preserves_json_null {
			json_null_fields.insert(field.name.clone());
		}

		fields.insert(field.name.clone(), decoded_value);
	}

	let renamed_fields: Vec<_> = M::field_metadata()
		.into_iter()
		.filter(|field| {
			!is_json_field_type(&field.field_type) && field.db_column_name() != field.name
		})
		.collect();
	if !renamed_fields.is_empty() {
		let mut aliased_fields = fields.clone();
		let mut has_physical_alias = false;
		for field in &renamed_fields {
			if let Some(value) = fields.get(field.db_column_name()).cloned() {
				aliased_fields.entry(field.name.clone()).or_insert(value);
				has_physical_alias = true;
			}
		}

		if has_physical_alias {
			let aliased_data = serde_json::Value::Object(aliased_fields);
			match deserialize_model_value::<M>(aliased_data, &json_null_fields) {
				Ok(model) => return Ok(model),
				Err(aliased_error) => {
					if let Ok(model) = deserialize_model_value::<M>(
						serde_json::Value::Object(fields.clone()),
						&json_null_fields,
					) {
						return Ok(model);
					}

					let mut logical_fields = fields;
					for field in renamed_fields {
						let column_name = field.db_column_name().to_string();
						if logical_fields.contains_key(&field.name) {
							logical_fields.remove(&column_name);
						} else if let Some(value) = logical_fields.remove(&column_name) {
							logical_fields.insert(field.name, value);
						}
					}
					return deserialize_model_value(
						serde_json::Value::Object(logical_fields),
						&json_null_fields,
					)
					.map_err(|error| FieldCodecError::Serialization(error.to_string()))
					.or(Err(FieldCodecError::Serialization(
						aliased_error.to_string(),
					)));
				}
			}
		}
	}

	match deserialize_model_value::<M>(serde_json::Value::Object(fields.clone()), &json_null_fields)
	{
		Ok(model) => Ok(model),
		Err(logical_error) => {
			let mut physical_fields = fields;
			for field in M::field_metadata().into_iter().filter(|field| {
				!is_json_field_type(&field.field_type) && field.db_column_name() != field.name
			}) {
				if let Some(value) = physical_fields.remove(&field.name) {
					physical_fields.insert(field.db_column_name().to_string(), value);
				}
			}
			deserialize_model_value::<M>(
				serde_json::Value::Object(physical_fields),
				&json_null_fields,
			)
			.map_err(|_| FieldCodecError::Serialization(logical_error.to_string()))
		}
	}
}

pub(crate) fn database_value_from_json(
	value: serde_json::Value,
	storage_kind: Option<DatabaseStorageKind>,
) -> Result<DatabaseValue, FieldCodecError> {
	match storage_kind {
		Some(DatabaseStorageKind::Json) => Ok(DatabaseValue::Json(value)),
		_ if value.is_null() => Ok(DatabaseValue::Null),
		#[cfg(feature = "pgvector")]
		Some(DatabaseStorageKind::Vector(dimensions)) => {
			if dimensions == 0 || dimensions > MAX_DENSE_VECTOR_DIMENSIONS {
				return Err(FieldCodecError::Serialization(format!(
					"vector dimension {dimensions} exceeds maximum {MAX_DENSE_VECTOR_DIMENSIONS}"
				)));
			}
			let values = serde_json::from_value::<Vec<f32>>(value)
				.map_err(|error| FieldCodecError::Serialization(error.to_string()))?;
			if values.len() != dimensions {
				return Err(FieldCodecError::Serialization(format!(
					"vector dimension mismatch: expected {dimensions}, got {}",
					values.len()
				)));
			}
			if let Some((index, _)) = values
				.iter()
				.enumerate()
				.find(|(_, value)| !value.is_finite())
			{
				return Err(FieldCodecError::Serialization(format!(
					"vector element at index {index} is not finite"
				)));
			}
			Ok(DatabaseValue::Vector(values))
		}
		Some(DatabaseStorageKind::Bool) => serde_json::from_value(value)
			.map(DatabaseValue::Bool)
			.map_err(|error| FieldCodecError::Serialization(error.to_string())),
		Some(DatabaseStorageKind::I32) => serde_json::from_value(value)
			.map(DatabaseValue::I32)
			.map_err(|error| FieldCodecError::Serialization(error.to_string())),
		Some(DatabaseStorageKind::I64) => serde_json::from_value(value)
			.map(DatabaseValue::I64)
			.map_err(|error| FieldCodecError::Serialization(error.to_string())),
		Some(DatabaseStorageKind::F32) => match value {
			serde_json::Value::String(value) => value
				.parse::<f32>()
				.map(DatabaseValue::F32)
				.map_err(|error| FieldCodecError::Serialization(error.to_string())),
			value => serde_json::from_value(value)
				.map(DatabaseValue::F32)
				.map_err(|error| FieldCodecError::Serialization(error.to_string())),
		},
		Some(DatabaseStorageKind::F64) => match value {
			serde_json::Value::String(value) => value
				.parse::<f64>()
				.map(DatabaseValue::F64)
				.map_err(|error| FieldCodecError::Serialization(error.to_string())),
			value => serde_json::from_value(value)
				.map(DatabaseValue::F64)
				.map_err(|error| FieldCodecError::Serialization(error.to_string())),
		},
		Some(DatabaseStorageKind::Decimal) => {
			serde_json::from_value::<rust_decimal::Decimal>(value)
				.map(DatabaseValue::Decimal)
				.map_err(|error| FieldCodecError::Serialization(error.to_string()))
		}
		Some(DatabaseStorageKind::String) => serde_json::from_value(value)
			.map(DatabaseValue::String)
			.map_err(|error| FieldCodecError::Serialization(error.to_string())),
		Some(DatabaseStorageKind::Bytes) => match value {
			serde_json::Value::Array(_) => serde_json::from_value::<Vec<u8>>(value)
				.map(DatabaseValue::Bytes)
				.map_err(|error| FieldCodecError::Serialization(error.to_string())),
			value => serde_json::from_value::<String>(value)
				.map_err(|error| FieldCodecError::Serialization(error.to_string()))
				.and_then(|value| {
					base64::engine::general_purpose::STANDARD
						.decode(value)
						.map(DatabaseValue::Bytes)
						.map_err(|error| FieldCodecError::Serialization(error.to_string()))
				}),
		},
		Some(DatabaseStorageKind::Uuid) => serde_json::from_value::<String>(value)
			.map_err(|error| FieldCodecError::Serialization(error.to_string()))
			.and_then(|value| {
				uuid::Uuid::parse_str(&value)
					.map(DatabaseValue::Uuid)
					.map_err(|error| FieldCodecError::Serialization(error.to_string()))
			}),
		Some(DatabaseStorageKind::Date) => serde_json::from_value::<String>(value)
			.map_err(|error| FieldCodecError::Serialization(error.to_string()))
			.and_then(|value| {
				chrono::NaiveDate::parse_from_str(&value, "%Y-%m-%d")
					.map(DatabaseValue::Date)
					.map_err(|error| FieldCodecError::Serialization(error.to_string()))
			}),
		Some(DatabaseStorageKind::Time) => serde_json::from_value::<String>(value)
			.map_err(|error| FieldCodecError::Serialization(error.to_string()))
			.and_then(|value| {
				chrono::NaiveTime::parse_from_str(&value, "%H:%M:%S%.f")
					.map(DatabaseValue::Time)
					.map_err(|error| FieldCodecError::Serialization(error.to_string()))
			}),
		Some(DatabaseStorageKind::DateTime) => serde_json::from_value::<String>(value)
			.map_err(|error| FieldCodecError::Serialization(error.to_string()))
			.and_then(|value| {
				chrono::DateTime::parse_from_rfc3339(&value)
					.map(|value| DatabaseValue::DateTime(value.with_timezone(&chrono::Utc)))
					.map_err(|error| FieldCodecError::Serialization(error.to_string()))
			}),
		Some(DatabaseStorageKind::NaiveDateTime) => serde_json::from_value::<String>(value)
			.map_err(|error| FieldCodecError::Serialization(error.to_string()))
			.and_then(|value| {
				chrono::NaiveDateTime::parse_from_str(&value, "%Y-%m-%d %H:%M:%S%.f")
					.or_else(|_| {
						chrono::DateTime::parse_from_rfc3339(&value)
							.map(|datetime| datetime.naive_local())
					})
					.map(DatabaseValue::NaiveDateTime)
					.map_err(|error| FieldCodecError::Serialization(error.to_string()))
			}),
		None => DatabaseValue::try_from_json_value(value),
	}
}

pub(crate) fn deserialize_model_value<M: Model>(
	data: serde_json::Value,
	json_null_fields: &HashSet<String>,
) -> Result<M, serde_json::Error> {
	if json_null_fields.is_empty() {
		return serde_json::from_value(data);
	}

	let serde_json::Value::Object(fields) = data else {
		return serde_json::from_value(data);
	};
	let entries = fields.into_iter().map(|(name, value)| {
		let value = if json_null_fields.contains(&name) {
			ModelFieldValue::PresentJsonNull
		} else {
			ModelFieldValue::Value(value)
		};
		(StringDeserializer::<serde_json::Error>::new(name), value)
	});
	M::deserialize(MapDeserializer::<_, serde_json::Error>::new(entries))
}

enum ModelFieldValue {
	Value(serde_json::Value),
	PresentJsonNull,
}

impl<'de> IntoDeserializer<'de, serde_json::Error> for ModelFieldValue {
	type Deserializer = Self;

	fn into_deserializer(self) -> Self::Deserializer {
		self
	}
}

impl<'de> Deserializer<'de> for ModelFieldValue {
	type Error = serde_json::Error;

	fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: Visitor<'de>,
	{
		match self {
			Self::Value(value) => value.into_deserializer().deserialize_any(visitor),
			Self::PresentJsonNull => serde_json::Value::Null
				.into_deserializer()
				.deserialize_any(visitor),
		}
	}

	fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: Visitor<'de>,
	{
		match self {
			Self::Value(value) => value.into_deserializer().deserialize_option(visitor),
			Self::PresentJsonNull => {
				visitor.visit_some(serde_json::Value::Null.into_deserializer())
			}
		}
	}

	serde::forward_to_deserialize_any! {
		bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes byte_buf
		unit unit_struct newtype_struct seq tuple tuple_struct map struct enum identifier ignored_any
	}
}

#[cfg(test)]
mod tests {
	use super::{Json, database_value_from_json, deserialize_model_row};
	use crate::orm::{DatabaseStorageKind, DatabaseValue};
	use reinhardt_core::macros::model;
	use serde::{Deserialize, Serialize};
	use serde_json::json;
	use std::collections::HashSet;

	#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
	struct StyleSettings {
		indent_width: u8,
		theme: String,
	}

	#[model(app_label = "tests", table_name = "byte_row_models")]
	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct ByteRowModel {
		#[field(primary_key = true)]
		id: i64,
		payload: Vec<u8>,
	}

	#[model(app_label = "tests", table_name = "decimal_float_row_models")]
	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct DecimalFloatRowModel {
		#[field(primary_key = true)]
		id: i64,
		amount: f64,
	}

	#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
	struct NullablePayload {
		enabled: bool,
	}

	#[model(app_label = "tests", table_name = "nullable_json_row_models")]
	#[derive(Debug, Clone, Serialize, Deserialize)]
	struct NullableJsonRowModel {
		#[field(primary_key = true)]
		id: i64,
		#[field(null = true)]
		payload: Option<Json<NullablePayload>>,
	}

	#[test]
	fn json_wrapper_serializes_as_inner_value() {
		let settings = Json::new(StyleSettings {
			indent_width: 2,
			theme: "paper".to_string(),
		});

		let value = serde_json::to_value(&settings).unwrap();

		assert_eq!(
			value,
			json!({
				"indent_width": 2,
				"theme": "paper"
			})
		);
	}

	#[test]
	fn json_wrapper_deserializes_as_inner_value() {
		let settings: Json<StyleSettings> = serde_json::from_value(json!({
			"indent_width": 4,
			"theme": "ink"
		}))
		.unwrap();

		assert_eq!(settings.indent_width, 4);
		assert_eq!(settings.theme, "ink");
	}

	#[test]
	fn json_value_is_supported_as_inner_type() {
		let metadata = Json::new(json!({ "language": "ja", "tags": ["draft"] }));

		assert_eq!(metadata["language"], "ja");
		assert_eq!(metadata.to_json_value().unwrap()["tags"][0], "draft");
	}

	#[test]
	fn byte_storage_accepts_serde_byte_arrays() {
		let value =
			database_value_from_json(json!([0, 1, 127, 255]), Some(DatabaseStorageKind::Bytes))
				.expect("byte arrays should decode");

		assert_eq!(value, DatabaseValue::Bytes(vec![0, 1, 127, 255]));
	}

	#[test]
	fn json_storage_preserves_json_null() {
		let value = database_value_from_json(json!(null), Some(DatabaseStorageKind::Json))
			.expect("JSON null should decode");

		assert_eq!(value, DatabaseValue::Json(json!(null)));
	}

	#[cfg(feature = "pgvector")]
	#[test]
	fn vector_storage_decodes_numeric_json_arrays() {
		let value =
			database_value_from_json(json!([1.0, 2.0, 3.0]), Some(DatabaseStorageKind::Vector(3)))
				.unwrap();

		assert_eq!(value, DatabaseValue::Vector(vec![1.0, 2.0, 3.0]));
	}

	#[test]
	#[cfg(feature = "pgvector")]
	fn vector_storage_rejects_runtime_dimension_mismatches() {
		let error =
			database_value_from_json(json!([1.0, 2.0]), Some(DatabaseStorageKind::Vector(3)))
				.unwrap_err();

		assert_eq!(
			error.to_string(),
			"field serialization failed: vector dimension mismatch: expected 3, got 2"
		);
	}

	#[test]
	#[cfg(feature = "pgvector")]
	fn vector_storage_rejects_unsupported_declared_dimensions() {
		for (dimensions, value) in [(0, json!([])), (2_001, json!(vec![0.0; 2_001]))] {
			let error =
				database_value_from_json(value, Some(DatabaseStorageKind::Vector(dimensions)))
					.unwrap_err();

			assert_eq!(
				error.to_string(),
				format!(
					"field serialization failed: vector dimension {dimensions} exceeds maximum 2000"
				)
			);
		}
	}

	#[test]
	fn naive_datetime_storage_accepts_backend_rfc3339_text_without_shifting() {
		let value = database_value_from_json(
			json!("2026-07-26T09:30:00.123456Z"),
			Some(DatabaseStorageKind::NaiveDateTime),
		)
		.expect("RFC 3339 timestamp should decode as a naive wall-clock value");

		assert_eq!(
			value,
			DatabaseValue::NaiveDateTime(
				chrono::NaiveDateTime::parse_from_str(
					"2026-07-26 09:30:00.123456",
					"%Y-%m-%d %H:%M:%S%.f",
				)
				.expect("expected test datetime"),
			)
		);
	}

	#[test]
	fn model_rows_decode_byte_vectors_from_base64_without_json_parsing() {
		// Arrange
		let row = json!({ "id": 1, "payload": "AQID" });

		// Act
		let model = deserialize_model_row::<ByteRowModel>(row, HashSet::new(), HashSet::new())
			.expect("base64 byte data should hydrate");

		// Assert
		assert_eq!(model.id, 1);
		assert_eq!(model.payload, vec![1, 2, 3]);
	}

	#[test]
	fn model_rows_decode_decimal_strings_as_f64() {
		// Arrange
		let row = json!({ "id": 1, "amount": "99.99" });

		// Act
		let model =
			deserialize_model_row::<DecimalFloatRowModel>(row, HashSet::new(), HashSet::new())
				.expect("decimal strings should hydrate as f64 values");

		// Assert
		assert_eq!(model.id, 1);
		assert_eq!(model.amount, 99.99);
	}

	#[test]
	fn model_rows_keep_sql_null_as_none_for_typed_json_fields() {
		// Arrange
		let row = json!({ "id": 1, "payload": null });
		let native_json_fields = HashSet::from([String::from("payload")]);

		// Act
		let model =
			deserialize_model_row::<NullableJsonRowModel>(row, HashSet::new(), native_json_fields)
				.expect("SQL NULL should hydrate as None");

		// Assert
		assert_eq!(model.id, 1);
		assert_eq!(model.payload, None);
	}
}
