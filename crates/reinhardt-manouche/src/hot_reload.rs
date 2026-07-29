//! Compiler-side lowering for development template hot reload.

use proc_macro2::{Delimiter, TokenStream, TokenTree};
use quote::ToTokens;
use sha2::{Digest, Sha256};
use std::fmt;

use crate::core::{
	AttrValue, IntrinsicEvent, TypedControlBinding, TypedControlBindingExpr, TypedPageComponent,
	TypedPageElement, TypedPageElse, TypedPageFor, TypedPageIf, TypedPageMacro, TypedPageNode,
};

/// Source information used to derive a stable template identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompilerTemplateKeySeed {
	/// Stable source identifier.
	pub source_id: String,
	/// One-based macro callsite line.
	pub callsite_line: u32,
	/// One-based macro callsite column.
	pub callsite_column: u32,
	/// Deterministic nested-template index.
	pub nested_template_index: u32,
}

/// Compiler-side dynamic slot identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CompilerDynamicSlotId(pub u32);

/// SHA-256 hash of a template's dynamic ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompilerDynamicAbiHash(pub [u8; 32]);

/// Compiler-side description of one dynamic slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerDynamicSlotSignature {
	/// Stable slot identity within the compiled template tree.
	pub slot_id: CompilerDynamicSlotId,
	/// Span-independent canonical Rust tokens for the dynamic input.
	pub canonical_tokens: Vec<String>,
	/// Semantic role of the dynamic input.
	pub semantic_kind: CompilerSlotKind,
}

/// A compiler-side static template and its dynamic ABI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManoucheHotReloadTemplate {
	/// Static DOM tree with dynamic positions represented as slots.
	pub static_tree: CompilerStaticTemplateNode,
	/// Dynamic slots owned directly by this template.
	pub slots: Vec<CompilerDynamicSlotSignature>,
	/// Nested templates for control-flow and component bodies.
	pub nested: Vec<ManoucheHotReloadTemplate>,
	/// ABI hash over dynamic slots and nested template ABIs.
	pub abi_hash: CompilerDynamicAbiHash,
}

/// Static template tree emitted by Manouche lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilerStaticTemplateNode {
	/// HTML element with only literal attributes.
	Element {
		/// Element tag name.
		tag: String,
		/// Literal attributes in source order.
		static_attrs: Vec<(String, String)>,
		/// Child nodes in source order.
		children: Vec<CompilerStaticTemplateNode>,
	},
	/// Literal text.
	Text(String),
	/// Position occupied by a dynamic slot.
	Slot(CompilerDynamicSlotId),
}

/// Semantic role used when computing the dynamic ABI.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CompilerSlotKind {
	/// Rust expression rendered as a child.
	Expression,
	/// Dynamic element attribute.
	DynamicAttribute {
		/// HTML attribute name.
		name: String,
	},
	/// Element event handler.
	Event {
		/// DOM event name.
		name: String,
	},
	/// Conditional branch condition.
	IfCondition,
	/// Loop iteration expression.
	ForIteration,
	/// Stable loop key expression.
	ForKey,
	/// Component invocation and props.
	ComponentInvocation,
	/// Head metadata expression.
	HeadExpression,
}

/// Error returned when a template cannot be lowered safely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotReloadError {
	/// The typed AST node is not supported by the development ABI.
	UnsupportedNode(String),
	/// The AST does not provide enough information for a stable slot.
	AmbiguousSlot(String),
	/// Canonical token generation failed or received invalid input.
	Canonicalization(String),
}

impl fmt::Display for HotReloadError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::UnsupportedNode(message) => write!(formatter, "unsupported node: {message}"),
			Self::AmbiguousSlot(message) => write!(formatter, "ambiguous slot: {message}"),
			Self::Canonicalization(message) => {
				write!(formatter, "canonicalization failed: {message}")
			}
		}
	}
}

impl std::error::Error for HotReloadError {}

/// Lowers one validated page macro into static-template and dynamic-ABI data.
pub fn lower_page_macro(
	macro_ast: &TypedPageMacro,
) -> Result<ManoucheHotReloadTemplate, HotReloadError> {
	let mut context = LoweringContext::default();
	let mut template = context.lower_template(&macro_ast.body().nodes)?;
	if let Some(head) = &macro_ast.head {
		let slot_id = context.allocate_slot_id();
		template.slots.push(CompilerDynamicSlotSignature {
			slot_id,
			canonical_tokens: vec![canonical_tokens(head)?],
			semantic_kind: CompilerSlotKind::HeadExpression,
		});
		template.abi_hash = calculate_abi_hash(&template.slots, &template.nested);
	}
	Ok(template)
}

