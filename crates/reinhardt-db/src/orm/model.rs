use base64::Engine;
use reinhardt_core::exception::{DatabaseError, DatabaseErrorKind, Error};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

use super::inspection::FieldInfo;
use super::{DatabaseField, DatabaseScalar, DatabaseValue, FieldCodecError};

/// Deserializes one route segment into a model primary-key type.
#[doc(hidden)]
pub fn deserialize_primary_key_from_str<T>(value: &str) -> Result<T, serde_json::Error>
where
	T: serde::de::DeserializeOwned,
{
	serde_json::from_value(serde_json::Value::String(value.to_owned()))
		.or_else(|_| serde_json::from_str(value))
}

/// Deserializes a route segment through a generated primary-key database codec.
#[doc(hidden)]
pub fn deserialize_primary_key_from_database_str<M>(
	value: &str,
) -> Result<M::PrimaryKey, FieldCodecError>
where
	M: Model,
	M::PrimaryKey: DatabaseField,
{
	let storage_kind = <M::PrimaryKey as DatabaseField>::Storage::STORAGE_KIND;
	let value = match storage_kind {
		super::DatabaseStorageKind::Decimal | super::DatabaseStorageKind::String => {
			serde_json::Value::String(value.to_owned())
		}
		_ => serde_json::from_str(value)
			.unwrap_or_else(|_| serde_json::Value::String(value.to_owned())),
	};
	let database_value = super::json::database_value_from_json(value, Some(storage_kind))?;
	let decoded = M::decode_database_field(M::primary_key_field(), database_value)?;
	serde_json::from_value(decoded)
		.map_err(|error| FieldCodecError::Serialization(error.to_string()))
}

fn legacy_storage_kind(field_type: &str) -> Option<super::DatabaseStorageKind> {
	if field_type.contains("UuidField") || field_type.contains("UUIDField") {
		Some(super::DatabaseStorageKind::Uuid)
	} else if field_type.contains("DateTimeField") {
		Some(super::DatabaseStorageKind::DateTime)
	} else if field_type.contains("DateField") {
		Some(super::DatabaseStorageKind::Date)
	} else if field_type.contains("TimeField") {
		Some(super::DatabaseStorageKind::Time)
	} else if field_type.contains("BooleanField") {
		Some(super::DatabaseStorageKind::Bool)
	} else if field_type.contains("BigIntegerField") {
		Some(super::DatabaseStorageKind::I64)
	} else if field_type.contains("IntegerField") {
		Some(super::DatabaseStorageKind::I32)
	} else if field_type.contains("FloatField") {
		Some(super::DatabaseStorageKind::F64)
	} else if field_type.contains("DecimalField") {
		Some(super::DatabaseStorageKind::Decimal)
	} else {
		None
	}
}

/// Convert a route component using the storage type recorded for its model field.
#[doc(hidden)]
pub fn filter_value_from_field(
	field: &FieldInfo,
	value: &str,
) -> Result<super::query::FilterValue, FieldCodecError> {
	use super::DatabaseStorageKind;

	let Some(storage_kind) = field
		.storage_kind
		.or_else(|| legacy_storage_kind(&field.field_type))
	else {
		return Ok(super::query::FilterValue::String(value.to_owned()));
	};

	let database_value =
		match storage_kind {
			DatabaseStorageKind::Bool => DatabaseValue::Bool(value.parse().map_err(|_| {
				FieldCodecError::Serialization(format!("invalid boolean value: {value}"))
			})?),
			DatabaseStorageKind::I32 => DatabaseValue::I32(value.parse().map_err(|_| {
				FieldCodecError::Serialization(format!("invalid i32 value: {value}"))
			})?),
			DatabaseStorageKind::I64 => DatabaseValue::I64(value.parse().map_err(|_| {
				FieldCodecError::Serialization(format!("invalid i64 value: {value}"))
			})?),
			DatabaseStorageKind::F32 => DatabaseValue::F32(value.parse().map_err(|_| {
				FieldCodecError::Serialization(format!("invalid f32 value: {value}"))
			})?),
			DatabaseStorageKind::F64 => DatabaseValue::F64(value.parse().map_err(|_| {
				FieldCodecError::Serialization(format!("invalid f64 value: {value}"))
			})?),
			DatabaseStorageKind::Decimal => {
				DatabaseValue::Decimal(value.parse().map_err(|_| {
					FieldCodecError::Serialization(format!("invalid decimal value: {value}"))
				})?)
			}
			DatabaseStorageKind::String => DatabaseValue::String(value.to_owned()),
			DatabaseStorageKind::Bytes => DatabaseValue::Bytes(
				base64::engine::general_purpose::STANDARD
					.decode(value)
					.map_err(|error| FieldCodecError::Serialization(error.to_string()))?,
			),
			DatabaseStorageKind::Uuid => DatabaseValue::Uuid(value.parse().map_err(|_| {
				FieldCodecError::Serialization(format!("invalid UUID value: {value}"))
			})?),
			DatabaseStorageKind::Date => DatabaseValue::Date(value.parse().map_err(|_| {
				FieldCodecError::Serialization(format!("invalid date value: {value}"))
			})?),
			DatabaseStorageKind::Time => DatabaseValue::Time(value.parse().map_err(|_| {
				FieldCodecError::Serialization(format!("invalid time value: {value}"))
			})?),
			DatabaseStorageKind::DateTime => DatabaseValue::DateTime(
				chrono::DateTime::parse_from_rfc3339(value)
					.or_else(|_| value.parse::<chrono::DateTime<chrono::FixedOffset>>())
					.map_err(|_| {
						FieldCodecError::Serialization(format!("invalid datetime value: {value}"))
					})?
					.with_timezone(&chrono::Utc),
			),
			DatabaseStorageKind::NaiveDateTime => DatabaseValue::NaiveDateTime(
				chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
					.or_else(|_| {
						chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f")
					})
					.map_err(|_| {
						FieldCodecError::Serialization(format!("invalid datetime value: {value}"))
					})?,
			),
			_ => return Ok(super::query::FilterValue::String(value.to_owned())),
		};

	Ok(super::query::FilterValue::Typed(Ok(database_value)))
}

/// JSON carrier used only for final whole-model assembly after field decoding.
#[doc(hidden)]
pub type ModelFieldJsonValue = serde_json::Value;

/// Serializes a decoded typed database field for final model assembly.
#[doc(hidden)]
pub fn serialize_decoded_database_field<T: Serialize>(
	value: T,
) -> Result<ModelFieldJsonValue, FieldCodecError> {
	serde_json::to_value(value).map_err(|error| FieldCodecError::Serialization(error.to_string()))
}

