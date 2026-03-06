//! Typed AST node definitions for the form! macro.
//!
//! This module provides typed versions of form AST nodes, where property values
//! have explicit type information. This enables stronger compile-time validation
//! and better error messages.
//!
//! The typed AST is produced by the validator after transforming and validating
//! the untyped AST from the parser.
//!
//! See [`TypedFormMacro`] for the type hierarchy diagram.
//!
//! # Type Summary
//!
//! | Category | Types |
//! |----------|-------|
//! | Core | `TypedFormMacro`, `TypedFormAction`, `FormMethod` |
//! | Fields | `TypedFormFieldEntry`, `TypedFormFieldDef`, `TypedFormFieldGroup` |
//! | Field Types | `TypedFieldType`, `TypedWidget` |
//! | Properties | `TypedFieldValidation`, `TypedFieldDisplay`, `TypedFieldStyling` |
//! | State | `TypedFormState`, `TypedFormCallbacks`, `TypedFormWatch` |
//! | Customization | `TypedWrapper`, `TypedIcon`, `TypedCustomAttr`, `TypedFormSlots` |
//! | Validation | `TypedFormValidator`, `TypedClientValidator` |
//!
//! # Transformation Flow
//!
//! The validator transforms the untyped AST (`form_node.rs`) into typed AST:
//!
//! 1. **Parsing**: `form!` → `FormMacro` (untyped)
//! 2. **Validation**: Semantic checks (field types, widget compatibility, etc.)
//! 3. **Transformation**: `FormMacro` → `TypedFormMacro` (typed)
//! 4. **Code Generation**: `TypedFormMacro` → Rust code

use proc_macro2::Span;
use syn::{ExprClosure, Ident, Path};

#[cfg_attr(doc, aquamarine::aquamarine)]
/// The top-level typed AST node representing a validated form! macro invocation.
///
/// This is the result of successful validation and transformation of an untyped
/// `FormMacro`. All validation rules have been enforced at this point.
///
/// # Type Hierarchy
///
/// ```mermaid
/// classDiagram
///     class TypedFormMacro {
///         +Ident name
///         +TypedFormAction action
///         +FormMethod method
///         +TypedFormStyling styling
///         +Option~TypedFormState~ state
///         +TypedFormCallbacks callbacks
///         +Option~TypedFormWatch~ watch
///         +Vec~TypedFormFieldEntry~ fields
///         +Vec~TypedFormValidator~ validators
///     }
///
///     TypedFormMacro --> TypedFormAction
///     TypedFormMacro --> TypedFormStyling
///     TypedFormMacro --> TypedFormCallbacks
///     TypedFormMacro --> TypedFormFieldEntry
///
///     class TypedFormFieldEntry {
///         <<enumeration>>
///         Field~TypedFormFieldDef~
///         Group~TypedFormFieldGroup~
///     }
///
///     TypedFormFieldEntry --> TypedFormFieldDef
///     TypedFormFieldEntry --> TypedFormFieldGroup
///
///     class TypedFormFieldDef {
///         +TypedFieldType field_type
///         +TypedFieldValidation validation
///         +TypedFieldDisplay display
///         +TypedWidget widget
///     }
/// ```
#[derive(Debug)]
pub struct TypedFormMacro {
	/// Form struct name (validated identifier)
	pub name: Ident,
	/// Validated form action configuration
	pub action: TypedFormAction,
	/// HTTP method (validated, defaults to Post)
	pub method: FormMethod,
	/// Form-level styling configuration
	pub styling: TypedFormStyling,
	/// UI state configuration (loading, error, success signals)
	pub state: Option<TypedFormState>,
	/// Validated form submission callbacks
	pub callbacks: TypedFormCallbacks,
	/// Validated watch block for reactive computed views
	pub watch: Option<TypedFormWatch>,
	/// Validated derived/computed values block for reactive signals
	pub derived: Option<TypedFormDerived>,
	/// Redirect URL on successful form submission
	///
	/// Validated to start with `/` or be a valid URL pattern.
	/// Supports parameter expansion with `{param}` syntax.
	pub redirect_on_success: Option<String>,
	/// Initial value loader server_fn
	///
	/// When specified, generates an async method to load initial values.
	pub initial_loader: Option<Path>,
	/// Choices loader server_fn for dynamic `ChoiceField`
	///
	/// When specified, generates an async method to load choice options
	/// for fields that have `choices_from` specified.
	pub choices_loader: Option<Path>,
	/// Slot definitions for custom UI elements
	pub slots: Option<TypedFormSlots>,
	/// Validated field definitions (can include field groups)
	pub fields: Vec<TypedFormFieldEntry>,
	/// Validated server-side validators
	pub validators: Vec<TypedFormValidator>,
	/// Validated client-side validators
	pub client_validators: Vec<TypedClientValidator>,
	/// Span for error reporting
	pub span: Span,
}

/// Typed form action configuration.
///
/// Validated to ensure exactly one action method is specified.
#[derive(Debug, Clone)]
pub enum TypedFormAction {
	/// URL action with validated string
	Url(String),
	/// server_fn action with validated path
	ServerFn(Path),
	/// No action specified (form handles submission manually)
	None,
}

/// HTTP method for form submission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FormMethod {
	#[default]
	Post,
	Get,
	Put,
	Delete,
	Patch,
}

impl FormMethod {
	/// Returns the HTTP method string.
	pub fn as_str(&self) -> &'static str {
		match self {
			FormMethod::Post => "POST",
			FormMethod::Get => "GET",
			FormMethod::Put => "PUT",
			FormMethod::Delete => "DELETE",
			FormMethod::Patch => "PATCH",
		}
	}

	/// Returns the lowercase method string for HTML forms.
	pub fn as_html_method(&self) -> &'static str {
		match self {
			FormMethod::Post => "post",
			FormMethod::Get => "get",
			// HTML forms only support GET and POST natively
			// PUT/DELETE/PATCH need JavaScript handling
			FormMethod::Put => "post",
			FormMethod::Delete => "post",
			FormMethod::Patch => "post",
		}
	}

	/// Returns true if this method requires JavaScript handling.
	pub fn requires_js(&self) -> bool {
		matches!(
			self,
			FormMethod::Put | FormMethod::Delete | FormMethod::Patch
		)
	}
}

/// Form-level styling configuration.
#[derive(Debug, Clone, Default)]
pub struct TypedFormStyling {
	/// Form element CSS class
	pub class: Option<String>,
}

