//! Schema contracts describing fields available to model-backed forms.

/// The target-neutral input kind for a model-backed form field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModelFormFieldKind {
	/// A text input with optional length bounds and multiline mode.
	Text {
		/// The minimum permitted string length, when constrained.
		min_length: Option<usize>,
		/// The maximum permitted string length, when constrained.
		max_length: Option<usize>,
		/// Whether the field accepts multiple lines.
		multiline: bool,
	},
	/// An email input with optional length bounds.
	Email {
		/// The minimum permitted string length, when constrained.
		min_length: Option<usize>,
		/// The maximum permitted string length, when constrained.
		max_length: Option<usize>,
	},
	/// A URL input with optional length bounds.
	Url {
		/// The minimum permitted string length, when constrained.
		min_length: Option<usize>,
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
	/// A floating-point input with optional inclusive bounds.
	Float {
		/// The inclusive minimum value, when constrained.
		min: Option<f64>,
		/// The inclusive maximum value, when constrained.
		max: Option<f64>,
	},
	/// A decimal input with optional inclusive bounds.
	Decimal {
		/// The inclusive minimum value, when constrained.
		min: Option<&'static str>,
		/// The inclusive maximum value, when constrained.
		max: Option<&'static str>,
	},
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
	/// A browser-selected file input.
	File,
	/// A browser-selected image input.
	Image,
}

/// Compile-time metadata for a field exposed by a model-backed form.
#[derive(Debug, Clone, Copy, PartialEq)]
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

	/// Returns whether an omitted boolean field defaults to `true`.
	fn default_boolean_is_true(_field: &str) -> bool {
		false
	}

	/// Returns whether a generated relationship identifier targets `T`.
	///
	/// This keeps relation-aware form helpers target-safe without exposing ORM
	/// metadata to shared form schemas.
	fn relation_target_matches<T: 'static>(_field: &str) -> bool {
		false
	}
}

/// Supplies target-neutral field metadata for a model form contract.
pub trait ModelFormContractSchema {
	/// Returns the fields exposed by this form contract.
	fn fields() -> &'static [ModelFormFieldDescriptor];

	/// Returns whether an omitted boolean field defaults to `true`.
	fn default_boolean_is_true(_field: &str) -> bool {
		false
	}

	/// Returns whether a generated relationship identifier targets `T`.
	fn relation_target_matches<T: 'static>(_field: &str) -> bool {
		false
	}
}

impl<S: ModelFormSchema> ModelFormContractSchema for S {
	fn fields() -> &'static [ModelFormFieldDescriptor] {
		<S as ModelFormSchema>::fields()
	}

	fn default_boolean_is_true(field: &str) -> bool {
		<S as ModelFormSchema>::default_boolean_is_true(field)
	}

	fn relation_target_matches<T: 'static>(field: &str) -> bool {
		<S as ModelFormSchema>::relation_target_matches::<T>(field)
	}
}

/// A typed field token exposed by a target-neutral model form contract.
pub trait ModelFormContractField: Copy + Eq + std::hash::Hash + std::fmt::Debug + 'static {
	/// Returns the source field name represented by this token.
	fn name(self) -> &'static str;
}

/// A target-neutral named model form contract.
pub trait ModelFormContract: 'static {
	/// The concrete payload accepted by this form contract.
	type Data: Default
		+ crate::model_form::ModelFormPayload<Self::Policy>
		+ serde::Serialize
		+ serde::de::DeserializeOwned;
	/// The target-neutral schema for this form contract.
	type Schema: ModelFormContractSchema;
	/// The typed field tokens exposed by this form contract.
	type Field: ModelFormContractField;

	/// The generated field-selection policy for this form contract.
	#[doc(hidden)]
	type Policy: crate::model_form::ModelFormPolicy;

	/// Returns the contract fields in declaration order.
	fn fields() -> &'static [Self::Field];
}

/// Supplies the database table name for shared model-form metadata.
pub trait ModelFormTableName {
	/// Returns the database table backing the model.
	fn table_name() -> &'static str;
}

