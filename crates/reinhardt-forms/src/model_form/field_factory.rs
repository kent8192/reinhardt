//! Native form-field construction from target-neutral model descriptors.

use chrono::{DateTime, Datelike, NaiveDateTime, SecondsFormat, Utc};
use reinhardt_core::model_form::{ModelFormFieldDescriptor, ModelFormFieldKind};
use reinhardt_core::validators::{EmailValidator, UrlValidator, Validator};
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::{
	BooleanField, CharField, DateField, DateTimeField, DecimalField, FieldError, FieldResult,
	FloatField, FormField, IntegerField, JSONField, TimeField, UUIDField, Widget,
};

#[derive(Debug, Clone, Copy)]
enum ModelDateTimeKind {
	AwareUtc,
	Naive,
}

#[derive(Debug, Clone, Copy)]
enum ModelFormatKind {
	Email,
	Url,
}

struct ModelFormatField {
	inner: CharField,
	kind: ModelFormatKind,
}

impl FormField for ModelFormatField {
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
		let cleaned = self.inner.clean(value)?;
		let Some(value) = cleaned.as_str() else {
			return Err(FieldError::invalid(None, "Expected string"));
		};
		if value.is_empty() {
			return Ok(cleaned);
		}

		let valid = match self.kind {
			ModelFormatKind::Email => EmailValidator::new().validate(value).is_ok(),
			ModelFormatKind::Url => UrlValidator::new().validate(value).is_ok(),
		};
		if valid {
			Ok(cleaned)
		} else {
			let message = match self.kind {
				ModelFormatKind::Email => "Enter a valid email address",
				ModelFormatKind::Url => "Enter a valid URL",
			};
			Err(FieldError::Validation(message.to_owned()))
		}
	}
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

struct StoredFileField {
	name: String,
	required: bool,
	widget: Widget,
	trusted_value: Option<serde_json::Value>,
}

impl StoredFileField {
	fn new(name: String, required: bool, trusted_value: Option<serde_json::Value>) -> Self {
		Self {
			name,
			required,
			widget: Widget::FileInput,
			trusted_value,
		}
	}
}

impl FormField for StoredFileField {
	fn name(&self) -> &str {
		&self.name
	}

	fn label(&self) -> Option<&str> {
		None
	}

	fn required(&self) -> bool {
		self.required
	}

	fn help_text(&self) -> Option<&str> {
		None
	}

	fn widget(&self) -> &Widget {
		&self.widget
	}

	fn initial(&self) -> Option<&serde_json::Value> {
		None
	}

