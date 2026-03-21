//! Form IR types for form! macro.

use proc_macro2::Span;
use syn::Expr;

/// IR for a form! macro.
#[derive(Debug)]
pub struct FormIR {
	/// Form name
	pub name: String,
	/// Form action
	pub action: FormActionIR,
	/// HTTP method
	pub method: FormMethodIR,
	/// Form fields
	pub fields: Vec<FieldIR>,
	/// Form-level styling
	pub styling: FormStylingIR,
	/// Original span
	pub span: Span,
}

/// IR for form action.
#[derive(Debug)]
pub enum FormActionIR {
	/// URL action
	Url(String),
	/// Server function
	ServerFn(String),
}

/// IR for form method.
#[derive(Debug)]
pub enum FormMethodIR {
	/// HTTP GET method.
	Get,
	/// HTTP POST method.
	Post,
	/// HTTP PUT method.
	Put,
	/// HTTP PATCH method.
	Patch,
	/// HTTP DELETE method.
	Delete,
}

/// IR for a form field.
#[derive(Debug)]
pub struct FieldIR {
	/// Field name
	pub name: String,
	/// Field type
	pub field_type: FieldTypeIR,
	/// Field label
	pub label: Option<String>,
	/// Whether field is required
	pub required: bool,
	/// Validation rules
	pub validators: Vec<ValidatorIR>,
	/// Widget configuration
	pub widget: WidgetIR,
	/// Original span
	pub span: Span,
}

/// IR for field types.
#[derive(Debug)]
pub enum FieldTypeIR {
	/// Character/text field.
	CharField,
	/// Integer number field.
	IntegerField,
	/// Floating-point number field.
	FloatField,
	/// Boolean/checkbox field.
	BooleanField,
	/// Selection from predefined choices.
	ChoiceField,
	/// Date-only field.
	DateField,
	/// Date and time field.
	DateTimeField,
	/// Email address field.
	EmailField,
	/// URL field.
	UrlField,
	/// File upload field.
	FileField,
	/// Custom field type with the given type name.
	Custom(String),
}

/// IR for a validator.
#[derive(Debug)]
pub struct ValidatorIR {
	/// Validator expression
	pub expr: Expr,
	/// Error message
	pub message: String,
	/// Original span
	pub span: Span,
}

/// IR for widget configuration.
#[derive(Debug)]
pub struct WidgetIR {
	/// Widget type
	pub widget_type: WidgetTypeIR,
	/// Widget attributes
	pub attrs: Vec<(String, String)>,
}

/// IR for widget types.
#[derive(Debug)]
pub enum WidgetTypeIR {
	/// Text input widget.
	TextInput,
	/// Password input widget.
	PasswordInput,
	/// Multi-line text area widget.
	TextArea,
	/// Dropdown select widget.
	Select,
	/// Checkbox widget.
	Checkbox,
	/// Radio button widget.
	Radio,
	/// File upload widget.
	FileInput,
	/// Date picker widget.
	DateInput,
	/// Date-time picker widget.
	DateTimeInput,
	/// Hidden input widget.
	Hidden,
	/// Custom widget type with the given type name.
	Custom(String),
}

/// IR for form styling.
#[derive(Debug)]
pub struct FormStylingIR {
	/// CSS class for form
	pub class: Option<String>,
	/// Additional form attributes
	pub attrs: Vec<(String, String)>,
}