/// Validated UI state configuration for form submission.
///
/// This represents the validated state configuration after checking
/// that all field names are valid (`loading`, `error`, `success`).
///
/// ## Generated Signals
///
/// | Field | Signal Type | Description |
/// |-------|-------------|-------------|
/// | `loading` | `Signal<bool>` | True during form submission |
/// | `error` | `Signal<Option<String>>` | Contains error message if submission failed |
/// | `success` | `Signal<bool>` | True after successful submission |
#[derive(Debug, Clone)]
pub struct TypedFormState {
	/// Whether loading state is enabled
	pub loading: bool,
	/// Whether error state is enabled
	pub error: bool,
	/// Whether success state is enabled
	pub success: bool,
	/// Span for error reporting
	pub span: Span,
}

impl TypedFormState {
	/// Creates a new TypedFormState with all states disabled.
	pub fn new(span: Span) -> Self {
		Self {
			loading: false,
			error: false,
			success: false,
			span,
		}
	}

	/// Returns true if any state field is enabled.
	pub fn has_any(&self) -> bool {
		self.loading || self.error || self.success
	}

	/// Returns true if loading state is enabled.
	pub fn has_loading(&self) -> bool {
		self.loading
	}

	/// Returns true if error state is enabled.
	pub fn has_error(&self) -> bool {
		self.error
	}

	/// Returns true if success state is enabled.
	pub fn has_success(&self) -> bool {
		self.success
	}
}

/// Validated form submission callbacks.
///
/// This holds the validated callback closures that are called at different
/// stages of form submission. The closures are stored as-is from parsing,
/// as their validation is done during type checking.
///
/// ## Callback Signatures
///
/// | Callback | Signature | Description |
/// |----------|-----------|-------------|
/// | `on_submit` | `\|form: &Self\| { ... }` | Called before submission starts |
/// | `on_success` | `\|result: T\| { ... }` | Called when server_fn returns successfully |
/// | `on_error` | `\|error: ServerFnError\| { ... }` | Called when submission fails |
/// | `on_loading` | `\|is_loading: bool\| { ... }` | Called when loading state changes |
#[derive(Debug, Clone, Default)]
pub struct TypedFormCallbacks {
	/// Callback called before form submission starts.
	pub on_submit: Option<ExprClosure>,
	/// Callback called when submission succeeds.
	pub on_success: Option<ExprClosure>,
	/// Callback called when submission fails.
	pub on_error: Option<ExprClosure>,
	/// Callback called when loading state changes.
	pub on_loading: Option<ExprClosure>,
	/// Span for error reporting (from first callback)
	pub span: Option<Span>,
}

impl TypedFormCallbacks {
	/// Creates a new empty TypedFormCallbacks.
	pub fn new() -> Self {
		Self::default()
	}

	/// Returns true if any callback is defined.
	pub fn has_any(&self) -> bool {
		self.on_submit.is_some()
			|| self.on_success.is_some()
			|| self.on_error.is_some()
			|| self.on_loading.is_some()
	}

	/// Returns true if on_submit callback is defined.
	pub fn has_on_submit(&self) -> bool {
		self.on_submit.is_some()
	}

	/// Returns true if on_success callback is defined.
	pub fn has_on_success(&self) -> bool {
		self.on_success.is_some()
	}

	/// Returns true if on_error callback is defined.
	pub fn has_on_error(&self) -> bool {
		self.on_error.is_some()
	}

	/// Returns true if on_loading callback is defined.
	pub fn has_on_loading(&self) -> bool {
		self.on_loading.is_some()
	}
}

/// Validated watch block containing named reactive closures.
///
/// Watch items generate methods on the form struct that return Views
/// which automatically re-render when their Signal dependencies change.
///
/// ## Example Generated Code
///
/// ```ignore
/// impl LoginForm {
///     pub fn error_display(&self) -> impl IntoView {
///         let form = self.clone();
///         Effect::new(move || {
///             // closure body from watch item
///         })
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct TypedFormWatch {
	/// List of validated watch items
	pub items: Vec<TypedFormWatchItem>,
	/// Span for error reporting
	pub span: Span,
}

impl TypedFormWatch {
	/// Creates a new empty TypedFormWatch.
	pub fn new(span: Span) -> Self {
		Self {
			items: Vec::new(),
			span,
		}
	}

	/// Returns true if no watch items are defined.
	pub fn is_empty(&self) -> bool {
		self.items.is_empty()
	}
}

/// A validated watch item with name and closure.
///
/// Each watch item generates a method on the form struct that returns
/// a reactive view. The closure receives the form instance and can access
/// any Signals to create reactive dependencies.
#[derive(Debug, Clone)]
pub struct TypedFormWatchItem {
	/// Watch item name (becomes method name on form struct)
	pub name: Ident,
	/// Validated closure that generates the View.
	/// The closure receives `&FormName` as parameter.
	pub closure: ExprClosure,
	/// Span for error reporting
	pub span: Span,
}

/// Validated derived/computed values block.
///
/// Derived items generate `Memo<T>` accessors on the form struct that
/// automatically recompute when their Signal dependencies change.
/// Unlike watch blocks which produce Views, derived blocks produce values.
///
/// ## Generated Code Example
///
/// ```ignore
/// // Given:
/// derived: {
///     char_count: |form| form.content().get().len(),
/// }
///
/// // Generates:
/// impl TweetForm {
///     pub fn char_count(&self) -> Memo<usize> {
///         let form = self.clone();
///         Memo::new(move || form.content().get().len())
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct TypedFormDerived {
	/// List of validated derived items
	pub items: Vec<TypedDerivedItem>,
	/// Span for error reporting
	pub span: Span,
}

/// A validated derived item with name and closure.
///
/// Each derived item generates a method on the form struct that returns
/// a `Memo<T>`. The closure receives `&FormName` as parameter and computes
/// the derived value, which is automatically memoized.
#[derive(Debug, Clone)]
pub struct TypedDerivedItem {
	/// Derived item name (becomes method name on form struct)
	pub name: Ident,
	/// Validated closure that computes the derived value.
	/// The closure receives `&FormName` as parameter and returns `T`.
	pub closure: ExprClosure,
	/// Span for error reporting
	pub span: Span,
}

/// Validated slot definitions for custom UI elements in the form.
///
/// Slots allow inserting custom content before and after form fields.
#[derive(Debug, Clone)]
pub struct TypedFormSlots {
	/// Closure rendered before all fields
	pub before_fields: Option<ExprClosure>,
	/// Closure rendered after all fields
	pub after_fields: Option<ExprClosure>,
	/// Span for error reporting
	pub span: Span,
}

