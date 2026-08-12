//! Model information types

use serde::{Deserialize, Serialize};

/// Relation control rendered for a foreign-key field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationWidget {
	/// Searchable autocomplete control.
	Autocomplete,
	/// Direct primary-key input with a resolved label.
	RawId,
}

/// Display-safe option returned by a relation lookup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationOption {
	/// Related object's primary key.
	pub id: String,
	/// Related object's display label.
	pub label: String,
}

/// Model information for dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
	/// Model name
	pub name: String,
	/// List URL
	pub list_url: String,
}

/// Field metadata for dynamic form generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInfo {
	/// Field name (e.g., "username", "email")
	pub name: String,
	/// Display label (e.g., "Username", "Email Address")
	pub label: String,
	/// Field type
	pub field_type: FieldType,
	/// Whether the field is required
	pub required: bool,
	/// Whether the field is readonly
	pub readonly: bool,
	/// Help text displayed below the field
	pub help_text: Option<String>,
	/// Placeholder text for input
	pub placeholder: Option<String>,
}

/// Field type for form rendering
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "options")]
pub enum FieldType {
	/// Text input (single line)
	Text,
	/// Textarea (multi-line)
	TextArea,
	/// Number input
	Number,
	/// Boolean checkbox
	Boolean,
	/// Email input
	Email,
	/// Date input
	Date,
	/// DateTime input
	DateTime,
	/// Select dropdown with choices.
	Select {
		/// Available choices as `(value, label)` pairs.
		choices: Vec<(String, String)>,
	},
	/// Multiple select.
	MultiSelect {
		/// Available choices as `(value, label)` pairs.
		choices: Vec<(String, String)>,
	},
	/// Permission-aware foreign-key relation control.
	Relation {
		/// Logical relation field name from the model.
		field_name: String,
		/// Relation control kind.
		widget: RelationWidget,
		/// Current related option for edit forms.
		selected: Option<RelationOption>,
		/// Whether the relation is displayed without a submitted control.
		#[serde(default)]
		readonly: bool,
	},
	/// File upload
	File,
	/// Hidden field
	Hidden,
}

/// Rendering specification for a form field.
///
/// This type preserves the structural information needed to emit the
/// correct HTML element (e.g., `<input>`, `<textarea>`, `<select>`),
/// along with any choices required for `<select>` options. It is derived
/// from `FieldType` via `From<&FieldType>`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum FormFieldSpec {
	/// Plain `<input>` element with the given HTML `type` attribute.
	Input {
		/// Value for the HTML `type` attribute (e.g., "text", "email",
		/// "number", "checkbox", "date", "datetime-local").
		///
		/// Owned `String` (not `&'static str`) so the variant can round-trip
		/// through `serde` deserialization at API boundaries — borrowed
		/// `'static` strings cannot be reconstructed from incoming JSON.
		html_type: String,
	},
	/// `<textarea>` element for multi-line text.
	TextArea,
	/// `<select>` dropdown with the given `(value, label)` choices.
	Select {
		/// Available choices as `(value, label)` pairs.
		choices: Vec<(String, String)>,
	},
	/// `<select multiple>` dropdown with the given `(value, label)` choices.
	MultiSelect {
		/// Available choices as `(value, label)` pairs.
		choices: Vec<(String, String)>,
	},
	/// Permission-aware foreign-key relation control.
	Relation {
		/// Logical relation field name from the model.
		field_name: String,
		/// Relation control kind.
		widget: RelationWidget,
		/// Current related option for edit forms.
		selected: Option<RelationOption>,
		/// Whether the relation is displayed without a submitted control.
		#[serde(default)]
		readonly: bool,
	},
	/// `<input type="file">` for file uploads.
	File,
	/// `<input type="hidden">` for hidden values.
	Hidden,
}