/// Canonical fixture field values keyed by model field name.
#[doc(hidden)]
pub type FixtureFields = serde_json::Map<String, serde_json::Value>;

/// One canonical fixture field value.
#[doc(hidden)]
pub type FixtureValue = serde_json::Value;

/// Trait for type-safe field selectors
///
/// This trait is automatically implemented for field selector structs generated
/// by the `#[model(...)]` macro (e.g., `UserFields`).
pub trait FieldSelector: Clone {
	/// Set table alias for all fields
	///
	/// This is used for self-joins where the same table appears multiple times
	/// with different aliases.
	fn with_alias(self, alias: &str) -> Self;
}

/// Core trait for database models
/// Uses composition instead of inheritance - models can implement multiple traits
///
/// # Breaking Change (Phase 4)
///
/// A new associated type `Fields` has been added. It provides a type-safe field selector.
/// When using the `#[model(...)]` macro, this implementation is automatically generated.
pub trait Model: Serialize + for<'de> Deserialize<'de> + Send + Sync + Clone {
	/// The primary key type
	type PrimaryKey: Send + Sync + Clone + std::fmt::Display;

	/// Type-safe field selector
	///
	/// This type is automatically generated by the `#[model(...)]` macro as `{Model}Fields`.
	/// It provides compile-time type safety for field references in queries.
	type Fields: FieldSelector;

	/// The manager type returned by `objects()`.
	///
	/// Defaults to [`Manager<Self>`](super::Manager) when no custom manager is
	/// configured. When `#[model(manager = MyManager)]` is specified, the macro
	/// sets this to the custom manager type, so `objects()` returns the custom
	/// manager directly.
	type Objects: super::custom_manager::CustomManager<Model = Self> + Default;

	/// Get the table name
	fn table_name() -> &'static str;

	/// Create a new field selector instance
	///
	/// This method is automatically implemented by the `#[model(...)]` macro.
	/// It returns a new instance of the type-safe field selector.
	fn new_fields() -> Self::Fields;

