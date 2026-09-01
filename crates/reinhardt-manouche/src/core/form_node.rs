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
//!         password: [
//!             #[client(on = input)]
//!             |v| v.len() >= 8 => "Password must be at least 8 characters",
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
	/// Runs after a successful generated submit; custom runtime behavior belongs
	/// in `use_form(&form)`.
	pub redirect_on_success: Option<LitStr>,
	/// Redirect URL expression on successful form submission
	///
	/// Accepts an arbitrary Rust expression (e.g. a function call returning a `String`)
	/// for dynamic redirect targets that cannot be expressed as a static `LitStr`.
	pub success_url: Option<Expr>,
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
	/// Model-backed form source and its selected fields.
	pub model_source: Option<ModelFormSource>,
	/// Unified validators. Each rule carries an optional scope annotation
	/// (`#[server]` / `#[client(on = ...)]`) that controls where it executes.
	pub validators: Vec<FormValidator>,
	/// Arguments appended positionally to the server_fn call but never exposed
	/// as user-facing form fields. Each entry maps a server_fn argument name to
	/// the expression evaluated at submit time.
	///
	/// Order in the source matches the order in which arguments are appended
	/// after the regular form-field arguments.
	///
	/// See `StripArgument` for details. Tracked under reinhardt-web#3971.
	pub strip_arguments: Vec<StripArgument>,
	/// Source syntax used for `strip_arguments`.
	pub ambient_arguments_source: Option<AmbientArgumentsSource>,
	/// Span for error reporting
	pub span: Span,
}

/// Source configuration for a model-backed form.
///
/// Named target-neutral contracts use [`ModelFormSource::contract`] and can be
/// identified with [`ModelFormSource::contract_path`]. The four public fields
/// remain unchanged for compatibility with existing AST consumers.
#[derive(Debug, Clone)]
pub struct ModelFormSource {
	/// Model type used to generate form fields.
	pub model: Path,
	/// Nameable policy enforced by the server-function payload.
	pub policy: Path,
	/// Fields selected from the model.
	pub selection: ModelFieldSelection,
	/// Presentation overrides for selected model fields.
	pub overrides: Vec<ModelFieldOverride>,
}

impl ModelFormSource {
	/// Creates a source backed by a named target-neutral model-form contract.
	pub fn contract(contract: Path, overrides: Vec<ModelFieldOverride>) -> Self {
		Self {
			model: contract,
			// An empty policy path is not a valid user-authored Rust path and is
			// reserved as the internal marker for a contract source. Keeping the
			// legacy four fields intact preserves struct-literal compatibility.
			policy: Path {
				leading_colon: None,
				segments: Default::default(),
			},
			selection: ModelFieldSelection::Fields(Vec::new()),
			overrides,
		}
	}

	/// Returns the contract path when this source is backed by a named contract.
	pub fn contract_path(&self) -> Option<&Path> {
		self.policy.segments.is_empty().then_some(&self.model)
	}
}

/// Selection policy for a model-backed form.
#[derive(Debug, Clone)]
pub enum ModelFieldSelection {
	/// Include only the listed model fields.
	Fields(Vec<Ident>),
	/// Include every model field except the listed identifiers.
	Exclude(Vec<Ident>),
}

/// Presentation override for one model-backed form field.
#[derive(Debug, Clone)]
pub struct ModelFieldOverride {
	/// Model field receiving the override.
	pub field: Ident,
	/// Optional widget identifier.
	pub widget: Option<Ident>,
	/// Optional display label.
	pub label: Option<LitStr>,
	/// Optional help text.
	pub help_text: Option<LitStr>,
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
/// Can be a regular field definition, a layout group, a repeatable collection,
/// or a submit button.
#[derive(Debug, Clone)]
pub enum FormFieldEntry {
	/// A single field definition
	Field(FormFieldDef),
	/// A group of related fields
	Group(FormFieldGroup),
	/// A repeatable collection of field entries
	Collection(Box<FormFieldCollection>),
	/// A submit button (not a data field — generates no Signal)
	SubmitButton(FormSubmitButtonDef),
	/// A reset button (not a data field).
	ResetButton(FormControlEntryDef),
	/// A generic button (not a data field).
	Button(FormControlEntryDef),
	/// An image submit control (not a generated form value).
	ImageInput(FormControlEntryDef),
	/// An output element (not a generated form value).
	Output(FormControlEntryDef),
	/// A meter element (not a generated form value).
	Meter(FormControlEntryDef),
	/// A progress element (not a generated form value).
	Progress(FormControlEntryDef),
	/// A datalist element (not a generated form value).
	Datalist(FormDatalistDef),
}

impl FormFieldEntry {
	/// Returns true if this is a field group.
	pub fn is_group(&self) -> bool {
		matches!(self, FormFieldEntry::Group(_))
	}

