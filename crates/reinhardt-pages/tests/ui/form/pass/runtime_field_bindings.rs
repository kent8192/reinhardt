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
				a11y: off,
				bind: runtime.field(form.name_field())
			}
			input {
				a11y: off,
				type: "number",
				bind: runtime.field(form.count_field())
			}
			input {
				a11y: off,
				type: "checkbox",
				bind: runtime.field(form.active_field())
			}
			input {
				a11y: off,
				type: "radio",
				value: "yes",
				bind: runtime.field(form.choice_field())
			}
			input {
				a11y: off,
				type: "radio",
				value: "yes",
				bind: runtime.field(form.choice_typed_field())
			}
			select {
				a11y: off,
				bind: runtime.field(form.choice_field())
			}
			select {
				a11y: off,
				multiple: true,
				bind: runtime.field(form.labels_field())
			}
		});
	});
}
