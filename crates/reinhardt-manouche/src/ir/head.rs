//! Head IR types for head! macro.

use proc_macro2::Span;

/// IR for a head! macro.
#[derive(Debug)]
pub struct HeadIR {
	/// Head elements
	pub elements: Vec<HeadElementIR>,
	/// Original span
	pub span: Span,
}

/// IR for a head element.
#[derive(Debug)]
pub enum HeadElementIR {
	/// Title element
	Title(TitleIR),
	/// Meta element
	Meta(MetaIR),
	/// Link element
	Link(LinkIR),
	/// Script element
	Script(ScriptIR),
	/// Style element
	Style(StyleIR),
}

/// IR for title element.
#[derive(Debug)]
pub struct TitleIR {
	/// Title content
	pub content: String,
	/// Original span
	pub span: Span,
}

/// IR for meta element.
#[derive(Debug)]
pub struct MetaIR {
	/// Meta attributes (name, content, etc.)
	pub attrs: Vec<(String, String)>,
	/// Original span
	pub span: Span,
}

/// IR for link element.
#[derive(Debug)]
pub struct LinkIR {
	/// Relationship
	pub rel: String,
	/// Href
	pub href: String,
	/// Additional attributes
	pub attrs: Vec<(String, String)>,
	/// Original span
	pub span: Span,
}

/// IR for script element.
#[derive(Debug)]
pub struct ScriptIR {
	/// Source URL (if external)
	pub src: Option<String>,
	/// Inline content (if internal)
	pub content: Option<String>,
	/// Whether async
	pub is_async: bool,
	/// Whether defer
	pub defer: bool,
	/// Module type
	pub is_module: bool,
	/// Original span
	pub span: Span,
}

/// IR for style element.
#[derive(Debug)]
pub struct StyleIR {
	/// CSS content
	pub content: String,
	/// Original span
	pub span: Span,
}