	fn clean(&self, value: Option<&serde_json::Value>) -> FieldResult<serde_json::Value> {
		let Some(value) = value else {
			return if self.required {
				Err(FieldError::required(None))
			} else {
				Ok(serde_json::Value::Null)
			};
		};
		if value.is_null() {
			return if self.required {
				Err(FieldError::required(None))
			} else {
				Ok(serde_json::Value::Null)
			};
		}

		let Some(object) = value.as_object() else {
			return Err(FieldError::invalid(
				None,
				"Expected storage-backed file reference",
			));
		};
		let has_path = object
			.get("path")
			.and_then(serde_json::Value::as_str)
			.is_some_and(|path| !path.is_empty());
		let has_storage = object
			.get("storage")
			.and_then(serde_json::Value::as_str)
			.is_some_and(|storage| !storage.is_empty());
		if !has_path || !has_storage {
			return Err(FieldError::invalid(
				None,
				"Expected storage-backed file reference",
			));
		}

		if self.trusted_value.as_ref() == Some(value) {
			Ok(value.clone())
		} else {
			Err(FieldError::invalid(
				None,
				"Stored file references must come from the existing instance",
			))
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
	use rstest::rstest;
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
	fn storage_field_kinds_use_storage_reference_file_inputs() {
		let existing = json!({"path": "uploads/report.pdf", "storage": "default"});
		let upload = json!({"filename": "report.pdf", "size": 1});
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
				trim: false,
			});

			assert_eq!(field.name(), name);
			assert!(field.required());
			assert!(matches!(field.widget(), Widget::FileInput));
			assert!(field.clean(Some(&existing)).is_err());
			let trusted = create_form_field_with_trusted_value(
				&ModelFormFieldDescriptor {
					name,
					kind,
					required: true,
					has_default: false,
					nullable: false,
					editable: true,
					generated_relation_id: false,
					trim: false,
				},
				Some(&existing),
			);
			assert_eq!(trusted.clean(Some(&existing)).unwrap(), existing);
			assert!(trusted.clean(Some(&upload)).is_err());
		}
	}

	#[rstest]
	fn generated_textual_fields_strip_only_when_declared() {
		for kind in [
			ModelFormFieldKind::Text {
				min_length: None,
				max_length: None,
				multiline: false,
			},
			ModelFormFieldKind::Email {
				min_length: None,
				max_length: None,
			},
			ModelFormFieldKind::Url {
				min_length: None,
				max_length: None,
			},
		] {
			let value = match kind {
				ModelFormFieldKind::Text { .. } => json!("  value  "),
				ModelFormFieldKind::Email { .. } => json!("  person@example.com  "),
				ModelFormFieldKind::Url { .. } => json!("  https://example.com  "),
				_ => unreachable!("only textual field kinds are included"),
			};
			let expected = match kind {
				ModelFormFieldKind::Text { .. } => json!("value"),
				ModelFormFieldKind::Email { .. } => json!("person@example.com"),
				ModelFormFieldKind::Url { .. } => json!("https://example.com"),
				_ => unreachable!("only textual field kinds are included"),
			};
			let expected_untrimmed_error = match kind {
				ModelFormFieldKind::Email { .. } => Some("Enter a valid email address"),
				ModelFormFieldKind::Url { .. } => Some("Enter a valid URL"),
				ModelFormFieldKind::Text { .. } => None,
				_ => unreachable!("only textual field kinds are included"),
			};
			let descriptor = |trim| ModelFormFieldDescriptor {
				name: "value",
				kind,
				required: true,
				has_default: false,
				nullable: false,
				editable: true,
				generated_relation_id: false,
				trim,
			};

			let untrimmed = create_form_field(&descriptor(false));
			let trimmed = create_form_field(&descriptor(true));

			if let Some(message) = expected_untrimmed_error {
				assert_eq!(
					untrimmed.clean(Some(&value)).unwrap_err().to_string(),
					message
				);
			} else {
				assert_eq!(untrimmed.clean(Some(&value)).unwrap(), value);
			}
			assert_eq!(trimmed.clean(Some(&value)).unwrap(), expected);
		}
	}

	#[rstest]
	fn generated_format_fields_use_target_neutral_validation_boundaries() {
		let descriptor = |kind, trim| ModelFormFieldDescriptor {
			name: "value",
			kind,
			required: true,
			has_default: false,
			nullable: false,
			editable: true,
			generated_relation_id: false,
			trim,
		};
		let email = create_form_field(&descriptor(
			ModelFormFieldKind::Email {
				min_length: None,
				max_length: None,
			},
			true,
		));
		let url = create_form_field(&descriptor(
			ModelFormFieldKind::Url {
				min_length: None,
				max_length: None,
			},
			true,
		));

		assert_eq!(
			email.clean(None).unwrap_err().to_string(),
			"This field is required."
		);
		assert_eq!(
			email.clean(Some(&json!("   "))).unwrap_err().to_string(),
			"This field is required."
		);
		assert_eq!(
			email
				.clean(Some(&json!("person@localhost")))
				.unwrap_err()
				.to_string(),
			"Enter a valid email address"
		);
		assert_eq!(
			url.clean(Some(&json!("https://example.com?query=value")))
				.unwrap(),
			json!("https://example.com?query=value")
		);
		assert_eq!(
			url.clean(Some(&json!("https://example.com:123456/")))
				.unwrap_err()
				.to_string(),
			"Enter a valid URL"
		);
	}
}

/// Creates the native form field described by generated model metadata.
#[cfg(test)]
pub(super) fn create_form_field(descriptor: &ModelFormFieldDescriptor) -> Box<dyn FormField> {
	create_form_field_with_trusted_value(descriptor, None)
}

pub(super) fn create_form_field_with_trusted_value(
	descriptor: &ModelFormFieldDescriptor,
	trusted_value: Option<&serde_json::Value>,
) -> Box<dyn FormField> {
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
			field.strip = descriptor.trim;
			if multiline {
				field.widget = Widget::TextArea;
			}
			Box::new(field)
		}
		ModelFormFieldKind::Email {
			min_length,
			max_length,
		} => {
			let mut field = CharField::new(name);
			field.required = descriptor.required;
			field.min_length = min_length;
			field.max_length = max_length;
			field.strip = descriptor.trim;
			field.widget = Widget::EmailInput;
			Box::new(ModelFormatField {
				inner: field,
				kind: ModelFormatKind::Email,
			})
		}
		ModelFormFieldKind::Url {
			min_length,
			max_length,
		} => {
			let mut field = CharField::new(name);
			field.required = descriptor.required;
			field.min_length = min_length;
			field.max_length = max_length;
			field.strip = descriptor.trim;
			Box::new(ModelFormatField {
				inner: field,
				kind: ModelFormatKind::Url,
			})
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
		ModelFormFieldKind::File | ModelFormFieldKind::Image => Box::new(StoredFileField::new(
			name,
			descriptor.required,
			trusted_value.cloned(),
		)),
	}
}
