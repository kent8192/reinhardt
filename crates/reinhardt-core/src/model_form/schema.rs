//! Schema contracts describing fields available to model-backed forms.

/// The target-neutral input kind for a model-backed form field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelFormFieldKind {
	/// A text input with an optional maximum length and multiline mode.
	Text {
		/// The maximum permitted string length, when constrained.
		max_length: Option<usize>,
		/// Whether the field accepts multiple lines.
		multiline: bool,
	},
	/// An email input with an optional maximum length.
	Email {
		/// The maximum permitted string length, when constrained.
		max_length: Option<usize>,
	},
	/// A URL input with an optional maximum length.
	Url {
		/// The maximum permitted string length, when constrained.
		max_length: Option<usize>,
	},
	/// An integer input with optional inclusive bounds.
	Integer {
		/// The inclusive minimum value, when constrained.
		min: Option<i64>,
		/// The inclusive maximum value, when constrained.
		max: Option<i64>,
	},
	/// A floating-point input.
	Float,
	/// A decimal input.
	Decimal,
	/// A boolean input.
	Boolean,
	/// A calendar-date input.
	Date,
	/// A time-of-day input.
	Time,
	/// A timezone-aware date-and-time input.
	DateTime,
	/// A timezone-naive date-and-time input.
	NaiveDateTime,
	/// A UUID input.
	Uuid,
	/// A JSON input.
	Json,
}

/// Compile-time metadata for a field exposed by a model-backed form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelFormFieldDescriptor {
	/// The model field name.
	pub name: &'static str,
	/// The target-neutral field kind.
	pub kind: ModelFormFieldKind,
	/// Whether input must supply a value for this field.
	pub required: bool,
	/// Whether the model provides a value when input omits this field.
	pub has_default: bool,
	/// Whether an explicit empty control value clears the model field to null.
	pub nullable: bool,
	/// Whether the field is editable through a form.
	pub editable: bool,
	/// Whether the field is a generated relationship identifier.
	pub generated_relation_id: bool,
}

/// Supplies compile-time field metadata for a model-backed form.
pub trait ModelFormSchema {
	/// The model described by this schema.
	type Model;

	/// Returns the model fields known to this form schema.
	fn fields() -> &'static [ModelFormFieldDescriptor];
}

/// Supplies the target-neutral form kind for a model primary key.
///
/// This allows generated foreign-key identifiers to use their target model's
/// actual scalar input kind without importing ORM metadata on shared targets.
pub trait ModelFormPrimaryKey {
	/// The target-neutral input kind for this model's primary key.
	const FIELD_KIND: ModelFormFieldKind;
}

#[cfg(test)]
mod tests {
	use crate::model_form::{ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPrimaryKey};

	struct TextPrimaryKey;

	impl ModelFormPrimaryKey for TextPrimaryKey {
		const FIELD_KIND: ModelFormFieldKind = ModelFormFieldKind::Text {
			max_length: Some(64),
			multiline: false,
		};
	}

	#[test]
	fn descriptor_keeps_required_and_default_independent() {
		let descriptor = ModelFormFieldDescriptor {
			name: "title",
			kind: ModelFormFieldKind::Text {
				max_length: Some(200),
				multiline: false,
			},
			required: true,
			has_default: false,
			nullable: false,
			editable: true,
			generated_relation_id: false,
		};

		assert!(descriptor.required);
		assert!(!descriptor.has_default);
	}

	#[test]
	fn primary_key_kind_is_available_without_orm_metadata() {
		assert_eq!(
			TextPrimaryKey::FIELD_KIND,
			ModelFormFieldKind::Text {
				max_length: Some(64),
				multiline: false,
			}
		);
	}
}
