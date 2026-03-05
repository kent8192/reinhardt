//! Untyped AST node definitions for the `form!` macro in reinhardt-pages.
//!
//! These structures represent the raw parse output before semantic validation.
//! The form! macro in reinhardt-pages is designed to work with both SSR (Server-Side Rendering)
//! and CSR (Client-Side Rendering/WASM) targets.
//!
//! ## DSL Structure
//!
//! ```text
//! form! {
//!     name: LoginForm,
//!     action: "/api/login",       // OR server_fn: submit_login
//!     method: Post,               // Optional, defaults to Post
//!     class: "form-container",    // Optional form-level styling
//!
//!     fields: {
//!         username: CharField {
//!             required,
//!             max_length: 150,
//!             label: "Username",
//!             placeholder: "Enter username",
//!             class: "input-field",
//!             wrapper_class: "field-group",
//!         },
//!         password: CharField {
//!             required,
//!             widget: PasswordInput,
//!             min_length: 8,
//!         },
//!     },
//!
//!     validators: {
//!         username: [
//!             |v| !v.trim().is_empty() => "Username cannot be empty",
//!         ],
//!     },
//!
//!     client_validators: {
//!         password: [
//!             "value.length >= 8" => "Password must be at least 8 characters",
//!         ],
//!     },
//! }
//! ```

use proc_macro2::Span;
use syn::{Expr, ExprClosure, Ident, LitStr, Path};

/// Top-level form macro AST.
///
/// Represents the entire `form! { ... }` invocation with support for
/// SSR and CSR rendering targets.
#[derive(Debug, Clone)]
pub struct FormMacro {
	/// Form struct name (required, e.g., `name: LoginForm`).
	///
	/// `None` indicates the name has not been parsed yet. The parser
	/// validates that a name is provided before returning, so downstream
	/// consumers can safely unwrap this value.
	pub name: Option<Ident>,
	/// Form action configuration
	pub action: FormAction,
	/// HTTP method (defaults to Post)
	pub method: Option<Ident>,
	/// Form-level CSS class
	pub class: Option<LitStr>,
	/// UI state configuration (loading, error, success signals)
	pub state: Option<FormState>,
	/// Form submission callbacks
	pub callbacks: FormCallbacks,
	/// Watch block for reactive computed views
	pub watch: Option<FormWatch>,
	/// Derived/computed values block for reactive signals
	pub derived: Option<FormDerived>,
	/// Redirect URL on successful form submission
	///
	/// Supports static paths (`"/profile"`) or dynamic paths with parameter expansion (`"/profile/{id}"`).
	/// The redirect is triggered after `on_success` callback (if any) completes.
	pub redirect_on_success: Option<LitStr>,
	/// Initial value loader server_fn
	///
	/// When specified, the form will call this server_fn to load initial values
	/// for fields that have `initial_from` specified.
	pub initial_loader: Option<Path>,
	/// Choices loader server_fn for dynamic `ChoiceField`
	///
	/// When specified, the form will call this server_fn to load choice options
	/// for fields that have `choices_from` specified. The loader returns a struct
	/// containing the choice data, and individual fields use `choice_value` and
	/// `choice_label` to extract the value and label from each choice item.
	pub choices_loader: Option<Path>,
	/// Slot definitions for custom UI elements
	pub slots: Option<FormSlots>,
	/// Field definitions (can include field groups)
	pub fields: Vec<FormFieldEntry>,
	/// Server-side validators
	pub validators: Vec<FormValidator>,
	/// Client-side validators (JavaScript expressions)
	pub client_validators: Vec<ClientValidator>,
	/// Span for error reporting
	pub span: Span,
}

/// Form action configuration.
///
/// Supports two modes:
/// - URL string: `action: "/api/login"`
/// - server_fn: `server_fn: submit_login`
#[derive(Debug, Clone)]
pub enum FormAction {
	/// URL action (traditional form submission)
	Url(LitStr),
	/// server_fn action (type-safe RPC)
	ServerFn(Path),
	/// No action specified (will be set programmatically)
	None,
}

/// An entry in the form fields list.
///
/// Can be either a regular field definition or a field group
/// containing multiple related fields.
#[derive(Debug, Clone)]
pub enum FormFieldEntry {
	/// A single field definition
	Field(FormFieldDef),
	/// A group of related fields
	Group(FormFieldGroup),
}

impl FormFieldEntry {
	/// Returns true if this is a field group.
	pub fn is_group(&self) -> bool {
		matches!(self, FormFieldEntry::Group(_))
	}

	/// Returns true if this is a regular field.
	pub fn is_field(&self) -> bool {
		matches!(self, FormFieldEntry::Field(_))
	}