	/// Returns true if this is a field collection.
	pub fn is_collection(&self) -> bool {
		matches!(self, FormFieldEntry::Collection(_))
	}

	/// Returns true if this is a regular field.
	pub fn is_field(&self) -> bool {
		matches!(self, FormFieldEntry::Field(_))
	}

	/// Returns true if this is a submit button.
	pub fn is_submit_button(&self) -> bool {
		matches!(self, FormFieldEntry::SubmitButton(_))
	}

	/// Returns true if this is a non-value control entry.
	pub fn is_control_entry(&self) -> bool {
		self.as_control_entry().is_some()
	}

	/// Returns the name of the entry (field name or group name).
	pub fn name(&self) -> &Ident {
		match self {
			FormFieldEntry::Field(f) => &f.name,
			FormFieldEntry::Group(g) => &g.name,
			FormFieldEntry::Collection(collection) => &collection.name,
			FormFieldEntry::SubmitButton(b) => &b.name,
			FormFieldEntry::ResetButton(c)
			| FormFieldEntry::Button(c)
			| FormFieldEntry::ImageInput(c)
			| FormFieldEntry::Output(c)
			| FormFieldEntry::Meter(c)
			| FormFieldEntry::Progress(c) => &c.name,
			FormFieldEntry::Datalist(d) => &d.name,
		}
	}

	/// Returns the span for error reporting.
	pub fn span(&self) -> Span {
		match self {
			FormFieldEntry::Field(f) => f.span,
			FormFieldEntry::Group(g) => g.span,
			FormFieldEntry::Collection(collection) => collection.span,
			FormFieldEntry::SubmitButton(b) => b.span,
			FormFieldEntry::ResetButton(c)
			| FormFieldEntry::Button(c)
			| FormFieldEntry::ImageInput(c)
			| FormFieldEntry::Output(c)
			| FormFieldEntry::Meter(c)
			| FormFieldEntry::Progress(c) => c.span,
			FormFieldEntry::Datalist(d) => d.span,
		}
	}

	/// Returns a reference to the inner field if this is a Field variant.
	pub fn as_field(&self) -> Option<&FormFieldDef> {
		match self {
			FormFieldEntry::Field(f) => Some(f),
			_ => None,
		}
	}

	/// Returns a reference to the inner group if this is a Group variant.
	pub fn as_group(&self) -> Option<&FormFieldGroup> {
		match self {
			FormFieldEntry::Group(g) => Some(g),
			_ => None,
		}
	}

	/// Returns a reference to the inner collection if this is a Collection variant.
	pub fn as_collection(&self) -> Option<&FormFieldCollection> {
		match self {
			FormFieldEntry::Collection(collection) => Some(collection.as_ref()),
			_ => None,
		}
	}

	/// Returns a reference to the inner submit button if this is a SubmitButton variant.
	pub fn as_submit_button(&self) -> Option<&FormSubmitButtonDef> {
		match self {
			FormFieldEntry::SubmitButton(b) => Some(b),
			_ => None,
		}
	}

	/// Returns a reference to the inner control entry if this is a non-value entry.
	pub fn as_control_entry(&self) -> Option<&FormControlEntryDef> {
		match self {
			FormFieldEntry::ResetButton(c)
			| FormFieldEntry::Button(c)
			| FormFieldEntry::ImageInput(c)
			| FormFieldEntry::Output(c)
			| FormFieldEntry::Meter(c)
			| FormFieldEntry::Progress(c) => Some(c),
			_ => None,
		}
	}