impl TypedFormSlots {
	/// Creates a new empty TypedFormSlots.
	pub fn new(span: Span) -> Self {
		Self {
			before_fields: None,
			after_fields: None,
			span,
		}
	}

	/// Returns true if no slots are defined.
	pub fn is_empty(&self) -> bool {
		self.before_fields.is_none() && self.after_fields.is_none()
	}
}

/// Configuration for dynamic choice field options.
///
/// Used with `ChoiceField` to configure how choices are loaded
/// and displayed from a `choices_loader` server_fn result.
#[derive(Debug, Clone)]
pub struct TypedChoicesConfig {
	/// Field name in the loader result containing the choice array
	///
	/// e.g., "choices" extracts data from `response.choices`
	pub choices_from: String,
	/// Property path for extracting the value from each choice item
	///
	/// e.g., "id" extracts `choice.id` as the form value.
	/// Defaults to "value" if not specified.
	pub choice_value: String,
	/// Property path for extracting the display label from each choice item
	///
	/// e.g., "choice_text" extracts `choice.choice_text` for display.
	/// Defaults to "label" if not specified.
	pub choice_label: String,
	/// Span for error reporting
	pub span: Span,
}

impl TypedChoicesConfig {
	/// Creates a new choices config with required choices_from.
	pub fn new(choices_from: String, span: Span) -> Self {
		Self {
			choices_from,
			choice_value: "value".to_string(),
			choice_label: "label".to_string(),
			span,
		}
	}

	/// Creates a new choices config with all fields specified.
	pub fn with_paths(
		choices_from: String,
		choice_value: String,
		choice_label: String,
		span: Span,
	) -> Self {
		Self {
			choices_from,
			choice_value,
			choice_label,
			span,
		}
	}
}

/// A validated field definition with typed properties.
#[derive(Debug)]
pub struct TypedFormFieldDef {
	/// Field name identifier
	pub name: Ident,
	/// Validated field type
	pub field_type: TypedFieldType,
	/// Validation properties
	pub validation: TypedFieldValidation,
	/// Display properties
	pub display: TypedFieldDisplay,
	/// Styling properties
	pub styling: TypedFieldStyling,
	/// Widget type
	pub widget: TypedWidget,
	/// Custom wrapper element for the field container
	pub wrapper: Option<TypedWrapper>,
	/// SVG icon for the field
	pub icon: Option<TypedIcon>,
	/// Custom attributes (aria-*, data-*)
	pub custom_attrs: Vec<TypedCustomAttr>,
	/// Two-way binding enabled (default: true)
	///
	/// When true, the form automatically generates an @input handler
	/// to update the Signal when the user types. Set to false to disable
	/// automatic binding and use a custom handler instead.
	pub bind: bool,
	/// Initial value source field name
	///
	/// Maps this field to a property in the data returned by `initial_loader`.
	pub initial_from: Option<String>,
	/// Dynamic choices configuration for `ChoiceField`
	///
	/// When specified, the field will load choices from a `choices_loader`
	/// server_fn and render them dynamically.
	pub choices_config: Option<TypedChoicesConfig>,
	/// Span for error reporting
	pub span: Span,
}

/// An entry in the typed form fields list.
///
/// Can be either a regular field definition or a field group
/// containing multiple related fields.
#[derive(Debug)]
pub enum TypedFormFieldEntry {
	/// A single field definition
	///
	/// Boxed to reduce enum size difference between variants.
	Field(Box<TypedFormFieldDef>),
	/// A group of related fields
	Group(TypedFormFieldGroup),
}

impl TypedFormFieldEntry {
	/// Returns true if this is a field group.
	pub fn is_group(&self) -> bool {
		matches!(self, TypedFormFieldEntry::Group(_))
	}

	/// Returns true if this is a regular field.
	pub fn is_field(&self) -> bool {
		matches!(self, TypedFormFieldEntry::Field(_))
	}

	/// Returns the name of the entry.
	pub fn name(&self) -> &Ident {
		match self {
			TypedFormFieldEntry::Field(f) => &f.as_ref().name,
			TypedFormFieldEntry::Group(g) => &g.name,
		}
	}

	/// Returns the span for error reporting.
	pub fn span(&self) -> Span {
		match self {
			TypedFormFieldEntry::Field(f) => f.as_ref().span,
			TypedFormFieldEntry::Group(g) => g.span,
		}
	}

	/// Returns a reference to the inner field if this is a Field variant.
	pub fn as_field(&self) -> Option<&TypedFormFieldDef> {
		match self {
			TypedFormFieldEntry::Field(f) => Some(f.as_ref()),
			TypedFormFieldEntry::Group(_) => None,
		}
	}

	/// Returns a reference to the inner group if this is a Group variant.
	pub fn as_group(&self) -> Option<&TypedFormFieldGroup> {
		match self {
			TypedFormFieldEntry::Field(_) => None,
			TypedFormFieldEntry::Group(g) => Some(g),
		}
	}
}

/// A validated group of related fields.
///
/// Field groups allow organizing multiple fields under a common container
/// with shared styling and an optional label.
#[derive(Debug)]
pub struct TypedFormFieldGroup {
	/// Group name identifier
	pub name: Ident,
	/// Group label text (for display)
	pub label: Option<String>,
	/// Group CSS class
	pub class: Option<String>,
	/// Validated fields within the group
	pub fields: Vec<TypedFormFieldDef>,
	/// Span for error reporting
	pub span: Span,
}

impl TypedFormFieldGroup {
	/// Returns the number of fields in this group.
	pub fn field_count(&self) -> usize {
		self.fields.len()
	}
}

