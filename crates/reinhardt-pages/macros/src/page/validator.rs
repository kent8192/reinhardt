//! Validation and transformation logic for page! macro AST.
//!
//! This module transforms the untyped AST from the parser into a typed AST,
//! while performing semantic validation and type checking.
//!
//! ## Validation Rules
//!
//! 1. **Event Handlers**: Must be closure expressions with 0 or 1 arguments
//! 2. **Attributes**: data-* and aria-* attributes must follow naming conventions
//! 3. **Element Nesting**: Void elements cannot have children, interactive elements cannot nest
//! 4. **Required Attributes**: img elements must have alt attributes (accessibility)
//! 5. **Attribute Types**: Certain attributes must be specific types (e.g., img src must be string literal)

use proc_macro2::Span;
use syn::{Expr, Result};

use reinhardt_core::security::xss::is_safe_url;
use reinhardt_manouche::core::{
	PageAttr, PageBody, PageComponent, PageElement, PageElse, PageEvent, PageMacro, PageNode,
	PageWatch, TypedPageAttr, TypedPageBody, TypedPageComponent, TypedPageElement, TypedPageElse,
	TypedPageFor, TypedPageIf, TypedPageMacro, TypedPageNode, TypedPageWatch, types::AttrValue,
};

/// Validates and transforms the entire PageMacro AST into a typed AST.
///
/// This is the main entry point for validation. It performs semantic checks
/// and type transformations, returning a fully validated and typed AST.
///
/// # Errors
///
/// Returns a compilation error if any validation rule is violated.
///
/// # Returns
///
/// A `TypedPageMacro` with validated and type-safe attribute values.
pub(super) fn validate(ast: &PageMacro) -> Result<TypedPageMacro> {
	let typed_body = transform_body(&ast.body, &[])?;

	Ok(TypedPageMacro {
		head: ast.head.clone(),
		params: ast.params.clone(),
		body: typed_body,
		span: ast.span,
	})
}

/// Transforms a PageBody into a TypedPageBody.
///
/// # Arguments
///
/// * `body` - The untyped body to transform
/// * `parent_tags` - Stack of parent element tag names (for nesting validation)
fn transform_body(body: &PageBody, parent_tags: &[String]) -> Result<TypedPageBody> {
	let nodes = transform_nodes(&body.nodes, parent_tags)?;
	Ok(TypedPageBody {
		nodes,
		span: body.span,
	})
}

/// Transforms a slice of page nodes into typed nodes recursively.
///
/// # Arguments
///
/// * `nodes` - The nodes to transform
/// * `parent_tags` - Stack of parent element tag names (for nesting validation)
fn transform_nodes(nodes: &[PageNode], parent_tags: &[String]) -> Result<Vec<TypedPageNode>> {
	let mut typed_nodes = Vec::new();

	for node in nodes {
		typed_nodes.push(transform_node(node, parent_tags)?);
	}

	Ok(typed_nodes)
}

/// Transforms a single PageNode into a TypedPageNode.
///
/// Dispatches to the appropriate transformation function based on node type.
fn transform_node(node: &PageNode, parent_tags: &[String]) -> Result<TypedPageNode> {
	match node {
		PageNode::Element(elem) => Ok(TypedPageNode::Element(transform_element(
			elem,
			parent_tags,
		)?)),
		PageNode::Text(text) => Ok(TypedPageNode::Text(text.clone())),
		PageNode::Expression(expr) => Ok(TypedPageNode::Expression(expr.clone())),
		PageNode::If(if_node) => Ok(TypedPageNode::If(transform_if(if_node, parent_tags)?)),
		PageNode::For(for_node) => Ok(TypedPageNode::For(transform_for(for_node, parent_tags)?)),
		PageNode::Component(comp) => Ok(TypedPageNode::Component(transform_component(
			comp,
			parent_tags,
		)?)),
		PageNode::Watch(watch_node) => Ok(TypedPageNode::Watch(transform_watch(
			watch_node,
			parent_tags,
		)?)),
	}
}

/// Transforms a PageIf node (if/else if/else).
///
/// Recursively validates all branches.
fn transform_if(
	if_node: &reinhardt_manouche::core::PageIf,
	parent_tags: &[String],
) -> Result<TypedPageIf> {
	// Transform then branch
	let then_branch = transform_nodes(&if_node.then_branch, parent_tags)?;

	// Transform else branch if present
	let else_branch = if let Some(else_br) = &if_node.else_branch {
		Some(transform_else(else_br, parent_tags)?)
	} else {
		None
	};

	Ok(TypedPageIf {
		condition: if_node.condition.clone(),
		then_branch,
		else_branch,
		span: if_node.span,
	})
}

/// Transforms a PageElse branch.
fn transform_else(else_branch: &PageElse, parent_tags: &[String]) -> Result<TypedPageElse> {
	match else_branch {
		PageElse::Block(nodes) => {
			let typed_nodes = transform_nodes(nodes, parent_tags)?;
			Ok(TypedPageElse::Block(typed_nodes))
		}
		PageElse::If(nested_if) => {
			// Recursively transform nested if
			let typed_if = transform_if(nested_if, parent_tags)?;
			Ok(TypedPageElse::If(Box::new(typed_if)))
		}
	}
}

