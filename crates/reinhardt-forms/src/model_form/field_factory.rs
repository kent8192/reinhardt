//! Native form-field construction from target-neutral model descriptors.

use chrono::{DateTime, Datelike, NaiveDateTime, SecondsFormat, Utc};
use reinhardt_core::model_form::{ModelFormFieldDescriptor, ModelFormFieldKind};
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::{
	BooleanField, CharField, DateField, DateTimeField, DecimalField, EmailField, FieldError,
	FieldResult, FileField, FloatField, FormField, ImageField, IntegerField, JSONField, TimeField,
	URLField, UUIDField, Widget,
};

#[derive(Debug, Clone, Copy)]
enum ModelDateTimeKind {
	AwareUtc,
	Naive,
}

struct ModelDateTimeField {
	inner: DateTimeField,
	kind: ModelDateTimeKind,
}

struct ModelIntegerField {
	inner: IntegerField,
}

impl ModelIntegerField {
	fn new(name: String, required: bool, min: Option<i64>, max: Option<i64>) -> Self {
		let mut inner = IntegerField::new(name);
		inner.required = required;
		inner.min_value = min;
		inner.max_value = max;
		Self { inner }
	}

	fn clean_unsigned(&self, value: &serde_json::Value) -> FieldResult<serde_json::Value> {
		let number = match value {
			serde_json::Value::Number(number) => number.as_u64(),
			serde_json::Value::String(raw) => raw.trim().parse::<u64>().ok(),
			_ => None,
		};
		let Some(number) = number else {
			return self.inner.clean(Some(value));
		};

		if let Some(min) = self.inner.min_value
			&& min > 0
			&& number < min as u64
		{
			return Err(FieldError::Validation(format!(
				"Ensure this value is greater than or equal to {}",
				min
			)));
		}

		if let Some(max) = self.inner.max_value
			&& (max < 0 || number > max as u64)
		{
			return Err(FieldError::Validation(format!(
				"Ensure this value is less than or equal to {}",
				max
			)));
		}

		Ok(serde_json::Value::Number(number.into()))
	}
}

impl FormField for ModelIntegerField {
	fn name(&self) -> &str {
		self.inner.name()
	}

	fn label(&self) -> Option<&str> {
		self.inner.label()
	}

	fn required(&self) -> bool {
		self.inner.required
	}

	fn help_text(&self) -> Option<&str> {
		self.inner.help_text()
	}

	fn widget(&self) -> &Widget {
		self.inner.widget()
	}

	fn initial(&self) -> Option<&serde_json::Value> {
		self.inner.initial()
	}

	fn clean(&self, value: Option<&serde_json::Value>) -> FieldResult<serde_json::Value> {
		match value {
			Some(value) if value.as_i64().is_none() => self.clean_unsigned(value),
			_ => self.inner.clean(value),
		}
	}
}

struct ModelDecimalField {
	inner: DecimalField,
}

impl ModelDecimalField {
	fn new(name: String, required: bool, min: Option<&str>, max: Option<&str>) -> Self {
		let mut inner = DecimalField::new(name);
		inner.required = required;
		inner.min_decimal_value = min.and_then(|value| Decimal::from_str(value).ok());
		inner.max_decimal_value = max.and_then(|value| Decimal::from_str(value).ok());
		Self { inner }
	}
}

impl FormField for ModelDecimalField {
	fn name(&self) -> &str {
		self.inner.name()
	}

	fn label(&self) -> Option<&str> {
		self.inner.label()
	}

	fn required(&self) -> bool {
		self.inner.required
	}

	fn help_text(&self) -> Option<&str> {
		self.inner.help_text.as_deref()
	}

	fn widget(&self) -> &Widget {
		self.inner.widget()
	}

	fn initial(&self) -> Option<&serde_json::Value> {
		self.inner.initial.as_ref()
	}

	fn clean(&self, value: Option<&serde_json::Value>) -> FieldResult<serde_json::Value> {
		let cleaned = self.inner.clean(value)?;
		let Some(value) = value else {
			return Ok(cleaned);
		};

		match value {
			serde_json::Value::String(raw) if !raw.trim().is_empty() => {
				Ok(serde_json::Value::String(raw.trim().to_owned()))
			}
			serde_json::Value::Number(number) => Ok(serde_json::Value::String(number.to_string())),
			_ => Ok(cleaned),
		}
	}
}

