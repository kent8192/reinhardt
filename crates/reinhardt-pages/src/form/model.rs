//! Target-neutral runtime state for model-backed forms.

use regex::Regex;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::LazyLock;

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
		let mut values = HashMap::new();
		for descriptor in S::fields() {
			if descriptor.editable
				&& P::allows(descriptor.name)
				&& !descriptor.nullable
				&& !descriptor.has_default
				&& matches!(descriptor.kind, ModelFormFieldKind::Boolean)
			{
				values.insert(descriptor.name, serde_json::Value::Bool(false));
			}
		}
		Self {
			values,
			_schema: PhantomData,
			_policy: PhantomData,
		}
	}

	/// Stores a control value after validating it against the generated schema.
	/// An empty string clears a nullable field and removes other optional values.
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
		if !descriptor.required
			&& matches!(&value, serde_json::Value::String(text) if text.is_empty())
		{
			if descriptor.nullable {
				if descriptor.has_default && !self.values.contains_key(descriptor.name) {
					return Ok(());
				}
				self.values.insert(descriptor.name, serde_json::Value::Null);
				return Ok(());
			}
			if !matches!(
				descriptor.kind,
				ModelFormFieldKind::Text { .. }
					| ModelFormFieldKind::Email { .. }
					| ModelFormFieldKind::Url { .. }
			) {
				self.values.remove(descriptor.name);
				return Ok(());
			}
			if descriptor.has_default {
				self.values.remove(descriptor.name);
				return Ok(());
			}
		}

		match convert_control_value(descriptor, value) {
			Ok(converted) => {
				self.values.insert(descriptor.name, converted);
				Ok(())
			}
			Err(error) => {
				self.values.remove(descriptor.name);
				Err(error)
			}
		}
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

	/// Clears every value that belongs to the active form policy.
	pub fn clear_selected_values(&mut self) {
		self.values.retain(|field, _| {
			!S::fields().iter().any(|descriptor| {
				descriptor.name == *field && descriptor.editable && P::allows(field)
			})
		});
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

	/// Builds a payload using a nameable policy while retaining this form's
	/// field-selection policy for the values copied into it.
	pub fn build_payload_for<D, Q>(&self) -> Result<D, ModelFormPayloadError>
	where
		D: Default + ModelFormPayload<Q>,
		Q: ModelFormPolicy,
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
		ModelFormFieldKind::Text {
			min_length,
			max_length,
			..
		} => {
			let text = expect_string(descriptor.name, value)?.trim().to_owned();
			if text.is_empty() && !descriptor.required {
				return Ok(serde_json::Value::String(text));
			}
			if let Some(min_length) = min_length
				&& text.chars().count() < min_length
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must contain at least {min_length} characters"),
				));
			}
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
		ModelFormFieldKind::Email {
			min_length,
			max_length,
		}
		| ModelFormFieldKind::Url {
			min_length,
			max_length,
		} => {
			let text = expect_string(descriptor.name, value)?.trim().to_owned();
			if text.is_empty() && !descriptor.required {
				return Ok(serde_json::Value::String(text));
			}
			if let Some(min_length) = min_length
				&& text.chars().count() < min_length
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must contain at least {min_length} characters"),
				));
			}
			if let Some(max_length) = max_length
				&& text.chars().count() > max_length
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must contain at most {max_length} characters"),
				));
			}
			let is_valid = match descriptor.kind {
				ModelFormFieldKind::Email { .. } => is_email(&text),
				ModelFormFieldKind::Url { .. } => is_url(&text),
				_ => unreachable!("email and URL fields are handled by this conversion branch"),
			};
			if !is_valid {
				return Err(invalid_value(descriptor.name, "has an invalid format"));
			}
			Ok(serde_json::Value::String(text))
		}
		ModelFormFieldKind::Integer { min, max } => {
			let number = match value {
				serde_json::Value::Number(number)
					if number.as_i64().is_some() || number.as_u64().is_some() =>
				{
					number
				}
				serde_json::Value::Number(_) => {
					return Err(invalid_value(descriptor.name, "expected an integer"));
				}
				serde_json::Value::String(text) => match text.parse::<i64>() {
					Ok(integer) => serde_json::Number::from(integer),
					Err(signed_error) => match text.parse::<u64>() {
						Ok(integer) => serde_json::Number::from(integer),
						Err(_) => {
							return Err(invalid_value(
								descriptor.name,
								format!("invalid integer: {signed_error}"),
							));
						}
					},
				},
				_ => return Err(invalid_value(descriptor.name, "expected an integer")),
			};

			if let Some(integer) = number.as_i64() {
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
			} else if let Some(integer) = number.as_u64() {
				if let Some(min) = min
					&& min >= 0 && integer < min as u64
				{
					return Err(invalid_value(
						descriptor.name,
						format!("must be greater than or equal to {min}"),
					));
				}
				if let Some(max) = max
					&& (max < 0 || integer > max as u64)
				{
					return Err(invalid_value(
						descriptor.name,
						format!("must be less than or equal to {max}"),
					));
				}
			}
			Ok(serde_json::Value::Number(number))
		}
		ModelFormFieldKind::Float { min, max } => {
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
			if let Some(min) = min
				&& float < min
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must be greater than or equal to {min}"),
				));
			}
			if let Some(max) = max
				&& float > max
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must be less than or equal to {max}"),
				));
			}
			Ok(serde_json::Value::Number(number))
		}
		ModelFormFieldKind::Decimal { min, max } => {
			let decimal_text = match &value {
				serde_json::Value::Number(number) => number.to_string(),
				serde_json::Value::String(text) => text.clone(),
				_ => return Err(invalid_value(descriptor.name, "expected a decimal number")),
			};
			let decimal = Decimal::from_str(&decimal_text).map_err(|error| {
				invalid_value(descriptor.name, format!("invalid decimal: {error}"))
			})?;
			if let Some(min) = min
				&& decimal < Decimal::from_str(min).expect("generated decimal minimum is valid")
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must be greater than or equal to {min}"),
				));
			}
			if let Some(max) = max
				&& decimal > Decimal::from_str(max).expect("generated decimal maximum is valid")
			{
				return Err(invalid_value(
					descriptor.name,
					format!("must be less than or equal to {max}"),
				));
			}
			Ok(value)
		}
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
			normalize_time(&text)
				.map(serde_json::Value::String)
				.ok_or_else(|| invalid_value(descriptor.name, "expected HH:MM[:SS]"))
		}
		ModelFormFieldKind::DateTime | ModelFormFieldKind::NaiveDateTime => {
			let text = expect_string(descriptor.name, value)?;
			normalize_datetime_local(
				&text,
				matches!(descriptor.kind, ModelFormFieldKind::DateTime),
			)
			.map(serde_json::Value::String)
			.ok_or_else(|| {
				invalid_value(descriptor.name, "expected YYYY-MM-DDTHH:MM[:SS[.fraction]]")
			})
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

static EMAIL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
	Regex::new(r"^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$")
		.expect("native email validation pattern is valid")
});

