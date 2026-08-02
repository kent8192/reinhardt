//! Parser for the page! macro DSL.
//!
//! This module implements parsing logic to convert TokenStream into AST nodes.
//!
//! ## Parsing Strategy
//!
//! The parser uses `syn`'s parsing infrastructure with custom parsing logic for:
//! - Closure-style parameters: `|name: Type, ...|`
//! - Element syntax: `tag { ... }`
//! - Attribute syntax: `key: value,`
//! - Event syntax: `@event: handler,`
//! - Child nodes: nested elements, text, expressions
//! - Component invocation (spec §3.5):
//!   * `Component(arg: val)` — legacy positional / paren form
//!   * `Component { prop: val, @event: h, child { ... } }` — brace form
//!
//! Disambiguation: an identifier followed by `{` is routed to either the
//! element parser (lowercase tag) or the brace-component parser (PascalCase
//! identifier). See [`component_brace`] for the brace-body grammar.

mod component_brace;

use proc_macro2::TokenStream;
use syn::{
	Expr, Ident, LitStr, Pat, Result, Token, Type, braced,
	ext::IdentExt,
	parenthesized,
	parse::{Parse, ParseStream},
	token,
};

use crate::{
	ComponentInvocationForm, IntrinsicEvent, NamedSlot, PageAttr, PageBody, PageComponent,
	PageComponentArg, PageElement, PageElse, PageExpression, PageFor, PageIf, PageMacro,
	PageMacroForm, PageNode, PageParam, PageText, PageWatch,
};

/// Parses a `page!` macro invocation into an untyped AST.
///
/// # Errors
///
/// Returns a `syn::Error` if the input is not valid page! syntax.
pub fn parse_page(input: TokenStream) -> syn::Result<PageMacro> {
	syn::parse2(input)
}

