//! IntoPage trait and Page enum for component rendering.
//!
//! This module re-exports the core Page types from `reinhardt-types` and provides
//! WASM-specific extensions for DOM mounting.

// Re-export core types from reinhardt-types
pub use reinhardt_core::types::page::{
	Head, IntoPage, LinkTag, MetaTag, MountError, Outlet, Page, PageElement, PageEventHandler,
	Reactive, ReactiveIf, ScriptTag, StyleTag,
};

#[cfg(native)]
pub use reinhardt_core::types::page::NativeEvent;
// Re-export boolean attribute utilities (used in WASM mount)
// Note: EventType is re-exported from dom::event module
#[cfg(wasm)]
pub(super) use reinhardt_core::types::page::{
	is_boolean_attr, is_boolean_attr_truthy, is_safe_html_attribute, is_safe_html_element_name,
};

#[cfg(wasm)]
use crate::component::reactive_if::{
	ReactiveAttributeEffects, ReactiveIfNode, ReactiveNode, store_reactive_node,
	with_reactive_node_transaction,
};
#[cfg(all(wasm, feature = "hmr"))]
use crate::component::reactive_if::{
	clear_reactive_node_store, new_reactive_node_store, with_reactive_node_store,
};
#[cfg(wasm)]
use crate::dom::control_binding::ControlBindingController;
#[cfg(wasm)]
use crate::dom::{Element, EventHandle};
#[cfg(all(wasm, feature = "hmr"))]
use crate::hmr::{
	DynamicSlotId, TemplatePatch, TemplatePatchBatch,
	patch_transaction::PatchTransaction,
	template_instance::{
		DomRange, DynamicRange, MountedSlot, ReactiveOwnerHandle, TemplateInstance,
	},
	template_registry::TemplateRegistry,
};
#[cfg(all(wasm, feature = "hmr"))]
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
#[cfg(wasm)]
use wasm_bindgen::JsCast;

#[cfg(all(wasm, feature = "hmr"))]
struct TemplateMountContext {
	slots: BTreeMap<DynamicSlotId, MountedSlot>,
}

#[cfg(all(wasm, feature = "hmr"))]
thread_local! {
	static TEMPLATE_MOUNT_CONTEXTS: RefCell<Vec<Rc<RefCell<TemplateMountContext>>>> = const { RefCell::new(Vec::new()) };
}

#[cfg(all(wasm, feature = "hmr"))]
fn with_template_mount_context<T>(
	mount: impl FnOnce() -> Result<T, MountError>,
) -> Result<(T, BTreeMap<DynamicSlotId, MountedSlot>), MountError> {
	let context = Rc::new(RefCell::new(TemplateMountContext {
		slots: BTreeMap::new(),
	}));
	TEMPLATE_MOUNT_CONTEXTS.with(|contexts| contexts.borrow_mut().push(Rc::clone(&context)));
	let result = mount();
	TEMPLATE_MOUNT_CONTEXTS.with(|contexts| {
		let popped = contexts.borrow_mut().pop();
		debug_assert!(popped.is_some_and(|value| Rc::ptr_eq(&value, &context)));
	});
	let slots = std::mem::take(&mut context.borrow_mut().slots);
	result.map(|value| (value, slots))
}

#[cfg(all(wasm, feature = "hmr"))]
fn record_template_slot(slot_id: DynamicSlotId, slot: MountedSlot) {
	TEMPLATE_MOUNT_CONTEXTS.with(|contexts| {
		if let Some(context) = contexts.borrow().last() {
			context.borrow_mut().slots.insert(slot_id, slot);
		}
	});
}

#[cfg(all(wasm, feature = "hmr"))]
fn nodes_between(start: &web_sys::Comment, end: &web_sys::Comment) -> Vec<web_sys::Node> {
	let end_node: web_sys::Node = end.clone().unchecked_into();
	let mut current = start.next_sibling();
	let mut nodes = Vec::new();
	while let Some(node) = current {
		if node.is_same_node(Some(&end_node)) {
			break;
		}
		current = node.next_sibling();
		nodes.push(node);
	}
	nodes
}

