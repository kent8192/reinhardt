use crate::orm::expressions::FieldRef;
use crate::orm::field_codec::{
	DatabaseField, DatabaseScalar, DatabaseValue, FieldCodecContext, FieldCodecError,
	IntoFieldValue,
};
use reinhardt_core::exception::{DatabaseErrorKind, Error, Result};
use std::marker::PhantomData;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TypedAssignment<M> {
	pub(crate) logical_name: &'static str,
	pub(crate) column_name: &'static str,
	pub(crate) value: DatabaseValue,
	pub(crate) marker: PhantomData<fn() -> M>,
}

impl<M> TypedAssignment<M> {
	pub(crate) fn new<T, V>(field: FieldRef<M, T>, value: V) -> Result<Self>
	where
		T: DatabaseField,
		V: IntoFieldValue<T>,
	{
		Ok(Self {
			logical_name: field.logical_name(),
			column_name: field.name(),
			value: value.into_field_value().map_err(field_codec_error)?,
			marker: PhantomData,
		})
	}
}

macro_rules! impl_unsigned_field_value {
	($($type:ty),+ $(,)?) => {
		$(
			impl IntoFieldValue<i64> for $type {
				fn into_field_value(self) -> std::result::Result<DatabaseValue, FieldCodecError> {
					i64::try_from(self)
						.map(DatabaseValue::I64)
						.map_err(|_| {
							FieldCodecError::Serialization(format!(
								"unsigned integer value {self} exceeds i64 database range"
							))
						})
				}
			}

			impl IntoFieldValue<Option<i64>> for $type {
				fn into_field_value(self) -> std::result::Result<DatabaseValue, FieldCodecError> {
					<Self as IntoFieldValue<i64>>::into_field_value(self)
				}
			}
		)+
	};
}

impl_unsigned_field_value!(u8, u16, u32, u64, usize);

macro_rules! impl_signed_i64_field_value {
	($($type:ty),+ $(,)?) => {
		$(
			impl IntoFieldValue<i64> for $type {
				fn into_field_value(self) -> std::result::Result<DatabaseValue, FieldCodecError> {
					Ok(DatabaseValue::I64(i64::from(self)))
				}
			}

			impl IntoFieldValue<Option<i64>> for $type {
				fn into_field_value(self) -> std::result::Result<DatabaseValue, FieldCodecError> {
					Ok(DatabaseValue::I64(i64::from(self)))
				}
			}
		)+
	};
}

impl_signed_i64_field_value!(i8, i16, i32);

/// Mutable typed view over values used by an upsert create branch.
pub struct UpsertCreate<'a, M> {
	pub(crate) lookup: &'a [TypedAssignment<M>],
	pub(crate) values: &'a mut Vec<TypedAssignment<M>>,
}

impl<M> UpsertCreate<'_, M> {
	/// Reads a typed value from the pending create values or immutable lookup.
	pub fn get<T>(&self, field: FieldRef<M, T>) -> Result<Option<T>>
	where
		T: DatabaseField,
	{
		let Some(assignment) = self
			.values
			.iter()
			.chain(self.lookup)
			.find(|assignment| assignment.logical_name == field.logical_name())
		else {
			return Ok(None);
		};
		let storage =
			T::Storage::from_database_value(assignment.value.clone()).map_err(field_codec_error)?;
		T::decode_database(
			storage,
			&FieldCodecContext::new(
				std::any::type_name::<M>(),
				field.logical_name(),
				field.name(),
			),
		)
		.map(Some)
		.map_err(field_codec_error)
	}

	/// Sets a typed create value while preserving immutable lookup fields.
	pub fn set<T, V>(&mut self, field: FieldRef<M, T>, value: V) -> Result<()>
	where
		T: DatabaseField,
		V: IntoFieldValue<T>,
	{
		if self
			.lookup
			.iter()
			.any(|assignment| assignment.logical_name == field.logical_name())
		{
			return Err(Error::Validation(format!(
				"upsert create hook cannot replace lookup field '{}'",
				field.logical_name()
			)));
		}

		let assignment = TypedAssignment::new(field, value)?;
		if let Some(existing) = self
			.values
			.iter_mut()
			.find(|existing| existing.logical_name == assignment.logical_name)
		{
			*existing = assignment;
		} else {
			self.values.push(assignment);
		}
		Ok(())
	}
}