fn is_email(value: &str) -> bool {
	EMAIL_REGEX.is_match(value)
}

fn is_url(value: &str) -> bool {
	url::Url::parse(value)
		.is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
}

fn is_date(value: &str) -> bool {
	let bytes = value.as_bytes();
	if !(bytes.len() == 10
		&& bytes[4] == b'-'
		&& bytes[7] == b'-'
		&& bytes
			.iter()
			.enumerate()
			.all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit()))
	{
		return false;
	}
	let Some(year) = value[0..4].parse::<u32>().ok() else {
		return false;
	};
	let Some(month) = value[5..7].parse::<u32>().ok() else {
		return false;
	};
	let Some(day) = value[8..10].parse::<u32>().ok() else {
		return false;
	};
	let max_day = match month {
		1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
		4 | 6 | 9 | 11 => 30,
		2 if year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100)) => {
			29
		}
		2 => 28,
		_ => return false,
	};
	(1..=max_day).contains(&day)
}

fn normalize_time(value: &str) -> Option<String> {
	let value = value.trim();
	let (time, meridiem) = match value.rsplit_once(' ') {
		Some((time, meridiem @ ("AM" | "PM"))) => (time, Some(meridiem)),
		Some(_) => return None,
		None => (value, None),
	};
	let (time, fraction) = time
		.split_once('.')
		.map_or((time, None), |(time, fraction)| (time, Some(fraction)));
	if fraction.is_some_and(|fraction| {
		fraction.is_empty()
			|| !(1..=9).contains(&fraction.len())
			|| !fraction.bytes().all(|byte| byte.is_ascii_digit())
	}) {
		return None;
	}
	let parts: Vec<_> = time.split(':').collect();
	if !matches!(parts.len(), 2 | 3)
		|| !parts
			.iter()
			.all(|part| part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_digit()))
	{
		return None;
	}
	let mut hour = parts[0].parse::<u32>().ok()?;
	let minute = parts[1].parse::<u32>().ok()?;
	let second = parts
		.get(2)
		.map_or(Some(0), |part| part.parse::<u32>().ok())?;
	if minute > 59 || second > 59 {
		return None;
	}
	match meridiem {
		Some("AM") if (1..=12).contains(&hour) => hour %= 12,
		Some("PM") if (1..=12).contains(&hour) => hour = (hour % 12) + 12,
		Some(_) => return None,
		None if hour > 23 => return None,
		None => {}
	}
	Some(match fraction {
		Some(fraction) => format!("{hour:02}:{minute:02}:{second:02}.{fraction}"),
		None => format!("{hour:02}:{minute:02}:{second:02}"),
	})
}

