//! Parser implementation for the form! macro AST.
//!
//! This module provides the `Parse` trait implementation for `FormMacro`,
//! allowing it to be parsed from a `TokenStream`.

use proc_macro2::{Span, TokenStream, TokenTree};
use syn::{
	Expr, ExprClosure, Ident, LitStr, Path, Result, Token, braced, bracketed,
	ext::IdentExt,
	parenthesized,
	parse::{Parse, ParseStream},
	token,
};

use crate::{
	AmbientArgumentsSource, ClientTrigger, CustomAttr, FormAction, FormChoiceGroup, FormChoiceItem,
	FormChoiceOption, FormControlEntryDef, FormControlEntryKind, FormCustomWidgetSpec,
	FormDatalistDef, FormDerived, FormDerivedItem, FormFieldCollection, FormFieldDef,
	FormFieldEntry, FormFieldGroup, FormFieldProperty, FormMacro, FormSlots, FormState,
	FormStateField, FormSubmitButtonDef, FormValidator, FormWatch, FormWatchItem, FormWidgetSpec,
	IconAttr, IconChild, IconElement, IconPosition, ModelFieldOverride, ModelFieldSelection,
	ModelFormSource, StripArgument, ValidatorRule, ValidatorScope, WrapperAttr, WrapperElement,
};

/// Parses a `form!` macro invocation into an untyped AST.
///
/// # Errors
///
/// Returns a `syn::Error` if the input is not valid form! syntax.
pub fn parse_form(input: TokenStream) -> syn::Result<FormMacro> {
	syn::parse2(input)
}

/// Detects whether the top-level form DSL uses `ambient_arguments` or the
/// deprecated `strip_arguments` alias.
#[must_use]
pub fn detect_ambient_arguments_source(input: &TokenStream) -> Option<AmbientArgumentsSource> {
	let mut tokens = input.clone().into_iter().peekable();

	while let Some(token) = tokens.next() {
		let TokenTree::Ident(ident) = token else {
			continue;
		};

		let source = match ident.to_string().as_str() {
			"ambient_arguments" => AmbientArgumentsSource::AmbientArguments,
			"strip_arguments" => AmbientArgumentsSource::StripArguments,
			_ => continue,
		};

		if matches!(tokens.peek(), Some(TokenTree::Punct(punct)) if punct.as_char() == ':') {
			return Some(source);
		}
	}

	None
}

/// Parses an optional generic-argument list of the form `<T1, T2, ...>` that may
/// appear immediately after a field-type identifier in the form! DSL.
///
/// - Returns `Ok(None)` when the next token is not `<`.
/// - Returns `Ok(Some(types))` for a non-empty list `<T1, ...>`.
/// - Returns `Err` for an empty `<>` (zero arguments), with a span at the
///   field-type identifier so the diagnostic points at the user's code.
fn parse_optional_field_type_generics(
	input: ParseStream,
	field_type_span: Span,
) -> Result<Option<syn::punctuated::Punctuated<syn::Type, Token![,]>>> {
	if !input.peek(Token![<]) {
		return Ok(None);
	}
	let _: Token![<] = input.parse()?;
	if input.peek(Token![>]) {
		return Err(syn::Error::new(
			field_type_span,
			"expected at least one type argument inside `<...>`",
		));
	}
	let args: syn::punctuated::Punctuated<syn::Type, Token![,]> =
		syn::punctuated::Punctuated::parse_separated_nonempty(input)?;
	let _: Token![>] = input.parse()?;
	Ok(Some(args))
}

impl Parse for FormMacro {
	fn parse(input: ParseStream) -> Result<Self> {
		let span = input.span();
		let mut form = FormMacro::new(None, span);
		let mut ambient_arguments_clause: Option<&'static str> = None;
		let mut model: Option<Path> = None;
		let mut model_selection: Option<ModelFieldSelection> = None;
		let mut overrides = Vec::new();
		let mut has_overrides = false;
		let mut has_explicit_fields = false;

		// Parse key-value pairs until we hit fields or validators
		while !input.is_empty() {
			let key: Ident = input.parse()?;
			input.parse::<Token![:]>()?;

			match key.to_string().as_str() {
				"name" => {
					form.name = Some(input.parse()?);
					parse_optional_comma(input)?;
				}
				"action" => {
					let url: LitStr = input.parse()?;
					form.action = FormAction::Url(url);
					parse_optional_comma(input)?;
				}
				"server_fn" => {
					let path: Path = input.parse()?;
					form.action = FormAction::ServerFn(path);
					parse_optional_comma(input)?;
				}
				"method" => {
					form.method = Some(input.parse()?);
					parse_optional_comma(input)?;
				}
				"class" => {
					form.class = Some(input.parse()?);
					parse_optional_comma(input)?;
				}
				"model" => {
					if model.is_some() {
						return Err(syn::Error::new(key.span(), "duplicate `model` property"));
					}
					model = Some(input.parse()?);
					parse_optional_comma(input)?;
				}
				"state" => {
					let content;
					braced!(content in input);
					form.state = Some(parse_form_state(&content)?);
					parse_optional_comma(input)?;
				}
				"on_submit" => {
					let closure: ExprClosure = input.parse()?;
					if form.callbacks.span.is_none() {
						form.callbacks.span = Some(key.span());
					}
					form.callbacks.on_submit = Some(closure);
					parse_optional_comma(input)?;
				}
				"on_success" => {
					let closure: ExprClosure = input.parse()?;
					if form.callbacks.span.is_none() {
						form.callbacks.span = Some(key.span());
					}
					form.callbacks.on_success = Some(closure);
					parse_optional_comma(input)?;
				}
				"on_success_ref" => {
					let closure: ExprClosure = input.parse()?;
					if form.callbacks.span.is_none() {
						form.callbacks.span = Some(key.span());
					}
					form.callbacks.on_success_ref = Some(closure);
					parse_optional_comma(input)?;
				}
				"on_error" => {
					let closure: ExprClosure = input.parse()?;
					if form.callbacks.span.is_none() {
						form.callbacks.span = Some(key.span());
					}
					form.callbacks.on_error = Some(closure);
					parse_optional_comma(input)?;
				}
				"on_loading" => {
					let closure: ExprClosure = input.parse()?;
					if form.callbacks.span.is_none() {
						form.callbacks.span = Some(key.span());
					}
					form.callbacks.on_loading = Some(closure);
					parse_optional_comma(input)?;
				}
				"watch" => {
					let content;
					braced!(content in input);
					form.watch = Some(parse_watch_block(&content, key.span())?);
					parse_optional_comma(input)?;
				}
				"derived" => {
					let content;
					braced!(content in input);
					form.derived = Some(parse_derived_block(&content, key.span())?);
					parse_optional_comma(input)?;
				}
				"redirect_on_success" => {
					form.redirect_on_success = Some(input.parse()?);
					parse_optional_comma(input)?;
				}
				"success_url" => {
					form.success_url = Some(input.parse()?);
					parse_optional_comma(input)?;
				}
				"initial_loader" => {
					form.initial_loader = Some(input.parse()?);
					parse_optional_comma(input)?;
				}
				"choices_loader" => {
					form.choices_loader = Some(input.parse()?);
					parse_optional_comma(input)?;
				}
				"slots" => {
					let content;
					braced!(content in input);
					form.slots = Some(parse_slots_block(&content, key.span())?);
					parse_optional_comma(input)?;
				}

				"fields" => {
					if input.peek(token::Bracket) {
						let content;
						bracketed!(content in input);
						set_model_selection(
							&mut model_selection,
							ModelFieldSelection::Fields(parse_model_field_list(&content)?),
							key.span(),
						)?;
					} else {
						let content;
						braced!(content in input);
						form.fields = parse_field_definitions(&content)?;
						has_explicit_fields = true;
					}
					parse_optional_comma(input)?;
				}
				"exclude" => {
					let content;
					bracketed!(content in input);
					set_model_selection(
						&mut model_selection,
						ModelFieldSelection::Exclude(parse_model_field_list(&content)?),
						key.span(),
					)?;
					parse_optional_comma(input)?;
				}
				"overrides" => {
					if has_overrides {
						return Err(syn::Error::new(
							key.span(),
							"duplicate `overrides` property",
						));
					}
					let content;
					braced!(content in input);
					overrides = parse_model_field_overrides(&content)?;
					has_overrides = true;
					parse_optional_comma(input)?;
				}
				"validators" => {
					let content;
					braced!(content in input);
					form.validators = parse_validators(&content)?;
					parse_optional_comma(input)?;
				}
				"client_validators" => {
					return Err(reject_client_validators_with_guidance(&key));
				}
				"ambient_arguments" => {
					let content;
					braced!(content in input);
					match ambient_arguments_clause {
						Some("ambient_arguments") => {
							return Err(syn::Error::new(
								key.span(),
								"duplicate `ambient_arguments` property",
							));
						}
						Some("strip_arguments") => {
							return Err(syn::Error::new(
								key.span(),
								"form! cannot specify both `ambient_arguments` and deprecated `strip_arguments`; use `ambient_arguments`",
							));
						}
						_ => {}
					}
					ambient_arguments_clause = Some("ambient_arguments");
					form.ambient_arguments_source = Some(AmbientArgumentsSource::AmbientArguments);
					form.strip_arguments = parse_ambient_arguments(&content)?;
					parse_optional_comma(input)?;
				}
				"strip_arguments" => {
					let content;
					braced!(content in input);
					match ambient_arguments_clause {
						Some("strip_arguments") => {
							return Err(syn::Error::new(
								key.span(),
								"duplicate `strip_arguments` property",
							));
						}
						Some("ambient_arguments") => {
							return Err(syn::Error::new(
								key.span(),
								"form! cannot specify both `ambient_arguments` and deprecated `strip_arguments`; use `ambient_arguments`",
							));
						}
						_ => {}
					}
					ambient_arguments_clause = Some("strip_arguments");
					form.ambient_arguments_source = Some(AmbientArgumentsSource::StripArguments);
					form.strip_arguments = parse_strip_arguments(&content)?;
					parse_optional_comma(input)?;
				}
				_ => {
					return Err(syn::Error::new(
						key.span(),
						format!(
							"Unknown form property: '{}'. Expected: name, action, server_fn, method, class, state, on_submit, on_success, on_success_ref, on_error, on_loading, watch, redirect_on_success, success_url, initial_loader, choices_loader, slots, fields, validators, derived, ambient_arguments, strip_arguments",
							key
						),
					));
				}
			}
		}

		match model {
			Some(model) => {
				if has_explicit_fields {
					return Err(syn::Error::new(
						span,
						"model-backed form! cannot use braced `fields`; use `fields: [...]` or `exclude: [...]`",
					));
				}
				let selection = model_selection.ok_or_else(|| {
					syn::Error::new(
						span,
						"model-backed form! requires exactly one of `fields: [...]` or `exclude: [...]`",
					)
				})?;
				form.model_source = Some(ModelFormSource {
					model,
					selection,
					overrides,
				});
			}
			None => {
				if model_selection.is_some() {
					return Err(syn::Error::new(
						span,
						"`fields: [...]` and `exclude` are only valid with `model`",
					));
				}
				if has_overrides {
					return Err(syn::Error::new(
						span,
						"`overrides` is only valid with `model`",
					));
				}
			}
		}

		// Validate required fields
		if form.name.is_none() {
			return Err(syn::Error::new(
				span,
				"form! macro requires 'name' property",
			));
		}

		Ok(form)
	}
}

fn set_model_selection(
	selection: &mut Option<ModelFieldSelection>,
	value: ModelFieldSelection,
	span: Span,
) -> Result<()> {
	if selection.is_some() {
		return Err(syn::Error::new(
			span,
			"model-backed form! requires exactly one of `fields: [...]` or `exclude: [...]`",
		));
	}
	*selection = Some(value);
	Ok(())
}

