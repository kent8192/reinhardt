use reinhardt_pages::{form, page, use_form};

fn main() {

	reinhardt_core::reactive::ReactiveScope::run(|| {
		let form = form! {
			name: RuntimeForm,
			action: "/runtime",
			fields: {
				name: CharField,
				count: IntegerField,
				active: BooleanField,
				choice: CharField,
				labels: MultipleChoiceField<String>,
			}
		};
		let runtime = use_form(&form).build();
		let _ = page!({
			input { bind: text(runtime.field(RuntimeFormField::Name)) }
			input { type: "number", bind: number(runtime.field(RuntimeFormField::Count)) }
			input { type: "checkbox", bind: checked(runtime.field(RuntimeFormField::Active)) }
			input { type: "radio", value: "yes", bind: radio(runtime.field(RuntimeFormField::Choice), "yes") }
			select { bind: selected(runtime.field(RuntimeFormField::Choice)) }
			select { multiple: true, bind: selected_many(runtime.field(RuntimeFormField::Labels)) }
		});
	});
}