struct ModelJsonField {
	inner: JSONField,
}

impl ModelJsonField {
	fn new(name: String, required: bool) -> Self {
		Self {
			inner: JSONField::new(name).required(required),
		}
	}
}

impl FormField for ModelJsonField {
	fn name(&self) -> &str {
		self.inner.name()
	}

	fn label(&self) -> Option<&str> {
		self.inner.label()
	}

	fn required(&self) -> bool {
		self.inner.required
	}

	fn help_text(&self) -> Option<&str> {
		if self.inner.help_text.is_empty() {
			None
		} else {
			Some(&self.inner.help_text)
		}
	}

	fn widget(&self) -> &Widget {
		self.inner.widget()
	}

	fn initial(&self) -> Option<&serde_json::Value> {
		self.inner.initial.as_ref()
	}

	fn clean(&self, value: Option<&serde_json::Value>) -> FieldResult<serde_json::Value> {
		match value {
			Some(
				value @ (serde_json::Value::Array(_)
				| serde_json::Value::Object(_)
				| serde_json::Value::Bool(_)
				| serde_json::Value::Number(_)
				| serde_json::Value::String(_)
				| serde_json::Value::Null),
			) => {
				let serialized = serde_json::to_string(value)
					.map_err(|error| FieldError::Validation(error.to_string()))?;
				self.inner
					.clean(Some(&serde_json::Value::String(serialized)))
			}
			_ => self.inner.clean(value),
		}
	}
}

impl ModelDateTimeField {
	fn new(name: String, required: bool, kind: ModelDateTimeKind) -> Self {
		let mut inner = DateTimeField::new(name);
		inner.required = required;
		Self { inner, kind }
	}

	fn normalize_parsed(&self, datetime: NaiveDateTime) -> serde_json::Value {
		match self.kind {
			ModelDateTimeKind::AwareUtc => serde_json::Value::String(
				datetime
					.and_utc()
					.to_rfc3339_opts(SecondsFormat::AutoSi, true),
			),
			ModelDateTimeKind::Naive => {
				serde_json::Value::String(datetime.format("%Y-%m-%dT%H:%M:%S%.f").to_string())
			}
		}
	}

	fn validate_year(year: i32) -> FieldResult<()> {
		if !(1000..=9999).contains(&year) {
			return Err(FieldError::Validation(
				"Enter a year between 1000 and 9999".to_owned(),
			));
		}
		Ok(())
	}
}

impl FormField for ModelDateTimeField {
	fn name(&self) -> &str {
		self.inner.name()
	}

	fn label(&self) -> Option<&str> {
		self.inner.label()
	}

	fn required(&self) -> bool {
		self.inner.required()
	}

	fn help_text(&self) -> Option<&str> {
		self.inner.help_text()
	}

	fn widget(&self) -> &Widget {
		self.inner.widget()
	}

	fn initial(&self) -> Option<&serde_json::Value> {
		self.inner.initial()
	}