/// Transforms a PageFor node.
fn transform_for(
	for_node: &reinhardt_manouche::core::PageFor,
	parent_tags: &[String],
) -> Result<TypedPageFor> {
	let body = transform_nodes(&for_node.body, parent_tags)?;

	Ok(TypedPageFor {
		pat: for_node.pat.clone(),
		iter: for_node.iter.clone(),
		body,
		span: for_node.span,
	})
}

/// Transforms a PageWatch node.
fn transform_watch(watch_node: &PageWatch, parent_tags: &[String]) -> Result<TypedPageWatch> {
	let inner = transform_node(&watch_node.expr, parent_tags)?;

	Ok(TypedPageWatch {
		expr: Box::new(inner),
		span: watch_node.span,
	})
}

/// Transforms a PageComponent node.
///
/// Recursively transforms the component's children (if any).
fn transform_component(comp: &PageComponent, parent_tags: &[String]) -> Result<TypedPageComponent> {
	// Transform children if present
	let typed_children = if let Some(children) = &comp.children {
		Some(transform_nodes(children, parent_tags)?)
	} else {
		None
	};

	Ok(TypedPageComponent {
		name: comp.name.clone(),
		args: comp.args.clone(),
		children: typed_children,
		span: comp.span,
	})
}

/// Transforms and validates an element and its children.
///
/// Performs the following checks:
/// - Event handler validation
/// - Attribute transformation and validation
/// - Element nesting rules
/// - Required attributes
/// - HTML specification compliance (Phase 2)
fn transform_element(elem: &PageElement, parent_tags: &[String]) -> Result<TypedPageElement> {
	let tag = elem.tag.to_string();

	// 1. Validate events (unchanged from untyped version)
	for event in &elem.events {
		validate_event_handler(event)?;
	}

	// 2. Transform and validate attributes
	let typed_attrs = transform_attrs(&elem.attrs, &tag)?;

	// 3. Validate element nesting
	validate_element_nesting(elem, parent_tags)?;

	// 4. Recursively transform children
	let mut child_tags = parent_tags.to_vec();
	child_tags.push(tag.clone());
	let typed_children = transform_nodes(&elem.children, &child_tags)?;

	// Create typed element
	let typed_element = TypedPageElement {
		tag: elem.tag.clone(),
		attrs: typed_attrs,
		events: elem.events.clone(),
		children: typed_children,
		span: elem.span,
	};

	// 6. Validate against HTML specification (Phase 2)
	super::html_spec::validate_against_spec(&typed_element)?;

	// 7. Validate accessibility requirements (Phase 5)
	validate_accessibility(
		&tag,
		&typed_element.attrs,
		&typed_element.children,
		elem.span,
	)?;

	Ok(typed_element)
}

/// Transforms attributes from untyped to typed, with validation.
///
/// This function converts `Expr` attribute values into `AttrValue`,
/// enabling type-specific validation.
fn transform_attrs(attrs: &[PageAttr], element_tag: &str) -> Result<Vec<TypedPageAttr>> {
	let mut typed_attrs = Vec::new();

	for attr in attrs {
		// Validate attribute naming conventions (data-*, aria-*)
		validate_attribute(attr, element_tag)?;

		// Transform to typed value
		let typed_value = AttrValue::from_expr(attr.value.clone());

		// Validate attribute type for specific elements/attributes
		validate_attr_type(&attr.name.to_string(), &typed_value, element_tag, attr.span)?;

		typed_attrs.push(TypedPageAttr {
			name: attr.name.clone(),
			value: typed_value,
			span: attr.span,
		});
	}

	Ok(typed_attrs)
}

/// Checks if an attribute is a URL attribute for the given element.
///
/// Note: img element's src attribute is excluded as it has separate validation rules.
fn is_url_attribute(attr_name: &str, element_tag: &str, url_attrs: &[(&str, &str)]) -> bool {
	// Exclude img src - it has separate validation rules
	if element_tag == "img" && attr_name == "src" {
		return false;
	}

	for (url_attr, applicable_tags) in url_attrs {
		if attr_name == *url_attr {
			for tag in applicable_tags.split(',').map(|s| s.trim()) {
				if tag == element_tag {
					return true;
				}
			}
		}
	}
	false
}

