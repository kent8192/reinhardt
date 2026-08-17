//! Model information types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Presentation style for an inline related-model form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InlineStyle {
	/// Render child rows in a compact table.
	Tabular,
	/// Render each child row as a labelled group.
	Stacked,
}

/// Values for one existing or blank inline child row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InlineRowInfo {
	/// Existing child primary key, or `None` for an extra row.
	pub id: Option<String>,
	/// Editable child values keyed by configured field name.
	pub values: HashMap<String, serde_json::Value>,
}

/// Form schema and rows for one configured inline child model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineFormInfo {
	/// Stable inline identifier used in control names.
	pub key: String,
	/// Child model display name.
	pub model_name: String,
	/// Inline presentation style.
	pub style: InlineStyle,
	/// Editable child field schema.
	pub fields: Vec<FieldInfo>,
	/// Existing rows followed by configured blank rows.
	pub rows: Vec<InlineRowInfo>,
	/// Whether existing rows may be changed; blank rows remain addable when false.
	#[serde(default)]
	pub can_change: bool,
	/// Whether existing rows may expose delete controls.
	pub can_delete: bool,
}

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

impl RelationOption {
	/// Create a relation option.
	pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
		Self {
			id: id.into(),
			label: label.into(),
		}
	}
}

/// Permission required to perform an admin action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelPermission {
	/// Permission to view model instances.
	View,
	/// Permission to add model instances.
	Add,
	/// Permission to change model instances.
	Change,
	/// Permission to delete model instances.
	Delete,
}

/// Metadata for an action available on an admin model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminAction {
	/// Machine-readable action name.
	pub name: String,
	/// Human-readable action label.
	pub label: String,
	/// Permission required to execute the action.
	pub permission: ModelPermission,
	/// Whether the action requires user confirmation.
	pub requires_confirmation: bool,
}

impl AdminAction {
	/// Creates metadata for an admin action.
	pub fn new(
		name: impl Into<String>,
		label: impl Into<String>,
		permission: ModelPermission,
		requires_confirmation: bool,
	) -> Self {
		Self {
			name: name.into(),
			label: label.into(),
			permission,
			requires_confirmation,
		}
	}
}

/// Result of executing an admin action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminActionOutcome {
	/// Canonical, deterministic, duplicate-free IDs changed by the action.
	pub successful_ids: Vec<String>,
	/// Total number of rows affected by the action.
	pub affected: u64,
}

impl AdminActionOutcome {
	/// Creates an admin action outcome.
	pub fn new(successful_ids: Vec<String>, affected: u64) -> Self {
		Self {
			successful_ids,
			affected,
		}
	}
}

/// A selected position within a date hierarchy.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateHierarchySelection {
	/// Selected year.
	pub year: Option<i32>,
	/// Selected month within [`Self::year`].
	pub month: Option<u32>,
	/// Selected day within [`Self::month`].
	pub day: Option<u32>,
}

/// The next date hierarchy granularity available for selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateHierarchyLevel {
	/// Select a year.
	Year,
	/// Select a month.
	Month,
	/// Select a day.
	Day,
}

/// Date hierarchy metadata included with a changelist response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateHierarchyInfo {
	/// Date or datetime field used for the hierarchy.
	pub field: String,
	/// Currently selected hierarchy position.
	pub selection: DateHierarchySelection,
	/// The next level the client may select.
	pub next_level: Option<DateHierarchyLevel>,
	/// Available values for [`Self::next_level`].
	pub choices: Vec<i32>,
}

/// Layout used to render a many-to-many relation selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationSelectorLayout {
	/// Side-by-side available and selected lists.
	Horizontal,
	/// Stacked available and selected lists.
	Vertical,
}