/// Validated field types with their associated Signal types.
///
/// Each field type maps to a specific Rust type and Signal wrapper.
/// The mapping ensures type safety between the form DSL and generated code.
///
/// # Field Type Mapping
///
/// | Field Type | Rust Type | Signal Type |
/// |------------|-----------|-------------|
/// | `CharField` | `String` | `Signal<String>` |
/// | `EmailField` | `String` | `Signal<String>` |
/// | `PasswordField` | `String` | `Signal<String>` |
/// | `TextField` | `String` | `Signal<String>` |
/// | `IntegerField` | `i64` | `Signal<i64>` |
/// | `FloatField` | `f64` | `Signal<f64>` |
/// | `BooleanField` | `bool` | `Signal<bool>` |
/// | `DateField` | `Option<NaiveDate>` | `Signal<Option<NaiveDate>>` |
/// | `TimeField` | `Option<NaiveTime>` | `Signal<Option<NaiveTime>>` |
/// | `DateTimeField` | `Option<NaiveDateTime>` | `Signal<Option<NaiveDateTime>>` |
/// | `ChoiceField` | `String` | `Signal<String>` |
/// | `MultipleChoiceField` | `Vec<String>` | `Signal<Vec<String>>` |
/// | `FileField` | `Option<File>` | `Signal<Option<File>>` |
/// | `HiddenField` | `String` | `Signal<String>` |
#[derive(Debug, Clone)]
pub enum TypedFieldType {
	/// CharField -> `Signal<String>`
	CharField,
	/// EmailField -> `Signal<String>`
	EmailField,
	/// UrlField -> `Signal<String>`
	UrlField,
	/// SlugField -> `Signal<String>`
	SlugField,
	/// TextField -> `Signal<String>`
	TextField,
	/// IntegerField -> `Signal<i64>`
	IntegerField,
	/// FloatField -> `Signal<f64>`
	FloatField,
	/// DecimalField -> `Signal<String>` (for precision)
	DecimalField,
	/// BooleanField -> `Signal<bool>`
	BooleanField,
	/// DateField -> `Signal<Option<NaiveDate>>`
	DateField,
	/// TimeField -> `Signal<Option<NaiveTime>>`
	TimeField,
	/// DateTimeField -> `Signal<Option<NaiveDateTime>>`
	DateTimeField,
	/// ChoiceField -> `Signal<String>`
	ChoiceField,
	/// MultipleChoiceField -> `Signal<Vec<String>>`
	MultipleChoiceField,
	/// FileField -> `Signal<Option<File>>`
	FileField,
	/// ImageField -> `Signal<Option<File>>`
	ImageField,
	/// HiddenField -> `Signal<String>`
	HiddenField,
	/// PasswordField -> `Signal<String>`
	PasswordField,
	/// UUIDField -> `Signal<String>`
	UuidField,
	/// JsonField -> `Signal<String>`
	JsonField,
	/// IpAddressField -> `Signal<String>`
	IpAddressField,
}

impl TypedFieldType {
	/// Returns the Rust type used in the generated struct.
	pub fn rust_type(&self) -> &'static str {
		match self {
			TypedFieldType::CharField
			| TypedFieldType::EmailField
			| TypedFieldType::UrlField
			| TypedFieldType::SlugField
			| TypedFieldType::TextField
			| TypedFieldType::DecimalField
			| TypedFieldType::ChoiceField
			| TypedFieldType::HiddenField
			| TypedFieldType::PasswordField
			| TypedFieldType::UuidField
			| TypedFieldType::JsonField
			| TypedFieldType::IpAddressField => "String",
			TypedFieldType::IntegerField => "i64",
			TypedFieldType::FloatField => "f64",
			TypedFieldType::BooleanField => "bool",
			TypedFieldType::DateField => "Option<chrono::NaiveDate>",
			TypedFieldType::TimeField => "Option<chrono::NaiveTime>",
			TypedFieldType::DateTimeField => "Option<chrono::NaiveDateTime>",
			TypedFieldType::MultipleChoiceField => "Vec<String>",
			TypedFieldType::FileField | TypedFieldType::ImageField => "Option<web_sys::File>",
		}
	}

	/// Returns the default Signal wrapper type.
	pub fn signal_type(&self) -> String {
		format!("Signal<{}>", self.rust_type())
	}

	/// Returns the default widget for this field type.
	pub fn default_widget(&self) -> TypedWidget {
		match self {
			TypedFieldType::CharField
			| TypedFieldType::SlugField
			| TypedFieldType::UuidField
			| TypedFieldType::IpAddressField => TypedWidget::TextInput,
			TypedFieldType::EmailField => TypedWidget::EmailInput,
			TypedFieldType::UrlField => TypedWidget::UrlInput,
			TypedFieldType::TextField | TypedFieldType::JsonField => TypedWidget::Textarea,
			TypedFieldType::IntegerField
			| TypedFieldType::FloatField
			| TypedFieldType::DecimalField => TypedWidget::NumberInput,
			TypedFieldType::BooleanField => TypedWidget::CheckboxInput,
			TypedFieldType::DateField => TypedWidget::DateInput,
			TypedFieldType::TimeField => TypedWidget::TimeInput,
			TypedFieldType::DateTimeField => TypedWidget::DateTimeInput,
			TypedFieldType::ChoiceField => TypedWidget::Select,
			TypedFieldType::MultipleChoiceField => TypedWidget::SelectMultiple,
			TypedFieldType::FileField | TypedFieldType::ImageField => TypedWidget::FileInput,
			TypedFieldType::HiddenField => TypedWidget::HiddenInput,
			TypedFieldType::PasswordField => TypedWidget::PasswordInput,
		}
	}
}

/// Validated widget types with their HTML element mappings.
///
/// Each widget type maps to a specific HTML element and input type.
/// The code generator uses this mapping to produce the correct HTML output.
///
/// # Widget HTML Mapping
///
/// | Widget | HTML Tag | Input Type |
/// |--------|----------|------------|
/// | `TextInput` | `<input>` | `text` |
/// | `EmailInput` | `<input>` | `email` |
/// | `PasswordInput` | `<input>` | `password` |
/// | `NumberInput` | `<input>` | `number` |
/// | `DateInput` | `<input>` | `date` |
/// | `TimeInput` | `<input>` | `time` |
/// | `DateTimeInput` | `<input>` | `datetime-local` |
/// | `CheckboxInput` | `<input>` | `checkbox` |
/// | `RadioInput` | `<input>` | `radio` |
/// | `FileInput` | `<input>` | `file` |
/// | `HiddenInput` | `<input>` | `hidden` |
/// | `Textarea` | `<textarea>` | - |
/// | `Select` | `<select>` | - |
/// | `SelectMultiple` | `<select multiple>` | - |
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedWidget {
	TextInput,
	EmailInput,
	PasswordInput,
	NumberInput,
	UrlInput,
	TelInput,
	DateInput,
	TimeInput,
	DateTimeInput,
	ColorInput,
	RangeInput,
	HiddenInput,
	Textarea,
	Select,
	SelectMultiple,
	CheckboxInput,
	RadioInput,
	RadioSelect,
	FileInput,
	SearchInput,
}