/// Hook view for the create or update branch of an upsert operation.
pub enum UpsertWrite<'a, M> {
	/// Mutable typed values for a row that will be created.
	Create(UpsertCreate<'a, M>),
	/// Mutable model loaded for an existing-row update.
	Update(&'a mut M),
}

fn field_codec_error(error: FieldCodecError) -> Error {
	let kind = match &error {
		FieldCodecError::TypeMismatch { .. } | FieldCodecError::InvalidEnumValue { .. } => {
			DatabaseErrorKind::Type
		}
		FieldCodecError::Serialization(_) => DatabaseErrorKind::Serialization,
	};
	Error::database_with_source(
		kind,
		format!("typed upsert field codec failed: {error}"),
		error,
	)
}

#[cfg(test)]
mod tests {
	use super::{TypedAssignment, UpsertCreate, field_codec_error};
	use crate::orm::field_codec::{
		DatabaseArrayType, DatabaseStorageKind, DatabaseValue, FieldCodecContext, FieldCodecError,
		ModelEnumRepr, ModelEnumValue,
	};
	use chrono::{TimeZone, Utc};
	use reinhardt_core::macros::{ModelEnum, model};
	use rstest::*;
	use rust_decimal::Decimal;
	use serde::{Deserialize, Serialize};

	#[derive(ModelEnum, Clone, Debug, PartialEq, Serialize, Deserialize)]
	#[model_enum(repr = "string")]
	enum Status {
		#[model_enum(value = "queued")]
		Queued,
	}

	#[model(app_label = "tests", table_name = "assignment_models")]
	#[derive(Clone, Debug, Serialize, Deserialize)]
	struct AssignmentModel {
		#[field(primary_key = true)]
		id: Option<i64>,
		#[field(db_column = "i32_col")]
		i32: i32,
		#[field(db_column = "i64_col")]
		i64: i64,
		#[field(db_column = "u8_col")]
		u8: i64,
		#[field(db_column = "u16_col")]
		u16: i64,
		#[field(db_column = "u32_col")]
		u32: i64,
		#[field(db_column = "u64_col")]
		u64: i64,
		#[field(db_column = "f32_col")]
		f32: f32,
		#[field(db_column = "f64_col")]
		f64: f64,
		#[field(db_column = "decimal_col")]
		decimal: Decimal,
		#[field(db_column = "bool_col")]
		bool: bool,
		#[field(db_column = "string_col", max_length = 32)]
		string: String,
		#[field(db_column = "str_col", max_length = 32)]
		str: String,
		#[field(db_column = "bytes_col")]
		bytes: Vec<u8>,
		#[field(db_column = "uuid_col")]
		uuid: uuid::Uuid,
		#[field(db_column = "json_col")]
		json: serde_json::Value,
		#[field(db_column = "date_col")]
		date: chrono::NaiveDate,
		#[field(db_column = "time_col")]
		time: chrono::NaiveTime,
		#[field(db_column = "datetime_col")]
		datetime: chrono::DateTime<Utc>,
		#[field(db_column = "naive_datetime_col")]
		naive_datetime: chrono::NaiveDateTime,
		#[field(db_column = "status_col", max_length = 16)]
		status: Status,
		#[field(db_column = "array_col")]
		array: Vec<i32>,
		#[field(db_column = "some_col")]
		some: Option<i32>,
		#[field(db_column = "none_col")]
		none: Option<i32>,
		#[field(db_column = "tag_slug", max_length = 64)]
		slug: String,
		#[field(db_column = "tag_rank")]
		rank: i32,
		#[field(db_column = "is_active")]
		active: bool,
		sequence: i64,
	}

	#[rstest]
	fn typed_assignment_encodes_every_supported_value_family() {
		let uuid = uuid::Uuid::from_u128(1);
		let date = chrono::NaiveDate::from_ymd_opt(2026, 7, 29).expect("valid date");
		let time = chrono::NaiveTime::from_hms_opt(12, 30, 45).expect("valid time");
		let datetime = Utc
			.with_ymd_and_hms(2026, 7, 29, 12, 30, 45)
			.single()
			.expect("valid UTC datetime");
		let naive_datetime = date.and_time(time);

		let assignments = vec![
			TypedAssignment::new(AssignmentModel::field_i32(), -4_i32).expect("encode i32"),
			TypedAssignment::new(AssignmentModel::field_i64(), -5_i64).expect("encode i64"),
			TypedAssignment::new(AssignmentModel::field_u8(), 6_u8).expect("encode u8"),
			TypedAssignment::new(AssignmentModel::field_u16(), 7_u16).expect("encode u16"),
			TypedAssignment::new(AssignmentModel::field_u32(), 8_u32).expect("encode u32"),
			TypedAssignment::new(AssignmentModel::field_u64(), 9_u64).expect("encode u64"),
			TypedAssignment::new(AssignmentModel::field_f32(), 1.25_f32).expect("encode f32"),
			TypedAssignment::new(AssignmentModel::field_f64(), 2.5_f64).expect("encode f64"),
			TypedAssignment::new(AssignmentModel::field_decimal(), Decimal::new(1234, 2))
				.expect("encode decimal"),
			TypedAssignment::new(AssignmentModel::field_bool(), true).expect("encode bool"),
			TypedAssignment::new(AssignmentModel::field_string(), "owned".to_owned())
				.expect("encode String"),
			TypedAssignment::new(AssignmentModel::field_str(), "borrowed").expect("encode str"),
			TypedAssignment::new(AssignmentModel::field_bytes(), vec![0, 255])
				.expect("encode bytes"),
			TypedAssignment::new(AssignmentModel::field_uuid(), uuid).expect("encode UUID"),
			TypedAssignment::new(
				AssignmentModel::field_json(),
				serde_json::json!({"ready": true}),
			)
			.expect("encode JSON"),
			TypedAssignment::new(AssignmentModel::field_date(), date).expect("encode date"),
			TypedAssignment::new(AssignmentModel::field_time(), time).expect("encode time"),
			TypedAssignment::new(AssignmentModel::field_datetime(), datetime)
				.expect("encode UTC datetime"),
			TypedAssignment::new(AssignmentModel::field_naive_datetime(), naive_datetime)
				.expect("encode naive datetime"),
			TypedAssignment::new(AssignmentModel::field_status(), Status::Queued)
				.expect("encode model enum"),
			TypedAssignment::new(AssignmentModel::field_array(), vec![1, 2]).expect("encode array"),
			TypedAssignment::new(AssignmentModel::field_some(), Some(3_i32)).expect("encode Some"),
			TypedAssignment::new(AssignmentModel::field_none(), None::<i32>).expect("encode None"),
		];

		assert_eq!(
			assignments
				.into_iter()
				.map(|assignment| (
					assignment.logical_name,
					assignment.column_name,
					assignment.value,
				))
				.collect::<Vec<_>>(),
			vec![
				("i32", "i32_col", DatabaseValue::I32(-4)),
				("i64", "i64_col", DatabaseValue::I64(-5)),
				("u8", "u8_col", DatabaseValue::I64(6)),
				("u16", "u16_col", DatabaseValue::I64(7)),
				("u32", "u32_col", DatabaseValue::I64(8)),
				("u64", "u64_col", DatabaseValue::I64(9)),
				("f32", "f32_col", DatabaseValue::F32(1.25)),
				("f64", "f64_col", DatabaseValue::F64(2.5)),
				(
					"decimal",
					"decimal_col",
					DatabaseValue::Decimal(Decimal::new(1234, 2)),
				),
				("bool", "bool_col", DatabaseValue::Bool(true)),
				(
					"string",
					"string_col",
					DatabaseValue::String("owned".to_owned()),
				),
				(
					"str",
					"str_col",
					DatabaseValue::String("borrowed".to_owned()),
				),
				("bytes", "bytes_col", DatabaseValue::Bytes(vec![0, 255])),
				("uuid", "uuid_col", DatabaseValue::Uuid(uuid)),
				(
					"json",
					"json_col",
					DatabaseValue::Json(serde_json::json!({"ready": true})),
				),
				("date", "date_col", DatabaseValue::Date(date)),
				("time", "time_col", DatabaseValue::Time(time)),
				(
					"datetime",
					"datetime_col",
					DatabaseValue::DateTime(datetime),
				),
				(
					"naive_datetime",
					"naive_datetime_col",
					DatabaseValue::NaiveDateTime(naive_datetime),
				),
				(
					"status",
					"status_col",
					DatabaseValue::String("queued".to_owned()),
				),
				(
					"array",
					"array_col",
					DatabaseValue::Array {
						element_type: DatabaseArrayType::I32,
						values: vec![DatabaseValue::I32(1), DatabaseValue::I32(2)],
					},
				),
				("some", "some_col", DatabaseValue::I32(3)),
				("none", "none_col", DatabaseValue::Null),
			],
		);
	}

	#[rstest]
	fn create_view_reads_lookup_and_mutable_values() {
		let lookup = vec![
			TypedAssignment::new(AssignmentModel::field_slug(), "rust").expect("encode lookup"),
		];
		let mut values = vec![
			TypedAssignment::new(AssignmentModel::field_rank(), 7_i32)
				.expect("encode create value"),
		];
		let create = UpsertCreate {
			lookup: &lookup,
			values: &mut values,
		};

		assert_eq!(
			create
				.get(AssignmentModel::field_slug())
				.expect("decode lookup"),
			Some("rust".to_owned())
		);
		assert_eq!(
			create
				.get(AssignmentModel::field_rank())
				.expect("decode create value"),
			Some(7)
		);
		assert_eq!(
			create
				.get(AssignmentModel::field_active())
				.expect("read missing value"),
			None
		);
	}

	#[rstest]
	fn create_view_sets_mutable_values_and_rejects_lookup_fields() {
		let lookup = vec![
			TypedAssignment::new(AssignmentModel::field_slug(), "rust").expect("encode lookup"),
		];
		let mut values = vec![
			TypedAssignment::new(AssignmentModel::field_rank(), 7_i32)
				.expect("encode create value"),
		];
		let mut create = UpsertCreate {
			lookup: &lookup,
			values: &mut values,
		};

		create
			.set(AssignmentModel::field_rank(), 8_i32)
			.expect("replace create value");
		create
			.set(AssignmentModel::field_active(), true)
			.expect("append create value");
		let error = create
			.set(AssignmentModel::field_slug(), "other")
			.expect_err("lookup mutation must fail");

		assert_eq!(
			create
				.get(AssignmentModel::field_rank())
				.expect("decode replaced value"),
			Some(8)
		);
		assert_eq!(
			create
				.get(AssignmentModel::field_active())
				.expect("decode appended value"),
			Some(true)
		);
		match error {
			reinhardt_core::exception::Error::Validation(message) => assert_eq!(
				message,
				"upsert create hook cannot replace lookup field 'slug'"
			),
			other => panic!("expected validation error, got {other}"),
		}
	}

	#[rstest]
	fn unsigned_assignment_rejects_values_larger_than_i64() {
		let error = TypedAssignment::new(AssignmentModel::field_sequence(), u64::MAX)
			.expect_err("u64 overflow must fail");

		assert_eq!(
			error.database_kind(),
			Some(reinhardt_core::exception::DatabaseErrorKind::Serialization)
		);
		assert_eq!(
			error
				.database_error()
				.expect("structured database error")
				.message(),
			"typed upsert field codec failed: field serialization failed: unsigned integer value 18446744073709551615 exceeds i64 database range"
		);
	}

	#[rstest]
	#[case(
		FieldCodecError::TypeMismatch {
			expected: DatabaseStorageKind::I32,
			actual: DatabaseValue::String("wrong".to_owned()),
		},
		reinhardt_core::exception::DatabaseErrorKind::Type
	)]
	#[case(
		FieldCodecError::invalid_enum(
			FieldCodecContext::new("AssignmentModel", "status", "status_col"),
			ModelEnumRepr::String,
			ModelEnumValue::String("unknown".to_owned()),
		),
		reinhardt_core::exception::DatabaseErrorKind::Type
	)]
	#[case(
		FieldCodecError::Serialization("invalid payload".to_owned()),
		reinhardt_core::exception::DatabaseErrorKind::Serialization
	)]
	fn field_codec_errors_keep_their_database_error_kind(
		#[case] source: FieldCodecError,
		#[case] expected: reinhardt_core::exception::DatabaseErrorKind,
	) {
		let error = field_codec_error(source);

		assert_eq!(error.database_kind(), Some(expected));
	}
}