impl From<&FieldType> for FormFieldSpec {
	fn from(field_type: &FieldType) -> Self {
		match field_type {
			FieldType::Text => FormFieldSpec::Input {
				html_type: "text".to_string(),
			},
			FieldType::Number => FormFieldSpec::Input {
				html_type: "number".to_string(),
			},
			FieldType::Boolean => FormFieldSpec::Input {
				html_type: "checkbox".to_string(),
			},
			FieldType::Email => FormFieldSpec::Input {
				html_type: "email".to_string(),
			},
			FieldType::Date => FormFieldSpec::Input {
				html_type: "date".to_string(),
			},
			FieldType::DateTime => FormFieldSpec::Input {
				html_type: "datetime-local".to_string(),
			},
			FieldType::TextArea => FormFieldSpec::TextArea,
			FieldType::Select { choices } => FormFieldSpec::Select {
				choices: choices.clone(),
			},
			FieldType::MultiSelect { choices } => FormFieldSpec::MultiSelect {
				choices: choices.clone(),
			},
			FieldType::Relation {
				field_name,
				widget,
				selected,
				readonly,
			} => FormFieldSpec::Relation {
				field_name: field_name.clone(),
				widget: *widget,
				selected: selected.clone(),
				readonly: *readonly,
			},
			FieldType::File => FormFieldSpec::File,
			FieldType::Hidden => FormFieldSpec::Hidden,
		}
	}
}

/// Filter type for UI rendering
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "options")]
pub enum FilterType {
	/// Boolean filter (Yes/No checkbox)
	Boolean,
	/// Choice filter (dropdown with predefined options).
	Choice {
		/// Available filter choices.
		choices: Vec<FilterChoice>,
	},
	/// Date range filter (predefined ranges like "Today", "Last 7 days").
	DateRange {
		/// Available date range options.
		ranges: Vec<FilterChoice>,
	},
	/// Number range filter (predefined ranges).
	NumberRange {
		/// Available number range options.
		ranges: Vec<FilterChoice>,
	},
}

/// Filter choice for Choice/DateRange/NumberRange filters
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilterChoice {
	/// Value to send to API
	pub value: String,
	/// Display label for UI
	pub label: String,
}

/// Filter metadata sent from backend to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterInfo {
	/// Field name (e.g., "status", "is_active")
	pub field: String,
	/// Display title (e.g., "Status", "Active")
	pub title: String,
	/// Filter type and options
	pub filter_type: FilterType,
	/// Current value (if filter is active)
	pub current_value: Option<String>,
}

/// Column metadata for list view display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
	/// Field name to extract from data
	pub field: String,
	/// Display label for column header
	pub label: String,
	/// Whether column is sortable
	pub sortable: bool,
}

#[cfg(all(test, server))]
mod tests {
	use super::*;
	use rstest::rstest;
	use serde_json::json;

	#[rstest]
	#[case(RelationWidget::Autocomplete, json!("autocomplete"))]
	#[case(RelationWidget::RawId, json!("raw_id"))]
	fn relation_widget_uses_stable_wire_values(
		#[case] widget: RelationWidget,
		#[case] expected: serde_json::Value,
	) {
		// Act
		let serialized = serde_json::to_value(widget).expect("relation widget should serialize");

		// Assert
		assert_eq!(serialized, expected);
	}

	#[rstest]
	fn relation_option_round_trips() {
		// Arrange
		let option = RelationOption {
			id: "42".to_string(),
			label: "Ada Lovelace".to_string(),
		};

		// Act
		let serialized = serde_json::to_value(&option).expect("relation option should serialize");
		let deserialized: RelationOption =
			serde_json::from_value(serialized.clone()).expect("relation option should deserialize");

		// Assert
		assert_eq!(serialized, json!({"id": "42", "label": "Ada Lovelace"}));
		assert_eq!(deserialized, option);
	}

	#[rstest]
	fn relation_field_type_serializes_and_converts_without_losing_metadata() {
		// Arrange
		let selected = RelationOption {
			id: "7".to_string(),
			label: "Grace Hopper".to_string(),
		};
		let field_type = FieldType::Relation {
			field_name: "author".to_string(),
			widget: RelationWidget::Autocomplete,
			selected: Some(selected.clone()),
			readonly: false,
		};

		// Act
		let serialized =
			serde_json::to_value(&field_type).expect("relation field type should serialize");
		let form_spec = FormFieldSpec::from(&field_type);

		// Assert
		assert_eq!(
			serialized,
			json!({
				"type": "Relation",
				"options": {
					"field_name": "author",
					"widget": "autocomplete",
					"selected": {"id": "7", "label": "Grace Hopper"},
					"readonly": false
				}
			})
		);
		assert_eq!(
			form_spec,
			FormFieldSpec::Relation {
				field_name: "author".to_string(),
				widget: RelationWidget::Autocomplete,
				selected: Some(selected),
				readonly: false,
			}
		);
	}
}
