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
	AmbientArgumentsSource, FormAction, FormCallbacks, FormChoiceItem, FormControlEntryDef,
	FormControlEntryKind, FormCustomWidgetSpec, FormDatalistDef, FormDerived, FormFieldCollection,
	FormFieldDef, FormFieldEntry, FormFieldGroup, FormFieldProperty, FormMacro, FormMethod,
	FormSlots, FormState, FormSubmitButtonDef, FormValidator, FormWatch, FormWidgetSpec,
	IconPosition, ModelFieldSelection, ModelFormSource, StripArgument, TypedButtonControlDef,
	TypedButtonKind, TypedChoiceGroup, TypedChoiceItem, TypedChoiceOption, TypedChoicesConfig,
	TypedCustomAttr, TypedCustomWidget, TypedDatalistDef, TypedDerivedItem, TypedFieldDisplay,
	TypedFieldNativeAttrs, TypedFieldStyling, TypedFieldType, TypedFieldValidation,
	TypedFormAction, TypedFormCallbacks, TypedFormDerived, TypedFormFieldCollection,
	TypedFormFieldDef, TypedFormFieldEntry, TypedFormFieldGroup, TypedFormMacro, TypedFormSlots,
	TypedFormState, TypedFormStyling, TypedFormValidator, TypedFormWatch, TypedFormWatchItem,
	TypedIcon, TypedIconAttr, TypedIconChild, TypedIconPosition, TypedImageInputDef, TypedMeterDef,
	TypedModelFieldOverride, TypedModelFieldSelection, TypedModelFormSource, TypedOutputDef,
	TypedProgressDef, TypedStripArgument, TypedSubmitButtonDef, TypedValidatorRule, TypedWidget,
	TypedWrapper, TypedWrapperAttr, ValidatorRule,
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
pub(super) fn validate(
	ast: &FormMacro,
	ambient_arguments_source: Option<AmbientArgumentsSource>,
) -> Result<TypedFormMacro> {
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

	let success_url = ast.success_url.clone();

	// Reject simultaneous `redirect_on_success` + `success_url`: both
	// generate `navigate()` calls in the same `submit()` body, which
	// would push two history entries and fire two observer cycles for a
	// single successful submit (Refs #4610 codegen review). Force the
	// user to pick one — `redirect_on_success` for a static URL, or
	// `success_url` for a dynamically-computed one.
	if let (Some(_), Some(url_expr)) = (ast.redirect_on_success.as_ref(), success_url.as_ref()) {
		return Err(Error::new_spanned(
			url_expr,
			"form! cannot specify both `redirect_on_success` and `success_url`: each generates an \
			 SPA navigation on successful submit, so combining them would push two history \
			 entries and fire two observer cycles. Use `redirect_on_success` for a static URL or \
			 `success_url` for a dynamically-computed one, not both.",
		));
	}

	// Transform initial_loader (pass through the Path)
	let initial_loader = ast.initial_loader.clone();

	// Transform choices_loader (pass through the Path)
	let choices_loader = ast.choices_loader.clone();

	// Transform slots
	let slots = transform_slots(&ast.slots)?;

	// Transform fields
	let fields = transform_fields(&ast.fields)?;
	validate_list_references(&fields)?;
	let model_source = transform_model_source(&ast.model_source)?;

	if model_source.is_some() && !matches!(action, TypedFormAction::ServerFn(_)) {
		return Err(Error::new(
			ast.span,
			"model-backed form! requires an explicit `server_fn`",
		));
	}
	if model_source.is_some() && (redirect_on_success.is_some() || success_url.is_some()) {
		return Err(Error::new(
			ast.span,
			"model-backed form! does not support `redirect_on_success` or `success_url`; configure submission lifecycle through `use_form(&form)`",
		));
	}
	if model_source.is_some() && initial_loader.is_some() {
		return Err(Error::new(
			ast.span,
			"model-backed form! does not support `initial_loader`; initialize values through the generated form state",
		));
	}
	if model_source.is_some() && choices_loader.is_some() {
		return Err(Error::new(
			ast.span,
			"model-backed form! does not support `choices_loader`; configure static choices through the generated model schema",
		));
	}
	if model_source.is_some() && !matches!(method, FormMethod::Post) {
		return Err(Error::new(
			ast.span,
			"model-backed form! requires `method: Post` for its server_fn action",
		));
	}
	if model_source.is_some() && slots.is_some() {
		return Err(Error::new(
			ast.span,
			"model-backed form! does not support `slots`; compose surrounding page content outside the generated form",
		));
	}
	if model_source.is_some() && (watch.is_some() || derived.is_some()) {
		return Err(Error::new(
			ast.span,
			"model-backed form! does not support `watch` or `derived` clauses",
		));
	}

	// Transform unified validators (scope filtering happens at codegen)
	let validators = transform_validators(&ast.validators, &ast.fields)?;

	// Transform stripped server_fn arguments (reinhardt-web#3971).
	let strip_arguments =
		transform_strip_arguments(&ast.strip_arguments, &ast.fields, ambient_arguments_source)?;

	if let TypedFormAction::ServerFn(_) = &action
		&& let Some(collection) = first_field_array_entry(&ast.fields)
	{
		return Err(Error::new(
			collection.span,
			"FieldArray is not supported with server_fn forms by reinhardt-pages macro codegen yet",
		));
	}

	// The parser guarantees that `name` is Some after successful parsing.
	let name = ast
		.name
		.clone()
		.expect("form name must be set after parsing");

	let mut typed = TypedFormMacro::new(name, action, ast.span);
	typed.method = method;
	typed.styling = styling;
	typed.state = state;
	typed.callbacks = callbacks;
	typed.watch = watch;
	typed.derived = derived;
	typed.redirect_on_success = redirect_on_success;
	typed.success_url = success_url;
	typed.initial_loader = initial_loader;
	typed.choices_loader = choices_loader;
	typed.slots = slots;
	typed.fields = fields;
	typed.model_source = model_source;
	typed.validators = validators;
	typed.strip_arguments = strip_arguments;

	Ok(typed)
}

fn transform_model_source(
	source: &Option<ModelFormSource>,
) -> Result<Option<TypedModelFormSource>> {
	let Some(source) = source else {
		return Ok(None);
	};

	let selection = match &source.selection {
		ModelFieldSelection::Fields(fields) => {
			validate_unique_model_field_names(fields, "fields")?;
			TypedModelFieldSelection::Fields(fields.clone())
		}
		ModelFieldSelection::Exclude(fields) => {
			validate_unique_model_field_names(fields, "exclude")?;
			TypedModelFieldSelection::Exclude(fields.clone())
		}
	};

	let selected_names = match &selection {
		TypedModelFieldSelection::Fields(fields) => Some(
			fields
				.iter()
				.map(ToString::to_string)
				.collect::<HashSet<_>>(),
		),
		TypedModelFieldSelection::Exclude(_) => None,
	};
	let excluded_names = match &selection {
		TypedModelFieldSelection::Exclude(fields) => fields
			.iter()
			.map(ToString::to_string)
			.collect::<HashSet<_>>(),
		TypedModelFieldSelection::Fields(_) => HashSet::new(),
	};

	let mut seen_overrides = HashSet::new();
	let overrides = source
		.overrides
		.iter()
		.map(|override_| {
			let field = override_.field.to_string();
			if !seen_overrides.insert(field.clone()) {
				return Err(Error::new(
					override_.field.span(),
					format!("duplicate `overrides` entry for field '{field}'"),
				));
			}
			if excluded_names.contains(&field) {
				return Err(Error::new(
					override_.field.span(),
					format!("override field '{field}' is present in `exclude`"),
				));
			}
			if let Some(selected_names) = &selected_names
				&& !selected_names.contains(&field)
			{
				return Err(Error::new(
					override_.field.span(),
					format!("override field '{field}' is not present in `fields`"),
				));
			}

			let widget = override_
				.widget
				.as_ref()
				.map(parse_model_widget)
				.transpose()?;
			Ok(TypedModelFieldOverride {
				field: override_.field.clone(),
				widget,
				label: override_.label.as_ref().map(syn::LitStr::value),
				help_text: override_.help_text.as_ref().map(syn::LitStr::value),
			})
		})
		.collect::<Result<Vec<_>>>()?;

	Ok(Some(TypedModelFormSource {
		model: source.model.clone(),
		policy: source.policy.clone(),
		selection,
		overrides,
	}))
}

fn validate_unique_model_field_names(fields: &[syn::Ident], clause: &str) -> Result<()> {
	let mut seen = HashSet::new();
	for field in fields {
		let name = field.to_string();
		if !seen.insert(name.clone()) {
			return Err(Error::new(
				field.span(),
				format!("duplicate `{clause}` identifier: '{name}'"),
			));
		}
	}
	Ok(())
}

fn first_field_array_entry(entries: &[FormFieldEntry]) -> Option<&FormFieldCollection> {
	entries.iter().find_map(|entry| match entry {
		FormFieldEntry::Collection(collection) => Some(collection.as_ref()),
		_ => None,
	})
}

/// Validates that all field names are unique.
fn validate_unique_field_names(entries: &[FormFieldEntry]) -> Result<()> {
	let mut seen = HashSet::new();

	for entry in entries {
		validate_unique_entry_name(entry, &mut seen, None)?;
	}

	Ok(())
}

fn validate_unique_entry_name(
	entry: &FormFieldEntry,
	seen: &mut HashSet<String>,
	group_name: Option<&str>,
) -> Result<()> {
	match entry {
		FormFieldEntry::Field(field) => {
			let name = field.name.to_string();
			if !seen.insert(name.clone()) {
				let message = if let Some(group_name) = group_name {
					format!(
						"duplicate field name: '{}' (in group '{}')",
						name, group_name
					)
				} else {
					format!("duplicate field name: '{}'", name)
				};
				return Err(Error::new(field.name.span(), message));
			}
		}
		FormFieldEntry::Group(group) => {
			let name = group.name.to_string();
			if !seen.insert(name.clone()) {
				return Err(Error::new(
					group.name.span(),
					format!("duplicate field/group name: '{}'", name),
				));
			}

			for child in &group.fields {
				validate_unique_entry_name(child, seen, Some(&name))?;
			}
		}
		FormFieldEntry::Collection(collection) => {
			let name = collection.name.to_string();
			if !seen.insert(name.clone()) {
				return Err(Error::new(
					collection.name.span(),
					format!("duplicate field/collection name: '{}'", name),
				));
			}
			validate_unique_collection_field_names(collection)?;
		}
		FormFieldEntry::SubmitButton(btn) => {
			let name = btn.name.to_string();
			if !seen.insert(name.clone()) {
				return Err(Error::new(
					btn.name.span(),
					format!("duplicate field/button name: '{}'", name),
				));
			}
		}
		FormFieldEntry::ResetButton(control)
		| FormFieldEntry::Button(control)
		| FormFieldEntry::ImageInput(control)
		| FormFieldEntry::Output(control)
		| FormFieldEntry::Meter(control)
		| FormFieldEntry::Progress(control) => {
			let name = control.name.to_string();
			if !seen.insert(name.clone()) {
				let message = if let Some(group_name) = group_name {
					format!(
						"duplicate field/control name: '{}' (in group '{}')",
						name, group_name
					)
				} else {
					format!("duplicate field/control name: '{}'", name)
				};
				return Err(Error::new(control.name.span(), message));
			}
		}
		FormFieldEntry::Datalist(datalist) => {
			let name = datalist.name.to_string();
			if !seen.insert(name.clone()) {
				let message = if let Some(group_name) = group_name {
					format!(
						"duplicate field/datalist name: '{}' (in group '{}')",
						name, group_name
					)
				} else {
					format!("duplicate field/datalist name: '{}'", name)
				};
				return Err(Error::new(datalist.name.span(), message));
			}
		}
	}

	Ok(())
}