	/// Get the app label for this model
	///
	/// This is used by the migration system to organize models by application.
	/// Defaults to "default" if not specified.
	fn app_label() -> &'static str {
		"default"
	}

	/// Get the primary key field name
	fn primary_key_field() -> &'static str {
		"id"
	}

	/// Get the physical database column that stores the primary key.
	///
	/// Manual model implementations default to the Rust field name. The model
	/// macro overrides this when `db_column` renames the primary-key column.
	fn primary_key_column() -> &'static str {
		Self::primary_key_field()
	}

	/// Get the physical database columns used as the default latest ordering.
	///
	/// Manual model implementations have no default latest ordering. The model
	/// macro overrides this method for `#[model(get_latest_by = (...))]`.
	fn latest_by_fields() -> &'static [&'static str] {
		&[]
	}

	/// Encodes a primary key into its canonical database representation.
	///
	/// Macro-generated models route this through the primary-key field's
	/// [`DatabaseField`] implementation. Manual model
	/// implementations retain the legacy numeric, UUID, or string fallback and
	/// can override this method for custom primary-key codecs.
	fn primary_key_database_value(pk: &Self::PrimaryKey) -> Result<DatabaseValue, FieldCodecError> {
		let value = pk.to_string();
		let field_type = Self::field_metadata()
			.into_iter()
			.find(|field| field.name == Self::primary_key_field())
			.map(|field| field.field_type);

		let value = match field_type
			.as_deref()
			.and_then(|value| value.rsplit('.').next())
		{
			Some("AutoField")
			| Some("IntegerField")
			| Some("BigAutoField")
			| Some("BigIntegerField") => value
				.parse::<i64>()
				.map(DatabaseValue::I64)
				.unwrap_or_else(|_| DatabaseValue::String(value.clone())),
			Some("UuidField") => uuid::Uuid::parse_str(&value)
				.map(DatabaseValue::Uuid)
				.unwrap_or_else(|_| DatabaseValue::String(value.clone())),
			_ => value
				.parse::<i64>()
				.map(DatabaseValue::I64)
				.unwrap_or(DatabaseValue::String(value)),
		};

		Ok(value)
	}

	/// Converts a primary key into a query filter value.
	///
	/// Primitive integer primary keys retain numeric bindings, while standard
	/// string primary keys retain exact string bindings. Other hand-written key
	/// types retain the historical numeric-or-string fallback for compatibility;
	/// custom string-like newtypes should override this method for exact binding.
	/// Derived models override this conversion for declared primary-key types with
	/// a dedicated database binding, such as strings, UUIDs, and timestamps.
	fn primary_key_filter_value(pk: Self::PrimaryKey) -> super::query::FilterValue {
		let value = pk.to_string();
		let type_name = std::any::type_name::<Self::PrimaryKey>();

		if [
			std::any::type_name::<i8>(),
			std::any::type_name::<i16>(),
			std::any::type_name::<i32>(),
			std::any::type_name::<i64>(),
			std::any::type_name::<isize>(),
			std::any::type_name::<i128>(),
		]
		.contains(&type_name)
		{
			return value
				.parse::<i128>()
				.map(super::query::FilterValue::from)
				.unwrap_or(super::query::FilterValue::String(value));
		}

		if [
			std::any::type_name::<u8>(),
			std::any::type_name::<u16>(),
			std::any::type_name::<u32>(),
			std::any::type_name::<u64>(),
			std::any::type_name::<usize>(),
			std::any::type_name::<u128>(),
		]
		.contains(&type_name)
		{
			return value
				.parse::<u128>()
				.map(super::query::FilterValue::from)
				.unwrap_or(super::query::FilterValue::String(value));
		}

		if matches!(
			type_name,
			name if name == std::any::type_name::<String>()
				|| name == std::any::type_name::<&str>()
				|| name == std::any::type_name::<std::borrow::Cow<'static, str>>()
		) {
			return super::query::FilterValue::String(value);
		}

		if type_name == std::any::type_name::<uuid::Uuid>() {
			return value
				.parse()
				.map(super::query::FilterValue::Uuid)
				.unwrap_or(super::query::FilterValue::String(value));
		}

		if type_name == std::any::type_name::<bool>() {
			return value
				.parse()
				.map(super::query::FilterValue::Boolean)
				.unwrap_or(super::query::FilterValue::String(value));
		}

		if type_name == std::any::type_name::<f32>() {
			return value
				.parse::<f32>()
				.map(|value| super::query::FilterValue::Float(value as f64))
				.unwrap_or(super::query::FilterValue::String(value));
		}

		if type_name == std::any::type_name::<f64>() {
			return value
				.parse::<f64>()
				.map(super::query::FilterValue::Float)
				.unwrap_or(super::query::FilterValue::String(value));
		}

		if type_name == std::any::type_name::<chrono::DateTime<chrono::Utc>>() {
			return chrono::DateTime::parse_from_rfc3339(&value)
				.or_else(|_| value.parse::<chrono::DateTime<chrono::FixedOffset>>())
				.map(|value| {
					super::query::FilterValue::Timestamp(value.with_timezone(&chrono::Utc))
				})
				.unwrap_or(super::query::FilterValue::String(value));
		}

		value
			.parse::<i64>()
			.map(super::query::FilterValue::Integer)
			.unwrap_or(super::query::FilterValue::String(value))
	}

	/// Converts a route primary key into a query filter value.
	///
	/// Derived models override this method so UUIDs, timestamps, and custom
	/// primary-key codecs use the same typed conversion as model instances.
	fn primary_key_filter_value_from_str(
		value: &str,
	) -> reinhardt_core::exception::Result<super::query::FilterValue> {
		use reinhardt_core::exception::Error;

		let type_name = std::any::type_name::<Self::PrimaryKey>();
		macro_rules! parse_standard_integer {
			($integer:ty, $category:literal) => {
				if type_name == std::any::type_name::<$integer>() {
					return value
						.parse::<$integer>()
						.map(super::query::FilterValue::from)
						.map_err(|_| {
							Error::Validation(format!(
								concat!("invalid ", $category, " primary key: {}"),
								value
							))
						});
				}
			};
		}

		parse_standard_integer!(i8, "integer");
		parse_standard_integer!(i16, "integer");
		parse_standard_integer!(i32, "integer");
		parse_standard_integer!(i64, "integer");
		parse_standard_integer!(isize, "integer");
		parse_standard_integer!(i128, "integer");
		parse_standard_integer!(u8, "unsigned integer");
		parse_standard_integer!(u16, "unsigned integer");
		parse_standard_integer!(u32, "unsigned integer");
		parse_standard_integer!(u64, "unsigned integer");
		parse_standard_integer!(usize, "unsigned integer");
		parse_standard_integer!(u128, "unsigned integer");

		if type_name == std::any::type_name::<uuid::Uuid>() {
			return value
				.parse()
				.map(super::query::FilterValue::Uuid)
				.map_err(|_| Error::Validation(format!("invalid UUID primary key: {value}")));
		}

		if type_name == std::any::type_name::<bool>() {
			return value
				.parse()
				.map(super::query::FilterValue::Boolean)
				.map_err(|_| Error::Validation(format!("invalid boolean primary key: {value}")));
		}

		if type_name == std::any::type_name::<f32>() {
			return value
				.parse::<f32>()
				.map(|value| super::query::FilterValue::Float(value as f64))
				.map_err(|_| Error::Validation(format!("invalid float primary key: {value}")));
		}

		if type_name == std::any::type_name::<f64>() {
			return value
				.parse::<f64>()
				.map(super::query::FilterValue::Float)
				.map_err(|_| Error::Validation(format!("invalid float primary key: {value}")));
		}

		if type_name == std::any::type_name::<chrono::DateTime<chrono::Utc>>() {
			return chrono::DateTime::parse_from_rfc3339(value)
				.or_else(|_| value.parse::<chrono::DateTime<chrono::FixedOffset>>())
				.map(|value| {
					super::query::FilterValue::Timestamp(value.with_timezone(&chrono::Utc))
				})
				.map_err(|_| Error::Validation(format!("invalid timestamp primary key: {value}")));
		}

		macro_rules! parse_typed_primary_key {
			($ty:ty, $variant:ident, $category:literal) => {
				if type_name == std::any::type_name::<$ty>() {
					return value
						.parse::<$ty>()
						.map(|parsed| {
							super::query::FilterValue::Typed(Ok(DatabaseValue::$variant(parsed)))
						})
						.map_err(|_| {
							Error::Validation(format!(
								concat!("invalid ", $category, " primary key: {}"),
								value
							))
						});
				}
			};
		}

		parse_typed_primary_key!(chrono::NaiveDate, Date, "date");
		parse_typed_primary_key!(chrono::NaiveTime, Time, "time");
		parse_typed_primary_key!(chrono::NaiveDateTime, NaiveDateTime, "naive datetime");
		parse_typed_primary_key!(rust_decimal::Decimal, Decimal, "decimal");

		Ok(super::query::FilterValue::String(value.to_owned()))
	}

	/// Get the primary key value
	///
	/// Returns an owned copy of the primary key. For composite primary keys,
	/// this constructs a new PK value from the component fields.
	fn primary_key(&self) -> Option<Self::PrimaryKey>;

	/// Set the primary key value
	fn set_primary_key(&mut self, value: Self::PrimaryKey);

	/// Get composite primary key definition if this model uses composite PK
	///
	/// Returns None for single primary key models, Some(CompositePrimaryKey) for composite PK models.
	fn composite_primary_key() -> Option<super::composite_pk::CompositePrimaryKey> {
		None
	}

	/// Get composite primary key values for this instance
	///
	/// Only meaningful for models with composite primary keys.
	/// Returns empty HashMap for single primary key models.
	fn get_composite_pk_values(&self) -> HashMap<String, super::composite_pk::PkValue> {
		HashMap::new()
	}

	/// Returns whether the named model field is currently `None`.
	///
	/// Macro-generated implementations inspect `Option<T>` fields directly so
	/// `None` remains distinguishable from a present value that serializes as
	/// JSON `null`. Manual implementations use this serialization-based fallback,
	/// which treats a serialized JSON `null` as `None`.
	///
	/// Returns `false` when serialization fails, the model does not serialize to
	/// an object, or the field name is unknown.
	fn field_is_none(&self, field_name: &str) -> bool {
		match serde_json::to_value(self) {
			Ok(serde_json::Value::Object(fields)) => {
				fields.get(field_name).is_some_and(|value| value.is_null())
			}
			_ => false,
		}
	}

	/// Encodes model fields into their canonical database representations.
	///
	/// Macro-generated models override this method with typed field codecs.
	/// This serde-based implementation preserves compatibility for manual model
	/// implementations.
	fn encode_database_fields(&self) -> Result<BTreeMap<String, DatabaseValue>, FieldCodecError> {
		let value = serde_json::to_value(self)
			.map_err(|error| FieldCodecError::Serialization(error.to_string()))?;
		let fields = value.as_object().ok_or_else(|| {
			FieldCodecError::Serialization("model must serialize to an object".to_owned())
		})?;
		let metadata = Self::field_metadata();

		fields
			.iter()
			.map(|(name, value)| {
				let metadata = metadata.iter().find(|field| field.name == *name);
				let storage_kind = metadata.and_then(|field| {
					field
						.storage_kind
						.or_else(|| legacy_storage_kind(&field.field_type))
				});
				let is_json_field = metadata
					.is_some_and(|field| super::json::is_json_field_type(&field.field_type));
				let value = if is_json_field && !self.field_is_none(name) {
					DatabaseValue::Json(value.clone())
				} else {
					super::json::database_value_from_json(value.clone(), storage_kind)?
				};
				Ok((name.clone(), value))
			})
			.collect()
	}

	/// Decodes one canonical database value for final model assembly.
	///
	/// Macro-generated models override this method with typed field codecs.
	fn decode_database_field(
		_field_name: &str,
		value: DatabaseValue,
	) -> Result<serde_json::Value, FieldCodecError> {
		value.into_json_value()
	}

	/// Validate canonical fixture fields before they are written to the database.
	///
	/// Macro-generated models override this with a projection that excludes
	/// database-generated fields and ignores API-facing serde naming rules.
	/// Manual models that expose field metadata use the fixture layer's canonical
	/// field mapping, because that metadata does not retain API-facing serde
	/// aliases. Manual implementations without metadata retain the serde-based
	/// fallback and can override this method when they need stricter validation.
	#[doc(hidden)]
	fn validate_fixture_fields(fields: &FixtureFields) -> Result<(), String> {
		if Self::field_metadata().is_empty() {
			let _: Self = serde_json::from_value(serde_json::Value::Object(fields.clone()))
				.map_err(|error| error.to_string())?;
		}
		Ok(())
	}

	/// Get field metadata for inspection
	///
	/// This method should be implemented to provide introspection capabilities.
	/// By default, returns an empty vector. Override this in derive macros or
	/// manual implementations to provide actual field metadata.
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_db::orm::Model;
	///
	/// struct User {
	///     id: i32,
	///     name: String,
	/// }
	///
	/// impl Model for User {
	///     // ... other required methods ...
	///
	///     fn field_metadata() -> Vec<super::inspection::FieldInfo> {
	///         vec![
	///             // Field metadata would be generated here
	///         ]
	///     }
	/// }
	/// ```
	fn field_metadata() -> Vec<super::inspection::FieldInfo> {
		Vec::new()
	}

	/// Get relationship metadata for inspection
	///
	/// This method should be implemented to provide relationship introspection.
	/// By default, returns an empty vector. Override this in derive macros or
	/// manual implementations to provide actual relationship metadata.
	fn relationship_metadata() -> Vec<super::inspection::RelationInfo> {
		Vec::new()
	}

	/// Get index metadata for inspection
	///
	/// This method should be implemented to provide index introspection.
	/// By default, returns an empty vector. Override this in derive macros or
	/// manual implementations to provide actual index metadata.
	fn index_metadata() -> Vec<super::inspection::IndexInfo> {
		Vec::new()
	}

	/// Get constraint metadata for inspection
	///
	/// This method should be implemented to provide constraint introspection.
	/// By default, returns an empty vector. Override this in derive macros or
	/// manual implementations to provide actual constraint metadata.
	fn constraint_metadata() -> Vec<super::inspection::ConstraintInfo> {
		Vec::new()
	}

	/// Get database-generated column names that must be omitted from ORM writes.
	fn generated_field_names() -> &'static [&'static str] {
		&[]
	}

	/// Return whether a scalar integer primary key uses zero as its auto-generated sentinel.
	///
	/// The model macro enables this for non-`Option` integer primary keys whose
	/// `auto_increment` setting is enabled. Manual model implementations can
	/// override it when they use the same database convention.
	fn primary_key_uses_zero_sentinel() -> bool {
		false
	}

	/// Django-style objects manager accessor
	///
	/// Returns the configured manager for this model type. When a custom manager
	/// is specified via `#[model(manager = MyManager)]`, this returns the custom
	/// manager; otherwise it returns the default [`Manager<Self>`](super::Manager).
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct MyModel { id: Option<i64> }
	/// # #[derive(Clone)]
	/// # struct MyModelFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for MyModelFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for MyModel {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = MyModelFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn app_label() -> &'static str { "app" }
	/// #     fn table_name() -> &'static str { "table" }
	/// #     fn new_fields() -> Self::Fields { MyModelFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id.clone() }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn primary_key_field() -> &'static str { "id" }
	/// # }
	///
	/// # #[tokio::main]
	/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
	/// let manager = MyModel::objects();
	/// let all_records = manager.all().all().await?;
	/// # Ok(())
	/// # }
	/// ```
	fn objects() -> Self::Objects
	where
		Self: Sized,
	{
		Self::Objects::default()
	}

	/// Save the model instance to the database with event dispatching
	///
	/// If the primary key is None, performs an INSERT and dispatches before_insert/after_insert events.
	/// If the primary key is Some, performs an UPDATE and dispatches before_update/after_update events.
	///
	/// Event listeners can veto the operation by returning `EventResult::Veto`.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64>, name: String }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn app_label() -> &'static str { "app" }
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id.clone() }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn primary_key_field() -> &'static str { "id" }
	/// # }
	///
	/// # #[tokio::main]
	/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
	/// let mut user = User { id: None, name: "John".to_string() };
	///
	/// // INSERT - triggers before_insert/after_insert events
	/// user.save().await?;
	///
	/// // UPDATE - triggers before_update/after_update events
	/// user.name = "Jane".to_string();
	/// user.save().await?;
	/// # Ok(())
	/// # }
	/// ```
	fn save(
		&mut self,
	) -> impl std::future::Future<Output = reinhardt_core::exception::Result<()>> + Send
	where
		Self: Sized,
	{
		async move {
			use super::manager::get_connection;

			let mut conn = get_connection().await?;
			self.save_with_conn(&mut conn).await
		}
	}

	/// Save the model instance through a caller-owned ORM executor.
	fn save_with_conn<'a, E>(
		&'a mut self,
		conn: &'a mut E,
	) -> impl std::future::Future<Output = reinhardt_core::exception::Result<()>> + Send + 'a
	where
		Self: Sized,
		E: super::connection::OrmExecutor + 'a,
	{
		async move {
			use super::events::{EventResult, get_active_registry};

			let registry = get_active_registry();
			let manager = super::Manager::<Self>::new();

			let json = serde_json::to_value(&*self).map_err(|error| {
				Error::from(DatabaseError::new(
					DatabaseErrorKind::Serialization,
					error.to_string(),
				))
			})?;

			let uses_zero_sentinel_primary_key = Self::primary_key_uses_zero_sentinel()
				&& json
					.as_object()
					.and_then(|fields| fields.get(Self::primary_key_field()))
					.is_some_and(|value| value.as_i64() == Some(0) || value.as_u64() == Some(0));

			if self.primary_key().is_none() || uses_zero_sentinel_primary_key {
				// INSERT: new record
				let instance_id = format!("{}-new-{}", Self::table_name(), uuid::Uuid::now_v7());

				// Dispatch before_insert event if registry is active
				if let Some(ref reg) = registry {
					let result = reg
						.dispatch_before_insert(Self::table_name(), &instance_id, &json)
						.await;
					if result == EventResult::Veto {
						return Err(DatabaseError::new(
							DatabaseErrorKind::Query,
							"Insert operation vetoed by event listener",
						)
						.into());
					}
				}

				// Perform the INSERT
				let created = manager.create_with_conn(conn, self).await?;
				*self = created;

				// Dispatch after_insert event if registry is active
				if let Some(ref reg) = registry {
					let final_id = format!(
						"{}-{}",
						Self::table_name(),
						self.primary_key()
							.map(|pk| pk.to_string())
							.unwrap_or_default()
					);
					reg.dispatch_after_insert(Self::table_name(), &final_id)
						.await;
				}
			} else {
				// UPDATE: existing record
				let instance_id = format!(
					"{}-{}",
					Self::table_name(),
					self.primary_key()
						.map(|pk| pk.to_string())
						.unwrap_or_default()
				);

				// Dispatch before_update event if registry is active
				if let Some(ref reg) = registry {
					let result = reg
						.dispatch_before_update(Self::table_name(), &instance_id, &json)
						.await;
					if result == EventResult::Veto {
						return Err(DatabaseError::new(
							DatabaseErrorKind::Query,
							"Update operation vetoed by event listener",
						)
						.into());
					}
				}

				// Perform the UPDATE
				let updated = manager.update_with_conn(conn, self).await?;
				*self = updated;

				// Dispatch after_update event if registry is active
				if let Some(ref reg) = registry {
					reg.dispatch_after_update(Self::table_name(), &instance_id)
						.await;
				}
			}

			Ok(())
		}
	}

	/// Save this model through a caller-owned transaction executor.
	fn save_with_executor(
		&mut self,
		executor: &mut dyn super::connection::TransactionExecutor,
	) -> impl std::future::Future<Output = Result<(), crate::backends::error::DatabaseError>> + Send
	where
		Self: Sized,
	{
		async move {
			use super::events::{EventResult, get_active_registry};

			let registry = get_active_registry();
			let manager = super::Manager::<Self>::new();
			let json = serde_json::to_value(&*self).map_err(|error| {
				DatabaseError::new(DatabaseErrorKind::Serialization, error.to_string())
			})?;
			let is_insert = self.primary_key().is_none();
			let instance_id = if is_insert {
				format!("{}-new-{}", Self::table_name(), uuid::Uuid::now_v7())
			} else {
				format!(
					"{}-{}",
					Self::table_name(),
					self.primary_key()
						.map(|pk| pk.to_string())
						.unwrap_or_default()
				)
			};

			if let Some(ref registry) = registry {
				let result = if is_insert {
					registry
						.dispatch_before_insert(Self::table_name(), &instance_id, &json)
						.await
				} else {
					registry
						.dispatch_before_update(Self::table_name(), &instance_id, &json)
						.await
				};
				if result == EventResult::Veto {
					let operation = if is_insert { "Insert" } else { "Update" };
					return Err(crate::backends::error::DatabaseError::new(
						crate::backends::error::DatabaseErrorKind::Query,
						format!("{operation} operation vetoed by event listener"),
					));
				}
			}

			*self = manager.save_with_executor(executor, self).await?;

			if let Some(ref registry) = registry {
				if is_insert {
					let final_id = format!(
						"{}-{}",
						Self::table_name(),
						self.primary_key()
							.map(|pk| pk.to_string())
							.unwrap_or_default()
					);
					registry
						.dispatch_after_insert(Self::table_name(), &final_id)
						.await;
				} else {
					registry
						.dispatch_after_update(Self::table_name(), &instance_id)
						.await;
				}
			}

			Ok(())
		}
	}

	/// Insert this model through a caller-owned transaction executor.
	///
	/// Unlike [`Self::save_with_executor`], this always performs an INSERT. This
	/// is required for resources whose natural or UUID primary key is assigned
	/// before creation.
	fn insert_with_executor(
		&mut self,
		executor: &mut dyn super::connection::TransactionExecutor,
	) -> impl std::future::Future<Output = Result<(), crate::backends::error::DatabaseError>> + Send
	where
		Self: Sized,
	{
		async move {
			use super::events::{EventResult, get_active_registry};

			let registry = get_active_registry();
			let manager = super::Manager::<Self>::new();
			let json = serde_json::to_value(&*self).map_err(|error| {
				DatabaseError::new(DatabaseErrorKind::Serialization, error.to_string())
			})?;
			let instance_id = format!("{}-new-{}", Self::table_name(), uuid::Uuid::now_v7());

			if let Some(ref registry) = registry {
				let result = registry
					.dispatch_before_insert(Self::table_name(), &instance_id, &json)
					.await;
				if result == EventResult::Veto {
					return Err(crate::backends::error::DatabaseError::new(
						crate::backends::error::DatabaseErrorKind::Query,
						"Insert operation vetoed by event listener",
					));
				}
			}

			*self = manager.insert_with_executor(executor, self).await?;

			if let Some(ref registry) = registry {
				let final_id = format!(
					"{}-{}",
					Self::table_name(),
					self.primary_key()
						.map(|pk| pk.to_string())
						.unwrap_or_default()
				);
				registry
					.dispatch_after_insert(Self::table_name(), &final_id)
					.await;
			}

			Ok(())
		}
	}

	/// Delete the model instance from the database with event dispatching
	///
	/// Dispatches before_delete/after_delete events. Event listeners can veto
	/// the operation by returning `EventResult::Veto`.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_db::orm::Model;
	/// use serde::{Serialize, Deserialize};
	/// # #[derive(Debug, Clone, Serialize, Deserialize)]
	/// # struct User { id: Option<i64>, name: String }
	/// # #[derive(Clone)]
	/// # struct UserFields;
	/// # impl reinhardt_db::orm::model::FieldSelector for UserFields {
	/// #     fn with_alias(self, _alias: &str) -> Self { self }
	/// # }
	/// # impl Model for User {
	/// #     type PrimaryKey = i64;
	/// #     type Fields = UserFields;
	/// #     type Objects = reinhardt_db::orm::Manager<Self>;
	/// #     fn app_label() -> &'static str { "app" }
	/// #     fn table_name() -> &'static str { "users" }
	/// #     fn new_fields() -> Self::Fields { UserFields }
	/// #     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id.clone() }
	/// #     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
	/// #     fn primary_key_field() -> &'static str { "id" }
	/// # }
	///
	/// # #[tokio::main]
	/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
	/// let mut user = User { id: Some(1), name: "John".to_string() };
	///
	/// // Triggers before_delete/after_delete events
	/// user.delete().await?;
	/// # Ok(())
	/// # }
	/// ```
	fn delete(
		&self,
	) -> impl std::future::Future<Output = reinhardt_core::exception::Result<()>> + Send
	where
		Self: Sized,
	{
		async move {
			use super::manager::get_connection;

			let mut conn = get_connection().await?;
			self.delete_with_conn(&mut conn).await
		}
	}

	/// Delete the model instance through a caller-owned ORM executor.
	fn delete_with_conn<'a, E>(
		&'a self,
		conn: &'a mut E,
	) -> impl std::future::Future<Output = reinhardt_core::exception::Result<()>> + Send + 'a
	where
		Self: Sized,
		E: super::connection::OrmExecutor + 'a,
	{
		async move {
			use super::events::{EventResult, get_active_registry};

			let pk = self.primary_key().ok_or_else(|| {
				Error::from(DatabaseError::new(
					DatabaseErrorKind::Query,
					"Cannot delete model without primary key",
				))
			})?;

			let manager = super::Manager::<Self>::new();

			let instance_id = format!("{}-{}", Self::table_name(), pk);

			// Dispatch before_delete event if registry is available
			if let Some(registry) = get_active_registry() {
				let result = registry
					.dispatch_before_delete(Self::table_name(), &instance_id)
					.await;
				if result == EventResult::Veto {
					return Err(DatabaseError::new(
						DatabaseErrorKind::Query,
						"Delete operation vetoed by event listener",
					)
					.into());
				}
			}

			// Perform the DELETE
			manager.delete_with_conn(conn, pk.clone()).await?;

			// Dispatch after_delete event if registry is available
			if let Some(registry) = get_active_registry() {
				registry
					.dispatch_after_delete(Self::table_name(), &instance_id)
					.await;
			}

			Ok(())
		}
	}

	/// Delete this model through a caller-owned transaction executor.
	fn delete_with_executor(
		&self,
		executor: &mut dyn super::connection::TransactionExecutor,
	) -> impl std::future::Future<Output = Result<(), crate::backends::error::DatabaseError>> + Send
	where
		Self: Sized,
	{
		async move {
			use super::events::{EventResult, get_active_registry};

			let pk = self.primary_key().ok_or_else(|| {
				crate::backends::error::DatabaseError::new(
					crate::backends::error::DatabaseErrorKind::Query,
					"Cannot delete model without primary key",
				)
			})?;
			let instance_id = format!("{}-{}", Self::table_name(), pk);

			if let Some(registry) = get_active_registry() {
				let result = registry
					.dispatch_before_delete(Self::table_name(), &instance_id)
					.await;
				if result == EventResult::Veto {
					return Err(crate::backends::error::DatabaseError::new(
						crate::backends::error::DatabaseErrorKind::Query,
						"Delete operation vetoed by event listener",
					));
				}
			}

			super::Manager::<Self>::new()
				.delete_with_executor(executor, pk)
				.await?;

			if let Some(registry) = get_active_registry() {
				registry
					.dispatch_after_delete(Self::table_name(), &instance_id)
					.await;
			}

			Ok(())
		}
	}
}

