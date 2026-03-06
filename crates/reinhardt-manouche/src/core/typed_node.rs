//! Typed AST node definitions for the page! macro.
//!
//! This module provides typed versions of AST nodes, where attribute values
//! have explicit type information. This enables stronger compile-time validation.
//!
//! The typed AST is produced by the validator after transforming and validating
//! the untyped AST from the parser.

use proc_macro2::Span;
use syn::{Expr, Ident, Pat};

use super::{PageComponentArg, PageEvent, PageExpression, PageParam, PageText, types::AttrValue};

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
	/// Closure-style parameters (props)
	pub params: Vec<PageParam>,
	/// The validated and typed body
	pub body: TypedPageBody,
	/// Span for error reporting
	pub span: Span,
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
	For(TypedPageFor),
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
	/// Event handlers (unchanged from untyped version)
	pub events: Vec<PageEvent>,
	/// Validated child nodes
	pub children: Vec<TypedPageNode>,
	/// Span for error reporting
	pub span: Span,
}

impl TypedPageElement {
	/// Creates a new TypedPageElement with the given tag.
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
		let name = self.name.to_string();
		// Remove r# prefix if present (raw identifiers in Rust)
		let name = name.strip_prefix("r#").unwrap_or(&name);
		name.replace('_', "-")
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

/// Typed component call node.
///
/// Components are Rust functions that return a View. They are called with
/// named arguments using the syntax: `ComponentName(arg1: value1, arg2: value2)`.
#[derive(Debug)]
pub struct TypedPageComponent {
	/// Component name (must be a valid Rust function name)
	pub name: Ident,
	/// Named arguments (unchanged from untyped version)
	pub args: Vec<PageComponentArg>,
	/// Optional typed children (content inside `{ }` after arguments)
	pub children: Option<Vec<TypedPageNode>>,
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