fn parse_model_field_list(input: ParseStream) -> Result<Vec<Ident>> {
	let fields = syn::punctuated::Punctuated::<Ident, Token![,]>::parse_terminated(input)?;
	Ok(fields.into_iter().collect())
}

fn parse_model_field_overrides(input: ParseStream) -> Result<Vec<ModelFieldOverride>> {
	let mut overrides = Vec::new();

	while !input.is_empty() {
		let field: Ident = input.parse()?;
		input.parse::<Token![:]>()?;
		let content;
		braced!(content in input);

		let mut widget = None;
		let mut label = None;
		let mut help_text = None;
		while !content.is_empty() {
			let property: Ident = content.parse()?;
			content.parse::<Token![:]>()?;
			match property.to_string().as_str() {
				"widget" => {
					if widget.is_some() {
						return Err(syn::Error::new(
							property.span(),
							"duplicate `widget` override",
						));
					}
					widget = Some(content.parse()?);
				}
				"label" => {
					if label.is_some() {
						return Err(syn::Error::new(
							property.span(),
							"duplicate `label` override",
						));
					}
					label = Some(content.parse()?);
				}
				"help_text" => {
					if help_text.is_some() {
						return Err(syn::Error::new(
							property.span(),
							"duplicate `help_text` override",
						));
					}
					help_text = Some(content.parse()?);
				}
				_ => {
					return Err(syn::Error::new(
						property.span(),
						format!(
							"unknown `overrides` property: `{property}`; expected widget, label, or help_text"
						),
					));
				}
			}
			parse_optional_comma(&content)?;
		}

		overrides.push(ModelFieldOverride {
			field,
			widget,
			label,
			help_text,
		});
		parse_optional_comma(input)?;
	}

	Ok(overrides)
}

/// Parses an optional trailing comma.
fn parse_optional_comma(input: ParseStream) -> Result<()> {
	if input.peek(Token![,]) {
		input.parse::<Token![,]>()?;
	}
	Ok(())
}

/// Parses field definitions inside the `fields: { ... }` block.
///
/// Supports regular fields, field groups, field collections, and submit buttons:
/// - Regular field: `username: CharField { required, ... }`
/// - Field group: `address: FieldGroup { label: "...", fields: { ... } }`
/// - Field collection: `items: FieldArray { fields: { ... } }`
fn parse_field_definitions(input: ParseStream) -> Result<Vec<FormFieldEntry>> {
	let mut entries = Vec::new();

	while !input.is_empty() {
		let span = input.span();
		let name: Ident = input.parse()?;
		input.parse::<Token![:]>()?;
		let field_type: Ident = input.parse()?;

		if field_type == "FieldGroup" {
			// Parse field group: FieldGroup { label: "...", class: "...", fields: { ... } }
			let content;
			braced!(content in input);
			let group = parse_field_group(name, &content, span)?;
			entries.push(FormFieldEntry::Group(group));
		} else if field_type == "FieldArray" {
			let content;
			braced!(content in input);
			let collection = parse_field_array(name, &content, span)?;
			entries.push(FormFieldEntry::Collection(Box::new(collection)));
		} else if field_type == "SubmitButton" {
			// Parse submit button: SubmitButton { label: "Sign in", class: "btn-primary" }
			let properties = if input.peek(token::Brace) {
				let content;
				braced!(content in input);
				parse_field_properties(&content)?
			} else {
				Vec::new()
			};

			entries.push(FormFieldEntry::SubmitButton(FormSubmitButtonDef {
				name,
				properties,
				span,
			}));
		} else if field_type == "Datalist" {
			let generics = parse_optional_field_type_generics(input, field_type.span())?;
			if generics.is_some() {
				return Err(syn::Error::new(
					field_type.span(),
					"Datalist does not accept generic arguments",
				));
			}

			let properties = if input.peek(token::Brace) {
				let content;
				braced!(content in input);
				parse_field_properties(&content)?
			} else {
				Vec::new()
			};

			entries.push(FormFieldEntry::Datalist(FormDatalistDef {
				name,
				properties,
				span,
			}));
		} else if let Some(kind) = parse_control_entry_kind(&field_type) {
			let generics = parse_optional_field_type_generics(input, field_type.span())?;
			if generics.is_some() {
				return Err(syn::Error::new(
					field_type.span(),
					format!("{field_type} does not accept generic arguments"),
				));
			}

			let properties = if input.peek(token::Brace) {
				let content;
				braced!(content in input);
				parse_field_properties(&content)?
			} else {
				Vec::new()
			};

			entries.push(control_entry_from_kind(name, kind, properties, span));
		} else {
			// Optionally consume `<T1, T2, ...>` generic arguments before the
			// property block. Only regular fields support generics; FieldGroup
			// and non-value entries are handled in the branches above.
			let generics = parse_optional_field_type_generics(input, field_type.span())?;

			// Parse regular field properties in braces: { required, max_length: 100, ... }
			let properties = if input.peek(token::Brace) {
				let content;
				braced!(content in input);
				parse_field_properties(&content)?
			} else {
				Vec::new()
			};

			entries.push(FormFieldEntry::Field(FormFieldDef {
				name,
				field_type,
				generics,
				properties,
				span,
			}));
		}

		parse_optional_comma(input)?;
	}

	Ok(entries)
}

fn parse_control_entry_kind(ident: &Ident) -> Option<FormControlEntryKind> {
	match ident.to_string().as_str() {
		"ResetButton" => Some(FormControlEntryKind::ResetButton),
		"Button" => Some(FormControlEntryKind::Button),
		"ImageInput" => Some(FormControlEntryKind::ImageInput),
		"Output" => Some(FormControlEntryKind::Output),
		"Meter" => Some(FormControlEntryKind::Meter),
		"Progress" => Some(FormControlEntryKind::Progress),
		_ => None,
	}
}

fn control_entry_from_kind(
	name: Ident,
	kind: FormControlEntryKind,
	properties: Vec<FormFieldProperty>,
	span: Span,
) -> FormFieldEntry {
	let def = FormControlEntryDef {
		name,
		kind,
		properties,
		span,
	};

	match kind {
		FormControlEntryKind::ResetButton => FormFieldEntry::ResetButton(def),
		FormControlEntryKind::Button => FormFieldEntry::Button(def),
		FormControlEntryKind::ImageInput => FormFieldEntry::ImageInput(def),
		FormControlEntryKind::Output => FormFieldEntry::Output(def),
		FormControlEntryKind::Meter => FormFieldEntry::Meter(def),
		FormControlEntryKind::Progress => FormFieldEntry::Progress(def),
	}
}

/// Parses a field group definition.
///
/// Field groups have optional label, class, and a required fields block.
fn parse_field_group(name: Ident, input: ParseStream, span: Span) -> Result<FormFieldGroup> {
	let mut label = None;
	let mut class = None;
	let mut fields = Vec::new();

	while !input.is_empty() {
		let key: Ident = input.parse()?;
		input.parse::<Token![:]>()?;

		match key.to_string().as_str() {
			"label" => {
				label = Some(input.parse()?);
			}
			"class" => {
				class = Some(input.parse()?);
			}
			"fields" => {
				let content;
				braced!(content in input);
				fields = parse_group_fields(&content)?;
			}
			_ => {
				return Err(syn::Error::new(
					key.span(),
					format!(
						"Unknown FieldGroup property: '{}'. Expected: label, class, fields",
						key
					),
				));
			}
		}

		parse_optional_comma(input)?;
	}

	Ok(FormFieldGroup {
		name,
		label,
		class,
		fields,
		span,
	})
}

/// Parses entries inside a field group (no nested groups allowed).
fn parse_field_array(name: Ident, input: ParseStream, span: Span) -> Result<FormFieldCollection> {
	let mut label = None;
	let mut class = None;
	let mut min_items = None;
	let mut max_items = None;
	let mut initial_from = None;
	let mut fields = Vec::new();
	let mut render_item = None;

	while !input.is_empty() {
		let key: Ident = input.parse()?;
		input.parse::<Token![:]>()?;

		match key.to_string().as_str() {
			"label" => label = Some(input.parse()?),
			"class" => class = Some(input.parse()?),
			"min_items" => min_items = Some(input.parse()?),
			"max_items" => max_items = Some(input.parse()?),
			"initial_from" => initial_from = Some(input.parse()?),
			"fields" => {
				let content;
				braced!(content in input);
				fields = parse_field_definitions(&content)?;
			}
			"render_item" => render_item = Some(input.parse()?),
			_ => {
				return Err(syn::Error::new(
					key.span(),
					format!(
						"Unknown FieldArray property: '{}'. Expected: label, class, min_items, max_items, initial_from, fields, render_item",
						key
					),
				));
			}
		}

		parse_optional_comma(input)?;
	}

	Ok(FormFieldCollection {
		name,
		label,
		class,
		min_items,
		max_items,
		initial_from,
		fields,
		render_item,
		span,
	})
}

/// Parses entries inside a field group (no nested groups allowed).
fn parse_group_fields(input: ParseStream) -> Result<Vec<FormFieldEntry>> {
	let entries = parse_field_definitions(input)?;
	for entry in &entries {
		if matches!(entry, FormFieldEntry::Group(_)) {
			return Err(syn::Error::new(
				entry.span(),
				"nested field groups are not allowed",
			));
		}
	}
	Ok(entries)
}