#[derive(Default)]
struct LoweringContext {
	next_slot_id: u32,
}

impl LoweringContext {
	fn allocate_slot_id(&mut self) -> CompilerDynamicSlotId {
		let slot_id = CompilerDynamicSlotId(self.next_slot_id);
		self.next_slot_id = self
			.next_slot_id
			.checked_add(1)
			.expect("template dynamic slot id overflow");
		slot_id
	}

	fn lower_template(
		&mut self,
		nodes: &[TypedPageNode],
	) -> Result<ManoucheHotReloadTemplate, HotReloadError> {
		let mut slots = Vec::new();
		let mut nested = Vec::new();
		let mut children = Vec::with_capacity(nodes.len());
		for node in nodes {
			children.push(self.lower_node(node, &mut slots, &mut nested)?);
		}

		let static_tree = match children.len() {
			0 => CompilerStaticTemplateNode::Text(String::new()),
			1 => children.pop().expect("one child was checked above"),
			_ => CompilerStaticTemplateNode::Element {
				tag: "__reinhardt_fragment__".to_owned(),
				static_attrs: Vec::new(),
				children,
			},
		};
		let abi_hash = calculate_abi_hash(&slots, &nested);
		Ok(ManoucheHotReloadTemplate {
			static_tree,
			slots,
			nested,
			abi_hash,
		})
	}

	fn lower_node(
		&mut self,
		node: &TypedPageNode,
		slots: &mut Vec<CompilerDynamicSlotSignature>,
		nested: &mut Vec<ManoucheHotReloadTemplate>,
	) -> Result<CompilerStaticTemplateNode, HotReloadError> {
		match node {
			TypedPageNode::Text(text) => Ok(CompilerStaticTemplateNode::Text(text.content.clone())),
			TypedPageNode::Expression(expression) => {
				let slot_id = self.allocate_slot_id();
				slots.push(CompilerDynamicSlotSignature {
					slot_id,
					canonical_tokens: vec![canonical_tokens(&expression.expr)?],
					semantic_kind: CompilerSlotKind::Expression,
				});
				Ok(CompilerStaticTemplateNode::Slot(slot_id))
			}
			TypedPageNode::Element(element) => self.lower_element(element, slots, nested),
			TypedPageNode::If(if_node) => self.lower_if(if_node, slots, nested),
			TypedPageNode::For(for_node) => self.lower_for(for_node, slots, nested),
			TypedPageNode::Component(component) => self.lower_component(component, slots, nested),
			TypedPageNode::Watch(watch) => self.lower_node(&watch.expr, slots, nested),
		}
	}

	fn lower_element(
		&mut self,
		element: &TypedPageElement,
		slots: &mut Vec<CompilerDynamicSlotSignature>,
		nested: &mut Vec<ManoucheHotReloadTemplate>,
	) -> Result<CompilerStaticTemplateNode, HotReloadError> {
		let mut static_attrs = Vec::new();
		let mut structural_slot = None;
		for attribute in &element.attrs {
			match &attribute.value {
				AttrValue::StringLit(value) => {
					static_attrs.push((attribute.html_name(), value.value()));
				}
				AttrValue::BoolLit(value) => {
					static_attrs.push((attribute.html_name(), value.value().to_string()));
				}
				AttrValue::IntLit(value) => {
					static_attrs.push((attribute.html_name(), value.to_token_stream().to_string()));
				}
				AttrValue::FloatLit(value) => {
					static_attrs.push((attribute.html_name(), value.to_token_stream().to_string()));
				}
				AttrValue::Dynamic(expression) => {
					let slot_id = self.allocate_slot_id();
					structural_slot = Some(slot_id);
					slots.push(CompilerDynamicSlotSignature {
						slot_id,
						canonical_tokens: vec![canonical_tokens(expression)?],
						semantic_kind: CompilerSlotKind::DynamicAttribute {
							name: attribute.html_name(),
						},
					});
				}
			}
		}

		if let Some(binding) = &element.control_binding {
			let slot_id = self.allocate_slot_id();
			structural_slot = Some(slot_id);
			slots.push(CompilerDynamicSlotSignature {
				slot_id,
				canonical_tokens: canonical_binding(binding)?,
				semantic_kind: CompilerSlotKind::Expression,
			});
		}
		for event in &element.events {
			let (name, canonical_tokens) = event_parts(event)?;
			let slot_id = self.allocate_slot_id();
			structural_slot = Some(slot_id);
			slots.push(CompilerDynamicSlotSignature {
				slot_id,
				canonical_tokens,
				semantic_kind: CompilerSlotKind::Event { name },
			});
		}

		let children = element
			.children
			.iter()
			.map(|child| self.lower_node(child, slots, nested))
			.collect::<Result<Vec<_>, _>>()?;
		Ok(match structural_slot {
			Some(slot_id) => CompilerStaticTemplateNode::Slot(slot_id),
			None => CompilerStaticTemplateNode::Element {
				tag: element.tag.to_string(),
				static_attrs,
				children,
			},
		})
	}

