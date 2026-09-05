//! In-memory Page-backed tree for native component testing.

use std::cell::RefCell;
use std::rc::Rc;

use reinhardt_core::reactive::{ReactiveScope, runtime::NodeId as ReactiveNodeId};
use reinhardt_core::types::page::{
	ControlBinding, ControlBindingError, ControlKind, ControlValue, ControlWriteOutcome, EventName,
	NativeEventFile, NativeEventTarget, Page, PageEventHandler, is_boolean_attr,
	is_boolean_attr_truthy,
};

use crate::reactive::query::QueryClient;

use super::fixture::{EventFixtureError, TargetStatePatch};
use super::scheduler::SchedulerScope;
#[cfg(feature = "msw")]
use super::server_fn_mock::SharedServerFnMocks;

/// Stable identifier for a node in the native test DOM.
pub(crate) type NodeId = usize;

/// Shared mutable screen state used by handles.
pub(crate) struct ScreenInner {
	/// Rendered native test DOM.
	pub dom: TestDom,
	/// Reactive scope that owns hook state created by the rendered view.
	pub reactive_scope: ReactiveScope,
	/// Harness scheduler active for this screen.
	pub scheduler: Rc<SchedulerScope>,
	/// Query client owned by this screen.
	pub query_client: QueryClient,
	/// Server function mocks registered for this screen.
	#[cfg(feature = "msw")]
	pub mocks: SharedServerFnMocks,
}

impl ScreenInner {
	pub(crate) fn rerender_reactive_anchors(&mut self) {
		let Self {
			dom,
			reactive_scope,
			..
		} = self;
		reactive_scope.enter(|| dom.rerender_reactive_anchors());
	}
}

pub(crate) struct TestDom {
	nodes: Vec<TestNode>,
	root: NodeId,
}

#[derive(Clone)]
pub(crate) struct ElementNode {
	pub tag: String,
	attrs: Vec<(String, String)>,
	reactive_attrs: Vec<reinhardt_core::types::page::ReactiveAttribute>,
	pub children: Vec<NodeId>,
	parent: Option<NodeId>,
	is_void: bool,
	event_handlers: Vec<(EventName, PageEventHandler)>,
	value: Option<String>,
	checked: bool,
	selected_values: Vec<String>,
	files: Vec<NativeEventFile>,
	content_editable: bool,
	control_binding: Option<ControlBinding>,
	option_value: Option<String>,
	is_composing: bool,
	pending_raw: Option<String>,
	last_committed_raw: Option<String>,
	last_observed_control_value: Option<ControlValue>,
	last_observed_signal_revision: Option<usize>,
}

enum TestNode {
	Removed,
	Root {
		children: Vec<NodeId>,
	},
	Element(Box<ElementNode>),
	Text {
		text: String,
		parent: Option<NodeId>,
	},
	ReactiveAnchor {
		children: Vec<NodeId>,
		parent: Option<NodeId>,
		render: Rc<dyn Fn() -> Page + 'static>,
	},
}

pub(crate) struct PendingControlBindingWrite {
	node_id: NodeId,
	binding: ControlBinding,
	value: ControlValue,
	raw: Option<String>,
	dedupe_next_input: bool,
}

pub(crate) struct CompletedControlBindingWrite {
	node_id: NodeId,
	binding: ControlBinding,
	outcome: ControlWriteOutcome,
	raw: Option<String>,
	dedupe_next_input: bool,
}

#[derive(Default)]
struct RejectedNumberSnapshotContext {
	snapshots: Vec<RejectedNumberSnapshot>,
	positions: Vec<(ReactiveNodeId, usize)>,
}

struct RejectedNumberSnapshot {
	target: ReactiveNodeId,
	position: usize,
	raw: String,
}

impl RejectedNumberSnapshotContext {
	fn from_subtree(dom: &TestDom, parent: NodeId) -> Self {
		let mut context = Self::default();
		for child in dom.children(parent) {
			context.collect_from_subtree(dom, *child);
		}
		context.positions.clear();
		context
	}

	fn collect_from_subtree(&mut self, dom: &TestDom, node_id: NodeId) {
		if let Some(element) = dom.element(node_id)
			&& let Some(binding) = element.control_binding.as_ref()
			&& binding.kind() == ControlKind::Number
		{
			let position = self.next_position(binding);
			if let Some(raw) = element.pending_raw.as_ref() {
				self.snapshots.push(RejectedNumberSnapshot {
					target: binding.target(),
					position,
					raw: raw.clone(),
				});
			}
		}
		for child in dom.children(node_id) {
			self.collect_from_subtree(dom, *child);
		}
	}

	fn take(&mut self, binding: &ControlBinding) -> Option<String> {
		if binding.kind() != ControlKind::Number {
			return None;
		}
		let position = self.next_position(binding);
		let index = self.snapshots.iter().position(|snapshot| {
			snapshot.target == binding.target() && snapshot.position == position
		})?;
		Some(self.snapshots.remove(index).raw)
	}

	fn next_position(&mut self, binding: &ControlBinding) -> usize {
		let position = self
			.positions
			.iter_mut()
			.find(|(target, _)| *target == binding.target());
		match position {
			Some((_, next)) => {
				let current = *next;
				*next += 1;
				current
			}
			None => {
				self.positions.push((binding.target(), 1));
				0
			}
		}
	}
}

impl PendingControlBindingWrite {
	pub(crate) fn execute(self) -> Result<CompletedControlBindingWrite, ControlBindingError> {
		let outcome = self.binding.write(self.value)?;
		Ok(CompletedControlBindingWrite {
			node_id: self.node_id,
			binding: self.binding,
			outcome,
			raw: self.raw,
			dedupe_next_input: self.dedupe_next_input,
		})
	}
}

impl TestDom {
	/// Renders a Page into an in-memory native test DOM.
	pub(crate) fn render(page: Page) -> Self {
		let mut dom = Self {
			nodes: vec![TestNode::Root {
				children: Vec::new(),
			}],
			root: 0,
		};
		dom.append_page(dom.root, page);
		dom
	}

	pub(crate) fn root(&self) -> NodeId {
		self.root
	}

	pub(crate) fn contains(&self, node_id: NodeId) -> bool {
		self.nodes
			.get(node_id)
			.is_some_and(|node| !matches!(node, TestNode::Removed))
	}

	pub(crate) fn element(&self, node_id: NodeId) -> Option<&ElementNode> {
		match self.nodes.get(node_id)? {
			TestNode::Element(node) => Some(node),
			_ => None,
		}
	}

	pub(crate) fn text_content(&self, node_id: NodeId) -> String {
		match self.nodes.get(node_id) {
			Some(TestNode::Removed) => String::new(),
			Some(TestNode::Root { children }) => self.children_text(children),
			Some(TestNode::Element(node)) => self.children_text(&node.children),
			Some(TestNode::Text { text, .. }) => text.clone(),
			Some(TestNode::ReactiveAnchor { children, .. }) => self.children_text(children),
			None => String::new(),
		}
	}

	pub(crate) fn visible_text_content(&self, node_id: NodeId) -> String {
		match self.nodes.get(node_id) {
			Some(TestNode::Removed) => String::new(),
			Some(TestNode::Root { children }) => self.children_visible_text(children),
			Some(TestNode::Element(_node)) if self.is_hidden(node_id) => String::new(),
			Some(TestNode::Element(node)) => self.children_visible_text(&node.children),
			Some(TestNode::Text { text, .. }) => text.clone(),
			Some(TestNode::ReactiveAnchor { children, .. }) => self.children_visible_text(children),
			None => String::new(),
		}
	}

	pub(crate) fn all_elements(&self) -> Vec<NodeId> {
		let mut nodes = Vec::new();
		self.collect_elements(self.root, &mut nodes);
		nodes
	}

	pub(crate) fn visible_elements(&self) -> Vec<NodeId> {
		self.all_elements()
			.into_iter()
			.filter(|node_id| !self.is_hidden(*node_id))
			.collect()
	}

	pub(crate) fn is_hidden(&self, node_id: NodeId) -> bool {
		let mut current = Some(node_id);
		while let Some(id) = current {
			if let Some(node) = self.element(id) {
				if node.has_attr("hidden") || node.attr("aria-hidden") == Some("true") {
					return true;
				}
				current = node.parent;
			} else {
				current = self.parent(id);
			}
		}
		false
	}