/// Closed widget configuration for an admin form field.
///
/// Runtime relation choices and selected values are resolved separately from
/// this declarative configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AdminWidget {
	/// Single-line text input.
	TextInput,
	/// Email input.
	EmailInput,
	/// Numeric input.
	NumberInput,
	/// Boolean checkbox.
	Checkbox,
	/// Date input.
	DateInput,
	/// Local date and time input.
	DateTimeInput,
	/// Multi-line text input.
	TextArea {
		/// Optional number of visible rows.
		rows: Option<u16>,
	},
	/// Single-value selection from explicit choices.
	Select {
		/// Available `(value, label)` choices.
		choices: Vec<(String, String)>,
	},
	/// Multiple-value selection from explicit choices.
	MultiSelect {
		/// Available `(value, label)` choices.
		choices: Vec<(String, String)>,
	},
	/// Permission-aware relation autocomplete control.
	Autocomplete,
	/// Direct relation primary-key control.
	RawId,
	/// Many-to-many relation selector.
	ManyToMany {
		/// Selector layout.
		layout: RelationSelectorLayout,
	},
	/// File upload input.
	FileInput,
	/// Hidden input.
	HiddenInput,
}

/// Optional presentation and requiredness overrides for one admin form field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormFieldOverride {
	/// Configured field name.
	pub field: String,
	/// Optional replacement widget.
	pub widget: Option<AdminWidget>,
	/// Optional display label.
	pub label: Option<String>,
	/// Optional help text.
	pub help_text: Option<String>,
	/// Optional input placeholder.
	pub placeholder: Option<String>,
	/// Optional requiredness override.
	pub required: Option<bool>,
}

impl FormFieldOverride {
	/// Create an empty override for a field.
	pub fn new(field: impl Into<String>) -> Self {
		Self {
			field: field.into(),
			widget: None,
			label: None,
			help_text: None,
			placeholder: None,
			required: None,
		}
	}

	/// Set the replacement widget.
	pub fn widget(mut self, widget: AdminWidget) -> Self {
		self.widget = Some(widget);
		self
	}

	/// Set the display label.
	pub fn label(mut self, label: impl Into<String>) -> Self {
		self.label = Some(label.into());
		self
	}

	/// Set the help text.
	pub fn help_text(mut self, help_text: impl Into<String>) -> Self {
		self.help_text = Some(help_text.into());
		self
	}

	/// Set the input placeholder.
	pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
		self.placeholder = Some(placeholder.into());
		self
	}

	/// Set the requiredness override.
	pub fn required(mut self, required: bool) -> Self {
		self.required = Some(required);
		self
	}
}

/// Client-side prepopulation rule for an admin form field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrepopulatedField {
	/// Field populated from the sources.
	pub target: String,
	/// Source fields in concatenation order.
	pub sources: Vec<String>,
}

impl PrepopulatedField {
	/// Create a prepopulation rule without validating metadata-dependent names.
	pub fn new<I, S>(target: impl Into<String>, sources: I) -> Self
	where
		I: IntoIterator<Item = S>,
		S: Into<String>,
	{
		Self {
			target: target.into(),
			sources: sources.into_iter().map(Into::into).collect(),
		}
	}
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
	/// Whether the field accepts an explicit null or clear value.
	#[serde(default)]
	pub nullable: bool,
	/// Whether the field is readonly
	pub readonly: bool,
	/// Help text displayed below the field
	pub help_text: Option<String>,
	/// Placeholder text for input
	pub placeholder: Option<String>,
}

/// A titled group of fields in an admin form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fieldset {
	/// Optional heading displayed for the group.
	pub title: Option<String>,
	/// Field names included in the group.
	pub fields: Vec<String>,
	/// Whether the group is collapsed initially.
	#[serde(default)]
	pub collapsed: bool,
}

impl Fieldset {
	/// Create an expanded fieldset from field names.
	pub fn new(title: Option<&str>, fields: &[&str]) -> Self {
		Self {
			title: title.map(String::from),
			fields: fields.iter().map(|field| String::from(*field)).collect(),
			collapsed: false,
		}
	}

	/// Mark the fieldset as initially collapsed.
	pub fn collapsed(mut self) -> Self {
		self.collapsed = true;
		self
	}
}