/// Replays the latest validated static overlay after a new instance mounts.
///
/// A static edit is delivered to currently mounted instances immediately, but
/// navigation or a later list iteration can create another instance from the
/// old compiled page value. Reapplying the validated overlay through the same
/// transaction path keeps that future instance consistent without evaluating
/// new Rust code or altering its dynamic ABI.
#[cfg(all(wasm, feature = "hmr"))]
fn replay_template_overlay(
	registry: &TemplateRegistry,
	descriptor: &crate::hmr::TemplateDescriptor,
) {
	let Some(static_tree) = registry.overlay_for(&descriptor.key) else {
		return;
	};
	let (build_id, manifest_digest, generation) = registry.identity();
	let batch = TemplatePatchBatch {
		build_id,
		manifest_digest,
		generation,
		patches: vec![TemplatePatch::static_replacement(descriptor, static_tree)],
	};
	if let Err(error) = PatchTransaction::plan(&batch, registry).and_then(PatchTransaction::commit)
	{
		web_sys::console::error_1(
			&format!("failed to replay template hot-reload overlay: {error}").into(),
		);
	}
}

/// Extension trait for mounting Page to DOM (WASM only).
///
/// This trait provides the `mount()` method for `Page` which is only available
/// in WASM environments where actual DOM manipulation is possible.
#[cfg(wasm)]
pub trait PageExt {
	/// Mounts the view to a DOM element (client-side only).
	fn mount(self, parent: &Element) -> Result<(), MountError>;
}

#[cfg(wasm)]
impl PageExt for Page {
	fn mount(self, parent: &Element) -> Result<(), MountError> {
		let manager = crate::document_head::ensure_browser_document_head_manager()
			.map_err(|error| error.into_mount_error())?;
		manager.begin_batch();
		let result = crate::document_head::with_document_head_manager(&manager, || {
			mount_inner(self, parent)
		});
		let reconcile = manager
			.end_batch(result.is_ok())
			.map_err(|error| error.into_mount_error());
		result.and(reconcile)
	}
}

#[cfg(wasm)]
use reinhardt_core::types::page::ControlValue;
use reinhardt_core::types::page::{ControlBinding, ControlKind};

pub(crate) fn controlled_attribute_is_overridden(
	binding: Option<&ControlBinding>,
	name: &str,
) -> bool {
	binding.is_some_and(|binding| match binding.kind() {
		ControlKind::Text | ControlKind::Number => name.eq_ignore_ascii_case("value"),
		ControlKind::Checkbox => name.eq_ignore_ascii_case("checked"),
		ControlKind::Radio => {
			name.eq_ignore_ascii_case("checked") || name.eq_ignore_ascii_case("value")
		}
		ControlKind::SelectOne | ControlKind::SelectMany => false,
	})
}

#[cfg(wasm)]
pub(crate) fn static_attribute_is_effective<N: AsRef<str>, V: AsRef<str>>(
	attrs: &[(N, V)],
	index: usize,
) -> bool {
	let name = attrs[index].0.as_ref();
	if is_boolean_attr(name) {
		return attrs
			.iter()
			.enumerate()
			.find(|(_, (candidate, value))| {
				candidate.as_ref().eq_ignore_ascii_case(name)
					&& is_boolean_attr_truthy(value.as_ref())
			})
			.map_or_else(
				|| {
					!attrs[..index]
						.iter()
						.any(|(earlier, _)| earlier.as_ref().eq_ignore_ascii_case(name))
				},
				|(truthy_index, _)| truthy_index == index,
			);
	}
	!attrs[..index]
		.iter()
		.any(|(earlier, _)| earlier.as_ref().eq_ignore_ascii_case(name))
}

