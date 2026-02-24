//! Validation and transformation logic for form! macro AST.
//!
//! This module transforms the untyped FormMacro AST into a typed TypedFormMacro,
//! performing semantic validation and type checking along the way.
//!
//! ## Validation Rules
//!
//! 1. **Field Types**: Must be valid field type identifiers (CharField, EmailField, etc.)
//! 2. **Field Properties**: Must match the expected type for each property
//! 3. **Widget Types**: Must be valid widget identifiers (TextInput, PasswordInput, etc.)
//! 4. **Required Name**: Form must have a name identifier
//! 5. **Unique Field Names**: Field names must be unique within the form
//! 6. **Valid Validators**: Validator closures must have correct signature

use proc_macro2::Span;
use std::collections::HashSet;
use syn::{Error, Result};

use reinhardt_manouche::core::{
	ClientValidator, ClientValidatorRule, FormAction, FormCallbacks, FormDerived, FormFieldDef,
	FormFieldEntry, FormFieldGroup, FormFieldProperty, FormMacro, FormMethod, FormSlots, FormState,
	FormValidator, FormWatch, IconPosition, TypedChoicesConfig, TypedClientValidator,
	TypedClientValidatorRule, TypedCustomAttr, TypedDerivedItem, TypedFieldDisplay,
	TypedFieldStyling, TypedFieldType, TypedFieldValidation, TypedFormAction, TypedFormCallbacks,
	TypedFormDerived, TypedFormFieldDef, TypedFormFieldEntry, TypedFormFieldGroup, TypedFormMacro,
	TypedFormSlots, TypedFormState, TypedFormStyling, TypedFormValidator, TypedFormWatch,
	TypedFormWatchItem, TypedIcon, TypedIconAttr, TypedIconChild, TypedIconPosition,
	TypedValidatorRule, TypedWidget, TypedWrapper, TypedWrapperAttr, ValidatorRule,
};

/// Allowlist of safe HTML tag names for wrapper and icon child elements.
///
/// Rejects dangerous tags like `<script>`, `<iframe>`, etc. to prevent XSS attacks.
///
/// Fixes #850
const ALLOWED_CHILD_TAGS: &[&str] = &[
	"div",
	"span",
	"p",
	"a",
	"b",
	"i",
	"strong",
	"em",
	"section",
	"article",
	"header",
	"footer",
	"nav",
	"aside",
	"main",
	"h1",
	"h2",
	"h3",
	"h4",
	"h5",
	"h6",
	"ul",
	"ol",
	"li",
	"dl",
	"dt",
	"dd",
	"table",
	"thead",
	"tbody",
	"tfoot",
	"tr",
	"th",
	"td",
	"caption",
	"figure",
	"figcaption",
	"blockquote",
	"pre",
	"code",
	"svg",
	"path",
	"circle",
	"rect",
	"g",
	"use",
	"symbol",
	"defs",
	"line",
	"polyline",
	"polygon",
	"text",
	"tspan",
];

/// Validates that a tag name is in the allowlist of safe HTML tags.
///
/// Returns an error if the tag is not allowed (e.g., `<script>`, `<iframe>`).
///
/// Fixes #850
fn validate_safe_tag(tag: &str, context: &str, span: Span) -> Result<()> {
	if !ALLOWED_CHILD_TAGS.contains(&tag) {
		return Err(Error::new(
			span,
			format!(
				"Tag <{}> is not allowed in {}.\n\
				Only safe HTML tags are permitted: {}.\n\n\
				Dangerous tags like <script>, <iframe>, <object>, etc. are blocked for security.",
				tag,
				context,
				ALLOWED_CHILD_TAGS.join(", "),
			),
		));
	}
	Ok(())
}

/// Validates and transforms the FormMacro AST into a typed AST.
///
/// This is the main entry point for form! macro validation.
///
/// # Errors
///
/// Returns a compilation error if any validation rule is violated.
pub(super) fn validate(ast: &FormMacro) -> Result<TypedFormMacro> {
	// Validate unique field names
	validate_unique_field_names(&ast.fields)?;

	// Transform action
	let action = transform_action(&ast.action)?;

	// Transform method
	let method = transform_method(&ast.method)?;

	// Transform form-level styling
	let styling = transform_form_styling(ast)?;

	// Transform state configuration
	let state = transform_state(&ast.state)?;

	// Transform callbacks
	let callbacks = transform_callbacks(&ast.callbacks)?;

	// Transform watch block
	let watch = transform_watch(&ast.watch)?;

	// Transform derived block
	let derived = transform_derived(&ast.derived)?;

	// Transform redirect configuration
	let redirect_on_success = transform_redirect(&ast.redirect_on_success)?;

	// Transform initial_loader (pass through the Path)
	let initial_loader = ast.initial_loader.clone();

	// Transform choices_loader (pass through the Path)
	let choices_loader = ast.choices_loader.clone();

	// Transform slots
	let slots = transform_slots(&ast.slots)?;

	// Transform fields
	let fields = transform_fields(&ast.fields)?;

	// Transform server-side validators
	let validators = transform_validators(&ast.validators, &ast.fields)?;

	// Transform client-side validators
	let client_validators = transform_client_validators(&ast.client_validators, &ast.fields)?;

	// The parser guarantees that `name` is Some after successful parsing.
	let name = ast
		.name
		.clone()
		.expect("form name must be set after parsing");

	Ok(TypedFormMacro {
		name,
		action,
		method,
		styling,
		state,
		callbacks,
		watch,
		derived,
		redirect_on_success,
		initial_loader,
		choices_loader,
		slots,
		fields,
		validators,
		client_validators,
		span: ast.span,
	})
}

/// Validates that all field names are unique.
fn validate_unique_field_names(entries: &[FormFieldEntry]) -> Result<()> {
	let mut seen = HashSet::new();

	for entry in entries {
		match entry {
			FormFieldEntry::Field(field) => {
				let name = field.name.to_string();
				if !seen.insert(name.clone()) {
					return Err(Error::new(
						field.name.span(),
						format!("duplicate field name: '{}'", name),
					));
				}
			}
			FormFieldEntry::Group(group) => {
				// Check group name is unique
				let group_name = group.name.to_string();
				if !seen.insert(group_name.clone()) {
					return Err(Error::new(
						group.name.span(),
						format!("duplicate field/group name: '{}'", group_name),
					));
				}

				// Check fields within the group
				for field in &group.fields {
					let name = field.name.to_string();
					if !seen.insert(name.clone()) {
						return Err(Error::new(
							field.name.span(),
							format!(
								"duplicate field name: '{}' (in group '{}')",
								name, group_name
							),
						));
					}
				}
			}
		}
	}

	Ok(())
}

/// Transforms FormAction to TypedFormAction.
fn transform_action(action: &FormAction) -> Result<TypedFormAction> {
	match action {
		FormAction::Url(lit) => Ok(TypedFormAction::Url(lit.value())),
		FormAction::ServerFn(ident) => Ok(TypedFormAction::ServerFn(ident.clone())),
		FormAction::None => Ok(TypedFormAction::None),
	}
}

/// Transforms method identifier to FormMethod enum.
fn transform_method(method: &Option<syn::Ident>) -> Result<FormMethod> {
	match method {
		Some(ident) => {
			let method_str = ident.to_string();
			match method_str.to_lowercase().as_str() {
				"get" => Ok(FormMethod::Get),
				"post" => Ok(FormMethod::Post),
				"put" => Ok(FormMethod::Put),
				"patch" => Ok(FormMethod::Patch),
				"delete" => Ok(FormMethod::Delete),
				_ => Err(Error::new(
					ident.span(),
					format!(
						"invalid HTTP method: '{}'. Expected: Get, Post, Put, Patch, or Delete",
						method_str
					),
				)),
			}
		}
		None => Ok(FormMethod::Post), // Default to POST
	}
}

/// Transforms form-level styling from FormMacro.
fn transform_form_styling(ast: &FormMacro) -> Result<TypedFormStyling> {
	Ok(TypedFormStyling {
		class: ast.class.as_ref().map(|lit| lit.value()),
	})
}

/// Valid state field names for form UI state management.
const VALID_STATE_FIELDS: &[&str] = &["loading", "error", "success"];

/// Transforms FormState to TypedFormState with validation.
///
/// Validates that all state field names are valid (`loading`, `error`, `success`).
fn transform_state(state: &Option<FormState>) -> Result<Option<TypedFormState>> {
	let Some(form_state) = state else {
		return Ok(None);
	};

	let mut typed_state = TypedFormState::new(form_state.span);

	for field in &form_state.fields {
		let name = field.name.to_string();
		match name.as_str() {
			"loading" => typed_state.loading = true,
			"error" => typed_state.error = true,
			"success" => typed_state.success = true,
			_ => {
				return Err(Error::new(
					field.span,
					format!(
						"invalid state field: '{}'. Expected one of: {}",
						name,
						VALID_STATE_FIELDS.join(", ")
					),
				));
			}
		}
	}

	Ok(Some(typed_state))
}

/// Transforms FormCallbacks to TypedFormCallbacks.
///
/// For callbacks, we simply pass through the closure expressions since
/// type checking is done by the Rust compiler during code generation.
fn transform_callbacks(callbacks: &FormCallbacks) -> Result<TypedFormCallbacks> {
	Ok(TypedFormCallbacks {
		on_submit: callbacks.on_submit.clone(),
		on_success: callbacks.on_success.clone(),
		on_error: callbacks.on_error.clone(),
		on_loading: callbacks.on_loading.clone(),
		span: callbacks.span,
	})
}

/// Transforms FormWatch to TypedFormWatch.
///
/// Watch items are validated for:
/// - Unique watch item names
/// - Valid closure structure (type checking is done by Rust compiler)
fn transform_watch(watch: &Option<FormWatch>) -> Result<Option<TypedFormWatch>> {
	let Some(watch) = watch else {
		return Ok(None);
	};

	// Validate unique watch item names
	let mut seen_names = HashSet::new();
	for item in &watch.items {
		let name = item.name.to_string();
		if !seen_names.insert(name.clone()) {
			return Err(Error::new(
				item.name.span(),
				format!("duplicate watch item name: '{}'", name),
			));
		}
	}

	// Transform watch items
	let items = watch
		.items
		.iter()
		.map(|item| TypedFormWatchItem {
			name: item.name.clone(),
			closure: item.closure.clone(),
			span: item.span,
		})
		.collect();

	Ok(Some(TypedFormWatch {
		items,
		span: watch.span,
	}))
}

