//! form! macro with legacy `client_validators:` block should fail with a customized
//! migration error (Fixes #3654).

use reinhardt_pages::form;

fn main() {
	// This should fail - `client_validators:` was never wired to code generation and
	// is rejected in 0.1.0-rc.16 with a guidance error pointing to the migration doc.
	let _form = form! {
		name: TestForm,
		action: "/api/submit",

		fields: {
			username: CharField { required },
		},

		client_validators: {
			username: [
				"value.length >= 3" => "Too short",
			],
		},
	};
}
