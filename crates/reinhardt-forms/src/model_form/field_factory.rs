//! Native form-field construction from target-neutral model descriptors.

use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};
use reinhardt_core::model_form::{ModelFormFieldDescriptor, ModelFormFieldKind};

use crate::{
	BooleanField, CharField, DateField, DateTimeField, DecimalField, EmailField, FieldError,
	FieldResult, FloatField, FormField, IntegerField, JSONField, TimeField, URLField, UUIDField,
	Widget,
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

/// Creates the native form field described by generated model metadata.
pub(super) fn create_form_field(descriptor: &ModelFormFieldDescriptor) -> Box<dyn FormField> {
	let name = descriptor.name.to_owned();

	match descriptor.kind {
		ModelFormFieldKind::Text {
			max_length,
			multiline,
		} => {
			let mut field = CharField::new(name);
			field.required = descriptor.required;
			field.max_length = max_length;
			if multiline {
				field.widget = Widget::TextArea;
			}
			Box::new(field)
		}
		ModelFormFieldKind::Email { max_length } => {
			let mut field = EmailField::new(name);
			field.required = descriptor.required;
			field.max_length = max_length;
			Box::new(field)
		}
		ModelFormFieldKind::Url { max_length } => {
			let mut field = URLField::new(name);
			field.required = descriptor.required;
			field.max_length = max_length;
			Box::new(field)
		}
		ModelFormFieldKind::Integer { min, max } => {
			let mut field = IntegerField::new(name);
			field.required = descriptor.required;
			field.min_value = min;
			field.max_value = max;
			Box::new(field)
		}
		ModelFormFieldKind::Float => {
			let mut field = FloatField::new(name);
			field.required = descriptor.required;
			Box::new(field)
		}
		ModelFormFieldKind::Decimal => {
			let mut field = DecimalField::new(name);
			field.required = descriptor.required;
			Box::new(field)
		}
		ModelFormFieldKind::Boolean => {
			let mut field = BooleanField::new(name);
			field.required = descriptor.required;
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
		ModelFormFieldKind::Json => Box::new(JSONField::new(name).required(descriptor.required)),
	}
}