	/// Returns a reference to the inner datalist if this is a Datalist variant.
	pub fn as_datalist(&self) -> Option<&FormDatalistDef> {
		match self {
			FormFieldEntry::Datalist(d) => Some(d),
			_ => None,
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
	/// Optional generic type arguments parsed from `FieldType<T1, T2, ...>`.
	/// `None` when no `<...>` follows the field type identifier in the DSL.
	pub generics: Option<syn::punctuated::Punctuated<syn::Type, syn::Token![,]>>,
	/// Field properties (validation and styling)
	pub properties: Vec<FormFieldProperty>,
	/// Span for error reporting
	pub span: Span,
}

/// A submit button definition in the form macro.
///
/// Example:
/// ```text
/// submit: SubmitButton {
///     label: "Sign in",
///     class: "btn-primary",
/// }
/// ```
#[derive(Debug, Clone)]
pub struct FormSubmitButtonDef {
	/// Button name identifier
	pub name: Ident,
	/// Button properties (label, class, id, disabled)
	pub properties: Vec<FormFieldProperty>,
	/// Span for error reporting
	pub span: Span,
}

/// Non-value form control entry kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormControlEntryKind {
	/// `<button type="reset">`.
	ResetButton,
	/// `<button type="button">`.
	Button,
	/// `<input type="image">`.
	ImageInput,
	/// `<output>`.
	Output,
	/// `<meter>`.
	Meter,
	/// `<progress>`.
	Progress,
}

/// A non-value form control entry definition in the form macro.
#[derive(Debug, Clone)]
pub struct FormControlEntryDef {
	/// Entry name identifier.
	pub name: Ident,
	/// Control entry kind.
	pub kind: FormControlEntryKind,
	/// Control properties.
	pub properties: Vec<FormFieldProperty>,
	/// Span for error reporting.
	pub span: Span,
}

/// A datalist entry definition in the form macro.
#[derive(Debug, Clone)]
pub struct FormDatalistDef {
	/// Datalist identifier. Referenced by field `list: name`.
	pub name: Ident,
	/// Datalist option properties.
	pub properties: Vec<FormFieldProperty>,
	/// Span for error reporting.
	pub span: Span,
}

/// A static choice item.
#[derive(Debug, Clone)]
pub enum FormChoiceItem {
	/// A single `<option>`.
	Option(Box<FormChoiceOption>),
	/// An `<optgroup>` containing options.
	Group(FormChoiceGroup),
}

/// A static option definition.
#[derive(Debug, Clone)]
pub struct FormChoiceOption {
	/// Option value expression.
	pub value: Expr,
	/// Optional label expression. Defaults to `value`.
	pub label: Option<Expr>,
	/// Whether the option is disabled.
	pub disabled: bool,
	/// Span for error reporting.
	pub span: Span,
}

/// A static optgroup definition.
#[derive(Debug, Clone)]
pub struct FormChoiceGroup {
	/// Group label.
	pub label: LitStr,
	/// Whether the entire group is disabled.
	pub disabled: bool,
	/// Grouped options.
	pub options: Vec<FormChoiceItem>,
	/// Span for error reporting.
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
	pub fields: Vec<FormFieldEntry>,
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

