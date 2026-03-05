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

use syn::{
	Expr, Ident, Pat, Result, Token, braced, parenthesized,
	parse::{Parse, ParseStream},
	token,
};

use crate::{
	PageAttr, PageBody, PageComponent, PageComponentArg, PageElement, PageElse, PageEvent,
	PageExpression, PageFor, PageIf, PageMacro, PageNode, PageParam, PageText, PageWatch,
};

/// Maximum nesting depth for page elements.
/// Prevents stack overflow from deeply nested page structures. (Fixes #824)
const MAX_NESTING_DEPTH: usize = 64;

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

		// Parse closure-style parameters: |param1: Type1, param2: Type2|
		let params = if input.peek(Token![|]) {
			parse_closure_params(input)?
		} else {
			Vec::new()
		};

		// Parse the body: { ... }
		let body = input.parse::<PageBody>()?;

		Ok(Self {
			head,
			params,
			body,
			span,
		})
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

		let nodes = parse_nodes(&content, MAX_NESTING_DEPTH)?;

		Ok(Self { nodes, span })
	}
}

/// Parses multiple nodes from a ParseStream.
///
/// The `depth` parameter limits recursion to prevent stack overflow. (Fixes #824)
fn parse_nodes(input: ParseStream, depth: usize) -> Result<Vec<PageNode>> {
	let mut nodes = Vec::new();

	while !input.is_empty() {
		nodes.push(parse_node(input, depth)?);
	}

	Ok(nodes)
}

/// Parses a single node from the input.
///
/// The `depth` parameter limits recursion to prevent stack overflow. (Fixes #824)
fn parse_node(input: ParseStream, depth: usize) -> Result<PageNode> {
	if depth == 0 {
		return Err(syn::Error::new(
			input.span(),
			"page element nesting depth exceeded maximum limit",
		));
	}

	// Check for string literal: "text"
	if input.peek(syn::LitStr) {
		return parse_text_node(input);
	}

	// Check for if expression
	if input.peek(Token![if]) {
		return parse_if_node(input, depth);
	}

	// Check for for loop
	if input.peek(Token![for]) {
		return parse_for_node(input, depth);
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
			return parse_watch_node(input, depth);
		}

		if fork.peek(token::Brace) {
			// It's an element: tag { ... }
			return parse_element_node(input, depth);
		} else if fork.peek(token::Paren) {
			// It's a component call: Component(args) or Component(args) { children }
			return parse_component_node(input, depth);
		} else {
			// It's an expression: variable or method call
			return parse_expression_node(input);
		}
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
///
/// The `depth` parameter limits recursion to prevent stack overflow. (Fixes #824)
fn parse_element_node(input: ParseStream, depth: usize) -> Result<PageNode> {
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
		if content.peek(Ident) {
			let fork = content.fork();
			let _ident: Ident = fork.parse()?;

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
		}

		// Otherwise, it's a child node
		element.children.push(parse_node(&content, depth - 1)?);
	}

	Ok(PageNode::Element(element))
}

