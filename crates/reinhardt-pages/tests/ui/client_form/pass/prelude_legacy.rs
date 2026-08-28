use reinhardt_core::reactive::ReactiveScope;
use reinhardt_pages::ClientForm as Form;
use reinhardt_pages::prelude::*;

#[derive(Clone, PartialEq, Form)]
#[client_form(name = LegacyForm)]
struct LegacyRequest {
	name: String,
}

fn main() {
	ReactiveScope::run(|| {
		assert_eq!(
			LegacyForm::new().runtime_field_by_name("name"),
			Some(LegacyFormField::Name),
		);
	});
}
