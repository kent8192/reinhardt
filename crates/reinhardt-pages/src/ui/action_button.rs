use std::rc::Rc;

use crate::callback::IntoEventHandler;
use crate::component::{Component, IntoPage, Page, PageElement};
use crate::dom::EventType;
use crate::reactive::Action;

/// A headless button that dispatches an [`Action`] with a payload when clicked.
///
/// The public constructors are available from [`reinhardt_pages::ui`]. Use
/// [`Self::new`] for a fixed cloneable payload, or [`Self::new_with`] when the
/// payload should be constructed at click time:
///
/// ```rust,ignore
/// use reinhardt_pages::component::Page;
/// use reinhardt_pages::ui::ActionButton;
///
/// let save_button = ActionButton::new(save, project_id.clone(), Page::text("Save"));
/// let current_project_id = project_id.clone();
/// let delete_button = ActionButton::new_with(
///     delete,
///     move || current_project_id.clone(),
///     Page::text("Delete"),
/// );
/// ```
///
/// Here `save` and `delete` are application-defined
/// [`Action`](crate::reactive::Action) handles and `project_id` is the
/// application payload. The child argument accepts any [`IntoPage`] value.
/// `new` requires `P: Clone`; `new_with` accepts `F: Fn() -> P + 'static`.
///
/// While the action is pending, the button owns its `disabled` and
/// `aria-busy="true"` attributes and ignores caller-provided values for those
/// attributes and `type`. The handler checks the action again immediately
/// before dispatch, which protects against duplicate events that were queued
/// before the DOM reflected the pending state.
///
/// The button renders as `<button type="button">` and derives its `disabled`
/// and `aria-busy` attributes from the action's current pending state. Caller
/// attributes with those managed names are ignored.
///
/// The click handler rechecks the pending state immediately before dispatching,
/// so stale events do not dispatch a second payload while the action is busy.
pub struct ActionButton<T, E, P>
where
	T: Clone + 'static,
	E: Clone + 'static,
	P: 'static,
{
	action: Action<T, E>,
	payload: Rc<dyn Fn() -> P>,
	children: Page,
	attrs: Vec<(String, String)>,
}

impl<T, E, P> ActionButton<T, E, P>
where
	T: Clone + 'static,
	E: Clone + 'static,
	P: 'static,
{
	/// Creates a button with a fixed, cloneable payload.
	///
	/// The payload is cloned for each dispatch. `C` is converted through
	/// [`IntoPage`], so text, an element, a fragment, or another component can
	/// be used as the button content.
	pub fn new<C>(action: Action<T, E>, payload: P, children: C) -> Self
	where
		P: Clone,
		C: IntoPage,
	{
		Self {
			action,
			payload: Rc::new(move || payload.clone()),
			children: children.into_page(),
			attrs: Vec::new(),
		}
	}

	/// Creates a button whose payload is built when the button is clicked.
	///
	/// The factory has the exact slot-like signature `F: Fn() -> P + 'static`.
	/// Use this form when the payload must read current application state at the
	/// time of the event rather than when the component is constructed.
	pub fn new_with<C, F>(action: Action<T, E>, payload: F, children: C) -> Self
	where
		C: IntoPage,
		F: Fn() -> P + 'static,
	{
		Self {
			action,
			payload: Rc::new(payload),
			children: children.into_page(),
			attrs: Vec::new(),
		}
	}

	/// Adds an attribute that is forwarded to the rendered button.
	///
	/// The `type`, `disabled`, and `aria-busy` attributes are managed by the
	/// component and therefore take precedence over caller-provided values,
	/// including case variants of those names.
	pub fn attr(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
		self.attrs.push((name.into(), value.into()));
		self
	}
}

impl<T, E, P> Component for ActionButton<T, E, P>
where
	T: Clone + 'static,
	E: Clone + 'static,
	P: 'static,
{
	fn render(&self) -> Page {
		let action = self.action;
		let payload = Rc::clone(&self.payload);
		let children = self.children.clone();
		let attrs = self.attrs.clone();

		let payload_for_click = Rc::clone(&payload);
		let on_click = move |_event| {
			if !action.is_pending() {
				action.dispatch(payload_for_click());
			}
		};

		let mut button = PageElement::new("button").attr("type", "button");
		for (name, value) in &attrs {
			if !name.eq_ignore_ascii_case("type")
				&& !name.eq_ignore_ascii_case("disabled")
				&& !name.eq_ignore_ascii_case("aria-busy")
			{
				button = button.attr(name.clone(), value.clone());
			}
		}
		let disabled_action = action;
		let busy_action = action;
		button
			.reactive_attr("disabled", move || {
				disabled_action.is_pending().then(|| "disabled".into())
			})
			.reactive_attr("aria-busy", move || {
				busy_action.is_pending().then(|| "true".into())
			})
			.on(EventType::Click, on_click.into_event_handler())
			.child(children)
			.into_page()
	}

	fn name() -> &'static str {
		"ActionButton"
	}
}

impl<T, E, P> IntoPage for ActionButton<T, E, P>
where
	T: Clone + 'static,
	E: Clone + 'static,
	P: 'static,
{
	fn into_page(self) -> Page {
		self.render()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::reactive::{ReactiveScope, use_action};
	#[cfg(all(native, feature = "testing"))]
	use std::{cell::RefCell, rc::Rc};

	#[test]
	fn action_button_renders_idle_markup() {
		ReactiveScope::run(|| {
			let action = use_action(|_: u32| async { Ok::<u32, String>(1) });
			let button = ActionButton::new(action, 7_u32, Page::text("Save"))
				.attr("aria-label", "Save project")
				.attr("type", "submit");

			assert_eq!(
				button.render().render_to_string(),
				r#"<button type="button" aria-label="Save project">Save</button>"#
			);
		});
	}

	#[test]
	fn action_button_reports_its_component_name() {
		assert_eq!(ActionButton::<u32, String, u32>::name(), "ActionButton");
	}

	#[cfg(all(native, feature = "testing"))]
	#[test]
	fn pending_state_controls_and_overrides_managed_attributes() {
		let queued = Rc::new(RefCell::new(Vec::new()));
		let queued_for_sink = Rc::clone(&queued);
		let _task_sink = crate::platform::install_task_sink(move |task| {
			queued_for_sink.borrow_mut().push(task);
		});

		ReactiveScope::run(|| {
			let action = use_action(|_: u32| async { Ok::<u32, String>(1) });
			action.dispatch(7_u32);

			let button = ActionButton::new(action, 7_u32, Page::text("Save"))
				.attr("type", "submit")
				.attr("disabled", "false")
				.attr("aria-busy", "false")
				.attr("data-kind", "save");

			assert_eq!(
				button.render().render_to_string(),
				r#"<button type="button" data-kind="save" disabled="disabled" aria-busy="true">Save</button>"#
			);
		});
		assert_eq!(queued.borrow().len(), 1);
	}
}
