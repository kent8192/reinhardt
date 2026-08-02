//! AST node definitions for the page! macro DSL.
//!
//! This module defines the Abstract Syntax Tree (AST) nodes that represent
//! the page! macro's Domain Specific Language (DSL).
//!
//! ## DSL Structure
//!
//! ```text
//! page!(|props| {
//!     element {
//!         attr: value,
//!         @event: handler,
//!         child_element { ... }
//!         "text content"
//!         { expression }
//!     }
//! })
//! ```

use proc_macro2::{Span, TokenStream};
use syn::spanned::Spanned;
use syn::{Expr, FnArg, Ident, LitStr, Pat, Type};

use reinhardt_event_catalog::KnownEvent;

/// The top-level AST node representing an entire page! macro invocation.
///
/// # Example
///
/// ```text
/// page!(|initial: i32| {
///     div {
///         class: "container",
///         h1 { "Hello" }
///     }
/// })
/// ```
///
/// With head directive:
///
/// ```text
/// page! {
///     #head: my_head,
///     |initial: i32| {
///         div { "content" }
///     }
/// }
/// ```
#[derive(Debug)]
pub struct PageMacro {
	/// Optional head section expression.
	///
	/// When present, the generated view will be wrapped with `.with_head(head_expr)`.
	pub head: Option<Expr>,
	/// The parsed page macro form.
	pub form: PageMacroForm,
	/// Span for error reporting
	pub span: Span,
}

impl PageMacro {
	/// Returns the body containing the view tree.
	pub fn body(&self) -> &PageBody {
		match &self.form {
			PageMacroForm::StrictClosure { body, .. } | PageMacroForm::ImplicitBody { body } => {
				body
			}
		}
	}

	/// Returns the closure-style parameters when the macro uses strict closure form.
	pub fn params(&self) -> &[PageParam] {
		match &self.form {
			PageMacroForm::StrictClosure { params, .. } => params,
			PageMacroForm::ImplicitBody { .. } => &[],
		}
	}

	/// Returns `true` when this macro uses body-only implicit capture form.
	pub fn is_implicit_body(&self) -> bool {
		matches!(self.form, PageMacroForm::ImplicitBody { .. })
	}
}

/// The syntactic form used by a `page!` macro invocation.
#[derive(Debug)]
pub enum PageMacroForm {
	/// A strict closure form such as `page!(|name: String| { ... })`.
	StrictClosure {
		/// Closure-style parameters (props).
		params: Vec<PageParam>,
		/// The body containing the view tree.
		body: PageBody,
	},
	/// A body-only form such as `page!({ ... })`.
	ImplicitBody {
		/// The body containing the view tree.
		body: PageBody,
	},
}

/// A single parameter in the page! macro's closure-style signature.
///
/// # Example
///
/// ```text
/// |name: String, count: i32|
/// ```
#[derive(Debug, Clone)]
pub struct PageParam {
	/// Parameter name
	pub name: Ident,
	/// Parameter type
	pub ty: Type,
	/// Span for error reporting
	pub span: Span,
}

impl PageParam {
	/// Creates a new PageParam from a syn::FnArg.
	pub fn from_fn_arg(arg: &FnArg) -> syn::Result<Self> {
		match arg {
			FnArg::Typed(pat_type) => {
				let name = match pat_type.pat.as_ref() {
					Pat::Ident(pat_ident) => pat_ident.ident.clone(),
					_ => {
						return Err(syn::Error::new_spanned(
							&pat_type.pat,
							"expected identifier pattern",
						));
					}
				};
				Ok(Self {
					name,
					ty: (*pat_type.ty).clone(),
					span: pat_type.pat.span(),
				})
			}
			FnArg::Receiver(_) => Err(syn::Error::new_spanned(arg, "`self` is not allowed here")),
		}
	}
}

/// The body of a page! macro, containing one or more nodes.
#[derive(Debug, Clone)]
pub struct PageBody {
	/// Root nodes of the view tree
	pub nodes: Vec<PageNode>,
	/// Span for error reporting
	pub span: Span,
}