	pub(crate) fn find_element_by_id(&self, id: &str) -> Option<NodeId> {
		self.all_elements()
			.into_iter()
			.find(|node_id| self.element(*node_id).and_then(|node| node.attr("id")) == Some(id))
	}

	pub(crate) fn label_for(&self, id: &str) -> Option<NodeId> {
		self.all_elements().into_iter().find(|node_id| {
			self.element(*node_id).is_some_and(|node| {
				node.tag == "label" && node.attr("for") == Some(id) && !self.is_hidden(*node_id)
			})
		})
	}

	pub(crate) fn closest_label(&self, node_id: NodeId) -> Option<NodeId> {
		let mut current = self.parent(node_id);
		while let Some(id) = current {
			if self.element(id).is_some_and(|node| node.tag == "label") {
				return Some(id);
			}
			current = self.parent(id);
		}
		None
	}

	pub(crate) fn event_handlers(
		&self,
		node_id: NodeId,
		event_name: &EventName,
		bubbles: bool,
	) -> Vec<(NodeId, PageEventHandler)> {
		let mut handlers = Vec::new();
		let mut current = Some(node_id);
		while let Some(id) = current {
			if let Some(node) = self.element(id) {
				handlers.extend(
					node.event_handlers
						.iter()
						.filter(|(candidate, _)| candidate.as_str() == event_name.as_str())
						.map(|(_, handler)| (id, handler.clone())),
				);
			}
			if !bubbles {
				break;
			}
			current = self.parent(id);
		}
		handlers
	}

	pub(crate) fn event_target(&self, node_id: NodeId) -> Option<NativeEventTarget> {
		let node = self.element(node_id)?;
		let mut target = node
			.attrs
			.iter()
			.fold(
				NativeEventTarget::new(&node.tag),
				|target, (name, value)| target.with_attribute(name, value),
			)
			.with_text_content(self.text_content(node_id));

		if let Some(value) = &node.value {
			target = target.with_value(value);
		} else if node.content_editable {
			target = target.with_value(self.text_content(node_id));
		}

		if node.tag.eq_ignore_ascii_case("input")
			&& node.attr("type").is_some_and(|kind| {
				["checkbox", "radio"]
					.iter()
					.any(|known| kind.eq_ignore_ascii_case(known))
			}) {
			target = target.with_checked(node.checked);
		}

		target = target.with_selected_values(node.selected_values.clone());
		target = target.with_files(node.files.clone());
		Some(target.with_content_editable(node.content_editable))
	}

	pub(crate) fn suppresses_events(&self, node_id: NodeId) -> bool {
		let mut current = Some(node_id);
		while let Some(id) = current {
			if self
				.element(id)
				.is_some_and(ElementNode::is_disabled_form_control)
			{
				return true;
			}
			current = self.parent(id);
		}
		false
	}

	pub(crate) fn apply_target_state(
		&mut self,
		node_id: NodeId,
		patch: &TargetStatePatch,
	) -> Result<(), EventFixtureError> {
		let node = match self.nodes.get_mut(node_id) {
			Some(TestNode::Element(node)) => node,
			_ => return Ok(()),
		};
		let final_content_editable = patch.content_editable.unwrap_or(node.content_editable);
		let supports_final_value =
			node.supports_value_with_content_editable(final_content_editable);
		let unsupported_property = if patch.value.is_some() && !supports_final_value {
			Some("value")
		} else if patch.checked.is_some()
			&& !(node.tag.eq_ignore_ascii_case("input")
				&& node.attr("type").is_some_and(|kind| {
					["checkbox", "radio"]
						.iter()
						.any(|known| kind.eq_ignore_ascii_case(known))
				})) {
			Some("checked")
		} else if patch.selected_values.is_some() && !node.tag.eq_ignore_ascii_case("select") {
			Some("selected_values")
		} else if patch.files.is_some()
			&& !(node.tag.eq_ignore_ascii_case("input")
				&& node
					.attr("type")
					.is_some_and(|kind| kind.eq_ignore_ascii_case("file")))
		{
			Some("files")
		} else {
			None
		};
		if let Some(property) = unsupported_property {
			return Err(EventFixtureError::UnsupportedTargetState {
				property,
				actual_tag: node.tag.clone(),
			});
		}

		node.content_editable = final_content_editable;
		if let Some(selected_values) = &patch.selected_values {
			node.value = if node.tag.eq_ignore_ascii_case("select") {
				Some(selected_values.first().cloned().unwrap_or_default())
			} else {
				selected_values.first().cloned()
			};
			node.selected_values.clone_from(selected_values);
		} else if let Some(value) = &patch.value {
			let value = node
				.control_binding
				.as_ref()
				.map(|binding| {
					normalize_native_control_value(node, binding, ControlValue::Text(value.clone()))
				})
				.and_then(|value| match value {
					ControlValue::Text(value) => Some(value),
					_ => None,
				})
				.unwrap_or_else(|| value.clone());
			node.value = Some(value.clone());
			if node.tag.eq_ignore_ascii_case("select") {
				node.selected_values = vec![value.clone()];
			}
		}
		if let Some(checked) = patch.checked {
			node.checked = checked;
		}
		if let Some(files) = &patch.files {
			node.files.clone_from(files);
		}
		let refresh_selected_options = node.tag.eq_ignore_ascii_case("select")
			&& (patch.value.is_some() || patch.selected_values.is_some());
		if refresh_selected_options {
			self.refresh_selected_options(node_id);
		}
		Ok(())
	}

	pub(crate) fn validate_control_binding(
		&self,
		node_id: NodeId,
	) -> Result<(), ControlBindingError> {
		let Some(node) = self.element(node_id) else {
			return Ok(());
		};
		if let Some(binding) = &node.control_binding {
			node.validate_control_binding(binding)?;
		}
		Ok(())
	}

	pub(crate) fn prepare_control_binding_commit(
		&mut self,
		node_id: NodeId,
		event_name: &EventName,
		input_is_composing: bool,
	) -> Result<(bool, Option<PendingControlBindingWrite>), ControlBindingError> {
		let node = match self.nodes.get_mut(node_id) {
			Some(TestNode::Element(node)) => node,
			_ => return Ok((false, None)),
		};
		let Some(binding) = node.control_binding.clone() else {
			return Ok((false, None));
		};
		node.validate_control_binding(&binding)?;

		let event_name = event_name.as_str();
		match binding.kind() {
			ControlKind::Text | ControlKind::Number => match event_name {
				"compositionstart" => {
					node.is_composing = true;
					node.pending_raw.clone_from(&node.value);
					Ok((true, None))
				}
				"compositionend" => {
					node.is_composing = false;
					let raw = node
						.value
						.clone()
						.or_else(|| node.pending_raw.clone())
						.ok_or(ControlBindingError::MissingProperty {
							control: binding.kind(),
							property: "value",
						})?;
					Ok((
						true,
						Some(PendingControlBindingWrite {
							node_id,
							binding,
							value: ControlValue::Text(raw.clone()),
							raw: Some(raw),
							dedupe_next_input: true,
						}),
					))
				}
				"input" => {
					let raw = node
						.value
						.clone()
						.ok_or(ControlBindingError::MissingProperty {
							control: binding.kind(),
							property: "value",
						})?;
					if node.is_composing || input_is_composing {
						node.last_committed_raw = None;
						node.pending_raw = Some(raw);
						return Ok((true, None));
					}
					if node.last_committed_raw.take().as_deref() == Some(raw.as_str()) {
						// The matching post-composition input is already committed by
						// compositionend. Retain rejected raw numeric text until the
						// bound signal changes, matching browser behavior.
						return Ok((true, None));
					}
					Ok((
						true,
						Some(PendingControlBindingWrite {
							node_id,
							binding,
							value: ControlValue::Text(raw.clone()),
							raw: Some(raw),
							dedupe_next_input: false,
						}),
					))
				}
				_ => Ok((false, None)),
			},
			ControlKind::Checkbox | ControlKind::Radio if event_name == "change" => Ok((
				true,
				Some(PendingControlBindingWrite {
					node_id,
					binding,
					value: ControlValue::Checked(node.checked),
					raw: None,
					dedupe_next_input: false,
				}),
			)),
			ControlKind::SelectOne if event_name == "change" => Ok((
				true,
				Some(PendingControlBindingWrite {
					node_id,
					binding,
					value: ControlValue::Text(node.value.clone().unwrap_or_default()),
					raw: None,
					dedupe_next_input: false,
				}),
			)),
			ControlKind::SelectMany if event_name == "change" => Ok((
				true,
				Some(PendingControlBindingWrite {
					node_id,
					binding,
					value: ControlValue::SelectedValues(node.selected_values.clone()),
					raw: None,
					dedupe_next_input: false,
				}),
			)),
			_ => Ok((false, None)),
		}
	}