/// Trait for models with timestamps - compose this with Model
/// This follows Rust's composition pattern rather than Django's inheritance
pub trait Timestamped {
	/// Returns the creation timestamp.
	fn created_at(&self) -> chrono::DateTime<chrono::Utc>;
	/// Returns the last update timestamp.
	fn updated_at(&self) -> chrono::DateTime<chrono::Utc>;
	/// Sets the last update timestamp.
	fn set_updated_at(&mut self, time: chrono::DateTime<chrono::Utc>);
}

/// Trait for soft-deletable models
/// Another composition trait instead of inheritance
pub trait SoftDeletable {
	/// Returns the deletion timestamp, or `None` if not deleted.
	fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>>;
	/// Sets the deletion timestamp, or `None` to restore.
	fn set_deleted_at(&mut self, time: Option<chrono::DateTime<chrono::Utc>>);
	/// Returns whether the model has been soft-deleted.
	fn is_deleted(&self) -> bool {
		self.deleted_at().is_some()
	}
}

/// Common timestamp fields that can be composed into structs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timestamps {
	/// The created at.
	pub created_at: chrono::DateTime<chrono::Utc>,
	/// The updated at.
	pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Timestamps {
	/// Creates a new Timestamps instance with current time
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::model::Timestamps;
	///
	/// let timestamps = Timestamps::now();
	/// assert!(timestamps.created_at <= chrono::Utc::now());
	/// assert!(timestamps.updated_at <= chrono::Utc::now());
	/// ```
	pub fn now() -> Self {
		let now = chrono::Utc::now();
		Self {
			created_at: now,
			updated_at: now,
		}
	}
	/// Updates the updated_at timestamp to current time
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::model::Timestamps;
	/// use chrono::Utc;
	///
	/// let mut timestamps = Timestamps::now();
	/// let old_updated = timestamps.updated_at;
	///
	/// // Wait a small amount to ensure time difference
	/// std::thread::sleep(std::time::Duration::from_millis(1));
	/// timestamps.touch();
	///
	/// assert!(timestamps.updated_at > old_updated);
	/// ```
	pub fn touch(&mut self) {
		self.updated_at = chrono::Utc::now();
	}
}

