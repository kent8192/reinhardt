//! Admin form customization contracts.

use serde_json::Value;
use std::collections::HashMap;

pub use crate::types::{AdminWidget, FormFieldOverride, PrepopulatedField};

/// Operation currently performed by an admin form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminFormMode {
	/// Create a new record.
	Create,
	/// Update an existing record.
	Update,
}

/// Owned admin form values keyed by configured field name.
pub type AdminFormData = HashMap<String, Value>;

/// Result returned by custom admin form hooks.
pub type AdminFormResult<T> = Result<T, AdminFormErrors>;

/// One field-local or form-global validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminFormError {
	field: Option<String>,
	message: String,
}

impl AdminFormError {
	/// Return the affected field, or `None` for a form-global error.
	pub fn field(&self) -> Option<&str> {
		self.field.as_deref()
	}

	/// Return the validation message.
	pub fn message(&self) -> &str {
		&self.message
	}
}

/// Ordered validation errors returned by custom admin form hooks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdminFormErrors {
	errors: Vec<AdminFormError>,
}

impl AdminFormErrors {
	/// Create errors containing one field-local message.
	pub fn field(field: impl Into<String>, message: impl Into<String>) -> Self {
		let mut errors = Self::default();
		errors.push_field(field, message);
		errors
	}

	/// Create errors containing one form-global message.
	pub fn global(message: impl Into<String>) -> Self {
		let mut errors = Self::default();
		errors.push_global(message);
		errors
	}

	/// Append a field-local validation message.
	pub fn push_field(&mut self, field: impl Into<String>, message: impl Into<String>) {
		self.errors.push(AdminFormError {
			field: Some(field.into()),
			message: message.into(),
		});
	}

	/// Append a form-global validation message.
	pub fn push_global(&mut self, message: impl Into<String>) {
		self.errors.push(AdminFormError {
			field: None,
			message: message.into(),
		});
	}

	/// Iterate over errors in insertion order.
	pub fn iter(&self) -> impl Iterator<Item = &AdminFormError> {
		self.errors.iter()
	}

	/// Return whether no validation messages were recorded.
	pub fn is_empty(&self) -> bool {
		self.errors.is_empty()
	}
}

/// Object-safe hook for customizing a model admin form.
pub trait AdminForm: std::fmt::Debug + Send + Sync {
	/// Return optional per-field schema overlays.
	fn schema(&self) -> Vec<FormFieldOverride> {
		Vec::new()
	}

	/// Normalize submitted data before validation and persistence.
	fn normalize(
		&self,
		_mode: AdminFormMode,
		data: AdminFormData,
	) -> AdminFormResult<AdminFormData> {
		Ok(data)
	}

	/// Validate normalized submitted data.
	fn validate(&self, _mode: AdminFormMode, _data: &AdminFormData) -> AdminFormResult<()> {
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;
	use std::collections::HashMap;

	#[derive(Debug)]
	struct DefaultForm;

	impl AdminForm for DefaultForm {}

	#[test]
	fn default_form_preserves_data_without_validation_errors() {
		let form = DefaultForm;
		let input = HashMap::from([(String::from("title"), json!("Rust"))]);

		assert!(form.schema().is_empty());
		assert_eq!(
			form.normalize(AdminFormMode::Create, input.clone())
				.unwrap(),
			input,
		);
		assert!(form.validate(AdminFormMode::Update, &input).is_ok());
	}

	#[test]
	fn form_errors_keep_field_and_global_messages_in_insertion_order() {
		let mut errors = AdminFormErrors::field("title", "must not be empty");
		errors.push_field("title", "must be unique");
		errors.push_global("form is invalid");

		let entries: Vec<_> = errors
			.iter()
			.map(|error| (error.field(), error.message()))
			.collect();

		assert_eq!(
			entries,
			vec![
				(Some("title"), "must not be empty"),
				(Some("title"), "must be unique"),
				(None, "form is invalid"),
			],
		);
	}
}