	pub(crate) fn record_control_binding_commit(
		&mut self,
		completed: CompletedControlBindingWrite,
	) {
		let Some(TestNode::Element(node)) = self.nodes.get_mut(completed.node_id) else {
			return;
		};
		node.last_committed_raw = completed
			.dedupe_next_input
			.then(|| completed.raw.clone())
			.flatten();
		node.record_write_outcome(&completed.binding, completed.outcome, completed.raw);
	}

	pub(crate) fn refresh_control_bindings(&mut self) {
		let mut selects = Vec::new();
		for node_id in 0..self.nodes.len() {
			let binding = {
				let TestNode::Element(element) = &mut self.nodes[node_id] else {
					continue;
				};
				element.refresh_reactive_attributes();
				element.control_binding.clone()
			};
			let Some(binding) = binding else {
				continue;
			};
			let conflicting_range = {
				let TestNode::Element(element) = &self.nodes[node_id] else {
					continue;
				};
				has_conflicting_native_range_binding(&self.nodes, Some(node_id), element, &binding)
			};
			let TestNode::Element(element) = &mut self.nodes[node_id] else {
				continue;
			};
			let value = binding.read();
			let normalized = normalize_native_control_value(element, &binding, value.clone());
			let applied = apply_native_control_normalization(
				element,
				&binding,
				&value,
				normalized,
				!conflicting_range,
			);
			let signal_revision = reinhardt_core::reactive::with_runtime(|runtime| {
				runtime.signal_revision(binding.target())
			});
			let retain_invalid_raw = binding.kind() == ControlKind::Number
				&& element.pending_raw.is_some()
				&& element.last_observed_control_value.as_ref() == Some(&value)
				&& element.last_observed_signal_revision == Some(signal_revision);
			if !retain_invalid_raw {
				element.pending_raw = None;
				element.apply_control_value(&binding, applied);
			}
			element.last_observed_control_value = Some(value);
			element.last_observed_signal_revision = Some(signal_revision);
			if matches!(
				binding.kind(),
				ControlKind::SelectOne | ControlKind::SelectMany
			) {
				selects.push(node_id);
			}
		}
		for select in selects {
			self.refresh_selected_options(select);
		}
	}

	pub(crate) fn value(&self, node_id: NodeId) -> Option<String> {
		self.element(node_id).and_then(|node| {
			node.value
				.clone()
				.or_else(|| node.content_editable.then(|| self.text_content(node_id)))
		})
	}

	pub(crate) fn children(&self, node_id: NodeId) -> &[NodeId] {
		match self.nodes.get(node_id) {
			Some(TestNode::Removed) => &[],
			Some(TestNode::Root { children }) => children,
			Some(TestNode::Element(node)) => &node.children,
			Some(TestNode::ReactiveAnchor { children, .. }) => children,
			_ => &[],
		}
	}

	pub(crate) fn text_node(&self, node_id: NodeId) -> Option<&str> {
		match self.nodes.get(node_id) {
			Some(TestNode::Text { text, .. }) => Some(text),
			_ => None,
		}
	}

	pub(crate) fn is_void(&self, node_id: NodeId) -> bool {
		self.element(node_id).is_some_and(|node| node.is_void)
	}

	fn append_page(&mut self, parent: NodeId, page: Page) {
		let mut rejected_number_snapshots = RejectedNumberSnapshotContext::default();
		self.append_page_with_rejected_number_snapshots(
			parent,
			page,
			&mut rejected_number_snapshots,
		);
	}