/// Parses field properties inside braces.
fn parse_field_properties(input: ParseStream) -> Result<Vec<FormFieldProperty>> {
	let mut properties = Vec::new();

	while !input.is_empty() {
		let span = input.span();

		// Check for widget keyword
		if input.peek(Ident::peek_any) {
			let name: Ident = input.call(Ident::parse_any)?;

			if name == "widget" {
				// widget: WidgetType
				input.parse::<Token![:]>()?;
				let widget_type: Ident = input.parse()?;
				let widget = if widget_type == "CustomWidget" {
					FormWidgetSpec::Custom(parse_custom_widget_spec(input, span)?)
				} else {
					FormWidgetSpec::Builtin(widget_type)
				};
				properties.push(FormFieldProperty::Widget { widget, span });
			} else if name == "wrapper" {
				// wrapper: element { attr: value, ... }
				input.parse::<Token![:]>()?;
				let tag: Ident = input.parse()?;

				// Parse optional attributes in braces
				let attrs = if input.peek(token::Brace) {
					let content;
					braced!(content in input);
					parse_wrapper_attrs(&content)?
				} else {
					Vec::new()
				};

				properties.push(FormFieldProperty::Wrapper {
					element: WrapperElement { tag, attrs, span },
					span,
				});
			} else if name == "icon" {
				// icon: svg { attrs..., children... }
				input.parse::<Token![:]>()?;
				let svg_tag: Ident = input.parse()?;

				if svg_tag != "svg" {
					return Err(syn::Error::new(
						svg_tag.span(),
						"icon must be an svg element",
					));
				}

				// Parse SVG content in braces
				let content;
				braced!(content in input);
				let (attrs, children) = parse_icon_content(&content)?;

				properties.push(FormFieldProperty::Icon {
					element: IconElement {
						attrs,
						children,
						span,
					},
					span,
				});
			} else if name == "icon_position" {
				// icon_position: "left" | "right" | "label"
				input.parse::<Token![:]>()?;
				let position_lit: LitStr = input.parse()?;
				let position_str = position_lit.value();

				let position = position_str.parse::<IconPosition>().ok().ok_or_else(|| {
					syn::Error::new(
						position_lit.span(),
						format!(
							"invalid icon_position '{}', must be 'left', 'right', or 'label'",
							position_str
						),
					)
				})?;

				properties.push(FormFieldProperty::IconPosition { position, span });
			} else if name == "attrs" {
				// attrs: { aria_label: "...", data_testid: "..." }
				input.parse::<Token![:]>()?;
				let content;
				braced!(content in input);
				let attrs = parse_custom_attrs(&content)?;
				properties.push(FormFieldProperty::Attrs { attrs, span });
			} else if name == "bind" {
				// bind: true or bind: false
				input.parse::<Token![:]>()?;
				let value: syn::LitBool = input.parse()?;
				properties.push(FormFieldProperty::Bind {
					enabled: value.value(),
					span,
				});
			} else if name == "initial_from" {
				// initial_from: "field_name" - specifies source field from initial_loader result
				input.parse::<Token![:]>()?;
				let field_name: LitStr = input.parse()?;
				properties.push(FormFieldProperty::InitialFrom { field_name, span });
			} else if name == "choices_from" {
				// choices_from: "choices" - specifies field in choices_loader result containing choice array
				input.parse::<Token![:]>()?;
				let field_name: LitStr = input.parse()?;
				properties.push(FormFieldProperty::ChoicesFrom { field_name, span });
			} else if name == "choices" || name == "options" {
				input.parse::<Token![:]>()?;
				let content;
				bracketed!(content in input);
				let choices = parse_choice_items(&content)?;
				properties.push(FormFieldProperty::Choices { choices, span });
			} else if name == "choice_value" {
				// choice_value: "id" - specifies property path for value extraction from each choice
				input.parse::<Token![:]>()?;
				let path: LitStr = input.parse()?;
				properties.push(FormFieldProperty::ChoiceValue { path, span });
			} else if name == "choice_label" {
				// choice_label: "choice_text" - specifies property path for label extraction from each choice
				input.parse::<Token![:]>()?;
				let path: LitStr = input.parse()?;
				properties.push(FormFieldProperty::ChoiceLabel { path, span });
			} else if name == "choice_disabled" {
				input.parse::<Token![:]>()?;
				let path: LitStr = input.parse()?;
				properties.push(FormFieldProperty::ChoiceDisabled { path, span });
			} else if name == "choice_group" {
				input.parse::<Token![:]>()?;
				let path: LitStr = input.parse()?;
				properties.push(FormFieldProperty::ChoiceGroup { path, span });
			} else if name == "choice_group_disabled" {
				input.parse::<Token![:]>()?;
				let path: LitStr = input.parse()?;
				properties.push(FormFieldProperty::ChoiceGroupDisabled { path, span });
			} else if input.peek(Token![:]) {
				// name: value
				input.parse::<Token![:]>()?;
				let value: Expr = input.parse()?;
				properties.push(FormFieldProperty::Named { name, value, span });
			} else {
				// Flag property (just identifier, no value)
				properties.push(FormFieldProperty::Flag { name, span });
			}
		}

		parse_optional_comma(input)?;
	}

	Ok(properties)
}

fn parse_custom_widget_spec(input: ParseStream, span: Span) -> Result<FormCustomWidgetSpec> {
	let component_content;
	parenthesized!(component_content in input);
	let component: Path = component_content.parse()?;
	if !component_content.is_empty() {
		return Err(syn::Error::new(
			component_content.span(),
			"expected a single CustomWidget component path",
		));
	}

	if !input.peek(token::Brace) {
		return Err(syn::Error::new(
			span,
			"CustomWidget requires a property body",
		));
	}

	let body;
	braced!(body in input);
	let mut experimental = false;
	let mut adapter = None;

	while !body.is_empty() {
		let property_span = body.span();
		let property: Ident = body.call(Ident::parse_any)?;

		if property == "experimental" {
			experimental = true;
		} else if property == "adapter" {
			body.parse::<Token![:]>()?;
			adapter = Some(body.parse()?);
		} else {
			return Err(syn::Error::new(
				property_span,
				"unknown CustomWidget property",
			));
		}

		parse_optional_comma(&body)?;
	}

	Ok(FormCustomWidgetSpec {
		component,
		experimental,
		adapter,
		span,
	})
}

fn parse_choice_items(input: ParseStream) -> Result<Vec<FormChoiceItem>> {
	let mut choices = Vec::new();

	while !input.is_empty() {
		choices.push(parse_choice_item(input)?);
		parse_optional_comma(input)?;
	}

	Ok(choices)
}

fn parse_choice_item(input: ParseStream) -> Result<FormChoiceItem> {
	if input.peek(Ident) {
		let fork = input.fork();
		let ident: Ident = fork.call(Ident::parse_any)?;
		if ident == "OptGroup" {
			return parse_choice_group(input).map(FormChoiceItem::Group);
		}
	}

	parse_choice_option(input).map(|option| FormChoiceItem::Option(Box::new(option)))
}

fn parse_choice_option(input: ParseStream) -> Result<FormChoiceOption> {
	let span = input.span();
	let content;
	parenthesized!(content in input);

	let value: Expr = content.parse()?;
	let label = if content.peek(Token![,]) {
		content.parse::<Token![,]>()?;
		if content.is_empty() {
			None
		} else {
			Some(content.parse()?)
		}
	} else {
		None
	};

	if !content.is_empty() {
		return Err(content.error("choice option tuple accepts one or two expressions"));
	}

	let disabled = parse_choice_disabled_block(input)?;

	Ok(FormChoiceOption {
		value,
		label,
		disabled,
		span,
	})
}

fn parse_choice_group(input: ParseStream) -> Result<FormChoiceGroup> {
	let span = input.span();
	let ident: Ident = input.call(Ident::parse_any)?;
	if ident != "OptGroup" {
		return Err(syn::Error::new(ident.span(), "expected OptGroup"));
	}

	let label_content;
	parenthesized!(label_content in input);
	let label: LitStr = label_content.parse()?;
	if !label_content.is_empty() {
		return Err(label_content.error("OptGroup accepts exactly one string label"));
	}

	let content;
	braced!(content in input);

	let mut disabled = false;
	let mut options = Vec::new();

	while !content.is_empty() {
		if content.peek(Ident) {
			let fork = content.fork();
			let flag: Ident = fork.call(Ident::parse_any)?;
			if flag == "disabled" {
				let flag: Ident = content.call(Ident::parse_any)?;
				if flag != "disabled" {
					return Err(syn::Error::new(flag.span(), "expected disabled"));
				}
				disabled = true;
				parse_optional_comma(&content)?;
				continue;
			}
		}

		options.push(parse_choice_item(&content)?);
		parse_optional_comma(&content)?;
	}

	Ok(FormChoiceGroup {
		label,
		disabled,
		options,
		span,
	})
}

fn parse_choice_disabled_block(input: ParseStream) -> Result<bool> {
	if !input.peek(token::Brace) {
		return Ok(false);
	}

	let content;
	braced!(content in input);
	let mut disabled = false;

	while !content.is_empty() {
		let flag: Ident = content.call(Ident::parse_any)?;
		if flag != "disabled" {
			return Err(syn::Error::new(flag.span(), "unknown choice option flag"));
		}
		disabled = true;
		parse_optional_comma(&content)?;
	}

	Ok(disabled)
}

/// Parses custom attributes for aria-* and data-*.
fn parse_custom_attrs(input: ParseStream) -> Result<Vec<CustomAttr>> {
	let mut attrs = Vec::new();

	while !input.is_empty() {
		let span = input.span();
		let name: Ident = input.parse()?;
		input.parse::<Token![:]>()?;
		let value: Expr = input.parse()?;

		attrs.push(CustomAttr { name, value, span });

		parse_optional_comma(input)?;
	}

	Ok(attrs)
}

/// Parses wrapper element attributes.
fn parse_wrapper_attrs(input: ParseStream) -> Result<Vec<WrapperAttr>> {
	let mut attrs = Vec::new();

	while !input.is_empty() {
		let span = input.span();
		let name: Ident = input.parse()?;
		input.parse::<Token![:]>()?;
		let value: Expr = input.parse()?;

		attrs.push(WrapperAttr { name, value, span });
		parse_optional_comma(input)?;
	}

	Ok(attrs)
}

/// Parses SVG icon content (attributes and child elements).
///
/// The content can contain:
/// - Attributes: `class: "...", viewBox: "..."`
/// - Child elements: `path { d: "..." }, circle { cx: 12, cy: 12, r: 5 }`
fn parse_icon_content(input: ParseStream) -> Result<(Vec<IconAttr>, Vec<IconChild>)> {
	let mut attrs = Vec::new();
	let mut children = Vec::new();

	while !input.is_empty() {
		let span = input.span();
		let name: Ident = input.parse()?;

		if input.peek(token::Brace) {
			// Child element: path { d: "..." }
			let content;
			braced!(content in input);
			let child = parse_icon_child(name, &content, span)?;
			children.push(child);
		} else if input.peek(Token![:]) {
			// Attribute: class: "..."
			input.parse::<Token![:]>()?;
			let value: Expr = input.parse()?;
			attrs.push(IconAttr { name, value, span });
		} else {
			return Err(syn::Error::new(
				span,
				"expected ':' for attribute or '{' for child element",
			));
		}

		parse_optional_comma(input)?;
	}

	Ok((attrs, children))
}

/// Parses a single SVG child element (path, circle, rect, g, etc.)
fn parse_icon_child(tag: Ident, input: ParseStream, span: Span) -> Result<IconChild> {
	let mut attrs = Vec::new();
	let mut children = Vec::new();

	while !input.is_empty() {
		let attr_span = input.span();
		let name: Ident = input.parse()?;

		if input.peek(token::Brace) {
			// Nested child element (for g, defs, etc.)
			let content;
			braced!(content in input);
			let child = parse_icon_child(name, &content, attr_span)?;
			children.push(child);
		} else if input.peek(Token![:]) {
			// Attribute
			input.parse::<Token![:]>()?;
			let value: Expr = input.parse()?;
			attrs.push(IconAttr {
				name,
				value,
				span: attr_span,
			});
		} else {
			return Err(syn::Error::new(
				attr_span,
				"expected ':' for attribute or '{' for child element",
			));
		}

		parse_optional_comma(input)?;
	}

	Ok(IconChild {
		tag,
		attrs,
		children,
		span,
	})
}

/// Parses ambient arguments inside the `ambient_arguments: { ... }` block.
///
/// Format: `arg_name: <expression>,` repeated. Each entry binds one
/// server_fn argument name to an expression evaluated at submit time on the
/// client; the resulting value is appended positionally to the server_fn
/// call after all regular form-field arguments.
///
/// Tracked under reinhardt-web#3971.
fn parse_ambient_arguments(input: ParseStream) -> Result<Vec<StripArgument>> {
	parse_strip_arguments(input)
}

/// Parses legacy strip_arguments inside the `strip_arguments: { ... }` block.
///
/// `strip_arguments` is a deprecated alias for `ambient_arguments`.
fn parse_strip_arguments(input: ParseStream) -> Result<Vec<StripArgument>> {
	let mut args = Vec::new();

	while !input.is_empty() {
		let span = input.span();
		let name: Ident = input.parse()?;
		input.parse::<Token![:]>()?;
		let value: Expr = input.parse()?;
		args.push(StripArgument { name, value, span });
		parse_optional_comma(input)?;
	}

	Ok(args)
}

/// Parses server-side validators inside the `validators: { ... }` block.
fn parse_validators(input: ParseStream) -> Result<Vec<FormValidator>> {
	let mut validators = Vec::new();

	while !input.is_empty() {
		let span = input.span();

		// Check for @form marker for form-level validators
		if input.peek(Token![@]) {
			input.parse::<Token![@]>()?;
			let form_ident: Ident = input.parse()?;
			if form_ident != "form" {
				return Err(syn::Error::new(
					form_ident.span(),
					"Expected '@form' for form-level validator",
				));
			}
			input.parse::<Token![:]>()?;

			// Parse rules array
			let rules = parse_validator_rules(input)?;
			validators.push(FormValidator::Form { rules, span });
		} else {
			// Field-level validator
			let field_name: Ident = input.parse()?;
			input.parse::<Token![:]>()?;

			// Parse rules array
			let rules = parse_validator_rules(input)?;
			validators.push(FormValidator::Field {
				field_name,
				rules,
				span,
			});
		}

		parse_optional_comma(input)?;
	}

	Ok(validators)
}

