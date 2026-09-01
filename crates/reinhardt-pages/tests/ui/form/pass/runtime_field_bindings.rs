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
				choice_typed: ChoiceField<String> {
					widget: RadioSelect,
					choices_from: "options",
					choice_value: "value",
					choice_label: "label"
				},
				labels: MultipleChoiceField<String>,
			}
		};
		let runtime = use_form(&form).build();
		let _ = page!({
			input {
				bind: runtime.field(RuntimeFormField::Name)
			}
			input {
				type: "number",
				bind: runtime.field(RuntimeFormField::Count)
			}
			input {
				type: "checkbox",
				bind: runtime.field(RuntimeFormField::Active)
			}
			input {
				type: "radio",
				value: "yes",
				bind: runtime.field(RuntimeFormField::Choice)
			}
			input {
				type: "radio",
				value: "yes",
				bind: runtime.field(RuntimeFormField::ChoiceTyped)
			}
			select {
				bind: runtime.field(RuntimeFormField::Choice)
			}
			select {
				multiple: true,
				bind: runtime.field(RuntimeFormField::Labels)
			}
		});
	});
}
