//! Intermediate Representation (IR) for code generation.
//!
//! This module defines platform-agnostic IR types that serve as the
//! input for code generation backends.

mod component;
pub mod form;
pub mod head;
pub mod lower;

pub use component::{
	AttrValueIR, AttributeIR, ComponentCallIR, ComponentIR, ConditionalIR, ElementIR, ElseBranchIR,
	EventIR, ExprIR, LoopIR, NodeIR, PropIR, TextIR, WatchIR,
};
pub use form::{
	FieldIR, FieldTypeIR, FormActionIR, FormIR, FormMethodIR, FormStylingIR, ValidatorIR, WidgetIR,
	WidgetTypeIR,
};
pub use head::{HeadElementIR, HeadIR, LinkIR, MetaIR, ScriptIR, StyleIR, TitleIR};
pub use lower::{lower_form, lower_head, lower_page};

/// Top-level IR enum for all macro types.
#[derive(Debug)]
pub enum IR {
	/// IR for page! macro
	Component(ComponentIR),
	/// IR for form! macro
	Form(FormIR),
	/// IR for head! macro
	Head(HeadIR),
}