/// Parses optional scope annotations before a validator rule.
///
/// Supported annotations:
/// - `#[server]` — server-side only
/// - `#[client(on = submit|input|blur)]` — client-side only with trigger
///
/// Multiple annotations combine: `#[server]` + `#[client(on = blur)]` =
/// `ServerAndClient { trigger: Blur }`. No annotation = `Both` (server + client submit).
fn parse_validator_scope(input: ParseStream) -> Result<ValidatorScope> {
	let mut has_server = false;
	let mut client_trigger: Option<ClientTrigger> = None;

	while input.peek(Token![#]) {
		input.parse::<Token![#]>()?;

		let attr_content;
		syn::bracketed!(attr_content in input);

		let attr_name: Ident = attr_content.parse()?;
		match attr_name.to_string().as_str() {
			"server" => {
				if has_server {
					return Err(syn::Error::new(
						attr_name.span(),
						"duplicate `#[server]` annotation",
					));
				}
				has_server = true;
			}
			"client" => {
				if client_trigger.is_some() {
					return Err(syn::Error::new(
						attr_name.span(),
						"duplicate `#[client(...)]` annotation",
					));
				}

				let inner;
				syn::parenthesized!(inner in attr_content);

				let on_ident: Ident = inner.parse()?;
				if on_ident != "on" {
					return Err(syn::Error::new(
						on_ident.span(),
						format!("expected `on`, found `{}`", on_ident),
					));
				}

				inner.parse::<Token![=]>()?;

				let trigger_ident: Ident = inner.parse()?;
				let trigger = match trigger_ident.to_string().as_str() {
					"submit" => ClientTrigger::Submit,
					"input" => ClientTrigger::Input,
					"blur" => ClientTrigger::Blur,
					other => {
						return Err(syn::Error::new(
							trigger_ident.span(),
							format!(
								"unknown trigger `{}`, expected one of: submit, input, blur",
								other
							),
						));
					}
				};

				client_trigger = Some(trigger);
			}
			other => {
				return Err(syn::Error::new(
					attr_name.span(),
					format!(
						"unknown validator attribute `{}`, expected `server` or `client`",
						other
					),
				));
			}
		}
	}

	Ok(match (has_server, client_trigger) {
		(false, None) => ValidatorScope::Both,
		(true, None) => ValidatorScope::Server,
		(false, Some(trigger)) => ValidatorScope::Client { trigger },
		(true, Some(trigger)) => ValidatorScope::ServerAndClient { trigger },
	})
}

/// Parses validator rules: [ #[scope]? |v| condition => "message", ... ]
fn parse_validator_rules(input: ParseStream) -> Result<Vec<ValidatorRule>> {
	let content;
	syn::bracketed!(content in input);

	let mut rules = Vec::new();

	while !content.is_empty() {
		let span = content.span();

		// Parse optional scope annotations
		let scope = parse_validator_scope(&content)?;

		// Parse closure: |v| condition
		let expr: ExprClosure = content.parse()?;

		// Parse arrow
		content.parse::<Token![=>]>()?;

		// Parse error message
		let message: LitStr = content.parse()?;

		rules.push(ValidatorRule {
			scope,
			expr,
			message,
			span,
		});

		parse_optional_comma(&content)?;
	}

	Ok(rules)
}

/// Rejects the legacy `client_validators: { ... }` block with a customized
/// compile error pointing users to the unified `validators:` block syntax.
///
/// The feature was never wired to code generation in any released version;
/// no runtime behavior is lost by this error. The identifier is retained
/// through 0.1.x so users upgrading from earlier RCs see actionable guidance
/// rather than a generic `unknown property` error. It will be removed from
/// the parser entirely in 0.2.0.
///
/// Tracked by: kent8192/reinhardt-web#3654
fn reject_client_validators_with_guidance(ident: &Ident) -> syn::Error {
	syn::Error::new(
		ident.span(),
		"\
`client_validators:` was never wired to code generation in any released version \
of reinhardt-web; removing it loses no runtime behavior.

Migrate to the unified `validators:` block with `#[client(on = submit|input|blur)]` \
annotations:

    // Before (no longer supported):
    client_validators: {
        username: [ \"value.length >= 3\" => \"Too short\" ],
    }

    // After:
    validators: {
        username: [
            #[client(on = input)]
            |v| v.len() >= 3 => \"Too short\",
        ],
    }

See: docs/migration/0.2.0-client-validators-removal.md

This custom error is retained through the 0.1.x series. In 0.2.0, the \
`client_validators` identifier will be removed from the parser and using it \
will produce a generic `unknown property` error instead.

Tracked by: kent8192/reinhardt-web#3654\
",
	)
}

/// Parses the form state configuration: `state: { loading, error, success }`.
///
/// The state block contains field names that indicate which UI state signals to enable.
/// Valid field names are: `loading`, `error`, `success`.
fn parse_form_state(input: ParseStream) -> Result<FormState> {
	let span = input.span();
	let mut state = FormState::new(span);

	while !input.is_empty() {
		let field_span = input.span();
		let field_name: Ident = input.parse()?;

		state.fields.push(FormStateField {
			name: field_name,
			span: field_span,
		});

		parse_optional_comma(input)?;
	}

	Ok(state)
}

/// Parses a watch block containing named closures.
///
/// Expected syntax:
/// ```text
/// watch: {
///     error_display: |form| {
///         if form.error().get().is_some() {
///             div { class: "error", form.error().get().unwrap() }
///         }
///     },
///     loading_indicator: |form| {
///         if *form.loading().get() {
///             div { class: "spinner" }
///         }
///     },
/// }
/// ```
fn parse_watch_block(input: ParseStream, span: Span) -> Result<FormWatch> {
	let mut watch = FormWatch::new(span);

	while !input.is_empty() {
		let item_span = input.span();
		let name: Ident = input.parse()?;
		input.parse::<Token![:]>()?;
		let closure: ExprClosure = input.parse()?;

		watch.items.push(FormWatchItem {
			name,
			closure,
			span: item_span,
		});

		parse_optional_comma(input)?;
	}

	Ok(watch)
}

/// Parses a derived block containing named closures for computed values.
///
/// Expected syntax:
/// ```text
/// derived: {
///     char_count: |form| form.content().get().len(),
///     is_over_limit: |form| form.char_count().get() > 280,
///     progress_percent: |form| {
///         (form.char_count().get() as f32 / 280.0 * 100.0).min(100.0)
///     },
/// }
/// ```
///
/// Unlike watch blocks which return View types, derived blocks return
/// arbitrary values that are wrapped in `Memo<T>`.
fn parse_derived_block(input: ParseStream, span: Span) -> Result<FormDerived> {
	let mut derived = FormDerived::new(span);

	while !input.is_empty() {
		let item_span = input.span();
		let name: Ident = input.parse()?;
		input.parse::<Token![:]>()?;
		let closure: ExprClosure = input.parse()?;

		derived.items.push(FormDerivedItem {
			name,
			closure,
			span: item_span,
		});

		parse_optional_comma(input)?;
	}

	Ok(derived)
}

/// Parses a slots block containing optional before_fields and after_fields closures.
///
/// Expected syntax:
/// ```text
/// slots: {
///     before_fields: || {
///         div { class: "form-header", "Please login" }
///     },
///     after_fields: || {
///         button { type: "submit", "Submit" }
///     },
/// }
/// ```
fn parse_slots_block(input: ParseStream, span: Span) -> Result<FormSlots> {
	let mut slots = FormSlots {
		before_fields: None,
		after_fields: None,
		span,
	};

	while !input.is_empty() {
		let name: Ident = input.parse()?;
		input.parse::<Token![:]>()?;
		let closure: ExprClosure = input.parse()?;

		match name.to_string().as_str() {
			"before_fields" => {
				if slots.before_fields.is_some() {
					return Err(syn::Error::new(
						name.span(),
						"'before_fields' is already defined",
					));
				}
				slots.before_fields = Some(closure);
			}
			"after_fields" => {
				if slots.after_fields.is_some() {
					return Err(syn::Error::new(
						name.span(),
						"'after_fields' is already defined",
					));
				}
				slots.after_fields = Some(closure);
			}
			_ => {
				return Err(syn::Error::new(
					name.span(),
					format!(
						"Unknown slot: '{}'. Expected: before_fields, after_fields",
						name
					),
				));
			}
		}

		parse_optional_comma(input)?;
	}

	Ok(slots)
}

#[cfg(test)]
mod tests {
	use super::*;
	use quote::quote;
	use rstest::rstest;

	#[rstest]
	fn test_parse_simple_form() {
		// Arrange
		let input = quote! {
			name: LoginForm,
			action: "/api/login",

			fields: {
				username: CharField { required, max_length: 150 },
				password: CharField { required, widget: PasswordInput },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert_eq!(form.name.as_ref().unwrap().to_string(), "LoginForm");
		assert_eq!(form.fields.len(), 2);
		assert!(matches!(form.action, FormAction::Url(_)));
	}

	#[rstest]
	fn test_parse_server_fn_action() {
		// Arrange
		let input = quote! {
			name: VoteForm,
			server_fn: submit_vote,

			fields: {
				choice_id: IntegerField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(matches!(form.action, FormAction::ServerFn(_)));
	}

	#[rstest]
	fn test_parse_model_form_with_selected_fields_and_overrides() {
		// Arrange
		let input = quote! {
			name: QuestionForm,
			model: Question,
			fields: [title, published_at],
			server_fn: save_question,
			overrides: {
				title: {
					widget: TextArea,
					label: "Question",
					help_text: "Enter the question",
				},
			},
		};

		// Act
		let form: FormMacro = syn::parse2(input).expect("model form should parse");

		// Assert
		assert!(form.fields.is_empty());
		let source = form
			.model_source
			.expect("model source should be recorded for model forms");
		assert_eq!(source.model.segments.last().unwrap().ident, "Question");
		let ModelFieldSelection::Fields(fields) = source.selection else {
			panic!("model form should select explicit fields");
		};
		assert_eq!(
			fields.iter().map(ToString::to_string).collect::<Vec<_>>(),
			["title", "published_at"]
		);
		assert_eq!(source.overrides.len(), 1);
		let override_ = &source.overrides[0];
		assert_eq!(override_.field, "title");
		assert_eq!(override_.widget.as_ref().unwrap(), "TextArea");
		assert_eq!(override_.label.as_ref().unwrap().value(), "Question");
		assert_eq!(
			override_.help_text.as_ref().unwrap().value(),
			"Enter the question"
		);
	}

	#[rstest]
	fn test_parse_model_form_with_excluded_fields() {
		// Arrange
		let input = quote! {
			name: QuestionForm,
			model: Question,
			exclude: [owner_id],
			server_fn: save_question,
		};

		// Act
		let form: FormMacro = syn::parse2(input).expect("model form should parse");

		// Assert
		assert!(form.fields.is_empty());
		let source = form
			.model_source
			.expect("model source should be recorded for model forms");
		let ModelFieldSelection::Exclude(fields) = source.selection else {
			panic!("model form should exclude fields");
		};
		assert_eq!(
			fields.iter().map(ToString::to_string).collect::<Vec<_>>(),
			["owner_id"]
		);
	}

	#[rstest]
	fn test_parse_missing_name() {
		// Arrange
		let input = quote! {
			action: "/api/test",

			fields: {
				field1: CharField {},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_err());
		assert!(result.unwrap_err().to_string().contains("name"));
	}

	#[rstest]
	fn test_parse_state_block() {
		// Arrange
		let input = quote! {
			name: ProfileForm,
			server_fn: update_profile,

			state: { loading, error, success },

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.state.is_some());
		let state = form.state.unwrap();
		assert!(state.has_loading());
		assert!(state.has_error());
		assert!(state.has_success());
	}

	#[rstest]
	fn test_parse_state_single_field() {
		// Arrange
		let input = quote! {
			name: SimpleForm,
			action: "/api/submit",

			state: { loading },

			fields: {
				data: CharField {},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let state = form.state.unwrap();
		assert!(state.has_loading());
		assert!(!state.has_error());
		assert!(!state.has_success());
	}

	#[rstest]
	fn test_parse_state_empty() {
		// Arrange
		let input = quote! {
			name: EmptyStateForm,
			action: "/api/test",

			state: {},

			fields: {
				field1: CharField {},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let state = form.state.unwrap();
		assert!(state.is_empty());
		assert!(!state.has_loading());
	}

	#[rstest]
	fn test_parse_form_without_state() {
		// Arrange
		let input = quote! {
			name: NoStateForm,
			action: "/api/test",

			fields: {
				field1: CharField {},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.state.is_none());
	}

	#[rstest]
	fn test_parse_callbacks_all() {
		// Arrange
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

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.callbacks.has_any());
		assert!(form.callbacks.on_submit.is_some());
		assert!(form.callbacks.on_success.is_some());
		assert!(form.callbacks.on_error.is_some());
		assert!(form.callbacks.on_loading.is_some());
	}

	#[rstest]
	fn test_parse_callbacks_single() {
		// Arrange
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

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.callbacks.has_any());
		assert!(form.callbacks.on_submit.is_none());
		assert!(form.callbacks.on_success.is_some());
		assert!(form.callbacks.on_error.is_none());
		assert!(form.callbacks.on_loading.is_none());
	}

	#[rstest]
	fn test_parse_on_success_ref() {
		// Arrange — issue #4624: the lifted `on_success_ref:` keyword.
		let input = quote! {
			name: OnSuccessRefForm,
			server_fn: update_profile,

			on_success_ref: |_form, _result| {
				let _ = _result;
			},

			fields: {
				data: CharField {},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok(), "form! with on_success_ref should parse");
		let form = result.unwrap();
		assert!(form.callbacks.has_any());
		assert!(form.callbacks.on_success_ref.is_some());
		assert!(form.callbacks.on_success.is_none());
	}

	#[rstest]
	fn test_parse_on_success_and_on_success_ref_coexist() {
		// Arrange — issue #4624 contract: both callbacks may be supplied
		// simultaneously. The validator does not reject the combination;
		// codegen documents the order (ref-first, move-second).
		let input = quote! {
			name: DualCallbackForm,
			server_fn: update_profile,

			on_success_ref: |_form, _result| {},
			on_success: |_result| {},

			fields: {
				data: CharField {},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok(), "coexistence must parse");
		let form = result.unwrap();
		assert!(form.callbacks.on_success_ref.is_some());
		assert!(form.callbacks.on_success.is_some());
	}

	#[rstest]
	fn test_parse_form_without_callbacks() {
		// Arrange
		let input = quote! {
			name: NoCallbackForm,
			action: "/api/test",

			fields: {
				field1: CharField {},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(!form.callbacks.has_any());
	}

	#[rstest]
	fn test_parse_callbacks_with_state() {
		// Arrange
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

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();

		// Check state
		assert!(form.state.is_some());
		let state = form.state.unwrap();
		assert!(state.has_loading());
		assert!(state.has_error());
		assert!(state.has_success());

		// Check callbacks
		assert!(form.callbacks.on_success.is_some());
		assert!(form.callbacks.on_error.is_some());
		assert!(form.callbacks.on_submit.is_none());
		assert!(form.callbacks.on_loading.is_none());
	}

	#[rstest]
	fn test_parse_wrapper_basic() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				username: CharField {
					wrapper: div { class: "relative" },
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert_eq!(form.fields.len(), 1);
		let field = form.fields[0].as_field().unwrap();
		let wrapper = field.get_wrapper();
		assert!(wrapper.is_some());
		let wrapper = wrapper.unwrap();
		assert_eq!(wrapper.tag.to_string(), "div");
		assert_eq!(wrapper.attrs.len(), 1);
		assert_eq!(wrapper.attrs[0].name.to_string(), "class");
	}

	#[rstest]
	fn test_parse_wrapper_multiple_attrs() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				email: EmailField {
					wrapper: div {
						class: "form-field",
						id: "email-wrapper",
						data_testid: "email-field",
					},
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();
		let wrapper = field.get_wrapper();
		assert!(wrapper.is_some());
		let wrapper = wrapper.unwrap();
		assert_eq!(wrapper.tag.to_string(), "div");
		assert_eq!(wrapper.attrs.len(), 3);
		assert_eq!(wrapper.attrs[0].name.to_string(), "class");
		assert_eq!(wrapper.attrs[1].name.to_string(), "id");
		assert_eq!(wrapper.attrs[2].name.to_string(), "data_testid");
	}

	#[rstest]
	fn test_parse_wrapper_no_attrs() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				username: CharField {
					wrapper: span,
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();
		let wrapper = field.get_wrapper();
		assert!(wrapper.is_some());
		let wrapper = wrapper.unwrap();
		assert_eq!(wrapper.tag.to_string(), "span");
		assert!(wrapper.attrs.is_empty());
	}

	#[rstest]
	fn test_parse_wrapper_with_other_properties() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				password: CharField {
					required,
					widget: PasswordInput,
					label: "Password",
					wrapper: div { class: "password-field" },
					class: "input-text",
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();

		// Check wrapper exists
		let wrapper = field.get_wrapper();
		assert!(wrapper.is_some());
		let wrapper = wrapper.unwrap();
		assert_eq!(wrapper.tag.to_string(), "div");

		// Check other properties are also parsed (use pattern matching for Widget/Wrapper)
		assert!(
			field.properties.iter().any(|p| {
				matches!(p, FormFieldProperty::Flag { name, .. } if name == "required")
			})
		);
		assert!(field.properties.iter().any(|p| {
			matches!(
				p,
				FormFieldProperty::Widget {
					widget: FormWidgetSpec::Builtin(widget_type),
					..
				} if widget_type == "PasswordInput"
			)
		}));
		assert!(
			field
				.properties
				.iter()
				.any(|p| { matches!(p, FormFieldProperty::Named { name, .. } if name == "label") })
		);
		assert!(
			field
				.properties
				.iter()
				.any(|p| { matches!(p, FormFieldProperty::Named { name, .. } if name == "class") })
		);
	}

	#[rstest]
	fn test_parse_icon_basic() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				username: CharField {
					icon: svg {
						class: "w-5 h-5",
						viewBox: "0 0 24 24",
						path { d: "M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4z" }
					},
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert_eq!(form.fields.len(), 1);
		let field = form.fields[0].as_field().unwrap();
		let icon = field.get_icon();
		assert!(icon.is_some());
		let icon = icon.unwrap();
		assert_eq!(icon.attrs.len(), 2); // class and viewBox
		assert_eq!(icon.children.len(), 1); // path
		assert_eq!(icon.children[0].tag.to_string(), "path");
	}

	#[rstest]
	fn test_parse_icon_with_position() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				email: EmailField {
					icon: svg {
						viewBox: "0 0 24 24",
						path { d: "M20 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2z" }
					},
					icon_position: "right",
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();
		assert!(field.has_icon());
		assert_eq!(field.get_icon_position(), IconPosition::Right);
	}

	#[rstest]
	fn test_parse_icon_position_label() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				username: CharField {
					icon: svg {
						viewBox: "0 0 24 24",
						circle { cx: "12", cy: "12", r: "10" }
					},
					icon_position: "label",
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();
		assert_eq!(field.get_icon_position(), IconPosition::Label);
	}

	#[rstest]
	fn test_parse_icon_position_invalid() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				username: CharField {
					icon_position: "invalid",
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("invalid icon_position")
		);
	}

	#[rstest]
	fn test_parse_icon_invalid_element() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				username: CharField {
					icon: div {
						class: "icon",
					},
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_err());
		assert!(result.unwrap_err().to_string().contains("svg element"));
	}

	#[rstest]
	fn test_parse_icon_with_nested_g_element() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				avatar: UrlField {
					icon: svg {
						viewBox: "0 0 24 24",
						fill: "none",
						g {
							stroke: "currentColor",
							path { d: "M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4z" }
							path { d: "M20 21v-2c0-2.21-1.79-4-4-4H8c-2.21 0-4 1.79-4 4v2" }
						}
					},
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();
		let icon = field.get_icon().unwrap();
		assert_eq!(icon.attrs.len(), 2); // viewBox and fill
		assert_eq!(icon.children.len(), 1); // g element
		assert_eq!(icon.children[0].tag.to_string(), "g");
		assert_eq!(icon.children[0].children.len(), 2); // two path elements
	}

	#[rstest]
	fn test_parse_icon_default_position() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				username: CharField {
					icon: svg {
						viewBox: "0 0 24 24",
					},
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();
		// Default position should be Left
		assert_eq!(field.get_icon_position(), IconPosition::Left);
	}

	#[rstest]
	fn test_parse_custom_attrs_basic() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				email: EmailField {
					attrs: {
						aria_label: "Email address",
						data_testid: "email-input",
					},
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();
		assert!(field.has_attrs());
		let attrs = field.get_attrs().unwrap();
		assert_eq!(attrs.len(), 2);
		assert_eq!(attrs[0].name.to_string(), "aria_label");
		assert_eq!(attrs[1].name.to_string(), "data_testid");
	}

	#[rstest]
	fn test_parse_custom_attrs_single() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				password: CharField {
					widget: PasswordInput,
					attrs: {
						aria_required: "true",
					},
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();
		let attrs = field.get_attrs().unwrap();
		assert_eq!(attrs.len(), 1);
		assert_eq!(attrs[0].name.to_string(), "aria_required");
	}

	#[rstest]
	fn test_parse_custom_attrs_empty() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				username: CharField {
					attrs: {},
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();
		let attrs = field.get_attrs().unwrap();
		assert!(attrs.is_empty());
	}

	#[rstest]
	fn test_parse_field_without_attrs() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();
		assert!(!field.has_attrs());
		assert!(field.get_attrs().is_none());
	}

	#[rstest]
	fn test_parse_bind_true() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				username: CharField {
					bind: true,
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();
		assert_eq!(field.get_bind(), Some(true));
		assert!(field.is_bind_enabled());
	}

	#[rstest]
	fn test_parse_bind_false() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				password: CharField {
					widget: PasswordInput,
					bind: false,
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();
		assert_eq!(field.get_bind(), Some(false));
		assert!(!field.is_bind_enabled());
	}

	#[rstest]
	fn test_parse_bind_default() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();
		// When bind is not specified, get_bind() returns None
		assert_eq!(field.get_bind(), None);
		// But is_bind_enabled() returns true (default)
		assert!(field.is_bind_enabled());
	}

	#[rstest]
	fn test_parse_bind_invalid_value() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				username: CharField {
					bind: maybe,
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_err());
		// When using LitBool, invalid values produce a parsing error
		let err = result.unwrap_err().to_string();
		assert!(err.contains("expected") || err.contains("bool"));
	}

	#[rstest]
	fn test_parse_bind_with_other_properties() {
		// Arrange
		let input = quote! {
			name: TestForm,
			server_fn: submit,

			fields: {
				email: EmailField {
					required,
					label: "Email Address",
					class: "input-email",
					bind: true,
					placeholder: "Enter email",
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();

		// Check bind is parsed correctly
		assert_eq!(field.get_bind(), Some(true));

		// Check other properties are also parsed
		assert!(field.is_required());
		assert!(field.get_label().is_some());
		assert!(field.get_class().is_some());
		assert!(field.get_placeholder().is_some());
	}

	#[rstest]
	fn test_parse_watch_block() {
		// Arrange
		let input = quote! {
			name: LoginForm,
			server_fn: submit,

			watch: {
				error_display: |form| {
					form.error()
				},
			},

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.watch.is_some());
		let watch = form.watch.unwrap();
		assert_eq!(watch.items.len(), 1);
		assert_eq!(watch.items[0].name.to_string(), "error_display");
	}

	#[rstest]
	fn test_parse_watch_multiple_items() {
		// Arrange
		let input = quote! {
			name: LoginForm,
			server_fn: submit,

			watch: {
				error_display: |form| { form.error() },
				loading_indicator: |form| { form.loading() },
				success_message: |form| { form.success() },
			},

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.watch.is_some());
		let watch = form.watch.unwrap();
		assert_eq!(watch.items.len(), 3);
		assert_eq!(watch.items[0].name.to_string(), "error_display");
		assert_eq!(watch.items[1].name.to_string(), "loading_indicator");
		assert_eq!(watch.items[2].name.to_string(), "success_message");
	}

	#[rstest]
	fn test_parse_watch_empty_block() {
		// Arrange
		let input = quote! {
			name: LoginForm,
			server_fn: submit,

			watch: {},

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.watch.is_some());
		let watch = form.watch.unwrap();
		assert!(watch.is_empty());
	}

	#[rstest]
	fn test_parse_watch_complex_closure() {
		// Arrange
		let input = quote! {
			name: LoginForm,
			server_fn: submit,

			watch: {
				conditional_display: |form| {
					if form.error().get().is_some() {
						let error = form.error().get().unwrap();
						format!("Error: {}", error)
					} else {
						String::new()
					}
				},
			},

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.watch.is_some());
		let watch = form.watch.unwrap();
		assert_eq!(watch.items.len(), 1);
		assert_eq!(watch.items[0].name.to_string(), "conditional_display");
	}

	#[rstest]
	fn test_parse_watch_with_other_form_options() {
		// Arrange
		let input = quote! {
			name: ProfileForm,
			server_fn: update_profile,
			class: "profile-form",

			state: {
				loading,
				error,
			},

			on_success: |result| { log!("Success!") },

			watch: {
				field_preview: |form| { form.username() },
			},

			fields: {
				username: CharField { required },
				email: EmailField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		// Check watch is parsed
		assert!(form.watch.is_some());
		let watch = form.watch.unwrap();
		assert_eq!(watch.items.len(), 1);
		assert_eq!(watch.items[0].name.to_string(), "field_preview");

		// Check other options are also parsed
		assert!(form.state.is_some());
		assert!(form.callbacks.on_success.is_some());
		assert_eq!(form.fields.len(), 2);
	}

	#[rstest]
	fn test_parse_no_watch_block() {
		// Arrange
		let input = quote! {
			name: SimpleForm,
			server_fn: submit,

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.watch.is_none());
	}

	#[rstest]
	fn test_parse_redirect_on_success() {
		// Arrange
		let input = quote! {
			name: ProfileForm,
			server_fn: update_profile,

			redirect_on_success: "/profile",

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.redirect_on_success.is_some());
		assert_eq!(form.redirect_on_success.unwrap().value(), "/profile");
	}

	#[rstest]
	fn test_parse_redirect_with_parameter() {
		// Arrange
		let input = quote! {
			name: ProfileForm,
			server_fn: update_profile,

			redirect_on_success: "/profile/{id}/edit",

			fields: {
				id: IntegerField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.redirect_on_success.is_some());
		assert_eq!(
			form.redirect_on_success.unwrap().value(),
			"/profile/{id}/edit"
		);
	}

	#[rstest]
	fn test_parse_redirect_full_url() {
		// Arrange
		let input = quote! {
			name: ExternalForm,
			server_fn: submit_external,

			redirect_on_success: "https://example.com/success",

			fields: {
				data: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.redirect_on_success.is_some());
		assert_eq!(
			form.redirect_on_success.unwrap().value(),
			"https://example.com/success"
		);
	}

	#[rstest]
	fn test_parse_no_redirect() {
		// Arrange
		let input = quote! {
			name: SimpleForm,
			server_fn: submit,

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.redirect_on_success.is_none());
	}

	#[rstest]
	fn test_parse_redirect_with_callbacks() {
		// Arrange
		let input = quote! {
			name: CallbackForm,
			server_fn: submit,

			on_success: |result| { log!("Success!") },
			redirect_on_success: "/dashboard",

			fields: {
				data: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.callbacks.on_success.is_some());
		assert!(form.redirect_on_success.is_some());
		assert_eq!(form.redirect_on_success.unwrap().value(), "/dashboard");
	}

	#[rstest]
	fn test_parse_success_url_closure() {
		// Arrange — minimal valid form that declares a closure-valued
		// `success_url:` attribute. Regression coverage for #4604/#4611: the
		// outer key/value loop already consumes the `:`, so the `success_url`
		// arm must not consume it again.
		let input = quote! {
			name: VotingForm,
			server_fn: submit,

			success_url: |_form, _value| String::new(),

			fields: {
				choice: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		let form = result.unwrap_or_else(|err| panic!("parse failed: {err}"));
		assert!(form.success_url.is_some());
	}

	#[rstest]
	fn test_parse_success_url_followed_by_sibling() {
		// Arrange — `success_url:` followed by a sibling attribute. This
		// pins the original symptom of #4604/#4611: under the bug, the
		// success_url arm consumed an extra `:`, causing the parser to bail
		// at the sibling key with "expected `:`".
		let input = quote! {
			name: VotingForm,
			server_fn: submit,

			success_url: |_form, _value| "/x".to_string(),
			on_error: |_err| {},

			fields: {
				choice: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		let form = result.unwrap_or_else(|err| panic!("parse failed: {err}"));
		assert!(form.success_url.is_some());
		assert!(form.callbacks.on_error.is_some());
	}

	// =====================================================
	// initial_loader tests
	// =====================================================

	#[rstest]
	fn test_parse_initial_loader() {
		// Arrange
		let input = quote! {
			name: ProfileEditForm,
			server_fn: update_profile,

			initial_loader: get_profile_data,

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.initial_loader.is_some());
		let loader = form.initial_loader.unwrap();
		assert_eq!(
			loader.segments.last().unwrap().ident.to_string(),
			"get_profile_data"
		);
	}

	#[rstest]
	fn test_parse_initial_loader_with_path() {
		// Arrange
		let input = quote! {
			name: ProfileEditForm,
			server_fn: update_profile,

			initial_loader: api::profile::get_profile_data,

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.initial_loader.is_some());
	}

	#[rstest]
	fn test_parse_without_initial_loader() {
		// Arrange
		let input = quote! {
			name: SimpleForm,
			server_fn: submit,

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.initial_loader.is_none());
	}

	#[rstest]
	fn test_parse_initial_from_field_property() {
		// Arrange
		let input = quote! {
			name: ProfileEditForm,
			server_fn: update_profile,

			initial_loader: get_profile_data,

			fields: {
				username: CharField {
					required,
					initial_from: "user_name",
				},
				email: EmailField {
					initial_from: "email_address",
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert_eq!(form.fields.len(), 2);

		// Check first field has initial_from
		let username_field = form.fields[0].as_field().unwrap();
		let username_initial_from = username_field.get_initial_from();
		assert!(username_initial_from.is_some());
		assert_eq!(username_initial_from.unwrap().value(), "user_name");

		// Check second field has initial_from
		let email_field = form.fields[1].as_field().unwrap();
		let email_initial_from = email_field.get_initial_from();
		assert!(email_initial_from.is_some());
		assert_eq!(email_initial_from.unwrap().value(), "email_address");
	}

	#[rstest]
	fn test_parse_field_without_initial_from() {
		// Arrange
		let input = quote! {
			name: SimpleForm,
			server_fn: submit,

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();
		assert!(field.get_initial_from().is_none());
	}

	// =====================================================
	// slots tests
	// =====================================================

	#[rstest]
	fn test_parse_slots_before_fields() {
		// Arrange
		let input = quote! {
			name: LoginForm,
			server_fn: submit,

			slots: {
				before_fields: || { "Header content" },
			},

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.slots.is_some());
		let slots = form.slots.unwrap();
		assert!(slots.before_fields.is_some());
		assert!(slots.after_fields.is_none());
	}

	#[rstest]
	fn test_parse_slots_after_fields() {
		// Arrange
		let input = quote! {
			name: LoginForm,
			server_fn: submit,

			slots: {
				after_fields: || { "Footer content" },
			},

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.slots.is_some());
		let slots = form.slots.unwrap();
		assert!(slots.before_fields.is_none());
		assert!(slots.after_fields.is_some());
	}

	#[rstest]
	fn test_parse_slots_both() {
		// Arrange
		let input = quote! {
			name: LoginForm,
			server_fn: submit,

			slots: {
				before_fields: || { view! { <div class="form-header">"Please login"</div> } },
				after_fields: || { view! { <button type="submit">"Submit"</button> } },
			},

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.slots.is_some());
		let slots = form.slots.unwrap();
		assert!(slots.before_fields.is_some());
		assert!(slots.after_fields.is_some());
	}

	#[rstest]
	fn test_parse_slots_empty() {
		// Arrange
		let input = quote! {
			name: LoginForm,
			server_fn: submit,

			slots: {},

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.slots.is_some());
		let slots = form.slots.unwrap();
		assert!(slots.before_fields.is_none());
		assert!(slots.after_fields.is_none());
	}

	#[rstest]
	fn test_parse_no_slots() {
		// Arrange
		let input = quote! {
			name: SimpleForm,
			server_fn: submit,

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.slots.is_none());
	}

	#[rstest]
	fn test_parse_slots_unknown_slot() {
		// Arrange
		let input = quote! {
			name: LoginForm,
			server_fn: submit,

			slots: {
				invalid_slot: || { "content" },
			},

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_err());
		assert!(result.unwrap_err().to_string().contains("Unknown slot"));
	}

	#[rstest]
	fn test_parse_slots_duplicate_before_fields() {
		// Arrange
		let input = quote! {
			name: LoginForm,
			server_fn: submit,

			slots: {
				before_fields: || { "first" },
				before_fields: || { "second" },
			},

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_err());
		assert!(result.unwrap_err().to_string().contains("already defined"));
	}

	#[rstest]
	fn test_parse_slots_with_other_options() {
		// Arrange
		let input = quote! {
			name: ProfileForm,
			server_fn: update_profile,
			class: "profile-form",

			state: { loading, error },

			on_success: |result| { log!("Success!") },

			slots: {
				before_fields: || { "Form Header" },
				after_fields: || { "Form Footer" },
			},

			fields: {
				username: CharField { required },
				email: EmailField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();

		// Check slots
		assert!(form.slots.is_some());
		let slots = form.slots.unwrap();
		assert!(slots.before_fields.is_some());
		assert!(slots.after_fields.is_some());

		// Check other options
		assert!(form.state.is_some());
		assert!(form.callbacks.on_success.is_some());
		assert_eq!(form.fields.len(), 2);
	}

	#[rstest]
	fn test_parse_full_form_with_all_features() {
		// Arrange
		let input = quote! {
			name: ProfileEditForm,
			server_fn: update_profile,
			class: "profile-form",

			state: { loading, error, success },

			on_submit: |form| { log!("Submitting...") },
			on_success: |result| { log!("Success!") },
			on_error: |e| { log!("Error: {}", e) },

			initial_loader: get_profile_data,

			redirect_on_success: "/profile",

			slots: {
				before_fields: || { "Edit your profile" },
				after_fields: || { "Save changes" },
			},

			watch: {
				error_display: |form| { form.error() },
			},

			fields: {
				username: CharField {
					required,
					initial_from: "username",
					label: "Username",
				},
				email: EmailField {
					required,
					initial_from: "email",
				},
			},

			validators: {
				username: [|v| !v.is_empty() => "Username required"],
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();

		// Check all features
		assert!(form.state.is_some());
		assert!(form.callbacks.on_submit.is_some());
		assert!(form.callbacks.on_success.is_some());
		assert!(form.callbacks.on_error.is_some());
		assert!(form.initial_loader.is_some());
		assert!(form.redirect_on_success.is_some());
		assert!(form.slots.is_some());
		assert!(form.watch.is_some());
		assert_eq!(form.fields.len(), 2);
		assert_eq!(form.validators.len(), 1);

		// Check initial_from on fields
		let field0 = form.fields[0].as_field().unwrap();
		let field1 = form.fields[1].as_field().unwrap();
		assert!(field0.get_initial_from().is_some());
		assert!(field1.get_initial_from().is_some());
	}

	// =====================================================
	// field group tests
	// =====================================================

	#[test]
	fn test_parse_field_array_entry() {
		let input: TokenStream = quote! {
			name: InvoiceForm,
			fields: {
				line_items: FieldArray {
					label: "Line items",
					min_items: 1,
					max_items: 10,
					initial_from: "line_items",
					fields: {
						description: CharField { required }
						quantity: IntegerField { initial: 1, required }
					},
					render_item: |item| { item.default_row() }
				}
			}
		};

		let form: FormMacro = syn::parse2(input).expect("FieldArray should parse");
		let entry = form.fields.first().expect("line_items entry");
		assert!(entry.is_collection());
		assert_eq!(entry.name().to_string(), "line_items");
		let collection = entry.as_collection().expect("collection entry");
		assert_eq!(
			collection.label.as_ref().map(LitStr::value),
			Some("Line items".to_string())
		);
		assert_eq!(
			collection
				.min_items
				.as_ref()
				.map(|lit| lit.base10_parse::<usize>().unwrap()),
			Some(1)
		);
		assert_eq!(
			collection
				.max_items
				.as_ref()
				.map(|lit| lit.base10_parse::<usize>().unwrap()),
			Some(10)
		);
		assert_eq!(
			collection.initial_from.as_ref().map(LitStr::value),
			Some("line_items".to_string())
		);
		assert_eq!(collection.fields.len(), 2);
		assert!(collection.render_item.is_some());
	}

	#[test]
	fn test_parse_nested_field_array_entry() {
		let input: TokenStream = quote! {
			name: SurveyForm,
			fields: {
				sections: FieldArray {
					fields: {
						title: CharField { required }
						questions: FieldArray {
							fields: {
								label: CharField { required }
							}
						}
					}
				}
			}
		};

		let form: FormMacro = syn::parse2(input).expect("nested FieldArray should parse");
		let sections = form.fields.first().unwrap().as_collection().unwrap();
		let questions = sections
			.fields
			.iter()
			.find(|entry| entry.name() == "questions")
			.and_then(FormFieldEntry::as_collection)
			.expect("questions nested collection");
		assert_eq!(questions.fields.len(), 1);
	}

	#[rstest]
	fn test_parse_field_group_basic() {
		// Arrange
		let input = quote! {
			name: AddressForm,
			server_fn: submit,

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

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert_eq!(form.fields.len(), 1);
		let entry = &form.fields[0];
		assert!(entry.is_group());
		assert!(!entry.is_field());
		let group = entry.as_group().unwrap();
		assert_eq!(group.name.to_string(), "address_group");
		assert_eq!(group.label_text(), Some("Address".to_string()));
		assert!(group.class_name().is_none());
		assert_eq!(group.fields.len(), 2);
		assert_eq!(
			group.fields[0].as_field().unwrap().name.to_string(),
			"street"
		);
		assert_eq!(group.fields[1].as_field().unwrap().name.to_string(), "city");
	}

	#[rstest]
	fn test_parse_field_group_with_class() {
		// Arrange
		let input = quote! {
			name: AddressForm,
			server_fn: submit,

			fields: {
				address_group: FieldGroup {
					label: "Address",
					class: "address-section",
					fields: {
						street: CharField { required },
						city: CharField { required },
					},
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let group = form.fields[0].as_group().unwrap();
		assert_eq!(group.label_text(), Some("Address".to_string()));
		assert_eq!(group.class_name(), Some("address-section".to_string()));
	}

	#[rstest]
	fn test_parse_field_group_without_label() {
		// Arrange
		let input = quote! {
			name: AddressForm,
			server_fn: submit,

			fields: {
				address_group: FieldGroup {
					class: "address-section",
					fields: {
						street: CharField { required },
						city: CharField { required },
					},
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let group = form.fields[0].as_group().unwrap();
		assert!(group.label_text().is_none());
		assert_eq!(group.class_name(), Some("address-section".to_string()));
	}

	#[rstest]
	fn test_parse_field_group_mixed_with_fields() {
		// Arrange
		let input = quote! {
			name: ProfileForm,
			server_fn: submit,

			fields: {
				username: CharField { required },
				address_group: FieldGroup {
					label: "Address",
					fields: {
						street: CharField { required },
						city: CharField { required },
					},
				},
				email: EmailField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert_eq!(form.fields.len(), 3);

		// First entry is a field
		assert!(form.fields[0].is_field());
		let username = form.fields[0].as_field().unwrap();
		assert_eq!(username.name.to_string(), "username");

		// Second entry is a group
		assert!(form.fields[1].is_group());
		let group = form.fields[1].as_group().unwrap();
		assert_eq!(group.name.to_string(), "address_group");
		assert_eq!(group.fields.len(), 2);

		// Third entry is a field
		assert!(form.fields[2].is_field());
		let email = form.fields[2].as_field().unwrap();
		assert_eq!(email.name.to_string(), "email");
	}

	#[rstest]
	fn test_parse_field_group_nested_prohibited() {
		// Arrange
		let input = quote! {
			name: AddressForm,
			server_fn: submit,

			fields: {
				outer_group: FieldGroup {
					label: "Outer",
					fields: {
						inner_group: FieldGroup {
							label: "Inner",
							fields: {
								field1: CharField { required },
							},
						},
					},
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert!(err.contains("nested field groups are not allowed"));
	}

	#[rstest]
	fn test_parse_field_group_field_count() {
		// Arrange
		let input = quote! {
			name: AddressForm,
			server_fn: submit,

			fields: {
				address_group: FieldGroup {
					label: "Address",
					fields: {
						street: CharField { required },
						city: CharField { required },
						zip: CharField { max_length: 10 },
					},
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let group = form.fields[0].as_group().unwrap();
		assert_eq!(group.field_count(), 3);
	}

	#[rstest]
	fn test_parse_field_group_fields_only() {
		// Arrange
		let input = quote! {
			name: AddressForm,
			server_fn: submit,

			fields: {
				address_group: FieldGroup {
					fields: {
						street: CharField { required },
						city: CharField { required },
					},
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let group = form.fields[0].as_group().unwrap();
		assert!(group.label_text().is_none());
		assert!(group.class_name().is_none());
		assert_eq!(group.field_count(), 2);
	}

	#[rstest]
	fn test_parse_multiple_field_groups() {
		// Arrange
		let input = quote! {
			name: AddressForm,
			server_fn: submit,

			fields: {
				personal_info: FieldGroup {
					label: "Personal Information",
					fields: {
						first_name: CharField { required },
						last_name: CharField { required },
					},
				},
				address_info: FieldGroup {
					label: "Address Information",
					fields: {
						street: CharField { required },
						city: CharField { required },
						zip: CharField { max_length: 10 },
					},
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert_eq!(form.fields.len(), 2);
		let group1 = form.fields[0].as_group().unwrap();
		assert_eq!(group1.name.to_string(), "personal_info");
		assert_eq!(group1.field_count(), 2);
		let group2 = form.fields[1].as_group().unwrap();
		assert_eq!(group2.name.to_string(), "address_info");
		assert_eq!(group2.field_count(), 3);
	}

	#[rstest]
	fn test_parse_field_group_with_field_properties() {
		// Arrange
		let input = quote! {
			name: AddressForm,
			server_fn: submit,

			fields: {
				address_group: FieldGroup {
					label: "Address",
					fields: {
						street: CharField {
							required,
							label: "Street Address",
							class: "input-street",
							placeholder: "Enter street",
						},
						city: CharField {
							required,
							label: "City",
						},
					},
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let group = form.fields[0].as_group().unwrap();
		let street = group.fields[0].as_field().unwrap();
		assert!(street.is_required());
		assert!(street.get_label().is_some());
		assert!(street.get_class().is_some());
		assert!(street.get_placeholder().is_some());
		let city = group.fields[1].as_field().unwrap();
		assert!(city.is_required());
		assert!(city.get_label().is_some());
	}

	// ============================================================
	// Derived Block Tests
	// ============================================================

	#[rstest]
	fn test_parse_derived_block() {
		// Arrange
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

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.derived.is_some());
		let derived = form.derived.unwrap();
		assert_eq!(derived.items.len(), 1);
		assert_eq!(derived.items[0].name.to_string(), "char_count");
	}

	#[rstest]
	fn test_parse_derived_multiple_items() {
		// Arrange
		let input = quote! {
			name: TweetForm,
			server_fn: create_tweet,

			derived: {
				char_count: |form| form.content().get().len(),
				is_over_limit: |form| form.char_count().get() > 280,
				progress_percent: |form| (form.char_count().get() as f32 / 280.0 * 100.0).min(100.0),
			},

			fields: {
				content: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.derived.is_some());
		let derived = form.derived.unwrap();
		assert_eq!(derived.items.len(), 3);
		assert_eq!(derived.items[0].name.to_string(), "char_count");
		assert_eq!(derived.items[1].name.to_string(), "is_over_limit");
		assert_eq!(derived.items[2].name.to_string(), "progress_percent");
	}

	#[rstest]
	fn test_parse_derived_empty_block() {
		// Arrange
		let input = quote! {
			name: SimpleForm,
			server_fn: submit,

			derived: {},

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.derived.is_some());
		let derived = form.derived.unwrap();
		assert!(derived.is_empty());
	}

	#[rstest]
	fn test_parse_derived_complex_closure() {
		// Arrange
		let input = quote! {
			name: PriceForm,
			server_fn: calculate,

			derived: {
				total_price: |form| {
					let quantity = form.quantity().get();
					let unit_price = form.unit_price().get();
					let discount = form.discount().get();
					(quantity as f64 * unit_price) * (1.0 - discount / 100.0)
				},
				formatted_price: |form| {
					format!("${:.2}", form.total_price().get())
				},
			},

			fields: {
				quantity: IntegerField { required },
				unit_price: DecimalField { required },
				discount: DecimalField { initial: "0" },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.derived.is_some());
		let derived = form.derived.unwrap();
		assert_eq!(derived.items.len(), 2);
		assert_eq!(derived.items[0].name.to_string(), "total_price");
		assert_eq!(derived.items[1].name.to_string(), "formatted_price");
	}

	#[rstest]
	fn test_parse_no_derived_block() {
		// Arrange
		let input = quote! {
			name: BasicForm,
			server_fn: submit,

			fields: {
				username: CharField { required },
				password: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.derived.is_none());
	}

	#[rstest]
	fn test_parse_derived_with_watch() {
		// Arrange
		let input = quote! {
			name: TweetForm,
			server_fn: create_tweet,

			derived: {
				char_count: |form| form.content().get().len(),
			},

			watch: {
				counter_display: |form| {
					format!("{}/280", form.char_count().get())
				},
			},

			fields: {
				content: CharField { required, bind: true },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.derived.is_some());
		assert!(form.watch.is_some());
		let derived = form.derived.unwrap();
		assert_eq!(derived.items.len(), 1);
		assert_eq!(derived.items[0].name.to_string(), "char_count");
		let watch = form.watch.unwrap();
		assert_eq!(watch.items.len(), 1);
		assert_eq!(watch.items[0].name.to_string(), "counter_display");
	}

	// ============================================================
	// Dynamic ChoiceField tests (choices_loader, choices_from, etc.)
	// ============================================================

	#[rstest]
	fn test_parse_choices_loader() {
		// Arrange
		let input = quote! {
			name: VotingForm,
			server_fn: vote,

			choices_loader: get_poll_choices,

			fields: {
				choice: ChoiceField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.choices_loader.is_some());
		let loader = form.choices_loader.unwrap();
		assert_eq!(
			loader.segments.last().unwrap().ident.to_string(),
			"get_poll_choices"
		);
	}

	#[rstest]
	fn test_parse_choices_loader_with_path() {
		// Arrange
		let input = quote! {
			name: VotingForm,
			server_fn: vote,

			choices_loader: api::polls::get_poll_choices,

			fields: {
				choice: ChoiceField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.choices_loader.is_some());
	}

	#[rstest]
	fn test_parse_without_choices_loader() {
		// Arrange
		let input = quote! {
			name: SimpleForm,
			server_fn: submit,

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert!(form.choices_loader.is_none());
	}

	#[rstest]
	fn test_parse_choices_from_field_property() {
		// Arrange
		let input = quote! {
			name: VotingForm,
			server_fn: vote,

			choices_loader: get_poll_data,

			fields: {
				choice: ChoiceField {
					required,
					choices_from: "poll_options",
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();
		let choices_from = field.get_choices_from();
		assert!(choices_from.is_some());
		assert_eq!(choices_from.unwrap().value(), "poll_options");
	}

	#[rstest]
	fn test_parse_choice_value_and_label_properties() {
		// Arrange
		let input = quote! {
			name: VotingForm,
			server_fn: vote,

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

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();

		// Check choices_from
		let choices_from = field.get_choices_from();
		assert!(choices_from.is_some());
		assert_eq!(choices_from.unwrap().value(), "choices");

		// Check choice_value
		let choice_value = field.get_choice_value();
		assert!(choice_value.is_some());
		assert_eq!(choice_value.unwrap().value(), "id");

		// Check choice_label
		let choice_label = field.get_choice_label();
		assert!(choice_label.is_some());
		assert_eq!(choice_label.unwrap().value(), "choice_text");

		// Verify it's marked as dynamic choice field
		assert!(field.is_dynamic_choice_field());
	}

	#[rstest]
	fn test_parse_field_without_choices_from() {
		// Arrange
		let input = quote! {
			name: SimpleForm,
			server_fn: submit,

			fields: {
				category: ChoiceField { required },
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();

		// Field without choices_from is not a dynamic choice field
		assert!(!field.has_choices_from());
		assert!(!field.is_dynamic_choice_field());
	}

	#[rstest]
	fn test_parse_dynamic_choice_field_with_widget() {
		// Arrange
		let input = quote! {
			name: VotingForm,
			server_fn: vote,

			choices_loader: get_poll_data,

			fields: {
				choice: ChoiceField {
					required,
					widget: RadioSelect,
					choices_from: "options",
					choice_value: "id",
					choice_label: "text",
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		let field = form.fields[0].as_field().unwrap();

		// Verify widget and choices properties coexist
		assert!(field.is_dynamic_choice_field());
		assert!(field.get_widget().is_some());
	}

	#[rstest]
	fn test_parse_choices_loader_with_initial_loader() {
		// Arrange
		let input = quote! {
			name: EditPollForm,
			server_fn: update_poll,

			initial_loader: get_poll_edit_data,
			choices_loader: get_choice_options,

			fields: {
				title: CharField { initial_from: "poll_title" },
				selected_choice: ChoiceField {
					choices_from: "available_choices",
					choice_value: "id",
					choice_label: "text",
				},
			},
		};

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();

		// Both loaders should be parsed
		assert!(form.initial_loader.is_some());
		assert!(form.choices_loader.is_some());

		// Verify initial_loader
		let initial_loader = form.initial_loader.unwrap();
		assert_eq!(
			initial_loader.segments.last().unwrap().ident.to_string(),
			"get_poll_edit_data"
		);

		// Verify choices_loader
		let choices_loader = form.choices_loader.unwrap();
		assert_eq!(
			choices_loader.segments.last().unwrap().ident.to_string(),
			"get_choice_options"
		);
	}

	#[rstest]
	fn test_parse_multiple_dynamic_choice_fields() {
		// Arrange
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

		// Act
		let result: Result<FormMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_ok());
		let form = result.unwrap();
		assert_eq!(form.fields.len(), 2);

		// Check first field
		let category_field = form.fields[0].as_field().unwrap();
		assert!(category_field.is_dynamic_choice_field());
		assert_eq!(
			category_field.get_choices_from().unwrap().value(),
			"categories"
		);
		assert_eq!(category_field.get_choice_value().unwrap().value(), "id");
		assert_eq!(category_field.get_choice_label().unwrap().value(), "name");

		// Check second field
		let status_field = form.fields[1].as_field().unwrap();
		assert!(status_field.is_dynamic_choice_field());
		assert_eq!(status_field.get_choices_from().unwrap().value(), "statuses");
		assert_eq!(status_field.get_choice_value().unwrap().value(), "code");
		assert_eq!(
			status_field.get_choice_label().unwrap().value(),
			"description"
		);
	}

	// --- SubmitButton tests (Fixes #3331) ---

	#[rstest]
	fn test_parse_submit_button_basic() {
		let input = quote! {
			name: TestForm,
			action: "/test",

			fields: {
				username: CharField { required },
				submit: SubmitButton { label: "Sign in", class: "btn-primary" },
			},
		};

		let result: Result<FormMacro> = syn::parse2(input);
		assert!(result.is_ok());
		let form = result.unwrap();

		assert_eq!(form.fields.len(), 2);
		assert!(form.fields[0].is_field());
		assert!(form.fields[1].is_submit_button());

		let btn = form.fields[1].as_submit_button().unwrap();
		assert_eq!(btn.name, "submit");
		assert_eq!(btn.properties.len(), 2);
	}

	#[rstest]
	fn test_parse_submit_button_minimal() {
		let input = quote! {
			name: TestForm,
			action: "/test",

			fields: {
				email: EmailField { required },
				go: SubmitButton {},
			},
		};

		let result: Result<FormMacro> = syn::parse2(input);
		assert!(result.is_ok());
		let form = result.unwrap();

		assert_eq!(form.fields.len(), 2);
		let btn = form.fields[1].as_submit_button().unwrap();
		assert_eq!(btn.name, "go");
		assert_eq!(btn.properties.len(), 0);
	}

	#[rstest]
	fn test_parse_submit_button_no_braces() {
		let input = quote! {
			name: TestForm,
			action: "/test",

			fields: {
				name: CharField {},
				submit: SubmitButton,
			},
		};

		let result: Result<FormMacro> = syn::parse2(input);
		assert!(result.is_ok());
		let form = result.unwrap();

		assert_eq!(form.fields.len(), 2);
		let btn = form.fields[1].as_submit_button().unwrap();
		assert_eq!(btn.properties.len(), 0);
	}
}

#[cfg(test)]
mod scope_tests {
	use super::*;
	use crate::core::{ClientTrigger, ValidatorRule, ValidatorScope};
	use quote::quote;
	use rstest::*;

	/// Wrapper that parses `[ ... ]` as a rules list via `parse_validator_rules`.
	#[derive(Debug)]
	struct ValidatorRulesWrapper(Vec<ValidatorRule>);

	impl Parse for ValidatorRulesWrapper {
		fn parse(input: ParseStream) -> Result<Self> {
			parse_validator_rules(input).map(ValidatorRulesWrapper)
		}
	}

	#[rstest]
	#[case("[ |v| v.len() > 0 => \"err\" ]", ValidatorScope::Both)]
	#[case("[ #[server] |v| v.len() > 0 => \"err\" ]", ValidatorScope::Server)]
	#[case(
		"[ #[client(on = submit)] |v| v.len() > 0 => \"err\" ]",
		ValidatorScope::Client {
			trigger: ClientTrigger::Submit,
		}
	)]
	#[case(
		"[ #[client(on = input)] |v| v.len() > 0 => \"err\" ]",
		ValidatorScope::Client {
			trigger: ClientTrigger::Input,
		}
	)]
	#[case(
		"[ #[client(on = blur)] |v| v.len() > 0 => \"err\" ]",
		ValidatorScope::Client {
			trigger: ClientTrigger::Blur,
		}
	)]
	#[case(
		"[ #[server] #[client(on = blur)] |v| v.len() > 0 => \"err\" ]",
		ValidatorScope::ServerAndClient {
			trigger: ClientTrigger::Blur,
		}
	)]
	fn test_scope_parsing(#[case] input: &str, #[case] expected_scope: ValidatorScope) {
		// Act
		let wrapper: ValidatorRulesWrapper = syn::parse_str(input).unwrap();

		// Assert
		assert_eq!(wrapper.0.len(), 1);
		assert_eq!(wrapper.0[0].scope, expected_scope);
	}

	#[rstest]
	#[case(
		"[ #[client(on = hover)] |v| v.len() > 0 => \"err\" ]",
		"unknown trigger"
	)]
	#[case(
		"[ #[unknown] |v| v.len() > 0 => \"err\" ]",
		"unknown validator attribute"
	)]
	#[case("[ #[server] #[server] |v| v.len() > 0 => \"err\" ]", "duplicate")]
	fn test_scope_parsing_errors(#[case] input: &str, #[case] expected_fragment: &str) {
		// Act
		let result: Result<ValidatorRulesWrapper> = syn::parse_str(input);

		// Assert
		let err = result.expect_err("expected parse error");
		assert!(
			err.to_string().contains(expected_fragment),
			"Expected error containing '{}', got: {}",
			expected_fragment,
			err
		);
	}

	// ---- Field-type generic argument parsing (Issue #4397) ----

	#[rstest]
	fn parses_field_type_with_generic_argument() {
		// Arrange
		let input = quote! {
			name: MyForm,
			action: "/x",

			fields: {
				question_id: HiddenField<i64> { initial: 0i64 },
			},
		};

		// Act
		let parsed: FormMacro = syn::parse2(input).expect("should parse");

		// Assert
		let field = parsed.fields[0]
			.as_field()
			.expect("question_id should be a regular field");
		assert_eq!(field.name.to_string(), "question_id");
		let generics = field.generics.as_ref().expect("generics should be Some");
		assert_eq!(generics.len(), 1);
		let first = &generics[0];
		let first_str = quote!(#first).to_string();
		assert_eq!(first_str.replace(' ', ""), "i64");
	}

	#[rstest]
	fn parses_field_type_without_generic_argument() {
		// Arrange
		let input = quote! {
			name: MyForm,
			action: "/x",

			fields: {
				username: CharField { required },
			},
		};

		// Act
		let parsed: FormMacro = syn::parse2(input).expect("should parse");

		// Assert
		let field = parsed.fields[0]
			.as_field()
			.expect("username should be a regular field");
		assert_eq!(field.name.to_string(), "username");
		assert!(field.generics.is_none());
	}

	#[rstest]
	fn rejects_zero_generic_arguments() {
		// Arrange
		let input = quote! {
			name: MyForm,
			action: "/x",

			fields: {
				question_id: HiddenField<> {},
			},
		};

		// Act
		let res: Result<FormMacro> = syn::parse2(input);

		// Assert
		let err = res.expect_err("empty <> must be rejected");
		assert!(
			err.to_string()
				.contains("expected at least one type argument"),
			"unexpected error message: {err}",
		);
	}
}