/// A single node in the page! DSL.
///
/// Nodes can be:
/// - Elements (e.g., `div { ... }`)
/// - Text literals (e.g., `"Hello"`)
/// - Expressions (e.g., `{ some_variable }` or `format!(...)`)
/// - Control flow (e.g., `if condition { ... }` or `for item in items { ... }`)
/// - Components (e.g., `MyButton(label: "Click")`)
/// - Reactive blocks (e.g., `watch { if signal.get() { ... } }`)
#[derive(Debug, Clone)]
pub enum PageNode {
	/// An HTML element (e.g., `div { class: "x", ... }`)
	Element(PageElement),
	/// A text literal (e.g., `"Hello, World!"`)
	Text(PageText),
	/// A Rust expression that produces IntoView (e.g., `name` or `format!(...)`)
	Expression(PageExpression),
	/// Conditional rendering (e.g., `if condition { ... }`)
	If(PageIf),
	/// List rendering (e.g., `for item in items { ... }`)
	For(Box<PageFor>),
	/// A component call (e.g., `MyButton(label: "Click")`)
	Component(PageComponent),
	/// Reactive watch block (e.g., `watch { if signal.get() { ... } }`)
	Watch(PageWatch),
}

/// An HTML element node.
///
/// # Example
///
/// ```text
/// div {
///     class: "container",
///     id: "main",
///     @click: |e| { handle_click(e) },
///     h1 { "Title" }
///     p { "Content" }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct PageElement {
	/// Tag name (e.g., "div", "span", "button")
	pub tag: Ident,
	/// Regular attributes (e.g., `class: "x"`)
	pub attrs: Vec<PageAttr>,
	/// Event handlers (e.g., `@click: |e| { ... }`)
	pub events: Vec<IntrinsicEvent>,
	/// Child nodes
	pub children: Vec<PageNode>,
	/// Span for error reporting
	pub span: Span,
}

impl PageElement {
	/// Creates a new PageElement with the given tag.
	pub fn new(tag: Ident, span: Span) -> Self {
		Self {
			tag,
			attrs: Vec::new(),
			events: Vec::new(),
			children: Vec::new(),
			span,
		}
	}

	/// Checks if this is a void element (self-closing, no children allowed).
	pub fn is_void(&self) -> bool {
		matches!(
			self.tag.to_string().as_str(),
			"area"
				| "base" | "br"
				| "col" | "embed"
				| "hr" | "img"
				| "input" | "link"
				| "meta" | "source"
				| "track" | "wbr"
		)
	}
}

/// A regular attribute on an element.
///
/// # Example
///
/// ```text
/// class: "container"
/// id: "main"
/// data_testid: "my-element"  // becomes data-testid
/// disabled: is_disabled      // dynamic value
/// ```
#[derive(Debug, Clone)]
pub struct PageAttr {
	/// Attribute name (underscores converted to hyphens in output)
	pub name: Ident,
	/// Attribute value expression
	pub value: Expr,
	/// Span for error reporting
	pub span: Span,
}

impl PageAttr {
	/// Returns the HTML attribute name (converts underscores to hyphens).
	///
	/// Also strips the `r#` prefix if present (for raw identifiers like `r#for`, `r#type`).
	pub fn html_name(&self) -> String {
		crate::core::attr_utils::ident_to_html_attr_name(&self.name.to_string())
	}
}

/// An intrinsic element event handler (prefixed with `@`).
///
/// # Example
///
/// ```text
/// @click: |e| { handle_click(e) }
/// @input: |e| { handle_input(e) }
/// @submit: |e| { e.prevent_default(); submit() }
/// ```
// The typed variant intentionally preserves concrete syn::Type and syn::Expr fields.
// Boxing payload_type or handler would change the public AST field contract and add
// indirection to a parse-only structure.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum IntrinsicEvent {
	/// A standardized element event resolved through the event catalog.
	Standard {
		/// Closed catalog event identifier.
		event: KnownEvent,
		/// Handler expression.
		handler: Expr,
	},
	/// An explicitly named raw custom DOM event.
	RawCustom {
		/// Exact custom DOM event name.
		name: LitStr,
		/// Handler expression.
		handler: Expr,
	},
	/// An explicitly named custom DOM event with a typed payload.
	TypedCustom {
		/// Exact custom DOM event name.
		name: LitStr,
		/// Payload type carried by the custom event.
		payload_type: Type,
		/// Handler expression.
		handler: Expr,
	},
}

impl IntrinsicEvent {
	/// Returns the event handler expression.
	#[must_use]
	pub const fn handler(&self) -> &Expr {
		match self {
			Self::Standard { handler, .. }
			| Self::RawCustom { handler, .. }
			| Self::TypedCustom { handler, .. } => handler,
		}
	}
}

/// A component event prop whose type is declared by the component's props.
#[derive(Debug, Clone)]
pub struct ComponentEventProp {
	/// Event prop identifier without the `@` prefix.
	pub name: Ident,
	/// Handler expression passed to the component builder.
	pub handler: Expr,
}