/// Validates enumerated attribute values.
///
/// Checks if a string literal attribute value is one of the allowed values
/// for enumerated attributes (like `input[type]`, `button[type]`, etc.).
///
/// # Parameters
///
/// * `attr_name` - The attribute name
/// * `value` - The attribute value
/// * `element_tag` - The element tag name
/// * `span` - The span for error reporting
///
/// # Returns
///
/// Ok if validation passes, Err with descriptive error message otherwise.
fn validate_enum_attr(
	attr_name: &str,
	value: &AttrValue,
	element_tag: &str,
	span: Span,
) -> Result<()> {
	let Some(enum_spec) = super::html_spec::get_enum_attr_spec(element_tag, attr_name) else {
		return Ok(());
	};

	let Some(str_value) = value.as_string() else {
		return Ok(()); // Dynamic expressions are OK
	};

	if !enum_spec.valid_values.contains(&str_value.as_str()) {
		// Use .first() for safe access instead of direct indexing
		let example_value = enum_spec.valid_values.first().copied().unwrap_or("...");
		return Err(syn::Error::new(
			span,
			format!(
				"Invalid value '{}' for attribute '{}' on element <{}>.\n\
				Valid values are: {}\n\n\
				Examples:\n\
				  Correct:   {}=\"{}\"\n\
				  Incorrect: {}=\"{}\"",
				str_value,
				attr_name,
				element_tag,
				enum_spec.valid_values.join("\", \""),
				attr_name,
				example_value,
				attr_name,
				str_value
			),
		));
	}

	Ok(())
}

/// Checks if children nodes contain meaningful content.
///
/// Returns true if any child contains non-whitespace text or is a dynamic expression.
fn has_meaningful_content(children: &[TypedPageNode]) -> bool {
	for child in children {
		match child {
			TypedPageNode::Text(text) => {
				if !text.content.trim().is_empty() {
					return true;
				}
			}
			TypedPageNode::Element(elem) => {
				if has_meaningful_content(&elem.children) {
					return true;
				}
			}
			TypedPageNode::Expression(_)
			| TypedPageNode::Component(_)
			| TypedPageNode::If(_)
			| TypedPageNode::For(_)
			| TypedPageNode::Watch(_) => {
				// Dynamic content - assume it will have meaningful content at runtime
				return true;
			}
		}
	}
	false
}

/// Validates button element accessibility requirements.
///
/// Button elements must have either:
/// - Text content (direct or nested)
/// - aria-label attribute
/// - aria-labelledby attribute
fn validate_button_accessibility(
	attrs: &[TypedPageAttr],
	children: &[TypedPageNode],
	span: Span,
) -> Result<()> {
	// Check for aria-label or aria-labelledby attributes
	let has_aria_label = attrs.iter().any(|attr| {
		let name = attr.name.to_string();
		name == "aria_label" || name == "aria_labelledby"
	});

	if has_aria_label {
		return Ok(());
	}

	// Check for text content
	if !has_meaningful_content(children) {
		return Err(syn::Error::new(
			span,
			"Element <button> requires accessible text.\n\
			Either provide text content or use 'aria_label' attribute.\n\n\
			Examples:\n\
			  Correct:   button { \"Click me\" }\n\
			  Correct:   button { aria_label: \"Close\" }\n\
			  Correct:   button { span { \"Submit\" } }\n\
			  Incorrect: button {}",
		));
	}

	Ok(())
}

/// Validates accessibility requirements for elements.
///
/// Currently validates:
/// - button elements: Must have text content or aria-label
fn validate_accessibility(
	tag: &str,
	attrs: &[TypedPageAttr],
	children: &[TypedPageNode],
	span: Span,
) -> Result<()> {
	if tag == "button" {
		validate_button_accessibility(attrs, children, span)?
	}
	Ok(())
}

