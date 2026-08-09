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
	/// Whether existing rows may expose delete controls.
	pub can_delete: bool,
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
	/// Whether the column can be edited directly in the list view.
	#[serde(default)]
	pub editable: bool,
	/// Whether the column links to the row detail view.
	#[serde(default)]
	pub linked: bool,
	/// Whether an editable value is required.
	#[serde(default)]
	pub required: bool,
	/// Input rendering specification for editable columns.
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub form_spec: Option<FormFieldSpec>,
}