impl TypedWidget {
	/// Returns the HTML input type attribute value.
	pub fn html_type(&self) -> &'static str {
		match self {
			TypedWidget::TextInput => "text",
			TypedWidget::EmailInput => "email",
			TypedWidget::PasswordInput => "password",
			TypedWidget::NumberInput => "number",
			TypedWidget::UrlInput => "url",
			TypedWidget::TelInput => "tel",
			TypedWidget::DateInput => "date",
			TypedWidget::TimeInput => "time",
			TypedWidget::DateTimeInput => "datetime-local",
			TypedWidget::ColorInput => "color",
			TypedWidget::RangeInput => "range",
			TypedWidget::HiddenInput => "hidden",
			TypedWidget::CheckboxInput => "checkbox",
			TypedWidget::RadioInput | TypedWidget::RadioSelect => "radio",
			TypedWidget::FileInput => "file",
			TypedWidget::SearchInput => "search",
			// These are not input types
			TypedWidget::Textarea => "text",
			TypedWidget::Select => "text",
			TypedWidget::SelectMultiple => "text",
		}
	}

	/// Returns true if this widget uses an <input> element.
	pub fn is_input(&self) -> bool {
		!matches!(
			self,
			TypedWidget::Textarea | TypedWidget::Select | TypedWidget::SelectMultiple
		)
	}

	/// Returns the HTML tag name for this widget.
	pub fn html_tag(&self) -> &'static str {
		match self {
			TypedWidget::Textarea => "textarea",
			TypedWidget::Select | TypedWidget::SelectMultiple => "select",
			_ => "input",
		}
	}
}

/// Validation-related properties of a field.
///
/// These properties control HTML5 validation attributes and server-side validation rules.
///
/// # Validation Properties
///
/// | Property | Type | HTML Attribute | Description |
/// |----------|------|----------------|-------------|
/// | `required` | `bool` | `required` | Field must have a value |
/// | `min_length` | `Option<i64>` | `minlength` | Minimum string length |
/// | `max_length` | `Option<i64>` | `maxlength` | Maximum string length |
/// | `min_value` | `Option<i64>` | `min` | Minimum numeric value |
/// | `max_value` | `Option<i64>` | `max` | Maximum numeric value |
/// | `pattern` | `Option<String>` | `pattern` | Regex pattern for validation |
#[derive(Debug, Clone, Default)]
pub struct TypedFieldValidation {
	/// Whether the field is required
	pub required: bool,
	/// Maximum length constraint
	pub max_length: Option<i64>,
	/// Minimum length constraint
	pub min_length: Option<i64>,
	/// Minimum value constraint (for numeric fields)
	pub min_value: Option<i64>,
	/// Maximum value constraint (for numeric fields)
	pub max_value: Option<i64>,
	/// Regex pattern for validation
	pub pattern: Option<String>,
}

/// Display-related properties of a field.
///
/// These properties control the visual appearance and behavior of form fields.
///
/// # Display Properties
///
/// | Property | Type | Description |
/// |----------|------|-------------|
/// | `label` | `Option<String>` | Label text displayed above the input |
/// | `placeholder` | `Option<String>` | Placeholder text shown when input is empty |
/// | `help_text` | `Option<String>` | Help text displayed below the input |
/// | `disabled` | `bool` | Whether the field is disabled (cannot be edited) |
/// | `readonly` | `bool` | Whether the field is read-only (can be copied but not edited) |
/// | `autofocus` | `bool` | Whether this field should receive focus on page load |
#[derive(Debug, Clone, Default)]
pub struct TypedFieldDisplay {
	/// Label text
	pub label: Option<String>,
	/// Placeholder text
	pub placeholder: Option<String>,
	/// Help text
	pub help_text: Option<String>,
	/// Whether the field is disabled
	pub disabled: bool,
	/// Whether the field is readonly
	pub readonly: bool,
	/// Whether to autofocus this field
	pub autofocus: bool,
}

/// Styling-related properties of a field.
///
/// These properties control CSS classes applied to different parts of the field.
/// Each property has a default value that provides consistent styling out of the box.
///
/// # Styling Properties
///
/// | Property | Default Value | Applied To |
/// |----------|---------------|------------|
/// | `class` | `"reinhardt-input"` | The input element |
/// | `wrapper_class` | `"reinhardt-field"` | The field container div |
/// | `label_class` | `"reinhardt-label"` | The label element |
/// | `error_class` | `"reinhardt-error"` | The error message element |
#[derive(Debug, Clone, Default)]
pub struct TypedFieldStyling {
	/// CSS class for the input element
	pub class: Option<String>,
	/// CSS class for the wrapper element
	pub wrapper_class: Option<String>,
	/// CSS class for the label element
	pub label_class: Option<String>,
	/// CSS class for the error element
	pub error_class: Option<String>,
}

impl TypedFieldStyling {
	/// Returns the CSS class for the input element, with default fallback.
	pub fn input_class(&self) -> &str {
		self.class.as_deref().unwrap_or("reinhardt-input")
	}

	/// Returns the CSS class for the wrapper element, with default fallback.
	pub fn wrapper_class(&self) -> &str {
		self.wrapper_class.as_deref().unwrap_or("reinhardt-field")
	}

	/// Returns the CSS class for the label element, with default fallback.
	pub fn label_class(&self) -> &str {
		self.label_class.as_deref().unwrap_or("reinhardt-label")
	}

	/// Returns the CSS class for the error element, with default fallback.
	pub fn error_class(&self) -> &str {
		self.error_class.as_deref().unwrap_or("reinhardt-error")
	}
}

/// Typed server-side validator for a specific field.
#[derive(Debug)]
pub struct TypedFormValidator {
	/// Field name being validated
	pub field_name: Ident,
	/// Validation rules for this field
	pub rules: Vec<TypedValidatorRule>,
	/// Span for error reporting
	pub span: Span,
}

/// A typed validation rule with condition expression and error message.
#[derive(Debug)]
pub struct TypedValidatorRule {
	/// Validation condition expression (should evaluate to bool)
	pub condition: syn::Expr,
	/// Error message when validation fails
	pub message: String,
	/// Span for error reporting
	pub span: Span,
}

/// Typed client-side validator.
#[derive(Debug)]
pub struct TypedClientValidator {
	/// Field name to validate
	pub field_name: Ident,
	/// Validation rules
	pub rules: Vec<TypedClientValidatorRule>,
	/// Span for error reporting
	pub span: Span,
}

/// A typed client-side validation rule.
#[derive(Debug)]
pub struct TypedClientValidatorRule {
	/// JavaScript condition expression for validation
	pub js_condition: String,
	/// Error message when validation fails
	pub message: String,
	/// Span for error reporting
	pub span: Span,
}