/// Transforms FormDerived to TypedFormDerived.
///
/// Derived items are validated for:
/// - Unique derived item names
/// - No conflicts with watch item names or field names
/// - Valid closure structure (type checking is done by Rust compiler)
fn transform_derived(derived: &Option<FormDerived>) -> Result<Option<TypedFormDerived>> {
	let Some(derived) = derived else {
		return Ok(None);
	};

	// Validate unique derived item names
	let mut seen_names = HashSet::new();
	for item in &derived.items {
		let name = item.name.to_string();
		if !seen_names.insert(name.clone()) {
			return Err(Error::new(
				item.name.span(),
				format!("duplicate derived item name: '{}'", name),
			));
		}
	}

	// Transform derived items
	let items = derived
		.items
		.iter()
		.map(|item| TypedDerivedItem {
			name: item.name.clone(),
			closure: item.closure.clone(),
			span: item.span,
		})
		.collect();

	Ok(Some(TypedFormDerived {
		items,
		span: derived.span,
	}))
}

/// Transforms redirect_on_success configuration.
///
/// Validates that the redirect path:
/// - Starts with `/` (relative paths)
/// - Or is a valid URL pattern
/// - Supports `{param}` syntax for dynamic parameters
fn transform_redirect(redirect: &Option<syn::LitStr>) -> Result<Option<String>> {
	let Some(redirect) = redirect else {
		return Ok(None);
	};

	let path = redirect.value();

	// Validate path format
	if path.is_empty() {
		return Err(Error::new(
			redirect.span(),
			"redirect_on_success path cannot be empty",
		));
	}

	// Reject insecure HTTP URLs - redirect may leak credentials or session tokens
	if path.starts_with("http://") {
		return Err(Error::new(
			redirect.span(),
			"redirect_on_success rejects insecure HTTP URLs to prevent credential leakage; \
			 use HTTPS or a relative path instead",
		));
	}

	// Path must start with / or be a valid HTTPS URL
	if !path.starts_with('/') && !path.starts_with("https://") {
		return Err(Error::new(
			redirect.span(),
			"redirect_on_success path must start with '/' or be a full HTTPS URL (https://)",
		));
	}

	Ok(Some(path))
}

/// Transforms FormSlots to TypedFormSlots.
///
/// Slots allow inserting custom content before and after form fields.
/// The closures are passed through directly since type checking is done by the Rust compiler.
fn transform_slots(slots: &Option<FormSlots>) -> Result<Option<TypedFormSlots>> {
	let Some(slots) = slots else {
		return Ok(None);
	};

	Ok(Some(TypedFormSlots {
		before_fields: slots.before_fields.clone(),
		after_fields: slots.after_fields.clone(),
		span: slots.span,
	}))
}

/// Transforms all field entries (fields and field groups).
fn transform_fields(entries: &[FormFieldEntry]) -> Result<Vec<TypedFormFieldEntry>> {
	entries.iter().map(transform_field_entry).collect()
}

/// Transforms a single field entry (either a field or a group).
fn transform_field_entry(entry: &FormFieldEntry) -> Result<TypedFormFieldEntry> {
	match entry {
		FormFieldEntry::Field(field) => {
			let typed_field = transform_field(field)?;
			Ok(TypedFormFieldEntry::Field(Box::new(typed_field)))
		}
		FormFieldEntry::Group(group) => {
			let typed_group = transform_field_group(group)?;
			Ok(TypedFormFieldEntry::Group(typed_group))
		}
	}
}

/// Transforms a field group into a typed field group.
fn transform_field_group(group: &FormFieldGroup) -> Result<TypedFormFieldGroup> {
	// Transform each field in the group
	let typed_fields: Vec<TypedFormFieldDef> = group
		.fields
		.iter()
		.map(transform_field)
		.collect::<Result<_>>()?;

	Ok(TypedFormFieldGroup {
		name: group.name.clone(),
		label: group.label.as_ref().map(|l| l.value()),
		class: group.class.as_ref().map(|c| c.value()),
		fields: typed_fields,
		span: group.span,
	})
}

/// Transforms a single field definition.
fn transform_field(field: &FormFieldDef) -> Result<TypedFormFieldDef> {
	// Parse field type
	let field_type = parse_field_type(&field.field_type)?;

	// Extract properties into categories
	let validation = extract_validation_properties(&field.properties)?;
	let display = extract_display_properties(&field.properties)?;
	let styling = extract_styling_properties(&field.properties)?;
	let widget = extract_widget(&field.properties, &field_type)?;
	let wrapper = extract_wrapper(&field.properties)?;
	let icon = extract_icon(&field.properties)?;
	let custom_attrs = extract_custom_attrs(&field.properties)?;
	let bind = extract_bind(&field.properties);
	let initial_from = extract_initial_from(&field.properties);
	let choices_config = extract_choices_config(&field.properties);

	Ok(TypedFormFieldDef {
		name: field.name.clone(),
		field_type,
		widget,
		validation,
		display,
		styling,
		wrapper,
		icon,
		custom_attrs,
		bind,
		initial_from,
		choices_config,
		span: field.span,
	})
}

/// Parses field type identifier into TypedFieldType enum.
fn parse_field_type(ident: &syn::Ident) -> Result<TypedFieldType> {
	let type_str = ident.to_string();
	match type_str.as_str() {
		"CharField" => Ok(TypedFieldType::CharField),
		"TextField" => Ok(TypedFieldType::TextField),
		"EmailField" => Ok(TypedFieldType::EmailField),
		"PasswordField" => Ok(TypedFieldType::PasswordField),
		"IntegerField" => Ok(TypedFieldType::IntegerField),
		"FloatField" => Ok(TypedFieldType::FloatField),
		"DecimalField" => Ok(TypedFieldType::DecimalField),
		"BooleanField" => Ok(TypedFieldType::BooleanField),
		"DateField" => Ok(TypedFieldType::DateField),
		"TimeField" => Ok(TypedFieldType::TimeField),
		"DateTimeField" => Ok(TypedFieldType::DateTimeField),
		"ChoiceField" => Ok(TypedFieldType::ChoiceField),
		"MultipleChoiceField" => Ok(TypedFieldType::MultipleChoiceField),
		"FileField" => Ok(TypedFieldType::FileField),
		"ImageField" => Ok(TypedFieldType::ImageField),
		"UrlField" => Ok(TypedFieldType::UrlField),
		"SlugField" => Ok(TypedFieldType::SlugField),
		"UuidField" => Ok(TypedFieldType::UuidField),
		"IpAddressField" => Ok(TypedFieldType::IpAddressField),
		"JsonField" => Ok(TypedFieldType::JsonField),
		"HiddenField" => Ok(TypedFieldType::HiddenField),
		_ => Err(Error::new(
			ident.span(),
			format!(
				"unknown field type: '{}'. Expected one of: CharField, TextField, EmailField, \
				PasswordField, IntegerField, FloatField, DecimalField, BooleanField, DateField, \
				TimeField, DateTimeField, ChoiceField, MultipleChoiceField, FileField, ImageField, \
				UrlField, SlugField, UuidField, IpAddressField, JsonField, HiddenField",
				type_str
			),
		)),
	}
}

/// Extracts validation-related properties.
fn extract_validation_properties(properties: &[FormFieldProperty]) -> Result<TypedFieldValidation> {
	let mut required = false;
	let mut min_length = None;
	let mut max_length = None;
	let mut min_value = None;
	let mut max_value = None;
	let mut pattern = None;

	for prop in properties {
		match prop {
			FormFieldProperty::Flag { name, span: _ } => {
				if name == "required" {
					required = true;
				}
				// Ignore other flags
			}
			FormFieldProperty::Named { name, value, span } => {
				let name_str = name.to_string();
				match name_str.as_str() {
					"required" => {
						if let syn::Expr::Lit(lit) = value {
							if let syn::Lit::Bool(b) = &lit.lit {
								required = b.value;
							} else {
								return Err(Error::new(
									*span,
									"'required' must be a boolean value",
								));
							}
						} else {
							return Err(Error::new(*span, "'required' must be a boolean value"));
						}
					}
					"min_length" => {
						min_length = Some(extract_int_value_from_expr(value, "min_length", *span)?);
					}
					"max_length" => {
						max_length = Some(extract_int_value_from_expr(value, "max_length", *span)?);
					}
					"min_value" => {
						min_value = Some(extract_int_value_from_expr(value, "min_value", *span)?);
					}
					"max_value" => {
						max_value = Some(extract_int_value_from_expr(value, "max_value", *span)?);
					}
					"pattern" => {
						pattern = Some(extract_string_value_from_expr(value, "pattern", *span)?);
					}
					_ => {} // Ignore non-validation properties
				}
			}
			FormFieldProperty::Widget { .. } => {} // Ignore widget properties
			FormFieldProperty::Wrapper { .. } => {} // Ignore wrapper properties
			FormFieldProperty::Icon { .. } => {}   // Ignore icon properties
			FormFieldProperty::IconPosition { .. } => {} // Ignore icon position properties
			FormFieldProperty::Attrs { .. } => {}  // Ignore custom attrs properties
			FormFieldProperty::Bind { .. } => {}   // Ignore bind properties
			FormFieldProperty::InitialFrom { .. } => {} // Ignore initial_from properties
			FormFieldProperty::ChoicesFrom { .. } => {} // Ignore choices_from properties
			FormFieldProperty::ChoiceValue { .. } => {} // Ignore choice_value properties
			FormFieldProperty::ChoiceLabel { .. } => {} // Ignore choice_label properties
		}
	}

	Ok(TypedFieldValidation {
		required,
		min_length,
		max_length,
		min_value,
		max_value,
		pattern,
	})
}