/// Validates attribute type for specific elements and attributes.
///
/// # Validation Rules
///
/// - Boolean attributes must have dynamic expressions only (no literals)
/// - Numeric attributes must have integer literals or dynamic expressions (no strings/floats/booleans)
/// - URL attributes are checked for dangerous schemes (javascript:, data:, vbscript:) for XSS prevention
/// - Enumerated attributes are validated against allowed values (`input[type]`, `button[type]`, etc.)
/// - `img` element `src` attribute must be a string literal and non-empty
///
/// Future phases will add accessibility checks.
fn validate_attr_type(
	attr_name: &str,
	value: &AttrValue,
	element_tag: &str,
	span: Span,
) -> Result<()> {
	// Boolean attributes validation - must use dynamic expressions only
	const BOOLEAN_ATTRS: &[&str] = &[
		"disabled",
		"required",
		"readonly",
		"checked",
		"selected",
		"autofocus",
		"autoplay",
		"controls",
		"loop",
		"muted",
		"default",
		"defer",
		"formnovalidate",
		"hidden",
		"ismap",
		"multiple",
		"novalidate",
		"open",
		"reversed",
	];

	// Numeric attributes that must have integer literal or dynamic values
	const NUMERIC_ATTRS: &[&str] = &[
		"maxlength",
		"minlength",
		"size",
		"min",
		"max",
		"step",
		"rows",
		"cols",
		"colspan",
		"rowspan",
		"tabindex",
	];

	// URL attributes that should be validated for dangerous schemes
	// Each entry is (attribute_name, applicable_element_tags)
	const URL_ATTRS: &[(&str, &str)] = &[
		("href", "a, area, link"),
		("action", "form"),
		("formaction", "button, input"),
		("src", "iframe, video, audio, source, script, embed"),
	];

	if BOOLEAN_ATTRS.contains(&attr_name) {
		// 1. String literals are prohibited
		if value.is_string_literal() {
			return Err(syn::Error::new(
				span,
				format!(
					"Boolean attribute '{}' cannot have a string literal value.\n\
					HTML boolean attributes represent true/false by their presence/absence:\n\
					  - Attribute present = true\n\
					  - Attribute absent = false\n\n\
					Use a variable or expression for dynamic boolean values:\n\
					  Correct:   {}: is_disabled\n\
					  Correct:   {}: state.is_active()\n\
					  Incorrect: {}: \"true\"\n\
					  Incorrect: {}: \"disabled\"",
					attr_name, attr_name, attr_name, attr_name, attr_name
				),
			));
		}

		// 2. Boolean literals are prohibited
		if value.is_bool_literal() {
			return Err(syn::Error::new(
				span,
				format!(
					"Boolean attribute '{}' cannot have a boolean literal value.\n\
					HTML boolean attributes represent true/false by their presence/absence:\n\
					  - Attribute present = true\n\
					  - Attribute absent = false\n\n\
					Use a variable or expression for dynamic boolean values:\n\
					  Correct:   {}: is_disabled\n\
					  Correct:   {}: state.is_active()\n\
					  Incorrect: {}: true\n\
					  Incorrect: {}: false",
					attr_name, attr_name, attr_name, attr_name, attr_name
				),
			));
		}

		// 3. Numeric literals are prohibited
		if value.is_int_literal() || value.is_float_literal() {
			return Err(syn::Error::new(
				span,
				format!(
					"Boolean attribute '{}' cannot have a numeric literal value.\n\
					HTML boolean attributes represent true/false by their presence/absence:\n\
					  - Attribute present = true\n\
					  - Attribute absent = false\n\n\
					Use a variable or expression for dynamic boolean values:\n\
					  Correct:   {}: is_disabled\n\
					  Correct:   {}: state.is_active()\n\
					  Incorrect: {}: 1\n\
					  Incorrect: {}: 0",
					attr_name, attr_name, attr_name, attr_name, attr_name
				),
			));
		}

		// 4. Dynamic expressions are OK (no check needed)
	}

	// Numeric attributes validation - must be integer literal or dynamic
	if NUMERIC_ATTRS.contains(&attr_name) {
		match value {
			AttrValue::StringLit(_) => {
				return Err(syn::Error::new(
					span,
					format!(
						"Attribute '{}' must be an integer literal or dynamic expression, not a string literal.\n\n\
						Examples:\n\
						  Incorrect: {}=\"100\"  // String literal is not allowed\n\
						  Correct:   {}=100      // Integer literal\n\
						  Correct:   {}=max_len  // Dynamic expression",
						attr_name, attr_name, attr_name, attr_name
					),
				));
			}
			AttrValue::FloatLit(_) => {
				return Err(syn::Error::new(
					span,
					format!(
						"Attribute '{}' must be an integer, not a floating-point number.\n\n\
						Examples:\n\
						  Incorrect: {}=100.0   // Float literal is not allowed\n\
						  Correct:   {}=100     // Integer literal",
						attr_name, attr_name, attr_name
					),
				));
			}
			AttrValue::BoolLit(_) => {
				return Err(syn::Error::new(
					span,
					format!(
						"Attribute '{}' must be an integer, not a boolean.\n\n\
						Use an integer literal or dynamic expression instead.",
						attr_name
					),
				));
			}
			AttrValue::IntLit(_) | AttrValue::Dynamic(_) => {
				// OK: Integer literal or dynamic expression
			}
		}
	}

	// URL attributes safety check - prevent XSS attacks
	if is_url_attribute(attr_name, element_tag, URL_ATTRS)
		&& let Some(url_str) = value.as_string()
	{
		// Check for empty strings first
		if url_str.trim().is_empty() {
			return Err(syn::Error::new(
				span,
				format!(
					"URL attribute '{}' cannot be empty.\n\n\
						Provide a valid URL or use a dynamic expression.",
					attr_name
				),
			));
		}

		// Check for dangerous schemes (case-insensitive) using is_safe_url from reinhardt-core
		// Fixes #849
		if !is_safe_url(&url_str) {
			return Err(syn::Error::new(
				span,
				format!(
					"Dangerous URL scheme detected in attribute '{}'.\n\
						The URL scheme can be used for XSS (Cross-Site Scripting) attacks.\n\n\
						Security risk: This URL could execute arbitrary JavaScript code.\n\n\
						Use safe URL schemes instead:\n\
						  - https://example.com\n\
						  - /path/to/resource\n\
						  - #anchor\n\
						  - mailto:user@example.com",
					attr_name
				),
			));
		}
	}
	// Dynamic expressions are OK (runtime validation recommended)

	// Enumerated attributes validation - check if value is in allowed list
	validate_enum_attr(attr_name, value, element_tag, span)?;

	// img element src attribute validation
	// Fixes #849
	if element_tag == "img" && attr_name == "src" {
		// Must be a string literal
		if !value.is_string_literal() {
			return Err(syn::Error::new(
				span,
				"Element <img> 'src' attribute must be a string literal",
			));
		}

		// Must not be empty
		if let Some(src_value) = value.as_string() {
			if src_value.trim().is_empty() {
				return Err(syn::Error::new(
					span,
					"Element <img> 'src' attribute must not be empty",
				));
			}

			// Check for dangerous URL schemes (XSS prevention) using is_safe_url from reinhardt-core
			// Fixes #849
			if !is_safe_url(&src_value) {
				return Err(syn::Error::new(
					span,
					"Dangerous URL scheme detected in <img> 'src' attribute.\n\
					The URL scheme can be used for XSS (Cross-Site Scripting) attacks.\n\n\
					Security risk: This URL could execute arbitrary JavaScript code.\n\n\
					Use safe URL schemes instead:\n\
					  - https://example.com/image.png\n\
					  - /path/to/image.png\n\
					  - image.png",
				));
			}
		}
	}

	Ok(())
}