/// A validated wrapper element definition for custom field containers.
///
/// Wrappers allow specifying custom HTML elements to wrap around form fields,
/// providing flexibility for styling and layout requirements.
///
/// # Example
///
/// ```text
/// wrapper: div { class: "relative", id: "field-wrapper" }
/// ```
#[derive(Debug, Clone)]
pub struct TypedWrapper {
	/// The HTML tag name for the wrapper element
	pub tag: String,
	/// Attributes for the wrapper element
	pub attrs: Vec<TypedWrapperAttr>,
	/// Span for error reporting
	pub span: Span,
}

/// A validated attribute on a wrapper element.
#[derive(Debug, Clone)]
pub struct TypedWrapperAttr {
	/// Attribute name (e.g., "class", "id")
	pub name: String,
	/// Attribute value as a string
	pub value: String,
	/// Span for error reporting
	pub span: Span,
}

impl TypedWrapper {
	/// Returns true if this wrapper has any attributes.
	pub fn has_attrs(&self) -> bool {
		!self.attrs.is_empty()
	}

	/// Returns the value of the class attribute, if present.
	pub fn class(&self) -> Option<&str> {
		self.attrs
			.iter()
			.find(|a| a.name == "class")
			.map(|a| a.value.as_str())
	}

	/// Returns the value of the id attribute, if present.
	pub fn id(&self) -> Option<&str> {
		self.attrs
			.iter()
			.find(|a| a.name == "id")
			.map(|a| a.value.as_str())
	}
}

/// A validated custom attribute for accessibility or data attributes.
///
/// Custom attributes allow adding aria-* and data-* attributes to form fields
/// for accessibility and testing purposes.
///
/// # Example
///
/// ```text
/// attrs: {
///     aria_label: "Email address",
///     aria_required: "true",
///     data_testid: "email-input",
/// }
/// ```
///
/// Note: Underscores in attribute names are converted to hyphens in the generated HTML
/// (e.g., `aria_label` becomes `aria-label`).
#[derive(Debug, Clone)]
pub struct TypedCustomAttr {
	/// Attribute name with underscores (e.g., "aria_label", "data_testid")
	pub name: String,
	/// Attribute value as a string
	pub value: String,
	/// Span for error reporting
	pub span: Span,
}

impl TypedCustomAttr {
	/// Returns the HTML attribute name with hyphens instead of underscores.
	///
	/// # Example
	///
	/// ```ignore
	/// let attr = TypedCustomAttr { name: "aria_label".to_string(), ... };
	/// assert_eq!(attr.html_name(), "aria-label");
	/// ```
	pub fn html_name(&self) -> String {
		self.name.replace('_', "-")
	}

	/// Returns true if this is an aria-* attribute.
	pub fn is_aria(&self) -> bool {
		self.name.starts_with("aria_")
	}

	/// Returns true if this is a data-* attribute.
	pub fn is_data(&self) -> bool {
		self.name.starts_with("data_")
	}
}

/// A validated SVG icon element for form fields.
///
/// Icons can be displayed alongside input fields to provide visual context.
/// The icon position determines where the icon appears relative to the input.
///
/// # Example
///
/// ```text
/// icon: svg {
///     class: "w-5 h-5 text-gray-400",
///     viewBox: "0 0 24 24",
///     path { d: "M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4z" }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct TypedIcon {
	/// SVG attributes (class, viewBox, fill, stroke, etc.)
	pub attrs: Vec<TypedIconAttr>,
	/// Child elements (path, circle, rect, g, etc.)
	pub children: Vec<TypedIconChild>,
	/// Icon position relative to the input field
	pub position: TypedIconPosition,
	/// Span for error reporting
	pub span: Span,
}

/// A validated attribute on an SVG icon element.
#[derive(Debug, Clone)]
pub struct TypedIconAttr {
	/// Attribute name (e.g., "class", "viewBox", "fill")
	pub name: String,
	/// Attribute value as a string
	pub value: String,
	/// Span for error reporting
	pub span: Span,
}

/// A validated child element within an SVG icon.
#[derive(Debug, Clone)]
pub struct TypedIconChild {
	/// Element tag (e.g., "path", "circle", "rect", "g")
	pub tag: String,
	/// Element attributes
	pub attrs: Vec<TypedIconAttr>,
	/// Nested children (for grouping elements like `g`)
	pub children: Vec<TypedIconChild>,
	/// Span for error reporting
	pub span: Span,
}

/// Position of the icon relative to the input field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TypedIconPosition {
	/// Icon on the left side of the input (default)
	#[default]
	Left,
	/// Icon on the right side of the input
	Right,
	/// Icon within the label element
	Label,
}

impl TypedIconPosition {
	/// Returns the string representation of the position.
	pub fn as_str(&self) -> &'static str {
		match self {
			TypedIconPosition::Left => "left",
			TypedIconPosition::Right => "right",
			TypedIconPosition::Label => "label",
		}
	}

	/// Returns a CSS class suffix for the position.
	pub fn css_class_suffix(&self) -> &'static str {
		match self {
			TypedIconPosition::Left => "icon-left",
			TypedIconPosition::Right => "icon-right",
			TypedIconPosition::Label => "icon-label",
		}
	}
}

impl TypedIcon {
	/// Returns true if this icon has any attributes.
	pub fn has_attrs(&self) -> bool {
		!self.attrs.is_empty()
	}

	/// Returns true if this icon has any children.
	pub fn has_children(&self) -> bool {
		!self.children.is_empty()
	}

	/// Returns the value of the class attribute, if present.
	pub fn class(&self) -> Option<&str> {
		self.attrs
			.iter()
			.find(|a| a.name == "class")
			.map(|a| a.value.as_str())
	}

	/// Returns the value of the viewBox attribute, if present.
	pub fn view_box(&self) -> Option<&str> {
		self.attrs
			.iter()
			.find(|a| a.name == "viewBox")
			.map(|a| a.value.as_str())
	}

	/// Returns the value of the fill attribute, if present.
	pub fn fill(&self) -> Option<&str> {
		self.attrs
			.iter()
			.find(|a| a.name == "fill")
			.map(|a| a.value.as_str())
	}

	/// Returns the value of the stroke attribute, if present.
	pub fn stroke(&self) -> Option<&str> {
		self.attrs
			.iter()
			.find(|a| a.name == "stroke")
			.map(|a| a.value.as_str())
	}
}

impl TypedIconChild {
	/// Returns true if this child has any attributes.
	pub fn has_attrs(&self) -> bool {
		!self.attrs.is_empty()
	}

	/// Returns true if this child has any nested children.
	pub fn has_children(&self) -> bool {
		!self.children.is_empty()
	}

