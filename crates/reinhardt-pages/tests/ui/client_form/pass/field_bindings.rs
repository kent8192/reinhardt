use reinhardt_pages::control_binding::__private::{
	CheckboxBinding, NumberBinding, RadioBinding, SelectOneBinding, TextBinding, into_control_binding,
};
use reinhardt_pages::{ClientForm, use_form};

#[derive(Clone, ClientForm)]
struct Profile {
	email: String,
	age: i64,
	active: bool,
}

fn main() {
	reinhardt_core::reactive::ReactiveScope::run(|| {
		let form = ProfileClientForm::new();
		let runtime = use_form(&form).build();
		let _ = into_control_binding::<TextBinding, _>(runtime.field(ProfileClientFormField::Email), ());
		let _ = into_control_binding::<RadioBinding, _>(
			runtime.field(ProfileClientFormField::Email),
			"work".to_string(),
		);
		let _ = into_control_binding::<SelectOneBinding, _>(
			runtime.field(ProfileClientFormField::Email),
			(),
		);
		let _ = into_control_binding::<NumberBinding, _>(runtime.field(ProfileClientFormField::Age), ());
		let _ = into_control_binding::<CheckboxBinding, _>(
			runtime.field(ProfileClientFormField::Active),
			(),
		);
	});
}