	fn lower_if(
		&mut self,
		if_node: &TypedPageIf,
		slots: &mut Vec<CompilerDynamicSlotSignature>,
		nested: &mut Vec<ManoucheHotReloadTemplate>,
	) -> Result<CompilerStaticTemplateNode, HotReloadError> {
		let condition_slot = self.allocate_slot_id();
		slots.push(CompilerDynamicSlotSignature {
			slot_id: condition_slot,
			canonical_tokens: vec![canonical_tokens(&if_node.condition)?],
			semantic_kind: CompilerSlotKind::IfCondition,
		});
		nested.push(self.lower_template(&if_node.then_branch)?);
		if let Some(else_branch) = &if_node.else_branch {
			match else_branch {
				TypedPageElse::Block(nodes) => nested.push(self.lower_template(nodes)?),
				TypedPageElse::If(nested_if) => nested.push(self.lower_if_template(nested_if)?),
			}
		}
		Ok(CompilerStaticTemplateNode::Slot(condition_slot))
	}

	fn lower_if_template(
		&mut self,
		if_node: &TypedPageIf,
	) -> Result<ManoucheHotReloadTemplate, HotReloadError> {
		let mut slots = Vec::new();
		let mut nested = Vec::new();
		let static_tree = self.lower_if(if_node, &mut slots, &mut nested)?;
		let abi_hash = calculate_abi_hash(&slots, &nested);
		Ok(ManoucheHotReloadTemplate {
			static_tree,
			slots,
			nested,
			abi_hash,
		})
	}

	fn lower_for(
		&mut self,
		for_node: &TypedPageFor,
		slots: &mut Vec<CompilerDynamicSlotSignature>,
		nested: &mut Vec<ManoucheHotReloadTemplate>,
	) -> Result<CompilerStaticTemplateNode, HotReloadError> {
		let iteration_slot = self.allocate_slot_id();
		let mut structural_slot = iteration_slot;
		slots.push(CompilerDynamicSlotSignature {
			slot_id: iteration_slot,
			canonical_tokens: vec![
				canonical_tokens(&for_node.pat)?,
				canonical_tokens(&for_node.iter)?,
			],
			semantic_kind: CompilerSlotKind::ForIteration,
		});
		if let Some(key) = &for_node.key {
			let key_slot = self.allocate_slot_id();
			structural_slot = key_slot;
			slots.push(CompilerDynamicSlotSignature {
				slot_id: key_slot,
				canonical_tokens: vec![canonical_tokens(key)?],
				semantic_kind: CompilerSlotKind::ForKey,
			});
		}
		nested.push(self.lower_template(&for_node.body)?);
		Ok(CompilerStaticTemplateNode::Slot(structural_slot))
	}

	fn lower_component(
		&mut self,
		component: &TypedPageComponent,
		slots: &mut Vec<CompilerDynamicSlotSignature>,
		nested: &mut Vec<ManoucheHotReloadTemplate>,
	) -> Result<CompilerStaticTemplateNode, HotReloadError> {
		let slot_id = self.allocate_slot_id();
		slots.push(CompilerDynamicSlotSignature {
			slot_id,
			canonical_tokens: canonical_component(component)?,
			semantic_kind: CompilerSlotKind::ComponentInvocation,
		});
		if let Some(children) = &component.children {
			nested.push(self.lower_template(children)?);
		}
		for named_slot in &component.named_slots {
			nested.push(self.lower_template(&named_slot.children)?);
		}
		Ok(CompilerStaticTemplateNode::Slot(slot_id))
	}
}

