//! SubmitButton in form! macro fields
//!
//! Fixes #3331: form! macro should support SubmitButton in fields block.

use reinhardt_pages::form;

fn main() {
	// Basic form with SubmitButton
	let _login_form = form! {
		name: LoginForm,
		action: "/api/login",

		fields: {
			username: CharField { required, label: "Username" },
			password: CharField { required, widget: PasswordInput, label: "Password" },
			submit: SubmitButton { label: "Sign in", class: "btn-primary" },
		},
	};

	// SubmitButton with minimal properties (defaults to "Submit" label)
	let _minimal_form = form! {
		name: MinimalForm,
		action: "/api/submit",

		fields: {
			name: CharField { required },
			submit: SubmitButton {},
		},
	};

	// SubmitButton with id and disabled
	let _disabled_form = form! {
		name: DisabledForm,
		action: "/api/submit",

		fields: {
			email: EmailField { required },
			submit: SubmitButton { label: "Send", id: "submit-btn", disabled },
		},
	};
}