fn normalize_datetime_local(value: &str, aware: bool) -> Option<String> {
	let (date, time) = value.split_once('T').or_else(|| value.split_once(' '))?;
	if !is_date(date) {
		return None;
	}
	let time = if aware {
		time.strip_suffix('Z').unwrap_or(time)
	} else if time.ends_with('Z') {
		return None;
	} else {
		time
	};
	if time.contains(['+', '-']) {
		return None;
	}
	let (whole_time, fraction) = time
		.split_once('.')
		.map_or((time, None), |(whole, fraction)| (whole, Some(fraction)));
	if fraction.is_some_and(|fraction| {
		fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit())
	}) {
		return None;
	}
	let mut parts = whole_time.split(':');
	let hour = parts.next()?.parse::<u32>().ok()?;
	let minute = parts.next()?.parse::<u32>().ok()?;
	let second = parts
		.next()
		.map_or(Some(0), |second| second.parse::<u32>().ok())?;
	if parts.next().is_some() || hour > 23 || minute > 59 || second > 59 {
		return None;
	}
	let fraction = fraction.map_or_else(String::new, |fraction| format!(".{fraction}"));
	let timezone = if aware { "Z" } else { "" };
	Some(format!(
		"{date}T{hour:02}:{minute:02}:{second:02}{fraction}{timezone}"
	))
}

#[cfg(test)]
mod tests {
	use super::ModelFormState;
	use reinhardt_core::model_form::{
		AllEditableModelFields, ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormSchema,
	};

	struct NullableBooleanSchema;

	impl ModelFormSchema for NullableBooleanSchema {
		type Model = ();

		fn fields() -> &'static [ModelFormFieldDescriptor] {
			const FIELDS: [ModelFormFieldDescriptor; 1] = [ModelFormFieldDescriptor {
				name: "published",
				kind: ModelFormFieldKind::Boolean,
				required: false,
				has_default: false,
				nullable: true,
				editable: true,
				generated_relation_id: false,
			}];
			&FIELDS
		}
	}

	#[test]
	fn nullable_boolean_starts_unset() {
		let state = ModelFormState::<NullableBooleanSchema, AllEditableModelFields>::new();

		assert_eq!(state.value("published"), None);
	}

	struct NullableDefaultSchema;

	impl ModelFormSchema for NullableDefaultSchema {
		type Model = ();

		fn fields() -> &'static [ModelFormFieldDescriptor] {
			const FIELDS: [ModelFormFieldDescriptor; 1] = [ModelFormFieldDescriptor {
				name: "summary",
				kind: ModelFormFieldKind::Text {
					min_length: None,
					max_length: None,
					multiline: false,
				},
				required: false,
				has_default: true,
				nullable: true,
				editable: true,
				generated_relation_id: false,
			}];
			&FIELDS
		}
	}

	#[test]
	fn untouched_nullable_default_remains_omitted() {
		let mut state = ModelFormState::<NullableDefaultSchema, AllEditableModelFields>::new();

		state
			.set_value("summary", serde_json::Value::String(String::new()))
			.expect("an empty optional control should be accepted");

		assert_eq!(state.value("summary"), None);
	}

	struct OptionalContactSchema;

	impl ModelFormSchema for OptionalContactSchema {
		type Model = ();

		fn fields() -> &'static [ModelFormFieldDescriptor] {
			const FIELDS: [ModelFormFieldDescriptor; 2] = [
				ModelFormFieldDescriptor {
					name: "email",
					kind: ModelFormFieldKind::Email {
						min_length: None,
						max_length: None,
					},
					required: false,
					has_default: false,
					nullable: false,
					editable: true,
					generated_relation_id: false,
				},
				ModelFormFieldDescriptor {
					name: "website",
					kind: ModelFormFieldKind::Url {
						min_length: None,
						max_length: None,
					},
					required: false,
					has_default: false,
					nullable: false,
					editable: true,
					generated_relation_id: false,
				},
			];
			&FIELDS
		}
	}

	#[test]
	fn optional_email_and_url_accept_empty_controls() {
		let mut state = ModelFormState::<OptionalContactSchema, AllEditableModelFields>::new();

		for field in ["email", "website"] {
			state
				.set_value(field, serde_json::Value::String(String::new()))
				.expect("an empty optional contact control should be accepted");
			assert_eq!(
				state.value(field),
				Some(&serde_json::Value::String(String::new()))
			);
		}
	}
}
