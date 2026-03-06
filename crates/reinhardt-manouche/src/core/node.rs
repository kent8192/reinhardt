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
use syn::{Expr, FnArg, Ident, Pat, Type};

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
	/// Closure-style parameters (props)
	pub params: Vec<PageParam>,
	/// The body containing the view tree
	pub body: PageBody,
	/// Span for error reporting
	pub span: Span,
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
	For(PageFor),
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
	pub events: Vec<PageEvent>,
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
	pub fn html_name(&self) -> String {
		self.name.to_string().replace('_', "-")
	}
}

/// An event handler on an element (prefixed with `@`).
///
/// # Example
///
/// ```text
/// @click: |e| { handle_click(e) }
/// @input: |e| { handle_input(e) }
/// @submit: |e| { e.prevent_default(); submit() }
/// ```
#[derive(Debug, Clone)]
pub struct PageEvent {
	/// Event type name (e.g., "click", "input", "submit")
	pub event_type: Ident,
	/// Handler expression (closure)
	pub handler: Expr,
	/// Span for error reporting
	pub span: Span,
}

impl PageEvent {
	/// Returns the DOM event type string.
	pub fn dom_event_type(&self) -> String {
		self.event_type.to_string()
	}
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
/// for item in items {
///     li { item.name }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct PageFor {
	/// Loop variable pattern
	pub pat: Pat,
	/// Iterator expression
	pub iter: Expr,
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

/// A component call node.
///
/// Components are Rust functions that return a View. They are called with
/// named arguments using the syntax: `ComponentName(arg1: value1, arg2: value2)`.
///
/// # Example
///
/// ```text
/// // Simple component call
/// MyButton(label: "Click", disabled: false)
///
/// // Component with children
/// MyWrapper(class: "container") {
///     p { "Child content" }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct PageComponent {
	/// Component name (must be a valid Rust function name)
	pub name: Ident,
	/// Named arguments
	pub args: Vec<PageComponentArg>,
	/// Optional children (content inside `{ }` after arguments)
	pub children: Option<Vec<PageNode>>,
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
	fn test_page_event_dom_event_type() {
		// Arrange
		let event = PageEvent {
			event_type: Ident::new("click", Span::call_site()),
			handler: syn::parse_quote!(|_| {}),
			span: Span::call_site(),
		};

		// Act
		let result = event.dom_event_type();

		// Assert
		assert_eq!(result, "click");
	}
}