	/// Returns the name of the entry (field name or group name).
	pub fn name(&self) -> &Ident {
		match self {
			FormFieldEntry::Field(f) => &f.name,
			FormFieldEntry::Group(g) => &g.name,
		}
	}

	/// Returns the span for error reporting.
	pub fn span(&self) -> Span {
		match self {
			FormFieldEntry::Field(f) => f.span,
			FormFieldEntry::Group(g) => g.span,
		}
	}

	/// Returns a reference to the inner field if this is a Field variant.
	pub fn as_field(&self) -> Option<&FormFieldDef> {
		match self {
			FormFieldEntry::Field(f) => Some(f),
			FormFieldEntry::Group(_) => None,
		}
	}

	/// Returns a reference to the inner group if this is a Group variant.
	pub fn as_group(&self) -> Option<&FormFieldGroup> {
		match self {
			FormFieldEntry::Field(_) => None,
			FormFieldEntry::Group(g) => Some(g),
		}
	}
}

/// A single field definition in the form macro.
///
/// Example:
/// ```ignore
/// username: CharField {
///     required,
///     max_length: 100,
///     label: "Username",
///     class: "input-field",
///     wrapper_class: "field-group",
/// }
/// ```
#[derive(Debug, Clone)]
pub struct FormFieldDef {
	/// Field name identifier
	pub name: Ident,
	/// Field type identifier (e.g., CharField, EmailField)
	pub field_type: Ident,
	/// Field properties (validation and styling)
	pub properties: Vec<FormFieldProperty>,
	/// Span for error reporting
	pub span: Span,
}

/// A group of related fields in the form macro.
///
/// Field groups allow organizing multiple fields under a common container
/// with shared styling. Groups cannot be nested.
///
/// ## Example DSL
///
/// ```ignore
/// address_group: FieldGroup {
///     label: "Address",
///     class: "address-section",
///
///     fields: {
///         street: CharField { required },
///         city: CharField { required },
///         zip: CharField { required, max_length: 10 },
///     },
/// }
/// ```
#[derive(Debug, Clone)]
pub struct FormFieldGroup {
	/// Group name identifier
	pub name: Ident,
	/// Group-level label text
	pub label: Option<LitStr>,
	/// Group-level CSS class
	pub class: Option<LitStr>,
	/// Fields within the group
	pub fields: Vec<FormFieldDef>,
	/// Span for error reporting
	pub span: Span,
}

impl FormFieldGroup {
	/// Returns the label text if specified.
	pub fn label_text(&self) -> Option<String> {
		self.label.as_ref().map(|l| l.value())
	}

	/// Returns the class name if specified.
	pub fn class_name(&self) -> Option<String> {
		self.class.as_ref().map(|c| c.value())
	}

	/// Returns the number of fields in this group.
	pub fn field_count(&self) -> usize {
		self.fields.len()
	}
}

/// A property within a field definition.
#[derive(Debug, Clone)]
pub enum FormFieldProperty {
	/// Named property with a value: `max_length: 100`, `label: "Username"`
	Named {
		name: Ident,
		value: Expr,
		span: Span,
	},
	/// Flag property (boolean true): `required`
	Flag { name: Ident, span: Span },
	/// Widget specification: `widget: PasswordInput`
	Widget { widget_type: Ident, span: Span },
	/// Custom wrapper element: `wrapper: div { class: "relative" }`
	Wrapper { element: WrapperElement, span: Span },
	/// SVG icon for the field: `icon: svg { ... }`
	Icon { element: IconElement, span: Span },
	/// Icon position: `icon_position: "left"`
	IconPosition { position: IconPosition, span: Span },
	/// Custom attributes: `attrs: { aria_label: "...", data_testid: "..." }`
	Attrs { attrs: Vec<CustomAttr>, span: Span },
	/// Two-way binding option: `bind: true` or `bind: false`
	///
	/// When true (default), the form automatically generates an @input handler
	/// to update the Signal when the user types. Set to false to disable
	/// automatic binding and use a custom handler instead.
	Bind { enabled: bool, span: Span },
	/// Initial value source: `initial_from: "field_name"`
	///
	/// Maps this field to a property in the data returned by `initial_loader`.
	/// The value is the property name in the loaded data structure.
	InitialFrom { field_name: LitStr, span: Span },
	/// Choices source for dynamic ChoiceField: `choices_from: "choices"`
	///
	/// Specifies which field in the data returned by `choices_loader` contains
	/// the array of choice options. Used with `ChoiceField` to populate
	/// radio buttons, checkboxes, or select dropdowns dynamically.
	ChoicesFrom { field_name: LitStr, span: Span },
	/// Choice value path: `choice_value: "id"`
	///
	/// Specifies which property of each choice item to use as the form value.
	/// The default is "value" if not specified.
	ChoiceValue { path: LitStr, span: Span },
	/// Choice label path: `choice_label: "choice_text"`
	///
	/// Specifies which property of each choice item to use as the display label.
	/// The default is "label" if not specified.
	ChoiceLabel { path: LitStr, span: Span },
}

