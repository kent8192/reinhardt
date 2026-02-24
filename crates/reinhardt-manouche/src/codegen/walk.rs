//! Helper functions for walking IR trees.

use crate::ir::*;

use super::visitor::IRVisitor;

/// Walks all children of an element and collects outputs.
pub fn walk_element_children<V: IRVisitor>(visitor: &mut V, element: &ElementIR) -> Vec<V::Output> {
	element
		.children
		.iter()
		.map(|n| visitor.visit_node(n))
		.collect()
}

/// Walks all attributes of an element and collects outputs.
pub fn walk_element_attributes<V: IRVisitor>(
	visitor: &mut V,
	element: &ElementIR,
) -> Vec<V::Output> {
	element
		.attributes
		.iter()
		.map(|a| visitor.visit_attribute(a))
		.collect()
}

/// Walks all events of an element and collects outputs.
pub fn walk_element_events<V: IRVisitor>(visitor: &mut V, element: &ElementIR) -> Vec<V::Output> {
	element
		.events
		.iter()
		.map(|e| visitor.visit_event(e))
		.collect()
}

/// Walks all props of a component and collects outputs.
pub fn walk_component_props<V: IRVisitor>(
	visitor: &mut V,
	component: &ComponentIR,
) -> Vec<V::Output> {
	component
		.props
		.iter()
		.map(|p| visitor.visit_prop(p))
		.collect()
}

/// Walks all body nodes of a component and collects outputs.
pub fn walk_component_body<V: IRVisitor>(
	visitor: &mut V,
	component: &ComponentIR,
) -> Vec<V::Output> {
	component
		.body
		.iter()
		.map(|n| visitor.visit_node(n))
		.collect()
}

/// Walks all fields of a form and collects outputs.
pub fn walk_form_fields<V: IRVisitor>(visitor: &mut V, form: &FormIR) -> Vec<V::Output> {
	form.fields.iter().map(|f| visitor.visit_field(f)).collect()
}

/// Walks all elements of a head and collects outputs.
pub fn walk_head_elements<V: IRVisitor>(visitor: &mut V, head: &HeadIR) -> Vec<V::Output> {
	head.elements
		.iter()
		.map(|e| visitor.visit_head_element(e))
		.collect()
}
