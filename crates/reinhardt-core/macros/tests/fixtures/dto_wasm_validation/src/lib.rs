#![deny(unexpected_cfgs)]

use reinhardt::dto;
use reinhardt::pages::reactive::ReactiveScope;
use reinhardt::pages::server_fn::{ServerFnError, server_fn};
use reinhardt::pages::{ClientForm, FormRuntimeSource};
use serde::{Deserialize, Serialize};

#[dto]
#[derive(Debug, Clone, Serialize, Deserialize, ClientForm)]
#[client_form(validate, server_fn = submit_signup)]
pub struct ClientSignup {
	#[validate(email(message = "Invalid email address"))]
	pub email: String,

	#[validate(url(message = "Invalid homepage URL"))]
	pub homepage_url: Option<String>,

	#[validate(length(
		min = 3,
		max = 64,
		message = "Username must be between 3 and 64 characters"
	))]
	pub username: String,

	#[validate(range(min = 18, max = 130, message = "Age is outside the allowed range"))]
	pub age: u8,
}

#[server_fn]
pub async fn submit_signup(_request: ClientSignup) -> Result<(), ServerFnError> {
	Ok(())
}

pub fn invalid_signup_is_rejected() -> bool {
	let request = ClientSignup {
		email: String::from("not-an-email"),
		homepage_url: Some(String::from("not-a-url")),
		username: String::from("xy"),
		age: 17,
	};

	reinhardt::Validate::validate(&request).is_err()
}

#[cfg(test)]
mod tests {
	use super::*;
	use wasm_bindgen_test::wasm_bindgen_test;

	#[wasm_bindgen_test]
	fn invalid_signup_reports_field_errors() {
		assert!(invalid_signup_is_rejected());

		let request = ClientSignup {
			email: String::from("not-an-email"),
			homepage_url: Some(String::from("not-a-url")),
			username: String::from("xy"),
			age: 17,
		};

		let errors = reinhardt::Validate::validate(&request)
			.expect_err("invalid signup must fail validation");
		let field_errors = errors.field_errors();

		assert!(field_errors.contains_key("email"));
		assert!(field_errors.contains_key("homepage_url"));
		assert!(field_errors.contains_key("username"));
		assert!(field_errors.contains_key("age"));
	}

	#[wasm_bindgen_test]
	fn invalid_signup_client_form_reports_dto_field_errors() {
		ReactiveScope::run(|| {
			let form = ClientSignupClientForm::new();
			let errors = form
				.runtime_validate()
				.expect_err("an empty client form must fail DTO validation");

			assert!(
				errors
					.field_errors()
					.contains_key(&ClientSignupClientFormField::Email)
			);
			assert!(
				errors
					.field_errors()
					.contains_key(&ClientSignupClientFormField::Username)
			);
			assert!(
				errors
					.field_errors()
					.contains_key(&ClientSignupClientFormField::Age)
			);
		});
	}
}