/// A custom attribute for accessibility or data attributes.
///
/// Supports aria-* and data-* attributes on form fields.
/// Underscore in names is converted to hyphen (e.g., `aria_label` → `aria-label`).
///
/// ## Example DSL
///
/// ```ignore
/// attrs: {
///     aria_label: "Email address",
///     aria_required: "true",
///     data_testid: "email-input",
/// }
/// ```
#[derive(Debug, Clone)]
pub struct CustomAttr {
	/// Attribute name (using underscores, e.g., "aria_label")
	pub name: Ident,
	/// Attribute value
	pub value: Expr,
	/// Span for error reporting
	pub span: Span,
}

/// A wrapper element definition for custom field containers.
///
/// Allows defining a custom HTML element to wrap around the input field.
/// The input field will be placed as a child of this wrapper element.
///
/// ## Example DSL
///
/// ```ignore
/// wrapper: div {
///     class: "relative flex items-center",
/// }
/// ```
#[derive(Debug, Clone)]
pub struct WrapperElement {
	/// Element tag name (e.g., "div", "span")
	pub tag: Ident,
	/// Element attributes
	pub attrs: Vec<WrapperAttr>,
	/// Span for error reporting
	pub span: Span,
}

/// An attribute on a wrapper element.
#[derive(Debug, Clone)]
pub struct WrapperAttr {
	/// Attribute name
	pub name: Ident,
	/// Attribute value
	pub value: Expr,
	/// Span for error reporting
	pub span: Span,
}

/// An SVG icon element for form fields.
///
/// Allows defining an SVG icon to display alongside the input field.
/// The icon can be positioned left, right, or within the label.
///
/// ## Example DSL
///
/// ```ignore
/// icon: svg {
///     class: "w-5 h-5 text-gray-400",
///     viewBox: "0 0 24 24",
///     path { d: "M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4z" }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct IconElement {
	/// SVG attributes (class, viewBox, fill, stroke, etc.)
	pub attrs: Vec<IconAttr>,
	/// Child elements (path, circle, rect, g, etc.)
	pub children: Vec<IconChild>,
	/// Span for error reporting
	pub span: Span,
}

/// An attribute on an SVG icon element.
#[derive(Debug, Clone)]
pub struct IconAttr {
	/// Attribute name (e.g., "class", "viewBox", "fill")
	pub name: Ident,
	/// Attribute value
	pub value: Expr,
	/// Span for error reporting
	pub span: Span,
}

/// A child element within an SVG icon.
///
/// Supports common SVG child elements like path, circle, rect, line, etc.
#[derive(Debug, Clone)]
pub struct IconChild {
	/// Element tag (e.g., "path", "circle", "rect", "g")
	pub tag: Ident,
	/// Element attributes
	pub attrs: Vec<IconAttr>,
	/// Nested children (for grouping elements like `g`)
	pub children: Vec<IconChild>,
	/// Span for error reporting
	pub span: Span,
}

/// Position of the icon relative to the input field.
///
/// ## Positions
///
/// | Position | Description |
/// |----------|-------------|
/// | `left` | Icon appears to the left of the input field |
/// | `right` | Icon appears to the right of the input field |
/// | `label` | Icon appears within the label element |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconPosition {
	/// Icon on the left side of the input (default)
	#[default]
	Left,
	/// Icon on the right side of the input
	Right,
	/// Icon within the label element
	Label,
}

impl std::str::FromStr for IconPosition {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"left" => Ok(IconPosition::Left),
			"right" => Ok(IconPosition::Right),
			"label" => Ok(IconPosition::Label),
			_ => Err(()),
		}
	}
}

impl IconPosition {
	/// Returns the string representation of the position.
	pub fn as_str(&self) -> &'static str {
		match self {
			IconPosition::Left => "left",
			IconPosition::Right => "right",
			IconPosition::Label => "label",
		}
	}
}

impl IconElement {
	/// Creates a new empty IconElement.
	pub fn new(span: Span) -> Self {
		Self {
			attrs: Vec::new(),
			children: Vec::new(),
			span,
		}
	}

	/// Gets the class attribute value if present.
	pub fn get_class(&self) -> Option<&Expr> {
		self.attrs
			.iter()
			.find(|a| a.name == "class")
			.map(|a| &a.value)
	}