/// Parses an attribute: `name: value,`
fn parse_attr(input: ParseStream) -> Result<PageAttr> {
	let name: Ident = input.parse()?;
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
fn parse_event(input: ParseStream) -> Result<PageEvent> {
	input.parse::<Token![@]>()?;
	let event_type: Ident = input.parse()?;
	let span = event_type.span();
	input.parse::<Token![:]>()?;
	let handler: Expr = input.parse()?;

	// Consume optional trailing comma
	if input.peek(Token![,]) {
		input.parse::<Token![,]>()?;
	}

	Ok(PageEvent {
		event_type,
		handler,
		span,
	})
}

/// Parses a braced expression: `{ expr }`
fn parse_braced_expression(input: ParseStream) -> Result<PageNode> {
	let span = input.span();
	let content;
	braced!(content in input);
	let expr: Expr = content.parse()?;

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
///
/// The `depth` parameter limits recursion to prevent stack overflow
/// from deeply nested if/else-if chains. (Fixes #824)
fn parse_if_node(input: ParseStream, depth: usize) -> Result<PageNode> {
	let span = input.span();
	input.parse::<Token![if]>()?;

	// Parse condition (everything until the brace)
	let condition = parse_condition(input)?;

	// Parse then branch
	let content;
	braced!(content in input);
	let then_branch = parse_nodes(&content, depth - 1)?;

	// Parse optional else branch
	let else_branch = if input.peek(Token![else]) {
		input.parse::<Token![else]>()?;

		if input.peek(Token![if]) {
			// else if
			let else_if = parse_if_node(input, depth - 1)?;
			match else_if {
				PageNode::If(if_node) => Some(PageElse::If(Box::new(if_node))),
				_ => {
					return Err(syn::Error::new(
						input.span(),
						"internal error: expected if node in else-if chain",
					));
				}
			}
		} else {
			// else { ... }
			let content;
			braced!(content in input);
			let else_nodes = parse_nodes(&content, depth - 1)?;
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

/// Parses a for node: `for pat in iter { ... }`
///
/// The `depth` parameter limits recursion to prevent stack overflow. (Fixes #824)
fn parse_for_node(input: ParseStream, depth: usize) -> Result<PageNode> {
	let span = input.span();
	input.parse::<Token![for]>()?;

	// Parse pattern (use parse_single for single pattern without alternatives)
	let pat = Pat::parse_single(input)?;

	input.parse::<Token![in]>()?;

	// Parse iterator expression (until brace)
	let iter = parse_condition(input)?;

	// Parse body
	let content;
	braced!(content in input);
	let body = parse_nodes(&content, depth - 1)?;

	Ok(PageNode::For(PageFor {
		pat,
		iter,
		body,
		span,
	}))
}

/// Parses a watch node: `watch { expr }`
///
/// The watch block wraps an expression in a reactive context,
/// allowing Signal dependencies to be automatically tracked.
///
/// The `depth` parameter limits recursion to prevent stack overflow. (Fixes #824)
fn parse_watch_node(input: ParseStream, depth: usize) -> Result<PageNode> {
	let span = input.span();

	// Consume the "watch" identifier
	let watch_ident: Ident = input.parse()?;
	debug_assert_eq!(watch_ident, "watch");

	// Parse the braced content
	let content;
	braced!(content in input);

	// Parse the inner expression as a single node
	// The watch block must contain exactly one expression (if, match, etc.)
	let inner_node = parse_node(&content, depth - 1)?;

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
fn parse_component_node(input: ParseStream, depth: usize) -> Result<PageNode> {
	let name: Ident = input.parse()?;
	let span = name.span();

	// Parse arguments in parentheses
	let args_content;
	parenthesized!(args_content in input);
	let args = parse_component_args(&args_content)?;

	// Parse optional children in braces
	let children = if input.peek(token::Brace) {
		let content;
		braced!(content in input);
		Some(parse_nodes(&content, depth - 1)?)
	} else {
		None
	};

	Ok(PageNode::Component(PageComponent {
		name,
		args,
		children,
		span,
	}))
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

	#[test]
	fn test_parse_empty_params() {
		let input = quote!(|| { div { "hello" } });
		let result: PageMacro = syn::parse2(input).unwrap();
		assert!(result.params.is_empty());
		assert_eq!(result.body.nodes.len(), 1);
	}

	#[test]
	fn test_parse_single_param() {
		let input = quote!(|name: String| { div { "hello" } });
		let result: PageMacro = syn::parse2(input).unwrap();
		assert_eq!(result.params.len(), 1);
		assert_eq!(result.params[0].name.to_string(), "name");
	}

	#[test]
	fn test_parse_multiple_params() {
		let input = quote!(|name: String, count: i32| { div { "hello" } });
		let result: PageMacro = syn::parse2(input).unwrap();
		assert_eq!(result.params.len(), 2);
		assert_eq!(result.params[0].name.to_string(), "name");
		assert_eq!(result.params[1].name.to_string(), "count");
	}

	#[test]
	fn test_parse_simple_element() {
		let input = quote!(|| { div { "hello" } });
		let result: PageMacro = syn::parse2(input).unwrap();

		match &result.body.nodes[0] {
			PageNode::Element(elem) => {
				assert_eq!(elem.tag.to_string(), "div");
				assert_eq!(elem.children.len(), 1);
			}
			_ => panic!("expected Element"),
		}
	}

	#[test]
	fn test_parse_element_with_attrs() {
		let input = quote!(|| {
			div {
				class: "container",
				id: "main",
				"hello"
			}
		});
		let result: PageMacro = syn::parse2(input).unwrap();

		match &result.body.nodes[0] {
			PageNode::Element(elem) => {
				assert_eq!(elem.attrs.len(), 2);
				assert_eq!(elem.attrs[0].name.to_string(), "class");
				assert_eq!(elem.attrs[1].name.to_string(), "id");
				assert_eq!(elem.children.len(), 1);
			}
			_ => panic!("expected Element"),
		}
	}

	#[test]
	fn test_parse_element_with_event() {
		let input = quote!(|| {
			button {
				@click: |_| { handle_click(); },
				"Click me"
			}
		});
		let result: PageMacro = syn::parse2(input).unwrap();

		match &result.body.nodes[0] {
			PageNode::Element(elem) => {
				assert_eq!(elem.tag.to_string(), "button");
				assert_eq!(elem.events.len(), 1);
				assert_eq!(elem.events[0].event_type.to_string(), "click");
			}
			_ => panic!("expected Element"),
		}
	}

	#[test]
	fn test_parse_nested_elements() {
		let input = quote!(|| {
			div {
				h1 { "Title" }
				p { "Content" }
			}
		});
		let result: PageMacro = syn::parse2(input).unwrap();

		match &result.body.nodes[0] {
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

	#[test]
	fn test_parse_text_node() {
		let input = quote!(|| { "Hello, World!" });
		let result: PageMacro = syn::parse2(input).unwrap();

		match &result.body.nodes[0] {
			PageNode::Text(text) => {
				assert_eq!(text.content, "Hello, World!");
			}
			_ => panic!("expected Text"),
		}
	}

	#[test]
	fn test_parse_if_node() {
		let input = quote!(|| {
			if show {
				span { "visible" }
			}
		});
		let result: PageMacro = syn::parse2(input).unwrap();

		match &result.body.nodes[0] {
			PageNode::If(if_node) => {
				assert_eq!(if_node.then_branch.len(), 1);
				assert!(if_node.else_branch.is_none());
			}
			_ => panic!("expected If"),
		}
	}

	#[test]
	fn test_parse_if_else_node() {
		let input = quote!(|| {
			if is_admin {
				span { "Admin" }
			} else {
				span { "User" }
			}
		});
		let result: PageMacro = syn::parse2(input).unwrap();

		match &result.body.nodes[0] {
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

	#[test]
	fn test_parse_for_node() {
		let input = quote!(|| {
			for item in items {
				li { item }
			}
		});
		let result: PageMacro = syn::parse2(input).unwrap();

		match &result.body.nodes[0] {
			PageNode::For(for_node) => {
				assert_eq!(for_node.body.len(), 1);
			}
			_ => panic!("expected For"),
		}
	}

	#[test]
	fn test_parse_component_basic() {
		let input = quote!(|| {
			MyButton(label: "Click")
		});
		let result: PageMacro = syn::parse2(input).unwrap();

		match &result.body.nodes[0] {
			PageNode::Component(comp) => {
				assert_eq!(comp.name.to_string(), "MyButton");
				assert_eq!(comp.args.len(), 1);
				assert_eq!(comp.args[0].name.to_string(), "label");
				assert!(comp.children.is_none());
			}
			_ => panic!("expected Component"),
		}
	}

	#[test]
	fn test_parse_component_multiple_args() {
		let input = quote!(|| {
			MyButton(label: "Click", disabled: true, count: 42)
		});
		let result: PageMacro = syn::parse2(input).unwrap();

		match &result.body.nodes[0] {
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

	#[test]
	fn test_parse_component_with_children() {
		let input = quote!(|| {
			MyWrapper(class: "container") {
				p { "Content" }
			}
		});
		let result: PageMacro = syn::parse2(input).unwrap();

		match &result.body.nodes[0] {
			PageNode::Component(comp) => {
				assert_eq!(comp.name.to_string(), "MyWrapper");
				assert_eq!(comp.args.len(), 1);
				assert!(comp.children.is_some());
				assert_eq!(comp.children.as_ref().unwrap().len(), 1);
			}
			_ => panic!("expected Component"),
		}
	}

	#[test]
	fn test_parse_component_empty_args() {
		let input = quote!(|| { MyComponent() });
		let result: PageMacro = syn::parse2(input).unwrap();

		match &result.body.nodes[0] {
			PageNode::Component(comp) => {
				assert_eq!(comp.name.to_string(), "MyComponent");
				assert!(comp.args.is_empty());
				assert!(comp.children.is_none());
			}
			_ => panic!("expected Component"),
		}
	}

	#[test]
	fn test_parse_component_trailing_comma() {
		let input = quote!(|| {
			MyButton(label: "Click",)
		});
		let result: PageMacro = syn::parse2(input).unwrap();

		match &result.body.nodes[0] {
			PageNode::Component(comp) => {
				assert_eq!(comp.args.len(), 1);
			}
			_ => panic!("expected Component"),
		}
	}

	#[test]
	fn test_parse_component_with_expression_arg() {
		let input = quote!(|| {
			MyComponent(count: items.len())
		});
		let result: PageMacro = syn::parse2(input).unwrap();

		match &result.body.nodes[0] {
			PageNode::Component(comp) => {
				assert_eq!(comp.args.len(), 1);
				assert_eq!(comp.args[0].name.to_string(), "count");
			}
			_ => panic!("expected Component"),
		}
	}

	#[test]
	fn test_parse_mixed_elements_and_components() {
		let input = quote!(|| {
			div {
				MyComponent(label: "test")
				p { "text" }
			}
		});
		let result: PageMacro = syn::parse2(input).unwrap();

		match &result.body.nodes[0] {
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

	#[test]
	fn test_parse_with_head_directive() {
		use proc_macro2::{Punct, Spacing};
		// Build: # head : my_head , || { div { "hello" } }
		let pound = Punct::new('#', Spacing::Alone);
		let input = quote! {
			#pound head: my_head,
			|| { div { "hello" } }
		};
		let result: PageMacro = syn::parse2(input).unwrap();
		assert!(result.head.is_some());
		assert!(result.params.is_empty());
		assert_eq!(result.body.nodes.len(), 1);
	}

	#[test]
	fn test_parse_with_head_directive_and_params() {
		use proc_macro2::{Punct, Spacing};
		let pound = Punct::new('#', Spacing::Alone);
		let input = quote! {
			#pound head: create_head(),
			|name: String| { div { name } }
		};
		let result: PageMacro = syn::parse2(input).unwrap();
		assert!(result.head.is_some());
		assert_eq!(result.params.len(), 1);
		assert_eq!(result.params[0].name.to_string(), "name");
	}

	#[test]
	fn test_parse_unknown_directive_error() {
		use proc_macro2::{Punct, Spacing};
		let pound = Punct::new('#', Spacing::Alone);
		let input = quote! {
			#pound unknown: value,
			|| { div { "hello" } }
		};
		let result: syn::Result<PageMacro> = syn::parse2(input);
		assert!(result.is_err());
		let err_msg = result.unwrap_err().to_string();
		assert!(err_msg.contains("#unknown"));
		assert!(err_msg.contains("#head"));
	}
}