/// A text literal node.
///
/// # Example
///
/// ```text
/// "Hello, World!"
/// ```
#[derive(Debug, Clone)]
pub struct PageText {
	/// The text content
	pub content: String,
	/// Span for error reporting
	pub span: Span,
}

/// A Rust expression node that produces IntoView.
///
/// # Example
///
/// ```text
/// name
/// format!("Hello, {}!", name)
/// some_variable.clone()
/// ```
#[derive(Debug, Clone)]
pub struct PageExpression {
	/// The expression
	pub expr: Expr,
	/// Whether this was wrapped in braces `{ expr }`
	pub braced: bool,
	/// Span for error reporting
	pub span: Span,
}

/// Conditional rendering node.
///
/// # Example
///
/// ```text
/// if show_header {
///     h1 { "Header" }
/// }
///
/// if is_admin {
///     span { class: "badge", "Admin" }
/// } else {
///     span { "User" }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct PageIf {
	/// Condition expression
	pub condition: Expr,
	/// Nodes to render when condition is true
	pub then_branch: Vec<PageNode>,
	/// Optional else branch
	pub else_branch: Option<PageElse>,
	/// Span for error reporting
	pub span: Span,
}

/// The else branch of a conditional.
#[derive(Debug, Clone)]
pub enum PageElse {
	/// `else { ... }`
	Block(Vec<PageNode>),
	/// `else if condition { ... }`
	If(Box<PageIf>),
}

/// List rendering node.
///
/// # Example
///
/// ```text
/// for item in items @key(item.id) {
///     li { item.name }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct PageFor {
	/// Loop variable pattern
	pub pat: Pat,
	/// Iterator expression
	pub iter: Expr,
	/// Optional stable key expression for identity-preserving reconciliation.
	pub key: Option<Expr>,
	/// Body nodes (rendered for each item)
	pub body: Vec<PageNode>,
	/// Span for error reporting
	pub span: Span,
}

/// Reactive watch block node.
///
/// Wraps an expression in a reactive context, allowing Signal dependencies
/// to be automatically tracked and the view to be re-rendered when they change.
///
/// # Example
///
/// ```text
/// watch {
///     if error.get().is_some() {
///         div { "Error occurred!" }
///     } else {
///         div { "All good" }
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct PageWatch {
	/// The expression inside the watch block (must return a View).
	/// This is typically an if/else or match expression that depends on Signals.
	pub expr: Box<PageNode>,
	/// Span for error reporting
	pub span: Span,
}

/// A named argument in a component call.
///
/// # Example
///
/// ```text
/// label: "Click me"
/// disabled: is_disabled
/// count: items.len()
/// ```
#[derive(Debug, Clone)]
pub struct PageComponentArg {
	/// Argument name
	pub name: Ident,
	/// Argument value expression
	pub value: Expr,
	/// Span for error reporting
	pub span: Span,
}

/// How a component was invoked in the source DSL.
///
/// The two forms are syntactically distinguishable but semantically
/// equivalent for callers — the parser routes each form to a dedicated
/// codegen branch in `reinhardt-pages-macros`.
///
/// # Forms
///
/// - `Paren` — legacy positional form: `ComponentName(arg1: value1, ...)`.
///   Codegen emits a direct function call with positional arguments.
/// - `Brace` — React-style form: `ComponentName { prop: value, @event: handler, child { ... } }`.
///   Codegen emits a `bon::Builder` chain on a `<ComponentName>Props` struct
///   (spec §3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentInvocationForm {
	/// Legacy positional invocation: `Component(arg1: value1)`.
	Paren,
	/// Brace invocation introduced by spec §3.5: `Component { prop: value }`.
	Brace,
}

/// A named children slot inside a component body.
///
/// Named slots use the `$slotname { ... }` syntax and allow components
/// to accept multiple named children groups mapped to distinct builder setters.
///
/// # Example
///
/// ```text
/// Table {
///     $header { div { "Name" } }
///     $body { for user in users { Row { ... } } }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct NamedSlot {
	/// Slot name without the `$` prefix (e.g., "header", "body")
	pub name: Ident,
	/// Child nodes inside the slot
	pub children: Vec<PageNode>,
	/// Span for error reporting
	pub span: Span,
}

