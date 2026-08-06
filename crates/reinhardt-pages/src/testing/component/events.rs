//! Stable element handles and synthetic event dispatch.

use std::cell::RefCell;
use std::rc::Rc;

use reinhardt_core::types::page::{
	NativeEvent, NativeEventPayload, NativeEventTarget, PageEventHandler,
};

use crate::reactive::query::with_query_client;

use super::error::EventError;
use super::fixture::EventFixture;
#[cfg(feature = "msw")]
use super::server_fn_mock;
use super::tree::{NodeId, ScreenInner};

/// Stable handle to an element in a rendered native test screen.
#[derive(Clone)]
pub struct ElementHandle {
	pub(crate) inner: Rc<RefCell<ScreenInner>>,
	pub(crate) node_id: NodeId,
}

impl ElementHandle {
	pub(crate) fn new(inner: Rc<RefCell<ScreenInner>>, node_id: NodeId) -> Self {
		Self { inner, node_id }
	}

	/// Dispatches a click event and panics when dispatch fails.
	pub fn click(&self) {
		self.try_click().expect("click dispatch failed");
	}

	/// Dispatches a click event.
	pub fn try_click(&self) -> Result<(), EventError> {
		self.dispatch(EventFixture::click())
	}

	/// Dispatches a submit event and panics when dispatch fails.
	pub fn submit(&self) {
		self.try_submit().expect("submit dispatch failed");
	}

	/// Dispatches a submit event.
	pub fn try_submit(&self) -> Result<(), EventError> {
		self.dispatch(EventFixture::submit())
	}

	/// Updates the element value and dispatches an input event.
	pub fn input(&self, value: impl Into<String>) {
		self.try_input(value).expect("input dispatch failed");
	}

	/// Updates the element value and dispatches an input event.
	pub fn try_input(&self, value: impl Into<String>) -> Result<(), EventError> {
		self.dispatch(EventFixture::input().value(value))
	}

	/// Updates the element value and dispatches a change event.
	pub fn change(&self, value: impl Into<String>) {
		self.try_change(value).expect("change dispatch failed");
	}

	/// Updates the element value and dispatches a change event.
	pub fn try_change(&self, value: impl Into<String>) -> Result<(), EventError> {
		self.dispatch(EventFixture::change().value(value))
	}

	/// Updates the element checked state and dispatches a change event.
	pub fn change_checked(&self, value: bool) {
		self.try_change_checked(value)
			.expect("checked change dispatch failed");
	}

	/// Updates the element checked state and dispatches a change event.
	pub fn try_change_checked(&self, value: bool) -> Result<(), EventError> {
		self.dispatch(EventFixture::change().checked(value))
	}

	/// Dispatches a key-down event with the supplied logical key.
	pub fn key_down(&self, key: impl Into<String>) {
		self.try_key_down(key).expect("key-down dispatch failed");
	}

	/// Dispatches a key-down event with the supplied logical key.
	pub fn try_key_down(&self, key: impl Into<String>) -> Result<(), EventError> {
		self.dispatch(EventFixture::key_down().key(key))
	}

	/// Returns the element text content.
	pub fn text(&self) -> String {
		self.try_text().expect("element text read failed")
	}

	/// Tries to return the element text content.
	pub fn try_text(&self) -> Result<String, EventError> {
		let borrowed = self.inner.borrow();
		if !borrowed.dom.contains(self.node_id) {
			return Err(EventError::DetachedElement);
		}
		Ok(borrowed.dom.text_content(self.node_id))
	}

	/// Returns the element tag name.
	pub fn tag_name(&self) -> String {
		self.try_tag_name().expect("element tag read failed")
	}

	/// Tries to return the element tag name.
	pub fn try_tag_name(&self) -> Result<String, EventError> {
		let borrowed = self.inner.borrow();
		if !borrowed.dom.contains(self.node_id) {
			return Err(EventError::DetachedElement);
		}
		borrowed
			.dom
			.element(self.node_id)
			.map(|node| node.tag.clone())
			.ok_or(EventError::UnsupportedElement)
	}

	/// Returns the current internal form value for value-bearing elements.
	pub fn value(&self) -> Option<String> {
		self.try_value().expect("element value read failed")
	}

