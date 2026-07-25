//! Native form-field construction from target-neutral model descriptors.

use reinhardt_core::model_form::{ModelFormFieldDescriptor, ModelFormFieldKind};

use crate::{
	BooleanField, CharField, DateField, DateTimeField, DecimalField, EmailField, FloatField,
	FormField, IntegerField, JSONField, TimeField, URLField, UUIDField, Widget,
};

/// Creates the native form field described by generated model metadata.
pub(super) fn create_form_field(descriptor: &ModelFormFieldDescriptor) -> Box<dyn FormField> {
	let name = descriptor.name.to_owned();

	match descriptor.kind {
		ModelFormFieldKind::Text {
			max_length,
			multiline,
		} => {
			let mut field = CharField::new(name);
			field.required = descriptor.required;
			field.max_length = max_length;
			if multiline {
				field.widget = Widget::TextArea;
			}
			Box::new(field)
		}
		ModelFormFieldKind::Email { max_length } => {
			let mut field = EmailField::new(name);
			field.required = descriptor.required;
			field.max_length = max_length;
			Box::new(field)
		}
		ModelFormFieldKind::Url { max_length } => {
			let mut field = URLField::new(name);
			field.required = descriptor.required;
			field.max_length = max_length;
			Box::new(field)
		}
		ModelFormFieldKind::Integer { min, max } => {
			let mut field = IntegerField::new(name);
			field.required = descriptor.required;
			field.min_value = min;
			field.max_value = max;
			Box::new(field)
		}
		ModelFormFieldKind::Float => {
			let mut field = FloatField::new(name);
			field.required = descriptor.required;
			Box::new(field)
		}
		ModelFormFieldKind::Decimal => {
			let mut field = DecimalField::new(name);
			field.required = descriptor.required;
			Box::new(field)
		}
		ModelFormFieldKind::Boolean => {
			let mut field = BooleanField::new(name);
			field.required = descriptor.required;
			Box::new(field)
		}
		ModelFormFieldKind::Date => {
			let mut field = DateField::new(name);
			field.required = descriptor.required;
			Box::new(field)
		}
		ModelFormFieldKind::Time => {
			let mut field = TimeField::new(name);
			field.required = descriptor.required;
			Box::new(field)
		}
		ModelFormFieldKind::DateTime => {
			let mut field = DateTimeField::new(name);
			field.required = descriptor.required;
			Box::new(field)
		}
		ModelFormFieldKind::Uuid => Box::new(UUIDField::new(name).required(descriptor.required)),
		ModelFormFieldKind::Json => Box::new(JSONField::new(name).required(descriptor.required)),
	}
}
