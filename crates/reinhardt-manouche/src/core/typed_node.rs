//! Typed AST node definitions for the page! macro.
//!
//! This module provides typed versions of AST nodes, where attribute values
//! have explicit type information. This enables stronger compile-time validation.
//!
//! The typed AST is produced by the validator after transforming and validating
//! the untyped AST from the parser.

use proc_macro2::Span;
use syn::{Expr, Ident, Pat};

use super::{
	ComponentEventProp, ComponentInvocationForm, IntrinsicEvent, PageComponentArg, PageExpression,
	PageParam, PageText, types::AttrValue,
};

/// The top-level typed AST node representing a validated page! macro invocation.
///
/// This is the result of successful validation and transformation of an untyped
/// `PageMacro`. All validation rules have been enforced at this point.
#[derive(Debug)]
pub struct TypedPageMacro {
	/// Optional head expression.
	///
	/// When present, the generated view will be wrapped with `.with_head(head_expr)`.
	pub head: Option<syn::Expr>,
	/// The validated and typed page macro form.
	pub form: TypedPageMacroForm,
	/// Span for error reporting
	pub span: Span,
}

impl TypedPageMacro {
	/// Returns the validated and typed body.
	pub fn body(&self) -> &TypedPageBody {
		match &self.form {
			TypedPageMacroForm::StrictClosure { body, .. }
			| TypedPageMacroForm::ImplicitBody { body, .. } => body,
		}
	}

	/// Returns the closure-style parameters when this macro uses strict closure form.
	pub fn params(&self) -> &[PageParam] {
		match &self.form {
			TypedPageMacroForm::StrictClosure { params, .. } => params,
			TypedPageMacroForm::ImplicitBody { .. } => &[],
		}
	}

	/// Returns implicit captures discovered for body-only form.
	pub fn implicit_captures(&self) -> &[ImplicitPageCapture] {
		match &self.form {
			TypedPageMacroForm::StrictClosure { .. } => &[],
			TypedPageMacroForm::ImplicitBody { captures, .. } => captures,
		}
	}

	/// Returns `true` when this macro uses body-only implicit capture form.
	pub fn is_implicit_body(&self) -> bool {
		matches!(self.form, TypedPageMacroForm::ImplicitBody { .. })
	}
}

/// A captured identifier used by body-only `page!` form.
#[derive(Debug, Clone)]
pub struct ImplicitPageCapture {
	/// Captured identifier.
	pub ident: syn::Ident,
	/// Span for error reporting.
	pub span: Span,
}

/// The typed syntactic form used by a `page!` macro invocation.
#[derive(Debug)]
pub enum TypedPageMacroForm {
	/// A strict closure form such as `page!(|name: String| { ... })`.
	StrictClosure {
		/// Closure-style parameters (props).
		params: Vec<PageParam>,
		/// The validated and typed body.
		body: TypedPageBody,
	},
	/// A body-only form with implicit captures.
	ImplicitBody {
		/// Captured identifiers discovered in the body.
		captures: Vec<ImplicitPageCapture>,
		/// The validated and typed body.
		body: TypedPageBody,
	},
}

/// The typed body of a page! macro, containing validated nodes.
#[derive(Debug)]
pub struct TypedPageBody {
	/// Validated root nodes
	pub nodes: Vec<TypedPageNode>,
	/// Span for error reporting
	pub span: Span,
}

/// A typed node in the page! DSL.
///
/// This is similar to `PageNode` but with type-safe attribute values.
#[derive(Debug)]
pub enum TypedPageNode {
	/// An HTML element with typed attributes
	Element(TypedPageElement),
	/// A text literal
	Text(PageText),
	/// A Rust expression
	Expression(PageExpression),
	/// Conditional rendering
	If(TypedPageIf),
	/// List rendering
	For(Box<TypedPageFor>),
	/// A component call with typed children
	Component(TypedPageComponent),
	/// Reactive watch block
	Watch(TypedPageWatch),
}