/// Soft delete field that can be composed into structs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftDelete {
	/// The deleted at.
	pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl SoftDelete {
	/// Creates a new SoftDelete instance with no deletion timestamp
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::model::SoftDelete;
	///
	/// let soft_delete = SoftDelete::new();
	/// assert!(soft_delete.deleted_at.is_none());
	/// ```
	pub fn new() -> Self {
		Self { deleted_at: None }
	}
	/// Marks the record as deleted by setting the deletion timestamp
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::model::SoftDelete;
	///
	/// let mut soft_delete = SoftDelete::new();
	/// assert!(!soft_delete.is_deleted());
	///
	/// soft_delete.delete();
	/// assert!(soft_delete.is_deleted());
	/// assert!(soft_delete.deleted_at.is_some());
	/// ```
	pub fn delete(&mut self) {
		self.deleted_at = Some(chrono::Utc::now());
	}
	/// Restores a soft-deleted record by clearing the deletion timestamp
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::model::SoftDelete;
	///
	/// let mut soft_delete = SoftDelete::new();
	/// soft_delete.delete();
	/// assert!(soft_delete.is_deleted());
	///
	/// soft_delete.restore();
	/// assert!(!soft_delete.is_deleted());
	/// assert!(soft_delete.deleted_at.is_none());
	/// ```
	pub fn restore(&mut self) {
		self.deleted_at = None;
	}

	/// Check if the record is soft-deleted
	pub fn is_deleted(&self) -> bool {
		self.deleted_at.is_some()
	}
}