impl Parse for PageMacro {
	fn parse(input: ParseStream) -> Result<Self> {
		let span = input.span();

		// Parse optional #head directive: #head: expr,
		let head = if input.peek(Token![#]) {
			input.parse::<Token![#]>()?;
			let directive_name: Ident = input.parse()?;
			if directive_name != "head" {
				return Err(syn::Error::new(
					directive_name.span(),
					format!("Unknown directive '#{}'. Expected '#head'.", directive_name),
				));
			}
			input.parse::<Token![:]>()?;
			let head_expr: Expr = input.parse()?;
			// Optional trailing comma
			if input.peek(Token![,]) {
				input.parse::<Token![,]>()?;
			}
			Some(head_expr)
		} else {
			None
		};

		let form = if input.peek(Token![|]) {
			let params = parse_closure_params(input)?;
			let body = input.parse::<PageBody>()?;
			PageMacroForm::StrictClosure { params, body }
		} else {
			let body = input.parse::<PageBody>()?;
			PageMacroForm::ImplicitBody { body }
		};

		Ok(Self { head, form, span })
	}
}

/// Parses closure-style parameters: `|name: Type, ...|`
fn parse_closure_params(input: ParseStream) -> Result<Vec<PageParam>> {
	input.parse::<Token![|]>()?;

	// Handle empty params: ||
	if input.peek(Token![|]) {
		input.parse::<Token![|]>()?;
		return Ok(Vec::new());
	}

	let mut params = Vec::new();

	loop {
		// Parse parameter: name: Type
		let name: Ident = input.parse()?;
		input.parse::<Token![:]>()?;
		let ty: syn::Type = input.parse()?;

		params.push(PageParam {
			span: name.span(),
			name,
			ty,
		});

		// Check for comma or closing |
		if input.peek(Token![,]) {
			input.parse::<Token![,]>()?;
			// Allow trailing comma before |
			if input.peek(Token![|]) {
				break;
			}
		} else {
			break;
		}
	}

	input.parse::<Token![|]>()?;

	Ok(params)
}

impl Parse for PageBody {
	fn parse(input: ParseStream) -> Result<Self> {
		let span = input.span();
		let content;
		braced!(content in input);

		let nodes = parse_nodes(&content)?;

		Ok(Self { nodes, span })
	}
}

/// Parses multiple nodes from a ParseStream.
fn parse_nodes(input: ParseStream) -> Result<Vec<PageNode>> {
	let mut nodes = Vec::new();

	while !input.is_empty() {
		nodes.push(parse_node(input)?);
	}

	Ok(nodes)
}

/// Returns `true` when an identifier starts with an ASCII uppercase letter.
///
/// Spec §3.5 routes such identifiers to the component parsers (paren or
/// brace form); lowercase identifiers are HTML elements.
fn is_pascal_case_ident(ident: &Ident) -> bool {
	ident
		.to_string()
		.chars()
		.next()
		.is_some_and(|c| c.is_ascii_uppercase())
}

/// Parses a single node from the input.
pub(super) fn parse_node(input: ParseStream) -> Result<PageNode> {
	// Check for string literal: "text"
	if input.peek(syn::LitStr) {
		return parse_text_node(input);
	}

	// Check for if expression
	if input.peek(Token![if]) {
		return parse_if_node(input);
	}

	// Check for for loop
	if input.peek(Token![for]) {
		return parse_for_node(input);
	}

	// Check for braced expression: { expr }
	if input.peek(token::Brace) {
		return parse_braced_expression(input);
	}

	// Check for identifier - could be watch, element, component, macro call, or expression
	if input.peek(Ident) {
		// Look ahead to see if it's watch, an element, component, or expression
		let fork = input.fork();
		let ident: Ident = fork.parse()?;

		// Check for watch keyword: watch { ... }
		if ident == "watch" && fork.peek(token::Brace) {
			return parse_watch_node(input);
		}

		if fork.peek(token::Brace) {
			// Disambiguate by case (spec §3.5):
			// - PascalCase + `{` → component brace-form invocation
			// - lowercase + `{` → HTML element
			if is_pascal_case_ident(&ident) {
				return component_brace::parse_component_brace_node(input);
			}
			return parse_element_node(input);
		} else if fork.peek(token::Paren) {
			// It's a component call: Component(args) or Component(args) { children }
			return parse_component_node(input);
		} else {
			// It's an expression: variable or method call
			return parse_expression_node(input);
		}
	}

	// Check for named slot syntax `$slotname { ... }` — only valid inside component body
	if input.peek(Token![$]) {
		return Err(syn::Error::new(
			input.span(),
			"`$slotname { }` is only valid inside a component body",
		));
	}

	// Otherwise, try to parse as a general expression
	parse_expression_node(input)
}

/// Parses a text literal node: "string"
fn parse_text_node(input: ParseStream) -> Result<PageNode> {
	let lit: syn::LitStr = input.parse()?;
	Ok(PageNode::Text(PageText {
		content: lit.value(),
		span: lit.span(),
	}))
}

/// Parses an element node: tag { attrs, events, children }
fn parse_element_node(input: ParseStream) -> Result<PageNode> {
	let tag: Ident = input.parse()?;
	let span = tag.span();

	let content;
	braced!(content in input);

	let mut element = PageElement::new(tag, span);

	// Parse contents: attrs, events, and children
	while !content.is_empty() {
		// Check for event handler: @event: handler
		if content.peek(Token![@]) {
			element.events.push(parse_event(&content)?);
			continue;
		}

		// Check for attribute: key: value,
		// Attributes are identified by: ident : expr ,
		// Use peek_any to also match reserved keywords (type, for, etc.)
		if content.peek(Ident::peek_any) {
			let fork = content.fork();
			let ident = Ident::parse_any(&fork)?;

			// If followed by :, it's an attribute
			if fork.peek(Token![:]) {
				// But not if followed by { (that's a child element)
				let fork2 = fork.fork();
				fork2.parse::<Token![:]>()?;

				// Check if this is a shorthand attr or an element
				// After parsing the value, if there's a , it's definitely an attr
				// We need to check if what follows : is an expression or a block
				element.attrs.push(parse_attr(&content)?);
				continue;
			}

			// Standalone boolean attribute (e.g., `required`, `disabled`)
			// Must not be followed by { (element) or ( (component call)
			let ident_str = ident.to_string();
			if crate::core::attr_utils::BOOLEAN_ATTRS.contains(&ident_str.as_str())
				&& !fork.peek(token::Brace)
				&& !fork.peek(token::Paren)
			{
				element.attrs.push(parse_standalone_boolean_attr(&content)?);
				continue;
			}
		}

		// Otherwise, it's a child node
		element.children.push(parse_node(&content)?);
	}

	Ok(PageNode::Element(element))
}

/// Parses a standalone boolean attribute: `required`, `disabled`, etc.
///
/// Generates a synthetic `true` literal as the attribute value.
fn parse_standalone_boolean_attr(input: ParseStream) -> Result<PageAttr> {
	let name = Ident::parse_any(input)?;
	let span = name.span();

	// Consume optional trailing comma
	if input.peek(Token![,]) {
		input.parse::<Token![,]>()?;
	}

	// Create synthetic `true` literal value
	let value = Expr::Lit(syn::ExprLit {
		attrs: vec![],
		lit: syn::Lit::Bool(syn::LitBool::new(true, span)),
	});

	Ok(PageAttr { name, value, span })
}

/// Parses an attribute: `name: value,`
fn parse_attr(input: ParseStream) -> Result<PageAttr> {
	let name = Ident::parse_any(input)?;
	let span = name.span();
	input.parse::<Token![:]>()?;

	// Parse attribute value as expression
	// Macro calls like `asset!("path")` are valid Expr::Macro
	let value: Expr = input.parse()?;

	// Consume optional trailing comma
	if input.peek(Token![,]) {
		input.parse::<Token![,]>()?;
	}

	Ok(PageAttr { name, value, span })
}

/// Parses an event handler: `@event: handler,`
fn parse_event(input: ParseStream) -> Result<IntrinsicEvent> {
	input.parse::<Token![@]>()?;
	let event_name: Ident = input.parse()?;

	if event_name == "custom" {
		let payload_type = if input.peek(Token![::]) {
			input.parse::<Token![::]>()?;
			input.parse::<Token![<]>()?;
			if input.peek(Token![>]) {
				return Err(syn::Error::new(
					input.span(),
					"`@custom` requires exactly one payload type",
				));
			}
			let payload_type: Type = input.parse()?;
			if input.peek(Token![,]) {
				return Err(syn::Error::new(
					input.span(),
					"`@custom` accepts exactly one payload type",
				));
			}
			input.parse::<Token![>]>()?;
			Some(payload_type)
		} else {
			None
		};

		if !input.peek(token::Paren) {
			let syntax = if payload_type.is_some() {
				"`@custom::<T>` requires syntax `@custom::<T>(\"event-name\"): handler`"
			} else {
				"`@custom` requires syntax `@custom(\"event-name\"): handler`"
			};
			return Err(syn::Error::new(event_name.span(), syntax));
		}

		let content;
		parenthesized!(content in input);
		if !content.peek(LitStr) {
			return Err(syn::Error::new(
				content.span(),
				"`@custom` event name must be a string literal",
			));
		}
		let name: LitStr = content.parse()?;
		if !content.is_empty() {
			return Err(syn::Error::new(
				content.span(),
				"`@custom` accepts exactly one string literal event name",
			));
		}
		input.parse::<Token![:]>()?;
		let handler: Expr = input.parse()?;

		if input.peek(Token![,]) {
			input.parse::<Token![,]>()?;
		}

		return match payload_type {
			Some(payload_type) => Ok(IntrinsicEvent::TypedCustom {
				name,
				payload_type,
				handler,
			}),
			None => Ok(IntrinsicEvent::RawCustom { name, handler }),
		};
	}

	let event = reinhardt_event_catalog::event_spec(&event_name.to_string())
		.map(|spec| spec.kind)
		.ok_or_else(|| unknown_standard_event_error(&event_name))?;
	input.parse::<Token![:]>()?;
	let handler: Expr = input.parse()?;

	// Consume optional trailing comma
	if input.peek(Token![,]) {
		input.parse::<Token![,]>()?;
	}

	Ok(IntrinsicEvent::Standard { event, handler })
}

fn unknown_standard_event_error(name: &Ident) -> syn::Error {
	let name_text = name.to_string();
	let mut candidates: Vec<(usize, &'static str)> = reinhardt_event_catalog::EVENT_SPECS
		.iter()
		.map(|spec| (edit_distance(&name_text, spec.dom_name), spec.dom_name))
		.collect();
	candidates.sort_unstable();

	let max_distance = usize::max(2, name_text.chars().count() / 3);
	let suggestions: Vec<&str> = candidates
		.first()
		.filter(|(distance, _)| *distance <= max_distance)
		.map(|(minimum, _)| {
			candidates
				.iter()
				.take_while(|(distance, _)| distance == minimum)
				.take(3)
				.map(|(_, candidate)| *candidate)
				.collect()
		})
		.unwrap_or_default();

	let message = match suggestions.as_slice() {
		[] => format!("unknown standard element event `{name_text}`"),
		[suggestion] => {
			format!("unknown standard element event `{name_text}`; did you mean `{suggestion}`?")
		}
		_ => format!(
			"unknown standard element event `{name_text}`; did you mean one of {}?",
			suggestions
				.iter()
				.map(|suggestion| format!("`{suggestion}`"))
				.collect::<Vec<_>>()
				.join(", ")
		),
	};

	syn::Error::new(name.span(), message)
}

fn edit_distance(left: &str, right: &str) -> usize {
	let mut previous: Vec<usize> = (0..=right.chars().count()).collect();
	let mut current = vec![0; previous.len()];

	for (left_index, left_char) in left.chars().enumerate() {
		current[0] = left_index + 1;
		for (right_index, right_char) in right.chars().enumerate() {
			let substitution = previous[right_index] + usize::from(left_char != right_char);
			current[right_index + 1] = usize::min(
				usize::min(previous[right_index + 1] + 1, current[right_index] + 1),
				substitution,
			);
		}
		std::mem::swap(&mut previous, &mut current);
	}

	previous[right.chars().count()]
}

/// Parses a braced expression: `{ expr }` (or `{ stmts...; expr }`).
///
/// Uses `Block::parse_within` so block bodies with let-statements such as
/// `{ let x = ...; expr }` are accepted. When the body is a single trailing
/// expression with no statements, the inner Expr is returned directly to
/// preserve the legacy AST shape — wrapping it in `Expr::Block` would cause
/// downstream formatters to emit a redundant outer `{ ... }`.
fn parse_braced_expression(input: ParseStream) -> Result<PageNode> {
	let span = input.span();
	let content;
	let brace_token = braced!(content in input);
	// Accept a full block body (statements + trailing expression) so
	// interpolations such as `{ let x = ...; expr }` are valid. A single
	// expression remains valid because `Block::parse_within` accepts it as
	// a one-element statement list.
	let stmts = syn::Block::parse_within(&content)?;

	let expr = match stmts.as_slice() {
		// Single trailing expression (no trailing `;`): unwrap to preserve
		// the historical PageExpression shape so legacy formatters do not
		// see an extra `Expr::Block` wrapper.
		[syn::Stmt::Expr(inner, None)] => inner.clone(),
		_ => Expr::Block(syn::ExprBlock {
			attrs: Vec::new(),
			label: None,
			block: syn::Block { brace_token, stmts },
		}),
	};

	Ok(PageNode::Expression(PageExpression {
		expr,
		braced: true,
		span,
	}))
}

/// Parses an expression node (variable, method call, etc.)
fn parse_expression_node(input: ParseStream) -> Result<PageNode> {
	let span = input.span();
	let expr: Expr = input.parse()?;

	Ok(PageNode::Expression(PageExpression {
		expr,
		braced: false,
		span,
	}))
}

/// Parses an if node: `if condition { ... } else { ... }`
fn parse_if_node(input: ParseStream) -> Result<PageNode> {
	let span = input.span();
	input.parse::<Token![if]>()?;

	// Parse condition (everything until the brace)
	let condition = parse_condition(input)?;

	// Parse then branch
	let content;
	braced!(content in input);
	let then_branch = parse_nodes(&content)?;

	// Parse optional else branch
	let else_branch = if input.peek(Token![else]) {
		input.parse::<Token![else]>()?;

		if input.peek(Token![if]) {
			// else if
			let else_if = parse_if_node(input)?;
			match else_if {
				PageNode::If(if_node) => Some(PageElse::If(Box::new(if_node))),
				_ => unreachable!(),
			}
		} else {
			// else { ... }
			let content;
			braced!(content in input);
			let else_nodes = parse_nodes(&content)?;
			Some(PageElse::Block(else_nodes))
		}
	} else {
		None
	};

	Ok(PageNode::If(PageIf {
		condition,
		then_branch,
		else_branch,
		span,
	}))
}

/// Parses a condition expression (stops at opening brace).
fn parse_condition(input: ParseStream) -> Result<Expr> {
	// We need to parse an expression but stop before the {
	// This is tricky because Expr::parse would consume the block too
	// We use a technique of parsing tokens until we hit the brace

	let mut tokens = proc_macro2::TokenStream::new();

	while !input.is_empty() && !input.peek(token::Brace) {
		// Parse one token tree at a time
		let tt: proc_macro2::TokenTree = input.parse()?;
		tokens.extend(std::iter::once(tt));
	}

	syn::parse2(tokens)
}

/// Parses a for iterator expression (stops at `@key(...)` or opening brace).
fn parse_for_iter(input: ParseStream) -> Result<Expr> {
	let mut tokens = proc_macro2::TokenStream::new();

	while !input.is_empty() && !input.peek(token::Brace) && !input.peek(Token![@]) {
		let tt: proc_macro2::TokenTree = input.parse()?;
		tokens.extend(std::iter::once(tt));
	}

	syn::parse2(tokens)
}

/// Parses an optional keyed-list marker: `@key(expr)`.
fn parse_optional_for_key(input: ParseStream) -> Result<Option<Expr>> {
	if !input.peek(Token![@]) {
		return Ok(None);
	}

	input.parse::<Token![@]>()?;
	let key_ident: Ident = input.parse()?;
	if key_ident != "key" {
		return Err(syn::Error::new(
			key_ident.span(),
			"expected `key` after `@` in for loop",
		));
	}

	let content;
	parenthesized!(content in input);
	let key = content.parse::<Expr>()?;
	if !content.is_empty() {
		return Err(content.error("unexpected tokens after keyed for expression"));
	}

	Ok(Some(key))
}

/// Parses a for node: `for pat in iter [@key(expr)] { ... }`
fn parse_for_node(input: ParseStream) -> Result<PageNode> {
	let span = input.span();
	input.parse::<Token![for]>()?;

	// Parse pattern (use parse_single for single pattern without alternatives)
	let pat = Pat::parse_single(input)?;

	input.parse::<Token![in]>()?;

	// Parse iterator expression and optional stable key.
	let iter = parse_for_iter(input)?;
	let key = parse_optional_for_key(input)?;

	// Parse body
	let content;
	braced!(content in input);
	let body = parse_nodes(&content)?;

	Ok(PageNode::For(Box::new(PageFor {
		pat,
		iter,
		key,
		body,
		span,
	})))
}

/// Parses a watch node: `watch { expr }`
///
/// The watch block wraps an expression in a reactive context,
/// allowing Signal dependencies to be automatically tracked.
fn parse_watch_node(input: ParseStream) -> Result<PageNode> {
	let span = input.span();

	// Consume the "watch" identifier
	let watch_ident: Ident = input.parse()?;
	assert_eq!(
		watch_ident, "watch",
		"parser in wrong state: expected 'watch' identifier"
	);

	// Parse the braced content
	let content;
	braced!(content in input);

	// Parse the inner expression as a single node
	// The watch block must contain exactly one expression (if, match, etc.)
	let inner_node = parse_node(&content)?;

	// Ensure there's nothing else in the block
	if !content.is_empty() {
		return Err(content.error("watch block must contain exactly one expression"));
	}

	Ok(PageNode::Watch(PageWatch {
		expr: Box::new(inner_node),
		span,
	}))
}

/// Parses a component call: `Name(arg: value, ...) { children }`
///
/// # Example
///
/// ```text
/// MyButton(label: "Click", disabled: false)
/// MyWrapper(class: "container") {
///     p { "Child content" }
/// }
/// ```
fn parse_component_node(input: ParseStream) -> Result<PageNode> {
	let name: Ident = input.parse()?;
	let span = name.span();

	// Parse arguments in parentheses
	let args_content;
	parenthesized!(args_content in input);
	let args = parse_component_args(&args_content)?;

	// Parse optional body in braces (may contain default children and/or named slots)
	let (children, named_slots) = if input.peek(token::Brace) {
		let content;
		braced!(content in input);
		parse_component_body(&content)?
	} else {
		(None, Vec::new())
	};

	Ok(PageNode::Component(PageComponent {
		name,
		invocation_form: ComponentInvocationForm::Paren,
		args,
		// Paren form does not support inline event props; @event handlers
		// in this form are passed positionally like any other named arg.
		events: Vec::new(),
		children,
		named_slots,
		span,
	}))
}

/// Converts a DSL slot name to snake_case for duplicate detection.
fn slot_name_to_snake_case(name: &str) -> String {
	let mut result = String::with_capacity(name.len() + 4);
	for (i, ch) in name.chars().enumerate() {
		if ch.is_uppercase() {
			if i > 0 {
				result.push('_');
			}
			result.push(ch.to_ascii_lowercase());
		} else {
			result.push(ch);
		}
	}
	result
}
fn parse_component_body(input: ParseStream) -> Result<(Option<Vec<PageNode>>, Vec<NamedSlot>)> {
	let mut default_children: Vec<PageNode> = Vec::new();
	let mut named_slots: Vec<NamedSlot> = Vec::new();

	while !input.is_empty() {
		if input.peek(Token![$]) {
			input.parse::<Token![$]>()?;
			let name: Ident = input.parse()?;
			let span = name.span();

			// Compare snake_case-normalized names so that `$bodyContent` and
			// `$body_content` are detected as duplicates (both map to the same
			// builder setter `.body_content(...)`).
			let normalized = slot_name_to_snake_case(&name.to_string());
			if named_slots
				.iter()
				.any(|s: &NamedSlot| slot_name_to_snake_case(&s.name.to_string()) == normalized)
			{
				return Err(syn::Error::new(
					span,
					format!("duplicate named slot `{}` in component", name),
				));
			}

			let content;
			braced!(content in input);
			let children = parse_nodes(&content)?;

			named_slots.push(NamedSlot {
				name,
				children,
				span,
			});
		} else {
			default_children.push(parse_node(input)?);
		}
	}

	let children = if default_children.is_empty() {
		None
	} else {
		Some(default_children)
	};

	Ok((children, named_slots))
}

/// Parses component arguments: `name: value, ...`
fn parse_component_args(input: ParseStream) -> Result<Vec<PageComponentArg>> {
	let mut args = Vec::new();

	while !input.is_empty() {
		let name: Ident = input.parse()?;
		let span = name.span();
		input.parse::<Token![:]>()?;
		let value: Expr = input.parse()?;

		args.push(PageComponentArg { name, value, span });

		// Consume optional trailing comma
		if input.peek(Token![,]) {
			input.parse::<Token![,]>()?;
		}
	}

	Ok(args)
}

#[cfg(test)]
mod tests {
	use super::*;
	use quote::quote;
	use rstest::rstest;

	#[rstest]
	fn test_parse_empty_params() {
		// Arrange
		let input = quote!(|| { div { "hello" } });

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		assert!(result.params().is_empty());
		assert_eq!(result.body().nodes.len(), 1);
	}

	#[rstest]
	fn test_parse_body_only_as_implicit_body() {
		// Arrange
		let input = quote!({ div { "Hello" } });

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		assert!(result.is_implicit_body());
		assert!(matches!(result.form, PageMacroForm::ImplicitBody { .. }));
		assert!(result.params().is_empty());
		assert_eq!(result.body().nodes.len(), 1);
	}

	#[rstest]
	fn test_parse_empty_closure_as_strict_closure() {
		// Arrange
		let input = quote!(|| { div { "Static" } });

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		assert!(!result.is_implicit_body());
		assert!(matches!(result.form, PageMacroForm::StrictClosure { .. }));
		assert!(result.params().is_empty());
		assert_eq!(result.body().nodes.len(), 1);
	}

	#[rstest]
	fn test_parse_single_param() {
		// Arrange
		let input = quote!(|name: String| { div { "hello" } });

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		assert_eq!(result.params().len(), 1);
		assert_eq!(result.params()[0].name.to_string(), "name");
	}

	#[rstest]
	fn test_parse_multiple_params() {
		// Arrange
		let input = quote!(|name: String, count: i32| { div { "hello" } });

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		assert_eq!(result.params().len(), 2);
		assert_eq!(result.params()[0].name.to_string(), "name");
		assert_eq!(result.params()[1].name.to_string(), "count");
	}

	#[rstest]
	fn test_parse_simple_element() {
		// Arrange
		let input = quote!(|| { div { "hello" } });

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Element(elem) => {
				assert_eq!(elem.tag.to_string(), "div");
				assert_eq!(elem.children.len(), 1);
			}
			_ => panic!("expected Element"),
		}
	}

	#[rstest]
	fn test_parse_element_with_attrs() {
		// Arrange
		let input = quote!(|| {
			div {
				class: "container",
				id: "main",
				"hello"
			}
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Element(elem) => {
				assert_eq!(elem.attrs.len(), 2);
				assert_eq!(elem.attrs[0].name.to_string(), "class");
				assert_eq!(elem.attrs[1].name.to_string(), "id");
				assert_eq!(elem.children.len(), 1);
			}
			_ => panic!("expected Element"),
		}
	}

	#[rstest]
	fn test_parse_element_with_event() {
		// Arrange
		let input = quote!(|| {
			button {
				@click: |_| { handle_click(); },
				"Click me"
			}
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Element(elem) => {
				assert_eq!(elem.tag.to_string(), "button");
				assert_eq!(elem.events.len(), 1);
				assert!(matches!(
					&elem.events[0],
					crate::core::IntrinsicEvent::Standard {
						event: reinhardt_event_catalog::KnownEvent::Click,
						..
					}
				));
			}
			_ => panic!("expected Element"),
		}
	}

	#[rstest]
	fn test_parse_mixed_case_svg_timing_event_exactly() {
		// Arrange
		let input = quote!(|| { svg { @beginEvent: |_| {}, } });

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Element(elem) => assert!(matches!(
				&elem.events[0],
				crate::core::IntrinsicEvent::Standard {
					event: reinhardt_event_catalog::KnownEvent::BeginEvent,
					..
				}
			)),
			_ => panic!("expected Element"),
		}
	}

	#[rstest]
	fn test_parse_raw_custom_intrinsic_event() {
		// Arrange
		let input = quote!(|| { div { @custom("item-selected"): |_| {}, } });

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Element(elem) => match &elem.events[0] {
				crate::core::IntrinsicEvent::RawCustom { name, .. } => {
					assert_eq!(name.value(), "item-selected");
				}
				other => panic!("expected raw custom intrinsic event, got {other:?}"),
			},
			_ => panic!("expected Element"),
		}
	}

	#[rstest]
	fn test_parse_typed_custom_intrinsic_event() {
		// Arrange
		let input = quote!(|| {
			div {
				@custom::<crate::events::Envelope<Vec<u64>>>("item-selected"): |_| {},
			}
		});

		// Act
		let result: PageMacro = syn::parse2(input).expect("typed custom event");

		// Assert
		let PageNode::Element(element) = &result.body().nodes[0] else {
			panic!("expected element");
		};
		let crate::core::IntrinsicEvent::TypedCustom {
			name, payload_type, ..
		} = &element.events[0]
		else {
			panic!("expected typed custom event");
		};
		assert_eq!(name.value(), "item-selected");
		assert_eq!(
			quote!(#payload_type).to_string(),
			"crate :: events :: Envelope < Vec < u64 > >"
		);
	}

	#[rstest]
	#[case(
		quote!(|| { div { @custom::<>("name"): |_| {}, } }),
		"`@custom` requires exactly one payload type"
	)]
	#[case(
		quote!(|| { div { @custom::<A, B>("name"): |_| {}, } }),
		"`@custom` accepts exactly one payload type"
	)]
	#[case(
		quote!(|| { div { @custom::<Payload>(name): |_| {}, } }),
		"`@custom` event name must be a string literal"
	)]
	fn test_parse_typed_custom_intrinsic_event_diagnostics(
		#[case] input: proc_macro2::TokenStream,
		#[case] expected: &str,
	) {
		// Act
		let error = syn::parse2::<PageMacro>(input).expect_err("invalid custom event must fail");

		// Assert
		assert_eq!(error.to_string(), expected);
	}

	#[rstest]
	fn test_parse_unknown_intrinsic_event_suggests_nearest_catalog_name() {
		// Arrange
		let input = quote!(|| { button { @clik: |_| {}, "Click" } });

		// Act
		let error = syn::parse2::<PageMacro>(input).expect_err("unknown event must fail");

		// Assert
		assert_eq!(
			error.to_string(),
			"unknown standard element event `clik`; did you mean `click`?"
		);
	}

	#[rstest]
	fn test_parse_custom_event_call_is_rejected_on_components() {
		// Arrange
		let input = quote!(|| { Card { @custom("item-selected"): |_| {}, } });

		// Act
		let error = syn::parse2::<PageMacro>(input)
			.expect_err("custom intrinsic syntax must not parse as a component prop");

		// Assert
		assert_eq!(
			error.to_string(),
			"`@custom(\"...\")` is only valid on intrinsic elements"
		);
	}

	#[rstest]
	fn test_parse_typed_custom_event_call_is_rejected_on_components() {
		// Arrange
		let input = quote!(|| { Card { @custom::<Payload>("item-selected"): |_| {}, } });

		// Act
		let error = syn::parse2::<PageMacro>(input)
			.expect_err("typed custom intrinsic syntax must not parse as a component prop");

		// Assert
		assert_eq!(
			error.to_string(),
			"`@custom::<T>(\"...\")` is only valid on intrinsic elements"
		);
	}

	#[rstest]
	fn test_parse_nested_elements() {
		// Arrange
		let input = quote!(|| {
			div {
				h1 { "Title" }
				p { "Content" }
			}
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Element(elem) => {
				assert_eq!(elem.tag.to_string(), "div");
				assert_eq!(elem.children.len(), 2);

				match &elem.children[0] {
					PageNode::Element(h1) => assert_eq!(h1.tag.to_string(), "h1"),
					_ => panic!("expected h1 Element"),
				}

				match &elem.children[1] {
					PageNode::Element(p) => assert_eq!(p.tag.to_string(), "p"),
					_ => panic!("expected p Element"),
				}
			}
			_ => panic!("expected Element"),
		}
	}

	#[rstest]
	fn test_parse_text_node() {
		// Arrange
		let input = quote!(|| { "Hello, World!" });

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Text(text) => {
				assert_eq!(text.content, "Hello, World!");
			}
			_ => panic!("expected Text"),
		}
	}

	#[rstest]
	fn test_parse_if_node() {
		// Arrange
		let input = quote!(|| {
			if show {
				span { "visible" }
			}
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::If(if_node) => {
				assert_eq!(if_node.then_branch.len(), 1);
				assert!(if_node.else_branch.is_none());
			}
			_ => panic!("expected If"),
		}
	}

	#[rstest]
	fn test_parse_if_else_node() {
		// Arrange
		let input = quote!(|| {
			if is_admin {
				span { "Admin" }
			} else {
				span { "User" }
			}
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::If(if_node) => {
				assert!(if_node.else_branch.is_some());
				match &if_node.else_branch {
					Some(PageElse::Block(nodes)) => {
						assert_eq!(nodes.len(), 1);
					}
					_ => panic!("expected else block"),
				}
			}
			_ => panic!("expected If"),
		}
	}

	#[rstest]
	fn test_parse_for_node() {
		// Arrange
		let input = quote!(|| {
			for item in items {
				li { item }
			}
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::For(for_node) => {
				assert_eq!(for_node.body.len(), 1);
			}
			_ => panic!("expected For"),
		}
	}

	#[rstest]
	fn test_parse_keyed_for_node() {
		// Arrange
		let input = quote!(|| {
			for item in items @key(item.id.clone()) {
				li { item.name }
			}
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::For(for_node) => {
				assert!(for_node.key.is_some());
				assert_eq!(for_node.body.len(), 1);
			}
			_ => panic!("expected For"),
		}
	}

	#[rstest]
	fn test_parse_component_basic() {
		// Arrange
		let input = quote!(|| {
			MyButton(label: "Click")
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Component(comp) => {
				assert_eq!(comp.name.to_string(), "MyButton");
				assert_eq!(comp.args.len(), 1);
				assert_eq!(comp.args[0].name.to_string(), "label");
				assert!(comp.children.is_none());
			}
			_ => panic!("expected Component"),
		}
	}

	#[rstest]
	fn test_parse_component_multiple_args() {
		// Arrange
		let input = quote!(|| {
			MyButton(label: "Click", disabled: true, count: 42)
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Component(comp) => {
				assert_eq!(comp.name.to_string(), "MyButton");
				assert_eq!(comp.args.len(), 3);
				assert_eq!(comp.args[0].name.to_string(), "label");
				assert_eq!(comp.args[1].name.to_string(), "disabled");
				assert_eq!(comp.args[2].name.to_string(), "count");
			}
			_ => panic!("expected Component"),
		}
	}

	#[rstest]
	fn test_parse_component_with_children() {
		// Arrange
		let input = quote!(|| {
			MyWrapper(class: "container") {
				p { "Content" }
			}
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Component(comp) => {
				assert_eq!(comp.name.to_string(), "MyWrapper");
				assert_eq!(comp.args.len(), 1);
				assert!(comp.children.is_some());
				assert_eq!(comp.children.as_ref().unwrap().len(), 1);
			}
			_ => panic!("expected Component"),
		}
	}

	#[rstest]
	fn test_parse_component_empty_args() {
		// Arrange
		let input = quote!(|| { MyComponent() });

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Component(comp) => {
				assert_eq!(comp.name.to_string(), "MyComponent");
				assert!(comp.args.is_empty());
				assert!(comp.children.is_none());
			}
			_ => panic!("expected Component"),
		}
	}

	#[rstest]
	fn test_parse_component_trailing_comma() {
		// Arrange
		let input = quote!(|| {
			MyButton(label: "Click",)
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Component(comp) => {
				assert_eq!(comp.args.len(), 1);
			}
			_ => panic!("expected Component"),
		}
	}

	#[rstest]
	fn test_parse_component_with_expression_arg() {
		// Arrange
		let input = quote!(|| {
			MyComponent(count: items.len())
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Component(comp) => {
				assert_eq!(comp.args.len(), 1);
				assert_eq!(comp.args[0].name.to_string(), "count");
			}
			_ => panic!("expected Component"),
		}
	}

	#[rstest]
	fn test_parse_mixed_elements_and_components() {
		// Arrange
		let input = quote!(|| {
			div {
				MyComponent(label: "test")
				p { "text" }
			}
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Element(elem) => {
				assert_eq!(elem.tag.to_string(), "div");
				assert_eq!(elem.children.len(), 2);

				match &elem.children[0] {
					PageNode::Component(comp) => {
						assert_eq!(comp.name.to_string(), "MyComponent");
					}
					_ => panic!("expected Component as first child"),
				}

				match &elem.children[1] {
					PageNode::Element(p) => {
						assert_eq!(p.tag.to_string(), "p");
					}
					_ => panic!("expected Element as second child"),
				}
			}
			_ => panic!("expected Element"),
		}
	}

	#[rstest]
	fn test_parse_with_head_directive() {
		// Arrange
		use proc_macro2::{Punct, Spacing};
		// Build: # head : my_head , || { div { "hello" } }
		let pound = Punct::new('#', Spacing::Alone);
		let input = quote! {
			#pound head: my_head,
			|| { div { "hello" } }
		};

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		assert!(result.head.is_some());
		assert!(result.params().is_empty());
		assert_eq!(result.body().nodes.len(), 1);
	}

	#[rstest]
	fn test_parse_with_head_directive_and_implicit_body() {
		// Arrange
		use proc_macro2::{Punct, Spacing};
		let pound = Punct::new('#', Spacing::Alone);
		let input = quote! {
			#pound head: page_head,
			{ main { "Project settings" } }
		};

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		assert!(result.head.is_some());
		assert!(result.is_implicit_body());
		assert!(result.params().is_empty());
		assert_eq!(result.body().nodes.len(), 1);
	}

	#[rstest]
	fn test_parse_with_head_directive_and_params() {
		// Arrange
		use proc_macro2::{Punct, Spacing};
		let pound = Punct::new('#', Spacing::Alone);
		let input = quote! {
			#pound head: create_head(),
			|name: String| { div { name } }
		};

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		assert!(result.head.is_some());
		assert_eq!(result.params().len(), 1);
		assert_eq!(result.params()[0].name.to_string(), "name");
	}

	#[rstest]
	fn test_parse_unknown_directive_error() {
		// Arrange
		use proc_macro2::{Punct, Spacing};
		let pound = Punct::new('#', Spacing::Alone);
		let input = quote! {
			#pound unknown: value,
			|| { div { "hello" } }
		};

		// Act
		let result: syn::Result<PageMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("#unknown"));
		assert!(err_msg.contains("#head"));
	}

	#[rstest]
	fn test_parse_page_function() {
		// Arrange
		let input = quote!(|| { div { "hello" } });

		// Act
		let result = parse_page(input);

		// Assert
		assert!(result.is_ok());
		let page = result.unwrap();
		assert!(page.params().is_empty());
		assert_eq!(page.body().nodes.len(), 1);
	}

	#[rstest]
	#[case("type")]
	#[case("for")]
	fn test_parse_reserved_keyword_as_attr_name(#[case] keyword: &str) {
		// Arrange
		let input_str = format!("|| {{ input {{ {keyword}: \"text\", }} }}",);

		// Act
		let result: PageMacro = syn::parse_str(&input_str).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Element(elem) => {
				assert_eq!(elem.attrs.len(), 1);
				assert_eq!(elem.attrs[0].name.to_string(), keyword);
			}
			_ => panic!("expected Element"),
		}
	}

	#[rstest]
	fn test_parse_standalone_boolean_attr() {
		// Arrange
		let input = quote!(|| { input { required } });

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Element(elem) => {
				assert_eq!(elem.attrs.len(), 1);
				assert_eq!(elem.attrs[0].name.to_string(), "required");
				match &elem.attrs[0].value {
					Expr::Lit(lit) => match &lit.lit {
						syn::Lit::Bool(b) => assert!(b.value()),
						_ => panic!("expected BoolLit"),
					},
					_ => panic!("expected Lit expr"),
				}
			}
			_ => panic!("expected Element"),
		}
	}

	#[rstest]
	fn test_parse_multiple_standalone_boolean_attrs() {
		// Arrange
		let input = quote!(|| { input { required disabled readonly } });

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Element(elem) => {
				assert_eq!(elem.attrs.len(), 3);
				assert_eq!(elem.attrs[0].name.to_string(), "required");
				assert_eq!(elem.attrs[1].name.to_string(), "disabled");
				assert_eq!(elem.attrs[2].name.to_string(), "readonly");
			}
			_ => panic!("expected Element"),
		}
	}

	#[rstest]
	fn test_parse_mixed_attrs_and_standalone_boolean() {
		// Arrange
		let input = quote!(|| {
			input {
				r#type: "text",
				required,
				class: "form-input",
			}
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Element(elem) => {
				assert_eq!(elem.attrs.len(), 3);
				assert_eq!(elem.attrs[0].html_name(), "type");
				assert_eq!(elem.attrs[1].name.to_string(), "required");
				assert_eq!(elem.attrs[2].name.to_string(), "class");
			}
			_ => panic!("expected Element"),
		}
	}

	// ---- Brace-form component invocation (spec §3.5) ----

	#[rstest]
	fn parses_component_brace_with_named_prop() {
		// Arrange
		let input = quote!(|| { Card { item: 42 } });

		// Act
		let ast: PageMacro = syn::parse2(input).unwrap();
		let body = &ast.body().nodes;

		// Assert
		assert_eq!(body.len(), 1);
		match &body[0] {
			PageNode::Component(c) => {
				assert_eq!(c.name.to_string(), "Card");
				assert_eq!(c.invocation_form, ComponentInvocationForm::Brace);
				assert_eq!(c.args.len(), 1);
				assert_eq!(c.args[0].name.to_string(), "item");
				assert!(c.events.is_empty());
				assert!(c.children.is_none());
			}
			other => panic!("expected Component, got {:?}", other),
		}
	}

	#[rstest]
	fn parses_component_brace_with_event_prop() {
		// Arrange
		let input = quote!(|| { Card { item: 1, @click: |_| {}, } });

		// Act
		let ast: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &ast.body().nodes[0] {
			PageNode::Component(c) => {
				assert_eq!(c.invocation_form, ComponentInvocationForm::Brace);
				assert_eq!(c.args.len(), 1);
				assert_eq!(c.events.len(), 1);
				assert_eq!(c.events[0].name.to_string(), "click");
			}
			other => panic!("expected Component, got {:?}", other),
		}
	}

	#[rstest]
	fn parses_component_brace_with_child_element() {
		// Arrange
		let input = quote!(|| { Card { item: 1, p { "child" } } });

		// Act
		let ast: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &ast.body().nodes[0] {
			PageNode::Component(c) => {
				assert_eq!(c.args.len(), 1);
				let cs = c.children.as_ref().expect("children present");
				assert_eq!(cs.len(), 1);
				match &cs[0] {
					PageNode::Element(e) => assert_eq!(e.tag.to_string(), "p"),
					other => panic!("expected child Element, got {:?}", other),
				}
			}
			other => panic!("expected Component, got {:?}", other),
		}
	}

	#[rstest]
	fn parses_nested_component_brace() {
		// Arrange
		let input = quote!(|| { Card { item: 1, Inner { x: 2 } } });

		// Act
		let ast: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &ast.body().nodes[0] {
			PageNode::Component(outer) => {
				let cs = outer.children.as_ref().expect("children");
				match &cs[0] {
					PageNode::Component(inner) => {
						assert_eq!(inner.name.to_string(), "Inner");
						assert_eq!(inner.invocation_form, ComponentInvocationForm::Brace);
						assert_eq!(inner.args.len(), 1);
						assert_eq!(inner.args[0].name.to_string(), "x");
					}
					other => panic!("expected nested Component, got {:?}", other),
				}
			}
			other => panic!("expected outer Component, got {:?}", other),
		}
	}

	#[rstest]
	fn paren_form_keeps_paren_invocation_form() {
		// Arrange
		let input = quote!(|| { MyButton(label: "Click") });

		// Act
		let ast: PageMacro = syn::parse2(input).unwrap();

		// Assert: regression — legacy paren form is preserved and
		// distinguished from the new brace form via invocation_form.
		match &ast.body().nodes[0] {
			PageNode::Component(c) => {
				assert_eq!(c.invocation_form, ComponentInvocationForm::Paren);
				assert!(c.events.is_empty());
			}
			other => panic!("expected Component, got {:?}", other),
		}
	}

	#[rstest]
	fn parses_component_brace_empty_body() {
		// Arrange
		let input = quote!(|| { Card {} });

		// Act
		let ast: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &ast.body().nodes[0] {
			PageNode::Component(c) => {
				assert_eq!(c.invocation_form, ComponentInvocationForm::Brace);
				assert!(c.args.is_empty());
				assert!(c.events.is_empty());
				assert!(c.children.is_none());
			}
			other => panic!("expected Component, got {:?}", other),
		}
	}

	#[rstest]
	fn parses_component_brace_with_multiple_children() {
		// Arrange
		let input = quote!(|| {
			Card {
				title: "x",
				p { "one" }
				p { "two" }
			}
		});

		// Act
		let ast: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &ast.body().nodes[0] {
			PageNode::Component(c) => {
				assert_eq!(c.args.len(), 1);
				let cs = c.children.as_ref().expect("children");
				assert_eq!(cs.len(), 2);
			}
			other => panic!("expected Component, got {:?}", other),
		}
	}

	#[rstest]
	fn test_parse_standalone_boolean_with_children() {
		// Arrange
		let input = quote!(|| {
			select {
				required
				option { "A" }
				option { "B" }
			}
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		match &result.body().nodes[0] {
			PageNode::Element(elem) => {
				assert_eq!(elem.attrs.len(), 1);
				assert_eq!(elem.attrs[0].name.to_string(), "required");
				assert_eq!(elem.children.len(), 2);
			}
			_ => panic!("expected Element"),
		}
	}

	#[rstest]
	fn test_parse_component_with_named_slot() {
		// Arrange
		let input = quote!(|| {
			Table(args: 1) {
				$header { div { "Title" } }
				$body { div { "Content" } }
			}
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		let body = result.body();
		assert_eq!(body.nodes.len(), 1);
		if let PageNode::Component(comp) = &body.nodes[0] {
			assert_eq!(comp.name, "Table");
			assert_eq!(comp.named_slots.len(), 2);
			assert_eq!(comp.named_slots[0].name, "header");
			assert_eq!(comp.named_slots[1].name, "body");
			assert_eq!(comp.named_slots[0].children.len(), 1);
			assert_eq!(comp.named_slots[1].children.len(), 1);
		} else {
			panic!("expected Component node");
		}
	}

	#[rstest]
	fn test_parse_named_slot_duplicate_rejected() {
		// Arrange
		let input = quote!(|| {
			Table(args: 1) {
				$header { div { "A" } }
				$header { div { "B" } }
			}
		});

		// Act
		let result: syn::Result<PageMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert_eq!(err, "duplicate named slot `header` in component");
	}

	#[rstest]
	fn test_named_slot_outside_component_rejected() {
		// Arrange
		let input = quote!(|| {
			div {
				$header { "this is invalid" }
			}
		});

		// Act
		let result: syn::Result<PageMacro> = syn::parse2(input);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err().to_string();
		assert_eq!(err, "`$slotname { }` is only valid inside a component body");
	}

	#[rstest]
	fn test_parse_component_mixed_default_and_named_slots() {
		// Arrange
		let input = quote!(|| {
			Table(args: 1) {
				div { "default child" }
				$header { div { "Header" } }
				span { "another default" }
			}
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		if let PageNode::Component(comp) = &result.body().nodes[0] {
			assert!(comp.children.is_some());
			assert_eq!(comp.children.as_ref().unwrap().len(), 2);
			assert_eq!(comp.named_slots.len(), 1);
			assert_eq!(comp.named_slots[0].name, "header");
		} else {
			panic!("expected Component node");
		}
	}

	#[rstest]
	fn test_parse_component_empty_named_slot() {
		// Arrange
		let input = quote!(|| {
			Table(args: 1) {
				$header {}
			}
		});

		// Act
		let result: PageMacro = syn::parse2(input).unwrap();

		// Assert
		if let PageNode::Component(comp) = &result.body().nodes[0] {
			assert_eq!(comp.named_slots.len(), 1);
			assert_eq!(comp.named_slots[0].name, "header");
			assert!(comp.named_slots[0].children.is_empty());
		} else {
			panic!("expected Component node");
		}
	}
}
