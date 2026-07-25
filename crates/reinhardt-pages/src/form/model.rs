//! Target-neutral runtime state for model-backed forms.

use std::collections::HashMap;
use std::marker::PhantomData;

use reinhardt_core::model_form::{
	ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPayload, ModelFormPayloadError,
	ModelFormPolicy, ModelFormSchema,
};

/// Dynamic control state for a model-backed form.
pub struct ModelFormState<S, P>
where
	S: ModelFormSchema,
	P: ModelFormPolicy,
{
	values: HashMap<&'static str, serde_json::Value>,
	_schema: PhantomData<S>,
	_policy: PhantomData<P>,
}

impl<S, P> ModelFormState<S, P>
where
	S: ModelFormSchema,
	P: ModelFormPolicy,
{
	/// Creates empty model-form control state.
	pub fn new() -> Self {
		Self {
			values: HashMap::new(),
			_schema: PhantomData,
			_policy: PhantomData,
		}
	}

	/// Stores a control value after validating it against the generated schema.
	///
	/// # Errors
	///
	/// Returns a typed payload error when the field is unknown, forbidden by the
	/// form policy, or cannot be converted according to its model field kind.
	pub fn set_value(
		&mut self,
		field: &str,
		value: serde_json::Value,
	) -> Result<(), ModelFormPayloadError> {
		let descriptor = S::fields()
			.iter()
			.find(|descriptor| descriptor.name == field)
			.ok_or_else(|| ModelFormPayloadError::UnknownField {
				field: field.to_owned(),
			})?;
		if !P::allows(field) {
			return Err(ModelFormPayloadError::ForbiddenField {
				field: field.to_owned(),
			});
		}

		let converted = convert_control_value(descriptor, value)?;
		self.values.insert(descriptor.name, converted);
		Ok(())
	}

	/// Returns the converted value stored for a model field.
	pub fn value(&self, field: &str) -> Option<&serde_json::Value> {
		self.values.get(field)
	}

	/// Returns selected editable descriptors in generated schema order.
	pub fn selected_descriptors(&self) -> Vec<&'static ModelFormFieldDescriptor> {
		S::fields()
			.iter()
			.filter(|descriptor| descriptor.editable && P::allows(descriptor.name))
			.collect()
	}

	/// Builds the one typed payload sent to the configured server function.
	///
	/// # Errors
	///
	/// Returns the first typed error raised while applying a converted control
	/// value to the generated payload.
	pub fn build_payload<D>(&self) -> Result<D, ModelFormPayloadError>
	where
		D: Default + ModelFormPayload<P>,
	{
		let mut payload = D::default();
		for descriptor in self.selected_descriptors() {
			if let Some(value) = self.values.get(descriptor.name) {
				payload.set_json(descriptor.name, value.clone())?;
			}
		}
		Ok(payload)
	}
}

impl<S, P> Default for ModelFormState<S, P>
where
	S: ModelFormSchema,
	P: ModelFormPolicy,
{
	fn default() -> Self {
		Self::new()
	}
}

