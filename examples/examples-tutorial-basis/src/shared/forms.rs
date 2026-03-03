//! Form definitions for examples-tutorial-basis
//!
//! These forms are used server-side to generate FormMetadata
//! that is sent to the WASM client for CSRF token retrieval.

use reinhardt::forms::field::Widget;
use reinhardt::forms::{CharField, Form};

/// Create vote form definition
///
/// This form is primarily used to generate CSRF tokens for the voting form.
/// The actual choice selection uses dynamic radio buttons.
///
/// Fields:
/// - choice: The selected choice ID (hidden field for form metadata purposes)
pub fn create_vote_form() -> Form {
	let mut form = Form::new();

	form.add_field(Box::new(
		CharField::new("choice".to_string())
			.with_label("Choice")
			.with_widget(Widget::HiddenInput)
			.required(),
	));

	form
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt::forms::wasm_compat::FormExt;
	use rstest::rstest;

	#[rstest]
	fn test_vote_form_metadata() {
		let form = create_vote_form();
		let metadata = form.to_metadata();

		assert_eq!(metadata.fields.len(), 1);
		assert_eq!(metadata.fields[0].name, "choice");
		assert!(metadata.fields[0].required);
	}
}