/// Supplies the target-neutral form kind for a model primary key.
///
/// This allows generated foreign-key identifiers to use their target model's
/// actual scalar input kind without importing ORM metadata on shared targets.
pub trait ModelFormPrimaryKey {
	/// The target-neutral input kind for this model's primary key.
	const FIELD_KIND: ModelFormFieldKind;
}

/// Supplies the complete field list that composes a model primary key.
///
/// Unlike [`ModelFormPrimaryKey`], this trait also supports composite primary
/// keys. It is intended for relation-aware form helpers that must exclude all
/// target primary-key fields from an update payload.
pub trait ModelFormPrimaryKeyFields {
	/// Returns the field names that compose this model's primary key.
	fn primary_key_fields() -> &'static [&'static str];

	/// Returns the target-neutral input kind when this model has one supported scalar primary key.
	fn primary_key_field_kind() -> Option<ModelFormFieldKind> {
		None
	}
}

#[cfg(test)]
mod tests {
	use crate::model_form::{
		ModelFormContract, ModelFormContractField, ModelFormContractSchema,
		ModelFormFieldDescriptor, ModelFormFieldKind, ModelFormPayload, ModelFormPayloadError,
		ModelFormPolicy, ModelFormPrimaryKey, ModelFormPrimaryKeyFields, ModelFormSchema,
	};

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

	#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
	enum PublicField {
		Title,
	}

	impl ModelFormContractField for PublicField {
		fn name(self) -> &'static str {
			match self {
				Self::Title => "title",
			}
		}
	}

	struct PublicPolicy;

	impl ModelFormPolicy for PublicPolicy {
		fn allows(field: &str) -> bool {
			field == "title"
		}
	}

	#[derive(Default, serde::Serialize, serde::Deserialize)]
	struct PublicData;

	impl ModelFormPayload<PublicPolicy> for PublicData {
		fn supplied_fields(&self) -> Vec<&'static str> {
			Vec::new()
		}

		fn forbidden_fields(&self) -> &[&'static str] {
			&[]
		}

		fn get_json(&self, _field: &str) -> Option<serde_json::Value> {
			None
		}

		fn set_json(
			&mut self,
			field: &str,
			_value: serde_json::Value,
		) -> Result<(), ModelFormPayloadError> {
			Err(ModelFormPayloadError::UnknownField {
				field: field.to_owned(),
			})
		}
	}

	struct PublicSchema;

	impl ModelFormContractSchema for PublicSchema {
		fn fields() -> &'static [ModelFormFieldDescriptor] {
			<LegacySchema as ModelFormSchema>::fields()
		}
	}

	struct PublicContract;

	impl ModelFormContract for PublicContract {
		type Data = PublicData;
		type Schema = PublicSchema;
		type Field = PublicField;
		type Policy = PublicPolicy;

		fn fields() -> &'static [Self::Field] {
			&[PublicField::Title]
		}
	}

	struct TextPrimaryKey;

	impl ModelFormPrimaryKey for TextPrimaryKey {
		const FIELD_KIND: ModelFormFieldKind = ModelFormFieldKind::Text {
			min_length: None,
			max_length: Some(64),
			multiline: false,
		};
	}

	impl ModelFormPrimaryKeyFields for TextPrimaryKey {
		fn primary_key_fields() -> &'static [&'static str] {
			&["id"]
		}
	}

	#[test]
	fn descriptor_keeps_required_and_default_independent() {
		let descriptor = ModelFormFieldDescriptor {
			name: "title",
			kind: ModelFormFieldKind::Text {
				min_length: None,
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
				min_length: None,
				max_length: Some(64),
				multiline: false,
			}
		);
	}

	#[test]
	fn primary_key_fields_support_composite_keys() {
		assert_eq!(TextPrimaryKey::primary_key_fields(), ["id"]);
	}

	#[test]
	fn legacy_schema_adapts_to_the_target_neutral_contract() {
		assert_eq!(
			<LegacySchema as ModelFormContractSchema>::fields(),
			<LegacySchema as ModelFormSchema>::fields(),
		);
	}

	#[test]
	fn named_contract_keeps_field_tokens_in_source_order() {
		assert_eq!(
			<PublicContract as ModelFormContract>::fields(),
			&[PublicField::Title],
		);
		assert_eq!(PublicField::Title.name(), "title");
	}
}