fn event_parts(event: &IntrinsicEvent) -> Result<(String, Vec<String>), HotReloadError> {
	match event {
		IntrinsicEvent::Standard { event, handler } => Ok((
			event.to_string(),
			vec!["variant:standard".to_owned(), canonical_tokens(handler)?],
		)),
		IntrinsicEvent::RawCustom { name, handler } => Ok((
			name.value(),
			vec!["variant:raw_custom".to_owned(), canonical_tokens(handler)?],
		)),
		IntrinsicEvent::TypedCustom {
			name,
			payload_type,
			handler,
		} => Ok((
			name.value(),
			vec![
				"variant:typed_custom".to_owned(),
				format!("payload_type:{}", canonical_tokens(payload_type)?),
				canonical_tokens(handler)?,
			],
		)),
	}
}

fn canonical_binding(binding: &TypedControlBinding) -> Result<Vec<String>, HotReloadError> {
	let mut tokens = vec![format!("kind:{:?}", binding.kind)];
	match &binding.expression {
		TypedControlBindingExpr::Direct(value) => tokens.push(canonical_tokens(value)?),
		TypedControlBindingExpr::NumberWithError { value, error } => {
			tokens.push(canonical_tokens(value)?);
			tokens.push(canonical_tokens(error)?);
		}
	}
	if let Some(radio_value) = &binding.radio_value {
		tokens.push(canonical_tokens(radio_value)?);
	}
	Ok(tokens)
}

fn canonical_component(component: &TypedPageComponent) -> Result<Vec<String>, HotReloadError> {
	let mut tokens = vec![
		format!("name:{}", component.name),
		format!("form:{:?}", component.invocation_form),
	];
	for argument in &component.args {
		tokens.push(format!("arg:{}", argument.name));
		tokens.push(canonical_tokens(&argument.value)?);
	}
	for event in &component.events {
		tokens.push(format!("event:{}", event.name));
		tokens.push(canonical_tokens(&event.handler)?);
	}
	for named_slot in &component.named_slots {
		tokens.push(format!("slot:{}", named_slot.name));
	}
	Ok(tokens)
}

fn calculate_abi_hash(
	slots: &[CompilerDynamicSlotSignature],
	nested: &[ManoucheHotReloadTemplate],
) -> CompilerDynamicAbiHash {
	let mut hasher = Sha256::new();
	for slot in slots {
		hasher.update(slot.slot_id.0.to_le_bytes());
		hasher.update(slot_kind_key(&slot.semantic_kind).as_bytes());
		for token in &slot.canonical_tokens {
			hasher.update(token.as_bytes());
			hasher.update([0]);
		}
	}
	for nested_template in nested {
		hasher.update(nested_template.abi_hash.0);
	}
	CompilerDynamicAbiHash(hasher.finalize().into())
}

fn slot_kind_key(kind: &CompilerSlotKind) -> String {
	match kind {
		CompilerSlotKind::Expression => "expression".to_owned(),
		CompilerSlotKind::DynamicAttribute { name } => format!("dynamic_attribute:{name}"),
		CompilerSlotKind::Event { name } => format!("event:{name}"),
		CompilerSlotKind::IfCondition => "if_condition".to_owned(),
		CompilerSlotKind::ForIteration => "for_iteration".to_owned(),
		CompilerSlotKind::ForKey => "for_key".to_owned(),
		CompilerSlotKind::ComponentInvocation => "component_invocation".to_owned(),
		CompilerSlotKind::HeadExpression => "head_expression".to_owned(),
	}
}

fn canonical_tokens<T: ToTokens>(value: &T) -> Result<String, HotReloadError> {
	canonical_stream(&value.to_token_stream())
}

fn canonical_stream(stream: &TokenStream) -> Result<String, HotReloadError> {
	stream
		.clone()
		.into_iter()
		.map(canonical_token)
		.collect::<Result<Vec<_>, _>>()
		.map(|tokens| tokens.join("|"))
}