	/// Gets the viewBox attribute value if present.
	pub fn get_view_box(&self) -> Option<&Expr> {
		self.attrs
			.iter()
			.find(|a| a.name == "viewBox")
			.map(|a| &a.value)
	}
}

impl IconChild {
	/// Creates a new IconChild with the given tag.
	pub fn new(tag: Ident, span: Span) -> Self {
		Self {
			tag,
			attrs: Vec::new(),
			children: Vec::new(),
			span,
		}
	}
}

impl FormFieldProperty {
	/// Returns the property name for named properties and flags.
	///
	/// Returns `None` for structural properties (Widget, Wrapper, Icon,
	/// IconPosition, Attrs, Bind, InitialFrom, ChoicesFrom, ChoiceValue,
	/// ChoiceLabel) that do not have a direct name identifier.
	pub fn name(&self) -> Option<&Ident> {
		match self {
			FormFieldProperty::Named { name, .. } | FormFieldProperty::Flag { name, .. } => {
				Some(name)
			}
			_ => None,
		}
	}

	/// Returns the span for error reporting.
	pub fn span(&self) -> Span {
		match self {
			FormFieldProperty::Named { span, .. } => *span,
			FormFieldProperty::Flag { span, .. } => *span,
			FormFieldProperty::Widget { span, .. } => *span,
			FormFieldProperty::Wrapper { span, .. } => *span,
			FormFieldProperty::Icon { span, .. } => *span,
			FormFieldProperty::IconPosition { span, .. } => *span,
			FormFieldProperty::Attrs { span, .. } => *span,
			FormFieldProperty::Bind { span, .. } => *span,
			FormFieldProperty::InitialFrom { span, .. } => *span,
			FormFieldProperty::ChoicesFrom { span, .. } => *span,
			FormFieldProperty::ChoiceValue { span, .. } => *span,
			FormFieldProperty::ChoiceLabel { span, .. } => *span,
		}
	}

	/// Returns true if this is a styling property.
	pub fn is_styling(&self) -> bool {
		match self {
			FormFieldProperty::Named { name, .. } => {
				let name_str = name.to_string();
				matches!(
					name_str.as_str(),
					"class" | "wrapper_class" | "label_class" | "error_class"
				)
			}
			_ => false,
		}
	}
}

/// Server-side validator definition.
#[derive(Debug, Clone)]
pub enum FormValidator {
	/// Field-level validator: `username: [|v| ... => "error"]`
	Field {
		field_name: Ident,
		rules: Vec<ValidatorRule>,
		span: Span,
	},
	/// Form-level validator: `@form: [|data| ... => "error"]`
	Form {
		rules: Vec<ValidatorRule>,
		span: Span,
	},
}

/// A single validation rule with closure and error message.
#[derive(Debug, Clone)]
pub struct ValidatorRule {
	/// Validation closure expression
	pub expr: ExprClosure,
	/// Error message when validation fails
	pub message: LitStr,
	/// Span for error reporting
	pub span: Span,
}

/// Client-side validator definition (JavaScript expressions).
#[derive(Debug, Clone)]
pub struct ClientValidator {
	/// Field name to validate
	pub field_name: Ident,
	/// Validation rules
	pub rules: Vec<ClientValidatorRule>,
	/// Span for error reporting
	pub span: Span,
}

/// A single client-side validation rule.
#[derive(Debug, Clone)]
pub struct ClientValidatorRule {
	/// JavaScript expression for validation
	pub js_expr: LitStr,
	/// Error message when validation fails
	pub message: LitStr,
	/// Span for error reporting
	pub span: Span,
}

/// Form submission callbacks configuration.
///
/// Allows defining custom behavior at different stages of form submission:
/// - Before submission starts
/// - On successful submission
/// - On error
/// - When loading state changes
///
/// ## Example DSL
///
/// ```ignore
/// form! {
///     name: ProfileForm,
///     server_fn: update_profile,
///
///     on_submit: |form| {
///         // Called before submission starts
///     },
///     on_success: |result| {
///         // Called when server_fn returns successfully
///     },
///     on_error: |e| {
///         // Called when submission fails
///     },
///     on_loading: |is_loading| {
///         // Called when loading state changes
///     },
///
///     fields: { ... }
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct FormCallbacks {
	/// Callback called before form submission starts.
	/// Signature: `|form: &FormName| { ... }`
	pub on_submit: Option<ExprClosure>,
	/// Callback called when submission succeeds.
	/// Receives the server_fn return value.
	/// Signature: `|result: T| { ... }`
	pub on_success: Option<ExprClosure>,
	/// Callback called when submission fails.
	/// Signature: `|error: ServerFnError| { ... }`
	pub on_error: Option<ExprClosure>,
	/// Callback called when loading state changes.
	/// Signature: `|is_loading: bool| { ... }`
	pub on_loading: Option<ExprClosure>,
	/// Span for error reporting (from first callback parsed)
	pub span: Option<Span>,
}