/// Extracts display-related properties.
fn extract_display_properties(properties: &[FormFieldProperty]) -> Result<TypedFieldDisplay> {
	let mut label = None;
	let mut placeholder = None;
	let mut help_text = None;
	let mut disabled = false;
	let mut readonly = false;
	let mut autofocus = false;

	for prop in properties {
		match prop {
			FormFieldProperty::Flag { name, .. } => {
				let name_str = name.to_string();
				match name_str.as_str() {
					"disabled" => disabled = true,
					"readonly" => readonly = true,
					"autofocus" => autofocus = true,
					_ => {} // Ignore other flags
				}
			}
			FormFieldProperty::Named { name, value, span } => {
				let name_str = name.to_string();
				match name_str.as_str() {
					"label" => {
						label = Some(extract_string_value_from_expr(value, "label", *span)?);
					}
					"placeholder" => {
						placeholder =
							Some(extract_string_value_from_expr(value, "placeholder", *span)?);
					}
					"help_text" => {
						help_text =
							Some(extract_string_value_from_expr(value, "help_text", *span)?);
					}
					"disabled" => {
						if let syn::Expr::Lit(lit) = value
							&& let syn::Lit::Bool(b) = &lit.lit
						{
							disabled = b.value;
						} else {
							return Err(Error::new(
								*span,
								"'disabled' must be a boolean value (true or false), or use as a flag without a value",
							));
						}
					}
					"readonly" => {
						if let syn::Expr::Lit(lit) = value
							&& let syn::Lit::Bool(b) = &lit.lit
						{
							readonly = b.value;
						} else {
							return Err(Error::new(
								*span,
								"'readonly' must be a boolean value (true or false), or use as a flag without a value",
							));
						}
					}
					"autofocus" => {
						if let syn::Expr::Lit(lit) = value
							&& let syn::Lit::Bool(b) = &lit.lit
						{
							autofocus = b.value;
						} else {
							return Err(Error::new(
								*span,
								"'autofocus' must be a boolean value (true or false), or use as a flag without a value",
							));
						}
					}
					_ => {} // Ignore non-display properties
				}
			}
			FormFieldProperty::Widget { .. } => {} // Ignore widget properties
			FormFieldProperty::Wrapper { .. } => {} // Ignore wrapper properties
			FormFieldProperty::Icon { .. } => {}   // Ignore icon properties
			FormFieldProperty::IconPosition { .. } => {} // Ignore icon position properties
			FormFieldProperty::Attrs { .. } => {}  // Ignore custom attrs properties
			FormFieldProperty::Bind { .. } => {}   // Ignore bind properties
			FormFieldProperty::InitialFrom { .. } => {} // Ignore initial_from properties
			FormFieldProperty::ChoicesFrom { .. } => {} // Ignore choices_from properties
			FormFieldProperty::ChoiceValue { .. } => {} // Ignore choice_value properties
			FormFieldProperty::ChoiceLabel { .. } => {} // Ignore choice_label properties
		}
	}

	Ok(TypedFieldDisplay {
		label,
		placeholder,
		help_text,
		disabled,
		readonly,
		autofocus,
	})
}

/// Extracts styling-related properties.
fn extract_styling_properties(properties: &[FormFieldProperty]) -> Result<TypedFieldStyling> {
	let mut class = None;
	let mut wrapper_class = None;
	let mut label_class = None;
	let mut error_class = None;

	for prop in properties {
		if let FormFieldProperty::Named { name, value, span } = prop {
			let name_str = name.to_string();
			match name_str.as_str() {
				"class" => {
					class = Some(extract_string_value_from_expr(value, "class", *span)?);
				}
				"wrapper_class" => {
					wrapper_class = Some(extract_string_value_from_expr(
						value,
						"wrapper_class",
						*span,
					)?);
				}
				"label_class" => {
					label_class =
						Some(extract_string_value_from_expr(value, "label_class", *span)?);
				}
				"error_class" => {
					error_class =
						Some(extract_string_value_from_expr(value, "error_class", *span)?);
				}
				_ => {} // Ignore non-styling properties
			}
		}
	}

	Ok(TypedFieldStyling {
		class,
		wrapper_class,
		label_class,
		error_class,
	})
}

/// Extracts widget property and returns TypedWidget.
fn extract_widget(
	properties: &[FormFieldProperty],
	field_type: &TypedFieldType,
) -> Result<TypedWidget> {
	// Look for explicit widget property
	for prop in properties {
		match prop {
			FormFieldProperty::Widget {
				widget_type,
				span: _,
			} => {
				return parse_widget(widget_type);
			}
			FormFieldProperty::Named { name, value, span } if name == "widget" => {
				// Handle widget specified as named property: widget: PasswordInput
				if let syn::Expr::Path(path) = value
					&& let Some(ident) = path.path.get_ident()
				{
					return parse_widget(ident);
				}
				return Err(Error::new(
					*span,
					"'widget' must be a widget type identifier (e.g., TextInput, PasswordInput)",
				));
			}
			_ => {} // Continue searching
		}
	}

	// Return default widget for field type
	Ok(field_type.default_widget())
}

/// Parses widget identifier into TypedWidget enum.
fn parse_widget(ident: &syn::Ident) -> Result<TypedWidget> {
	let widget_str = ident.to_string();
	match widget_str.as_str() {
		"TextInput" => Ok(TypedWidget::TextInput),
		"PasswordInput" => Ok(TypedWidget::PasswordInput),
		"EmailInput" => Ok(TypedWidget::EmailInput),
		"NumberInput" => Ok(TypedWidget::NumberInput),
		"Textarea" => Ok(TypedWidget::Textarea),
		"CheckboxInput" => Ok(TypedWidget::CheckboxInput),
		"RadioSelect" => Ok(TypedWidget::RadioSelect),
		"Select" => Ok(TypedWidget::Select),
		"SelectMultiple" => Ok(TypedWidget::SelectMultiple),
		"DateInput" => Ok(TypedWidget::DateInput),
		"TimeInput" => Ok(TypedWidget::TimeInput),
		"DateTimeInput" => Ok(TypedWidget::DateTimeInput),
		"FileInput" => Ok(TypedWidget::FileInput),
		"HiddenInput" => Ok(TypedWidget::HiddenInput),
		"ColorInput" => Ok(TypedWidget::ColorInput),
		"RangeInput" => Ok(TypedWidget::RangeInput),
		"UrlInput" => Ok(TypedWidget::UrlInput),
		"TelInput" => Ok(TypedWidget::TelInput),
		"SearchInput" => Ok(TypedWidget::SearchInput),
		_ => Err(Error::new(
			ident.span(),
			format!(
				"unknown widget type: '{}'. Expected one of: TextInput, PasswordInput, \
				EmailInput, NumberInput, Textarea, CheckboxInput, RadioSelect, Select, \
				SelectMultiple, DateInput, TimeInput, DateTimeInput, FileInput, HiddenInput, \
				ColorInput, RangeInput, UrlInput, TelInput, SearchInput",
				widget_str
			),
		)),
	}
}

/// Extracts wrapper property and transforms it into `TypedWrapper`.
///
/// Wrapper properties specify custom HTML elements to wrap around form fields:
///
/// ```text
/// wrapper: div { class: "relative", id: "field-wrapper" }
/// ```
fn extract_wrapper(properties: &[FormFieldProperty]) -> Result<Option<TypedWrapper>> {
	for prop in properties {
		if let FormFieldProperty::Wrapper { element, span } = prop {
			// Validate wrapper tag name against allowlist (Fixes #850)
			validate_safe_tag(&element.tag.to_string(), "wrapper", *span)?;

			// Transform wrapper attributes
			let attrs = element
				.attrs
				.iter()
				.map(|attr| {
					let value = extract_string_value_from_expr(
						&attr.value,
						&attr.name.to_string(),
						attr.span,
					)?;
					Ok(TypedWrapperAttr {
						name: attr.name.to_string(),
						value,
						span: attr.span,
					})
				})
				.collect::<Result<Vec<_>>>()?;

			return Ok(Some(TypedWrapper {
				tag: element.tag.to_string(),
				attrs,
				span: *span,
			}));
		}
	}
	Ok(None)
}

/// Extracts icon properties and transforms them into `TypedIcon`.
///
/// Icon properties specify an SVG icon to display with the form field:
///
/// ```text
/// icon: svg {
///     class: "w-5 h-5 text-gray-400",
///     viewBox: "0 0 24 24",
///     path { d: "M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4z" }
/// }
/// icon_position: "left"
/// ```
fn extract_icon(properties: &[FormFieldProperty]) -> Result<Option<TypedIcon>> {
	// First, find the icon element if it exists
	let mut icon_element = None;
	let mut icon_span = Span::call_site();
	let mut position = TypedIconPosition::default();

	for prop in properties {
		match prop {
			FormFieldProperty::Icon { element, span } => {
				icon_element = Some(element);
				icon_span = *span;
			}
			FormFieldProperty::IconPosition {
				position: pos,
				span: _,
			} => {
				position = convert_icon_position(*pos);
			}
			_ => {}
		}
	}

	// If no icon element, return None
	let element = match icon_element {
		Some(e) => e,
		None => return Ok(None),
	};

	// Transform icon attributes
	let attrs = element
		.attrs
		.iter()
		.map(|attr| {
			let value =
				extract_string_value_from_expr(&attr.value, &attr.name.to_string(), attr.span)?;
			Ok(TypedIconAttr {
				name: attr.name.to_string(),
				value,
				span: attr.span,
			})
		})
		.collect::<Result<Vec<_>>>()?;

	// Transform children recursively
	let children = element
		.children
		.iter()
		.map(transform_icon_child)
		.collect::<Result<Vec<_>>>()?;

	Ok(Some(TypedIcon {
		attrs,
		children,
		position,
		span: icon_span,
	}))
}

/// Transforms a single icon child element recursively.
fn transform_icon_child(child: &reinhardt_manouche::core::IconChild) -> Result<TypedIconChild> {
	// Validate icon child tag name against allowlist (Fixes #850)
	validate_safe_tag(&child.tag.to_string(), "icon child", child.span)?;

	let attrs = child
		.attrs
		.iter()
		.map(|attr| {
			let value =
				extract_string_value_from_expr(&attr.value, &attr.name.to_string(), attr.span)?;
			Ok(TypedIconAttr {
				name: attr.name.to_string(),
				value,
				span: attr.span,
			})
		})
		.collect::<Result<Vec<_>>>()?;

	// Recursively transform nested children
	let children = child
		.children
		.iter()
		.map(transform_icon_child)
		.collect::<Result<Vec<_>>>()?;

	Ok(TypedIconChild {
		tag: child.tag.to_string(),
		attrs,
		children,
		span: child.span,
	})
}

/// Converts untyped IconPosition to TypedIconPosition.
fn convert_icon_position(pos: IconPosition) -> TypedIconPosition {
	match pos {
		IconPosition::Left => TypedIconPosition::Left,
		IconPosition::Right => TypedIconPosition::Right,
		IconPosition::Label => TypedIconPosition::Label,
	}
}