	/// Returns the number of value fields in this group.
	pub fn field_count(&self) -> usize {
		self.fields
			.iter()
			.filter(|entry| matches!(entry, FormFieldEntry::Field(_)))
			.count()
	}
}

/// A repeatable collection of fields in the form macro.
#[derive(Debug, Clone)]
pub struct FormFieldCollection {
	/// Collection name identifier
	pub name: Ident,
	/// Collection-level label text
	pub label: Option<LitStr>,
	/// Collection-level CSS class
	pub class: Option<LitStr>,
	/// Minimum number of items to render or accept
	pub min_items: Option<syn::LitInt>,
	/// Maximum number of items to render or accept
	pub max_items: Option<syn::LitInt>,
	/// Initial data source path
	pub initial_from: Option<LitStr>,
	/// Field entries within each collection item
	pub fields: Vec<FormFieldEntry>,
	/// Custom item renderer closure
	pub render_item: Option<ExprClosure>,
	/// Span for error reporting
	pub span: Span,
}

/// A widget specification before semantic validation.
#[derive(Debug, Clone)]
pub enum FormWidgetSpec {
	/// Built-in widget identifier, e.g. `TextInput`.
	Builtin(Ident),
	/// Experimental custom widget adapter specification.
	Custom(FormCustomWidgetSpec),
}

/// Experimental custom widget metadata before semantic validation.
#[derive(Debug, Clone)]
pub struct FormCustomWidgetSpec {
	/// Component function or constructor path.
	pub component: Path,
	/// Whether the `experimental` marker was present.
	pub experimental: bool,
	/// Adapter type path.
	pub adapter: Option<Path>,
	/// Source location span.
	pub span: Span,
}

/// A property within a field definition.
#[derive(Debug, Clone)]
pub enum FormFieldProperty {
	/// Named property with a value: `max_length: 100`, `label: "Username"`
	Named {
		/// Property name identifier.
		name: Ident,
		/// Property value expression.
		value: Expr,
		/// Source location span.
		span: Span,
	},
	/// Flag property (boolean true): `required`
	Flag {
		/// Flag name identifier.
		name: Ident,
		/// Source location span.
		span: Span,
	},
	/// Widget specification: `widget: PasswordInput` or experimental `CustomWidget(...)`.
	Widget {
		/// Widget specification.
		widget: FormWidgetSpec,
		/// Source location span.
		span: Span,
	},
	/// Custom wrapper element: `wrapper: div { class: "relative" }`
	Wrapper {
		/// The wrapper HTML element definition.
		element: WrapperElement,
		/// Source location span.
		span: Span,
	},
	/// SVG icon for the field: `icon: svg { ... }`
	Icon {
		/// The icon element definition.
		element: IconElement,
		/// Source location span.
		span: Span,
	},
	/// Icon position: `icon_position: "left"`
	IconPosition {
		/// The icon placement position.
		position: IconPosition,
		/// Source location span.
		span: Span,
	},
	/// Custom attributes: `attrs: { aria_label: "...", data_testid: "..." }`
	Attrs {
		/// List of custom attribute definitions.
		attrs: Vec<CustomAttr>,
		/// Source location span.
		span: Span,
	},
	/// Two-way binding option: `bind: true` or `bind: false`
	///
	/// When true (default), the form automatically generates an @input handler
	/// to update the Signal when the user types. Set to false to disable
	/// automatic binding and use a custom handler instead.
	Bind {
		/// Whether two-way binding is enabled.
		enabled: bool,
		/// Source location span.
		span: Span,
	},
	/// Initial value source: `initial_from: "field_name"`
	///
	/// Maps this field to a property in the data returned by `initial_loader`.
	/// The value is the property name in the loaded data structure.
	InitialFrom {
		/// Property name in the loaded data structure.
		field_name: LitStr,
		/// Source location span.
		span: Span,
	},
	/// Choices source for dynamic ChoiceField: `choices_from: "choices"`
	///
	/// Specifies which field in the data returned by `choices_loader` contains
	/// the array of choice options. Used with `ChoiceField` to populate
	/// radio buttons, checkboxes, or select dropdowns dynamically.
	ChoicesFrom {
		/// Property name in the loaded data for choices.
		field_name: LitStr,
		/// Source location span.
		span: Span,
	},
	/// Static select/datalist choices: `choices: [("a", "A")]`
	Choices {
		/// Static choice items.
		choices: Vec<FormChoiceItem>,
		/// Source location span.
		span: Span,
	},
	/// Choice value path: `choice_value: "id"`
	///
	/// Specifies which property of each choice item to use as the form value.
	/// The default is "value" if not specified.
	ChoiceValue {
		/// Property path for the choice value.
		path: LitStr,
		/// Source location span.
		span: Span,
	},
	/// Choice label path: `choice_label: "choice_text"`
	///
	/// Specifies which property of each choice item to use as the display label.
	/// The default is "label" if not specified.
	ChoiceLabel {
		/// Property path for the choice display label.
		path: LitStr,
		/// Source location span.
		span: Span,
	},
	/// Choice disabled path: `choice_disabled: "disabled"`
	ChoiceDisabled {
		/// Property path for the disabled flag.
		path: LitStr,
		/// Source location span.
		span: Span,
	},
	/// Choice group path: `choice_group: "group"`
	ChoiceGroup {
		/// Property path for the choice group label.
		path: LitStr,
		/// Source location span.
		span: Span,
	},
	/// Choice group disabled path: `choice_group_disabled: "group_disabled"`
	ChoiceGroupDisabled {
		/// Property path for the group disabled flag.
		path: LitStr,
		/// Source location span.
		span: Span,
	},
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
			FormFieldProperty::Choices { span, .. } => *span,
			FormFieldProperty::ChoiceValue { span, .. } => *span,
			FormFieldProperty::ChoiceLabel { span, .. } => *span,
			FormFieldProperty::ChoiceDisabled { span, .. } => *span,
			FormFieldProperty::ChoiceGroup { span, .. } => *span,
			FormFieldProperty::ChoiceGroupDisabled { span, .. } => *span,
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

/// Client-side validation trigger event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientTrigger {
	/// Fire on form submit (default for `Both` scope).
	Submit,
	/// Fire on input event (real-time as user types).
	Input,
	/// Fire on blur event (when field loses focus).
	Blur,
}

/// Validator execution scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidatorScope {
	/// Runs on both server (`.validate()`) and client (submit). Default.
	Both,
	/// Server-side only (`.validate()`).
	Server,
	/// Client-side only, with specified trigger.
	Client {
		/// Client-side trigger event.
		trigger: ClientTrigger,
	},
	/// Both server and client, with specified client trigger.
	ServerAndClient {
		/// Client-side trigger event.
		trigger: ClientTrigger,
	},
}

impl ValidatorScope {
	/// Returns `true` if this scope includes server-side execution.
	pub fn includes_server(&self) -> bool {
		matches!(
			self,
			Self::Both | Self::Server | Self::ServerAndClient { .. }
		)
	}