#[cfg(wasm)]
pub(crate) fn controlled_attribute_affects_value(
	element: &Element,
	binding: &ControlBinding,
	name: &str,
) -> bool {
	match binding.kind() {
		ControlKind::Text => {
			name.eq_ignore_ascii_case("type")
				|| (name.eq_ignore_ascii_case("multiple")
					&& element
						.as_web_sys()
						.dyn_ref::<web_sys::HtmlInputElement>()
						.is_some_and(|input| input.type_().eq_ignore_ascii_case("email")))
		}
		ControlKind::Number => ["type", "min", "max", "step"]
			.iter()
			.any(|attribute| name.eq_ignore_ascii_case(attribute)),
		ControlKind::SelectOne | ControlKind::SelectMany => name.eq_ignore_ascii_case("multiple"),
		ControlKind::Checkbox | ControlKind::Radio => false,
	}
}

#[cfg(wasm)]
pub(crate) fn initialize_control_default(element: &Element, binding: &ControlBinding) {
	let value = crate::reactive::untracked(|| binding.read());
	match (binding.kind(), value) {
		(ControlKind::Text | ControlKind::Number, ControlValue::Text(value)) => {
			if let Some(input) = element.as_web_sys().dyn_ref::<web_sys::HtmlInputElement>() {
				if input.type_().eq_ignore_ascii_case("password") {
					input.set_value(&value);
				} else {
					input.set_default_value(&value);
				}
			} else if let Some(textarea) = element
				.as_web_sys()
				.dyn_ref::<web_sys::HtmlTextAreaElement>()
			{
				let _ = textarea.set_default_value(&value);
			}
		}
		(ControlKind::Checkbox | ControlKind::Radio, ControlValue::Checked(checked)) => {
			if let Some(input) = element.as_web_sys().dyn_ref::<web_sys::HtmlInputElement>() {
				input.set_default_checked(checked);
			}
		}
		(ControlKind::SelectOne, ControlValue::Text(value)) => {
			if let Some(select) = element.as_web_sys().dyn_ref::<web_sys::HtmlSelectElement>() {
				let options = select.options();
				let mut selected = false;
				for index in 0..options.length() {
					if let Some(option) = options
						.item(index)
						.and_then(|option| option.dyn_into::<web_sys::HtmlOptionElement>().ok())
					{
						let matches = !selected && option.value() == value;
						option.set_default_selected(matches);
						selected |= matches;
					}
				}
			}
		}
		(ControlKind::SelectMany, ControlValue::SelectedValues(values)) => {
			if let Some(select) = element.as_web_sys().dyn_ref::<web_sys::HtmlSelectElement>() {
				let options = select.options();
				for index in 0..options.length() {
					if let Some(option) = options
						.item(index)
						.and_then(|option| option.dyn_into::<web_sys::HtmlOptionElement>().ok())
					{
						option.set_default_selected(
							values.iter().any(|value| value == &option.value()),
						);
					}
				}
			}
		}
		_ => {}
	}
}