	fn append_page_with_rejected_number_snapshots(
		&mut self,
		parent: NodeId,
		page: Page,
		rejected_number_snapshots: &mut RejectedNumberSnapshotContext,
	) {
		match page {
			Page::Element(element) => {
				let option_value = element
					.tag_name()
					.eq_ignore_ascii_case("option")
					.then(|| crate::ssr::control_binding::option_value(&element));
				let (
					tag,
					attrs,
					reactive_attrs,
					children,
					is_void,
					event_handlers,
					control_binding,
				) = element.into_parts_with_control_binding();
				let attrs = attrs
					.into_iter()
					.map(|(name, value)| (name.into_owned(), value.into_owned()))
					.collect::<Vec<_>>();
				let value = attrs
					.iter()
					.find(|(name, _)| name == "value")
					.map(|(_, value)| value.clone());
				let checked = attrs.iter().any(|(name, _)| name == "checked");
				let selected_values = value.clone().into_iter().collect();
				let content_editable = attrs
					.iter()
					.find(|(name, _)| name == "contenteditable")
					.is_some_and(|(_, value)| value != "false");
				let last_observed_control_value =
					control_binding.as_ref().map(ControlBinding::read);
				let last_observed_signal_revision = control_binding.as_ref().map(|binding| {
					reinhardt_core::reactive::with_runtime(|runtime| {
						runtime.signal_revision(binding.target())
					})
				});
				let refresh_controlled_select = control_binding.as_ref().is_some_and(|binding| {
					matches!(
						binding.kind(),
						ControlKind::SelectOne | ControlKind::SelectMany
					)
				});
				let rejected_number_raw = control_binding
					.as_ref()
					.and_then(|binding| rejected_number_snapshots.take(binding));
				let mut element_node = ElementNode {
					tag: tag.into_owned(),
					attrs,
					reactive_attrs,
					children: Vec::new(),
					parent: Some(parent),
					is_void,
					event_handlers,
					value,
					checked,
					selected_values,
					files: Vec::new(),
					content_editable,
					control_binding,
					option_value,
					is_composing: false,
					pending_raw: rejected_number_raw.clone(),
					last_committed_raw: None,
					last_observed_control_value: last_observed_control_value.clone(),
					last_observed_signal_revision,
				};
				element_node.refresh_reactive_attributes();
				let binding_supported = element_node
					.control_binding
					.as_ref()
					.is_none_or(|binding| element_node.validate_control_binding(binding).is_ok());
				if binding_supported
					&& let (Some(binding), Some(value)) = (
						element_node.control_binding.clone(),
						last_observed_control_value,
					) {
					element_node.project_controlled_attributes(&binding, &value);
					let normalized =
						normalize_native_control_value(&element_node, &binding, value.clone());
					let normalization_write_back_allowed = !has_conflicting_native_range_binding(
						&self.nodes,
						None,
						&element_node,
						&binding,
					);
					let applied = apply_native_control_normalization(
						&element_node,
						&binding,
						&value,
						normalized,
						normalization_write_back_allowed,
					);
					element_node.last_observed_control_value = Some(binding.read());
					element_node.last_observed_signal_revision =
						Some(reinhardt_core::reactive::with_runtime(|runtime| {
							runtime.signal_revision(binding.target())
						}));
					element_node.apply_control_value(&binding, applied);
				}
				if let Some(raw) = rejected_number_raw {
					element_node.value = Some(raw);
				}
				let node_id = self.push_node(parent, TestNode::Element(Box::new(element_node)));
				let suppress_bound_textarea_children = self.element(node_id).is_some_and(|node| {
					node.tag.eq_ignore_ascii_case("textarea")
						&& node
							.control_binding
							.as_ref()
							.is_some_and(|binding| binding.kind() == ControlKind::Text)
				});
				if !suppress_bound_textarea_children {
					for child in children {
						self.append_page_with_rejected_number_snapshots(
							node_id,
							child,
							rejected_number_snapshots,
						);
					}
				}
				if refresh_controlled_select {
					self.refresh_selected_options(node_id);
				}
			}
			Page::Text(text) => {
				self.push_node(
					parent,
					TestNode::Text {
						text: text.into_owned(),
						parent: Some(parent),
					},
				);
			}
			Page::Fragment(children) => {
				for child in children {
					self.append_page_with_rejected_number_snapshots(
						parent,
						child,
						rejected_number_snapshots,
					);
				}
			}
			Page::KeyedFragment(children) => {
				for (_, child) in children {
					self.append_page_with_rejected_number_snapshots(
						parent,
						child,
						rejected_number_snapshots,
					);
				}
			}
			Page::Outlet(outlet) => {
				let id = outlet.id().map(str::to_string);
				if let Some(child) = outlet.into_child() {
					self.append_page_with_rejected_number_snapshots(
						parent,
						child,
						rejected_number_snapshots,
					);
				} else if let Some(id) = id {
					self.push_node(
						parent,
						TestNode::Element(Box::new(ElementNode {
							tag: "reinhardt-outlet".to_string(),
							attrs: vec![
								("data-rh-outlet-id".to_string(), id),
								("style".to_string(), "display: contents;".to_string()),
							],
							reactive_attrs: Vec::new(),
							children: Vec::new(),
							parent: Some(parent),
							is_void: false,
							event_handlers: Default::default(),
							value: None,
							checked: false,
							selected_values: Vec::new(),
							files: Vec::new(),
							content_editable: false,
							control_binding: None,
							option_value: None,
							is_composing: false,
							pending_raw: None,
							last_committed_raw: None,
							last_observed_control_value: None,
							last_observed_signal_revision: None,
						})),
					);
				}
			}
			Page::Empty => {}
			Page::WithHead { view, .. } => self.append_page_with_rejected_number_snapshots(
				parent,
				*view,
				rejected_number_snapshots,
			),
			#[cfg(feature = "hmr")]
			Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => self
				.append_page_with_rejected_number_snapshots(
					parent,
					*view,
					rejected_number_snapshots,
				),
			Page::ReactiveIf(reactive_if) => {
				let (condition, then_view, else_view) = reactive_if.into_parts();
				let render: Rc<dyn Fn() -> Page + 'static> = Rc::new(move || {
					if condition() {
						then_view()
					} else {
						else_view()
					}
				});
				self.append_reactive_anchor(parent, render, rejected_number_snapshots);
			}
			Page::Reactive(reactive) => {
				let render_arc = reactive.into_render();
				let render: Rc<dyn Fn() -> Page + 'static> = Rc::new(move || render_arc());
				self.append_reactive_anchor(parent, render, rejected_number_snapshots);
			}
			Page::Suspense(node) => {
				let render: Rc<dyn Fn() -> Page + 'static> = Rc::new(move || node.render_branch());
				self.append_reactive_anchor(parent, render, rejected_number_snapshots);
			}
			Page::Deferred(node) => {
				let content = node.render_content();
				self.append_page_with_rejected_number_snapshots(
					parent,
					content,
					rejected_number_snapshots,
				);
			}
		}
	}

	fn append_reactive_anchor(
		&mut self,
		parent: NodeId,
		render: Rc<dyn Fn() -> Page + 'static>,
		rejected_number_snapshots: &mut RejectedNumberSnapshotContext,
	) {
		let anchor = self.push_node(
			parent,
			TestNode::ReactiveAnchor {
				children: Vec::new(),
				parent: Some(parent),
				render: Rc::clone(&render),
			},
		);
		self.append_page_with_rejected_number_snapshots(
			anchor,
			render(),
			rejected_number_snapshots,
		);
	}

	fn push_node(&mut self, parent: NodeId, node: TestNode) -> NodeId {
		let node_id = self.nodes.len();
		self.nodes.push(node);
		match self.nodes.get_mut(parent) {
			Some(TestNode::Removed) => {}
			Some(TestNode::Root { children }) => children.push(node_id),
			Some(TestNode::Element(element)) => element.children.push(node_id),
			Some(TestNode::ReactiveAnchor { children, .. }) => children.push(node_id),
			_ => {}
		}
		node_id
	}

	fn children_text(&self, children: &[NodeId]) -> String {
		children
			.iter()
			.map(|child| self.text_content(*child))
			.collect::<String>()
	}

	fn children_visible_text(&self, children: &[NodeId]) -> String {
		children
			.iter()
			.map(|child| self.visible_text_content(*child))
			.collect::<String>()
	}

	fn collect_elements(&self, node_id: NodeId, output: &mut Vec<NodeId>) {
		if self.element(node_id).is_some() {
			output.push(node_id);
		}
		for child in self.children(node_id) {
			self.collect_elements(*child, output);
		}
	}

	fn parent(&self, node_id: NodeId) -> Option<NodeId> {
		match self.nodes.get(node_id) {
			Some(TestNode::Removed) => None,
			Some(TestNode::Element(node)) => node.parent,
			Some(TestNode::Text { parent, .. }) => *parent,
			Some(TestNode::ReactiveAnchor { parent, .. }) => *parent,
			_ => None,
		}
	}

	pub(crate) fn rerender_reactive_anchors(&mut self) {
		let anchors = self
			.nodes
			.iter()
			.enumerate()
			.filter_map(|(node_id, node)| match node {
				TestNode::ReactiveAnchor { render, .. } => Some((node_id, Rc::clone(render))),
				_ => None,
			})
			.collect::<Vec<_>>();

		for (anchor, render) in anchors {
			if !self.contains(anchor) {
				continue;
			}
			let mut rejected_number_snapshots =
				RejectedNumberSnapshotContext::from_subtree(self, anchor);
			self.clear_children(anchor);
			self.append_page_with_rejected_number_snapshots(
				anchor,
				render(),
				&mut rejected_number_snapshots,
			);
		}
	}

	fn clear_children(&mut self, node_id: NodeId) {
		let children = match self.nodes.get_mut(node_id) {
			Some(TestNode::Root { children }) => std::mem::take(children),
			Some(TestNode::Element(node)) => std::mem::take(&mut node.children),
			Some(TestNode::ReactiveAnchor { children, .. }) => std::mem::take(children),
			_ => Vec::new(),
		};
		for child in children {
			self.remove_subtree(child);
		}
	}

	fn remove_subtree(&mut self, node_id: NodeId) {
		let children = self.children(node_id).to_vec();
		for child in children {
			self.remove_subtree(child);
		}
		if let Some(node) = self.nodes.get_mut(node_id) {
			*node = TestNode::Removed;
		}
	}

	fn refresh_selected_options(&mut self, select_id: NodeId) {
		let Some(select) = self.element(select_id) else {
			return;
		};
		if !select.tag.eq_ignore_ascii_case("select") {
			return;
		}
		let requested_values = select.selected_values.clone();
		let multiple = select.attr("multiple").is_some_and(is_boolean_attr_truthy);
		let children = select.children.clone();
		let mut selected_values = Vec::new();
		for child in children {
			self.refresh_selected_options_in_subtree(
				child,
				&requested_values,
				multiple,
				&mut selected_values,
			);
		}
		if let Some(TestNode::Element(select)) = self.nodes.get_mut(select_id) {
			select.value = Some(selected_values.first().cloned().unwrap_or_default());
			select.selected_values = selected_values;
		}
	}

	fn refresh_selected_options_in_subtree(
		&mut self,
		node_id: NodeId,
		requested_values: &[String],
		multiple: bool,
		selected_values: &mut Vec<String>,
	) {
		let children = self.children(node_id).to_vec();
		let effective_value = self.element(node_id).and_then(|node| {
			(node.tag.eq_ignore_ascii_case("option"))
				.then(|| node.option_value.clone().unwrap_or_default())
		});
		if let Some(TestNode::Element(node)) = self.nodes.get_mut(node_id)
			&& node.tag.eq_ignore_ascii_case("option")
		{
			let selected = effective_value.as_ref().is_some_and(|value| {
				requested_values.iter().any(|candidate| candidate == value)
					&& (multiple || selected_values.is_empty())
			});
			node.attrs
				.retain(|(name, _)| !name.eq_ignore_ascii_case("selected"));
			if selected {
				node.attrs
					.push(("selected".to_owned(), "selected".to_owned()));
				selected_values.push(effective_value.expect("option value should exist"));
			}
		}
		for child in children {
			self.refresh_selected_options_in_subtree(
				child,
				requested_values,
				multiple,
				selected_values,
			);
		}
	}
}