fn validate_list_references(entries: &[TypedFormFieldEntry]) -> Result<()> {
	let mut datalist_names = HashSet::new();
	collect_datalist_names(entries, &mut datalist_names);
	validate_entry_list_references(entries, &datalist_names)
}

fn collect_datalist_names(entries: &[TypedFormFieldEntry], datalist_names: &mut HashSet<String>) {
	for entry in entries {
		match entry {
			TypedFormFieldEntry::Group(group) => {
				collect_datalist_names(&group.fields, datalist_names);
			}
			TypedFormFieldEntry::Collection(collection) => {
				collect_datalist_names(&collection.fields, datalist_names);
			}
			TypedFormFieldEntry::Datalist(datalist) => {
				datalist_names.insert(datalist.name.to_string());
			}
			TypedFormFieldEntry::Field(_)
			| TypedFormFieldEntry::SubmitButton(_)
			| TypedFormFieldEntry::ResetButton(_)
			| TypedFormFieldEntry::Button(_)
			| TypedFormFieldEntry::ImageInput(_)
			| TypedFormFieldEntry::Output(_)
			| TypedFormFieldEntry::Meter(_)
			| TypedFormFieldEntry::Progress(_) => {}
		}
	}
}

fn validate_entry_list_references(
	entries: &[TypedFormFieldEntry],
	datalist_names: &HashSet<String>,
) -> Result<()> {
	for entry in entries {
		match entry {
			TypedFormFieldEntry::Field(field) => {
				if let Some(list) = &field.native_attrs.list {
					let list_name = list.to_string();
					if !datalist_names.contains(&list_name) {
						return Err(Error::new(
							list.span(),
							format!(
								"list reference '{list_name}' must resolve to a Datalist entry",
							),
						));
					}
				}
			}
			TypedFormFieldEntry::Group(group) => {
				validate_entry_list_references(&group.fields, datalist_names)?;
			}
			TypedFormFieldEntry::Collection(collection) => {
				validate_entry_list_references(&collection.fields, datalist_names)?;
			}
			TypedFormFieldEntry::SubmitButton(_)
			| TypedFormFieldEntry::ResetButton(_)
			| TypedFormFieldEntry::Button(_)
			| TypedFormFieldEntry::ImageInput(_)
			| TypedFormFieldEntry::Output(_)
			| TypedFormFieldEntry::Meter(_)
			| TypedFormFieldEntry::Progress(_)
			| TypedFormFieldEntry::Datalist(_) => {}
		}
	}

	Ok(())
}