fn convert_control_value(
	descriptor: &ModelFormFieldDescriptor,
	value: serde_json::Value,
) -> Result<serde_json::Value, ModelFormPayloadError> {
	match descriptor.kind {
		ModelFormFieldKind::Text { max_length, .. }
		| ModelFormFieldKind::Email { max_length }
		| ModelFormFieldKind::Url { max_length } => {
			let text = expect_string(descriptor.name, value)?;
			if let Some(max_length) = max_length
				&& text.chars().count() > max_length
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must contain at most {max_length} characters"),
				));
			}
			Ok(serde_json::Value::String(text))
		}
		ModelFormFieldKind::Integer { min, max } => {
			let integer = match value {
				serde_json::Value::Number(number) => number
					.as_i64()
					.ok_or_else(|| invalid_value(descriptor.name, "expected a signed integer"))?,
				serde_json::Value::String(text) => text.parse::<i64>().map_err(|error| {
					invalid_value(descriptor.name, format!("invalid integer: {error}"))
				})?,
				_ => return Err(invalid_value(descriptor.name, "expected an integer")),
			};
			if let Some(min) = min
				&& integer < min
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must be greater than or equal to {min}"),
				));
			}
			if let Some(max) = max
				&& integer > max
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must be less than or equal to {max}"),
				));
			}
			Ok(serde_json::Value::from(integer))
		}
		ModelFormFieldKind::Float => {
			let float = match value {
				serde_json::Value::Number(number) => number
					.as_f64()
					.ok_or_else(|| invalid_value(descriptor.name, "expected a finite number"))?,
				serde_json::Value::String(text) => text.parse::<f64>().map_err(|error| {
					invalid_value(descriptor.name, format!("invalid number: {error}"))
				})?,
				_ => return Err(invalid_value(descriptor.name, "expected a number")),
			};
			let number = serde_json::Number::from_f64(float)
				.ok_or_else(|| invalid_value(descriptor.name, "expected a finite number"))?;
			Ok(serde_json::Value::Number(number))
		}
		ModelFormFieldKind::Decimal => match value {
			serde_json::Value::Number(number) => Ok(serde_json::Value::Number(number)),
			serde_json::Value::String(text) => {
				let parsed = serde_json::from_str::<serde_json::Value>(&text).map_err(|error| {
					invalid_value(descriptor.name, format!("invalid decimal: {error}"))
				})?;
				if !parsed.is_number() {
					return Err(invalid_value(descriptor.name, "expected a decimal number"));
				}
				Ok(serde_json::Value::String(text))
			}
			_ => Err(invalid_value(descriptor.name, "expected a decimal number")),
		},
		ModelFormFieldKind::Boolean => match value {
			serde_json::Value::Bool(value) => Ok(serde_json::Value::Bool(value)),
			serde_json::Value::String(text) => match text.as_str() {
				"true" => Ok(serde_json::Value::Bool(true)),
				"false" => Ok(serde_json::Value::Bool(false)),
				_ => Err(invalid_value(descriptor.name, "expected true or false")),
			},
			_ => Err(invalid_value(descriptor.name, "expected a boolean")),
		},
		ModelFormFieldKind::Date => {
			let text = expect_string(descriptor.name, value)?;
			if !is_date(&text) {
				return Err(invalid_value(descriptor.name, "expected YYYY-MM-DD"));
			}
			Ok(serde_json::Value::String(text))
		}
		ModelFormFieldKind::Time => {
			let text = expect_string(descriptor.name, value)?;
			if !is_time(&text) {
				return Err(invalid_value(descriptor.name, "expected HH:MM[:SS]"));
			}
			Ok(serde_json::Value::String(text))
		}
		ModelFormFieldKind::DateTime => {
			let text = expect_string(descriptor.name, value)?;
			let Some((date, time)) = text.split_once('T').or_else(|| text.split_once(' ')) else {
				return Err(invalid_value(descriptor.name, "expected a date and time"));
			};
			if !is_date(date) || !is_time(time) {
				return Err(invalid_value(
					descriptor.name,
					"expected YYYY-MM-DDTHH:MM[:SS]",
				));
			}
			Ok(serde_json::Value::String(text))
		}
		ModelFormFieldKind::Uuid => {
			let text = expect_string(descriptor.name, value)?;
			uuid::Uuid::parse_str(&text).map_err(|error| {
				invalid_value(descriptor.name, format!("invalid UUID: {error}"))
			})?;
			Ok(serde_json::Value::String(text))
		}
		ModelFormFieldKind::Json => match value {
			serde_json::Value::String(text) => serde_json::from_str(&text)
				.map_err(|error| invalid_value(descriptor.name, format!("invalid JSON: {error}"))),
			value => Ok(value),
		},
	}
}

fn expect_string(field: &str, value: serde_json::Value) -> Result<String, ModelFormPayloadError> {
	match value {
		serde_json::Value::String(text) => Ok(text),
		_ => Err(invalid_value(field, "expected a string")),
	}
}

fn invalid_value(field: &str, message: impl Into<String>) -> ModelFormPayloadError {
	ModelFormPayloadError::InvalidValue {
		field: field.to_owned(),
		message: message.into(),
	}
}

fn is_date(value: &str) -> bool {
	let bytes = value.as_bytes();
	bytes.len() == 10
		&& bytes[4] == b'-'
		&& bytes[7] == b'-'
		&& bytes
			.iter()
			.enumerate()
			.all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

fn is_time(value: &str) -> bool {
	let value = value
		.strip_suffix('Z')
		.unwrap_or(value)
		.split_once(['+', '-'])
		.map_or(value, |(time, _)| time);
	let bytes = value.as_bytes();
	matches!(bytes.len(), 5 | 8 | 12)
		&& bytes.get(2) == Some(&b':')
		&& (bytes.len() == 5 || bytes.get(5) == Some(&b':'))
		&& bytes
			.iter()
			.enumerate()
			.all(|(index, byte)| matches!(index, 2 | 5) || *byte == b'.' || byte.is_ascii_digit())
}