impl ElementNode {
	fn validate_control_binding(
		&self,
		binding: &ControlBinding,
	) -> Result<(), ControlBindingError> {
		let supported = match binding.kind() {
			ControlKind::Text => {
				self.tag.eq_ignore_ascii_case("textarea")
					|| (self.tag.eq_ignore_ascii_case("input")
						&& crate::control_binding::is_effective_text_input_type(self.attr("type")))
			}
			ControlKind::Number => {
				self.tag.eq_ignore_ascii_case("input")
					&& self
						.attr("type")
						.is_some_and(crate::control_binding::is_number_input_type)
			}
			ControlKind::Checkbox => {
				self.tag.eq_ignore_ascii_case("input")
					&& self
						.attr("type")
						.is_some_and(|kind| kind.eq_ignore_ascii_case("checkbox"))
			}
			ControlKind::Radio => {
				self.tag.eq_ignore_ascii_case("input")
					&& self
						.attr("type")
						.is_some_and(|kind| kind.eq_ignore_ascii_case("radio"))
			}
			ControlKind::SelectOne => {
				self.tag.eq_ignore_ascii_case("select")
					&& !self.attr("multiple").is_some_and(is_boolean_attr_truthy)
			}
			ControlKind::SelectMany => {
				self.tag.eq_ignore_ascii_case("select")
					&& self.attr("multiple").is_some_and(is_boolean_attr_truthy)
			}
		};
		if supported {
			Ok(())
		} else {
			Err(ControlBindingError::UnsupportedElement {
				control: binding.kind(),
				actual_tag: self.tag.clone(),
			})
		}
	}

	fn record_write_outcome(
		&mut self,
		binding: &ControlBinding,
		outcome: ControlWriteOutcome,
		raw: Option<String>,
	) {
		match outcome {
			ControlWriteOutcome::Committed | ControlWriteOutcome::Ignored => {
				self.pending_raw = None;
				self.last_observed_control_value = Some(binding.read());
			}
			ControlWriteOutcome::Rejected(_) => {
				self.pending_raw = raw;
			}
		}
	}

	fn apply_control_value(&mut self, binding: &ControlBinding, value: ControlValue) {
		self.project_controlled_attributes(binding, &value);
		match value {
			ControlValue::Text(value) => {
				self.value = Some(value.clone());
				if self.tag.eq_ignore_ascii_case("select") {
					self.selected_values = vec![value];
				}
			}
			ControlValue::Checked(checked) => {
				self.checked = checked;
				if binding.kind() == ControlKind::Radio {
					self.value = binding.radio_value().map(str::to_owned);
				}
			}
			ControlValue::SelectedValues(values) => {
				self.value = Some(values.first().cloned().unwrap_or_default());
				self.selected_values = values;
			}
		}
	}

	fn project_controlled_attributes(&mut self, binding: &ControlBinding, value: &ControlValue) {
		let omits_password_value = binding.kind() == ControlKind::Text
			&& self.tag.eq_ignore_ascii_case("input")
			&& self
				.attr("type")
				.is_some_and(|input_type| input_type.eq_ignore_ascii_case("password"));
		let projects_value = self.tag.eq_ignore_ascii_case("input")
			&& matches!(
				binding.kind(),
				ControlKind::Text | ControlKind::Number | ControlKind::Radio
			);
		if projects_value {
			self.attrs
				.retain(|(name, _)| !name.eq_ignore_ascii_case("value"));
			let projected_value = match binding.kind() {
				ControlKind::Text | ControlKind::Number => match value {
					ControlValue::Text(value) => Some(value.as_str()),
					_ => None,
				},
				ControlKind::Radio => binding.radio_value(),
				_ => None,
			};
			if let Some(value) = projected_value.filter(|_| !omits_password_value) {
				self.attrs.push(("value".to_owned(), value.to_owned()));
			}
		}

		if matches!(binding.kind(), ControlKind::Checkbox | ControlKind::Radio) {
			self.attrs
				.retain(|(name, _)| !name.eq_ignore_ascii_case("checked"));
			if matches!(value, ControlValue::Checked(true)) {
				self.attrs
					.push(("checked".to_owned(), "checked".to_owned()));
			}
		}
	}

	fn refresh_reactive_attributes(&mut self) {
		for (index, attribute) in self.reactive_attrs.iter().enumerate() {
			if self.reactive_attrs[index + 1..]
				.iter()
				.any(|later| later.name().eq_ignore_ascii_case(attribute.name()))
			{
				continue;
			}
			let value = attribute.value();
			if self.control_binding.as_ref().is_some_and(|binding| {
				!crate::control_binding::controlled_attribute_update_is_supported(
					&self.tag,
					binding.kind(),
					attribute.name(),
					value.as_deref(),
				)
			}) {
				continue;
			}
			self.attrs
				.retain(|(name, _)| !name.eq_ignore_ascii_case(attribute.name()));
			if let Some(value) = value
				&& (!is_boolean_attr(attribute.name()) || is_boolean_attr_truthy(&value))
			{
				self.attrs
					.push((attribute.name().to_owned(), value.into_owned()));
			}
		}
	}

	pub(crate) fn attr(&self, name: &str) -> Option<&str> {
		self.attrs
			.iter()
			.find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
			.map(|(_, value)| value.as_str())
	}

	pub(crate) fn attrs(&self) -> &[(String, String)] {
		&self.attrs
	}

	pub(crate) fn has_attr(&self, name: &str) -> bool {
		self.attr(name).is_some()
	}

	pub(crate) fn supports_value(&self) -> bool {
		self.supports_value_with_content_editable(self.content_editable)
	}

	fn supports_value_with_content_editable(&self, content_editable: bool) -> bool {
		content_editable
			|| (["input", "textarea", "select"]
				.iter()
				.any(|tag| self.tag.eq_ignore_ascii_case(tag))
				&& !(self.tag.eq_ignore_ascii_case("input")
					&& self
						.attr("type")
						.is_some_and(|kind| kind.eq_ignore_ascii_case("hidden"))))
	}

	pub(crate) fn is_disabled_form_control(&self) -> bool {
		self.has_attr("disabled")
			&& [
				"button", "fieldset", "input", "optgroup", "option", "select", "textarea",
			]
			.iter()
			.any(|tag| self.tag.eq_ignore_ascii_case(tag))
	}
}

fn normalize_native_control_value(
	element: &ElementNode,
	binding: &ControlBinding,
	value: ControlValue,
) -> ControlValue {
	let ControlValue::Text(raw) = value else {
		return value;
	};
	if !element.tag.eq_ignore_ascii_case("input") {
		return ControlValue::Text(raw);
	}
	let input_type = element.attr("type").unwrap_or("text");
	if binding.kind() == ControlKind::Text && input_type.eq_ignore_ascii_case("color") {
		return ControlValue::Text(normalize_native_color_value(&raw));
	}
	if binding.kind() == ControlKind::Text
		&& crate::control_binding::is_effective_text_input_type(Some(input_type))
	{
		let mut normalized = if input_type_removes_line_breaks(input_type) {
			raw.replace(['\r', '\n'], "")
		} else {
			raw
		};
		if input_type.eq_ignore_ascii_case("email")
			&& element.attr("multiple").is_some_and(is_boolean_attr_truthy)
		{
			normalized = normalized
				.split(',')
				.map(|value| value.trim_matches(|character: char| character.is_ascii_whitespace()))
				.collect::<Vec<_>>()
				.join(",");
		} else if ["url", "email"]
			.iter()
			.any(|known| input_type.eq_ignore_ascii_case(known))
		{
			normalized = normalized
				.trim_matches(|character: char| character.is_ascii_whitespace())
				.to_owned();
		}
		if input_type.eq_ignore_ascii_case("datetime-local")
			&& let Some((date, time)) = normalized.split_once(' ')
		{
			normalized = format!("{date}T{time}");
		}
		if is_temporal_input_type(input_type) {
			if !is_valid_temporal_input_value(input_type, &normalized) {
				normalized.clear();
			} else if input_type.eq_ignore_ascii_case("datetime-local") {
				normalized = normalize_datetime_local_value(&normalized);
			}
		}
		return ControlValue::Text(normalized);
	}
	if binding.kind() != ControlKind::Number || !input_type.eq_ignore_ascii_case("range") {
		return ControlValue::Text(raw);
	}
	let Some((min, max, step, step_base)) = native_range_constraints(element) else {
		return ControlValue::Text(raw);
	};
	let number = raw
		.parse::<f64>()
		.ok()
		.filter(|number| number.is_finite())
		.unwrap_or_else(|| if max < min { min } else { min.midpoint(max) });
	let normalized = if max < min {
		min
	} else {
		let clamped = number.clamp(min, max);
		match step {
			None => clamped,
			Some(step) => {
				let quotient = (clamped - step_base) / step;
				let lower = step_base + quotient.floor() * step;
				let upper = step_base + quotient.ceil() * step;
				let epsilon = step * 1e-12 + f64::EPSILON;
				let lower = (lower >= min - epsilon && lower <= max + epsilon).then_some(lower);
				let upper = (upper >= min - epsilon && upper <= max + epsilon).then_some(upper);
				match (lower, upper) {
					(Some(lower), Some(upper)) => {
						let lower = lower.clamp(min, max);
						let upper = upper.clamp(min, max);
						if clamped - lower < upper - clamped {
							lower
						} else {
							upper
						}
					}
					(Some(value), None) | (None, Some(value)) => value.clamp(min, max),
					(None, None) => clamped,
				}
			}
		}
	};
	let normalized = normalized.to_string();
	if normalized == raw {
		ControlValue::Text(raw)
	} else {
		ControlValue::Text(normalized)
	}
}