#[cfg(wasm)]
fn mount_inner(page: Page, parent: &Element) -> Result<(), MountError> {
	use crate::dom::document;

	match page {
		Page::Element(el) => {
			let doc = document();
			let (tag, attrs, reactive_attrs, children, _is_void, event_handlers, control_binding) =
				el.into_parts_with_control_binding();
			if !is_safe_html_element_name(&tag) {
				for child in children {
					mount_inner(child, parent)?;
				}
				return Ok(());
			}

			let element = doc
				.create_element(&tag)
				.map_err(|_| MountError::CreateElementFailed)?;

			for (index, (name, value)) in attrs.iter().enumerate() {
				if !static_attribute_is_effective(&attrs, index) {
					continue;
				}
				if !is_safe_html_attribute(name, value) {
					continue;
				}
				// Skip boolean attributes with falsy values (empty, "false", "0")
				// This ensures `disabled: ""` doesn't set the attribute
				let name_str: &str = name.as_ref();
				let value_str: &str = value.as_ref();
				let is_boolean = is_boolean_attr(name_str);
				let is_falsy = !is_boolean_attr_truthy(value_str);

				if is_boolean && is_falsy {
					continue;
				}
				if controlled_attribute_is_overridden(control_binding.as_ref(), name.as_ref()) {
					continue;
				}

				element
					.set_attribute(name, value)
					.map_err(|err_str: String| {
						// Log detailed error to browser console
						let msg: wasm_bindgen::JsValue = format!(
							"[SetAttributeFailed] attribute='{}', value='{}'",
							name, value
						)
						.into();
						let label: wasm_bindgen::JsValue = "Error message:".into();
						let err_msg: wasm_bindgen::JsValue = err_str.into();
						web_sys::console::error_3(&msg, &label, &err_msg);
						MountError::SetAttributeFailed
					})?;
			}

			let mount_children_before_binding = tag.eq_ignore_ascii_case("select");
			let skip_bound_textarea_children =
				control_binding.is_some() && tag.eq_ignore_ascii_case("textarea");
			let mount_element = || {
				let mut children = children.into_iter();
				if mount_children_before_binding {
					for child in children.by_ref() {
						mount_inner(child, &element)?;
					}
				}

				let initializing_reactive_attributes = std::rc::Rc::new(std::cell::Cell::new(true));
				let reactive_attribute_effects = reactive_attrs
					.iter()
					.enumerate()
					.filter(|(index, attribute)| {
						!reactive_attrs[*index + 1..]
							.iter()
							.any(|later| later.name().eq_ignore_ascii_case(attribute.name()))
					})
					.filter(|(_, attribute)| {
						!controlled_attribute_is_overridden(
							control_binding.as_ref(),
							attribute.name(),
						)
					})
					.map(|(_, attribute)| {
						let attribute = attribute.clone();
						let element = element.clone();
						let binding = control_binding.clone();
						let initializing = std::rc::Rc::clone(&initializing_reactive_attributes);
						crate::reactive::Effect::new(move || {
							let value = attribute.value();
							if binding.as_ref().is_some_and(|binding| {
								!crate::control_binding::controlled_attribute_update_is_supported(
									&element.as_web_sys().tag_name(),
									binding.kind(),
									attribute.name(),
									value.as_deref(),
								)
							}) {
								return;
							}
							match value {
								Some(value)
									if !is_safe_html_attribute(attribute.name(), &value) =>
								{
									let _ = element.remove_attribute(attribute.name());
								}
								Some(value)
									if is_boolean_attr(attribute.name())
										&& !is_boolean_attr_truthy(&value) =>
								{
									let _ = element.remove_attribute(attribute.name());
								}
								Some(value) => {
									let _ = element.set_attribute(attribute.name(), &value);
								}
								None => {
									let _ = element.remove_attribute(attribute.name());
								}
							}
							if !initializing.get()
								&& let Some(binding) = binding.as_ref()
								&& controlled_attribute_affects_value(
									&element,
									binding,
									attribute.name(),
								) && let Err(error) =
								crate::dom::control_binding::reconcile_control_binding(
									&element, binding,
								) {
								web_sys::console::error_1(
									&format!("controlled input attribute update failed: {error}")
										.into(),
								);
							}
						})
					})
					.collect::<Vec<_>>();
				if let Some(binding) = control_binding.as_ref() {
					initialize_control_default(&element, binding);
				}
				if !reactive_attribute_effects.is_empty() {
					initializing_reactive_attributes.set(false);
					if let Some(binding) = control_binding.as_ref() {
						crate::dom::control_binding::reconcile_control_binding(&element, binding)?;
					}
				}
				let binding_controller = control_binding
					.clone()
					.map(|binding| ControlBindingController::mount(element.clone(), binding))
					.transpose()?;
				let mut event_handles: Vec<EventHandle> = Vec::new();

				for (event_type, handler) in event_handlers {
					let handler_clone = handler.clone();
					#[cfg(feature = "i18n")]
					let i18n_context = crate::i18n::current_i18n_callback_context();
					event_handles.push(element.add_event_listener_with_event(
						event_type.as_str(),
						move |event| {
							#[cfg(feature = "i18n")]
							{
								crate::i18n::with_optional_i18n_context(
									i18n_context.as_ref(),
									|| handler_clone(event),
								);
							}
							#[cfg(not(feature = "i18n"))]
							handler_clone(event);
						},
					));
				}

				if !skip_bound_textarea_children {
					for child in children {
						mount_inner(child, &element)?;
					}
				}

				parent
					.append_child(element.clone())
					.map_err(|_| MountError::AppendChildFailed)?;
				store_reactive_node((
					binding_controller,
					event_handles,
					ReactiveAttributeEffects::new(reactive_attribute_effects),
				));
				Ok::<(), MountError>(())
			};
			with_reactive_node_transaction(mount_element)?;
		}
		Page::Text(text) => {
			let window = web_sys::window().ok_or(MountError::NoWindow)?;
			let document = window.document().ok_or(MountError::NoDocument)?;
			let text_node = document.create_text_node(&text);
			parent
				.inner()
				.append_child(&text_node)
				.map_err(|_| MountError::AppendChildFailed)?;
		}
		Page::Fragment(children) => {
			for child in children {
				mount_inner(child, parent)?;
			}
		}
		Page::KeyedFragment(children) => {
			for (_, child) in children {
				mount_inner(child, parent)?;
			}
		}
		Page::Outlet(outlet) => {
			let id = outlet.id().map(str::to_string);
			if let Some(child) = outlet.into_child() {
				mount_inner(child, parent)?;
			} else if let Some(id) = id {
				let doc = document();
				let host = doc
					.create_element("reinhardt-outlet")
					.map_err(|_| MountError::CreateElementFailed)?;
				host.set_attribute("data-rh-outlet-id", &id)
					.map_err(|_| MountError::SetAttributeFailed)?;
				host.set_attribute("style", "display: contents;")
					.map_err(|_| MountError::SetAttributeFailed)?;
				parent
					.append_child(host)
					.map_err(|_| MountError::AppendChildFailed)?;
			}
		}
		Page::Empty => {}
		Page::WithHead { view, head } => {
			with_reactive_node_transaction(|| {
				let registration = crate::document_head::current_document_head_manager()
					.and_then(|manager| manager.register_static_page(head))
					.map_err(|error| error.into_mount_error())?;
				store_reactive_node(registration);
				mount_inner(*view, parent)
			})?;
		}
		#[cfg(feature = "hmr")]
		Page::DevTemplate { metadata, view } => {
			let descriptor = metadata
				.downcast_ref::<crate::hmr::TemplateDescriptor>()
				.cloned();
			let registry = crate::hmr::bridge::active_registry();
			let (Some(descriptor), Some(registry)) = (descriptor, registry) else {
				mount_inner(*view, parent)?;
				return Ok(());
			};

			let deferred_patch = registry.register_descriptor_tree(descriptor.clone());
			if matches!(
				deferred_patch,
				crate::hmr::template_registry::DeferredPatchOutcome::AbiMismatch
			) && let Some(window) = web_sys::window()
			{
				let _ = window.location().reload();
			}
			let document = web_sys::window()
				.ok_or(MountError::NoWindow)?
				.document()
				.ok_or(MountError::NoDocument)?;
			let start = document.create_comment("reinhardt-hmr-template-start");
			parent
				.inner()
				.append_child(&start)
				.map_err(|_| MountError::AppendChildFailed)?;
			let mounted = with_template_mount_context(|| mount_inner(*view, parent));
			let ((), slots) = match mounted {
				Ok(value) => value,
				Err(error) => {
					let _ = parent.inner().remove_child(&start);
					return Err(error);
				}
			};
			let end = document.create_comment("reinhardt-hmr-template-end");
			parent
				.inner()
				.append_child(&end)
				.map_err(|_| MountError::AppendChildFailed)?;
			let instance = TemplateInstance {
				root_range: DomRange {
					nodes: nodes_between(&start, &end),
					start,
					end,
				},
				slots,
				nested: Vec::new(),
			};
			let guard = registry.mount_instance(descriptor.key.clone(), instance);
			replay_template_overlay(&registry, &descriptor);
			store_reactive_node(guard);
		}
		#[cfg(feature = "hmr")]
		Page::DevSlot { slot_id, view } => {
			let captures_slot =
				TEMPLATE_MOUNT_CONTEXTS.with(|contexts| !contexts.borrow().is_empty());
			if !captures_slot {
				mount_inner(*view, parent)?;
				return Ok(());
			}

			let dynamic_slot_id = DynamicSlotId(slot_id);
			let document = web_sys::window()
				.ok_or(MountError::NoWindow)?
				.document()
				.ok_or(MountError::NoDocument)?;
			let start = document.create_comment(&format!("reinhardt-hmr-slot-{slot_id}-start"));
			parent
				.inner()
				.append_child(&start)
				.map_err(|_| MountError::AppendChildFailed)?;
			let store = new_reactive_node_store();
			let mounted = with_reactive_node_store(&store, || mount_inner(*view, parent));
			if let Err(error) = mounted {
				clear_reactive_node_store(&store);
				let _ = parent.inner().remove_child(&start);
				return Err(error);
			}
			let end = document.create_comment(&format!("reinhardt-hmr-slot-{slot_id}-end"));
			parent
				.inner()
				.append_child(&end)
				.map_err(|_| MountError::AppendChildFailed)?;
			record_template_slot(
				dynamic_slot_id,
				MountedSlot::DynamicRange(DynamicRange {
					range: DomRange {
						nodes: nodes_between(&start, &end),
						start,
						end,
					},
					owner: ReactiveOwnerHandle::from_store(store),
				}),
			);
		}
		Page::ReactiveIf(reactive_if) => {
			// Decompose the ReactiveIf to get the closures
			let (condition, then_view, else_view) = reactive_if.into_parts();

			// Create a ReactiveIfNode that manages DOM updates reactively.
			// The node uses an Effect to monitor condition changes and swaps
			// DOM nodes when the condition value changes.
			let node = ReactiveIfNode::new(parent, condition, then_view, else_view);
			// Store the node to keep it alive for the lifetime of the DOM element
			store_reactive_node(node);
		}
		Page::Reactive(reactive) => {
			// Get the render closure from the Reactive
			let render = reactive.into_render();

			// Create a ReactiveNode that manages DOM updates reactively.
			// The node uses an Effect to monitor dependency changes and
			// re-renders when they change.
			let node = ReactiveNode::new(parent, render);
			// Store the node to keep it alive for the lifetime of the DOM element
			store_reactive_node(node);
		}
		Page::Suspense(node) => {
			mount_inner(node.render_branch(), parent)?;
		}
		Page::Deferred(node) => {
			mount_inner(node.content(), parent)?;
		}
	}

	Ok(())
}

/// Extension trait for mounting Page (non-WASM stub).
///
/// In non-WASM environments, this trait provides a no-op stub for API compatibility.
#[cfg(native)]
pub trait PageExt {
	/// Mounts the view (non-WASM stub).
	///
	/// In non-WASM environments, this function is a no-op stub that always succeeds.
	/// The `parent` parameter is unused and exists only for API compatibility.
	fn mount<T>(self, _parent: &T) -> Result<(), MountError>;
}

#[cfg(native)]
impl PageExt for Page {
	fn mount<T>(self, _parent: &T) -> Result<(), MountError> {
		Ok(())
	}
}