/// Extracts custom attributes (aria-*, data-*) from field properties.
///
/// Custom attributes allow adding accessibility and data attributes to form fields:
///
/// ```text
/// attrs: {
///     aria_label: "Email address",
///     aria_required: "true",
///     data_testid: "email-input",
/// }
/// ```
///
/// Note: Only `aria_*` and `data_*` prefixed attribute names are allowed.
fn extract_custom_attrs(properties: &[FormFieldProperty]) -> Result<Vec<TypedCustomAttr>> {
	for prop in properties {
		if let FormFieldProperty::Attrs { attrs, span: _ } = prop {
			let mut result = Vec::new();

			for attr in attrs {
				let name = attr.name.to_string();

				// Validate that attribute name starts with aria_ or data_
				if !name.starts_with("aria_") && !name.starts_with("data_") {
					return Err(Error::new(
						attr.span,
						format!(
							"invalid custom attribute: '{}'. \
							Custom attributes must start with 'aria_' or 'data_' prefix",
							name
						),
					));
				}

				// Extract the string value
				let value = extract_string_value_from_expr(&attr.value, &name, attr.span)?;

				result.push(TypedCustomAttr {
					name,
					value,
					span: attr.span,
				});
			}

			return Ok(result);
		}
	}

	Ok(Vec::new())
}

/// Extracts the bind property from field properties.
///
/// Returns `true` (enabled) if not explicitly specified.
fn extract_bind(properties: &[FormFieldProperty]) -> bool {
	for prop in properties {
		if let FormFieldProperty::Bind { enabled, .. } = prop {
			return *enabled;
		}
	}
	// Default to enabled
	true
}

/// Extracts the initial_from property from field properties.
///
/// This specifies which field from the initial_loader result should be used
/// to populate this field's initial value.
///
/// ```text
/// initial_from: "source_field_name"
/// ```
fn extract_initial_from(properties: &[FormFieldProperty]) -> Option<String> {
	for prop in properties {
		if let FormFieldProperty::InitialFrom { field_name, .. } = prop {
			return Some(field_name.value());
		}
	}
	None
}

/// Extracts dynamic choices configuration from field properties.
///
/// For `ChoiceField` with dynamic options loaded from a `choices_loader` server_fn.
///
/// ```text
/// choices_from: "choices"
/// choice_value: "id"
/// choice_label: "choice_text"
/// ```
fn extract_choices_config(properties: &[FormFieldProperty]) -> Option<TypedChoicesConfig> {
	let mut choices_from: Option<(String, Span)> = None;
	let mut choice_value: Option<String> = None;
	let mut choice_label: Option<String> = None;

	for prop in properties {
		match prop {
			FormFieldProperty::ChoicesFrom { field_name, span } => {
				choices_from = Some((field_name.value(), *span));
			}
			FormFieldProperty::ChoiceValue { path, .. } => {
				choice_value = Some(path.value());
			}
			FormFieldProperty::ChoiceLabel { path, .. } => {
				choice_label = Some(path.value());
			}
			_ => {}
		}
	}

	// Only return config if choices_from is specified
	choices_from.map(|(from, span)| {
		TypedChoicesConfig::with_paths(
			from,
			choice_value.unwrap_or_else(|| "value".to_string()),
			choice_label.unwrap_or_else(|| "label".to_string()),
			span,
		)
	})
}

/// Checks if a field with the given name exists in the field entries.
///
/// This checks both top-level fields and fields within groups.
fn field_exists(entries: &[FormFieldEntry], name: &syn::Ident) -> bool {
	for entry in entries {
		match entry {
			FormFieldEntry::Field(field) => {
				if field.name == *name {
					return true;
				}
			}
			FormFieldEntry::Group(group) => {
				if group.fields.iter().any(|f| f.name == *name) {
					return true;
				}
			}
		}
	}
	false
}

/// Transforms server-side validators.
fn transform_validators(
	validators: &[FormValidator],
	fields: &[FormFieldEntry],
) -> Result<Vec<TypedFormValidator>> {
	let mut result = Vec::new();

	for validator in validators {
		match validator {
			FormValidator::Field {
				field_name,
				rules,
				span,
			} => {
				// Validate that field exists (including in groups)
				if !field_exists(fields, field_name) {
					return Err(Error::new(
						field_name.span(),
						format!("validator references unknown field: '{}'", field_name),
					));
				}

				let typed_rules = rules
					.iter()
					.map(transform_validator_rule)
					.collect::<Result<Vec<_>>>()?;

				result.push(TypedFormValidator {
					field_name: field_name.clone(),
					rules: typed_rules,
					span: *span,
				});
			}
			// Fixes #848: emit compile error instead of silently discarding
			FormValidator::Form { rules: _, span } => {
				return Err(Error::new(
					*span,
					"form-level validators (@form) are not yet supported. \
					Use field-level validators instead",
				));
			}
		}
	}

	Ok(result)
}

/// Transforms a validator rule.
///
/// Converts the closure expression to a regular expression for code generation.
fn transform_validator_rule(rule: &ValidatorRule) -> Result<TypedValidatorRule> {
	// Convert ExprClosure body to Expr for use in validation
	let condition: syn::Expr = (*rule.expr.body).clone();

	Ok(TypedValidatorRule {
		condition,
		message: rule.message.value(),
		span: rule.span,
	})
}

/// Transforms client-side validators.
fn transform_client_validators(
	validators: &[ClientValidator],
	fields: &[FormFieldEntry],
) -> Result<Vec<TypedClientValidator>> {
	validators
		.iter()
		.map(|v| transform_client_validator(v, fields))
		.collect()
}

/// Transforms a single client-side validator.
fn transform_client_validator(
	validator: &ClientValidator,
	fields: &[FormFieldEntry],
) -> Result<TypedClientValidator> {
	// Validate that field exists (including in groups)
	if !field_exists(fields, &validator.field_name) {
		return Err(Error::new(
			validator.field_name.span(),
			format!(
				"client validator references unknown field: '{}'",
				validator.field_name
			),
		));
	}

	let rules = validator
		.rules
		.iter()
		.map(transform_client_validator_rule)
		.collect::<Result<Vec<_>>>()?;

	Ok(TypedClientValidator {
		field_name: validator.field_name.clone(),
		rules,
		span: validator.span,
	})
}

/// Transforms a client validator rule.
fn transform_client_validator_rule(rule: &ClientValidatorRule) -> Result<TypedClientValidatorRule> {
	Ok(TypedClientValidatorRule {
		js_condition: rule.js_expr.value(),
		message: rule.message.value(),
		span: rule.span,
	})
}

/// Extracts an integer value from an optional expression.
/// Reserved for future enhanced validation.
#[allow(dead_code)]
fn extract_int_value(value: &Option<syn::Expr>, prop_name: &str, span: Span) -> Result<i64> {
	match value {
		Some(syn::Expr::Lit(lit)) => {
			if let syn::Lit::Int(int_lit) = &lit.lit {
				int_lit.base10_parse::<i64>().map_err(|_| {
					Error::new(span, format!("'{}' must be a valid integer", prop_name))
				})
			} else {
				Err(Error::new(
					span,
					format!("'{}' must be an integer value", prop_name),
				))
			}
		}
		Some(syn::Expr::Unary(unary)) => {
			// Handle negative numbers like -10
			if let syn::UnOp::Neg(_) = unary.op
				&& let syn::Expr::Lit(lit) = &*unary.expr
				&& let syn::Lit::Int(int_lit) = &lit.lit
			{
				let val = int_lit.base10_parse::<i64>().map_err(|_| {
					Error::new(span, format!("'{}' must be a valid integer", prop_name))
				})?;
				return Ok(-val);
			}
			Err(Error::new(
				span,
				format!("'{}' must be an integer value", prop_name),
			))
		}
		None => Err(Error::new(
			span,
			format!("'{}' requires a value", prop_name),
		)),
		_ => Err(Error::new(
			span,
			format!("'{}' must be an integer value", prop_name),
		)),
	}
}

/// Extracts a string value from an optional expression.
/// Reserved for future enhanced validation.
#[allow(dead_code)]
fn extract_string_value(value: &Option<syn::Expr>, prop_name: &str, span: Span) -> Result<String> {
	match value {
		Some(syn::Expr::Lit(lit)) => {
			if let syn::Lit::Str(str_lit) = &lit.lit {
				Ok(str_lit.value())
			} else {
				Err(Error::new(
					span,
					format!("'{}' must be a string value", prop_name),
				))
			}
		}
		None => Err(Error::new(
			span,
			format!("'{}' requires a value", prop_name),
		)),
		_ => Err(Error::new(
			span,
			format!("'{}' must be a string value", prop_name),
		)),
	}
}

/// Extracts an integer value from an expression (non-optional version).
fn extract_int_value_from_expr(value: &syn::Expr, prop_name: &str, span: Span) -> Result<i64> {
	match value {
		syn::Expr::Lit(lit) => {
			if let syn::Lit::Int(int_lit) = &lit.lit {
				int_lit.base10_parse::<i64>().map_err(|_| {
					Error::new(span, format!("'{}' must be a valid integer", prop_name))
				})
			} else {
				Err(Error::new(
					span,
					format!("'{}' must be an integer value", prop_name),
				))
			}
		}
		syn::Expr::Unary(unary) => {
			// Handle negative numbers like -10
			if let syn::UnOp::Neg(_) = unary.op
				&& let syn::Expr::Lit(lit) = &*unary.expr
				&& let syn::Lit::Int(int_lit) = &lit.lit
			{
				let val = int_lit.base10_parse::<i64>().map_err(|_| {
					Error::new(span, format!("'{}' must be a valid integer", prop_name))
				})?;
				return Ok(-val);
			}
			Err(Error::new(
				span,
				format!("'{}' must be an integer value", prop_name),
			))
		}
		_ => Err(Error::new(
			span,
			format!("'{}' must be an integer value", prop_name),
		)),
	}
}