/// A component call node.
///
/// Components are Rust functions that return a View. They can be called with
/// two forms:
///
/// - Positional (legacy): `MyButton(label: "Click", disabled: false)`
/// - Brace (spec §3.5):  `MyButton { label: "Click", @click: h, p { "child" } }`
///
/// The `invocation_form` discriminator records which form was used so codegen
/// can emit the right call pattern (direct positional call vs. bon Builder chain).
///
/// # Example
///
/// ```text
/// // Simple component call (Paren form)
/// MyButton(label: "Click", disabled: false)
///
/// // Component with children (Paren form)
/// MyWrapper(class: "container") {
///     p { "Child content" }
/// }
///
/// // Brace form with intermixed props, events, and children
/// MyCard {
///     title: "Hello",
///     @click: |_| { handle() },
///     p { "child" }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct PageComponent {
	/// Component name (must be a valid Rust function name)
	pub name: Ident,
	/// How the component was invoked (paren vs brace form).
	pub invocation_form: ComponentInvocationForm,
	/// Named arguments / props
	pub args: Vec<PageComponentArg>,
	/// Event props (`@event: handler`). Only populated for `Brace` form;
	/// always empty for `Paren` form to preserve backward compatibility.
	pub events: Vec<ComponentEventProp>,
	/// Optional children (content inside `{ }` after arguments)
	pub children: Option<Vec<PageNode>>,
	/// Named children slots (e.g., `$header { ... }`)
	pub named_slots: Vec<NamedSlot>,
	/// Span for error reporting
	pub span: Span,
}

/// Helper to convert TokenStream to a displayable format for debugging.
pub fn debug_tokens(tokens: &TokenStream) -> String {
	tokens.to_string()
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_page_attr_html_name() {
		// Arrange
		let attr = PageAttr {
			name: Ident::new("data_testid", Span::call_site()),
			value: syn::parse_quote!("test"),
			span: Span::call_site(),
		};

		// Act
		let result = attr.html_name();

		// Assert
		assert_eq!(result, "data-testid");
	}

	#[rstest]
	fn test_page_attr_html_name_aria() {
		// Arrange
		let attr = PageAttr {
			name: Ident::new("aria_label", Span::call_site()),
			value: syn::parse_quote!("Navigation"),
			span: Span::call_site(),
		};

		// Act
		let result = attr.html_name();

		// Assert
		assert_eq!(result, "aria-label");
	}

	#[rstest]
	fn test_page_element_void_tags_are_detected() {
		// Arrange
		let void_tags = ["br", "hr", "img", "input", "meta", "link"];

		for tag in void_tags {
			// Act
			let elem = PageElement::new(Ident::new(tag, Span::call_site()), Span::call_site());

			// Assert
			assert!(elem.is_void(), "{} should be a void element", tag);
		}
	}

	#[rstest]
	fn test_page_element_non_void_tags_are_not_detected() {
		// Arrange
		let non_void_tags = ["div", "span", "p", "button", "a"];

		for tag in non_void_tags {
			// Act
			let elem = PageElement::new(Ident::new(tag, Span::call_site()), Span::call_site());

			// Assert
			assert!(!elem.is_void(), "{} should not be a void element", tag);
		}
	}

	#[rstest]
	fn test_intrinsic_event_preserves_catalog_identity() {
		// Arrange
		let event = IntrinsicEvent::Standard {
			event: KnownEvent::Click,
			handler: syn::parse_quote!(|_| {}),
		};

		// Act
		let result = match event {
			IntrinsicEvent::Standard { event, .. } => event,
			IntrinsicEvent::RawCustom { .. } | IntrinsicEvent::TypedCustom { .. } => {
				panic!("expected standard event")
			}
		};

		// Assert
		assert_eq!(result, KnownEvent::Click);
	}

	#[rstest]
	fn component_invocation_form_round_trip() {
		// Arrange
		let comp = PageComponent {
			name: Ident::new("Card", Span::call_site()),
			invocation_form: ComponentInvocationForm::Brace,
			args: vec![],
			events: vec![],
			children: None,
			named_slots: vec![],
			span: Span::call_site(),
		};

		// Act + Assert
		assert!(matches!(
			comp.invocation_form,
			ComponentInvocationForm::Brace
		));
		assert!(comp.events.is_empty());
		assert_eq!(comp.name.to_string(), "Card");
	}

	#[rstest]
	fn component_invocation_form_paren_is_default_for_legacy_sites() {
		// Arrange: Construction sites that did not opt into the brace form
		// MUST pass `Paren` explicitly so behavior is preserved.
		let comp = PageComponent {
			name: Ident::new("MyButton", Span::call_site()),
			invocation_form: ComponentInvocationForm::Paren,
			args: vec![],
			events: vec![],
			children: None,
			named_slots: vec![],
			span: Span::call_site(),
		};

		// Act + Assert
		assert_eq!(comp.invocation_form, ComponentInvocationForm::Paren);
	}
}
