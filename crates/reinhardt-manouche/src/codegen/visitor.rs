//! IRVisitor trait for code generation.

use crate::ir::{
	AttributeIR, ComponentCallIR, ComponentIR, ConditionalIR, ElementIR, EventIR, ExprIR, FieldIR,
	FormIR, HeadElementIR, HeadIR, LoopIR, NodeIR, PropIR, TextIR, WatchIR,
};

/// Trait for visiting IR nodes and generating output.
///
/// Implement this trait to create a code generator for a specific target
/// (e.g., web-sys, native GUI).
pub trait IRVisitor {
	/// The output type produced by visiting nodes.
	type Output;

	// Component visitors
	/// Visits a component IR.
	fn visit_component(&mut self, ir: &ComponentIR) -> Self::Output;

	/// Visits a property IR.
	fn visit_prop(&mut self, ir: &PropIR) -> Self::Output;

	// Node visitors
	/// Visits a node IR.
	fn visit_node(&mut self, ir: &NodeIR) -> Self::Output {
		match ir {
			NodeIR::Element(e) => self.visit_element(e),
			NodeIR::Text(t) => self.visit_text(t),
			NodeIR::Expression(e) => self.visit_expression(e),
			NodeIR::Conditional(c) => self.visit_conditional(c),
			NodeIR::Loop(l) => self.visit_loop(l),
			NodeIR::Fragment(f) => self.visit_fragment(f),
			NodeIR::Component(c) => self.visit_component_call(c),
			NodeIR::Watch(w) => self.visit_watch(w),
		}
	}

	/// Visits an element IR.
	fn visit_element(&mut self, ir: &ElementIR) -> Self::Output;

	/// Visits a text IR.
	fn visit_text(&mut self, ir: &TextIR) -> Self::Output;

	/// Visits an expression IR.
	fn visit_expression(&mut self, ir: &ExprIR) -> Self::Output;

	/// Visits a conditional IR.
	fn visit_conditional(&mut self, ir: &ConditionalIR) -> Self::Output;

	/// Visits a loop IR.
	fn visit_loop(&mut self, ir: &LoopIR) -> Self::Output;

	/// Visits a fragment (multiple nodes).
	fn visit_fragment(&mut self, ir: &[NodeIR]) -> Self::Output;

	/// Visits a component call IR.
	fn visit_component_call(&mut self, ir: &ComponentCallIR) -> Self::Output;

	/// Visits a watch IR.
	fn visit_watch(&mut self, ir: &WatchIR) -> Self::Output;

	// Attribute & Event visitors
	/// Visits an attribute IR.
	fn visit_attribute(&mut self, ir: &AttributeIR) -> Self::Output;

	/// Visits an event IR.
	fn visit_event(&mut self, ir: &EventIR) -> Self::Output;

	// Form visitors
	/// Visits a form IR.
	fn visit_form(&mut self, ir: &FormIR) -> Self::Output;

	/// Visits a field IR.
	fn visit_field(&mut self, ir: &FieldIR) -> Self::Output;

	// Head visitors
	/// Visits a head IR.
	fn visit_head(&mut self, ir: &HeadIR) -> Self::Output;

	/// Visits a head element IR.
	fn visit_head_element(&mut self, ir: &HeadElementIR) -> Self::Output;
}