impl FormCallbacks {
	/// Creates a new empty FormCallbacks.
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
}

/// UI state configuration for form submission.
///
/// Defines which state signals (loading, error, success) are enabled for the form.
/// These signals are automatically managed during form submission lifecycle.
///
/// ## Example DSL
///
/// ```ignore
/// state: { loading, error, success },
/// ```
///
/// ## Available State Fields
///
/// | Field | Signal Type | Description |
/// |-------|-------------|-------------|
/// | `loading` | `Signal<bool>` | True during form submission |
/// | `error` | `Signal<Option<String>>` | Contains error message if submission failed |
/// | `success` | `Signal<bool>` | True after successful submission |
#[derive(Debug, Clone)]
pub struct FormState {
	/// List of enabled state fields
	pub fields: Vec<FormStateField>,
	/// Span for error reporting
	pub span: Span,
}

/// A state field to enable in the form.
///
/// Valid field names are: `loading`, `error`, `success`.
/// Each field name implies a specific Signal type:
/// - `loading` → `Signal<bool>`
/// - `error` → `Signal<Option<String>>`
/// - `success` → `Signal<bool>`
#[derive(Debug, Clone)]
pub struct FormStateField {
	/// Field name (loading, error, or success)
	pub name: Ident,
	/// Span for error reporting
	pub span: Span,
}

impl FormState {
	/// Creates a new empty FormState.
	pub fn new(span: Span) -> Self {
		Self {
			fields: Vec::new(),
			span,
		}
	}

	/// Returns true if loading state is enabled.
	pub fn has_loading(&self) -> bool {
		self.fields.iter().any(|f| f.name == "loading")
	}

	/// Returns true if error state is enabled.
	pub fn has_error(&self) -> bool {
		self.fields.iter().any(|f| f.name == "error")
	}

	/// Returns true if success state is enabled.
	pub fn has_success(&self) -> bool {
		self.fields.iter().any(|f| f.name == "success")
	}

	/// Returns true if any state field is enabled.
	pub fn is_empty(&self) -> bool {
		self.fields.is_empty()
	}
}

/// Watch block for reactive computed views.
///
/// Contains named watch items that define reactive views that re-render
/// when their dependencies (Signals) change.
///
/// ## Example DSL
///
/// ```ignore
/// watch: {
///     error_display: |form| {
///         if let Some(err) = form.error().get() {
///             div { class: "error", err }
///         }
///     },
///     loading_spinner: |form| {
///         if *form.loading().get() {
///             div { class: "spinner", "Loading..." }
///         }
///     },
/// }
/// ```
#[derive(Debug, Clone)]
pub struct FormWatch {
	/// List of named watch items
	pub items: Vec<FormWatchItem>,
	/// Span for error reporting
	pub span: Span,
}

impl FormWatch {
	/// Creates a new empty FormWatch.
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

/// A single watch item within a watch block.
///
/// Each watch item has a name and a closure that generates a View.
/// The closure receives the form instance as a parameter and can
/// access any Signals to create reactive dependencies.
#[derive(Debug, Clone)]
pub struct FormWatchItem {
	/// Watch item name (used for generated method name)
	pub name: Ident,
	/// Closure that generates the View
	/// Signature: `|form: &FormName| { ... }` or `|form| { ... }`
	pub closure: ExprClosure,
	/// Span for error reporting
	pub span: Span,
}

/// Derived/computed values block for reactive computed signals.
///
/// Contains named derived items that define computed values based on
/// other form fields or signals. Each item generates a `Memo<T>` that
/// automatically updates when its dependencies change.
///
/// ## Example DSL
///
/// ```ignore
/// derived: {
///     char_count: |form| form.content().get().len(),
///     is_over_limit: |form| form.char_count().get() > 280,
///     progress_percent: |form| (form.char_count().get() as f32 / 280.0 * 100.0).min(100.0),
/// }
/// ```
///
/// ## Generated Code
///
/// Each derived item generates a `Memo<T>` accessor on the form struct:
///
/// ```ignore
/// impl MyForm {
///     pub fn char_count(&self) -> Memo<usize> {
///         Memo::new(move || self.content().get().len())
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct FormDerived {
	/// List of named derived items
	pub items: Vec<FormDerivedItem>,
	/// Span for error reporting
	pub span: Span,
}

/// A single derived item within a derived block.
///
/// Each derived item has a name and a closure that computes a value.
/// The closure receives the form instance as a parameter and can
/// access any Signals or other derived values to create reactive dependencies.
///
/// ## Note
///
/// Unlike watch items which return `View`, derived items return a value
/// that will be wrapped in `Memo<T>`. The type `T` is inferred from
/// the closure's return type.
#[derive(Debug, Clone)]
pub struct FormDerivedItem {
	/// Derived item name (used for generated accessor method name)
	pub name: Ident,
	/// Closure that computes the derived value
	/// Signature: `|form: &FormName| -> T { ... }` or `|form| { ... }`
	pub closure: ExprClosure,
	/// Span for error reporting
	pub span: Span,
}

impl FormDerived {
	/// Creates a new empty FormDerived.
	pub fn new(span: Span) -> Self {
		Self {
			items: Vec::new(),
			span,
		}
	}