/// Extracts a string value from an expression (non-optional version).
fn extract_string_value_from_expr(
	value: &syn::Expr,
	prop_name: &str,
	span: Span,
) -> Result<String> {
	match value {
		syn::Expr::Lit(lit) => {
			if let syn::Lit::Str(str_lit) = &lit.lit {
				Ok(str_lit.value())
			} else {
				Err(Error::new(
					span,
					format!("'{}' must be a string value", prop_name),
				))
			}
		}
		_ => Err(Error::new(
			span,
			format!("'{}' must be a string value", prop_name),
		)),
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use quote::quote;

	fn parse_and_validate(input: proc_macro2::TokenStream) -> Result<TypedFormMacro> {
		let ast: FormMacro = syn::parse2(input)?;
		validate(&ast)
	}

	#[rstest::rstest]
	fn test_validate_simple_form() {
		let input = quote! {
			name: LoginForm,
			action: "/api/login",

			fields: {
				username: CharField { required, max_length: 150 },
				password: CharField { required, widget: PasswordInput },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert_eq!(typed.name.to_string(), "LoginForm");
		assert_eq!(typed.fields.len(), 2);
		assert!(matches!(typed.action, TypedFormAction::Url(_)));
	}

	#[rstest::rstest]
	fn test_validate_server_fn_action() {
		let input = quote! {
			name: VoteForm,
			server_fn: submit_vote,

			fields: {
				choice_id: IntegerField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(matches!(typed.action, TypedFormAction::ServerFn(_)));
	}

	#[rstest::rstest]
	fn test_validate_duplicate_field_names() {
		let input = quote! {
			name: TestForm,
			action: "/test",

			fields: {
				username: CharField { required },
				username: EmailField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("duplicate field name")
		);
	}

	#[rstest::rstest]
	fn test_validate_unknown_field_type() {
		let input = quote! {
			name: TestForm,
			action: "/test",

			fields: {
				unknown: UnknownField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("unknown field type")
		);
	}

	#[rstest::rstest]
	fn test_validate_validator_unknown_field() {
		let input = quote! {
			name: TestForm,
			action: "/test",

			fields: {
				username: CharField { required },
			},

			validators: {
				nonexistent: [
					|v| !v.is_empty() => "Cannot be empty",
				],
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_err());
		assert!(result.unwrap_err().to_string().contains("unknown field"));
	}

	#[rstest::rstest]
	fn test_validate_styling_properties() {
		let input = quote! {
			name: StyledForm,
			action: "/test",
			class: "my-form",

			fields: {
				username: CharField {
					required,
					class: "input-field",
					wrapper_class: "field-wrapper",
					label_class: "field-label",
					error_class: "field-error",
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert_eq!(typed.styling.class, Some("my-form".to_string()));
		assert_eq!(
			typed.fields[0].as_field().unwrap().styling.class,
			Some("input-field".to_string())
		);
	}

	#[rstest::rstest]
	fn test_validate_state_all_fields() {
		let input = quote! {
			name: StateForm,
			server_fn: submit_form,

			state: { loading, error, success },

			fields: {
				data: CharField {},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		let state = typed.state.expect("state should be Some");
		assert!(state.has_loading());
		assert!(state.has_error());
		assert!(state.has_success());
	}

	#[rstest::rstest]
	fn test_validate_state_single_field() {
		let input = quote! {
			name: LoadingForm,
			action: "/test",

			state: { loading },

			fields: {
				data: CharField {},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		let state = typed.state.expect("state should be Some");
		assert!(state.has_loading());
		assert!(!state.has_error());
		assert!(!state.has_success());
	}

	#[rstest::rstest]
	fn test_validate_state_invalid_field() {
		let input = quote! {
			name: InvalidStateForm,
			action: "/test",

			state: { loading, invalid_field },

			fields: {
				data: CharField {},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("invalid state field")
		);
	}

	#[rstest::rstest]
	fn test_validate_form_without_state() {
		let input = quote! {
			name: NoStateForm,
			action: "/test",

			fields: {
				data: CharField {},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.state.is_none());
	}

	#[rstest::rstest]
	fn test_validate_callbacks_all() {
		let input = quote! {
			name: CallbackForm,
			server_fn: submit_form,

			on_submit: |form| { /* submit handler */ },
			on_success: |result| { /* success handler */ },
			on_error: |e| { /* error handler */ },
			on_loading: |is_loading| { /* loading handler */ },

			fields: {
				data: CharField {},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.callbacks.has_any());
		assert!(typed.callbacks.has_on_submit());
		assert!(typed.callbacks.has_on_success());
		assert!(typed.callbacks.has_on_error());
		assert!(typed.callbacks.has_on_loading());
	}

	#[rstest::rstest]
	fn test_validate_callbacks_single() {
		let input = quote! {
			name: SingleCallbackForm,
			server_fn: submit_form,

			on_success: |result| {
				log::info!("Success!");
			},

			fields: {
				data: CharField {},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.callbacks.has_any());
		assert!(!typed.callbacks.has_on_submit());
		assert!(typed.callbacks.has_on_success());
		assert!(!typed.callbacks.has_on_error());
		assert!(!typed.callbacks.has_on_loading());
	}

	#[rstest::rstest]
	fn test_validate_form_without_callbacks() {
		let input = quote! {
			name: NoCallbackForm,
			action: "/test",

			fields: {
				data: CharField {},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(!typed.callbacks.has_any());
	}

	#[rstest::rstest]
	fn test_validate_callbacks_with_state() {
		let input = quote! {
			name: FullForm,
			server_fn: submit_data,

			state: { loading, error, success },

			on_success: |result| {
				navigate("/dashboard");
			},
			on_error: |e| {
				show_toast(&e.to_string());
			},

			fields: {
				username: CharField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();

		// Check state
		assert!(typed.state.is_some());
		let state = typed.state.as_ref().unwrap();
		assert!(state.has_loading());
		assert!(state.has_error());
		assert!(state.has_success());

		// Check callbacks
		assert!(typed.callbacks.has_on_success());
		assert!(typed.callbacks.has_on_error());
		assert!(!typed.callbacks.has_on_submit());
		assert!(!typed.callbacks.has_on_loading());
	}

	#[rstest::rstest]
	fn test_validate_wrapper_basic() {
		let input = quote! {
			name: WrapperForm,
			action: "/test",

			fields: {
				username: CharField {
					wrapper: div { class: "relative" },
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.fields[0].as_field().unwrap().has_wrapper());
		let wrapper = typed.fields[0]
			.as_field()
			.unwrap()
			.wrapper
			.as_ref()
			.unwrap();
		assert_eq!(wrapper.tag, "div");
		assert_eq!(wrapper.class(), Some("relative"));
	}

	#[rstest::rstest]
	fn test_validate_wrapper_multiple_attrs() {
		let input = quote! {
			name: WrapperForm,
			action: "/test",

			fields: {
				email: EmailField {
					wrapper: div {
						class: "form-field",
						id: "email-wrapper",
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		let wrapper = typed.fields[0]
			.as_field()
			.unwrap()
			.wrapper
			.as_ref()
			.unwrap();
		assert_eq!(wrapper.tag, "div");
		assert_eq!(wrapper.class(), Some("form-field"));
		assert_eq!(wrapper.id(), Some("email-wrapper"));
		assert_eq!(wrapper.attrs.len(), 2);
	}

	#[rstest::rstest]
	fn test_validate_wrapper_no_attrs() {
		let input = quote! {
			name: WrapperForm,
			action: "/test",

			fields: {
				username: CharField {
					wrapper: span,
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		let wrapper = typed.fields[0]
			.as_field()
			.unwrap()
			.wrapper
			.as_ref()
			.unwrap();
		assert_eq!(wrapper.tag, "span");
		assert!(!wrapper.has_attrs());
	}

	#[rstest::rstest]
	fn test_validate_field_without_wrapper() {
		let input = quote! {
			name: NoWrapperForm,
			action: "/test",

			fields: {
				username: CharField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(!typed.fields[0].as_field().unwrap().has_wrapper());
	}

	// =========================================================================
	// Icon Tests
	// =========================================================================

	#[rstest::rstest]
	fn test_validate_basic_icon() {
		let input = quote! {
			name: IconForm,
			action: "/test",

			fields: {
				username: CharField {
					icon: svg {
						class: "w-5 h-5",
						viewBox: "0 0 24 24",
						path { d: "M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4" }
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.fields[0].as_field().unwrap().has_icon());
		let icon = typed.fields[0].as_field().unwrap().icon.as_ref().unwrap();
		assert_eq!(icon.attrs.len(), 2); // class, viewBox
		assert_eq!(icon.children.len(), 1); // path
		assert_eq!(icon.position, TypedIconPosition::Left); // default
	}

	#[rstest::rstest]
	fn test_validate_icon_with_position() {
		let input = quote! {
			name: IconPositionForm,
			action: "/test",

			fields: {
				email: EmailField {
					icon: svg {
						viewBox: "0 0 24 24",
						path { d: "M0 0h24v24H0z" }
					},
					icon_position: "right",
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.fields[0].as_field().unwrap().has_icon());
		assert_eq!(
			typed.fields[0].as_field().unwrap().icon_position(),
			Some(TypedIconPosition::Right)
		);
	}

	#[rstest::rstest]
	fn test_validate_icon_position_label() {
		let input = quote! {
			name: IconLabelForm,
			action: "/test",

			fields: {
				search: CharField {
					icon: svg {
						viewBox: "0 0 24 24",
						circle { cx: "11", cy: "11", r: "8" }
					},
					icon_position: "label",
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert_eq!(
			typed.fields[0].as_field().unwrap().icon_position(),
			Some(TypedIconPosition::Label)
		);
	}

	#[rstest::rstest]
	fn test_validate_icon_with_nested_group() {
		let input = quote! {
			name: NestedIconForm,
			action: "/test",

			fields: {
				password: CharField {
					icon: svg {
						viewBox: "0 0 24 24",
						g {
							fill: "none",
							stroke: "currentColor",
							path { d: "M12 15v2m0 0v2m0-2h2" }
							circle { cx: "12", cy: "12", r: "10" }
						}
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.fields[0].as_field().unwrap().has_icon());
		let icon = typed.fields[0].as_field().unwrap().icon.as_ref().unwrap();
		assert_eq!(icon.children.len(), 1); // g element

		// Check nested group
		let g_child = &icon.children[0];
		assert_eq!(g_child.tag, "g");
		assert_eq!(g_child.children.len(), 2); // path, circle
	}

	#[rstest::rstest]
	fn test_validate_field_without_icon() {
		let input = quote! {
			name: NoIconForm,
			action: "/test",

			fields: {
				username: CharField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(!typed.fields[0].as_field().unwrap().has_icon());
		assert_eq!(typed.fields[0].as_field().unwrap().icon_position(), None); // No icon, no position
	}

	#[rstest::rstest]
	fn test_validate_icon_multiple_children() {
		let input = quote! {
			name: MultiChildIconForm,
			action: "/test",

			fields: {
				status: CharField {
					icon: svg {
						viewBox: "0 0 24 24",
						fill: "none",
						stroke: "currentColor",
						path { d: "M5 13l4 4L19 7" }
						path { d: "M12 22c5.523 0 10-4.477 10-10" }
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		let icon = typed.fields[0].as_field().unwrap().icon.as_ref().unwrap();
		assert_eq!(icon.attrs.len(), 3); // viewBox, fill, stroke
		assert_eq!(icon.children.len(), 2); // two paths
	}

	// =========================================================================
	// Custom Attrs Tests
	// =========================================================================

	#[rstest::rstest]
	fn test_validate_custom_attrs_aria() {
		let input = quote! {
			name: AriaForm,
			action: "/test",

			fields: {
				email: EmailField {
					attrs: {
						aria_label: "Email address",
						aria_required: "true",
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.fields[0].as_field().unwrap().has_custom_attrs());
		assert_eq!(typed.fields[0].as_field().unwrap().custom_attrs.len(), 2);

		let aria_attrs: Vec<_> = typed.fields[0].as_field().unwrap().aria_attrs().collect();
		assert_eq!(aria_attrs.len(), 2);
		assert_eq!(aria_attrs[0].name, "aria_label");
		assert_eq!(aria_attrs[0].value, "Email address");
		assert_eq!(aria_attrs[0].html_name(), "aria-label");
	}

	#[rstest::rstest]
	fn test_validate_custom_attrs_data() {
		let input = quote! {
			name: DataForm,
			action: "/test",

			fields: {
				username: CharField {
					attrs: {
						data_testid: "username-input",
						data_analytics: "signup-username",
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		let data_attrs: Vec<_> = typed.fields[0].as_field().unwrap().data_attrs().collect();
		assert_eq!(data_attrs.len(), 2);
		assert_eq!(data_attrs[0].html_name(), "data-testid");
	}

	#[rstest::rstest]
	fn test_validate_custom_attrs_mixed() {
		let input = quote! {
			name: MixedAttrsForm,
			action: "/test",

			fields: {
				password: CharField {
					widget: PasswordInput,
					attrs: {
						aria_label: "Password",
						data_testid: "password-field",
						aria_describedby: "password-help",
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert_eq!(typed.fields[0].as_field().unwrap().custom_attrs.len(), 3);
		assert_eq!(typed.fields[0].as_field().unwrap().aria_attrs().count(), 2);
		assert_eq!(typed.fields[0].as_field().unwrap().data_attrs().count(), 1);
	}

	#[rstest::rstest]
	fn test_validate_custom_attrs_invalid_prefix() {
		let input = quote! {
			name: InvalidAttrsForm,
			action: "/test",

			fields: {
				email: EmailField {
					attrs: {
						invalid_attr: "value",
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("invalid custom attribute"));
		assert!(err.contains("must start with 'aria_' or 'data_'"));
	}

	#[rstest::rstest]
	fn test_validate_field_without_custom_attrs() {
		let input = quote! {
			name: NoAttrsForm,
			action: "/test",

			fields: {
				username: CharField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(!typed.fields[0].as_field().unwrap().has_custom_attrs());
		assert_eq!(typed.fields[0].as_field().unwrap().custom_attrs.len(), 0);
	}

	#[rstest::rstest]
	fn test_validate_custom_attrs_with_other_properties() {
		let input = quote! {
			name: CombinedForm,
			action: "/test",

			fields: {
				search: CharField {
					required,
					label: "Search",
					placeholder: "Enter search term",
					class: "search-input",
					attrs: {
						aria_label: "Search field",
						data_cy: "search-input",
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.fields[0].as_field().unwrap().validation.required);
		assert_eq!(
			typed.fields[0].as_field().unwrap().display.label,
			Some("Search".to_string())
		);
		assert_eq!(
			typed.fields[0].as_field().unwrap().styling.class,
			Some("search-input".to_string())
		);
		assert_eq!(typed.fields[0].as_field().unwrap().custom_attrs.len(), 2);
	}

	#[rstest::rstest]
	fn test_validate_bind_true() {
		let input = quote! {
			name: BindTrueForm,
			action: "/test",

			fields: {
				username: CharField {
					bind: true,
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.fields[0].as_field().unwrap().is_bind_enabled());
		assert!(typed.fields[0].as_field().unwrap().bind);
	}

	#[rstest::rstest]
	fn test_validate_bind_false() {
		let input = quote! {
			name: BindFalseForm,
			action: "/test",

			fields: {
				password: CharField {
					widget: PasswordInput,
					bind: false,
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(!typed.fields[0].as_field().unwrap().is_bind_enabled());
		assert!(!typed.fields[0].as_field().unwrap().bind);
	}

	#[rstest::rstest]
	fn test_validate_bind_default() {
		let input = quote! {
			name: BindDefaultForm,
			action: "/test",

			fields: {
				email: EmailField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		// Default should be true (enabled)
		assert!(typed.fields[0].as_field().unwrap().is_bind_enabled());
		assert!(typed.fields[0].as_field().unwrap().bind);
	}

	#[rstest::rstest]
	fn test_validate_bind_with_other_properties() {
		let input = quote! {
			name: BindCombinedForm,
			action: "/test",

			fields: {
				search: CharField {
					required,
					label: "Search",
					placeholder: "Enter search term",
					bind: false,
					class: "search-input",
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		// Check bind
		assert!(!typed.fields[0].as_field().unwrap().is_bind_enabled());
		// Check other properties
		assert!(typed.fields[0].as_field().unwrap().validation.required);
		assert_eq!(
			typed.fields[0].as_field().unwrap().display.label,
			Some("Search".to_string())
		);
		assert_eq!(
			typed.fields[0].as_field().unwrap().display.placeholder,
			Some("Enter search term".to_string())
		);
		assert_eq!(
			typed.fields[0].as_field().unwrap().styling.class,
			Some("search-input".to_string())
		);
	}

	// =========================================================================
	// Initial Loader Tests
	// =========================================================================

	#[rstest::rstest]
	fn test_validate_initial_loader_basic() {
		let input = quote! {
			name: ProfileEditForm,
			server_fn: update_profile,
			initial_loader: get_profile_data,

			fields: {
				username: CharField { required },
				email: EmailField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.initial_loader.is_some());
		let loader = typed.initial_loader.as_ref().unwrap();
		// Check that the path contains the expected identifier
		assert!(loader.segments.len() > 0);
		assert_eq!(
			loader.segments.last().unwrap().ident.to_string(),
			"get_profile_data"
		);
	}

	#[rstest::rstest]
	fn test_validate_initial_loader_with_path() {
		let input = quote! {
			name: SettingsForm,
			server_fn: save_settings,
			initial_loader: api::settings::get_settings,

			fields: {
				theme: CharField {},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.initial_loader.is_some());
		let loader = typed.initial_loader.as_ref().unwrap();
		assert_eq!(loader.segments.len(), 3);
		assert_eq!(
			loader.segments.last().unwrap().ident.to_string(),
			"get_settings"
		);
	}

	#[rstest::rstest]
	fn test_validate_form_without_initial_loader() {
		let input = quote! {
			name: SimpleForm,
			action: "/test",

			fields: {
				data: CharField {},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.initial_loader.is_none());
	}

	#[rstest::rstest]
	fn test_validate_initial_loader_with_callbacks() {
		let input = quote! {
			name: LoaderCallbackForm,
			server_fn: update_data,
			initial_loader: fetch_data,

			on_success: |result| { /* handle success */ },

			fields: {
				name: CharField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.initial_loader.is_some());
		assert!(typed.callbacks.has_on_success());
	}

	// =========================================================================
	// Initial From Tests (Field Property)
	// =========================================================================

	#[rstest::rstest]
	fn test_validate_initial_from_basic() {
		let input = quote! {
			name: EditForm,
			server_fn: update_item,
			initial_loader: get_item,

			fields: {
				title: CharField {
					required,
					initial_from: "title",
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.fields[0].as_field().unwrap().initial_from.is_some());
		assert_eq!(
			typed.fields[0]
				.as_field()
				.unwrap()
				.initial_from
				.as_ref()
				.unwrap(),
			"title"
		);
	}

	#[rstest::rstest]
	fn test_validate_initial_from_multiple_fields() {
		let input = quote! {
			name: UserEditForm,
			server_fn: update_user,
			initial_loader: get_user,

			fields: {
				username: CharField {
					required,
					initial_from: "username",
				},
				email: EmailField {
					required,
					initial_from: "email_address",
				},
				bio: TextField {
					initial_from: "biography",
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert_eq!(
			typed.fields[0].as_field().unwrap().initial_from,
			Some("username".to_string())
		);
		assert_eq!(
			typed.fields[1].as_field().unwrap().initial_from,
			Some("email_address".to_string())
		);
		assert_eq!(
			typed.fields[2].as_field().unwrap().initial_from,
			Some("biography".to_string())
		);
	}

	#[rstest::rstest]
	fn test_validate_initial_from_partial() {
		let input = quote! {
			name: PartialInitForm,
			server_fn: submit_form,
			initial_loader: get_partial_data,

			fields: {
				name: CharField {
					initial_from: "name",
				},
				description: TextField {},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.fields[0].as_field().unwrap().initial_from.is_some());
		assert!(typed.fields[1].as_field().unwrap().initial_from.is_none());
	}

	#[rstest::rstest]
	fn test_validate_field_without_initial_from() {
		let input = quote! {
			name: NoInitialForm,
			action: "/test",

			fields: {
				data: CharField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.fields[0].as_field().unwrap().initial_from.is_none());
	}

	#[rstest::rstest]
	fn test_validate_initial_from_with_other_properties() {
		let input = quote! {
			name: CombinedInitForm,
			server_fn: save_data,
			initial_loader: load_data,

			fields: {
				search: CharField {
					required,
					label: "Search Term",
					placeholder: "Enter value",
					initial_from: "search_term",
					class: "search-input",
					bind: true,
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.fields[0].as_field().unwrap().validation.required);
		assert_eq!(
			typed.fields[0].as_field().unwrap().display.label,
			Some("Search Term".to_string())
		);
		assert_eq!(
			typed.fields[0].as_field().unwrap().initial_from,
			Some("search_term".to_string())
		);
		assert!(typed.fields[0].as_field().unwrap().bind);
	}

	// =========================================================================
	// Slots Tests
	// =========================================================================

	#[rstest::rstest]
	fn test_validate_slots_before_fields() {
		let input = quote! {
			name: SlotsBeforeForm,
			action: "/test",

			slots: {
				before_fields: || {
					view! { <div class="form-header">"Welcome"</div> }
				},
			},

			fields: {
				data: CharField {},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.slots.is_some());
		let slots = typed.slots.as_ref().unwrap();
		assert!(slots.before_fields.is_some());
		assert!(slots.after_fields.is_none());
	}

	#[rstest::rstest]
	fn test_validate_slots_after_fields() {
		let input = quote! {
			name: SlotsAfterForm,
			action: "/test",

			slots: {
				after_fields: || {
					view! { <button type="submit">"Submit"</button> }
				},
			},

			fields: {
				data: CharField {},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.slots.is_some());
		let slots = typed.slots.as_ref().unwrap();
		assert!(slots.before_fields.is_none());
		assert!(slots.after_fields.is_some());
	}

	#[rstest::rstest]
	fn test_validate_slots_both() {
		let input = quote! {
			name: SlotsBothForm,
			action: "/test",

			slots: {
				before_fields: || {
					view! { <div class="header">"Form Header"</div> }
				},
				after_fields: || {
					view! { <div class="footer">"Form Footer"</div> }
				},
			},

			fields: {
				data: CharField {},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.slots.is_some());
		let slots = typed.slots.as_ref().unwrap();
		assert!(slots.before_fields.is_some());
		assert!(slots.after_fields.is_some());
	}

	#[rstest::rstest]
	fn test_validate_form_without_slots() {
		let input = quote! {
			name: NoSlotsForm,
			action: "/test",

			fields: {
				data: CharField {},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.slots.is_none());
	}

	#[rstest::rstest]
	fn test_validate_slots_with_state_and_callbacks() {
		let input = quote! {
			name: FullFeaturedForm,
			server_fn: submit_data,

			state: { loading, error },

			on_success: |result| { /* handle */ },

			slots: {
				before_fields: || {
					view! { <h2>"Enter Information"</h2> }
				},
				after_fields: || {
					view! { <button type="submit">"Save"</button> }
				},
			},

			fields: {
				name: CharField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		// Check state
		assert!(typed.state.is_some());
		let state = typed.state.as_ref().unwrap();
		assert!(state.has_loading());
		assert!(state.has_error());

		// Check callbacks
		assert!(typed.callbacks.has_on_success());

		// Check slots
		assert!(typed.slots.is_some());
		let slots = typed.slots.as_ref().unwrap();
		assert!(slots.before_fields.is_some());
		assert!(slots.after_fields.is_some());
	}

	#[rstest::rstest]
	fn test_validate_full_step9_features() {
		let input = quote! {
			name: CompleteStep9Form,
			server_fn: update_profile,
			initial_loader: get_profile,

			state: { loading, error, success },

			on_success: |result| {
				navigate("/profile");
			},

			slots: {
				before_fields: || {
					view! { <div class="form-intro">"Edit your profile"</div> }
				},
				after_fields: || {
					view! {
						<div class="button-group">
							<button type="submit">"Save"</button>
						</div>
					}
				},
			},

			fields: {
				username: CharField {
					required,
					label: "Username",
					initial_from: "username",
				},
				email: EmailField {
					required,
					label: "Email",
					initial_from: "email",
				},
				bio: TextField {
					label: "Biography",
					initial_from: "bio",
					placeholder: "Tell us about yourself",
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();

		// Check initial_loader
		assert!(typed.initial_loader.is_some());

		// Check state
		assert!(typed.state.is_some());

		// Check callbacks
		assert!(typed.callbacks.has_on_success());

		// Check slots
		assert!(typed.slots.is_some());

		// Check fields with initial_from
		assert_eq!(typed.fields.len(), 3);
		assert_eq!(
			typed.fields[0].as_field().unwrap().initial_from,
			Some("username".to_string())
		);
		assert_eq!(
			typed.fields[1].as_field().unwrap().initial_from,
			Some("email".to_string())
		);
		assert_eq!(
			typed.fields[2].as_field().unwrap().initial_from,
			Some("bio".to_string())
		);
	}

	// =========================================================================
	// Field Group Tests
	// =========================================================================

	#[rstest::rstest]
	fn test_validate_field_group_basic() {
		let input = quote! {
			name: AddressForm,
			action: "/test",

			fields: {
				address_group: FieldGroup {
					label: "Address",
					fields: {
						street: CharField { required },
						city: CharField { required },
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert_eq!(typed.fields.len(), 1);

		// Verify it's a group
		let group = typed.fields[0].as_group().unwrap();
		assert_eq!(group.name.to_string(), "address_group");
		assert_eq!(group.label, Some("Address".to_string()));
		assert_eq!(group.fields.len(), 2);

		// Check fields within the group
		assert_eq!(group.fields[0].name.to_string(), "street");
		assert!(group.fields[0].validation.required);
		assert_eq!(group.fields[1].name.to_string(), "city");
	}

	#[rstest::rstest]
	fn test_validate_field_group_with_class() {
		let input = quote! {
			name: StyledGroupForm,
			action: "/test",

			fields: {
				info_group: FieldGroup {
					label: "Personal Information",
					class: "form-section",
					fields: {
						name: CharField {},
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		let group = typed.fields[0].as_group().unwrap();
		assert_eq!(group.class, Some("form-section".to_string()));
	}

	#[rstest::rstest]
	fn test_validate_field_group_without_label() {
		let input = quote! {
			name: NoLabelGroupForm,
			action: "/test",

			fields: {
				hidden_group: FieldGroup {
					class: "hidden-section",
					fields: {
						data: CharField {},
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		let group = typed.fields[0].as_group().unwrap();
		assert!(group.label.is_none());
		assert_eq!(group.class, Some("hidden-section".to_string()));
	}

	#[rstest::rstest]
	fn test_validate_field_group_mixed_with_fields() {
		let input = quote! {
			name: MixedForm,
			action: "/test",

			fields: {
				email: EmailField { required },
				address_group: FieldGroup {
					label: "Address",
					fields: {
						street: CharField {},
						zip: CharField {},
					},
				},
				notes: TextField {},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert_eq!(typed.fields.len(), 3);

		// First is a field
		assert!(typed.fields[0].as_field().is_some());
		assert_eq!(
			typed.fields[0].as_field().unwrap().name.to_string(),
			"email"
		);

		// Second is a group
		assert!(typed.fields[1].as_group().is_some());
		let group = typed.fields[1].as_group().unwrap();
		assert_eq!(group.name.to_string(), "address_group");
		assert_eq!(group.fields.len(), 2);

		// Third is a field
		assert!(typed.fields[2].as_field().is_some());
		assert_eq!(
			typed.fields[2].as_field().unwrap().name.to_string(),
			"notes"
		);
	}

	#[rstest::rstest]
	fn test_validate_field_group_duplicate_field_names() {
		let input = quote! {
			name: DuplicateForm,
			action: "/test",

			fields: {
				email: EmailField {},
				info_group: FieldGroup {
					label: "Info",
					fields: {
						email: CharField {},
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("duplicate field name")
		);
	}

	#[rstest::rstest]
	fn test_validate_field_group_duplicate_group_names() {
		let input = quote! {
			name: DuplicateGroupForm,
			action: "/test",

			fields: {
				my_group: FieldGroup {
					label: "First",
					fields: {
						field1: CharField {},
					},
				},
				my_group: FieldGroup {
					label: "Second",
					fields: {
						field2: CharField {},
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("duplicate field/group name")
		);
	}

	#[rstest::rstest]
	fn test_validate_field_group_with_validators() {
		let input = quote! {
			name: ValidatedGroupForm,
			action: "/test",

			fields: {
				address_group: FieldGroup {
					label: "Address",
					fields: {
						street: CharField { required },
						zip: CharField { max_length: 10 },
					},
				},
			},

			validators: {
				street: [|v| !v.is_empty() => "Street is required"],
				zip: [|v| v.len() <= 10 => "Zip too long"],
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert_eq!(typed.validators.len(), 2);
	}

	#[rstest::rstest]
	fn test_validate_field_group_validator_unknown_field() {
		let input = quote! {
			name: InvalidValidatorForm,
			action: "/test",

			fields: {
				address_group: FieldGroup {
					label: "Address",
					fields: {
						street: CharField {},
					},
				},
			},

			validators: {
				nonexistent: [|v| !v.is_empty() => "Required"],
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_err());
		assert!(result.unwrap_err().to_string().contains("unknown field"));
	}

	#[rstest::rstest]
	fn test_validate_field_group_with_initial_from() {
		let input = quote! {
			name: InitialGroupForm,
			server_fn: update_data,
			initial_loader: get_data,

			fields: {
				profile_group: FieldGroup {
					label: "Profile",
					fields: {
						name: CharField { initial_from: "name" },
						bio: TextField { initial_from: "biography" },
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		let group = typed.fields[0].as_group().unwrap();
		assert_eq!(group.fields[0].initial_from, Some("name".to_string()));
		assert_eq!(group.fields[1].initial_from, Some("biography".to_string()));
	}

	#[rstest::rstest]
	fn test_validate_multiple_field_groups() {
		let input = quote! {
			name: MultiGroupForm,
			action: "/test",

			fields: {
				personal_group: FieldGroup {
					label: "Personal",
					class: "section-personal",
					fields: {
						name: CharField { required },
						age: IntegerField {},
					},
				},
				address_group: FieldGroup {
					label: "Address",
					class: "section-address",
					fields: {
						street: CharField {},
						city: CharField {},
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert_eq!(typed.fields.len(), 2);

		let group1 = typed.fields[0].as_group().unwrap();
		assert_eq!(group1.name.to_string(), "personal_group");
		assert_eq!(group1.label, Some("Personal".to_string()));
		assert_eq!(group1.fields.len(), 2);

		let group2 = typed.fields[1].as_group().unwrap();
		assert_eq!(group2.name.to_string(), "address_group");
		assert_eq!(group2.label, Some("Address".to_string()));
		assert_eq!(group2.fields.len(), 2);
	}

	#[rstest::rstest]
	fn test_validate_field_group_with_field_properties() {
		let input = quote! {
			name: PropertiesGroupForm,
			action: "/test",

			fields: {
				styled_group: FieldGroup {
					label: "Styled Fields",
					fields: {
						email: EmailField {
							required,
							label: "Email Address",
							placeholder: "you@example.com",
							class: "email-input",
							wrapper: div { class: "email-wrapper" },
						},
						password: CharField {
							required,
							widget: PasswordInput,
							bind: false,
						},
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		let group = typed.fields[0].as_group().unwrap();

		// Check email field properties
		let email = &group.fields[0];
		assert!(email.validation.required);
		assert_eq!(email.display.label, Some("Email Address".to_string()));
		assert_eq!(
			email.display.placeholder,
			Some("you@example.com".to_string())
		);
		assert!(email.has_wrapper());

		// Check password field properties
		let password = &group.fields[1];
		assert!(password.validation.required);
		assert!(matches!(password.widget, TypedWidget::PasswordInput));
		assert!(!password.bind);
	}

	// =========================================================================
	// Derived Block Tests
	// =========================================================================

	#[rstest::rstest]
	fn test_validate_derived_basic() {
		let input = quote! {
			name: TweetForm,
			server_fn: create_tweet,

			derived: {
				char_count: |form| form.content().get().len(),
			},

			fields: {
				content: CharField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.derived.is_some());
		let derived = typed.derived.unwrap();
		assert_eq!(derived.items.len(), 1);
		assert_eq!(derived.items[0].name.to_string(), "char_count");
	}

	#[rstest::rstest]
	fn test_validate_derived_multiple_items() {
		let input = quote! {
			name: PriceForm,
			server_fn: calculate,

			derived: {
				subtotal: |form| form.quantity().get() * form.price().get(),
				tax: |form| form.subtotal().get() * 0.1,
				total: |form| form.subtotal().get() + form.tax().get(),
			},

			fields: {
				quantity: IntegerField { required },
				price: DecimalField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.derived.is_some());
		let derived = typed.derived.unwrap();
		assert_eq!(derived.items.len(), 3);
		assert_eq!(derived.items[0].name.to_string(), "subtotal");
		assert_eq!(derived.items[1].name.to_string(), "tax");
		assert_eq!(derived.items[2].name.to_string(), "total");
	}

	#[rstest::rstest]
	fn test_validate_derived_duplicate_name() {
		let input = quote! {
			name: DuplicateForm,
			server_fn: submit,

			derived: {
				value: |form| form.x().get() + form.y().get(),
				value: |form| form.x().get() * form.y().get(),
			},

			fields: {
				x: IntegerField { required },
				y: IntegerField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("duplicate derived item name"));
		assert!(err.contains("value"));
	}

	#[rstest::rstest]
	fn test_validate_derived_empty() {
		let input = quote! {
			name: EmptyDerivedForm,
			server_fn: submit,

			derived: {},

			fields: {
				data: CharField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.derived.is_some());
		let derived = typed.derived.unwrap();
		assert!(derived.items.is_empty());
	}

	#[rstest::rstest]
	fn test_validate_no_derived_block() {
		let input = quote! {
			name: NoDerivedForm,
			server_fn: submit,

			fields: {
				data: CharField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.derived.is_none());
	}

	#[rstest::rstest]
	fn test_validate_derived_with_watch_and_state() {
		let input = quote! {
			name: CompleteForm,
			server_fn: create_tweet,

			state: { loading, error },

			derived: {
				char_count: |form| form.content().get().len(),
				is_valid: |form| form.char_count().get() <= 280,
			},

			watch: {
				counter: |form| {
					format!("{}/280", form.char_count().get())
				},
			},

			fields: {
				content: CharField { required, bind: true },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();

		// Check state
		assert!(typed.state.is_some());
		let state = typed.state.as_ref().unwrap();
		assert!(state.has_loading());
		assert!(state.has_error());

		// Check derived
		assert!(typed.derived.is_some());
		let derived = typed.derived.as_ref().unwrap();
		assert_eq!(derived.items.len(), 2);

		// Check watch
		assert!(typed.watch.is_some());
		let watch = typed.watch.as_ref().unwrap();
		assert_eq!(watch.items.len(), 1);
	}

	// =========================================================
	// Dynamic ChoiceField validation tests
	// =========================================================

	#[rstest::rstest]
	fn test_validate_choices_loader_basic() {
		let input = quote! {
			name: VotingForm,
			server_fn: submit_vote,
			choices_loader: get_poll_choices,

			fields: {
				choice: ChoiceField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.choices_loader.is_some());
	}

	#[rstest::rstest]
	fn test_validate_choices_config_basic() {
		let input = quote! {
			name: VotingForm,
			server_fn: submit_vote,
			choices_loader: get_poll_data,

			fields: {
				choice: ChoiceField {
					required,
					choices_from: "poll_options",
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		let field = typed.fields[0].as_field().unwrap();

		assert!(field.choices_config.is_some());
		let config = field.choices_config.as_ref().unwrap();
		assert_eq!(config.choices_from, "poll_options");
		// Default values for choice_value and choice_label
		assert_eq!(config.choice_value, "value");
		assert_eq!(config.choice_label, "label");
	}

	#[rstest::rstest]
	fn test_validate_choices_config_all_properties() {
		let input = quote! {
			name: VotingForm,
			server_fn: submit_vote,
			choices_loader: get_poll_data,

			fields: {
				choice: ChoiceField {
					required,
					choices_from: "choices",
					choice_value: "id",
					choice_label: "choice_text",
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		let field = typed.fields[0].as_field().unwrap();

		assert!(field.choices_config.is_some());
		let config = field.choices_config.as_ref().unwrap();
		assert_eq!(config.choices_from, "choices");
		assert_eq!(config.choice_value, "id");
		assert_eq!(config.choice_label, "choice_text");
	}

	#[rstest::rstest]
	fn test_validate_multiple_dynamic_choice_fields() {
		let input = quote! {
			name: FilterForm,
			server_fn: apply_filter,
			choices_loader: get_filter_options,

			fields: {
				category: ChoiceField {
					choices_from: "categories",
					choice_value: "id",
					choice_label: "name",
				},
				status: ChoiceField {
					choices_from: "statuses",
					choice_value: "code",
					choice_label: "description",
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert_eq!(typed.fields.len(), 2);

		// Check first field
		let category_field = typed.fields[0].as_field().unwrap();
		assert!(category_field.choices_config.is_some());
		let cat_config = category_field.choices_config.as_ref().unwrap();
		assert_eq!(cat_config.choices_from, "categories");
		assert_eq!(cat_config.choice_value, "id");
		assert_eq!(cat_config.choice_label, "name");

		// Check second field
		let status_field = typed.fields[1].as_field().unwrap();
		assert!(status_field.choices_config.is_some());
		let status_config = status_field.choices_config.as_ref().unwrap();
		assert_eq!(status_config.choices_from, "statuses");
		assert_eq!(status_config.choice_value, "code");
		assert_eq!(status_config.choice_label, "description");
	}

	#[rstest::rstest]
	fn test_validate_field_without_choices_config() {
		let input = quote! {
			name: SimpleForm,
			action: "/test",

			fields: {
				category: ChoiceField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		let field = typed.fields[0].as_field().unwrap();
		assert!(field.choices_config.is_none());
	}

	#[rstest::rstest]
	fn test_validate_choices_config_with_other_properties() {
		let input = quote! {
			name: CombinedForm,
			server_fn: save_data,
			choices_loader: load_options,

			fields: {
				priority: ChoiceField {
					required,
					label: "Priority Level",
					widget: RadioSelect,
					choices_from: "priorities",
					choice_value: "id",
					choice_label: "name",
					class: "priority-select",
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		let field = typed.fields[0].as_field().unwrap();

		assert!(field.validation.required);
		assert_eq!(field.display.label.as_ref().unwrap(), "Priority Level");
		assert!(field.choices_config.is_some());

		let config = field.choices_config.as_ref().unwrap();
		assert_eq!(config.choices_from, "priorities");
		assert_eq!(config.choice_value, "id");
		assert_eq!(config.choice_label, "name");
	}

	#[rstest::rstest]
	fn test_validate_choices_loader_with_initial_loader() {
		let input = quote! {
			name: EditPollForm,
			server_fn: update_poll,
			initial_loader: get_poll_edit_data,
			choices_loader: get_choice_options,

			fields: {
				title: CharField {
					initial_from: "poll_title",
				},
				selected_choice: ChoiceField {
					choices_from: "available_choices",
					choice_value: "id",
					choice_label: "text",
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();

		// Both loaders should be present
		assert!(typed.initial_loader.is_some());
		assert!(typed.choices_loader.is_some());

		// Verify initial_from on first field
		let title_field = typed.fields[0].as_field().unwrap();
		assert_eq!(title_field.initial_from, Some("poll_title".to_string()));

		// Verify choices_config on second field
		let choice_field = typed.fields[1].as_field().unwrap();
		assert!(choice_field.choices_config.is_some());
	}

	#[rstest::rstest]
	fn test_validate_choices_config_in_field_group() {
		let input = quote! {
			name: GroupedChoiceForm,
			server_fn: submit_grouped,
			choices_loader: get_group_options,

			fields: {
				filter_options: FieldGroup {
					label: "Filter Options",
					fields: {
						category: ChoiceField {
							choices_from: "categories",
							choice_value: "id",
							choice_label: "name",
						},
						status: ChoiceField {
							choices_from: "statuses",
							choice_value: "code",
							choice_label: "label",
						},
					},
				},
			},
		};

		let result = parse_and_validate(input);
		assert!(
			result.is_ok(),
			"Group validation failed: {:?}",
			result.unwrap_err()
		);

		let typed = result.unwrap();
		let group = typed.fields[0].as_group().unwrap();

		// Check fields within the group
		assert_eq!(group.fields.len(), 2);

		let category_field = &group.fields[0];
		assert!(category_field.choices_config.is_some());
		assert_eq!(
			category_field.choices_config.as_ref().unwrap().choices_from,
			"categories"
		);

		let status_field = &group.fields[1];
		assert!(status_field.choices_config.is_some());
		assert_eq!(
			status_field.choices_config.as_ref().unwrap().choices_from,
			"statuses"
		);
	}

	#[test]
	fn test_transform_redirect_rejects_http_url() {
		let lit = syn::LitStr::new("http://example.com/success", proc_macro2::Span::call_site());
		let result = transform_redirect(&Some(lit));
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("insecure HTTP"));
	}

	#[test]
	fn test_transform_redirect_allows_https_url() {
		let lit = syn::LitStr::new(
			"https://example.com/success",
			proc_macro2::Span::call_site(),
		);
		let result = transform_redirect(&Some(lit));
		assert!(result.is_ok());
		assert_eq!(
			result.unwrap(),
			Some("https://example.com/success".to_string())
		);
	}

	#[test]
	fn test_transform_redirect_allows_relative_path() {
		let lit = syn::LitStr::new("/dashboard", proc_macro2::Span::call_site());
		let result = transform_redirect(&Some(lit));
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), Some("/dashboard".to_string()));
	}

	#[test]
	fn test_transform_redirect_rejects_empty() {
		let lit = syn::LitStr::new("", proc_macro2::Span::call_site());
		let result = transform_redirect(&Some(lit));
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("cannot be empty"));
	}

	#[test]
	fn test_transform_redirect_rejects_invalid_scheme() {
		let lit = syn::LitStr::new("ftp://example.com/file", proc_macro2::Span::call_site());
		let result = transform_redirect(&Some(lit));
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("must start with '/'"));
	}

	#[test]
	fn test_transform_redirect_none() {
		let result = transform_redirect(&None);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), None);
	}
}