fn input_type_removes_line_breaks(input_type: &str) -> bool {
	["text", "search", "tel", "url", "email", "password"]
		.iter()
		.any(|known| input_type.eq_ignore_ascii_case(known))
}

fn normalize_datetime_local_value(value: &str) -> String {
	let Some((date, time)) = value.split_once('T').or_else(|| value.split_once(' ')) else {
		return value.to_owned();
	};
	let Some((hour, minute_and_seconds)) = time.split_once(':') else {
		return format!("{date}T{time}");
	};
	let Some((minute, seconds)) = minute_and_seconds.split_once(':') else {
		return format!("{date}T{hour}:{minute_and_seconds}");
	};
	let (second, fraction) = seconds.split_once('.').unwrap_or((seconds, ""));
	let fraction = fraction.trim_end_matches('0');
	if second == "00" && fraction.is_empty() {
		format!("{date}T{hour}:{minute}")
	} else if fraction.is_empty() {
		format!("{date}T{hour}:{minute}:{second}")
	} else {
		format!("{date}T{hour}:{minute}:{second}.{fraction}")
	}
}

fn is_temporal_input_type(input_type: &str) -> bool {
	["date", "datetime-local", "month", "time", "week"]
		.iter()
		.any(|known| input_type.eq_ignore_ascii_case(known))
}

fn is_valid_temporal_input_value(input_type: &str, value: &str) -> bool {
	if input_type.eq_ignore_ascii_case("date") {
		is_valid_html_date(value)
	} else if input_type.eq_ignore_ascii_case("datetime-local") {
		value
			.split_once('T')
			.or_else(|| value.split_once(' '))
			.is_some_and(|(date, time)| is_valid_html_date(date) && is_valid_html_time(time))
	} else if input_type.eq_ignore_ascii_case("month") {
		is_valid_html_month(value)
	} else if input_type.eq_ignore_ascii_case("time") {
		is_valid_html_time(value)
	} else if input_type.eq_ignore_ascii_case("week") {
		is_valid_html_week(value)
	} else {
		true
	}
}

fn is_valid_html_date(value: &str) -> bool {
	let Some((year, month_day)) = value.split_once('-') else {
		return false;
	};
	let Some((month, day)) = month_day.split_once('-') else {
		return false;
	};
	let Some(year) = parse_html_year(year) else {
		return false;
	};
	let Some(month) = parse_fixed_decimal(month, 2) else {
		return false;
	};
	let Some(day) = parse_fixed_decimal(day, 2) else {
		return false;
	};
	(1..=12).contains(&month) && day > 0 && day <= days_in_month(year, month)
}

fn is_valid_html_month(value: &str) -> bool {
	let Some((year, month)) = value.split_once('-') else {
		return false;
	};
	parse_html_year(year).is_some_and(|_| {
		parse_fixed_decimal(month, 2).is_some_and(|month| (1..=12).contains(&month))
	})
}

fn is_valid_html_week(value: &str) -> bool {
	let Some((year, week)) = value.split_once("-W") else {
		return false;
	};
	let Some(year) = parse_html_year(year) else {
		return false;
	};
	let Some(week) = parse_fixed_decimal(week, 2) else {
		return false;
	};
	(1..=53).contains(&week) && (week < 53 || has_iso_week_53(year))
}

fn is_valid_html_time(value: &str) -> bool {
	let Some((hour, minute_and_seconds)) = value.split_once(':') else {
		return false;
	};
	let Some(hour) = parse_fixed_decimal(hour, 2) else {
		return false;
	};
	let (minute, seconds) = match minute_and_seconds.split_once(':') {
		Some((minute, seconds)) => (minute, Some(seconds)),
		None => (minute_and_seconds, None),
	};
	let Some(minute) = parse_fixed_decimal(minute, 2) else {
		return false;
	};
	if hour > 23 || minute > 59 {
		return false;
	}
	let Some(seconds) = seconds else {
		return true;
	};
	let (second, fraction) = match seconds.split_once('.') {
		Some((second, fraction)) => (second, Some(fraction)),
		None => (seconds, None),
	};
	let Some(second) = parse_fixed_decimal(second, 2) else {
		return false;
	};
	second <= 59
		&& fraction.is_none_or(|fraction| {
			(1..=3).contains(&fraction.len()) && fraction.bytes().all(|byte| byte.is_ascii_digit())
		})
}

fn parse_html_year(value: &str) -> Option<u32> {
	(value.len() >= 4 && value.bytes().all(|byte| byte.is_ascii_digit()))
		.then(|| value.parse::<u32>().ok())
		.flatten()
		.filter(|year| *year > 0)
}

fn parse_fixed_decimal(value: &str, width: usize) -> Option<u32> {
	(value.len() == width && value.bytes().all(|byte| byte.is_ascii_digit()))
		.then(|| value.parse::<u32>().ok())
		.flatten()
}

fn days_in_month(year: u32, month: u32) -> u32 {
	match month {
		2 if is_leap_year(year) => 29,
		2 => 28,
		4 | 6 | 9 | 11 => 30,
		_ => 31,
	}
}

