#![allow(dead_code)] // Shared fixtures deliberately exercise individual aggregate contracts.

use std::borrow::Cow;

use reinhardt_db::orm::{
	DatabaseField, FieldCodecContext, FieldCodecError, FieldSelector, Manager, Model, ModelEnum,
	ModelEnumRepr, ModelEnumValueRef, NumericAggregateField, RelationJoinKind,
	RelationMultiplicity, RelationPath, RelationStep,
	expressions::{FieldRef, GeneratedModelField},
	relations::GeneratedRelationPath,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelRecord {
	pub id: Option<i64>,
}

#[derive(Clone)]
pub struct ModelRecordFields;

impl FieldSelector for ModelRecordFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

impl Model for ModelRecord {
	type PrimaryKey = i64;
	type Fields = ModelRecordFields;
	type Objects = Manager<Self>;

	fn table_name() -> &'static str {
		"model_records"
	}

	fn new_fields() -> Self::Fields {
		ModelRecordFields
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RelatedRecord {
	pub id: Option<i64>,
}

#[derive(Clone)]
pub struct RelatedRecordFields;

impl FieldSelector for RelatedRecordFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

impl Model for RelatedRecord {
	type PrimaryKey = i64;
	type Fields = RelatedRecordFields;
	type Objects = Manager<Self>;

	fn table_name() -> &'static str {
		"related_records"
	}

	fn new_fields() -> Self::Fields {
		RelatedRecordFields
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CustomAmount(pub i64);

impl DatabaseField for CustomAmount {
	type Storage = i64;

	fn encode_database(&self) -> Result<Self::Storage, FieldCodecError> {
		Ok(self.0)
	}

	fn decode_database(
		value: Self::Storage,
		_context: &FieldCodecContext,
	) -> Result<Self, FieldCodecError> {
		Ok(Self(value))
	}
}

impl NumericAggregateField for CustomAmount {}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Status(pub i64);

impl DatabaseField for Status {
	type Storage = i64;

	fn encode_database(&self) -> Result<Self::Storage, FieldCodecError> {
		Ok(self.0)
	}

	fn decode_database(
		value: Self::Storage,
		_context: &FieldCodecContext,
	) -> Result<Self, FieldCodecError> {
		Ok(Self(value))
	}
}

impl ModelEnum for Status {
	const REPR: ModelEnumRepr = ModelEnumRepr::I32;
	const VALUES: &'static [ModelEnumValueRef] = &[ModelEnumValueRef::I32(0)];
}

impl ModelRecord {
	pub fn field_i32() -> FieldRef<Self, i32, GeneratedModelField> {
		// SAFETY: the fixture declares an i32-backed persisted column named value_i32.
		unsafe { FieldRef::from_generated_model_field_with_names("value_i32", "value_i32") }
	}

	pub fn field_i64() -> FieldRef<Self, i64, GeneratedModelField> {
		// SAFETY: the fixture declares an i64-backed persisted column named value_i64.
		unsafe { FieldRef::from_generated_model_field_with_names("value_i64", "value_i64") }
	}

	pub fn field_f32() -> FieldRef<Self, f32, GeneratedModelField> {
		// SAFETY: the fixture declares an f32-backed persisted column named value_f32.
		unsafe { FieldRef::from_generated_model_field_with_names("value_f32", "value_f32") }
	}

	pub fn field_f64() -> FieldRef<Self, f64, GeneratedModelField> {
		// SAFETY: the fixture declares an f64-backed persisted column named value_f64.
		unsafe { FieldRef::from_generated_model_field_with_names("value_f64", "value_f64") }
	}

	pub fn field_decimal() -> FieldRef<Self, rust_decimal::Decimal, GeneratedModelField> {
		// SAFETY: the fixture declares a Decimal-backed persisted column named value_decimal.
		unsafe { FieldRef::from_generated_model_field_with_names("value_decimal", "value_decimal") }
	}

	pub fn field_optional_i64() -> FieldRef<Self, Option<i64>, GeneratedModelField> {
		// SAFETY: the fixture declares a nullable i64-backed persisted column named optional_i64.
		unsafe { FieldRef::from_generated_model_field_with_names("optional_i64", "optional_i64") }
	}

	pub fn field_name() -> FieldRef<Self, String, GeneratedModelField> {
		// SAFETY: the fixture declares a String-backed persisted column named name.
		unsafe { FieldRef::from_generated_model_field_with_names("name", "name") }
	}

	pub fn field_status() -> FieldRef<Self, Status, GeneratedModelField> {
		// SAFETY: the fixture declares a Status-backed persisted column named status.
		unsafe { FieldRef::from_generated_model_field_with_names("status", "status") }
	}

	pub fn field_uuid() -> FieldRef<Self, uuid::Uuid, GeneratedModelField> {
		// SAFETY: the fixture declares a UUID-backed persisted column named value_uuid.
		unsafe { FieldRef::from_generated_model_field_with_names("value_uuid", "value_uuid") }
	}

	pub fn field_date() -> FieldRef<Self, chrono::NaiveDate, GeneratedModelField> {
		// SAFETY: the fixture declares a date-backed persisted column named value_date.
		unsafe { FieldRef::from_generated_model_field_with_names("value_date", "value_date") }
	}

	pub fn field_time() -> FieldRef<Self, chrono::NaiveTime, GeneratedModelField> {
		// SAFETY: the fixture declares a time-backed persisted column named value_time.
		unsafe { FieldRef::from_generated_model_field_with_names("value_time", "value_time") }
	}

	pub fn field_datetime() -> FieldRef<Self, chrono::DateTime<chrono::Utc>, GeneratedModelField> {
		// SAFETY: the fixture declares a UTC datetime-backed persisted column named value_datetime.
		unsafe {
			FieldRef::from_generated_model_field_with_names("value_datetime", "value_datetime")
		}
	}

	pub fn field_naive_datetime() -> FieldRef<Self, chrono::NaiveDateTime, GeneratedModelField> {
		// SAFETY: the fixture declares a naive datetime-backed persisted column named value_naive_datetime.
		unsafe {
			FieldRef::from_generated_model_field_with_names(
				"value_naive_datetime",
				"value_naive_datetime",
			)
		}
	}

	pub fn field_custom_amount() -> FieldRef<Self, CustomAmount, GeneratedModelField> {
		// SAFETY: the fixture declares a CustomAmount-backed persisted column named custom_amount.
		unsafe { FieldRef::from_generated_model_field_with_names("custom_amount", "custom_amount") }
	}

	pub fn rel_related() -> RelationPath<Self, RelatedRecord, GeneratedRelationPath> {
		// SAFETY: the fixture relation is generated from the static model relation metadata below.
		unsafe {
			RelationPath::from_generated_steps(vec![RelationStep {
				name: Cow::Borrowed("related"),
				source_table: Cow::Borrowed("model_records"),
				target_table: Cow::Borrowed("related_records"),
				source_column: Cow::Borrowed("related_id"),
				target_column: Cow::Borrowed("id"),
				default_join_kind: RelationJoinKind::Inner,
				multiplicity: RelationMultiplicity::Single,
			}])
		}
	}
}

impl RelatedRecord {
	pub fn field_i64() -> FieldRef<Self, i64, GeneratedModelField> {
		// SAFETY: the fixture declares an i64-backed persisted column named value_i64.
		unsafe { FieldRef::from_generated_model_field_with_names("value_i64", "value_i64") }
	}

	pub fn field_optional_i64() -> FieldRef<Self, Option<i64>, GeneratedModelField> {
		// SAFETY: the fixture declares a nullable i64-backed persisted column named optional_i64.
		unsafe { FieldRef::from_generated_model_field_with_names("optional_i64", "optional_i64") }
	}
}