fn validate_unique_collection_field_names(collection: &FormFieldCollection) -> Result<()> {
	let mut seen = HashSet::new();

	for entry in &collection.fields {
		match entry {
			FormFieldEntry::Field(field) => {
				let name = field.name.to_string();
				if !seen.insert(name.clone()) {
					return Err(Error::new(
						field.name.span(),
						format!(
							"duplicate field name: '{}' (in collection '{}')",
							name, collection.name
						),
					));
				}
			}
			FormFieldEntry::Group(group) => {
				let group_name = group.name.to_string();
				if !seen.insert(group_name.clone()) {
					return Err(Error::new(
						group.name.span(),
						format!(
							"duplicate field/group name: '{}' (in collection '{}')",
							group_name, collection.name
						),
					));
				}

				for child in &group.fields {
					validate_unique_entry_name(child, &mut seen, Some(&group_name))?;
				}
			}
			FormFieldEntry::Collection(nested) => {
				let name = nested.name.to_string();
				if !seen.insert(name.clone()) {
					return Err(Error::new(
						nested.name.span(),
						format!(
							"duplicate field/collection name: '{}' (in collection '{}')",
							name, collection.name
						),
					));
				}
				validate_unique_collection_field_names(nested)?;
			}
			FormFieldEntry::SubmitButton(btn) => {
				let name = btn.name.to_string();
				if !seen.insert(name.clone()) {
					return Err(Error::new(
						btn.name.span(),
						format!(
							"duplicate field/button name: '{}' (in collection '{}')",
							name, collection.name
						),
					));
				}
			}
			FormFieldEntry::ResetButton(control)
			| FormFieldEntry::Button(control)
			| FormFieldEntry::ImageInput(control)
			| FormFieldEntry::Output(control)
			| FormFieldEntry::Meter(control)
			| FormFieldEntry::Progress(control) => {
				let name = control.name.to_string();
				if !seen.insert(name.clone()) {
					return Err(Error::new(
						control.name.span(),
						format!(
							"duplicate field/control name: '{}' (in collection '{}')",
							name, collection.name
						),
					));
				}
			}
			FormFieldEntry::Datalist(datalist) => {
				let name = datalist.name.to_string();
				if !seen.insert(name.clone()) {
					return Err(Error::new(
						datalist.name.span(),
						format!(
							"duplicate field/datalist name: '{}' (in collection '{}')",
							name, collection.name
						),
					));
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

/// Rejects removed form-level runtime state clauses.
fn transform_state(state: &Option<FormState>) -> Result<Option<TypedFormState>> {
	let Some(form_state) = state else {
		return Ok(None);
	};

	Err(Error::new(
		form_state.span,
		"`form! state: { ... }` was removed; use `use_form(&form).build().form_state()` for runtime form state",
	))
}

/// Transforms FormCallbacks to TypedFormCallbacks.
///
/// For callbacks, we simply pass through the closure expressions since
/// type checking is done by the Rust compiler during code generation.
fn transform_callbacks(callbacks: &FormCallbacks) -> Result<TypedFormCallbacks> {
	if callbacks.on_submit.is_some()
		|| callbacks.on_success.is_some()
		|| callbacks.on_success_ref.is_some()
		|| callbacks.on_error.is_some()
		|| callbacks.on_loading.is_some()
	{
		return Err(Error::new(
			callbacks.span.unwrap_or_else(Span::call_site),
			"`form!` runtime callback clauses were removed; configure submit lifecycle through `use_form(&form)`",
		));
	}

	Ok(TypedFormCallbacks {
		on_submit: callbacks.on_submit.clone(),
		on_success: callbacks.on_success.clone(),
		on_success_ref: callbacks.on_success_ref.clone(),
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
/// - Or is an HTTPS URL
/// - Supports `{param}` syntax for dynamic parameters
fn transform_redirect(redirect: &Option<syn::LitStr>) -> Result<Option<String>> {
	reinhardt_manouche::validator::validate_redirect_on_success(redirect)
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
	entries
		.iter()
		.map(|entry| transform_field_entry(entry, entries))
		.collect()
}

/// Transforms a single field entry (either a field or a group).
fn transform_field_entry(
	entry: &FormFieldEntry,
	all_entries: &[FormFieldEntry],
) -> Result<TypedFormFieldEntry> {
	match entry {
		FormFieldEntry::Field(field) => {
			let typed_field = transform_field(field)?;
			Ok(TypedFormFieldEntry::Field(Box::new(typed_field)))
		}
		FormFieldEntry::Group(group) => {
			let typed_group = transform_field_group(group, all_entries)?;
			Ok(TypedFormFieldEntry::Group(typed_group))
		}
		FormFieldEntry::Collection(collection) => {
			let typed_collection = transform_collection(collection)?;
			Ok(TypedFormFieldEntry::Collection(Box::new(typed_collection)))
		}
		FormFieldEntry::SubmitButton(btn) => {
			let typed_btn = transform_submit_button(btn)?;
			Ok(TypedFormFieldEntry::SubmitButton(typed_btn))
		}
		FormFieldEntry::ResetButton(control) => {
			let typed_control = transform_button_control(control)?;
			Ok(TypedFormFieldEntry::ResetButton(typed_control))
		}
		FormFieldEntry::Button(control) => {
			let typed_control = transform_button_control(control)?;
			Ok(TypedFormFieldEntry::Button(typed_control))
		}
		FormFieldEntry::ImageInput(control) => {
			let typed_control = transform_image_input(control)?;
			Ok(TypedFormFieldEntry::ImageInput(typed_control))
		}
		FormFieldEntry::Output(control) => {
			let typed_control = transform_output(control, all_entries)?;
			Ok(TypedFormFieldEntry::Output(typed_control))
		}
		FormFieldEntry::Meter(control) => {
			let typed_control = transform_meter(control)?;
			Ok(TypedFormFieldEntry::Meter(Box::new(typed_control)))
		}
		FormFieldEntry::Progress(control) => {
			let typed_control = transform_progress(control)?;
			Ok(TypedFormFieldEntry::Progress(Box::new(typed_control)))
		}
		FormFieldEntry::Datalist(datalist) => {
			let typed_datalist = transform_datalist(datalist)?;
			Ok(TypedFormFieldEntry::Datalist(Box::new(typed_datalist)))
		}
	}
}

/// Transforms a field group into a typed field group.
fn transform_field_group(
	group: &FormFieldGroup,
	all_entries: &[FormFieldEntry],
) -> Result<TypedFormFieldGroup> {
	let typed_fields: Vec<TypedFormFieldEntry> = group
		.fields
		.iter()
		.map(|entry| transform_field_entry(entry, all_entries))
		.collect::<Result<_>>()?;

	Ok(TypedFormFieldGroup {
		name: group.name.clone(),
		label: group.label.as_ref().map(|l| l.value()),
		class: group.class.as_ref().map(|c| c.value()),
		fields: typed_fields,
		span: group.span,
	})
}

/// Transforms a field collection into a typed field collection.
fn transform_collection(collection: &FormFieldCollection) -> Result<TypedFormFieldCollection> {
	let min_items = collection
		.min_items
		.as_ref()
		.map(|lit| {
			lit.base10_parse::<usize>().map_err(|_| {
				Error::new(lit.span(), "min_items must be a valid non-negative integer")
			})
		})
		.transpose()?;
	let max_items = collection
		.max_items
		.as_ref()
		.map(|lit| {
			lit.base10_parse::<usize>().map_err(|_| {
				Error::new(lit.span(), "max_items must be a valid non-negative integer")
			})
		})
		.transpose()?;

	if let (Some(min_items), Some(max_items)) = (min_items, max_items)
		&& min_items > max_items
	{
		return Err(Error::new(
			collection.span,
			"min_items cannot be greater than max_items",
		));
	}

	validate_collection_codegen_surface(collection)?;

	if let Some(nested) = collection.fields.iter().find_map(|entry| match entry {
		FormFieldEntry::Collection(nested) => Some(nested),
		_ => None,
	}) {
		return Err(Error::new(
			nested.span,
			"nested FieldArray is not supported by reinhardt-pages macro codegen yet",
		));
	}

	let fields = collection
		.fields
		.iter()
		.map(|entry| transform_field_entry(entry, &collection.fields))
		.collect::<Result<Vec<_>>>()?;

	Ok(TypedFormFieldCollection {
		name: collection.name.clone(),
		label: collection.label.as_ref().map(|lit| lit.value()),
		class: collection.class.as_ref().map(|lit| lit.value()),
		min_items,
		max_items,
		initial_from: collection.initial_from.as_ref().map(|lit| lit.value()),
		fields,
		render_item: collection.render_item.clone(),
		span: collection.span,
	})
}

fn validate_collection_codegen_surface(collection: &FormFieldCollection) -> Result<()> {
	if let Some(initial_from) = &collection.initial_from {
		return Err(Error::new(
			initial_from.span(),
			"FieldArray initial_from is not supported by reinhardt-pages macro codegen yet",
		));
	}
	if collection.render_item.is_some() {
		return Err(Error::new(
			collection.span,
			"FieldArray render_item is not supported by reinhardt-pages macro codegen yet",
		));
	}

	for entry in &collection.fields {
		match entry {
			FormFieldEntry::Field(field) => validate_collection_codegen_field(field)?,
			FormFieldEntry::Group(group) => {
				return Err(Error::new(
					group.span,
					"FieldGroup inside FieldArray is not supported by reinhardt-pages macro codegen yet",
				));
			}
			FormFieldEntry::Collection(_) => {}
			FormFieldEntry::SubmitButton(button) => {
				return Err(Error::new(
					button.span,
					"SubmitButton inside FieldArray is not supported by reinhardt-pages macro codegen yet",
				));
			}
			FormFieldEntry::ResetButton(control)
			| FormFieldEntry::Button(control)
			| FormFieldEntry::ImageInput(control)
			| FormFieldEntry::Output(control)
			| FormFieldEntry::Meter(control)
			| FormFieldEntry::Progress(control) => {
				return Err(Error::new(
					control.span,
					"non-value controls inside FieldArray are not supported by reinhardt-pages macro codegen yet",
				));
			}
			FormFieldEntry::Datalist(datalist) => {
				return Err(Error::new(
					datalist.span,
					"Datalist inside FieldArray is not supported by reinhardt-pages macro codegen yet",
				));
			}
		}
	}

	Ok(())
}

fn validate_collection_codegen_field(field: &FormFieldDef) -> Result<()> {
	let field_type = field.field_type.to_string();
	if field_type == "FileField" || field_type == "ImageField" {
		return Err(Error::new(
			field.span,
			"FileField and ImageField inside FieldArray are not supported by reinhardt-pages macro codegen yet",
		));
	}

	for property in &field.properties {
		match property {
			FormFieldProperty::Named { name, span, .. } if name == "initial" => {
				return Err(Error::new(
					*span,
					"initial is not supported inside FieldArray item fields yet",
				));
			}
			FormFieldProperty::InitialFrom { span, .. } => {
				return Err(Error::new(
					*span,
					"initial_from is not supported inside FieldArray item fields yet",
				));
			}
			FormFieldProperty::ChoicesFrom { span, .. }
			| FormFieldProperty::ChoiceValue { span, .. }
			| FormFieldProperty::ChoiceLabel { span, .. } => {
				return Err(Error::new(
					*span,
					"dynamic choices are not supported inside FieldArray item fields yet",
				));
			}
			_ => {}
		}
	}

	Ok(())
}

/// Transforms a submit button definition into a typed submit button.
const ALLOWED_SUBMIT_BUTTON_PROPERTIES: &[&str] = &["label", "class", "id", "disabled"];

fn transform_submit_button(btn: &FormSubmitButtonDef) -> Result<TypedSubmitButtonDef> {
	let mut label = None;
	let mut class = None;
	let mut id = None;
	let mut disabled = false;

	for prop in &btn.properties {
		match prop {
			FormFieldProperty::Named { name, value, span } => {
				let prop_name = name.to_string();
				match prop_name.as_str() {
					"label" => {
						if let syn::Expr::Lit(syn::ExprLit {
							lit: syn::Lit::Str(s),
							..
						}) = value
						{
							label = Some(s.value());
						} else {
							return Err(Error::new(*span, "label must be a string literal"));
						}
					}
					"class" => {
						if let syn::Expr::Lit(syn::ExprLit {
							lit: syn::Lit::Str(s),
							..
						}) = value
						{
							class = Some(s.value());
						} else {
							return Err(Error::new(*span, "class must be a string literal"));
						}
					}
					"id" => {
						if let syn::Expr::Lit(syn::ExprLit {
							lit: syn::Lit::Str(s),
							..
						}) = value
						{
							id = Some(s.value());
						} else {
							return Err(Error::new(*span, "id must be a string literal"));
						}
					}
					_ => {
						return Err(Error::new(
							*span,
							format!(
								"unknown SubmitButton property: '{}'. Allowed: {}",
								prop_name,
								ALLOWED_SUBMIT_BUTTON_PROPERTIES.join(", ")
							),
						));
					}
				}
			}
			FormFieldProperty::Flag { name, span } => {
				let prop_name = name.to_string();
				match prop_name.as_str() {
					"disabled" => disabled = true,
					_ => {
						return Err(Error::new(
							*span,
							format!(
								"unknown SubmitButton flag: '{}'. Allowed flags: disabled",
								prop_name
							),
						));
					}
				}
			}
			_ => {
				return Err(Error::new(
					btn.span,
					"SubmitButton only supports: label, class, id, disabled",
				));
			}
		}
	}

	Ok(TypedSubmitButtonDef {
		name: btn.name.clone(),
		label: label.unwrap_or_else(|| "Submit".to_string()),
		class,
		id,
		disabled,
		span: btn.span,
	})
}

fn transform_button_control(control: &FormControlEntryDef) -> Result<TypedButtonControlDef> {
	let kind_name = control_entry_kind_name(control.kind);
	let mut label = None;
	let mut class = None;
	let mut id = None;
	let mut disabled = false;

	for prop in &control.properties {
		match prop {
			FormFieldProperty::Named { name, value, span } => {
				let prop_name = name.to_string();
				match prop_name.as_str() {
					"label" => label = Some(expect_string_literal(value, *span, "label")?),
					"class" => class = Some(expect_string_literal(value, *span, "class")?),
					"id" => id = Some(expect_string_literal(value, *span, "id")?),
					_ => {
						return Err(Error::new(
							*span,
							format!("unknown {kind_name} property: '{prop_name}'"),
						));
					}
				}
			}
			FormFieldProperty::Flag { name, span } => {
				let prop_name = name.to_string();
				match prop_name.as_str() {
					"disabled" => disabled = true,
					_ => {
						return Err(Error::new(
							*span,
							format!("unknown {kind_name} flag: '{prop_name}'"),
						));
					}
				}
			}
			_ => {
				return Err(unknown_control_property_error(control.kind, prop));
			}
		}
	}

	let (kind, default_label) = match control.kind {
		FormControlEntryKind::ResetButton => (TypedButtonKind::Reset, "Reset"),
		FormControlEntryKind::Button => (TypedButtonKind::Button, "Button"),
		_ => {
			return Err(Error::new(
				control.span,
				format!("{kind_name} cannot be transformed as a button control"),
			));
		}
	};

	Ok(TypedButtonControlDef {
		name: control.name.clone(),
		kind,
		label: label.unwrap_or_else(|| default_label.to_string()),
		class,
		id,
		disabled,
		span: control.span,
	})
}

fn transform_image_input(control: &FormControlEntryDef) -> Result<TypedImageInputDef> {
	let mut src = None;
	let mut alt = None;
	let mut class = None;
	let mut id = None;
	let mut disabled = false;
	let mut width = None;
	let mut height = None;

	for prop in &control.properties {
		match prop {
			FormFieldProperty::Named { name, value, span } => {
				let prop_name = name.to_string();
				match prop_name.as_str() {
					"src" => src = Some(expect_string_literal(value, *span, "src")?),
					"alt" => alt = Some(expect_string_literal(value, *span, "alt")?),
					"class" => class = Some(expect_string_literal(value, *span, "class")?),
					"id" => id = Some(expect_string_literal(value, *span, "id")?),
					"width" => {
						width = Some(expect_positive_i64_literal(value, *span, "width")?);
					}
					"height" => {
						height = Some(expect_positive_i64_literal(value, *span, "height")?);
					}
					_ => {
						return Err(Error::new(
							*span,
							format!("unknown ImageInput property: '{prop_name}'"),
						));
					}
				}
			}
			FormFieldProperty::Flag { name, span } => {
				let prop_name = name.to_string();
				match prop_name.as_str() {
					"disabled" => disabled = true,
					_ => {
						return Err(Error::new(
							*span,
							format!("unknown ImageInput flag: '{prop_name}'"),
						));
					}
				}
			}
			_ => return Err(unknown_control_property_error(control.kind, prop)),
		}
	}

	Ok(TypedImageInputDef {
		name: control.name.clone(),
		src: src.ok_or_else(|| Error::new(control.span, "ImageInput requires src"))?,
		alt: alt.ok_or_else(|| Error::new(control.span, "ImageInput requires alt"))?,
		class,
		id,
		disabled,
		width,
		height,
		span: control.span,
	})
}

fn transform_output(
	control: &FormControlEntryDef,
	all_entries: &[FormFieldEntry],
) -> Result<TypedOutputDef> {
	let mut label = None;
	let mut for_fields = Vec::new();
	let mut class = None;
	let mut id = None;

	for prop in &control.properties {
		match prop {
			FormFieldProperty::Named { name, value, span } => {
				let prop_name = name.to_string();
				match prop_name.as_str() {
					"label" => label = Some(expect_string_literal(value, *span, "label")?),
					"for" => for_fields = expect_ident_array(value, *span, "for")?,
					"class" => class = Some(expect_string_literal(value, *span, "class")?),
					"id" => id = Some(expect_string_literal(value, *span, "id")?),
					_ => {
						return Err(Error::new(
							*span,
							format!("unknown Output property: '{prop_name}'"),
						));
					}
				}
			}
			FormFieldProperty::Flag { name, span } => {
				let prop_name = name.to_string();
				return Err(Error::new(
					*span,
					format!("unknown Output flag: '{prop_name}'"),
				));
			}
			_ => return Err(unknown_control_property_error(control.kind, prop)),
		}
	}

	for field in &for_fields {
		if !field_exists(all_entries, field) {
			return Err(Error::new(
				field.span(),
				format!("Output for references unknown value field: '{}'", field),
			));
		}
	}

	Ok(TypedOutputDef {
		name: control.name.clone(),
		label,
		for_fields,
		class,
		id,
		span: control.span,
	})
}

fn transform_meter(control: &FormControlEntryDef) -> Result<TypedMeterDef> {
	let mut label = None;
	let mut value = None;
	let mut min = None;
	let mut max = None;
	let mut low = None;
	let mut high = None;
	let mut optimum = None;
	let mut class = None;
	let mut id = None;

	for prop in &control.properties {
		match prop {
			FormFieldProperty::Named {
				name,
				value: expr,
				span,
			} => {
				let prop_name = name.to_string();
				match prop_name.as_str() {
					"label" => label = Some(expect_string_literal(expr, *span, "label")?),
					"value" => value = Some(expr.clone()),
					"min" => min = Some(expr.clone()),
					"max" => max = Some(expr.clone()),
					"low" => low = Some(expr.clone()),
					"high" => high = Some(expr.clone()),
					"optimum" => optimum = Some(expr.clone()),
					"class" => class = Some(expect_string_literal(expr, *span, "class")?),
					"id" => id = Some(expect_string_literal(expr, *span, "id")?),
					_ => {
						return Err(Error::new(
							*span,
							format!("unknown Meter property: '{prop_name}'"),
						));
					}
				}
			}
			FormFieldProperty::Flag { name, span } => {
				let prop_name = name.to_string();
				return Err(Error::new(
					*span,
					format!("unknown Meter flag: '{prop_name}'"),
				));
			}
			_ => return Err(unknown_control_property_error(control.kind, prop)),
		}
	}

	Ok(TypedMeterDef {
		name: control.name.clone(),
		label,
		value: value.ok_or_else(|| Error::new(control.span, "Meter requires value"))?,
		min,
		max,
		low,
		high,
		optimum,
		class,
		id,
		span: control.span,
	})
}

fn transform_progress(control: &FormControlEntryDef) -> Result<TypedProgressDef> {
	let mut label = None;
	let mut value = None;
	let mut max = None;
	let mut class = None;
	let mut id = None;

	for prop in &control.properties {
		match prop {
			FormFieldProperty::Named {
				name,
				value: expr,
				span,
			} => {
				let prop_name = name.to_string();
				match prop_name.as_str() {
					"label" => label = Some(expect_string_literal(expr, *span, "label")?),
					"value" => value = Some(expr.clone()),
					"max" => max = Some(expr.clone()),
					"class" => class = Some(expect_string_literal(expr, *span, "class")?),
					"id" => id = Some(expect_string_literal(expr, *span, "id")?),
					_ => {
						return Err(Error::new(
							*span,
							format!("unknown Progress property: '{prop_name}'"),
						));
					}
				}
			}
			FormFieldProperty::Flag { name, span } => {
				let prop_name = name.to_string();
				return Err(Error::new(
					*span,
					format!("unknown Progress flag: '{prop_name}'"),
				));
			}
			_ => return Err(unknown_control_property_error(control.kind, prop)),
		}
	}

	Ok(TypedProgressDef {
		name: control.name.clone(),
		label,
		value,
		max,
		class,
		id,
		span: control.span,
	})
}

fn transform_datalist(datalist: &FormDatalistDef) -> Result<TypedDatalistDef> {
	validate_datalist_properties(datalist)?;

	let static_choices = extract_static_choices(&datalist.properties)?;
	let choices_config = extract_choices_config(&datalist.properties);

	if !static_choices.is_empty() && choices_config.is_some() {
		return Err(Error::new(
			datalist.span,
			"Datalist cannot specify both options and choices_from",
		));
	}

	if static_choices.is_empty() && choices_config.is_none() {
		return Err(Error::new(
			datalist.span,
			"Datalist requires either options or choices_from",
		));
	}

	let static_choices = transform_static_choices(&static_choices, false, true)?;

	Ok(TypedDatalistDef {
		name: datalist.name.clone(),
		static_choices,
		choices_config,
		span: datalist.span,
	})
}

fn validate_datalist_properties(datalist: &FormDatalistDef) -> Result<()> {
	for prop in &datalist.properties {
		match prop {
			FormFieldProperty::Choices { .. }
			| FormFieldProperty::ChoicesFrom { .. }
			| FormFieldProperty::ChoiceValue { .. }
			| FormFieldProperty::ChoiceLabel { .. }
			| FormFieldProperty::ChoiceDisabled { .. } => {}
			FormFieldProperty::ChoiceGroup { .. }
			| FormFieldProperty::ChoiceGroupDisabled { .. } => {
				let prop_name = property_name(prop);
				return Err(Error::new(
					property_span(prop),
					format!("Datalist does not support {prop_name}; datalist options must be flat",),
				));
			}
			_ => {
				let prop_name = property_name(prop);
				return Err(Error::new(
					property_span(prop),
					format!("unknown Datalist property: '{prop_name}'"),
				));
			}
		}
	}

	Ok(())
}

fn control_entry_kind_name(kind: FormControlEntryKind) -> &'static str {
	match kind {
		FormControlEntryKind::ResetButton => "ResetButton",
		FormControlEntryKind::Button => "Button",
		FormControlEntryKind::ImageInput => "ImageInput",
		FormControlEntryKind::Output => "Output",
		FormControlEntryKind::Meter => "Meter",
		FormControlEntryKind::Progress => "Progress",
	}
}

fn property_name(prop: &FormFieldProperty) -> String {
	match prop {
		FormFieldProperty::Named { name, .. } | FormFieldProperty::Flag { name, .. } => {
			name.to_string()
		}
		FormFieldProperty::Widget { .. } => "widget".to_string(),
		FormFieldProperty::Wrapper { .. } => "wrapper".to_string(),
		FormFieldProperty::Icon { .. } => "icon".to_string(),
		FormFieldProperty::IconPosition { .. } => "icon_position".to_string(),
		FormFieldProperty::Attrs { .. } => "attrs".to_string(),
		FormFieldProperty::Bind { .. } => "bind".to_string(),
		FormFieldProperty::InitialFrom { .. } => "initial_from".to_string(),
		FormFieldProperty::ChoicesFrom { .. } => "choices_from".to_string(),
		FormFieldProperty::Choices { .. } => "choices".to_string(),
		FormFieldProperty::ChoiceValue { .. } => "choice_value".to_string(),
		FormFieldProperty::ChoiceLabel { .. } => "choice_label".to_string(),
		FormFieldProperty::ChoiceDisabled { .. } => "choice_disabled".to_string(),
		FormFieldProperty::ChoiceGroup { .. } => "choice_group".to_string(),
		FormFieldProperty::ChoiceGroupDisabled { .. } => "choice_group_disabled".to_string(),
	}
}

fn property_span(prop: &FormFieldProperty) -> Span {
	match prop {
		FormFieldProperty::Named { span, .. }
		| FormFieldProperty::Flag { span, .. }
		| FormFieldProperty::Widget { span, .. }
		| FormFieldProperty::Wrapper { span, .. }
		| FormFieldProperty::Icon { span, .. }
		| FormFieldProperty::IconPosition { span, .. }
		| FormFieldProperty::Attrs { span, .. }
		| FormFieldProperty::Bind { span, .. }
		| FormFieldProperty::InitialFrom { span, .. }
		| FormFieldProperty::ChoicesFrom { span, .. }
		| FormFieldProperty::Choices { span, .. }
		| FormFieldProperty::ChoiceValue { span, .. }
		| FormFieldProperty::ChoiceLabel { span, .. }
		| FormFieldProperty::ChoiceDisabled { span, .. }
		| FormFieldProperty::ChoiceGroup { span, .. }
		| FormFieldProperty::ChoiceGroupDisabled { span, .. } => *span,
	}
}

fn unknown_control_property_error(kind: FormControlEntryKind, prop: &FormFieldProperty) -> Error {
	Error::new(
		property_span(prop),
		format!(
			"unknown {} property: '{}'",
			control_entry_kind_name(kind),
			property_name(prop)
		),
	)
}

const ALLOWED_FIELD_PROPERTIES: &[&str] = &[
	"accept",
	"autocomplete",
	"autofocus",
	"capture",
	"class",
	"disabled",
	"error_class",
	"help_text",
	"initial",
	"label",
	"label_class",
	"list",
	"max",
	"max_length",
	"max_value",
	"min",
	"min_length",
	"min_value",
	"multiple",
	"pattern",
	"placeholder",
	"readonly",
	"required",
	"size",
	"step",
	"widget",
	"wrapper_class",
];

fn validate_known_field_properties(properties: &[FormFieldProperty]) -> Result<()> {
	for prop in properties {
		let (name, span) = match prop {
			FormFieldProperty::Named { name, span, .. }
			| FormFieldProperty::Flag { name, span } => (name.to_string(), *span),
			_ => continue,
		};

		if !ALLOWED_FIELD_PROPERTIES.contains(&name.as_str()) {
			return Err(Error::new(
				span,
				format!(
					"unknown field property: '{name}'. Use typed field properties for native form behavior (min, max, step, accept, capture, list, size), and use attrs only for aria_* or data_* custom attributes"
				),
			));
		}
	}

	Ok(())
}

/// Transforms a single field definition.
fn transform_field(field: &FormFieldDef) -> Result<TypedFormFieldDef> {
	// Parse field type
	let field_type = parse_field_type(&field.field_type, field.generics.as_ref())?;

	// Extract properties into categories
	let validation = extract_validation_properties(&field.properties)?;
	let display = extract_display_properties(&field.properties)?;
	let styling = extract_styling_properties(&field.properties)?;
	let widget = extract_widget(&field.properties, &field_type)?;
	validate_widget_field_compatibility(&field_type, &widget, field.span)?;
	let native_attrs = extract_native_attrs(&field.properties, &field_type, &widget)?;
	let wrapper = extract_wrapper(&field.properties)?;
	let icon = extract_icon(&field.properties)?;
	let custom_attrs = extract_custom_attrs(&field.properties)?;
	let bind = extract_bind(&field.properties);
	let initial_from = extract_initial_from(&field.properties);
	let initial_expr = extract_initial_expr(&field.properties);
	let choices_config = extract_choices_config(&field.properties);
	let static_choices_source = extract_static_choices(&field.properties)?;

	validate_radio_select_choice_group_properties(&field.properties, &widget)?;
	validate_known_field_properties(&field.properties)?;

	if !static_choices_source.is_empty() && choices_config.is_some() {
		return Err(Error::new(
			field.span,
			"field cannot specify both choices and choices_from",
		));
	}

	if choices_config.is_some() && !field_type_accepts_choices(&field_type) {
		return Err(Error::new(
			field.span,
			"choices_from can only be used with ChoiceField or MultipleChoiceField",
		));
	}

	if !static_choices_source.is_empty() && !field_type_accepts_choices(&field_type) {
		return Err(Error::new(
			field.span,
			"choices can only be used with ChoiceField or MultipleChoiceField",
		));
	}

	if !static_choices_source.is_empty()
		&& !matches!(widget, TypedWidget::Select | TypedWidget::SelectMultiple)
	{
		return Err(Error::new(
			field.span,
			"choices require Select or SelectMultiple widget",
		));
	}

	let static_choices = transform_static_choices(&static_choices_source, true, false)?;

	Ok(TypedFormFieldDef {
		name: field.name.clone(),
		field_type,
		widget,
		native_attrs,
		validation,
		display,
		styling,
		wrapper,
		icon,
		custom_attrs,
		bind,
		initial_from,
		initial_expr,
		choices_config,
		static_choices,
		span: field.span,
	})
}

fn field_type_accepts_choices(field_type: &TypedFieldType) -> bool {
	matches!(
		field_type,
		TypedFieldType::ChoiceField { .. } | TypedFieldType::MultipleChoiceField { .. }
	)
}

fn validate_radio_select_choice_group_properties(
	properties: &[FormFieldProperty],
	widget: &TypedWidget,
) -> Result<()> {
	if !matches!(widget, TypedWidget::RadioSelect) {
		return Ok(());
	}

	for prop in properties {
		match prop {
			FormFieldProperty::ChoiceGroup { .. } => {
				return Err(Error::new(
					property_span(prop),
					"choice_group is only supported on Select and SelectMultiple widgets",
				));
			}
			FormFieldProperty::ChoiceGroupDisabled { .. } => {
				return Err(Error::new(
					property_span(prop),
					"choice_group_disabled is only supported on Select and SelectMultiple widgets",
				));
			}
			_ => {}
		}
	}

	Ok(())
}

fn extract_static_choices(properties: &[FormFieldProperty]) -> Result<Vec<FormChoiceItem>> {
	let mut choices = None;

	for prop in properties {
		if let FormFieldProperty::Choices {
			choices: items,
			span,
		} = prop
		{
			if choices.is_some() {
				return Err(Error::new(
					*span,
					"choices/options specified more than once",
				));
			}
			choices = Some(items.clone());
		}
	}

	Ok(choices.unwrap_or_default())
}

fn transform_static_choices(
	choices: &[FormChoiceItem],
	allow_groups: bool,
	datalist: bool,
) -> Result<Vec<TypedChoiceItem>> {
	choices
		.iter()
		.map(|choice| transform_static_choice_item(choice, allow_groups, datalist))
		.collect()
}

fn transform_static_choice_item(
	choice: &FormChoiceItem,
	allow_groups: bool,
	datalist: bool,
) -> Result<TypedChoiceItem> {
	match choice {
		FormChoiceItem::Option(option) => Ok(TypedChoiceItem::Option(Box::new(
			transform_static_choice_option(option),
		))),
		FormChoiceItem::Group(group) => {
			if datalist {
				return Err(Error::new(
					group.span,
					"Datalist options cannot contain OptGroup",
				));
			}
			if !allow_groups {
				return Err(Error::new(
					group.span,
					"OptGroup is only supported by Select and SelectMultiple widgets",
				));
			}

			let mut options = Vec::with_capacity(group.options.len());
			for option in &group.options {
				match option {
					FormChoiceItem::Option(option) => {
						options.push(transform_static_choice_option(option));
					}
					FormChoiceItem::Group(group) => {
						return Err(Error::new(group.span, "nested OptGroup is not supported"));
					}
				}
			}

			Ok(TypedChoiceItem::Group(TypedChoiceGroup {
				label: group.label.value(),
				disabled: group.disabled,
				options,
				span: group.span,
			}))
		}
	}
}

fn transform_static_choice_option(
	option: &reinhardt_manouche::core::FormChoiceOption,
) -> TypedChoiceOption {
	TypedChoiceOption {
		value: option.value.clone(),
		label: option.label.clone().unwrap_or_else(|| option.value.clone()),
		disabled: option.disabled,
		span: option.span,
	}
}

/// Parses field type identifier into TypedFieldType enum.
///
/// `generics` carries the optional `<T1, T2, ...>` parsed off the field-type
/// identifier in the form! DSL. For non-generic field types, any generics
/// are rejected; for generic-capable field types, exactly one type argument
/// is required (or zero, in which case a default type is substituted).
fn parse_field_type(
	ident: &syn::Ident,
	generics: Option<&syn::punctuated::Punctuated<syn::Type, syn::Token![,]>>,
) -> Result<TypedFieldType> {
	let type_str = ident.to_string();

	// Helper: require exactly one type argument; default to `fallback` if absent.
	let require_one_generic = |fallback: syn::Type| -> Result<syn::Type> {
		match generics {
			None => Ok(fallback),
			Some(args) if args.len() == 1 => Ok(args[0].clone()),
			Some(args) => Err(Error::new(
				ident.span(),
				format!(
					"{} accepts exactly one type parameter, found {}",
					type_str,
					args.len(),
				),
			)),
		}
	};

	// Helper: reject any generic arguments on non-generic-capable field types.
	let reject_generics = || -> Result<()> {
		if generics.is_some() {
			Err(Error::new(
				ident.span(),
				format!("{} does not accept a type parameter", type_str),
			))
		} else {
			Ok(())
		}
	};

	match type_str.as_str() {
		// Non-generic field types
		"CharField" => {
			reject_generics()?;
			Ok(TypedFieldType::CharField)
		}
		"TextField" => {
			reject_generics()?;
			Ok(TypedFieldType::TextField)
		}
		"EmailField" => {
			reject_generics()?;
			Ok(TypedFieldType::EmailField)
		}
		"PasswordField" => {
			reject_generics()?;
			Ok(TypedFieldType::PasswordField)
		}
		"UrlField" => {
			reject_generics()?;
			Ok(TypedFieldType::UrlField)
		}
		"SlugField" => {
			reject_generics()?;
			Ok(TypedFieldType::SlugField)
		}
		"IntegerField" => {
			reject_generics()?;
			Ok(TypedFieldType::IntegerField)
		}
		"FloatField" => {
			reject_generics()?;
			Ok(TypedFieldType::FloatField)
		}
		"DecimalField" => {
			reject_generics()?;
			Ok(TypedFieldType::DecimalField)
		}
		"BooleanField" => {
			reject_generics()?;
			Ok(TypedFieldType::BooleanField)
		}
		"DateField" => {
			reject_generics()?;
			Ok(TypedFieldType::DateField)
		}
		"TimeField" => {
			reject_generics()?;
			Ok(TypedFieldType::TimeField)
		}
		"DateTimeField" => {
			reject_generics()?;
			Ok(TypedFieldType::DateTimeField)
		}
		"FileField" => {
			reject_generics()?;
			Ok(TypedFieldType::FileField)
		}
		"ImageField" => {
			reject_generics()?;
			Ok(TypedFieldType::ImageField)
		}
		"UuidField" => {
			reject_generics()?;
			Ok(TypedFieldType::UuidField)
		}

		// Generic-capable field types
		"HiddenField" => {
			let inner = require_one_generic(syn::parse_quote!(::std::string::String))?;
			Ok(TypedFieldType::HiddenField { inner })
		}
		"ChoiceField" => {
			let inner = require_one_generic(syn::parse_quote!(::std::string::String))?;
			Ok(TypedFieldType::ChoiceField { inner })
		}
		"MultipleChoiceField" => {
			let inner = require_one_generic(syn::parse_quote!(::std::string::String))?;
			Ok(TypedFieldType::MultipleChoiceField { inner })
		}
		"JsonField" => {
			let inner = require_one_generic(syn::parse_quote!(::std::string::String))?;
			Ok(TypedFieldType::JsonField { inner })
		}

		// Specialized (no generic accepted)
		"IpAddressField" => {
			reject_generics()?;
			Ok(TypedFieldType::IpAddressField)
		}

		_ => Err(Error::new(
			ident.span(),
			format!(
				"unknown field type: '{}'. Expected one of: CharField, TextField, EmailField, \
				PasswordField, IntegerField, FloatField, DecimalField, BooleanField, DateField, \
				TimeField, DateTimeField, ChoiceField, MultipleChoiceField, FileField, ImageField, \
				UrlField, SlugField, UuidField, IpAddressField, JsonField, HiddenField",
				type_str,
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
			FormFieldProperty::Choices { .. } => {} // Ignore static choice properties
			FormFieldProperty::ChoiceValue { .. } => {} // Ignore choice_value properties
			FormFieldProperty::ChoiceLabel { .. } => {} // Ignore choice_label properties
			FormFieldProperty::ChoiceDisabled { .. } => {} // Ignore choice_disabled properties
			FormFieldProperty::ChoiceGroup { .. } => {} // Ignore choice_group properties
			FormFieldProperty::ChoiceGroupDisabled { .. } => {} // Ignore choice_group_disabled properties
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
	let mut autocomplete = None;

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
					"autocomplete" => {
						autocomplete = Some(extract_string_value_from_expr(
							value,
							"autocomplete",
							*span,
						)?);
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
			FormFieldProperty::Choices { .. } => {} // Ignore static choice properties
			FormFieldProperty::ChoiceValue { .. } => {} // Ignore choice_value properties
			FormFieldProperty::ChoiceLabel { .. } => {} // Ignore choice_label properties
			FormFieldProperty::ChoiceDisabled { .. } => {} // Ignore choice_disabled properties
			FormFieldProperty::ChoiceGroup { .. } => {} // Ignore choice_group properties
			FormFieldProperty::ChoiceGroupDisabled { .. } => {} // Ignore choice_group_disabled properties
		}
	}

	Ok(TypedFieldDisplay {
		label,
		placeholder,
		help_text,
		disabled,
		readonly,
		autofocus,
		autocomplete,
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
			FormFieldProperty::Widget { widget, span: _ } => {
				return match widget {
					FormWidgetSpec::Builtin(widget_type) => parse_widget(widget_type),
					FormWidgetSpec::Custom(custom) => validate_custom_widget(custom),
				};
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

fn validate_custom_widget(custom: &FormCustomWidgetSpec) -> Result<TypedWidget> {
	if !custom.experimental {
		return Err(Error::new(
			custom.span,
			"CustomWidget requires the experimental marker",
		));
	}

	let adapter = custom
		.adapter
		.clone()
		.ok_or_else(|| Error::new(custom.span, "CustomWidget requires adapter"))?;

	Ok(TypedWidget::CustomExperimental(TypedCustomWidget {
		component: custom.component.clone(),
		adapter,
		span: custom.span,
	}))
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
		"MonthInput" => Ok(TypedWidget::MonthInput),
		"WeekInput" => Ok(TypedWidget::WeekInput),
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
					SelectMultiple, DateInput, MonthInput, WeekInput, TimeInput, DateTimeInput, \
					FileInput, HiddenInput, ColorInput, RangeInput, UrlInput, TelInput, SearchInput",
				widget_str
			),
		)),
	}
}

fn parse_model_widget(ident: &syn::Ident) -> Result<TypedWidget> {
	if ident == "TextArea" {
		return Ok(TypedWidget::Textarea);
	}
	if matches!(
		ident.to_string().as_str(),
		"Select" | "SelectMultiple" | "RadioSelect" | "MonthInput" | "WeekInput" | "FileInput"
	) {
		return Err(Error::new(
			ident.span(),
			"this widget is not supported by model-backed forms; use a supported scalar widget or an explicit non-model form",
		));
	}
	parse_widget(ident)
}

#[derive(Default)]
struct NativeAttrSpans {
	min: Option<Span>,
	max: Option<Span>,
	step: Option<Span>,
	multiple: Option<Span>,
	accept: Option<Span>,
	capture: Option<Span>,
	list: Option<Span>,
	size: Option<Span>,
}

fn validate_widget_field_compatibility(
	field_type: &TypedFieldType,
	widget: &TypedWidget,
	span: Span,
) -> Result<()> {
	match widget {
		TypedWidget::MonthInput if !is_string_valued_field(field_type) => Err(Error::new(
			span,
			"MonthInput is only supported on string-valued fields",
		)),
		TypedWidget::WeekInput if !is_string_valued_field(field_type) => Err(Error::new(
			span,
			"WeekInput is only supported on string-valued fields",
		)),
		_ => Ok(()),
	}
}

fn extract_native_attrs(
	properties: &[FormFieldProperty],
	field_type: &TypedFieldType,
	widget: &TypedWidget,
) -> Result<TypedFieldNativeAttrs> {
	let mut attrs = TypedFieldNativeAttrs::default();
	let mut spans = NativeAttrSpans::default();

	for prop in properties {
		match prop {
			FormFieldProperty::Named { name, value, span } => match name.to_string().as_str() {
				"min" => {
					attrs.min = Some(value.clone());
					spans.min = Some(*span);
				}
				"max" => {
					attrs.max = Some(value.clone());
					spans.max = Some(*span);
				}
				"step" => {
					attrs.step = Some(value.clone());
					spans.step = Some(*span);
				}
				"multiple" => {
					attrs.multiple = Some(expect_bool_literal(value, *span, "multiple")?);
					spans.multiple = Some(*span);
				}
				"accept" => {
					attrs.accept = Some(expect_string_literal(value, *span, "accept")?);
					spans.accept = Some(*span);
				}
				"capture" => {
					attrs.capture = Some(expect_string_literal(value, *span, "capture")?);
					spans.capture = Some(*span);
				}
				"list" => {
					attrs.list = Some(expect_ident_expr(value, *span, "list")?);
					spans.list = Some(*span);
				}
				"size" => {
					attrs.size = Some(expect_positive_i64_literal(value, *span, "size")?);
					spans.size = Some(*span);
				}
				_ => {}
			},
			FormFieldProperty::Flag { name, span } if name == "multiple" => {
				attrs.multiple = Some(true);
				spans.multiple = Some(*span);
			}
			_ => {}
		}
	}

	validate_native_attrs(field_type, widget, &attrs, &spans)?;
	Ok(attrs)
}

fn validate_native_attrs(
	field_type: &TypedFieldType,
	widget: &TypedWidget,
	attrs: &TypedFieldNativeAttrs,
	spans: &NativeAttrSpans,
) -> Result<()> {
	if attrs.accept.is_some() && !is_file_like_input(field_type, widget) {
		return Err(Error::new(
			spans.accept.unwrap_or_else(Span::call_site),
			"accept is only supported on file-like inputs",
		));
	}
	if attrs.capture.is_some() && !is_file_like_input(field_type, widget) {
		return Err(Error::new(
			spans.capture.unwrap_or_else(Span::call_site),
			"capture is only supported on file-like inputs",
		));
	}
	if attrs.multiple.is_some() {
		return Err(Error::new(
			spans.multiple.unwrap_or_else(Span::call_site),
			"multiple field property is not supported yet. Use SelectMultiple for multi-select fields; multi-file inputs require a future Vec<File> value contract",
		));
	}
	if attrs.list.is_some() && !is_text_like_input(widget) {
		return Err(Error::new(
			spans.list.unwrap_or_else(Span::call_site),
			"list is only supported on text-like inputs",
		));
	}
	if attrs.size.is_some() && !supports_size_attr(widget) {
		return Err(Error::new(
			spans.size.unwrap_or_else(Span::call_site),
			"size is only supported on text-like inputs",
		));
	}
	if attrs.min.is_some() && !supports_value_bound_attrs(widget) {
		return Err(Error::new(
			spans.min.unwrap_or_else(Span::call_site),
			"min is only supported on native value inputs",
		));
	}
	if attrs.max.is_some() && !supports_value_bound_attrs(widget) {
		return Err(Error::new(
			spans.max.unwrap_or_else(Span::call_site),
			"max is only supported on native value inputs",
		));
	}
	if attrs.step.is_some() && !supports_value_bound_attrs(widget) {
		return Err(Error::new(
			spans.step.unwrap_or_else(Span::call_site),
			"step is only supported on native value inputs",
		));
	}

	Ok(())
}

fn expect_string_literal(value: &syn::Expr, span: Span, prop_name: &str) -> Result<String> {
	if let syn::Expr::Lit(lit) = value
		&& let syn::Lit::Str(str_lit) = &lit.lit
	{
		return Ok(str_lit.value());
	}

	Err(Error::new(
		span,
		format!("{prop_name} must be a string literal"),
	))
}

fn expect_bool_literal(value: &syn::Expr, span: Span, prop_name: &str) -> Result<bool> {
	if let syn::Expr::Lit(lit) = value
		&& let syn::Lit::Bool(bool_lit) = &lit.lit
	{
		return Ok(bool_lit.value);
	}

	Err(Error::new(
		span,
		format!("{prop_name} must be true or false"),
	))
}

fn expect_ident_expr(value: &syn::Expr, span: Span, prop_name: &str) -> Result<syn::Ident> {
	if let syn::Expr::Path(path) = value
		&& path.path.leading_colon.is_none()
		&& path.path.segments.len() == 1
		&& let Some(ident) = path.path.get_ident()
	{
		return Ok(ident.clone());
	}

	Err(Error::new(
		span,
		format!("{prop_name} must be an identifier"),
	))
}

fn expect_ident_array(value: &syn::Expr, span: Span, prop_name: &str) -> Result<Vec<syn::Ident>> {
	let syn::Expr::Array(array) = value else {
		return Err(Error::new(
			span,
			format!("{prop_name} must be an array of field identifiers"),
		));
	};

	array
		.elems
		.iter()
		.map(|elem| expect_ident_expr(elem, span, prop_name))
		.collect()
}

fn expect_i64_literal(value: &syn::Expr, span: Span, prop_name: &str) -> Result<i64> {
	match value {
		syn::Expr::Lit(lit) => {
			if let syn::Lit::Int(int_lit) = &lit.lit {
				int_lit.base10_parse::<i64>().map_err(|_| {
					Error::new(span, format!("{prop_name} must be an integer literal"))
				})
			} else {
				Err(Error::new(
					span,
					format!("{prop_name} must be an integer literal"),
				))
			}
		}
		syn::Expr::Unary(unary) => {
			if let syn::UnOp::Neg(_) = unary.op
				&& let syn::Expr::Lit(lit) = &*unary.expr
				&& let syn::Lit::Int(int_lit) = &lit.lit
			{
				let value = int_lit.base10_parse::<i64>().map_err(|_| {
					Error::new(span, format!("{prop_name} must be an integer literal"))
				})?;
				Ok(-value)
			} else {
				Err(Error::new(
					span,
					format!("{prop_name} must be an integer literal"),
				))
			}
		}
		_ => Err(Error::new(
			span,
			format!("{prop_name} must be an integer literal"),
		)),
	}
}

fn expect_positive_i64_literal(value: &syn::Expr, span: Span, prop_name: &str) -> Result<i64> {
	let parsed = expect_i64_literal(value, span, prop_name)?;
	if parsed > 0 {
		return Ok(parsed);
	}

	Err(Error::new(
		span,
		format!("{prop_name} must be a positive integer literal"),
	))
}

fn is_string_valued_field(field_type: &TypedFieldType) -> bool {
	matches!(
		field_type,
		TypedFieldType::CharField
			| TypedFieldType::EmailField
			| TypedFieldType::UrlField
			| TypedFieldType::SlugField
			| TypedFieldType::PasswordField
			| TypedFieldType::TextField
	)
}

fn is_file_like_input(field_type: &TypedFieldType, widget: &TypedWidget) -> bool {
	matches!(
		(field_type, widget),
		(
			TypedFieldType::FileField | TypedFieldType::ImageField,
			TypedWidget::FileInput
		)
	)
}

fn is_text_like_input(widget: &TypedWidget) -> bool {
	matches!(
		widget,
		TypedWidget::TextInput
			| TypedWidget::SearchInput
			| TypedWidget::UrlInput
			| TypedWidget::TelInput
			| TypedWidget::EmailInput
			| TypedWidget::PasswordInput
			| TypedWidget::MonthInput
			| TypedWidget::WeekInput
	)
}

fn supports_size_attr(widget: &TypedWidget) -> bool {
	matches!(
		widget,
		TypedWidget::TextInput
			| TypedWidget::SearchInput
			| TypedWidget::UrlInput
			| TypedWidget::TelInput
			| TypedWidget::EmailInput
			| TypedWidget::PasswordInput
	)
}

fn supports_value_bound_attrs(widget: &TypedWidget) -> bool {
	matches!(
		widget,
		TypedWidget::NumberInput
			| TypedWidget::RangeInput
			| TypedWidget::DateInput
			| TypedWidget::TimeInput
			| TypedWidget::DateTimeInput
			| TypedWidget::MonthInput
			| TypedWidget::WeekInput
	)
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

/// Extracts the initial signal value expression from field properties.
///
/// When `initial: <expr>` is specified on a field, the expression is used as
/// the initial value for the field's `Signal::new(...)` instead of the type
/// default (issue #4386).
fn extract_initial_expr(properties: &[FormFieldProperty]) -> Option<syn::Expr> {
	for prop in properties {
		if let FormFieldProperty::Named { name, value, .. } = prop
			&& name == "initial"
		{
			return Some(value.clone());
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
	let mut choice_disabled: Option<String> = None;
	let mut choice_group: Option<String> = None;
	let mut choice_group_disabled: Option<String> = None;

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
			FormFieldProperty::ChoiceDisabled { path, .. } => {
				choice_disabled = Some(path.value());
			}
			FormFieldProperty::ChoiceGroup { path, .. } => {
				choice_group = Some(path.value());
			}
			FormFieldProperty::ChoiceGroupDisabled { path, .. } => {
				choice_group_disabled = Some(path.value());
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
			choice_disabled,
			choice_group,
			choice_group_disabled,
			span,
		)
	})
}

/// Transforms ambient argument entries into their typed form.
///
/// Validates two constraints:
/// 1. No duplicate argument names (each server_fn parameter may only be supplied once).
/// 2. No collision with declared form field names (would shadow a real input).
///
/// Tracked under reinhardt-web#3971.
fn transform_strip_arguments(
	args: &[StripArgument],
	fields: &[FormFieldEntry],
	source: Option<AmbientArgumentsSource>,
) -> Result<Vec<TypedStripArgument>> {
	let mut seen = HashSet::new();
	let mut typed = Vec::with_capacity(args.len());
	let keyword = ambient_arguments_keyword(source);
	let canonical_suffix = ambient_arguments_canonical_suffix(source);
	let collision_entry = ambient_arguments_collision_entry(source);

	for arg in args {
		let name_str = arg.name.to_string();

		if !seen.insert(name_str.clone()) {
			return Err(Error::new(
				arg.span,
				format!(
					"duplicate {keyword} entry '{name_str}': each server_fn argument may only appear once{canonical_suffix}"
				),
			));
		}

		if ambient_argument_surface_entry_exists(fields, &arg.name) {
			return Err(Error::new(
				arg.span,
				format!(
					"{keyword} key '{name_str}' collides with a declared form field; either rename the field or remove this {collision_entry} entry{canonical_suffix}"
				),
			));
		}

		typed.push(TypedStripArgument {
			name: arg.name.clone(),
			value: arg.value.clone(),
			span: arg.span,
		});
	}

	Ok(typed)
}

fn ambient_arguments_keyword(source: Option<AmbientArgumentsSource>) -> &'static str {
	match source {
		Some(AmbientArgumentsSource::StripArguments) => "strip_arguments",
		_ => "ambient_arguments",
	}
}

fn ambient_arguments_canonical_suffix(source: Option<AmbientArgumentsSource>) -> &'static str {
	match source {
		Some(AmbientArgumentsSource::StripArguments) => "; use ambient_arguments for new code",
		_ => "",
	}
}

fn ambient_arguments_collision_entry(source: Option<AmbientArgumentsSource>) -> &'static str {
	match source {
		Some(AmbientArgumentsSource::StripArguments) => "strip",
		_ => "ambient",
	}
}

/// Checks whether a scalar value field with the given name exists.
fn field_exists(entries: &[FormFieldEntry], name: &syn::Ident) -> bool {
	for entry in entries {
		match entry {
			FormFieldEntry::Field(field) => {
				if field.name == *name {
					return true;
				}
			}
			FormFieldEntry::Group(group) => {
				if field_exists(&group.fields, name) {
					return true;
				}
			}
			FormFieldEntry::Collection(_)
			| FormFieldEntry::SubmitButton(_)
			| FormFieldEntry::ResetButton(_)
			| FormFieldEntry::Button(_)
			| FormFieldEntry::ImageInput(_)
			| FormFieldEntry::Output(_)
			| FormFieldEntry::Meter(_)
			| FormFieldEntry::Progress(_)
			| FormFieldEntry::Datalist(_) => {}
		}
	}
	false
}

/// Checks if an ambient argument collides with a declared form surface entry.
///
/// Ambient arguments share the form's top-level submission surface, so this
/// includes top-level fields, group fields, and top-level collection names.
/// Collection item fields are not part of that surface.
fn ambient_argument_surface_entry_exists(entries: &[FormFieldEntry], name: &syn::Ident) -> bool {
	for entry in entries {
		match entry {
			FormFieldEntry::Field(field) => {
				if field.name == *name {
					return true;
				}
			}
			FormFieldEntry::Group(group) => {
				if field_exists(&group.fields, name) {
					return true;
				}
			}
			FormFieldEntry::Collection(collection) => {
				if collection.name == *name {
					return true;
				}
			}
			FormFieldEntry::SubmitButton(_)
			| FormFieldEntry::ResetButton(_)
			| FormFieldEntry::Button(_)
			| FormFieldEntry::ImageInput(_)
			| FormFieldEntry::Output(_)
			| FormFieldEntry::Meter(_)
			| FormFieldEntry::Progress(_)
			| FormFieldEntry::Datalist(_) => {}
		}
	}
	false
}

/// Checks if a field-level validator targets a declared scalar field.
///
/// Field validators can target top-level fields and fields within groups.
/// Collection names and collection item fields are intentionally excluded
/// until collection validation semantics are implemented.
fn validator_target_field_exists(entries: &[FormFieldEntry], name: &syn::Ident) -> bool {
	for entry in entries {
		match entry {
			FormFieldEntry::Field(field) => {
				if field.name == *name {
					return true;
				}
			}
			FormFieldEntry::Group(group) => {
				if field_exists(&group.fields, name) {
					return true;
				}
			}
			FormFieldEntry::Collection(_)
			| FormFieldEntry::SubmitButton(_)
			| FormFieldEntry::ResetButton(_)
			| FormFieldEntry::Button(_)
			| FormFieldEntry::ImageInput(_)
			| FormFieldEntry::Output(_)
			| FormFieldEntry::Meter(_)
			| FormFieldEntry::Progress(_)
			| FormFieldEntry::Datalist(_) => {}
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
				if !validator_target_field_exists(fields, field_name) {
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

/// Transforms a validator rule, propagating its execution scope.
///
/// Converts the closure expression to a regular expression for code generation.
fn transform_validator_rule(rule: &ValidatorRule) -> Result<TypedValidatorRule> {
	// Convert ExprClosure body to Expr for use in validation
	let condition: syn::Expr = (*rule.expr.body).clone();

	Ok(TypedValidatorRule {
		scope: rule.scope.clone(),
		condition,
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
	use quote::{ToTokens, quote};

	fn parse_and_validate(input: proc_macro2::TokenStream) -> Result<TypedFormMacro> {
		let ambient_arguments_source =
			reinhardt_manouche::parser::detect_ambient_arguments_source(&input);
		let ast: FormMacro = syn::parse2(input)?;
		validate(&ast, ambient_arguments_source)
	}

	fn assert_validator_parity(
		input: proc_macro2::TokenStream,
		mutate_ast: impl FnOnce(&mut FormMacro),
	) {
		let ambient_arguments_source =
			reinhardt_manouche::parser::detect_ambient_arguments_source(&input);
		let mut ast: FormMacro = syn::parse2(input).expect("form input must parse");
		mutate_ast(&mut ast);

		let macro_result = validate(&ast, ambient_arguments_source);
		let manouche_result =
			reinhardt_manouche::validator::validate_form_with_ambient_arguments_source(
				&ast,
				ambient_arguments_source,
			);

		match (macro_result, manouche_result) {
			(Ok(macro_typed), Ok(manouche_typed)) => {
				assert_eq!(macro_typed.name, manouche_typed.name);
				assert_eq!(
					macro_typed.redirect_on_success,
					manouche_typed.redirect_on_success
				);
				assert_eq!(
					macro_typed.success_url.is_some(),
					manouche_typed.success_url.is_some()
				);
				assert_eq!(
					strip_argument_signatures(&macro_typed.strip_arguments),
					strip_argument_signatures(&manouche_typed.strip_arguments)
				);
				assert_eq!(macro_typed.fields.len(), manouche_typed.fields.len());
			}
			(Err(macro_err), Err(manouche_err)) => {
				assert_eq!(macro_err.to_string(), manouche_err.to_string());
			}
			(Ok(_), Err(err)) => {
				panic!("macros validator accepted input rejected by manouche: {err}");
			}
			(Err(err), Ok(_)) => {
				panic!("macros validator rejected input accepted by manouche: {err}");
			}
		}
	}

	fn assert_validator_parity_for(input: proc_macro2::TokenStream) {
		assert_validator_parity(input, |_| {});
	}

	fn strip_argument_signatures(args: &[TypedStripArgument]) -> Vec<(String, String)> {
		args.iter()
			.map(|arg| {
				(
					arg.name.to_string(),
					arg.value.to_token_stream().to_string(),
				)
			})
			.collect()
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
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("`form! state: { ... }` was removed")
		);
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
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("`form! state: { ... }` was removed")
		);
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
				.contains("`form! state: { ... }` was removed")
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
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("runtime callback clauses were removed")
		);
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
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("runtime callback clauses were removed")
		);
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
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("`form! state: { ... }` was removed")
		);
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
		assert!(!loader.segments.is_empty());
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

			fields: {
				name: CharField { required },
			},
		};

		let result = parse_and_validate(input);
		assert!(result.is_ok());

		let typed = result.unwrap();
		assert!(typed.initial_loader.is_some());
		assert!(!typed.callbacks.has_any());
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
		assert!(typed.state.is_none());
		assert!(!typed.callbacks.has_any());

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
		assert!(typed.state.is_none());

		// Check callbacks
		assert!(!typed.callbacks.has_any());

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
		let street = group.fields[0].as_field().unwrap();
		assert_eq!(street.name.to_string(), "street");
		assert!(street.validation.required);
		assert_eq!(group.fields[1].as_field().unwrap().name.to_string(), "city");
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
		assert_eq!(
			group.fields[0].as_field().unwrap().initial_from,
			Some("name".to_string())
		);
		assert_eq!(
			group.fields[1].as_field().unwrap().initial_from,
			Some("biography".to_string())
		);
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
		let email = group.fields[0].as_field().unwrap();
		assert!(email.validation.required);
		assert_eq!(email.display.label, Some("Email Address".to_string()));
		assert_eq!(
			email.display.placeholder,
			Some("you@example.com".to_string())
		);
		assert!(email.has_wrapper());

		// Check password field properties
		let password = group.fields[1].as_field().unwrap();
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
		assert!(typed.state.is_none());

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

		let category_field = group.fields[0].as_field().unwrap();
		assert!(category_field.choices_config.is_some());
		assert_eq!(
			category_field.choices_config.as_ref().unwrap().choices_from,
			"categories"
		);

		let status_field = group.fields[1].as_field().unwrap();
		assert!(status_field.choices_config.is_some());
		assert_eq!(
			status_field.choices_config.as_ref().unwrap().choices_from,
			"statuses"
		);
	}

	#[rstest::rstest]
	fn test_validate_dynamic_datalist_and_list_reference() {
		let input = quote! {
			name: DynamicDatalistForm,
			action: "/search",
			choices_loader: load_suggestions,

			fields: {
				query: CharField {
					widget: SearchInput,
					list: suggestions,
				},
				suggestions: Datalist {
					choices_from: "items",
					choice_value: "value",
					choice_label: "label",
					choice_disabled: "disabled",
				},
			},
		};

		let typed = parse_and_validate(input).unwrap();
		let field = typed.fields[0].as_field().unwrap();
		assert_eq!(
			field.native_attrs.list.as_ref().unwrap().to_string(),
			"suggestions"
		);

		let datalist = typed.fields[1].as_datalist().unwrap();
		let config = datalist.choices_config.as_ref().unwrap();
		assert_eq!(config.choices_from, "items");
		assert_eq!(config.choice_value, "value");
		assert_eq!(config.choice_label, "label");
		assert_eq!(config.choice_disabled.as_deref(), Some("disabled"));
		assert!(datalist.static_choices.is_empty());
	}

	#[rstest::rstest]
	fn test_validate_list_rejects_missing_datalist() {
		let input = quote! {
			name: MissingDatalistForm,
			action: "/search",

			fields: {
				query: CharField {
					widget: SearchInput,
					list: suggestions,
				},
			},
		};

		let err = parse_and_validate(input).unwrap_err().to_string();
		assert_eq!(
			err,
			"list reference 'suggestions' must resolve to a Datalist entry"
		);
	}

	#[rstest::rstest]
	fn test_validate_list_rejects_value_field_reference() {
		let input = quote! {
			name: ValueFieldListForm,
			action: "/search",

			fields: {
				query: CharField {
					widget: SearchInput,
					list: suggestions,
				},
				suggestions: CharField {},
			},
		};

		let err = parse_and_validate(input).unwrap_err().to_string();
		assert_eq!(
			err,
			"list reference 'suggestions' must resolve to a Datalist entry"
		);
	}

	#[rstest::rstest]
	fn test_validate_datalist_rejects_choice_group() {
		let input = quote! {
			name: InvalidDatalistGroupForm,
			action: "/search",

			fields: {
				suggestions: Datalist {
					choices_from: "items",
					choice_group: "group",
				},
			},
		};

		let err = parse_and_validate(input).unwrap_err().to_string();
		assert_eq!(
			err,
			"Datalist does not support choice_group; datalist options must be flat"
		);
	}

	#[rstest::rstest]
	fn test_validate_datalist_rejects_unknown_property() {
		let input = quote! {
			name: InvalidDatalistPropertyForm,
			action: "/search",

			fields: {
				suggestions: Datalist {
					options: [("a", "A")],
					label: "Suggestions",
				},
			},
		};

		let err = parse_and_validate(input).unwrap_err().to_string();
		assert_eq!(err, "unknown Datalist property: 'label'");
	}

	#[test]
	fn test_transform_redirect_rejects_http_url() {
		let lit = syn::LitStr::new("http://example.com/success", proc_macro2::Span::call_site());
		let result = transform_redirect(&Some(lit));
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert_eq!(
			err,
			"redirect_on_success rejects HTTP URLs for security. Use HTTPS URL instead."
		);
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
		assert_eq!(
			result.unwrap_err().to_string(),
			"redirect_on_success path must be a relative path (starting with '/', './', or '../') or an HTTPS URL"
		);
	}

	#[test]
	fn test_transform_redirect_none() {
		let result = transform_redirect(&None);
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), None);
	}

	#[rstest::rstest]
	fn test_reject_simultaneous_redirect_on_success_and_success_url() {
		// Arrange — both navigation hooks specified at once. The generated
		// `submit()` would otherwise call `navigate()` twice for a single
		// successful submit (Refs #4610 codegen review).
		//
		// The `success_url:` token cannot be exercised through the
		// untyped parser today because of upstream issue #4604 / #4611
		// (the `reinhardt-manouche` `success_url:` parser arm consumes a
		// redundant `:`). To still test the validator's mutual-exclusion
		// branch *now*, parse the redirect-only form and graft a
		// hand-built `success_url` expression onto the AST, then invoke
		// `validate` directly.
		//
		// Ideal implementation after #4604 / #4611:
		//
		//     let input = quote! {
		//         name: VoteForm,
		//         action: "/api/vote",
		//         redirect_on_success: "/thanks",
		//         success_url: |_form| String::from("/thanks-too"),
		//         fields: { choice_id: IntegerField { required } },
		//     };
		//     let err = parse_and_validate(input)
		//         .expect_err("must reject both attributes together");
		//
		// — i.e. parse `success_url:` directly through the untyped parser
		// and reuse `parse_and_validate` instead of grafting onto the AST.
		let input = quote! {
			name: VoteForm,
			action: "/api/vote",
			redirect_on_success: "/thanks",

			fields: {
				choice_id: IntegerField { required },
			},
		};
		let mut ast: FormMacro = syn::parse2(input).expect("redirect-only form must parse");
		ast.success_url = Some(
			syn::parse_str::<syn::Expr>("|_form| String::from(\"/thanks-too\")")
				.expect("success_url expression must parse"),
		);

		// Act
		let result = validate(&ast, None);

		// Assert — strict equality on the full diagnostic so any wording
		// change forces an intentional test update (CLAUDE.md: prefer
		// `assert_eq!` over loose `contains`).
		let err = result.expect_err("must reject both attributes together");
		let msg = err.to_string();
		assert_eq!(
			msg,
			"form! cannot specify both `redirect_on_success` and `success_url`: each generates an \
			 SPA navigation on successful submit, so combining them would push two history \
			 entries and fire two observer cycles. Use `redirect_on_success` for a static URL or \
			 `success_url` for a dynamically-computed one, not both."
		);
	}

	#[rstest::rstest]
	fn validator_parity_rejects_removed_state_clause() {
		let input = quote! {
			name: StateForm,
			server_fn: submit_form,

			state: { loading, error, success },

			fields: {
				data: CharField {},
			},
		};

		assert_validator_parity_for(input);
	}

	#[rstest::rstest]
	fn validator_parity_rejects_removed_runtime_callback_clauses() {
		let input = quote! {
			name: CallbackForm,
			server_fn: submit_form,

			on_success: |result| { let _ = result; },
			on_error: |error| { let _ = error; },
			on_loading: |loading| { let _ = loading; },

			fields: {
				data: CharField {},
			},
		};

		assert_validator_parity_for(input);
	}

	#[rstest::rstest]
	fn validator_parity_rejects_ambient_argument_duplicate_with_same_diagnostic() {
		let input = quote! {
			name: VoteForm,
			server_fn: submit_vote,
			ambient_arguments: {
				csrf_token: csrf_a(),
				csrf_token: csrf_b(),
			},
			fields: {
				choice_id: IntegerField { required },
			},
		};

		assert_validator_parity_for(input);
	}

	#[rstest::rstest]
	fn validator_parity_rejects_strip_argument_collision_with_same_diagnostic() {
		let input = quote! {
			name: VoteForm,
			server_fn: submit_vote,
			strip_arguments: {
				choice_id: ambient_choice_id(),
			},
			fields: {
				choice_id: IntegerField { required },
			},
		};

		assert_validator_parity_for(input);
	}

	#[rstest::rstest]
	fn test_validate_strip_argument_rejects_field_array_collision() {
		let input = quote! {
			name: InvoiceForm,
			server_fn: submit_invoice,
			strip_arguments: {
				line_items: ambient_line_items(),
			},
			fields: {
				line_items: FieldArray {
					fields: {
						description: CharField {},
					},
				},
			},
		};

		let err = parse_and_validate(input).expect_err("FieldArray name collision must fail");
		assert_eq!(
			err.to_string(),
			"strip_arguments key 'line_items' collides with a declared form field; either rename the field or remove this strip entry; use ambient_arguments for new code"
		);
	}

	#[rstest::rstest]
	fn test_validate_server_fn_rejects_field_array() {
		let input = quote! {
			name: InvoiceForm,
			server_fn: submit_invoice,
			fields: {
				customer_name: CharField {},
				line_items: FieldArray {
					fields: {
						description: CharField {},
					},
				},
			},
		};

		let err = parse_and_validate(input).expect_err("server_fn FieldArray must fail");
		assert_eq!(
			err.to_string(),
			"FieldArray is not supported with server_fn forms by reinhardt-pages macro codegen yet"
		);
	}

	#[rstest::rstest]
	fn test_validate_field_validator_rejects_field_array_name() {
		let input = quote! {
			name: InvoiceForm,
			action: "/invoices",
			fields: {
				line_items: FieldArray {
					fields: {
						description: CharField {},
					},
				},
			},
			validators: {
				line_items: [|_v| true => "Line items are invalid"],
			},
		};

		let err = parse_and_validate(input).expect_err("collection validator target must fail");
		assert_eq!(
			err.to_string(),
			"validator references unknown field: 'line_items'"
		);
	}

	#[rstest::rstest]
	fn test_validate_nested_field_array_rejected() {
		let input = quote! {
			name: InvoiceForm,
			action: "/invoices",
			fields: {
				line_items: FieldArray {
					fields: {
						description: CharField {},
						discounts: FieldArray {
							fields: {
								amount: IntegerField {},
							},
						},
					},
				},
			},
		};

		let err = parse_and_validate(input).expect_err("nested FieldArray must fail");
		assert_eq!(
			err.to_string(),
			"nested FieldArray is not supported by reinhardt-pages macro codegen yet"
		);
	}

	#[rstest::rstest]
	fn test_validate_field_array_rejects_initial_on_item_field() {
		let input = quote! {
			name: InvoiceForm,
			action: "/invoices",
			fields: {
				line_items: FieldArray {
					fields: {
						quantity: IntegerField {
							initial: 1,
						},
					},
				},
			},
		};

		let err = parse_and_validate(input).expect_err("FieldArray item initial must fail");
		assert_eq!(
			err.to_string(),
			"initial is not supported inside FieldArray item fields yet"
		);
	}

	#[rstest::rstest]
	fn test_validate_field_array_rejects_file_fields() {
		let input = quote! {
			name: UploadForm,
			action: "/uploads",
			fields: {
				attachments: FieldArray {
					fields: {
						document: FileField {},
					},
				},
			},
		};

		let err = parse_and_validate(input).expect_err("FieldArray FileField must fail");
		assert_eq!(
			err.to_string(),
			"FileField and ImageField inside FieldArray are not supported by reinhardt-pages macro codegen yet"
		);
	}

	#[rstest::rstest]
	fn test_validate_field_array_rejects_field_groups() {
		let input = quote! {
			name: InvoiceForm,
			action: "/invoices",
			fields: {
				line_items: FieldArray {
					fields: {
						details: FieldGroup {
							fields: {
								description: CharField {},
							},
						},
					},
				},
			},
		};

		let err = parse_and_validate(input).expect_err("FieldArray FieldGroup must fail");
		assert_eq!(
			err.to_string(),
			"FieldGroup inside FieldArray is not supported by reinhardt-pages macro codegen yet"
		);
	}

	#[rstest::rstest]
	fn test_validate_field_array_rejects_render_item() {
		let input = quote! {
			name: InvoiceForm,
			action: "/invoices",
			fields: {
				line_items: FieldArray {
					fields: {
						description: CharField {},
					},
					render_item: |item| { item.default_row() },
				},
			},
		};

		let err = parse_and_validate(input).expect_err("FieldArray render_item must fail");
		assert_eq!(
			err.to_string(),
			"FieldArray render_item is not supported by reinhardt-pages macro codegen yet"
		);
	}

	#[rstest::rstest]
	fn validator_parity_rejects_simultaneous_redirect_and_success_url() {
		let input = quote! {
			name: VoteForm,
			action: "/api/vote",
			redirect_on_success: "/thanks",

			fields: {
				choice_id: IntegerField { required },
			},
		};

		assert_validator_parity(input, |ast| {
			ast.success_url = Some(
				syn::parse_str::<syn::Expr>("|_form| String::from(\"/thanks-too\")")
					.expect("success_url expression must parse"),
			);
		});
	}

	#[rstest::rstest]
	#[case::dot_relative("./success")]
	#[case::double_dot_relative("../home")]
	#[case::mixed_case_https("HTTPS://EXAMPLE.COM/success")]
	fn validator_parity_accepts_redirect_variants(#[case] redirect: &str) {
		let redirect = syn::LitStr::new(redirect, proc_macro2::Span::call_site());
		let input = quote! {
			name: RedirectForm,
			action: "/test",
			redirect_on_success: #redirect,

			fields: {
				data: CharField { required },
			},
		};

		assert_validator_parity_for(input);
	}

	#[rstest::rstest]
	fn test_redirect_on_success_alone_is_accepted() {
		// Arrange — only `redirect_on_success`, no `success_url`. Must
		// still validate cleanly (regression guard for the mutual-exclusion
		// check above).
		let input = quote! {
			name: VoteForm,
			action: "/api/vote",
			redirect_on_success: "/thanks",

			fields: {
				choice_id: IntegerField { required },
			},
		};

		// Act
		let result = parse_and_validate(input);

		// Assert
		assert!(
			result.is_ok(),
			"single-attribute case must pass: {result:?}"
		);
	}

	#[rstest::rstest]
	fn test_validate_file_field() {
		// Arrange
		let input = quote! {
			name: UploadForm,
			action: "/api/upload",

			fields: {
				document: FileField {},
			},
		};

		// Act
		let result = parse_and_validate(input);

		// Assert
		assert!(result.is_ok());
		let typed = result.unwrap();
		assert_eq!(typed.fields.len(), 1);
		let field = typed.fields[0].as_field().unwrap();
		assert!(matches!(field.field_type, TypedFieldType::FileField));
		assert!(matches!(field.widget, TypedWidget::FileInput));
	}

	#[rstest::rstest]
	fn test_validate_image_field() {
		// Arrange
		let input = quote! {
			name: AvatarForm,
			action: "/api/avatar",

			fields: {
				photo: ImageField {},
			},
		};

		// Act
		let result = parse_and_validate(input);

		// Assert
		assert!(result.is_ok());
		let typed = result.unwrap();
		assert_eq!(typed.fields.len(), 1);
		let field = typed.fields[0].as_field().unwrap();
		assert!(matches!(field.field_type, TypedFieldType::ImageField));
		assert!(matches!(field.widget, TypedWidget::FileInput));
	}

	#[rstest::rstest]
	fn test_validate_size_rejects_zero() {
		// Arrange
		let input = quote! {
			name: SearchForm,
			action: "/search",

			fields: {
				query: CharField {
					widget: SearchInput,
					size: 0,
				},
			},
		};

		// Act
		let result = parse_and_validate(input);

		// Assert
		assert!(result.is_err());
		assert_eq!(
			result.unwrap_err().to_string(),
			"size must be a positive integer literal"
		);
	}

	#[rstest::rstest]
	fn test_validate_size_rejects_negative() {
		// Arrange
		let input = quote! {
			name: SearchForm,
			action: "/search",

			fields: {
				query: CharField {
					widget: SearchInput,
					size: -1,
				},
			},
		};

		// Act
		let result = parse_and_validate(input);

		// Assert
		assert!(result.is_err());
		assert_eq!(
			result.unwrap_err().to_string(),
			"size must be a positive integer literal"
		);
	}

	#[rstest::rstest]
	fn test_validate_size_rejects_non_integer() {
		// Arrange
		let input = quote! {
			name: SearchForm,
			action: "/search",

			fields: {
				query: CharField {
					widget: SearchInput,
					size: "32",
				},
			},
		};

		// Act
		let result = parse_and_validate(input);

		// Assert
		assert!(result.is_err());
		assert_eq!(
			result.unwrap_err().to_string(),
			"size must be an integer literal"
		);
	}

	#[rstest::rstest]
	fn test_validate_form_with_mixed_file_and_text_fields() {
		// Arrange
		let input = quote! {
			name: ProfileForm,
			action: "/api/profile",

			fields: {
				name: CharField { required },
				avatar: ImageField {},
				resume: FileField {},
			},
		};

		// Act
		let result = parse_and_validate(input);

		// Assert
		assert!(result.is_ok());
		let typed = result.unwrap();
		assert_eq!(typed.fields.len(), 3);
		assert!(matches!(
			typed.fields[0].as_field().unwrap().field_type,
			TypedFieldType::CharField
		));
		assert!(matches!(
			typed.fields[1].as_field().unwrap().field_type,
			TypedFieldType::ImageField
		));
		assert!(matches!(
			typed.fields[2].as_field().unwrap().field_type,
			TypedFieldType::FileField
		));
	}

	#[rstest::rstest]
	fn test_model_form_rejects_widget_without_model_renderer() {
		let widget: syn::Ident = syn::parse_quote!(SelectMultiple);

		let error = parse_model_widget(&widget).unwrap_err();

		assert_eq!(
			error.to_string(),
			"this widget is not supported by model-backed forms; use a supported scalar widget or an explicit non-model form"
		);
	}

	#[rstest::rstest]
	fn test_model_form_rejects_implicit_redirects() {
		let input = quote! {
			name: QuestionForm,
			model: Question,
			policy: QuestionFields,
			fields: [title],
			server_fn: save_question,
			redirect_on_success: "/questions",
		};

		let error = parse_and_validate(input)
			.expect_err("model forms must reject redirects they cannot execute");

		assert!(
			error
				.to_string()
				.contains("does not support `redirect_on_success`")
		);
	}
}

#[cfg(test)]
mod scope_transform_tests {
	use super::*;
	use reinhardt_manouche::core::{ClientTrigger, ValidatorRule, ValidatorScope};
	use rstest::*;

	fn make_rule(scope: ValidatorScope) -> ValidatorRule {
		ValidatorRule {
			scope,
			expr: syn::parse_str("|v| v.len() > 0").unwrap(),
			message: syn::parse_str("\"error\"").unwrap(),
			span: proc_macro2::Span::call_site(),
		}
	}

	#[rstest]
	#[case(ValidatorScope::Both)]
	#[case(ValidatorScope::Server)]
	#[case(ValidatorScope::Client {
		trigger: ClientTrigger::Input,
	})]
	#[case(ValidatorScope::ServerAndClient {
		trigger: ClientTrigger::Blur,
	})]
	fn test_scope_propagated_through_transform(#[case] scope: ValidatorScope) {
		// Arrange
		let rule = make_rule(scope.clone());

		// Act
		let typed = transform_validator_rule(&rule).unwrap();

		// Assert
		assert_eq!(typed.scope, scope);
		assert_eq!(typed.message, "error");
	}
}
