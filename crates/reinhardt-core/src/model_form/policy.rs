//! Policy and payload contracts for model-backed forms.

use std::fmt;

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
	use crate::model_form::ModelFormPolicy;

	struct PublicOnly;

	impl ModelFormPolicy for PublicOnly {
		fn allows(field: &str) -> bool {
			field == "title"
		}
	}

	#[test]
	fn policy_rejects_known_but_unselected_fields() {
		assert!(PublicOnly::allows("title"));
		assert!(!PublicOnly::allows("owner_id"));
	}
}