	/// Returns `true` if this scope includes client-side execution.
	pub fn includes_client(&self) -> bool {
		matches!(
			self,
			Self::Both | Self::Client { .. } | Self::ServerAndClient { .. }
		)
	}

	/// Returns the client trigger, if any.
	pub fn client_trigger(&self) -> Option<&ClientTrigger> {
		match self {
			Self::Both => Some(&ClientTrigger::Submit),
			Self::Client { trigger } | Self::ServerAndClient { trigger } => Some(trigger),
			Self::Server => None,
		}
	}
}

/// Validator definition (unified server+client with scope annotations).
#[derive(Debug, Clone)]
pub enum FormValidator {
	/// Field-level validator: `username: [|v| ... => "error"]`
	Field {
		/// Name of the field to validate.
		field_name: Ident,
		/// Validation rules applied to this field.
		rules: Vec<ValidatorRule>,
		/// Source location span.
		span: Span,
	},
	/// Form-level validator: `@form: [|data| ... => "error"]`
	Form {
		/// Validation rules applied to the entire form.
		rules: Vec<ValidatorRule>,
		/// Source location span.
		span: Span,
	},
}

/// A single validation rule with closure, error message, and execution scope.
#[derive(Debug, Clone)]
pub struct ValidatorRule {
	/// Execution scope (default: `Both`). See `ValidatorScope`.
	pub scope: ValidatorScope,
	/// Validation closure expression
	pub expr: ExprClosure,
	/// Error message when validation fails
	pub message: LitStr,
	/// Span for error reporting
	pub span: Span,
}

/// Argument stripped from the surface form fields and instead passed directly
/// to the underlying server_fn at submit time.
///
/// Useful for values like CSRF tokens or other ambient context that should not
/// Argument supplied from ambient context rather than from the user-facing form.
///
/// The legacy `strip_arguments` DSL name is kept as a deprecated alias for
/// `ambient_arguments`.
///
/// ## Behavior
///
/// - The expression is evaluated **on the client side at submit time** and
///   appended positionally to the server_fn call after all regular form-field
///   arguments.
/// - The argument **does not** appear in the form struct, the rendered HTML,
///   or the validation pipeline.
/// - The corresponding server_fn argument **must** exist in the server_fn
///   signature; otherwise the build fails with a standard arity mismatch.
///
/// ## Relationship to CSRF auto-injection
///
/// Prior to reinhardt-web#3971 the `form!` macro silently appended a
/// `__csrf_token: String` argument to every `method != Get` server_fn call.
/// `ambient_arguments` makes that injection explicit and generalizes it to any
/// argument the user wants to supply outside the form's field surface.
///
/// ## Example DSL
///
/// ```ignore
/// form! {
///     server_fn: submit_vote,
///     method: Post,
///     ambient_arguments: {
///         csrf_token: ::reinhardt::reinhardt_pages::csrf::get_csrf_token()
///             .unwrap_or_default(),
///     },
///     fields: { ... }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct StripArgument {
	/// Argument name as it appears in the server_fn signature.
	pub name: Ident,
	/// Expression evaluated at submit time to supply the argument's value.
	pub value: Expr,
	/// Span for error reporting.
	pub span: Span,
}