	/// Returns the value of the d attribute (for path elements), if present.
	pub fn d_attr(&self) -> Option<&str> {
		self.attrs
			.iter()
			.find(|a| a.name == "d")
			.map(|a| a.value.as_str())
	}
}

impl TypedFormMacro {
	/// Creates a new TypedFormMacro with the given name and action.
	pub fn new(name: Ident, action: TypedFormAction, span: Span) -> Self {
		Self {
			name,
			action,
			method: FormMethod::default(),
			styling: TypedFormStyling::default(),
			state: None,
			callbacks: TypedFormCallbacks::new(),
			watch: None,
			derived: None,
			redirect_on_success: None,
			initial_loader: None,
			choices_loader: None,
			slots: None,
			fields: Vec::new(),
			validators: Vec::new(),
			client_validators: Vec::new(),
			span,
		}
	}

	/// Returns true if this form uses server_fn for submission.
	pub fn uses_server_fn(&self) -> bool {
		matches!(self.action, TypedFormAction::ServerFn(_))
	}

	/// Returns the action URL if using URL mode.
	pub fn action_url(&self) -> Option<&str> {
		match &self.action {
			TypedFormAction::Url(url) => Some(url),
			_ => None,
		}
	}

	/// Returns the server_fn path if using server_fn mode.
	pub fn server_fn_path(&self) -> Option<&Path> {
		match &self.action {
			TypedFormAction::ServerFn(path) => Some(path),
			_ => None,
		}
	}

	/// Returns the form-level CSS class with default fallback.
	pub fn form_class(&self) -> &str {
		self.styling.class.as_deref().unwrap_or("reinhardt-form")
	}
}

impl TypedFormFieldDef {
	/// Creates a new TypedFormFieldDef with the given name and type.
	pub fn new(name: Ident, field_type: TypedFieldType, span: Span) -> Self {
		let widget = field_type.default_widget();
		Self {
			name,
			field_type,
			validation: TypedFieldValidation::default(),
			display: TypedFieldDisplay::default(),
			styling: TypedFieldStyling::default(),
			widget,
			wrapper: None,
			icon: None,
			custom_attrs: Vec::new(),
			bind: true, // Default to enabled
			initial_from: None,
			choices_config: None,
			span,
		}
	}

	/// Returns true if two-way binding is enabled for this field.
	///
	/// When true, the form automatically generates an @input handler
	/// to update the Signal when the user types.
	pub fn is_bind_enabled(&self) -> bool {
		self.bind
	}

	/// Returns true if this field has a custom wrapper element.
	pub fn has_wrapper(&self) -> bool {
		self.wrapper.is_some()
	}

	/// Returns true if this field has an SVG icon.
	pub fn has_icon(&self) -> bool {
		self.icon.is_some()
	}

	/// Returns the icon position, defaulting to Left if icon exists but no position specified.
	pub fn icon_position(&self) -> Option<TypedIconPosition> {
		self.icon.as_ref().map(|i| i.position)
	}

	/// Returns true if this field has custom attributes.
	pub fn has_custom_attrs(&self) -> bool {
		!self.custom_attrs.is_empty()
	}

	/// Returns the aria-* attributes.
	pub fn aria_attrs(&self) -> impl Iterator<Item = &TypedCustomAttr> {
		self.custom_attrs.iter().filter(|a| a.is_aria())
	}

	/// Returns the data-* attributes.
	pub fn data_attrs(&self) -> impl Iterator<Item = &TypedCustomAttr> {
		self.custom_attrs.iter().filter(|a| a.is_data())
	}

	/// Returns the HTML name attribute for this field.
	pub fn html_name(&self) -> String {
		self.name.to_string()
	}

	/// Returns the HTML id attribute for this field.
	pub fn html_id(&self) -> String {
		format!("id_{}", self.name)
	}

	/// Returns true if this field has dynamic choices configuration.
	pub fn has_choices_config(&self) -> bool {
		self.choices_config.is_some()
	}