	/// Returns true if no derived items are defined.
	pub fn is_empty(&self) -> bool {
		self.items.is_empty()
	}
}

/// Slot definitions for custom UI elements in the form.
///
/// Slots allow inserting custom elements before, after, or between form fields.
///
/// ## Example
///
/// ```text
/// slots: {
///     before_fields: || {
///         div { class: "form-header", "Please fill out this form" }
///     },
///     after_fields: || {
///         button { type: "submit", "Submit" }
///     },
/// }
/// ```
#[derive(Debug, Clone)]
pub struct FormSlots {
	/// Closure rendered before all fields
	pub before_fields: Option<ExprClosure>,
	/// Closure rendered after all fields
	pub after_fields: Option<ExprClosure>,
	/// Span for error reporting
	pub span: Span,
}

impl FormSlots {
	/// Creates a new empty FormSlots.
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

impl FormMacro {
	/// Creates a new FormMacro with the given name and span.
	///
	/// Pass `None` for `name` when creating a placeholder before parsing.
	/// The parser will set the name when the `name:` property is encountered.
	pub fn new(name: Option<Ident>, span: Span) -> Self {
		Self {
			name,
			action: FormAction::None,
			method: None,
			class: None,
			state: None,
			callbacks: FormCallbacks::new(),
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
		matches!(self.action, FormAction::ServerFn(_))
	}

	/// Returns the action URL if set.
	pub fn action_url(&self) -> Option<&LitStr> {
		match &self.action {
			FormAction::Url(url) => Some(url),
			_ => None,
		}
	}

	/// Returns the server_fn path if set.
	pub fn server_fn_path(&self) -> Option<&Path> {
		match &self.action {
			FormAction::ServerFn(path) => Some(path),
			_ => None,
		}
	}
}

impl FormFieldDef {
	/// Creates a new field definition.
	pub fn new(name: Ident, field_type: Ident, span: Span) -> Self {
		Self {
			name,
			field_type,
			properties: Vec::new(),
			span,
		}
	}

	/// Returns true if this field has the `required` flag.
	pub fn is_required(&self) -> bool {
		self.properties
			.iter()
			.any(|p| matches!(p, FormFieldProperty::Flag { name, .. } if name == "required"))
	}

	/// Gets a named property value by name.
	pub fn get_property(&self, prop_name: &str) -> Option<&Expr> {
		self.properties.iter().find_map(|p| {
			if let FormFieldProperty::Named { name, value, .. } = p
				&& name == prop_name
			{
				return Some(value);
			}
			None
		})
	}

	/// Gets the widget type if specified.
	pub fn get_widget(&self) -> Option<&Ident> {
		self.properties.iter().find_map(|p| {
			if let FormFieldProperty::Widget { widget_type, .. } = p {
				Some(widget_type)
			} else {
				None
			}
		})
	}

	/// Gets the CSS class for the input element.
	pub fn get_class(&self) -> Option<&Expr> {
		self.get_property("class")
	}

	/// Gets the CSS class for the wrapper element.
	pub fn get_wrapper_class(&self) -> Option<&Expr> {
		self.get_property("wrapper_class")
	}

	/// Gets the CSS class for the label element.
	pub fn get_label_class(&self) -> Option<&Expr> {
		self.get_property("label_class")
	}

	/// Gets the CSS class for the error element.
	pub fn get_error_class(&self) -> Option<&Expr> {
		self.get_property("error_class")
	}

	/// Gets the label text if specified.
	pub fn get_label(&self) -> Option<&Expr> {
		self.get_property("label")
	}

	/// Gets the placeholder text if specified.
	pub fn get_placeholder(&self) -> Option<&Expr> {
		self.get_property("placeholder")
	}

	/// Gets the max_length constraint if specified.
	pub fn get_max_length(&self) -> Option<&Expr> {
		self.get_property("max_length")
	}