/// Field type for form rendering
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "options")]
pub enum FieldType {
	/// Text input (single line)
	Text,
	/// Textarea (multi-line)
	TextArea,
	/// Textarea (multi-line) with an explicit visible row count.
	///
	/// This public enum expansion is intentional: downstream exhaustive matches
	/// must handle this variant when supporting configured textarea rows.
	TextAreaWithRows {
		/// Optional number of visible rows.
		rows: Option<u16>,
	},
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
	/// Many-to-many relation selector.
	ManyToManySelector {
		/// Selector layout.
		layout: RelationSelectorLayout,
		/// Options available for selection.
		available: Vec<RelationOption>,
		/// Currently selected options.
		selected: Vec<RelationOption>,
		/// Last page consumed while prefetching available options.
		page: u64,
		/// Whether more available options can be loaded.
		has_more: bool,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
	/// `<textarea>` element with an explicit visible row count.
	///
	/// This public enum expansion is intentional: downstream exhaustive matches
	/// must handle this variant when supporting configured textarea rows.
	TextAreaWithRows {
		/// Optional number of visible rows.
		rows: Option<u16>,
	},
	/// `<textarea>` element whose value is encoded as structured JSON.
	Json,
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
	/// Many-to-many relation selector.
	ManyToManySelector {
		/// Selector layout.
		layout: RelationSelectorLayout,
		/// Options available for selection.
		available: Vec<RelationOption>,
		/// Currently selected options.
		selected: Vec<RelationOption>,
		/// Last page consumed while prefetching available options.
		page: u64,
		/// Whether more available options can be loaded.
		has_more: bool,
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
			FieldType::TextAreaWithRows { rows } => FormFieldSpec::TextAreaWithRows { rows: *rows },
			FieldType::Select { choices } => FormFieldSpec::Select {
				choices: choices.clone(),
			},
			FieldType::MultiSelect { choices } => FormFieldSpec::MultiSelect {
				choices: choices.clone(),
			},
			FieldType::ManyToManySelector {
				layout,
				available,
				selected,
				page,
				has_more,
			} => FormFieldSpec::ManyToManySelector {
				layout: *layout,
				available: available.clone(),
				selected: selected.clone(),
				page: *page,
				has_more: *has_more,
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

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn textarea_unit_variants_keep_the_legacy_wire_shape() {
		assert_eq!(
			serde_json::to_value(FieldType::TextArea).expect("field type should serialize"),
			json!({"type": "TextArea"})
		);
		assert_eq!(
			serde_json::from_value::<FieldType>(json!({"type": "TextArea"}))
				.expect("legacy field type should deserialize"),
			FieldType::TextArea
		);
		assert_eq!(
			serde_json::to_value(FormFieldSpec::TextArea).expect("form spec should serialize"),
			json!({"kind": "TextArea"})
		);
		assert_eq!(
			serde_json::from_value::<FormFieldSpec>(json!({"kind": "TextArea"}))
				.expect("legacy form spec should deserialize"),
			FormFieldSpec::TextArea
		);
	}

	#[test]
	fn many_to_many_selector_conversion_preserves_selector_data() {
		let field_type = FieldType::ManyToManySelector {
			layout: RelationSelectorLayout::Horizontal,
			available: vec![RelationOption::new("1", "Rust")],
			selected: vec![RelationOption::new("2", "WebAssembly")],
			page: 3,
			has_more: true,
		};

		assert_eq!(
			FormFieldSpec::from(&field_type),
			FormFieldSpec::ManyToManySelector {
				layout: RelationSelectorLayout::Horizontal,
				available: vec![RelationOption::new("1", "Rust")],
				selected: vec![RelationOption::new("2", "WebAssembly")],
				page: 3,
				has_more: true,
			}
		);
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
	/// Whether the column can be edited directly in the list view.
	#[serde(default)]
	pub editable: bool,
	/// Whether the column links to the row detail view.
	#[serde(default)]
	pub linked: bool,
	/// Whether an editable value is required.
	#[serde(default)]
	pub required: bool,
	/// Whether an editable value accepts the database NULL state.
	#[serde(default)]
	pub nullable: bool,
	/// Optional HTML numeric step for editable controls.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub step: Option<String>,
	/// Input rendering specification for editable columns.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub form_spec: Option<FormFieldSpec>,
}

#[cfg(all(test, server))]
mod relation_tests {
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