/// Validates event handlers.
///
/// # Rules
///
/// - If the handler is a closure, it must have 0 or 1 arguments
/// - Other expressions (variable references, Callback::new(), etc.) are allowed
///   and will be type-checked by the Rust compiler
///
/// # Errors
///
/// Returns a compilation error if the handler is a closure with more than 1 argument.
fn validate_event_handler(event: &PageEvent) -> Result<()> {
	// Only validate argument count for closure expressions
	// Other expressions (variables, method calls, etc.) are allowed
	// and will be type-checked by the Rust compiler
	if let Expr::Closure(closure) = &event.handler {
		let arg_count = closure.inputs.len();
		if arg_count > 1 {
			return Err(syn::Error::new_spanned(
				&event.handler,
				format!(
					"Event handler closure must have 0 or 1 arguments, but this closure has {} arguments",
					arg_count
				),
			));
		}
	}

	// All other expression types are allowed:
	// - Variable references: let handler = |_| {}; @click: handler
	// - Callback::new(): @click: Callback::new(|_| {})
	// - Method calls: @click: handler.clone()
	// Type checking for these will be done by the Rust compiler

	Ok(())
}

/// Validates attribute naming and values.
///
/// # Rules
///
/// - data-* attributes must match pattern: `data-[a-z][a-z0-9-]*`
/// - aria-* attributes must match pattern: `aria-[a-z-]+`
///
/// # Errors
///
/// Returns an error if naming conventions are violated.
fn validate_attribute(attr: &PageAttr, _element_tag: &str) -> Result<()> {
	let attr_name = attr.name.to_string();

	// Validate data-* attributes
	if attr_name.starts_with("data_") {
		let html_name = attr.html_name();
		// Check if all characters after "data-" are lowercase letters, digits, or hyphens
		let suffix = &html_name[5..]; // Skip "data-"
		if suffix.is_empty()
			|| !suffix
				.chars()
				.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
		{
			return Err(syn::Error::new_spanned(
				&attr.name,
				format!(
					"Invalid data attribute name '{}'. Must match pattern: data-[a-z][a-z0-9-]*",
					html_name
				),
			));
		}
		// Additionally check first character is lowercase letter
		if !suffix.chars().next().unwrap().is_ascii_lowercase() {
			return Err(syn::Error::new_spanned(
				&attr.name,
				format!(
					"Invalid data attribute name '{}'. Must start with a lowercase letter after 'data-'",
					html_name
				),
			));
		}
	}

	// Validate aria-* attributes
	if attr_name.starts_with("aria_") {
		let html_name = attr.html_name();
		// Check if all characters after "aria-" are lowercase letters or hyphens
		let suffix = &html_name[5..]; // Skip "aria-"
		if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
			return Err(syn::Error::new_spanned(
				&attr.name,
				format!(
					"Invalid aria attribute name '{}'. Must match pattern: aria-[a-z-]+",
					html_name
				),
			));
		}
	}

	Ok(())
}