fn is_leap_year(year: u32) -> bool {
	(year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

fn has_iso_week_53(year: u32) -> bool {
	let weekday = weekday_of_january_first(year);
	weekday == 4 || (weekday == 3 && is_leap_year(year))
}

fn weekday_of_january_first(year: u32) -> u32 {
	let year = i64::from(year) - 1;
	let weekday_sunday_zero = (year + year / 4 - year / 100 + year / 400 + 1) % 7;
	if weekday_sunday_zero == 0 {
		7
	} else {
		weekday_sunday_zero as u32
	}
}

fn native_range_constraints(element: &ElementNode) -> Option<(f64, f64, Option<f64>, f64)> {
	if !element.tag.eq_ignore_ascii_case("input")
		|| !element
			.attr("type")
			.is_some_and(|input_type| input_type.eq_ignore_ascii_case("range"))
	{
		return None;
	}
	let min_attribute = element
		.attr("min")
		.and_then(crate::control_binding::parse_html_number);
	let min = min_attribute.unwrap_or(0.0);
	let max = element
		.attr("max")
		.and_then(crate::control_binding::parse_html_number)
		.unwrap_or(100.0);
	let step_base = min_attribute
		.or_else(|| {
			element
				.attr("value")
				.and_then(crate::control_binding::parse_html_number)
		})
		.unwrap_or(0.0);
	let step = match element.attr("step") {
		Some(value) if value.eq_ignore_ascii_case("any") => None,
		Some(value) => crate::control_binding::parse_html_number(value)
			.filter(|value| value.is_finite() && *value > 0.0)
			.or(Some(1.0)),
		None => Some(1.0),
	};
	Some((min, max, step, step_base))
}

fn has_conflicting_native_range_binding(
	nodes: &[TestNode],
	current_node: Option<NodeId>,
	element: &ElementNode,
	binding: &ControlBinding,
) -> bool {
	if binding.kind() != ControlKind::Number {
		return false;
	}
	let Some((min, max, step, step_base)) = native_range_constraints(element) else {
		return false;
	};
	let constraints = (min, max.max(min), step, step_base);
	nodes.iter().enumerate().any(|(node_id, candidate)| {
		if current_node == Some(node_id) {
			return false;
		}
		let TestNode::Element(candidate) = candidate else {
			return false;
		};
		candidate
			.control_binding
			.as_ref()
			.is_some_and(|candidate_binding| {
				candidate_binding.kind() == ControlKind::Number
					&& candidate_binding.target() == binding.target()
					&& native_range_constraints(candidate).is_some_and(
						|(min, max, step, step_base)| {
							crate::control_binding::range_constraints_conflict(
								constraints,
								(min, max.max(min), step, step_base),
							)
						},
					)
			})
	})
}

fn normalize_native_color_value(raw: &str) -> String {
	let bytes = raw.as_bytes();
	if bytes.len() == 7
		&& bytes[0] == b'#'
		&& bytes[1..].iter().all(|byte| byte.is_ascii_hexdigit())
	{
		raw.to_ascii_lowercase()
	} else {
		"#000000".to_owned()
	}
}

fn apply_native_control_normalization(
	element: &ElementNode,
	binding: &ControlBinding,
	original: &ControlValue,
	normalized: ControlValue,
	normalization_write_back_allowed: bool,
) -> ControlValue {
	if &normalized == original
		|| !normalization_write_back_allowed
		|| !should_commit_native_normalization(element, binding, original, &normalized)
	{
		return normalized;
	}
	match binding.write(normalized.clone()) {
		Ok(ControlWriteOutcome::Committed | ControlWriteOutcome::Ignored) => normalized,
		Ok(ControlWriteOutcome::Rejected(_)) | Err(_) => original.clone(),
	}
}

fn should_commit_native_normalization(
	element: &ElementNode,
	binding: &ControlBinding,
	original: &ControlValue,
	normalized: &ControlValue,
) -> bool {
	if binding.kind() != ControlKind::Text {
		return true;
	}
	let (ControlValue::Text(original), ControlValue::Text(normalized)) = (original, normalized)
	else {
		return true;
	};
	if original.is_empty() || !normalized.is_empty() || !element.tag.eq_ignore_ascii_case("input") {
		return true;
	}
	let input_type = element.attr("type").unwrap_or("text");
	["date", "datetime-local", "month", "time", "week"]
		.iter()
		.all(|known| !input_type.eq_ignore_ascii_case(known))
}

#[cfg(test)]
mod case_normalization_tests {
	use super::*;
	use crate::reactive::{ReactiveScope, Signal};
	use reinhardt_core::page::IntoPage;
	use reinhardt_core::types::page::PageElement;
	use rstest::rstest;

	fn element(tag: &str, input_type: Option<&str>) -> ElementNode {
		ElementNode {
			tag: tag.to_owned(),
			attrs: input_type
				.map(|value| vec![("type".to_owned(), value.to_owned())])
				.unwrap_or_default(),
			reactive_attrs: Vec::new(),
			children: Vec::new(),
			parent: None,
			is_void: false,
			event_handlers: Vec::new(),
			value: None,
			checked: false,
			selected_values: Vec::new(),
			files: Vec::new(),
			content_editable: false,
			control_binding: None,
			option_value: None,
			is_composing: false,
			pending_raw: None,
			last_committed_raw: None,
			last_observed_control_value: None,
			last_observed_signal_revision: None,
		}
	}

	#[test]
	fn native_control_binding_validation_normalizes_ascii_case() {
		ReactiveScope::run(|| {
			assert!(
				element("INPUT", Some("RADIO"))
					.validate_control_binding(&ControlBinding::radio(
						Signal::new(String::new()),
						"choice".to_owned(),
					))
					.is_ok()
			);
			assert!(
				element("SELECT", None)
					.validate_control_binding(&ControlBinding::select_one(Signal::new(
						String::new()
					)))
					.is_ok()
			);
		});
	}

	#[test]
	fn native_value_support_normalizes_ascii_case() {
		assert!(element("TEXTAREA", None).supports_value());
		assert!(!element("INPUT", Some("HIDDEN")).supports_value());
	}

	#[test]
	fn native_controlled_attributes_replace_stale_input_projection() {
		ReactiveScope::run(|| {
			let text = Signal::new("current".to_owned());
			let dom = TestDom::render(
				PageElement::new("INPUT")
					.attr("VALUE", "stale")
					.control_binding(ControlBinding::text(text))
					.into_page(),
			);
			let node = dom.children(dom.root())[0];

			assert_eq!(
				dom.element(node).unwrap().attrs(),
				&[("value".to_owned(), "current".to_owned())]
			);
		});
	}

	#[rstest]
	fn native_password_control_omits_bound_value_after_reactive_type_change() {
		ReactiveScope::run(|| {
			let password_type = Signal::new(false);
			let reactive_password_type = password_type;
			let value = Signal::new("secret".to_owned());
			let mut dom = TestDom::render(
				PageElement::new("input")
					.attr("type", "text")
					.reactive_attr("type", move || {
						Some(if reactive_password_type.get() {
							"password".into()
						} else {
							"text".into()
						})
					})
					.control_binding(ControlBinding::text(value))
					.into_page(),
			);
			let node = dom.children(dom.root())[0];

			assert_eq!(dom.element(node).unwrap().attr("value"), Some("secret"));
			password_type.set(true);
			dom.refresh_control_bindings();

			assert_eq!(dom.element(node).unwrap().attr("type"), Some("password"));
			assert_eq!(dom.element(node).unwrap().attr("value"), None);
			assert_eq!(value.get(), "secret");
		});
	}

	#[test]
	fn native_controlled_boolean_attributes_follow_signal_state() {
		ReactiveScope::run(|| {
			let checked = Signal::new(false);
			let mut dom = TestDom::render(
				PageElement::new("input")
					.attr("CHECKED", "checked")
					.attr("type", "checkbox")
					.control_binding(ControlBinding::checkbox(checked.clone()))
					.into_page(),
			);
			let node = dom.children(dom.root())[0];

			assert_eq!(dom.element(node).unwrap().attr("checked"), None);
			checked.set(true);
			dom.refresh_control_bindings();
			assert_eq!(dom.element(node).unwrap().attr("checked"), Some("checked"));
			checked.set(false);
			dom.refresh_control_bindings();
			assert_eq!(dom.element(node).unwrap().attr("checked"), None);
		});
	}

	#[rstest]
	fn native_text_controls_apply_browser_value_sanitization() {
		ReactiveScope::run(|| {
			let binding = ControlBinding::text(Signal::new(String::new()));
			let url = element("input", Some("url"));
			assert_eq!(
				normalize_native_control_value(
					&url,
					&binding,
					ControlValue::Text("  https://example.test\n".to_owned()),
				),
				ControlValue::Text("https://example.test".to_owned())
			);

			let textarea = element("textarea", None);
			assert_eq!(
				normalize_native_control_value(
					&textarea,
					&binding,
					ControlValue::Text("line\nbreak".to_owned()),
				),
				ControlValue::Text("line\nbreak".to_owned())
			);

			let mut multiple_email = element("input", Some("email"));
			multiple_email
				.attrs
				.push(("multiple".to_owned(), "multiple".to_owned()));
			assert_eq!(
				normalize_native_control_value(
					&multiple_email,
					&binding,
					ControlValue::Text(
						" first@example.test,\u{a0}second@example.test \n".to_owned()
					),
				),
				ControlValue::Text("first@example.test,\u{a0}second@example.test".to_owned())
			);
		});
	}

	#[test]
	fn native_range_values_align_to_the_declared_step() {
		ReactiveScope::run(|| {
			let mut range = element("input", Some("range"));
			range.attrs.extend([
				("min".to_owned(), "0".to_owned()),
				("max".to_owned(), "10".to_owned()),
				("step".to_owned(), "2".to_owned()),
			]);
			let binding = ControlBinding::number(Signal::new(3_i32));

			assert_eq!(
				normalize_native_control_value(
					&range,
					&binding,
					ControlValue::Text("3".to_owned()),
				),
				ControlValue::Text("4".to_owned())
			);
		});
	}

	#[test]
	fn native_range_values_use_the_value_attribute_as_step_base() {
		ReactiveScope::run(|| {
			let mut range = element("input", Some("range"));
			range.attrs.extend([
				("max".to_owned(), "10".to_owned()),
				("step".to_owned(), "2".to_owned()),
				("value".to_owned(), "1".to_owned()),
			]);
			let binding = ControlBinding::number(Signal::new(3_i32));

			assert_eq!(
				normalize_native_control_value(
					&range,
					&binding,
					ControlValue::Text("3".to_owned()),
				),
				ControlValue::Text("3".to_owned())
			);
		});
	}

	#[test]
	fn native_range_rejection_keeps_the_signal_and_control_aligned() {
		ReactiveScope::run(|| {
			let value = Signal::new(100_u8);
			let dom = TestDom::render(
				PageElement::new("input")
					.attr("type", "range")
					.attr("min", "300")
					.attr("max", "400")
					.control_binding(ControlBinding::number(value.clone()))
					.into_page(),
			);
			let node = dom.children(dom.root())[0];

			assert_eq!(value.get(), 100);
			assert_eq!(dom.value(node).as_deref(), Some("100"));
		});
	}

	#[rstest]
	fn native_range_uses_the_browser_default_for_non_finite_values() {
		ReactiveScope::run(|| {
			let value = Signal::new(f64::NAN);
			let dom = TestDom::render(
				PageElement::new("input")
					.attr("type", "range")
					.control_binding(ControlBinding::number(value))
					.into_page(),
			);
			let node = dom.children(dom.root())[0];

			assert_eq!(value.get(), 50.0);
			assert_eq!(dom.value(node).as_deref(), Some("50"));
		});
	}

	#[rstest]
	fn native_conflicting_ranges_keep_browser_values_local() {
		ReactiveScope::run(|| {
			let value = Signal::new(150_i32);
			let mut dom = TestDom::render(
				PageElement::new("div")
					.child(
						PageElement::new("input")
							.attr("type", "range")
							.attr("max", "100")
							.control_binding(ControlBinding::number(value)),
					)
					.child(
						PageElement::new("input")
							.attr("type", "range")
							.attr("min", "200")
							.attr("max", "400")
							.control_binding(ControlBinding::number(value)),
					)
					.into_page(),
			);
			let container = dom.children(dom.root())[0];
			let ranges = dom.children(container).to_vec();

			assert_eq!(
				(value.get(), dom.value(ranges[0]), dom.value(ranges[1])),
				(100, Some("100".to_owned()), Some("200".to_owned()))
			);

			value.set(150);
			dom.refresh_control_bindings();

			assert_eq!(
				(value.get(), dom.value(ranges[0]), dom.value(ranges[1])),
				(150, Some("100".to_owned()), Some("200".to_owned()))
			);
		});
	}

	#[rstest]
	fn native_overlapping_ranges_without_a_common_step_keep_browser_values_local() {
		ReactiveScope::run(|| {
			// Arrange
			let value = Signal::new(3_i32);

			// Act
			let dom = TestDom::render(
				PageElement::new("div")
					.child(
						PageElement::new("input")
							.attr("type", "range")
							.attr("min", "0")
							.attr("max", "5")
							.attr("step", "4")
							.control_binding(ControlBinding::number(value)),
					)
					.child(
						PageElement::new("input")
							.attr("type", "range")
							.attr("min", "2")
							.attr("max", "5")
							.attr("step", "6")
							.control_binding(ControlBinding::number(value)),
					)
					.into_page(),
			);
			let container = dom.children(dom.root())[0];
			let ranges = dom.children(container);

			// Assert
			assert_eq!(
				(value.get(), dom.value(ranges[0]), dom.value(ranges[1])),
				(4, Some("4".to_owned()), Some("2".to_owned()))
			);
		});
	}

	#[rstest]
	fn native_continuous_and_stepped_ranges_without_a_shared_value_keep_values_local() {
		ReactiveScope::run(|| {
			// Arrange
			let value = Signal::new(0.5_f64);

			// Act
			let dom = TestDom::render(
				PageElement::new("div")
					.child(
						PageElement::new("input")
							.attr("type", "range")
							.attr("min", "0.5")
							.attr("max", "0.6")
							.attr("step", "any")
							.control_binding(ControlBinding::number(value)),
					)
					.child(
						PageElement::new("input")
							.attr("type", "range")
							.attr("min", "0")
							.attr("max", "0.6")
							.attr("step", "1")
							.control_binding(ControlBinding::number(value)),
					)
					.into_page(),
			);
			let container = dom.children(dom.root())[0];
			let ranges = dom.children(container);

			// Assert
			assert_eq!(
				(value.get(), dom.value(ranges[0]), dom.value(ranges[1])),
				(0.5, Some("0.5".to_owned()), Some("0".to_owned()))
			);
		});
	}

	#[rstest]
	fn native_range_normalization_precedes_later_reactive_attributes() {
		ReactiveScope::run(|| {
			let value = Signal::new(150_i32);
			let second_max = value;
			let mut dom = TestDom::render(
				PageElement::new("div")
					.child(
						PageElement::new("input")
							.attr("type", "range")
							.attr("max", "100")
							.control_binding(ControlBinding::number(value)),
					)
					.child(
						PageElement::new("input")
							.attr("type", "range")
							.reactive_attr("max", move || Some(second_max.get().to_string().into()))
							.control_binding(ControlBinding::number(value)),
					)
					.into_page(),
			);
			let container = dom.children(dom.root())[0];
			let ranges = dom.children(container).to_vec();

			value.set(150);
			dom.refresh_control_bindings();

			assert_eq!(value.get(), 100);
			assert_eq!(
				dom.element(ranges[1])
					.and_then(|element| element.attr("max")),
				Some("100")
			);
		});
	}

	#[test]
	fn native_color_values_use_the_browser_invalid_value_default() {
		ReactiveScope::run(|| {
			let color = element("input", Some("color"));
			let binding = ControlBinding::text(Signal::new(String::new()));

			assert_eq!(
				normalize_native_control_value(
					&color,
					&binding,
					ControlValue::Text("red".to_owned()),
				),
				ControlValue::Text("#000000".to_owned())
			);
			assert_eq!(
				normalize_native_control_value(
					&color,
					&binding,
					ControlValue::Text("#ABCDEF".to_owned()),
				),
				ControlValue::Text("#abcdef".to_owned())
			);
		});
	}

	#[test]
	fn native_controlled_select_removes_stale_selected_attribute_case_insensitively() {
		ReactiveScope::run(|| {
			let selected = Signal::new("current".to_owned());
			let dom = TestDom::render(
				PageElement::new("select")
					.control_binding(ControlBinding::select_one(selected))
					.child(
						PageElement::new("option")
							.attr("value", "stale")
							.attr("SELECTED", "selected"),
					)
					.child(PageElement::new("option").attr("value", "current"))
					.into_page(),
			);
			let options = dom
				.all_elements()
				.into_iter()
				.filter_map(|node| dom.element(node).filter(|element| element.tag == "option"))
				.collect::<Vec<_>>();

			assert_eq!(options[0].attr("selected"), None);
			assert_eq!(options[1].attr("selected"), Some("selected"));
		});
	}
}

#[cfg(feature = "msw")]
pub(crate) fn shared_screen_inner(
	dom: TestDom,
	reactive_scope: ReactiveScope,
	scheduler: Rc<SchedulerScope>,
	query_client: QueryClient,
	mocks: SharedServerFnMocks,
) -> Rc<RefCell<ScreenInner>> {
	Rc::new(RefCell::new(ScreenInner {
		dom,
		reactive_scope,
		scheduler,
		query_client,
		mocks,
	}))
}

#[cfg(not(feature = "msw"))]
pub(crate) fn shared_screen_inner(
	dom: TestDom,
	reactive_scope: ReactiveScope,
	scheduler: Rc<SchedulerScope>,
	query_client: QueryClient,
) -> Rc<RefCell<ScreenInner>> {
	Rc::new(RefCell::new(ScreenInner {
		dom,
		reactive_scope,
		scheduler,
		query_client,
	}))
}
