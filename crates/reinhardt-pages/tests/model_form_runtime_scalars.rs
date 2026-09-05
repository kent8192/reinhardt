//! Typed model-form runtime values work without optional Pages features on both targets.

use reinhardt_core::model_form::{
	AllEditableModelFields, ModelFormContract, ModelFormContractField, ModelFormContractSchema,
	ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPayload, ModelFormPayloadError,
	NativeModelFormPayload,
};
use reinhardt_pages::server_fn::{ServerFnError, server_fn};
use reinhardt_pages::{form, use_form};
use std::collections::HashMap;

struct ScalarContract;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ScalarField(&'static str);

impl ModelFormContractField for ScalarField {
	fn name(self) -> &'static str {
		self.0
	}
}

const fn scalar(
	name: &'static str,
	kind: ModelFormFieldKind,
	nullable: bool,
) -> ModelFormFieldDescriptor {
	ModelFormFieldDescriptor {
		name,
		kind,
		required: !nullable,
		has_default: false,
		nullable,
		editable: true,
		generated_relation_id: false,
	}
}

impl ModelFormContractSchema for ScalarContract {
	fn contract_fields() -> &'static [ModelFormFieldDescriptor] {
		const FIELDS: [ModelFormFieldDescriptor; 10] = [
			scalar("uuid", ModelFormFieldKind::Uuid, false),
			scalar("optional_uuid", ModelFormFieldKind::Uuid, true),
			scalar("date", ModelFormFieldKind::Date, false),
			scalar("optional_date", ModelFormFieldKind::Date, true),
			scalar("time", ModelFormFieldKind::Time, false),
			scalar("optional_time", ModelFormFieldKind::Time, true),
			scalar("naive_at", ModelFormFieldKind::NaiveDateTime, false),
			scalar("optional_naive_at", ModelFormFieldKind::NaiveDateTime, true),
			scalar("aware_at", ModelFormFieldKind::DateTime, false),
			scalar("optional_aware_at", ModelFormFieldKind::DateTime, true),
		];
		&FIELDS
	}
}

#[derive(Debug, Default, serde::Deserialize, serde::Serialize)]
struct ScalarData(HashMap<String, serde_json::Value>);

impl ModelFormPayload<AllEditableModelFields> for ScalarData {
	fn supplied_fields(&self) -> Vec<&'static str> {
		ScalarContract::contract_fields()
			.iter()
			.filter(|descriptor| self.0.contains_key(descriptor.name))
			.map(|descriptor| descriptor.name)
			.collect()
	}

	fn forbidden_fields(&self) -> &[&'static str] {
		&[]
	}

	fn get_json(&self, field: &str) -> Option<serde_json::Value> {
		self.0.get(field).cloned()
	}

	fn set_json(
		&mut self,
		field: &str,
		value: serde_json::Value,
	) -> Result<(), ModelFormPayloadError> {
		self.0.insert(field.to_owned(), value);
		Ok(())
	}
}

impl NativeModelFormPayload for ScalarData {
	fn from_native_form_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
		serde_json::from_value(value)
	}
}

impl ModelFormContract for ScalarContract {
	type Data = ScalarData;
	type Schema = Self;
	type Field = ScalarField;
	type Policy = AllEditableModelFields;

	fn fields() -> &'static [Self::Field] {
		&[
			ScalarField("uuid"),
			ScalarField("optional_uuid"),
			ScalarField("date"),
			ScalarField("optional_date"),
			ScalarField("time"),
			ScalarField("optional_time"),
			ScalarField("naive_at"),
			ScalarField("optional_naive_at"),
			ScalarField("aware_at"),
			ScalarField("optional_aware_at"),
		]
	}
}

#[server_fn(model_form = true)]
async fn save_scalars(payload: ScalarData) -> Result<ScalarData, ServerFnError> {
	Ok(payload)
}

#[cfg_attr(native, test)]
#[cfg_attr(wasm, wasm_bindgen_test::wasm_bindgen_test)]
fn named_model_form_runtime_accepts_uuid_and_chrono_values() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		// Arrange
		let form = form! {
			name: ScalarForm,
			model_form: ScalarContract,
			server_fn: save_scalars,
		};
		let runtime = use_form(&form).build();
		let uuid = uuid::Uuid::from_u128(42);
		let date = chrono::NaiveDate::from_ymd_opt(2026, 9, 5).unwrap();
		let time = chrono::NaiveTime::from_hms_nano_opt(12, 34, 56, 123_456_789).unwrap();
		let naive_at = date.and_time(time);
		let aware_at = naive_at.and_utc();

		macro_rules! assert_scalar {
			($field:literal, $type:ty, $value:expr, $expected:literal) => {{
				// Act
				let optional_field = concat!("optional_", $field);
				runtime.set_value(ScalarField($field), $value);
				runtime.set_value(ScalarField(optional_field), Some($value));
				let payload = form.data().unwrap();

				// Assert
				assert_eq!(payload.get_json($field), Some(serde_json::json!($expected)));
				assert_eq!(payload.get_json(optional_field), payload.get_json($field));
				runtime.set_value(ScalarField(optional_field), None::<$type>);
				assert_eq!(
					form.data().unwrap().get_json(optional_field),
					Some(serde_json::Value::Null)
				);
			}};
		}

		assert_scalar!(
			"uuid",
			uuid::Uuid,
			uuid,
			"00000000-0000-0000-0000-00000000002a"
		);
		assert_scalar!("date", chrono::NaiveDate, date, "2026-09-05");
		assert_scalar!("time", chrono::NaiveTime, time, "12:34:56.123456789");
		assert_scalar!(
			"naive_at",
			chrono::NaiveDateTime,
			naive_at,
			"2026-09-05T12:34:56.123456789"
		);
		assert_scalar!(
			"aware_at",
			chrono::DateTime<chrono::Utc>,
			aware_at,
			"2026-09-05T12:34:56.123456789Z"
		);
	});
}