/// Source syntax used for arguments supplied outside the user-facing form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbientArgumentsSource {
	/// Current `ambient_arguments: { ... }` syntax.
	AmbientArguments,
	/// Deprecated `strip_arguments: { ... }` alias.
	StripArguments,
}

/// Parsed form submission callbacks configuration.
///
/// These legacy `form!` runtime callback clauses are retained in the parsed AST
/// so older source can produce a targeted validation error. They are rejected by
/// validation; use `use_form(&form)` for runtime behavior.
#[derive(Debug, Clone, Default)]
pub struct FormCallbacks {
	/// Legacy callback clause retained for targeted validation errors.
	pub on_submit: Option<ExprClosure>,
	/// Legacy callback clause retained for targeted validation errors.
	pub on_success: Option<ExprClosure>,
	/// Legacy callback clause retained for targeted validation errors.
	pub on_success_ref: Option<ExprClosure>,
	/// Legacy callback clause retained for targeted validation errors.
	pub on_error: Option<ExprClosure>,
	/// Legacy callback clause retained for targeted validation errors.
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
			|| self.on_success_ref.is_some()
			|| self.on_error.is_some()
			|| self.on_loading.is_some()
	}
}

/// Parsed form submission state configuration.
///
/// This legacy `form! state: { ... }` clause is retained in the parsed AST so
/// older source can produce a targeted validation error. It is rejected by
/// validation; use `use_form(&form).build().form_state()` for runtime state.
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
			success_url: None,
			initial_loader: None,
			choices_loader: None,
			slots: None,
			fields: Vec::new(),
			model_source: None,
			validators: Vec::new(),
			strip_arguments: Vec::new(),
			ambient_arguments_source: None,
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
			generics: None,
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

	/// Gets the widget specification if specified.
	pub fn get_widget(&self) -> Option<&FormWidgetSpec> {
		self.properties.iter().find_map(|p| {
			if let FormFieldProperty::Widget { widget, .. } = p {
				Some(widget)
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

	/// Gets the choice_disabled path if specified.
	pub fn get_choice_disabled(&self) -> Option<&LitStr> {
		self.properties.iter().find_map(|p| {
			if let FormFieldProperty::ChoiceDisabled { path, .. } = p {
				Some(path)
			} else {
				None
			}
		})
	}

	/// Gets the choice_group path if specified.
	pub fn get_choice_group(&self) -> Option<&LitStr> {
		self.properties.iter().find_map(|p| {
			if let FormFieldProperty::ChoiceGroup { path, .. } = p {
				Some(path)
			} else {
				None
			}
		})
	}

	/// Gets the choice_group_disabled path if specified.
	pub fn get_choice_group_disabled(&self) -> Option<&LitStr> {
		self.properties.iter().find_map(|p| {
			if let FormFieldProperty::ChoiceGroupDisabled { path, .. } = p {
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
			widget: FormWidgetSpec::Builtin(Ident::new("PasswordInput", Span::call_site())),
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
		let choices = FormFieldProperty::Choices {
			choices: Vec::new(),
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
		let choice_disabled = FormFieldProperty::ChoiceDisabled {
			path: LitStr::new("disabled", Span::call_site()),
			span: Span::call_site(),
		};
		let choice_group = FormFieldProperty::ChoiceGroup {
			path: LitStr::new("group", Span::call_site()),
			span: Span::call_site(),
		};
		let choice_group_disabled = FormFieldProperty::ChoiceGroupDisabled {
			path: LitStr::new("group_disabled", Span::call_site()),
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
		assert!(choices.name().is_none());
		assert!(choice_value.name().is_none());
		assert!(choice_label.name().is_none());
		assert!(choice_disabled.name().is_none());
		assert!(choice_group.name().is_none());
		assert!(choice_group_disabled.name().is_none());
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
