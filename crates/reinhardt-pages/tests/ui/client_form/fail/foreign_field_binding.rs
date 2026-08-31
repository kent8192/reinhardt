use reinhardt_pages::{ClientForm, use_form};

#[derive(Clone, ClientForm)]
struct Profile {
	email: String,
}

#[derive(Clone, ClientForm)]
struct Login {
	email: String,
}

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let form = LoginClientForm::new();
		let runtime = use_form(&form).build();
		let _ = runtime.field(ProfileClientFormField::Email);
	});
}
