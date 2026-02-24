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
	Get,
	Post,
	Put,
	Patch,
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
	CharField,
	IntegerField,
	FloatField,
	BooleanField,
	ChoiceField,
	DateField,
	DateTimeField,
	EmailField,
	UrlField,
	FileField,
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
	TextInput,
	PasswordInput,
	TextArea,
	Select,
	Checkbox,
	Radio,
	FileInput,
	DateInput,
	DateTimeInput,
	Hidden,
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
