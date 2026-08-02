//! Parses the brace-body form of component invocation (spec §3.5).
//!
//! ```text
//! Card {
//!     item: x,
//!     @click: |_| { handle() },
//!     p { "child" }
//! }
//! ```
//!
//! Body items are identified by the `{` disambiguation rule (spec §3.2):
//!
//! - `ident :` after an identifier → named prop (`PageComponentArg`)
//! - `@ event :` → event prop (`ComponentEventProp`)
//! - lowercase `ident { ... }` → child HTML element
//! - PascalCase `Ident { ... }` → nested child component (brace form)
//! - PascalCase `Ident ( ... )` → nested child component (paren form)
//! - `{ expr }` / `"text"` / `if ...` / `for ...` → other child node
//!
//! This grammar is intentionally a strict subset of the top-level
//! `parse_node` grammar — we delegate to [`super::parse_node`] for any
//! item that is not a prop / event keyed by `ident :` / `@event :`.

use syn::{Expr, Ident, Result, Token, braced, ext::IdentExt, parse::ParseStream};

use crate::{
	ComponentEventProp, ComponentInvocationForm, NamedSlot, PageComponent, PageComponentArg,
	PageNode,
};

/// Parses one brace-form component invocation, starting at the PascalCase
/// identifier and consuming the trailing `{ ... }` block.
///
/// Caller (the top-level `parse_node`) has already verified that:
///
/// - the next token is an identifier, and
/// - it starts with an ASCII uppercase letter, and
/// - it is immediately followed by `{`.
pub(super) fn parse_component_brace_node(input: ParseStream) -> Result<PageNode> {
	let name: Ident = input.parse()?;
	let span = name.span();

	let content;
	braced!(content in input);

	let mut args: Vec<PageComponentArg> = Vec::new();
	let mut events: Vec<ComponentEventProp> = Vec::new();
	let mut children: Vec<PageNode> = Vec::new();
	let mut named_slots: Vec<NamedSlot> = Vec::new();

	while !content.is_empty() {
		// Event prop: `@event: handler`
		if content.peek(Token![@]) {
			events.push(parse_event_inline(&content)?);
			continue;
		}

		// Named prop: `ident: expr`
		//
		// We must distinguish:
		//   - `ident :` → named prop (regardless of whether `ident` would
		//     otherwise look like a tag / component, because the `:`
		//     pins the meaning)
		//   - `Ident { ... }` / `ident { ... }` / `Ident ( ... )` → child
		//     node (delegated to `super::parse_node`)
		if content.peek(Ident::peek_any) {
			let fork = content.fork();
			let _ = Ident::parse_any(&fork)?;
			if fork.peek(Token![:]) {
				args.push(parse_named_prop(&content)?);
				continue;
			}
		}

		// Named slot: `$slotname { ... }`
		if content.peek(Token![$]) {
			content.parse::<Token![$]>()?;
			let slot_name: Ident = content.parse()?;
			let slot_span = slot_name.span();

			// Check for duplicate named slot (E1)
			if named_slots.iter().any(|s: &NamedSlot| s.name == slot_name) {
				return Err(syn::Error::new(
					slot_span,
					format!("duplicate named slot `{}` in component", slot_name),
				));
			}

			let slot_content;
			braced!(slot_content in content);
			let slot_children = super::parse_nodes(&slot_content)?;

			named_slots.push(NamedSlot {
				name: slot_name,
				children: slot_children,
				span: slot_span,
			});
			// Optional trailing comma
			if content.peek(Token![,]) {
				content.parse::<Token![,]>()?;
			}
			continue;
		}

		// Anything else is a generic child node (HTML element, nested
		// component, text literal, braced expression, if / for, ...).
		children.push(super::parse_node(&content)?);
		// Optional trailing comma after a child node so that
		// `Card { p { "x" }, q { "y" } }` parses successfully.
		if content.peek(Token![,]) {
			content.parse::<Token![,]>()?;
		}
	}

	Ok(PageNode::Component(PageComponent {
		name,
		invocation_form: ComponentInvocationForm::Brace,
		args,
		events,
		children: if children.is_empty() {
			None
		} else {
			Some(children)
		},
		named_slots,
		span,
	}))
}

/// Parses one `ident: expr` named prop, consuming an optional trailing comma.
///
/// Uses `Ident::parse_any` so reserved keywords (e.g. `type`, `for`) are
/// valid prop names, matching the existing element-attribute parser.
fn parse_named_prop(input: ParseStream) -> Result<PageComponentArg> {
	let name = Ident::parse_any(input)?;
	let span = name.span();
	input.parse::<Token![:]>()?;
	let value: Expr = input.parse()?;

	// Optional trailing comma between props / children.
	if input.peek(Token![,]) {
		input.parse::<Token![,]>()?;
	}

	Ok(PageComponentArg { name, value, span })
}

/// Parses one `@event: handler` event prop, consuming an optional trailing comma.
fn parse_event_inline(input: ParseStream) -> Result<ComponentEventProp> {
	input.parse::<Token![@]>()?;
	let name: Ident = input.parse()?;
	if name == "custom" && (input.peek(syn::token::Paren) || input.peek(syn::Token![::])) {
		let syntax = if input.peek(syn::Token![::]) {
			"`@custom::<T>(\"...\")` is only valid on intrinsic elements"
		} else {
			"`@custom(\"...\")` is only valid on intrinsic elements"
		};
		return Err(syn::Error::new(name.span(), syntax));
	}
	input.parse::<Token![:]>()?;
	let handler: Expr = input.parse()?;

	if input.peek(Token![,]) {
		input.parse::<Token![,]>()?;
	}

	Ok(ComponentEventProp { name, handler })
}