/// Validates element nesting rules.
///
/// # Rules
///
/// - Void elements (br, hr, img, input, etc.) cannot have children
/// - Interactive elements (button, a, label, etc.) cannot be nested inside other interactive elements
///
/// # Errors
///
/// Returns an error if nesting rules are violated.
fn validate_element_nesting(elem: &PageElement, parent_tags: &[String]) -> Result<()> {
	let tag = elem.tag.to_string();

	// Check if void element has children
	if elem.is_void() && !elem.children.is_empty() {
		return Err(syn::Error::new_spanned(
			&elem.tag,
			format!("Void element <{}> cannot have children", tag),
		));
	}

	// Check interactive element nesting
	let interactive_elements = ["button", "a", "label", "select", "textarea"];
	if interactive_elements.contains(&tag.as_str()) {
		for parent_tag in parent_tags {
			if interactive_elements.contains(&parent_tag.as_str()) {
				return Err(syn::Error::new_spanned(
					&elem.tag,
					format!(
						"Interactive element <{}> cannot be nested inside another interactive element <{}>",
						tag, parent_tag
					),
				));
			}
		}
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use syn::parse_quote;

	#[test]
	fn test_validate_valid_closure() {
		let event = PageEvent {
			event_type: syn::Ident::new("click", proc_macro2::Span::call_site()),
			handler: parse_quote!(|_| {}),
			span: proc_macro2::Span::call_site(),
		};
		assert!(validate_event_handler(&event).is_ok());
	}

	#[test]
	fn test_validate_closure_with_one_arg() {
		let event = PageEvent {
			event_type: syn::Ident::new("click", proc_macro2::Span::call_site()),
			handler: parse_quote!(|e| {
				handle_click(e);
			}),
			span: proc_macro2::Span::call_site(),
		};
		assert!(validate_event_handler(&event).is_ok());
	}

	#[test]
	fn test_validate_closure_too_many_args() {
		let event = PageEvent {
			event_type: syn::Ident::new("click", proc_macro2::Span::call_site()),
			handler: parse_quote!(|a, b, c| {}),
			span: proc_macro2::Span::call_site(),
		};
		let result = validate_event_handler(&event);
		assert!(result.is_err());
		assert!(result.unwrap_err().to_string().contains("0 or 1 arguments"));
	}

	#[test]
	fn test_validate_valid_data_attribute() {
		let attr = PageAttr {
			name: syn::Ident::new("data_testid", proc_macro2::Span::call_site()),
			value: parse_quote!("test"),
			span: proc_macro2::Span::call_site(),
		};
		assert!(validate_attribute(&attr, "div").is_ok());
	}

	#[test]
	fn test_validate_invalid_data_attribute() {
		let attr = PageAttr {
			name: syn::Ident::new("data_", proc_macro2::Span::call_site()),
			value: parse_quote!("test"),
			span: proc_macro2::Span::call_site(),
		};
		let result = validate_attribute(&attr, "div");
		assert!(result.is_err());
	}

	#[test]
	fn test_validate_valid_aria_attribute() {
		let attr = PageAttr {
			name: syn::Ident::new("aria_label", proc_macro2::Span::call_site()),
			value: parse_quote!("Navigation"),
			span: proc_macro2::Span::call_site(),
		};
		assert!(validate_attribute(&attr, "div").is_ok());
	}

	#[test]
	fn test_void_element_with_children() {
		let mut elem = PageElement::new(
			syn::Ident::new("input", proc_macro2::Span::call_site()),
			proc_macro2::Span::call_site(),
		);
		elem.children
			.push(PageNode::Text(reinhardt_manouche::core::PageText {
				content: "text".to_string(),
				span: proc_macro2::Span::call_site(),
			}));

		let result = validate_element_nesting(&elem, &[]);
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("cannot have children")
		);
	}

	#[test]
	fn test_nested_interactive_elements() {
		let elem = PageElement::new(
			syn::Ident::new("button", proc_macro2::Span::call_site()),
			proc_macro2::Span::call_site(),
		);

		let result = validate_element_nesting(&elem, &["a".to_string()]);
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("cannot be nested inside another interactive element")
		);
	}

	#[test]
	fn test_transform_attrs_with_string_lit() {
		let attrs = vec![PageAttr {
			name: syn::Ident::new("src", proc_macro2::Span::call_site()),
			value: parse_quote!("/image.png"),
			span: proc_macro2::Span::call_site(),
		}];

		let result = transform_attrs(&attrs, "img");
		assert!(result.is_ok());
		let typed_attrs = result.unwrap();
		assert_eq!(typed_attrs.len(), 1);
		assert!(typed_attrs[0].value.is_string_literal());
	}

	#[test]
	fn test_transform_attrs_with_dynamic() {
		let attrs = vec![PageAttr {
			name: syn::Ident::new("src", proc_macro2::Span::call_site()),
			value: parse_quote!(image_url),
			span: proc_macro2::Span::call_site(),
		}];

		let result = transform_attrs(&attrs, "div");
		assert!(result.is_ok());
		let typed_attrs = result.unwrap();
		assert_eq!(typed_attrs.len(), 1);
		assert!(typed_attrs[0].value.is_dynamic());
	}

	#[test]
	fn test_validate_attr_type_img_src_literal() {
		let value = AttrValue::from_expr(parse_quote!("/image.png"));
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_attr_type_img_src_dynamic() {
		let value = AttrValue::from_expr(parse_quote!(image_url));
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("must be a string literal")
		);
	}

	#[test]
	fn test_validate_attr_type_img_src_empty() {
		let value = AttrValue::from_expr(parse_quote!(""));
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.to_string()
				.contains("must not be empty")
		);
	}

	// Boolean attribute tests - string literals are prohibited
	#[test]
	fn test_validate_boolean_attr_string_literal() {
		let value = AttrValue::from_expr(parse_quote!("disabled"));
		let result =
			validate_attr_type("disabled", &value, "button", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Boolean attribute"));
		assert!(err_msg.contains("cannot have a string literal value"));
	}

	#[test]
	fn test_validate_boolean_attr_string_empty() {
		let value = AttrValue::from_expr(parse_quote!(""));
		let result =
			validate_attr_type("disabled", &value, "button", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Boolean attribute"));
		assert!(err_msg.contains("cannot have a string literal value"));
	}

	#[test]
	fn test_validate_boolean_attr_bool_literal_true() {
		let value = AttrValue::from_expr(parse_quote!(true));
		let result =
			validate_attr_type("disabled", &value, "button", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Boolean attribute"));
		assert!(err_msg.contains("cannot have a boolean literal value"));
	}

	#[test]
	fn test_validate_boolean_attr_bool_literal_false() {
		let value = AttrValue::from_expr(parse_quote!(false));
		let result = validate_attr_type("checked", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Boolean attribute"));
		assert!(err_msg.contains("cannot have a boolean literal value"));
	}

	#[test]
	fn test_validate_boolean_attr_int_literal() {
		let value = AttrValue::from_expr(parse_quote!(1));
		let result =
			validate_attr_type("disabled", &value, "button", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Boolean attribute"));
		assert!(err_msg.contains("cannot have a numeric literal value"));
	}

	#[test]
	fn test_validate_boolean_attr_float_literal() {
		let value = AttrValue::from_expr(parse_quote!(1.0));
		let result =
			validate_attr_type("required", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Boolean attribute"));
		assert!(err_msg.contains("cannot have a numeric literal value"));
	}

	#[test]
	fn test_validate_boolean_attr_dynamic_variable() {
		let value = AttrValue::from_expr(parse_quote!(is_disabled));
		let result =
			validate_attr_type("disabled", &value, "button", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_boolean_attr_dynamic_function() {
		let value = AttrValue::from_expr(parse_quote!(is_disabled()));
		let result = validate_attr_type("checked", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_boolean_attr_dynamic_conditional() {
		let value = AttrValue::from_expr(parse_quote!(if condition { true } else { false }));
		let result =
			validate_attr_type("readonly", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	// Numeric attribute tests - string and float literals are prohibited
	#[test]
	fn test_validate_numeric_attr_string_literal() {
		let value = AttrValue::from_expr(parse_quote!("100"));
		let result =
			validate_attr_type("maxlength", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Attribute"));
		assert!(err_msg.contains("must be an integer literal or dynamic expression"));
	}

	#[test]
	fn test_validate_numeric_attr_float_literal() {
		let value = AttrValue::from_expr(parse_quote!(100.0));
		let result = validate_attr_type("rows", &value, "textarea", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Attribute"));
		assert!(err_msg.contains("must be an integer, not a floating-point number"));
	}

	#[test]
	fn test_validate_numeric_attr_bool_literal() {
		let value = AttrValue::from_expr(parse_quote!(true));
		let result = validate_attr_type("min", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Attribute"));
		assert!(err_msg.contains("must be an integer, not a boolean"));
	}

	#[test]
	fn test_validate_numeric_attr_int_literal() {
		let value = AttrValue::from_expr(parse_quote!(100));
		let result =
			validate_attr_type("maxlength", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_numeric_attr_dynamic_variable() {
		let value = AttrValue::from_expr(parse_quote!(max_len));
		let result =
			validate_attr_type("maxlength", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_numeric_attr_dynamic_function() {
		let value = AttrValue::from_expr(parse_quote!(get_max_len()));
		let result = validate_attr_type("cols", &value, "textarea", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	// URL attribute tests - dangerous schemes and empty strings are prohibited
	#[test]
	fn test_validate_url_attr_javascript_scheme() {
		let value = AttrValue::from_expr(parse_quote!("javascript:alert('xss')"));
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
		assert!(err_msg.contains("XSS"));
	}

	#[test]
	fn test_validate_url_attr_data_scheme() {
		let value =
			AttrValue::from_expr(parse_quote!("data:text/html,<script>alert('xss')</script>"));
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
	}

	#[test]
	fn test_validate_url_attr_vbscript_scheme() {
		let value = AttrValue::from_expr(parse_quote!("vbscript:msgbox('xss')"));
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
	}

	#[test]
	fn test_validate_url_attr_case_insensitive() {
		let value = AttrValue::from_expr(parse_quote!("JavaScript:alert(1)"));
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
	}

	#[test]
	fn test_validate_url_attr_empty_string() {
		let value = AttrValue::from_expr(parse_quote!(""));
		let result = validate_attr_type("action", &value, "form", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("cannot be empty"));
	}

	#[test]
	fn test_validate_url_attr_whitespace_string() {
		let value = AttrValue::from_expr(parse_quote!("   "));
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("cannot be empty"));
	}

	#[test]
	fn test_validate_url_attr_https_scheme() {
		let value = AttrValue::from_expr(parse_quote!("https://example.com"));
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_url_attr_relative_path() {
		let value = AttrValue::from_expr(parse_quote!("/path/to/page"));
		let result = validate_attr_type("action", &value, "form", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_url_attr_anchor() {
		let value = AttrValue::from_expr(parse_quote!("#section"));
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_url_attr_dynamic_variable() {
		let value = AttrValue::from_expr(parse_quote!(url_var));
		let result = validate_attr_type("href", &value, "a", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_url_attr_img_src_dangerous_scheme() {
		// img src should be validated for dangerous URL schemes (Fixes #849)
		let value = AttrValue::from_expr(parse_quote!("javascript:alert(1)"));
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
	}

	#[test]
	fn test_validate_url_attr_img_src_data_scheme() {
		let value =
			AttrValue::from_expr(parse_quote!("data:text/html,<script>alert('xss')</script>"));
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
	}

	#[test]
	fn test_validate_url_attr_img_src_vbscript_scheme() {
		let value = AttrValue::from_expr(parse_quote!("vbscript:alert(1)"));
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Dangerous URL scheme"));
	}

	#[test]
	fn test_validate_url_attr_img_src_safe_url() {
		let value = AttrValue::from_expr(parse_quote!("/images/photo.png"));
		let result = validate_attr_type("src", &value, "img", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	// Enumerated attribute tests - invalid values are prohibited
	#[test]
	fn test_validate_enum_attr_input_type_invalid() {
		let value = AttrValue::from_expr(parse_quote!("invalid"));
		let result = validate_attr_type("type", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Invalid value"));
		assert!(err_msg.contains("invalid"));
		assert!(err_msg.contains("input"));
	}

	#[test]
	fn test_validate_enum_attr_input_type_text() {
		let value = AttrValue::from_expr(parse_quote!("text"));
		let result = validate_attr_type("type", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_enum_attr_input_type_email() {
		let value = AttrValue::from_expr(parse_quote!("email"));
		let result = validate_attr_type("type", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_enum_attr_button_type_invalid() {
		let value = AttrValue::from_expr(parse_quote!("invalid"));
		let result = validate_attr_type("type", &value, "button", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Invalid value"));
		assert!(err_msg.contains("button"));
	}

	#[test]
	fn test_validate_enum_attr_button_type_submit() {
		let value = AttrValue::from_expr(parse_quote!("submit"));
		let result = validate_attr_type("type", &value, "button", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_enum_attr_form_method_invalid() {
		let value = AttrValue::from_expr(parse_quote!("put"));
		let result = validate_attr_type("method", &value, "form", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Invalid value"));
		assert!(err_msg.contains("put"));
	}

	#[test]
	fn test_validate_enum_attr_form_method_post() {
		let value = AttrValue::from_expr(parse_quote!("post"));
		let result = validate_attr_type("method", &value, "form", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_enum_attr_form_enctype_invalid() {
		let value = AttrValue::from_expr(parse_quote!("invalid"));
		let result = validate_attr_type("enctype", &value, "form", proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("Invalid value"));
	}

	#[test]
	fn test_validate_enum_attr_form_enctype_multipart() {
		let value = AttrValue::from_expr(parse_quote!("multipart/form-data"));
		let result = validate_attr_type("enctype", &value, "form", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_enum_attr_script_type_module() {
		let value = AttrValue::from_expr(parse_quote!("module"));
		let result = validate_attr_type("type", &value, "script", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_enum_attr_dynamic_variable() {
		let value = AttrValue::from_expr(parse_quote!(input_type));
		let result = validate_attr_type("type", &value, "input", proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	// Accessibility tests - button elements must have text or aria-label
	#[test]
	fn test_validate_button_accessibility_empty() {
		let attrs = vec![];
		let children = vec![];
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("requires accessible text"));
		assert!(err_msg.contains("aria_label"));
	}

	#[test]
	fn test_validate_button_accessibility_whitespace_only() {
		use reinhardt_manouche::core::{PageText, TypedPageNode};
		let attrs = vec![];
		let children = vec![TypedPageNode::Text(PageText {
			content: "   ".to_string(),
			span: proc_macro2::Span::call_site(),
		})];
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());
		assert!(result.is_err());
	}

	#[test]
	fn test_validate_button_accessibility_with_text() {
		use reinhardt_manouche::core::{PageText, TypedPageNode};
		let attrs = vec![];
		let children = vec![TypedPageNode::Text(PageText {
			content: "Click me".to_string(),
			span: proc_macro2::Span::call_site(),
		})];
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_button_accessibility_with_nested_text() {
		use reinhardt_manouche::core::{PageText, TypedPageElement, TypedPageNode};
		let attrs = vec![];
		let children = vec![TypedPageNode::Element(TypedPageElement {
			tag: syn::Ident::new("span", proc_macro2::Span::call_site()),
			attrs: vec![],
			events: vec![],
			children: vec![TypedPageNode::Text(PageText {
				content: "Submit".to_string(),
				span: proc_macro2::Span::call_site(),
			})],
			span: proc_macro2::Span::call_site(),
		})];
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_button_accessibility_with_aria_label() {
		use reinhardt_manouche::core::TypedPageAttr;
		let attrs = vec![TypedPageAttr {
			name: syn::Ident::new("aria_label", proc_macro2::Span::call_site()),
			value: AttrValue::from_expr(parse_quote!("Close")),
			span: proc_macro2::Span::call_site(),
		}];
		let children = vec![];
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}

	#[test]
	fn test_validate_button_accessibility_with_expression() {
		use reinhardt_manouche::core::{PageExpression, TypedPageNode};
		let attrs = vec![];
		let children = vec![TypedPageNode::Expression(PageExpression {
			expr: parse_quote!(button_text),
			braced: true,
			span: proc_macro2::Span::call_site(),
		})];
		let result =
			validate_button_accessibility(&attrs, &children, proc_macro2::Span::call_site());
		assert!(result.is_ok());
	}
}
