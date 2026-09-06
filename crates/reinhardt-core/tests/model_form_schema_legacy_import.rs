use reinhardt_core::model_form::*;

struct LegacySchema;

impl ModelFormSchema for LegacySchema {
	type Model = ();

	fn fields() -> &'static [ModelFormFieldDescriptor] {
		const FIELDS: [ModelFormFieldDescriptor; 1] = [ModelFormFieldDescriptor {
			name: "title",
			kind: ModelFormFieldKind::Text {
				min_length: None,
				max_length: None,
				multiline: false,
			},
			required: true,
			has_default: false,
			nullable: false,
			editable: true,
			generated_relation_id: false,
		}];
		&FIELDS
	}
}

#[test]
fn legacy_fields_remain_unambiguous_with_the_model_form_prelude() {
	assert_eq!(LegacySchema::fields()[0].name, "title");
}
