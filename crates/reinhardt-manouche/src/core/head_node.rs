//! AST node definitions for the head! macro.

use proc_macro2::Span;
use syn::{Expr, Ident};

/// The top-level AST node for a head! macro invocation.
#[derive(Debug)]
pub struct HeadMacro {
	/// The elements in the head section
	pub elements: Vec<HeadElement>,
	/// Span for error reporting
	pub span: Span,
}

/// An element in the head section.
#[derive(Debug)]
pub struct HeadElement {
	/// Element tag name (title, meta, link, script, style)
	pub tag: Ident,
	/// Attributes on the element
	pub attrs: Vec<HeadAttr>,
	/// Content (for title, style, script)
	pub content: Option<HeadContent>,
	/// Span for error reporting
	pub span: Span,
}

/// An attribute on a head element.
#[derive(Debug)]
pub struct HeadAttr {
	/// Attribute name
	pub name: Ident,
	/// Attribute value
	pub value: Expr,
	/// Span for error reporting
	pub span: Span,
}

/// Content within a head element.
#[derive(Debug)]
pub enum HeadContent {
	/// Text content (for title)
	Text(String),
	/// Expression content
	Expr(Expr),
}

/// Typed version of HeadMacro after validation.
#[derive(Debug)]
pub struct TypedHeadMacro {
	/// Validated head elements
	pub elements: Vec<TypedHeadElement>,
	/// Span for error reporting
	pub span: Span,
}

/// Typed head element after validation.
#[derive(Debug)]
pub struct TypedHeadElement {
	/// Element tag name
	pub tag: String,
	/// Typed attributes
	pub attrs: Vec<TypedHeadAttr>,
	/// Typed content
	pub content: Option<TypedHeadContent>,
	/// Span for error reporting
	pub span: Span,
}

/// Typed attribute on a head element.
#[derive(Debug)]
pub struct TypedHeadAttr {
	/// Attribute name (HTML-formatted)
	pub name: String,
	/// Attribute value as string
	pub value: String,
	/// Span for error reporting
	pub span: Span,
}

/// Typed content within a head element.
#[derive(Debug)]
pub enum TypedHeadContent {
	/// Static text content
	Static(String),
	/// Dynamic expression content
	Dynamic(Expr),
}
