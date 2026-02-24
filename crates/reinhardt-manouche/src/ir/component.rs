//! Component IR types for page! macro.

use proc_macro2::Span;
use syn::Expr;

/// IR for a page! component.
#[derive(Debug)]
pub struct ComponentIR {
	/// Component properties
	pub props: Vec<PropIR>,
	/// Body nodes
	pub body: Vec<NodeIR>,
	/// Original span
	pub span: Span,
}

/// IR for a component property.
#[derive(Debug)]
pub struct PropIR {
	/// Property name
	pub name: String,
	/// Property type (as string representation)
	pub ty: String,
	/// Original span
	pub span: Span,
}

/// IR for a node in the component body.
#[derive(Debug)]
pub enum NodeIR {
	/// HTML element
	Element(ElementIR),
	/// Text content
	Text(TextIR),
	/// Dynamic expression
	Expression(ExprIR),
	/// Conditional rendering
	Conditional(ConditionalIR),
	/// Loop rendering
	Loop(LoopIR),
	/// Fragment (multiple nodes)
	Fragment(Vec<NodeIR>),
	/// Component call
	Component(ComponentCallIR),
	/// Watch block
	Watch(WatchIR),
}

/// IR for an HTML element.
#[derive(Debug)]
pub struct ElementIR {
	/// Tag name
	pub tag: String,
	/// Attributes
	pub attributes: Vec<AttributeIR>,
	/// Event handlers
	pub events: Vec<EventIR>,
	/// Child nodes
	pub children: Vec<NodeIR>,
	/// Original span
	pub span: Span,
}

/// IR for a text node.
#[derive(Debug)]
pub struct TextIR {
	/// Text content
	pub content: String,
	/// Original span
	pub span: Span,
}

/// IR for a dynamic expression.
#[derive(Debug)]
pub struct ExprIR {
	/// The expression
	pub expr: Expr,
	/// Original span
	pub span: Span,
}

/// IR for an attribute.
#[derive(Debug)]
pub struct AttributeIR {
	/// Attribute name
	pub name: String,
	/// Attribute value
	pub value: AttrValueIR,
	/// Original span
	pub span: Span,
}

/// IR for an attribute value.
#[derive(Debug)]
pub enum AttrValueIR {
	/// Static string value
	Static(String),
	/// Dynamic expression value
	Dynamic(Expr),
	/// Boolean flag (attribute present = true)
	Flag,
}

/// IR for an event handler.
#[derive(Debug)]
pub struct EventIR {
	/// Event name (without "on" prefix)
	pub name: String,
	/// Handler expression
	pub handler: Expr,
	/// Original span
	pub span: Span,
}

/// IR for conditional rendering.
#[derive(Debug)]
pub struct ConditionalIR {
	/// Condition expression
	pub condition: Expr,
	/// Body when condition is true
	pub then_body: Vec<NodeIR>,
	/// Optional else/else-if branches
	pub else_branch: Option<Box<ElseBranchIR>>,
	/// Original span
	pub span: Span,
}

/// IR for else/else-if branches.
#[derive(Debug)]
pub enum ElseBranchIR {
	/// else if branch
	ElseIf(ConditionalIR),
	/// else branch
	Else(Vec<NodeIR>),
}

/// IR for loop rendering.
#[derive(Debug)]
pub struct LoopIR {
	/// Loop variable pattern
	pub pattern: String,
	/// Iterator expression
	pub iterator: Expr,
	/// Loop body
	pub body: Vec<NodeIR>,
	/// Original span
	pub span: Span,
}

/// IR for a component call.
#[derive(Debug)]
pub struct ComponentCallIR {
	/// Component name/path
	pub name: String,
	/// Arguments
	pub args: Vec<(String, Expr)>,
	/// Original span
	pub span: Span,
}

/// IR for a watch block.
#[derive(Debug)]
pub struct WatchIR {
	/// Dependencies to watch
	pub dependencies: Vec<Expr>,
	/// Body to re-render
	pub body: Vec<NodeIR>,
	/// Original span
	pub span: Span,
}