	fn clean(&self, value: Option<&serde_json::Value>) -> FieldResult<serde_json::Value> {
		if let Some(serde_json::Value::String(raw)) = value {
			let input = raw.trim();
			if !input.is_empty() {
				match self.kind {
					ModelDateTimeKind::AwareUtc => {
						if let Ok(datetime) = DateTime::parse_from_rfc3339(input) {
							Self::validate_year(datetime.year())?;
							return Ok(serde_json::Value::String(
								datetime
									.with_timezone(&Utc)
									.to_rfc3339_opts(SecondsFormat::AutoSi, true),
							));
						}
					}
					ModelDateTimeKind::Naive => {
						if let Ok(datetime) =
							NaiveDateTime::parse_from_str(input, "%Y-%m-%dT%H:%M:%S%.f")
						{
							Self::validate_year(datetime.year())?;
							return Ok(self.normalize_parsed(datetime));
						}
					}
				}
			}
		}

		match self.inner.clean(value)? {
			serde_json::Value::String(cleaned) => {
				let datetime = NaiveDateTime::parse_from_str(&cleaned, "%Y-%m-%d %H:%M:%S")
					.map_err(|_| FieldError::Validation("Enter a valid date/time".to_string()))?;
				Ok(self.normalize_parsed(datetime))
			}
			cleaned => Ok(cleaned),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn integer_field_rejects_unsigned_text_below_minimum() {
		let field = ModelIntegerField::new("quantity".to_owned(), true, Some(10), None);

		assert!(field.clean(Some(&json!("5"))).is_err());
		assert_eq!(field.clean(Some(&json!("10"))).unwrap(), json!(10));
	}

	#[test]
	fn datetime_field_rejects_out_of_range_years_in_iso_fast_paths() {
		let aware =
			ModelDateTimeField::new("aware_at".to_owned(), true, ModelDateTimeKind::AwareUtc);
		let naive = ModelDateTimeField::new("naive_at".to_owned(), true, ModelDateTimeKind::Naive);

		assert!(aware.clean(Some(&json!("0025-01-15T14:30:00Z"))).is_err());
		assert!(naive.clean(Some(&json!("0025-01-15T14:30:00"))).is_err());
	}

	#[test]
	fn storage_field_kinds_use_required_file_inputs() {
		for (name, kind) in [
			("document", ModelFormFieldKind::File),
			("avatar", ModelFormFieldKind::Image),
		] {
			let field = create_form_field(&ModelFormFieldDescriptor {
				name,
				kind,
				required: true,
				has_default: false,
				nullable: false,
				editable: true,
				generated_relation_id: false,
			});

			assert_eq!(field.name(), name);
			assert!(field.required());
			assert!(matches!(field.widget(), Widget::FileInput));
		}
	}
}

/// Creates the native form field described by generated model metadata.
pub(super) fn create_form_field(descriptor: &ModelFormFieldDescriptor) -> Box<dyn FormField> {
	let name = descriptor.name.to_owned();

	match descriptor.kind {
		ModelFormFieldKind::Text {
			min_length,
			max_length,
			multiline,
		} => {
			let mut field = CharField::new(name);
			field.required = descriptor.required;
			field.min_length = min_length;
			field.max_length = max_length;
			if multiline {
				field.widget = Widget::TextArea;
			}
			Box::new(field)
		}
		ModelFormFieldKind::Email {
			min_length,
			max_length,
		} => {
			let mut field = EmailField::new(name);
			field.required = descriptor.required;
			field.min_length = min_length;
			field.max_length = max_length;
			Box::new(field)
		}
		ModelFormFieldKind::Url {
			min_length,
			max_length,
		} => {
			let mut field = URLField::new(name);
			field.required = descriptor.required;
			field.min_length = min_length;
			field.max_length = max_length;
			Box::new(field)
		}
		ModelFormFieldKind::Integer { min, max } => {
			Box::new(ModelIntegerField::new(name, descriptor.required, min, max))
		}
		ModelFormFieldKind::Float { min, max } => {
			let mut field = FloatField::new(name);
			field.required = descriptor.required;
			field.min_value = min;
			field.max_value = max;
			Box::new(field)
		}
		ModelFormFieldKind::Decimal { min, max } => {
			Box::new(ModelDecimalField::new(name, descriptor.required, min, max))
		}
		ModelFormFieldKind::Boolean => {
			let mut field = BooleanField::new(name);
			// A model boolean is a value field: `false` is valid even when the
			// model field itself is required. BooleanField::required is reserved
			// for explicit consent checkboxes that must be true.
			field.required = false;
			Box::new(field)
		}
		ModelFormFieldKind::Date => {
			let mut field = DateField::new(name);
			field.required = descriptor.required;
			Box::new(field)
		}
		ModelFormFieldKind::Time => {
			let mut field = TimeField::new(name);
			field.required = descriptor.required;
			Box::new(field)
		}
		ModelFormFieldKind::DateTime => Box::new(ModelDateTimeField::new(
			name,
			descriptor.required,
			ModelDateTimeKind::AwareUtc,
		)),
		ModelFormFieldKind::NaiveDateTime => Box::new(ModelDateTimeField::new(
			name,
			descriptor.required,
			ModelDateTimeKind::Naive,
		)),
		ModelFormFieldKind::Uuid => Box::new(UUIDField::new(name).required(descriptor.required)),
		ModelFormFieldKind::Json => Box::new(ModelJsonField::new(name, descriptor.required)),
		ModelFormFieldKind::File => {
			let mut field = FileField::new(name);
			field.required = descriptor.required;
			Box::new(field)
		}
		ModelFormFieldKind::Image => {
			let mut field = ImageField::new(name);
			field.required = descriptor.required;
			Box::new(field)
		}
	}
}