	/// Gets the min_length constraint if specified.
	pub fn get_min_length(&self) -> Option<&Expr> {
		self.get_property("min_length")
	}

	/// Gets the custom wrapper element if specified.
	pub fn get_wrapper(&self) -> Option<&WrapperElement> {
		self.properties.iter().find_map(|p| {
			if let FormFieldProperty::Wrapper { element, .. } = p {
				Some(element)
			} else {
				None
			}
		})
	}

	/// Gets the SVG icon element if specified.
	pub fn get_icon(&self) -> Option<&IconElement> {
		self.properties.iter().find_map(|p| {
			if let FormFieldProperty::Icon { element, .. } = p {
				Some(element)
			} else {
				None
			}
		})
	}

	/// Gets the icon position if specified.
	///
	/// Returns `IconPosition::Left` as the default if not explicitly set.
	pub fn get_icon_position(&self) -> IconPosition {
		self.properties
			.iter()
			.find_map(|p| {
				if let FormFieldProperty::IconPosition { position, .. } = p {
					Some(*position)
				} else {
					None
				}
			})
			.unwrap_or_default()
	}

	/// Returns true if this field has an icon.
	pub fn has_icon(&self) -> bool {
		self.get_icon().is_some()
	}

	/// Gets the custom attributes if specified.
	pub fn get_attrs(&self) -> Option<&[CustomAttr]> {
		self.properties.iter().find_map(|p| {
			if let FormFieldProperty::Attrs { attrs, .. } = p {
				Some(attrs.as_slice())
			} else {
				None
			}
		})
	}

	/// Returns true if this field has custom attributes.
	pub fn has_attrs(&self) -> bool {
		self.get_attrs().is_some()
	}

	/// Gets the bind option if explicitly specified.
	///
	/// Returns `Some(true)` if `bind: true` is specified,
	/// `Some(false)` if `bind: false` is specified,
	/// or `None` if not explicitly set.
	pub fn get_bind(&self) -> Option<bool> {
		self.properties.iter().find_map(|p| {
			if let FormFieldProperty::Bind { enabled, .. } = p {
				Some(*enabled)
			} else {
				None
			}
		})
	}

	/// Returns whether two-way binding is enabled for this field.
	///
	/// Defaults to `true` if not explicitly specified.
	pub fn is_bind_enabled(&self) -> bool {
		self.get_bind().unwrap_or(true)
	}

	/// Gets the initial_from field name if specified.
	///
	/// This specifies the source field name from the `initial_loader` result
	/// that should be used to populate this field's initial value.
	pub fn get_initial_from(&self) -> Option<&LitStr> {
		self.properties.iter().find_map(|p| {
			if let FormFieldProperty::InitialFrom { field_name, .. } = p {
				Some(field_name)
			} else {
				None
			}
		})
	}

	/// Returns true if this field has an initial_from mapping.
	pub fn has_initial_from(&self) -> bool {
		self.get_initial_from().is_some()
	}

	/// Gets the choices_from field name if specified.
	///
	/// This specifies which field in the `choices_loader` result
	/// contains the array of choice options.
	pub fn get_choices_from(&self) -> Option<&LitStr> {
		self.properties.iter().find_map(|p| {
			if let FormFieldProperty::ChoicesFrom { field_name, .. } = p {
				Some(field_name)
			} else {
				None
			}
		})
	}

	/// Returns true if this field has a choices_from mapping.
	pub fn has_choices_from(&self) -> bool {
		self.get_choices_from().is_some()
	}

	/// Gets the choice_value path if specified.
	///
	/// This specifies which property of each choice item to use as the form value.
	pub fn get_choice_value(&self) -> Option<&LitStr> {
		self.properties.iter().find_map(|p| {
			if let FormFieldProperty::ChoiceValue { path, .. } = p {
				Some(path)
			} else {
				None
			}
		})
	}

	/// Gets the choice_label path if specified.
	///
	/// This specifies which property of each choice item to use as the display label.
	pub fn get_choice_label(&self) -> Option<&LitStr> {
		self.properties.iter().find_map(|p| {
			if let FormFieldProperty::ChoiceLabel { path, .. } = p {
				Some(path)
			} else {
				None
			}
		})
	}