	/// Returns true if this is a dynamic choice field.
	///
	/// A dynamic choice field has `choices_config` and will load its options
	/// from a `choices_loader` server_fn at runtime.
	pub fn is_dynamic_choice_field(&self) -> bool {
		self.has_choices_config()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_form_method_as_str() {
		// Arrange
		let post = FormMethod::Post;
		let get = FormMethod::Get;
		let put = FormMethod::Put;
		let delete = FormMethod::Delete;
		let patch = FormMethod::Patch;

		// Act
		let post_str = post.as_str();
		let get_str = get.as_str();
		let put_str = put.as_str();
		let delete_str = delete.as_str();
		let patch_str = patch.as_str();

		// Assert
		assert_eq!(post_str, "POST");
		assert_eq!(get_str, "GET");
		assert_eq!(put_str, "PUT");
		assert_eq!(delete_str, "DELETE");
		assert_eq!(patch_str, "PATCH");
	}

	#[rstest]
	fn test_form_method_requires_js() {
		// Arrange
		let post = FormMethod::Post;
		let get = FormMethod::Get;
		let put = FormMethod::Put;
		let delete = FormMethod::Delete;
		let patch = FormMethod::Patch;

		// Act
		let post_requires_js = post.requires_js();
		let get_requires_js = get.requires_js();
		let put_requires_js = put.requires_js();
		let delete_requires_js = delete.requires_js();
		let patch_requires_js = patch.requires_js();

		// Assert
		assert!(!post_requires_js);
		assert!(!get_requires_js);
		assert!(put_requires_js);
		assert!(delete_requires_js);
		assert!(patch_requires_js);
	}

	#[rstest]
	fn test_typed_field_type_rust_type() {
		// Arrange
		let char_field = TypedFieldType::CharField;
		let integer_field = TypedFieldType::IntegerField;
		let boolean_field = TypedFieldType::BooleanField;
		let date_field = TypedFieldType::DateField;

		// Act
		let char_type = char_field.rust_type();
		let integer_type = integer_field.rust_type();
		let boolean_type = boolean_field.rust_type();
		let date_type = date_field.rust_type();

		// Assert
		assert_eq!(char_type, "String");
		assert_eq!(integer_type, "i64");
		assert_eq!(boolean_type, "bool");
		assert_eq!(date_type, "Option<chrono::NaiveDate>");
	}

	#[rstest]
	fn test_typed_field_type_default_widget() {
		// Arrange
		let char_field = TypedFieldType::CharField;
		let email_field = TypedFieldType::EmailField;
		let password_field = TypedFieldType::PasswordField;
		let boolean_field = TypedFieldType::BooleanField;

		// Act
		let char_widget = char_field.default_widget();
		let email_widget = email_field.default_widget();
		let password_widget = password_field.default_widget();
		let boolean_widget = boolean_field.default_widget();

		// Assert
		assert_eq!(char_widget, TypedWidget::TextInput);
		assert_eq!(email_widget, TypedWidget::EmailInput);
		assert_eq!(password_widget, TypedWidget::PasswordInput);
		assert_eq!(boolean_widget, TypedWidget::CheckboxInput);
	}

	#[rstest]
	fn test_typed_widget_html_type() {
		// Arrange
		let text_input = TypedWidget::TextInput;
		let email_input = TypedWidget::EmailInput;
		let password_input = TypedWidget::PasswordInput;
		let number_input = TypedWidget::NumberInput;
		let date_input = TypedWidget::DateInput;

		// Act
		let text_type = text_input.html_type();
		let email_type = email_input.html_type();
		let password_type = password_input.html_type();
		let number_type = number_input.html_type();
		let date_type = date_input.html_type();

		// Assert
		assert_eq!(text_type, "text");
		assert_eq!(email_type, "email");
		assert_eq!(password_type, "password");
		assert_eq!(number_type, "number");
		assert_eq!(date_type, "date");
	}

	#[rstest]
	fn test_typed_widget_is_input() {
		// Arrange
		let text_input = TypedWidget::TextInput;
		let email_input = TypedWidget::EmailInput;
		let textarea = TypedWidget::Textarea;
		let select = TypedWidget::Select;

		// Act
		let text_is_input = text_input.is_input();
		let email_is_input = email_input.is_input();
		let textarea_is_input = textarea.is_input();
		let select_is_input = select.is_input();

		// Assert
		assert!(text_is_input);
		assert!(email_is_input);
		assert!(!textarea_is_input);
		assert!(!select_is_input);
	}

	#[rstest]
	fn test_typed_widget_html_tag() {
		// Arrange
		let text_input = TypedWidget::TextInput;
		let textarea = TypedWidget::Textarea;
		let select = TypedWidget::Select;

		// Act
		let text_tag = text_input.html_tag();
		let textarea_tag = textarea.html_tag();
		let select_tag = select.html_tag();

		// Assert
		assert_eq!(text_tag, "input");
		assert_eq!(textarea_tag, "textarea");
		assert_eq!(select_tag, "select");
	}

	#[rstest]
	fn test_typed_field_styling_defaults() {
		// Arrange
		let styling = TypedFieldStyling::default();

		// Act
		let input_class = styling.input_class();
		let wrapper_class = styling.wrapper_class();
		let label_class = styling.label_class();
		let error_class = styling.error_class();

		// Assert
		assert_eq!(input_class, "reinhardt-input");
		assert_eq!(wrapper_class, "reinhardt-field");
		assert_eq!(label_class, "reinhardt-label");
		assert_eq!(error_class, "reinhardt-error");
	}

	#[rstest]
	fn test_typed_field_styling_custom() {
		// Arrange
		let styling = TypedFieldStyling {
			class: Some("custom-input".to_string()),
			wrapper_class: Some("custom-wrapper".to_string()),
			label_class: Some("custom-label".to_string()),
			error_class: Some("custom-error".to_string()),
		};

		// Act
		let input_class = styling.input_class();
		let wrapper_class = styling.wrapper_class();
		let label_class = styling.label_class();
		let error_class = styling.error_class();

		// Assert
		assert_eq!(input_class, "custom-input");
		assert_eq!(wrapper_class, "custom-wrapper");
		assert_eq!(label_class, "custom-label");
		assert_eq!(error_class, "custom-error");
	}

	#[rstest]
	fn test_typed_form_field_def_html_name() {
		// Arrange
		let field = TypedFormFieldDef::new(
			Ident::new("username", Span::call_site()),
			TypedFieldType::CharField,
			Span::call_site(),
		);

		// Act
		let html_name = field.html_name();
		let html_id = field.html_id();

		// Assert
		assert_eq!(html_name, "username");
		assert_eq!(html_id, "id_username");
	}

	#[rstest]
	fn test_typed_custom_attr_html_name() {
		// Arrange
		let attr = TypedCustomAttr {
			name: "aria_label".to_string(),
			value: "Email address".to_string(),
			span: Span::call_site(),
		};

		// Act
		let html_name = attr.html_name();

		// Assert
		assert_eq!(html_name, "aria-label");
	}

	#[rstest]
	fn test_typed_custom_attr_is_aria() {
		// Arrange
		let aria_attr = TypedCustomAttr {
			name: "aria_label".to_string(),
			value: "Label".to_string(),
			span: Span::call_site(),
		};
		let data_attr = TypedCustomAttr {
			name: "data_testid".to_string(),
			value: "test".to_string(),
			span: Span::call_site(),
		};

		// Act
		let aria_is_aria = aria_attr.is_aria();
		let aria_is_data = aria_attr.is_data();
		let data_is_aria = data_attr.is_aria();
		let data_is_data = data_attr.is_data();

		// Assert
		assert!(aria_is_aria);
		assert!(!aria_is_data);
		assert!(!data_is_aria);
		assert!(data_is_data);
	}

	#[rstest]
	fn test_typed_custom_attr_data_prefix() {
		// Arrange
		let attr = TypedCustomAttr {
			name: "data_testid".to_string(),
			value: "email-input".to_string(),
			span: Span::call_site(),
		};

		// Act
		let html_name = attr.html_name();
		let is_data = attr.is_data();

		// Assert
		assert_eq!(html_name, "data-testid");
		assert!(is_data);
	}

	#[rstest]
	fn test_typed_form_field_custom_attrs() {
		// Arrange
		let mut field = TypedFormFieldDef::new(
			Ident::new("email", Span::call_site()),
			TypedFieldType::EmailField,
			Span::call_site(),
		);

		// Assert (initial state)
		assert!(!field.has_custom_attrs());

		// Act
		field.custom_attrs.push(TypedCustomAttr {
			name: "aria_label".to_string(),
			value: "Email".to_string(),
			span: Span::call_site(),
		});
		field.custom_attrs.push(TypedCustomAttr {
			name: "data_testid".to_string(),
			value: "email".to_string(),
			span: Span::call_site(),
		});

		// Assert (after adding attrs)
		assert!(field.has_custom_attrs());
		assert_eq!(field.aria_attrs().count(), 1);
		assert_eq!(field.data_attrs().count(), 1);
	}
}