/// A typed HTML element node.
///
/// The key difference from `PageElement` is that attributes are typed,
/// allowing for stronger validation of attribute values.
#[derive(Debug)]
pub struct TypedPageElement {
	/// Tag name
	pub tag: Ident,
	/// Typed attributes (with `AttrValue` instead of `Expr`)
	pub attrs: Vec<TypedPageAttr>,
	/// Typed controlled-value binding, kept separate from HTML attributes.
	pub control_binding: Option<Box<TypedControlBinding>>,
	/// Catalog-resolved intrinsic event handlers.
	pub events: Vec<IntrinsicEvent>,
	/// Validated child nodes
	pub children: Vec<TypedPageNode>,
	/// Whether compile-time accessibility validation is disabled for this element.
	pub a11y_disabled: bool,
	/// Span for error reporting
	pub span: Span,
}

impl TypedPageElement {
	/// Creates a new TypedPageElement with the given tag.
	pub fn new(tag: Ident, span: Span) -> Self {
		Self {
			tag,
			attrs: Vec::new(),
			control_binding: None,
			events: Vec::new(),
			children: Vec::new(),
			a11y_disabled: false,
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

/// A structurally validated controlled-value binding.
#[derive(Debug)]
pub struct TypedControlBinding {
	/// Control behavior selected from the element and its static attributes.
	pub kind: TypedControlBindingKind,
	/// Value signal expression and optional numeric error signal expression.
	pub expression: TypedControlBindingExpr,
	/// Owned radio choice expression.
	pub radio_value: Option<Expr>,
	/// Span for error reporting.
	pub span: Span,
}

/// The structurally classified control kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedControlBindingKind {
	/// Text input or textarea.
	Text,
	/// Numeric input.
	Number,
	/// Checkbox input.
	Checkbox,
	/// Radio input.
	Radio,
	/// File input.
	File,
	/// Single-select control.
	SelectOne,
	/// Multi-select control.
	SelectMany,
}

/// Expressions supplied to a controlled-value binding.
#[derive(Debug)]
pub enum TypedControlBindingExpr {
	/// A direct value signal expression.
	Direct(Box<Expr>),
	/// A numeric value signal paired with a parse-error signal.
	NumberWithError {
		/// Numeric value signal expression.
		value: Box<Expr>,
		/// Numeric parse-error signal expression.
		error: Box<Expr>,
	},
}

/// A typed attribute on an element.
///
/// Unlike `PageAttr`, this uses `AttrValue` instead of `Expr`,
/// which allows distinguishing between literals and dynamic expressions.
///
/// # Example
///
/// ```text
/// class: "container"  // AttrValue::StringLit
/// id: element_id      // AttrValue::Dynamic
/// disabled: true      // AttrValue::BoolLit
/// ```
#[derive(Debug)]
pub struct TypedPageAttr {
	/// Attribute name (underscores converted to hyphens in output)
	pub name: Ident,
	/// Typed attribute value
	pub value: AttrValue,
	/// Span for error reporting
	pub span: Span,
}

impl TypedPageAttr {
	/// Returns the HTML attribute name (converts underscores to hyphens).
	///
	/// Also strips the `r#` prefix if present (for raw identifiers like `r#for`, `r#type`).
	pub fn html_name(&self) -> String {
		crate::core::attr_utils::ident_to_html_attr_name(&self.name.to_string())
	}
}

/// Typed conditional rendering node.
#[derive(Debug)]
pub struct TypedPageIf {
	/// Condition expression
	pub condition: Expr,
	/// Validated nodes to render when condition is true
	pub then_branch: Vec<TypedPageNode>,
	/// Optional typed else branch
	pub else_branch: Option<TypedPageElse>,
	/// Span for error reporting
	pub span: Span,
}

/// The typed else branch of a conditional.
#[derive(Debug)]
pub enum TypedPageElse {
	/// `else { ... }`
	Block(Vec<TypedPageNode>),
	/// `else if condition { ... }`
	If(Box<TypedPageIf>),
}

/// Typed list rendering node.
#[derive(Debug)]
pub struct TypedPageFor {
	/// Loop variable pattern
	pub pat: Pat,
	/// Iterator expression
	pub iter: Expr,
	/// Optional stable key expression for identity-preserving reconciliation.
	pub key: Option<Expr>,
	/// Validated body nodes (rendered for each item)
	pub body: Vec<TypedPageNode>,
	/// Span for error reporting
	pub span: Span,
}

/// Typed reactive watch block node.
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
#[derive(Debug)]
pub struct TypedPageWatch {
	/// The expression inside the watch block (must return a View).
	/// This is typically an if/else or match expression that depends on Signals.
	pub expr: Box<TypedPageNode>,
	/// Span for error reporting
	pub span: Span,
}

/// A typed named children slot inside a component body.
///
/// This is the validated counterpart of `NamedSlot`, produced by the validator
/// after transforming the slot's children from untyped to typed AST nodes.
#[derive(Debug)]
pub struct TypedNamedSlot {
	/// Slot name without the `$` prefix
	pub name: Ident,
	/// Validated child nodes inside the slot
	pub children: Vec<TypedPageNode>,
	/// Span for error reporting
	pub span: Span,
}

/// Typed component call node.
///
/// Components are Rust functions that return a View. They can be invoked in
/// two syntactic forms — see [`ComponentInvocationForm`] for the distinction.
#[derive(Debug)]
pub struct TypedPageComponent {
	/// Component name (must be a valid Rust function name)
	pub name: Ident,
	/// How the component was invoked (paren vs brace form).
	///
	/// Codegen branches on this field to emit either a direct positional
	/// function call (paren form) or a `bon::Builder` chain on the
	/// `<Name>Props` struct (brace form, spec §3.5).
	pub invocation_form: ComponentInvocationForm,
	/// Named arguments / props (unchanged from untyped version)
	pub args: Vec<PageComponentArg>,
	/// Event props (`@event: handler`). Only populated for the `Brace` form;
	/// always empty for the `Paren` form.
	pub events: Vec<ComponentEventProp>,
	/// Optional typed children (content inside `{ }` after arguments)
	pub children: Option<Vec<TypedPageNode>>,
	/// Typed named children slots
	pub named_slots: Vec<TypedNamedSlot>,
	/// Span for error reporting
	pub span: Span,
}

#[cfg(test)]
mod tests {
	use super::*;
	use proc_macro2::Span;
	use rstest::rstest;
	use syn::parse_quote;

	#[rstest]
	fn test_typed_page_attr_html_name() {
		// Arrange
		let attr = TypedPageAttr {
			name: Ident::new("data_testid", Span::call_site()),
			value: AttrValue::from_expr(parse_quote!("test")),
			span: Span::call_site(),
		};

		// Act
		let html_name = attr.html_name();

		// Assert
		assert_eq!(html_name, "data-testid");
	}

	#[rstest]
	fn test_typed_page_attr_html_name_aria() {
		// Arrange
		let attr = TypedPageAttr {
			name: Ident::new("aria_label", Span::call_site()),
			value: AttrValue::from_expr(parse_quote!("Navigation")),
			span: Span::call_site(),
		};

		// Act
		let html_name = attr.html_name();

		// Assert
		assert_eq!(html_name, "aria-label");
	}

	#[rstest]
	fn test_typed_page_element_is_void() {
		// Arrange
		let void_tags = ["br", "hr", "img", "input", "meta", "link"];
		let non_void_tags = ["div", "span", "p", "button", "a"];

		// Act & Assert
		for tag in void_tags {
			let elem = TypedPageElement::new(Ident::new(tag, Span::call_site()), Span::call_site());
			assert!(elem.is_void(), "{} should be a void element", tag);
		}

		for tag in non_void_tags {
			let elem = TypedPageElement::new(Ident::new(tag, Span::call_site()), Span::call_site());
			assert!(!elem.is_void(), "{} should not be a void element", tag);
		}
	}

	#[rstest]
	fn test_typed_page_attr_with_string_lit() {
		// Arrange
		let attr = TypedPageAttr {
			name: Ident::new("src", Span::call_site()),
			value: AttrValue::from_expr(parse_quote!("/image.png")),
			span: Span::call_site(),
		};

		// Act
		let is_string_literal = attr.value.is_string_literal();

		// Assert
		assert!(is_string_literal);
	}

	#[rstest]
	fn test_typed_page_attr_with_dynamic() {
		// Arrange
		let attr = TypedPageAttr {
			name: Ident::new("src", Span::call_site()),
			value: AttrValue::from_expr(parse_quote!(image_url)),
			span: Span::call_site(),
		};

		// Act
		let is_dynamic = attr.value.is_dynamic();

		// Assert
		assert!(is_dynamic);
	}
}