	/// Returns true if this is a dynamic choice field (has choices_from configured).
	pub fn is_dynamic_choice_field(&self) -> bool {
		self.has_choices_from()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_form_field_property_is_styling() {
		// Arrange
		let class_prop = FormFieldProperty::Named {
			name: Ident::new("class", Span::call_site()),
			value: syn::parse_quote!("input-field"),
			span: Span::call_site(),
		};

		let wrapper_class_prop = FormFieldProperty::Named {
			name: Ident::new("wrapper_class", Span::call_site()),
			value: syn::parse_quote!("field-wrapper"),
			span: Span::call_site(),
		};

		let label_prop = FormFieldProperty::Named {
			name: Ident::new("label", Span::call_site()),
			value: syn::parse_quote!("Username"),
			span: Span::call_site(),
		};

		let required_flag = FormFieldProperty::Flag {
			name: Ident::new("required", Span::call_site()),
			span: Span::call_site(),
		};

		// Act & Assert
		assert!(class_prop.is_styling());
		assert!(wrapper_class_prop.is_styling());
		assert!(!label_prop.is_styling());
		assert!(!required_flag.is_styling());
	}

	#[rstest]
	fn test_form_field_def_is_required() {
		// Arrange
		let mut field = FormFieldDef::new(
			Ident::new("username", Span::call_site()),
			Ident::new("CharField", Span::call_site()),
			Span::call_site(),
		);

		// Act & Assert
		assert!(!field.is_required());

		// Arrange
		field.properties.push(FormFieldProperty::Flag {
			name: Ident::new("required", Span::call_site()),
			span: Span::call_site(),
		});

		// Act & Assert
		assert!(field.is_required());
	}

	#[rstest]
	fn test_form_field_property_name_returns_some_for_named() {
		// Arrange
		let prop = FormFieldProperty::Named {
			name: Ident::new("max_length", Span::call_site()),
			value: syn::parse_quote!(100),
			span: Span::call_site(),
		};

		// Act
		let result = prop.name();

		// Assert
		assert_eq!(result.unwrap().to_string(), "max_length");
	}

	#[rstest]
	fn test_form_field_property_name_returns_some_for_flag() {
		// Arrange
		let prop = FormFieldProperty::Flag {
			name: Ident::new("required", Span::call_site()),
			span: Span::call_site(),
		};

		// Act
		let result = prop.name();

		// Assert
		assert_eq!(result.unwrap().to_string(), "required");
	}

	#[rstest]
	fn test_form_field_property_name_returns_none_for_structural_variants() {
		// Arrange
		let widget = FormFieldProperty::Widget {
			widget_type: Ident::new("PasswordInput", Span::call_site()),
			span: Span::call_site(),
		};
		let wrapper = FormFieldProperty::Wrapper {
			element: WrapperElement {
				tag: Ident::new("div", Span::call_site()),
				attrs: Vec::new(),
				span: Span::call_site(),
			},
			span: Span::call_site(),
		};
		let icon = FormFieldProperty::Icon {
			element: IconElement::new(Span::call_site()),
			span: Span::call_site(),
		};
		let icon_position = FormFieldProperty::IconPosition {
			position: IconPosition::Left,
			span: Span::call_site(),
		};
		let attrs = FormFieldProperty::Attrs {
			attrs: Vec::new(),
			span: Span::call_site(),
		};
		let bind = FormFieldProperty::Bind {
			enabled: true,
			span: Span::call_site(),
		};
		let initial_from = FormFieldProperty::InitialFrom {
			field_name: LitStr::new("username", Span::call_site()),
			span: Span::call_site(),
		};
		let choices_from = FormFieldProperty::ChoicesFrom {
			field_name: LitStr::new("choices", Span::call_site()),
			span: Span::call_site(),
		};
		let choice_value = FormFieldProperty::ChoiceValue {
			path: LitStr::new("id", Span::call_site()),
			span: Span::call_site(),
		};
		let choice_label = FormFieldProperty::ChoiceLabel {
			path: LitStr::new("label", Span::call_site()),
			span: Span::call_site(),
		};

		// Act & Assert
		assert!(widget.name().is_none());
		assert!(wrapper.name().is_none());
		assert!(icon.name().is_none());
		assert!(icon_position.name().is_none());
		assert!(attrs.name().is_none());
		assert!(bind.name().is_none());
		assert!(initial_from.name().is_none());
		assert!(choices_from.name().is_none());
		assert!(choice_value.name().is_none());
		assert!(choice_label.name().is_none());
	}

	#[rstest]
	fn test_form_macro_uses_server_fn() {
		// Arrange
		let mut form = FormMacro::new(
			Some(Ident::new("LoginForm", Span::call_site())),
			Span::call_site(),
		);

		// Act & Assert
		assert!(!form.uses_server_fn());

		// Arrange
		form.action = FormAction::ServerFn(syn::parse_quote!(submit_login));

		// Act & Assert
		assert!(form.uses_server_fn());

		// Arrange
		form.action = FormAction::Url(LitStr::new("/api/login", Span::call_site()));

		// Act & Assert
		assert!(!form.uses_server_fn());
	}
}
