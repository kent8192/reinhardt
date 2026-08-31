//! Policy and payload contracts for model-backed forms.

use std::fmt;

use crate::model_form::{ModelFormFieldKind, ModelFormSchema};

/// Determines which known model fields a form may accept.
pub trait ModelFormPolicy: Send + Sync + 'static {
	/// Returns whether the named model field is permitted by this policy.
	fn allows(field: &str) -> bool;
}

/// A policy that permits every editable field supplied by a schema.
pub struct AllEditableModelFields;

impl ModelFormPolicy for AllEditableModelFields {
	fn allows(_field: &str) -> bool {
		true
	}
}

/// A target-neutral payload accepted by a model-backed form.
pub trait ModelFormPayload<P: ModelFormPolicy>: Sized {
	/// Returns the statically known fields supplied by this payload.
	fn supplied_fields(&self) -> Vec<&'static str>;

	/// Returns fields rejected by the form policy.
	fn forbidden_fields(&self) -> &[&'static str];

	/// Returns the JSON value supplied for a field, when present.
	fn get_json(&self, field: &str) -> Option<serde_json::Value>;

	/// Replaces the JSON value supplied for a field.
	///
	/// # Errors
	///
	/// Returns an error when the field is unknown, forbidden, or cannot accept the value.
	fn set_json(
		&mut self,
		field: &str,
		value: serde_json::Value,
	) -> Result<(), ModelFormPayloadError>;
}

/// Converts an already-normalized native HTML form object into a model-form payload.
///
/// Native checkboxes omit an unchecked non-optional boolean control. Implementations
/// may apply that HTML-specific convention without changing ordinary JSON decoding.
pub trait NativeModelFormPayload: Sized {
	/// Builds the payload from the native form object's JSON representation.
	fn from_native_form_value(value: serde_json::Value) -> Result<Self, serde_json::Error>;
}

/// Normalizes controls produced by a native HTML model form before decoding.
///
/// Browser form submissions represent every successful control as text and
/// omit unchecked checkboxes. The generated color-control marker also omits an
/// untouched optional color control when the browser supplies its synthetic
/// black fallback. An untouched optional range control likewise omits its
/// browser-generated minimum value. This conversion is intentionally limited to schema fields
/// permitted by the selected policy; unrelated controls such as the CSRF token
/// are removed before typed payload decoding.
///
/// # Errors
///
/// Returns a JSON error when a JSON control does not contain valid JSON text.
pub fn normalize_native_model_form_value<S, P>(
	mut value: serde_json::Value,
) -> Result<serde_json::Value, serde_json::Error>
where
	S: ModelFormSchema,
	P: ModelFormPolicy,
{
	let serde_json::Value::Object(values) = &mut value else {
		return Ok(value);
	};

	values.remove("csrfmiddlewaretoken");
	for descriptor in S::fields() {
		if !descriptor.editable || !P::allows(descriptor.name) {
			continue;
		}

		let checkbox_sentinel = format!("__reinhardt_checkbox_{}", descriptor.name);
		let checkbox_sentinel_value = values.remove(&checkbox_sentinel);
		let checkbox_was_unchecked = checkbox_sentinel_value
			.as_ref()
			.is_some_and(|value| value == &serde_json::Value::String("false".to_owned()));
		let checkbox_was_unset = checkbox_sentinel_value
			.as_ref()
			.is_some_and(|value| value == &serde_json::Value::String("unset".to_owned()));
		let color_sentinel = format!("__reinhardt_color_{}", descriptor.name);
		let color_was_edited = values
			.remove(&color_sentinel)
			.map(|value| value == serde_json::Value::String("true".to_owned()));
		let range_sentinel = format!("__reinhardt_range_{}", descriptor.name);
		let range_default = values.remove(&range_sentinel);
		let default_clear_sentinel = format!("__reinhardt_defaulted_{}", descriptor.name);
		let had_defaulted_value = values
			.remove(&default_clear_sentinel)
			.is_some_and(|value| value == serde_json::Value::String("true".to_owned()));
		let Some(control) = values.get_mut(descriptor.name) else {
			if matches!(descriptor.kind, ModelFormFieldKind::Boolean) && checkbox_was_unchecked {
				values.insert(descriptor.name.to_owned(), serde_json::Value::Bool(false));
			}
			continue;
		};
		let serde_json::Value::String(text) = control else {
			continue;
		};
		if color_was_edited == Some(false) && text == "#000000" {
			values.remove(descriptor.name);
			continue;
		}
		if range_default.as_ref() == Some(&serde_json::Value::String(text.clone())) {
			values.remove(descriptor.name);
			continue;
		}
		if checkbox_was_unset && text.is_empty() {
			values.remove(descriptor.name);
			continue;
		}

		let remove_empty = text.is_empty()
			&& !descriptor.required
			&& !descriptor.nullable
			&& (descriptor.has_default
				|| !matches!(
					descriptor.kind,
					ModelFormFieldKind::Text { .. }
						| ModelFormFieldKind::Email { .. }
						| ModelFormFieldKind::Url { .. }
				));
		if text.is_empty() && !descriptor.required && descriptor.nullable {
			if descriptor.has_default && !had_defaulted_value {
				values.remove(descriptor.name);
			} else {
				*control = serde_json::Value::Null;
			}
			continue;
		}
		if remove_empty {
			values.remove(descriptor.name);
			continue;
		}
		let normalized = match descriptor.kind {
			ModelFormFieldKind::Boolean => match text.as_str() {
				"true" | "on" | "1" => Some(serde_json::Value::Bool(true)),
				"false" | "off" | "0" => Some(serde_json::Value::Bool(false)),
				_ => None,
			},
			ModelFormFieldKind::Integer { .. } => text
				.parse::<i64>()
				.ok()
				.map(|number| serde_json::Value::Number(number.into()))
				.or_else(|| {
					text.parse::<u64>()
						.ok()
						.map(|number| serde_json::Value::Number(number.into()))
				}),
			ModelFormFieldKind::Float { min, max } => text
				.parse::<f64>()
				.ok()
				.filter(|number| number.is_finite())
				.filter(|number| min.is_none_or(|min| *number >= min))
				.filter(|number| max.is_none_or(|max| *number <= max))
				.and_then(serde_json::Number::from_f64)
				.map(serde_json::Value::Number),
			ModelFormFieldKind::Time if text.len() == 5 && text.as_bytes()[2] == b':' => {
				Some(serde_json::Value::String(format!("{text}:00")))
			}
			ModelFormFieldKind::Json => Some(serde_json::from_str(text)?),
			ModelFormFieldKind::DateTime | ModelFormFieldKind::NaiveDateTime
				if text.split_once('T').is_some_and(|(_, time)| {
					!time.ends_with('Z') && !time.contains(['+', '-'])
				}) =>
			{
				let normalized = text.split_once('T').map_or_else(
					|| text.to_owned(),
					|(date, time)| {
						if time.len() == 5
							&& time.as_bytes()[2] == b':'
							&& time
								.bytes()
								.enumerate()
								.all(|(index, byte)| index == 2 || byte.is_ascii_digit())
						{
							format!("{date}T{time}:00")
						} else {
							text.to_owned()
						}
					},
				);
				let timezone = matches!(descriptor.kind, ModelFormFieldKind::DateTime)
					.then_some("Z")
					.unwrap_or_default();
				Some(serde_json::Value::String(format!("{normalized}{timezone}")))
			}
			_ => None,
		};
		if let Some(normalized) = normalized {
			*control = normalized;
		}
	}

	Ok(value)
}

/// An error returned while reading or updating a model form payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelFormPayloadError {
	/// The payload does not define the supplied field.
	UnknownField {
		/// The field name that is absent from the payload.
		field: String,
	},
	/// The policy does not permit the supplied field.
	ForbiddenField {
		/// The field name rejected by the policy.
		field: String,
	},
	/// The supplied JSON value cannot be accepted for the field.
	InvalidValue {
		/// The field receiving the invalid value.
		field: String,
		/// A human-readable explanation of why the value is invalid.
		message: String,
	},
}