	/// Tries to return the current internal form value for value-bearing elements.
	pub fn try_value(&self) -> Result<Option<String>, EventError> {
		let borrowed = self.inner.borrow();
		if !borrowed.dom.contains(self.node_id) {
			return Err(EventError::DetachedElement);
		}
		Ok(borrowed.dom.value(self.node_id))
	}

	/// Dispatches one validated synthetic event fixture.
	pub fn dispatch(&self, fixture: EventFixture) -> Result<(), EventError> {
		let query_client = self.inner.borrow().query_client.clone();
		with_query_client(&query_client, || self.dispatch_in_query_context(fixture))
	}

	fn dispatch_in_query_context(&self, fixture: EventFixture) -> Result<(), EventError> {
		let event = fixture.build()?;
		let input_is_composing = matches!(
			event.payload(),
			NativeEventPayload::Input(data) if data.is_composing
		);
		let (binding_handled, pending_binding_write, scheduler) = {
			let mut borrowed = self.inner.borrow_mut();
			if !borrowed.dom.contains(self.node_id) {
				return Err(EventError::DetachedElement);
			}
			if borrowed.dom.element(self.node_id).is_none() {
				return Err(EventError::UnsupportedElement);
			}
			if borrowed.dom.suppresses_events(self.node_id) {
				return Ok(());
			}
			borrowed.dom.validate_control_binding(self.node_id)?;
			borrowed
				.dom
				.apply_target_state(self.node_id, fixture.target())?;
			let (binding_handled, pending_binding_write) = borrowed
				.dom
				.prepare_control_binding_commit(self.node_id, fixture.name(), input_is_composing)?;
			(
				binding_handled,
				pending_binding_write,
				Rc::clone(&borrowed.scheduler),
			)
		};
		#[cfg(feature = "msw")]
		let mocks = self.inner.borrow().mocks.clone();

		if let Some(pending_binding_write) = pending_binding_write {
			#[cfg(feature = "msw")]
			let completed = server_fn_mock::with_active(mocks.clone(), || {
				scheduler.with_current(|| pending_binding_write.execute())
			})?;
			#[cfg(not(feature = "msw"))]
			let completed = scheduler.with_current(|| pending_binding_write.execute())?;
			self.inner
				.borrow_mut()
				.dom
				.record_control_binding_commit(completed);
			self.inner.borrow_mut().dom.refresh_control_bindings();
		}

		let (handlers, target) = {
			let borrowed = self.inner.borrow();
			if !borrowed.dom.contains(self.node_id) {
				return Err(EventError::DetachedElement);
			}
			let handlers: Vec<(NodeId, PageEventHandler, NativeEventTarget)> = borrowed
				.dom
				.event_handlers(self.node_id, fixture.name(), event.base().bubbles)
				.into_iter()
				.filter_map(|(node_id, handler)| {
					borrowed
						.dom
						.event_target(node_id)
						.map(|target| (node_id, handler, target))
				})
				.collect();
			let target = borrowed
				.dom
				.event_target(self.node_id)
				.ok_or(EventError::UnsupportedElement)?;
			(handlers, target)
		};

		if handlers.is_empty() && !binding_handled {
			return Err(EventError::MissingHandler);
		}
		let event = event.with_target(target);
		#[cfg(feature = "msw")]
		{
			server_fn_mock::with_active(mocks, || {
				scheduler.with_current(|| {
					dispatch_handlers(&event, handlers);
				});
			});
		}
		#[cfg(not(feature = "msw"))]
		scheduler.with_current(|| dispatch_handlers(&event, handlers));
		Ok(())
	}
}

fn dispatch_handlers(
	event: &NativeEvent,
	handlers: Vec<(NodeId, PageEventHandler, NativeEventTarget)>,
) {
	let mut previous_node = None;
	for (node_id, handler, current_target) in handlers {
		if event.propagation_stopped() && previous_node != Some(node_id) {
			break;
		}
		if event.immediate_propagation_stopped() && previous_node == Some(node_id) {
			continue;
		}

		handler(event.with_current_target(current_target));
		previous_node = Some(node_id);
	}
}

impl std::fmt::Debug for ElementHandle {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ElementHandle")
			.field("node_id", &self.node_id)
			.finish_non_exhaustive()
	}
}