fn canonical_token(token: TokenTree) -> Result<String, HotReloadError> {
	match token {
		TokenTree::Ident(ident) => Ok(format!("ident:{}", ident)),
		TokenTree::Punct(punct) => Ok(format!("punct:{}:{:?}", punct.as_char(), punct.spacing())),
		TokenTree::Literal(literal) => Ok(format!("literal:{}", literal)),
		TokenTree::Group(group) => {
			let delimiter = match group.delimiter() {
				Delimiter::Parenthesis => "paren",
				Delimiter::Brace => "brace",
				Delimiter::Bracket => "bracket",
				Delimiter::None => "none",
			};
			Ok(format!(
				"group:{delimiter}[{}]",
				canonical_stream(&group.stream())?
			))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{parser::parse_page, validator::validate_page};
	use quote::quote;

	fn lower(input: TokenStream) -> ManoucheHotReloadTemplate {
		let parsed = parse_page(input).expect("page syntax should parse");
		let typed = validate_page(&parsed).expect("page syntax should validate");
		lower_page_macro(&typed).expect("page should lower")
	}

	#[test]
	fn lowers_static_element_and_literal_attributes() {
		let template = lower(quote! { { div { class: "shell", "Hello" } } });
		assert_eq!(
			template.static_tree,
			CompilerStaticTemplateNode::Element {
				tag: "div".to_owned(),
				static_attrs: vec![("class".to_owned(), "shell".to_owned())],
				children: vec![CompilerStaticTemplateNode::Text("Hello".to_owned())],
			}
		);
		assert!(template.slots.is_empty());
	}

	#[test]
	fn dynamic_expression_changes_abi_but_static_text_does_not() {
		let first = lower(quote! { |count: Signal<i32>| { div { "Count" count.get() } } });
		let static_edit = lower(quote! { |count: Signal<i32>| { div { "Total" count.get() } } });
		let dynamic_edit =
			lower(quote! { |count: Signal<i32>| { div { "Count" count.clone().get() } } });
		assert_ne!(first.static_tree, static_edit.static_tree);
		assert_eq!(first.abi_hash, static_edit.abi_hash);
		assert_ne!(first.abi_hash, dynamic_edit.abi_hash);
	}

	#[test]
	fn dynamic_attribute_and_event_are_distinct_slots() {
		let template = lower(quote! {
			{ button { class: class_name, @click: |event| { handle(event) }, "Click" } }
		});
		assert_eq!(template.slots.len(), 2);
		assert!(matches!(
			template.slots[0].semantic_kind,
			CompilerSlotKind::DynamicAttribute { ref name } if name == "class"
		));
		assert!(matches!(
			template.slots[1].semantic_kind,
			CompilerSlotKind::Event { ref name } if name == "click"
		));
		assert_eq!(
			template.static_tree,
			CompilerStaticTemplateNode::Slot(CompilerDynamicSlotId(1))
		);
	}

	#[test]
	fn event_variants_have_distinct_stable_dynamic_abis() {
		let standard = lower(quote! {
			{ button { @click: |_| {}, "Click" } }
		});
		let raw = lower(quote! {
			{ button { @custom("click"): |_| {}, "Click" } }
		});
		let typed_u64 = lower(quote! {
			{ button { @custom::<u64>("click"): |_| {}, "Click" } }
		});
		let typed_string = lower(quote! {
			{ button { @custom::<String>("click"): |_| {}, "Click" } }
		});

		assert_ne!(standard.abi_hash, raw.abi_hash);
		assert_ne!(raw.abi_hash, typed_u64.abi_hash);
		assert_ne!(typed_u64.abi_hash, typed_string.abi_hash);
		assert_eq!(
			typed_u64.abi_hash,
			lower(quote! {
				{ button { @custom::<u64>("click"): |_| {}, "Click" } }
			})
			.abi_hash
		);
	}

	#[test]
	fn control_flow_and_components_create_nested_templates() {
		let template = lower(quote! {
			|show: bool, items: Vec<i32>| {
				if show { div { "shown" } } else { div { "hidden" } }
				for item in items @key(item) { span { { item } } }
				Card { title: "Card", body { "content" } }
			}
		});
		assert!(!template.nested.is_empty());
		assert!(
			template
				.slots
				.iter()
				.any(|slot| matches!(slot.semantic_kind, CompilerSlotKind::IfCondition))
		);
		assert!(
			template
				.slots
				.iter()
				.any(|slot| matches!(slot.semantic_kind, CompilerSlotKind::ForKey))
		);
		assert!(
			template
				.slots
				.iter()
				.any(|slot| matches!(slot.semantic_kind, CompilerSlotKind::ComponentInvocation))
		);
	}
}