impl fmt::Display for ModelFormPayloadError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::UnknownField { field } => {
				write!(formatter, "unknown model form field '{field}'")
			}
			Self::ForbiddenField { field } => {
				write!(formatter, "forbidden model form field '{field}'")
			}
			Self::InvalidValue { field, message } => {
				write!(
					formatter,
					"invalid value for model form field '{field}': {message}"
				)
			}
		}
	}
}

impl std::error::Error for ModelFormPayloadError {}

#[cfg(test)]
mod tests {
	use super::{AllEditableModelFields, normalize_native_model_form_value};
	use crate::model_form::{
		ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPolicy, ModelFormSchema,
	};

	struct PublicOnly;

	impl ModelFormPolicy for PublicOnly {
		fn allows(field: &str) -> bool {
			field == "title"
		}
	}

	struct TestSchema;

	impl ModelFormSchema for TestSchema {
		type Model = ();

		fn fields() -> &'static [ModelFormFieldDescriptor] {
			const FIELDS: [ModelFormFieldDescriptor; 8] = [
				ModelFormFieldDescriptor {
					name: "enabled",
					kind: ModelFormFieldKind::Boolean,
					required: true,
					has_default: false,
					nullable: false,
					editable: true,
					generated_relation_id: false,
					trim: false,
				},
				ModelFormFieldDescriptor {
					name: "count",
					kind: ModelFormFieldKind::Integer {
						min: None,
						max: None,
					},
					required: true,
					has_default: false,
					nullable: false,
					editable: true,
					generated_relation_id: false,
					trim: false,
				},
				ModelFormFieldDescriptor {
					name: "metadata",
					kind: ModelFormFieldKind::Json,
					required: false,
					has_default: false,
					nullable: true,
					editable: true,
					generated_relation_id: false,
					trim: false,
				},
				ModelFormFieldDescriptor {
					name: "created_at",
					kind: ModelFormFieldKind::DateTime,
					required: true,
					has_default: false,
					nullable: false,
					editable: true,
					generated_relation_id: false,
					trim: false,
				},
				ModelFormFieldDescriptor {
					name: "title",
					kind: ModelFormFieldKind::Text {
						min_length: None,
						max_length: None,
						multiline: false,
					},
					required: false,
					has_default: false,
					nullable: false,
					editable: true,
					generated_relation_id: false,
					trim: false,
				},
				ModelFormFieldDescriptor {
					name: "accent",
					kind: ModelFormFieldKind::Text {
						min_length: None,
						max_length: None,
						multiline: false,
					},
					required: false,
					has_default: false,
					nullable: true,
					editable: true,
					generated_relation_id: false,
					trim: false,
				},
				ModelFormFieldDescriptor {
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
					trim: false,
				},
				ModelFormFieldDescriptor {
					name: "owner_id",
					kind: ModelFormFieldKind::Integer {
						min: None,
						max: None,
					},
					required: true,
					has_default: false,
					nullable: false,
					editable: false,
					generated_relation_id: false,
					trim: false,
				},
			];
			&FIELDS
		}
	}

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
				trim: false,
			}];
			&FIELDS
		}
	}

	#[test]
	fn policy_rejects_known_but_unselected_fields() {
		assert!(PublicOnly::allows("title"));
		assert!(!PublicOnly::allows("owner_id"));
	}

	#[test]
	fn native_normalization_removes_csrf_and_converts_controls() {
		let value = normalize_native_model_form_value::<TestSchema, AllEditableModelFields>(
			serde_json::json!({
				"csrfmiddlewaretoken": "token",
				"enabled": "on",
				"count": "30",
				"metadata": "{\"draft\":true}",
			}),
		)
		.expect("native form value should normalize");

		assert_eq!(
			value,
			serde_json::json!({
				"enabled": true,
				"count": 30,
				"metadata": {"draft": true},
			}),
		);
	}

	#[test]
	fn native_normalization_preserves_datetime_precision_and_checkbox_false() {
		let value = normalize_native_model_form_value::<TestSchema, AllEditableModelFields>(
			serde_json::json!({
				"__reinhardt_checkbox_enabled": "false",
				"created_at": "2026-07-26T09:30",
			}),
		)
		.expect("native form value should normalize");

		assert_eq!(
			value,
			serde_json::json!({
				"enabled": false,
				"created_at": "2026-07-26T09:30:00Z",
			}),
		);
	}

	#[test]
	fn native_normalization_preserves_nullable_boolean_unset_and_false() {
		let unset = normalize_native_model_form_value::<
			NullableBooleanSchema,
			AllEditableModelFields,
		>(serde_json::json!({
			"published": "",
			"__reinhardt_checkbox_published": "unset",
		}))
		.expect("nullable boolean control should normalize");
		assert_eq!(unset, serde_json::json!({}));

		let false_value = normalize_native_model_form_value::<
			NullableBooleanSchema,
			AllEditableModelFields,
		>(serde_json::json!({
			"published": "false",
			"__reinhardt_checkbox_published": "unset",
		}))
		.expect("nullable boolean false selection should normalize");
		assert_eq!(false_value, serde_json::json!({ "published": false }));
	}

	#[test]
	fn native_normalization_keeps_blank_optional_text() {
		let value = normalize_native_model_form_value::<TestSchema, AllEditableModelFields>(
			serde_json::json!({ "title": "" }),
		)
		.expect("native form value should normalize");

		assert_eq!(value, serde_json::json!({ "title": "" }));
	}

	#[test]
	fn native_normalization_omits_blank_nullable_defaults() {
		let value = normalize_native_model_form_value::<TestSchema, AllEditableModelFields>(
			serde_json::json!({ "summary": "" }),
		)
		.expect("native form value should normalize");

		assert_eq!(value, serde_json::json!({}));
	}

	#[test]
	fn native_normalization_preserves_explicit_nullable_default_clears() {
		let value = normalize_native_model_form_value::<TestSchema, AllEditableModelFields>(
			serde_json::json!({
				"summary": "",
				"__reinhardt_defaulted_summary": "true",
			}),
		)
		.expect("native form value should normalize");

		assert_eq!(value, serde_json::json!({ "summary": null }));
	}

	#[test]
	fn native_normalization_omits_an_unedited_optional_color_control() {
		let value = normalize_native_model_form_value::<TestSchema, AllEditableModelFields>(
			serde_json::json!({
				"accent": "#000000",
				"__reinhardt_color_accent": "false",
			}),
		)
		.expect("native form value should normalize");

		assert_eq!(value, serde_json::json!({}));
	}

	#[test]
	fn native_normalization_preserves_an_edited_optional_color_control() {
		let value = normalize_native_model_form_value::<TestSchema, AllEditableModelFields>(
			serde_json::json!({
				"accent": "#000000",
				"__reinhardt_color_accent": "true",
			}),
		)
		.expect("native form value should normalize");

		assert_eq!(value, serde_json::json!({ "accent": "#000000" }));
	}
}