impl Default for SoftDelete {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::Model;
	use crate::orm::fields::{BinaryField, CharField, Field};
	use crate::orm::inspection::FieldInfo;
	use crate::orm::{DatabaseStorageKind, DatabaseValue, FieldSelector, Manager};
	use reinhardt_core::macros::{ModelEnum, model};
	use rstest::rstest;
	use serde::{Deserialize, Serialize};
	use std::collections::HashMap;

	#[derive(ModelEnum, Clone, Debug, PartialEq, Serialize, Deserialize)]
	#[model_enum(repr = "string")]
	#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
	enum Status {
		#[model_enum(value = "queued")]
		Queued,
		#[model_enum(value = "running")]
		Running,
	}

	#[derive(ModelEnum, Clone, Debug, PartialEq, Serialize, Deserialize)]
	#[model_enum(repr = "i32")]
	enum Priority {
		#[model_enum(value = 10)]
		Low,
		#[model_enum(value = 20)]
		Normal,
	}

	#[model(app_label = "tests", table_name = "field_map_records")]
	#[derive(Clone, Debug, Serialize, Deserialize)]
	struct FieldMapRecord {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(max_length = 16)]
		status: Status,
		priority: Priority,
	}

	#[model(app_label = "tests", table_name = "decimal_primary_key_records")]
	#[derive(Clone, Debug, Serialize, Deserialize)]
	struct DecimalPrimaryKeyRecord {
		#[field(primary_key = true)]
		id: rust_decimal::Decimal,
	}

	#[model(app_label = "tests", table_name = "datetime_primary_key_records")]
	#[derive(Clone, Debug, Serialize, Deserialize)]
	struct DateTimePrimaryKeyRecord {
		#[field(primary_key = true)]
		id: chrono::DateTime<chrono::Utc>,
	}

	#[derive(Clone, Debug, Serialize, Deserialize)]
	struct LegacyTypedRecord {
		id: Option<i64>,
		external_id: uuid::Uuid,
		occurred_at: chrono::DateTime<chrono::Utc>,
	}

	#[derive(Clone, Debug)]
	struct LegacyTypedRecordFields;

	impl FieldSelector for LegacyTypedRecordFields {
		fn with_alias(self, _alias: &str) -> Self {
			self
		}
	}

	impl Model for LegacyTypedRecord {
		type PrimaryKey = i64;
		type Fields = LegacyTypedRecordFields;
		type Objects = Manager<Self>;

		fn table_name() -> &'static str {
			"legacy_typed_records"
		}

		fn primary_key(&self) -> Option<Self::PrimaryKey> {
			self.id
		}

		fn set_primary_key(&mut self, value: Self::PrimaryKey) {
			self.id = Some(value);
		}

		fn primary_key_field() -> &'static str {
			"id"
		}

		fn new_fields() -> Self::Fields {
			LegacyTypedRecordFields
		}

		fn field_metadata() -> Vec<FieldInfo> {
			["id", "external_id", "occurred_at"]
				.into_iter()
				.map(|name| FieldInfo {
					name: name.to_owned(),
					field_type: match name {
						"id" => "reinhardt.orm.models.BigIntegerField",
						"external_id" => "reinhardt.orm.models.UuidField",
						"occurred_at" => "reinhardt.orm.models.DateTimeField",
						_ => unreachable!(),
					}
					.to_owned(),
					storage_kind: None,
					domain: None,
					nullable: name == "id",
					primary_key: name == "id",
					unique: false,
					blank: false,
					editable: true,
					default: None,
					db_default: None,
					db_column: None,
					choices: None,
					attributes: HashMap::new(),
				})
				.collect()
		}
	}

	macro_rules! define_manual_primary_key_model {
		($name:ident, $key:ty, $table:literal) => {
			#[derive(Clone, Debug, Serialize, Deserialize)]
			struct $name {
				id: $key,
			}

			impl Model for $name {
				type PrimaryKey = $key;
				type Fields = LegacyTypedRecordFields;
				type Objects = Manager<Self>;

				fn table_name() -> &'static str {
					$table
				}

				fn primary_key(&self) -> Option<Self::PrimaryKey> {
					Some(self.id.clone())
				}

				fn set_primary_key(&mut self, value: Self::PrimaryKey) {
					self.id = value;
				}

				fn new_fields() -> Self::Fields {
					LegacyTypedRecordFields
				}
			}
		};
	}

	define_manual_primary_key_model!(ManualBooleanPrimaryKey, bool, "manual_boolean_keys");
	define_manual_primary_key_model!(ManualF32PrimaryKey, f32, "manual_f32_keys");
	define_manual_primary_key_model!(ManualF64PrimaryKey, f64, "manual_f64_keys");
	define_manual_primary_key_model!(
		ManualDateTimePrimaryKey,
		chrono::DateTime<chrono::Utc>,
		"manual_datetime_keys"
	);

	#[rstest]
	fn string_enum_database_value_survives_field_map_round_trip() {
		// Arrange
		let record = FieldMapRecord {
			id: None,
			status: Status::Queued,
			priority: Priority::Normal,
		};

		// Act
		let fields = record
			.encode_database_fields()
			.expect("model fields should encode");
		let database_value = fields
			.get("status")
			.cloned()
			.expect("status should be encoded");
		let decoded = FieldMapRecord::decode_database_field("status", database_value.clone())
			.expect("status should decode");
		let status: Status =
			serde_json::from_value(decoded).expect("decoded status should deserialize");

		// Assert
		assert_eq!(database_value, DatabaseValue::String("queued".to_owned()));
		assert_eq!(status, Status::Queued);
	}

	#[rstest]
	fn i32_enum_database_value_survives_field_map_round_trip() {
		// Arrange
		let record = FieldMapRecord {
			id: None,
			status: Status::Queued,
			priority: Priority::Normal,
		};

		// Act
		let fields = record
			.encode_database_fields()
			.expect("model fields should encode");
		let database_value = fields
			.get("priority")
			.cloned()
			.expect("priority should be encoded");
		let decoded = FieldMapRecord::decode_database_field("priority", database_value.clone())
			.expect("priority should decode");
		let priority: Priority =
			serde_json::from_value(decoded).expect("decoded priority should deserialize");

		// Assert
		assert_eq!(database_value, DatabaseValue::I32(20));
		assert_eq!(priority, Priority::Normal);
	}

	#[test]
	fn legacy_metadata_infers_uuid_and_datetime_database_values() {
		// Arrange
		let occurred_at = chrono::DateTime::parse_from_rfc3339("2026-07-18T12:00:00Z")
			.expect("timestamp should parse")
			.with_timezone(&chrono::Utc);
		let record = LegacyTypedRecord {
			id: Some(1),
			external_id: uuid::Uuid::nil(),
			occurred_at,
		};

		// Act
		let fields = record
			.encode_database_fields()
			.expect("legacy model fields should encode");

		// Assert
		assert_eq!(
			fields.get("external_id"),
			Some(&DatabaseValue::Uuid(uuid::Uuid::nil()))
		);
		assert_eq!(
			fields.get("occurred_at"),
			Some(&DatabaseValue::DateTime(occurred_at))
		);
	}

	#[rstest]
	fn legacy_metadata_infers_scalar_database_values() {
		let cases = [
			("BooleanField", "true", DatabaseValue::Bool(true)),
			("IntegerField", "7", DatabaseValue::I32(7)),
			(
				"BigIntegerField",
				"9007199254740993",
				DatabaseValue::I64(9007199254740993),
			),
			("FloatField", "1.25", DatabaseValue::F64(1.25)),
			(
				"DecimalField",
				"9007199254740993.123456789",
				DatabaseValue::Decimal("9007199254740993.123456789".parse().unwrap()),
			),
		];

		for (field_type, value, expected) in cases {
			let mut field = CharField::new(255);
			field.set_attributes_from_name("value");
			let mut info = FieldInfo::from_field(&field);
			info.field_type = format!("reinhardt.orm.models.{field_type}");

			let filter = super::filter_value_from_field(&info, value).unwrap();
			assert!(
				matches!(filter, crate::orm::query::FilterValue::Typed(Ok(actual)) if actual == expected)
			);
		}
	}

	#[rstest]
	fn datetime_route_values_accept_display_format() {
		let field = LegacyTypedRecord::field_metadata()
			.into_iter()
			.find(|field| field.name == "occurred_at")
			.expect("datetime metadata should exist");
		let filter = super::filter_value_from_field(&field, "2026-07-18 12:00:00 UTC")
			.expect("display-formatted datetime should parse");

		assert!(matches!(
			filter,
			crate::orm::query::FilterValue::Typed(Ok(DatabaseValue::DateTime(value)))
				if value == chrono::DateTime::parse_from_rfc3339("2026-07-18T12:00:00Z")
					.expect("expected datetime should parse")
					.with_timezone(&chrono::Utc)
		));
	}

	#[rstest]
	fn binary_route_values_decode_base64() {
		let mut field = BinaryField::new();
		field.set_attributes_from_name("payload");
		let mut info = FieldInfo::from_field(&field);
		info.storage_kind = Some(DatabaseStorageKind::Bytes);

		let filter = super::filter_value_from_field(&info, "AAH//w==")
			.expect("base64 binary route value should parse");

		let crate::orm::query::FilterValue::Typed(Ok(value)) = filter else {
			panic!("binary route value should produce a typed database value");
		};
		assert_eq!(value, DatabaseValue::Bytes(vec![0, 1, 255, 255]));
	}

	#[rstest]
	fn generated_datetime_primary_key_accepts_display_format() {
		let filter =
			DateTimePrimaryKeyRecord::primary_key_filter_value_from_str("2026-07-18 12:00:00 UTC")
				.expect("display-formatted datetime primary key should parse");

		assert!(matches!(
			filter,
			crate::orm::query::FilterValue::Timestamp(value)
				if value == chrono::DateTime::parse_from_rfc3339("2026-07-18T12:00:00Z")
					.expect("expected datetime should parse")
					.with_timezone(&chrono::Utc)
		));
	}

	#[rstest]
	fn manual_primary_keys_preserve_boolean_float_and_display_datetime_types() {
		assert!(matches!(
			ManualBooleanPrimaryKey::primary_key_filter_value(true),
			crate::orm::query::FilterValue::Boolean(true)
		));
		assert!(matches!(
			ManualF32PrimaryKey::primary_key_filter_value(1.25),
			crate::orm::query::FilterValue::Float(value) if (value - 1.25).abs() < f64::EPSILON
		));
		assert!(matches!(
			ManualF64PrimaryKey::primary_key_filter_value(2.5),
			crate::orm::query::FilterValue::Float(value) if (value - 2.5).abs() < f64::EPSILON
		));

		let filter =
			ManualDateTimePrimaryKey::primary_key_filter_value_from_str("2026-07-18 12:00:00 UTC")
				.unwrap();
		assert!(matches!(
			filter,
			crate::orm::query::FilterValue::Timestamp(value)
				if value == chrono::DateTime::parse_from_rfc3339("2026-07-18T12:00:00Z")
					.unwrap()
					.with_timezone(&chrono::Utc)
		));
	}

	#[rstest]
	fn generated_decimal_primary_key_preserves_route_precision() {
		let route_value = "9007199254740993.123456789";
		let filter = DecimalPrimaryKeyRecord::primary_key_filter_value_from_str(route_value)
			.expect("decimal primary key should parse");
		let expected: rust_decimal::Decimal = route_value.parse().expect("decimal should parse");

		assert!(matches!(
			filter,
			crate::orm::query::FilterValue::Typed(Ok(DatabaseValue::Decimal(value)))
				if value == expected
		));
	}
}
