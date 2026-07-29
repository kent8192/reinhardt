use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use reinhardt_core::page::IntoPage;
use reinhardt_core::reactive::{ReactiveScope, Signal};
use reinhardt_core::types::page::{DeferredNode, EventName, Page, PageElement, SuspenseNode};
use reinhardt_event_catalog::{EVENT_SPECS, EventInterface};
use rstest::rstest;

use super::{EventError, EventFixture, EventFixtureError, QueryError, Role, render};
use crate::Callback;
use crate::event::{
	ChangeEvent, ClickEvent, EventPayload, InputEvent, KeyDownEvent, Modifiers, Point, PointerKind,
	PointerMoveEvent, typed_event_handler,
};
use crate::reactive::hooks::use_effect;
use serial_test::serial;

use crate::reactive::{QueryFamily, QueryOptions, QueryStatus, use_query};

fn scheduled_query_component() -> Page {
	let query = use_query(
		QueryFamily::<(), _, _>::new("scheduled-query").query((), || async {
			Ok::<_, String>("Scheduled query result".to_string())
		}),
		QueryOptions::default(),
	);
	Page::reactive(move || match query.snapshot().status {
		QueryStatus::Idle | QueryStatus::Pending => Page::text("Loading"),
		QueryStatus::Success => Page::text(
			query
				.data()
				.expect("a successful query snapshot contains data"),
		),
		QueryStatus::Error => Page::text(
			query
				.error()
				.expect("an error query snapshot contains an error"),
		),
	})
}

#[cfg(not(feature = "msw"))]
fn screen_scoped_query_component(value: &'static str) -> Page {
	let query = use_query(
		QueryFamily::<(), _, _>::new("screen-scoped-query").query((), move || async move {
			Ok::<_, String>(value.to_string())
		}),
		QueryOptions::default(),
	);
	Page::reactive(move || match query.snapshot().status {
		QueryStatus::Idle | QueryStatus::Pending => Page::text("Loading"),
		QueryStatus::Success => Page::text(
			query
				.data()
				.expect("a successful query snapshot contains data"),
		),
		QueryStatus::Error => Page::text(
			query
				.error()
				.expect("an error query snapshot contains an error"),
		),
	})
}

#[tokio::test]
async fn query_fetches_are_scheduled_until_screen_settles() {
	// Arrange
	let screen = render(scheduled_query_component);

	// Assert
	assert_eq!(screen.pretty(), "Loading\n");

	// Act
	screen.settle().await;

	// Assert
	assert_eq!(screen.pretty(), "Scheduled query result\n");
}

#[cfg(not(feature = "msw"))]
#[tokio::test]
async fn query_cache_is_scoped_per_screen_without_msw() {
	// Arrange
	let first = render(|| screen_scoped_query_component("first"));
	first.settle().await;

	// Act
	let second = render(|| screen_scoped_query_component("second"));
	second.settle().await;
	let second_output = second.pretty();

	// Assert
	assert_eq!(first.pretty(), "first\n");
	assert_eq!(second_output, "second\n");
}

#[test]
fn renders_page_tree_and_pretty_output() {
	let screen = render(
		PageElement::new("section")
			.attr("id", "hero")
			.child(PageElement::new("h1").child("Hello"))
			.child(Page::fragment([Page::text("Intro")])),
	);

	assert_eq!(
		screen.pretty(),
		"<section id=\"hero\">\n  <h1>\n    Hello\n  </h1>\n  Intro\n</section>\n"
	);
}

#[test]
#[serial(reactive_runtime)]
fn render_creates_a_scope_for_hook_using_views() {
	let screen = render(|| {
		let count = Signal::new(42_i32);
		PageElement::new("p").child(count.get().to_string())
	});

	assert_eq!(screen.get_by_text("42").tag_name(), "p");
}

#[test]
#[serial(reactive_runtime)]
fn effect_cleanup_can_use_page_handles_until_scope_disposal_finishes() {
	let calls = Arc::new(AtomicUsize::new(0));
	let scope = ReactiveScope::new();

	scope.enter(|| {
		let callback = Callback::new({
			let calls = Arc::clone(&calls);
			move |_: ()| {
				calls.fetch_add(1, Ordering::Relaxed);
			}
		});
		use_effect(
			move || {
				let callback = callback;
				Some(move || callback.call(()))
			},
			crate::deps![],
		);
	});

	scope.dispose();

	assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[test]
fn suspense_nodes_render_active_branch_without_boundary_id() {
	let pending_screen = render(Page::Suspense(SuspenseNode::new(
		None,
		|| true,
		|| PageElement::new("span").child("Loading").into_page(),
		|| PageElement::new("span").child("Ready").into_page(),
	)));

	assert_eq!(pending_screen.get_by_text("Loading").tag_name(), "span");
	assert!(pending_screen.query_by_text("Ready").is_none());

	let resolved_screen = render(Page::Suspense(SuspenseNode::new(
		None,
		|| false,
		|| PageElement::new("span").child("Loading").into_page(),
		|| PageElement::new("span").child("Ready").into_page(),
	)));

	assert_eq!(resolved_screen.get_by_text("Ready").tag_name(), "span");
	assert!(resolved_screen.query_by_text("Loading").is_none());
}

#[test]
fn deferred_nodes_render_content_branch_with_custom_key() {
	let screen = render(Page::Deferred(DeferredNode::new(
		"deferred-test",
		|| {
			PageElement::new("span")
				.child("Deferred fallback")
				.into_page()
		},
		|| {
			PageElement::new("span")
				.child("Deferred content")
				.into_page()
		},
	)));

	assert_eq!(screen.get_by_text("Deferred content").tag_name(), "span");
	assert!(screen.query_by_text("Deferred fallback").is_none());
}

#[test]
fn pretty_text_only_screen_has_trailing_newline() {
	let screen = render(Page::text("Hello"));

	assert_eq!(screen.pretty(), "Hello\n");
}

#[test]
fn renders_pending_suspense_branch() {
	let screen = render(Page::Suspense(SuspenseNode::new(
		None,
		|| true,
		|| PageElement::new("p").child("Loading").into_page(),
		|| PageElement::new("main").child("Ready").into_page(),
	)));

	assert_eq!(screen.get_by_text("Loading").tag_name(), "p");
	assert!(screen.query_by_text("Ready").is_none());
}

#[test]
fn renders_deferred_content_branch() {
	let screen = render(Page::Deferred(DeferredNode::new(
		"deferred-content",
		|| PageElement::new("p").child("Deferred loading").into_page(),
		|| PageElement::new("main").child("Deferred ready").into_page(),
	)));

	assert_eq!(screen.get_by_text("Deferred ready").tag_name(), "main");
	assert!(screen.query_by_text("Deferred loading").is_none());
}

#[test]
fn queries_by_text_role_label_and_placeholder() {
	let screen = render(
		PageElement::new("form")
			.child(
				PageElement::new("label")
					.attr("for", "email")
					.child("Email"),
			)
			.child(
				PageElement::new("input")
					.attr("id", "email")
					.attr("placeholder", "name@example.com"),
			)
			.child(PageElement::new("button").child("Submit")),
	);

	assert_eq!(screen.get_by_text("Submit").tag_name(), "button");
	assert_eq!(screen.get_by_role(Role::Button, "Submit").text(), "Submit");
	assert_eq!(screen.get_by_label("Email").tag_name(), "input");
	assert_eq!(
		screen.get_by_placeholder("name@example.com").tag_name(),
		"input"
	);
}

#[tokio::test]
async fn optional_and_async_queries_cover_role_label_and_placeholder() {
	let screen = render(
		PageElement::new("form")
			.child(
				PageElement::new("label")
					.attr("for", "email")
					.child("Email"),
			)
			.child(
				PageElement::new("input")
					.attr("id", "email")
					.attr("placeholder", "name@example.com"),
			)
			.child(PageElement::new("button").child("Submit")),
	);

	assert_eq!(
		screen
			.query_by_role(Role::Button, "Submit")
			.expect("button should be found")
			.tag_name(),
		"button"
	);
	assert!(screen.query_by_role(Role::Button, "Missing").is_none());
	assert_eq!(
		screen
			.query_by_label("Email")
			.expect("label should be found")
			.tag_name(),
		"input"
	);
	assert!(screen.query_by_label("Missing").is_none());
	assert_eq!(
		screen
			.query_by_placeholder("name@example.com")
			.expect("placeholder should be found")
			.tag_name(),
		"input"
	);
	assert!(screen.query_by_placeholder("Missing").is_none());
	assert_eq!(screen.find_by_label("Email").await.tag_name(), "input");
	assert_eq!(
		screen
			.try_find_by_placeholder("name@example.com")
			.await
			.unwrap()
			.tag_name(),
		"input"
	);
}

#[test]
fn hidden_elements_are_excluded_from_queries() {
	let screen = render(
		PageElement::new("div")
			.child(
				PageElement::new("button")
					.attr("aria-hidden", "true")
					.child("Save"),
			)
			.child(PageElement::new("button").child("Save")),
	);

	assert_eq!(screen.get_by_text("Save").tag_name(), "button");
}

#[test]
fn suspense_nodes_render_active_branch_with_boundary_id() {
	let pending_screen = render(Page::Suspense(SuspenseNode::new(
		Some("pending-boundary".to_string()),
		|| true,
		|| PageElement::new("p").child("Loading").into_page(),
		|| PageElement::new("p").child("Ready").into_page(),
	)));
	let resolved_screen = render(Page::Suspense(SuspenseNode::new(
		Some("resolved-boundary".to_string()),
		|| false,
		|| PageElement::new("p").child("Loading").into_page(),
		|| PageElement::new("p").child("Ready").into_page(),
	)));

	assert_eq!(pending_screen.get_by_text("Loading").tag_name(), "p");
	assert!(pending_screen.query_by_text("Ready").is_none());
	assert_eq!(resolved_screen.get_by_text("Ready").tag_name(), "p");
	assert!(resolved_screen.query_by_text("Loading").is_none());
}

#[test]
fn deferred_nodes_render_content_branch_with_standard_key() {
	let screen = render(Page::Deferred(DeferredNode::new(
		"deferred-content",
		|| PageElement::new("p").child("Deferred fallback").into_page(),
		|| PageElement::new("p").child("Deferred content").into_page(),
	)));

	assert_eq!(screen.get_by_text("Deferred content").tag_name(), "p");
	assert!(screen.query_by_text("Deferred fallback").is_none());
}

#[test]
fn multiple_matches_return_query_error() {
	let screen = render(
		PageElement::new("div")
			.child(PageElement::new("button").child("Save"))
			.child(PageElement::new("button").child("Save")),
	);

	assert!(matches!(
		screen.try_get_by_text("Save"),
		Err(QueryError::MultipleMatches)
	));
}

#[test]
fn click_dispatches_native_event() {
	let clicked = Rc::new(Cell::new(false));
	let clicked_for_handler = clicked.clone();
	let screen = render(
		PageElement::new("button")
			.listener("click", move |_| clicked_for_handler.set(true))
			.child("Save"),
	);

	screen.get_by_role(Role::Button, "Save").click();

	assert!(clicked.get());
}

#[test]
fn input_updates_internal_value_before_dispatch() {
	let called = Rc::new(Cell::new(false));
	let called_for_handler = called.clone();
	let screen = render(
		PageElement::new("input")
			.attr("value", "old")
			.attr("placeholder", "Job name")
			.listener("input", move |_| called_for_handler.set(true)),
	);
	let input = screen.get_by_placeholder("Job name");

	input.input("new");

	assert!(called.get());
	assert_eq!(input.value().as_deref(), Some("new"));
}

#[tokio::test]
async fn detached_element_reads_return_error() {
	let scope = ReactiveScope::new();
	let (show, screen, button) = scope.enter(|| {
		let show = Signal::new(true);
		let show_for_render = show;
		let screen = render(Page::reactive(move || {
			if show_for_render.get() {
				PageElement::new("button").child("Save").into_page()
			} else {
				Page::Empty
			}
		}));
		let button = screen.get_by_role(Role::Button, "Save");
		(show, screen, button)
	});

	scope.enter(|| show.set(false));
	screen.settle().await;

	assert_eq!(button.try_text(), Err(EventError::DetachedElement));
	assert_eq!(button.try_tag_name(), Err(EventError::DetachedElement));
	assert_eq!(button.try_value(), Err(EventError::DetachedElement));
}

#[test]
fn missing_event_handler_returns_error() {
	let screen = render(PageElement::new("button").child("Save"));

	assert_eq!(
		screen.get_by_role(Role::Button, "Save").try_click(),
		Err(EventError::MissingHandler)
	);
}

#[rstest]
fn typed_click_fixture_reaches_the_click_handler() {
	let clicked = Arc::new(AtomicBool::new(false));
	let clicked_for_handler = Arc::clone(&clicked);
	let screen = render(
		PageElement::new("button")
			.on(
				ClickEvent::EVENT,
				typed_event_handler::<ClickEvent, _>(move |event: ClickEvent| {
					assert_eq!(event.event_type(), "click");
					clicked_for_handler.store(true, Ordering::SeqCst);
				}),
			)
			.child("Save"),
	);

	screen
		.get_by_role(Role::Button, "Save")
		.dispatch(EventFixture::click())
		.expect("click fixture should dispatch");

	assert!(clicked.load(Ordering::SeqCst));
}

#[rstest]
fn input_fixture_updates_value_before_typed_handler_runs() {
	let observed = Arc::new(Mutex::new(None));
	let observed_for_handler = Arc::clone(&observed);
	let screen = render(
		PageElement::new("input")
			.attr("aria-label", "Name")
			.attr("value", "old")
			.on(
				InputEvent::EVENT,
				typed_event_handler::<InputEvent, _>(move |event: InputEvent| {
					*observed_for_handler.lock().unwrap() = Some(event.value());
				}),
			),
	);
	let input = screen.get_by_label("Name");

	input
		.dispatch(EventFixture::input().value("new"))
		.expect("input fixture should dispatch");

	assert_eq!(input.value().as_deref(), Some("new"));
	assert_eq!(
		observed.lock().unwrap().as_ref(),
		Some(&Ok::<String, crate::event::EventTargetError>(
			"new".to_string()
		))
	);
}

#[rstest]
fn checked_fixture_updates_checkbox_before_change_handler_runs() {
	let observed = Arc::new(Mutex::new(None));
	let observed_for_handler = Arc::clone(&observed);
	let screen = render(
		PageElement::new("input")
			.attr("aria-label", "Enabled")
			.attr("type", "checkbox")
			.on(
				ChangeEvent::EVENT,
				typed_event_handler::<ChangeEvent, _>(move |event: ChangeEvent| {
					*observed_for_handler.lock().unwrap() = Some(event.checked());
				}),
			),
	);

	screen
		.get_by_label("Enabled")
		.dispatch(EventFixture::change().checked(true))
		.expect("checked fixture should dispatch");

	assert_eq!(
		observed.lock().unwrap().as_ref(),
		Some(&Ok::<bool, crate::event::EventTargetError>(true))
	);
}

#[rstest]
fn keyboard_fixture_carries_key_code_repeat_and_modifiers() {
	let observed = Arc::new(Mutex::new(None));
	let observed_for_handler = Arc::clone(&observed);
	let screen = render(PageElement::new("input").attr("aria-label", "Search").on(
		KeyDownEvent::EVENT,
		typed_event_handler::<KeyDownEvent, _>(move |event: KeyDownEvent| {
			*observed_for_handler.lock().unwrap() =
				Some((event.key(), event.code(), event.repeat(), event.modifiers()));
		}),
	));

	screen
		.get_by_label("Search")
		.dispatch(
			EventFixture::key_down()
				.key("Enter")
				.code("NumpadEnter")
				.repeat(true)
				.modifiers(Modifiers {
					control: true,
					shift: true,
					..Modifiers::default()
				}),
		)
		.expect("keyboard fixture should dispatch");

	assert_eq!(
		observed.lock().unwrap().as_ref(),
		Some(&(
			"Enter".to_string(),
			"NumpadEnter".to_string(),
			true,
			Modifiers {
				control: true,
				shift: true,
				..Modifiers::default()
			},
		))
	);
}

#[rstest]
fn pointer_fixture_carries_position_kind_and_pressure() {
	let observed = Arc::new(Mutex::new(None));
	let observed_for_handler = Arc::clone(&observed);
	let screen = render(
		PageElement::new("div")
			.on(
				PointerMoveEvent::EVENT,
				typed_event_handler::<PointerMoveEvent, _>(move |event: PointerMoveEvent| {
					*observed_for_handler.lock().unwrap() = Some((
						event.client_position(),
						event.pointer_type(),
						event.pressure(),
					));
				}),
			)
			.child("Canvas"),
	);

	screen
		.get_by_text("Canvas")
		.dispatch(
			EventFixture::pointer_move()
				.client_position(120.0, 80.0)
				.pointer_kind(PointerKind::Pen)
				.pressure(0.5),
		)
		.expect("pointer fixture should dispatch");

	assert_eq!(
		observed.lock().unwrap().as_ref(),
		Some(&(Point::new(120.0, 80.0), PointerKind::Pen, 0.5))
	);
}

#[rstest]
fn custom_fixture_dispatches_a_generic_raw_event() {
	let observed = Rc::new(RefCell::new(None));
	let observed_for_handler = Rc::clone(&observed);
	let screen = render(
		PageElement::new("div")
			.listener("item-selected", move |event| {
				*observed_for_handler.borrow_mut() =
					Some((event.event_type().to_string(), event.payload().interface()));
			})
			.child("Status"),
	);

	screen
		.get_by_text("Status")
		.dispatch(EventFixture::custom("item-selected"))
		.expect("custom fixture should dispatch");

	assert_eq!(
		observed.borrow().as_ref(),
		Some(&("item-selected".to_string(), EventInterface::Generic))
	);
}

#[rstest]
fn catalog_family_mismatch_is_rejected_during_build() {
	let error = EventFixture::click()
		.interface(EventInterface::Keyboard)
		.build()
		.expect_err("click must reject keyboard payload data");

	assert!(matches!(
		error,
		EventFixtureError::IncompatibleFamily {
			event,
			actual: EventInterface::Keyboard,
			..
		} if event == "click"
	));
}

#[rstest]
fn generic_dispatch_reports_a_missing_handler() {
	let screen = render(PageElement::new("button").child("Save"));

	assert_eq!(
		screen
			.get_by_role(Role::Button, "Save")
			.dispatch(EventFixture::click()),
		Err(EventError::MissingHandler)
	);
}

#[rstest]
fn unsupported_target_state_is_rejected_before_handler_invocation() {
	let called = Rc::new(Cell::new(false));
	let called_for_handler = Rc::clone(&called);
	let screen = render(
		PageElement::new("button")
			.listener("input", move |_| called_for_handler.set(true))
			.child("Save"),
	);

	let error = screen
		.get_by_role(Role::Button, "Save")
		.dispatch(EventFixture::input().value("invalid"))
		.expect_err("buttons cannot receive input values");

	assert!(matches!(
		error,
		EventError::InvalidFixture(EventFixtureError::UnsupportedTargetState {
			property: "value",
			actual_tag,
		}) if actual_tag == "button"
	));
	assert!(!called.get());
}

#[rstest]
fn invalid_target_state_does_not_apply_valid_fields_partially() {
	let screen = render(
		PageElement::new("input")
			.attr("aria-label", "Name")
			.attr("value", "old")
			.listener("input", |_| {}),
	);
	let input = screen.get_by_label("Name");

	let error = input
		.dispatch(EventFixture::input().value("new").checked(true))
		.expect_err("text inputs cannot receive checked state");

	assert!(matches!(
		error,
		EventError::InvalidFixture(EventFixtureError::UnsupportedTargetState {
			property: "checked",
			actual_tag,
		}) if actual_tag == "input"
	));
	assert_eq!(input.value().as_deref(), Some("old"));
}

#[rstest]
fn contenteditable_enable_and_value_patch_validate_against_final_state() {
	let observed = Arc::new(Mutex::new(None));
	let observed_for_handler = Arc::clone(&observed);
	let screen = render(
		PageElement::new("div")
			.listener("input", move |event| {
				*observed_for_handler.lock().unwrap() = event
					.current_target()
					.and_then(|target| target.value().map(ToOwned::to_owned));
			})
			.child("Draft"),
	);
	let editable = screen.get_by_text("Draft");

	editable
		.dispatch(EventFixture::input().content_editable(true).value("Edited"))
		.expect("final contenteditable target should accept a value");

	assert_eq!(editable.value().as_deref(), Some("Edited"));
	assert_eq!(observed.lock().unwrap().as_deref(), Some("Edited"));
}

#[rstest]
fn contenteditable_disable_and_value_patch_is_rejected_atomically() {
	let observed = Rc::new(RefCell::new(Vec::new()));
	let observed_for_handler = Rc::clone(&observed);
	let screen = render(
		PageElement::new("div")
			.attr("contenteditable", "true")
			.listener("input", move |event| {
				observed_for_handler.borrow_mut().push(
					event
						.current_target()
						.and_then(|target| target.value().map(ToOwned::to_owned)),
				);
			})
			.child("Draft"),
	);
	let editable = screen.get_by_text("Draft");

	let error = editable
		.dispatch(
			EventFixture::input()
				.content_editable(false)
				.value("Edited"),
		)
		.expect_err("final non-contenteditable target must reject a value");

	assert!(matches!(
		error,
		EventError::InvalidFixture(EventFixtureError::UnsupportedTargetState {
			property: "value",
			actual_tag,
		}) if actual_tag == "div"
	));
	assert_eq!(editable.value().as_deref(), Some("Draft"));
	assert!(observed.borrow().is_empty());

	editable
		.dispatch(EventFixture::input().value("Recovered"))
		.expect("rejected compound patch must preserve contenteditable state");

	assert_eq!(editable.value().as_deref(), Some("Recovered"));
	assert_eq!(
		observed.borrow().as_slice(),
		[Some("Recovered".to_string())]
	);
}

#[rstest]
fn known_and_custom_events_with_the_same_type_share_dom_dispatch() {
	let calls = Arc::new(Mutex::new(Vec::new()));
	let known_calls = Arc::clone(&calls);
	let custom_calls = Arc::clone(&calls);
	let screen = render(
		PageElement::new("button")
			.listener("click", move |_| known_calls.lock().unwrap().push("known"))
			.on(
				EventName::Custom(Cow::Borrowed("click")),
				Arc::new(move |_| custom_calls.lock().unwrap().push("custom")),
			)
			.child("Run"),
	);
	let button = screen.get_by_role(Role::Button, "Run");

	button
		.dispatch(EventFixture::click())
		.expect("known click should dispatch");
	button
		.dispatch(EventFixture::custom("click"))
		.expect("custom click should dispatch");

	assert_eq!(
		calls.lock().unwrap().as_slice(),
		["known", "custom", "known", "custom"]
	);
}

#[rstest]
fn invalid_fixture_is_exposed_as_the_event_error_source() {
	let screen = render(
		PageElement::new("button")
			.listener("click", |_| {})
			.child("Run"),
	);
	let event_error = screen
		.get_by_role(Role::Button, "Run")
		.dispatch(EventFixture::click().interface(EventInterface::Keyboard))
		.expect_err("click must reject keyboard payload data");

	let source = std::error::Error::source(&event_error)
		.expect("invalid fixture should be exposed as the source");

	assert!(matches!(
		source.downcast_ref::<EventFixtureError>(),
		Some(EventFixtureError::IncompatibleFamily {
			event,
			expected: EventInterface::Pointer,
			actual: EventInterface::Keyboard,
			..
		}) if event == "click"
	));
}

#[rstest]
fn every_catalog_event_builds_its_primary_family_defaults() {
	for spec in EVENT_SPECS {
		let event = EventFixture::new(spec.kind)
			.build()
			.expect("catalog default fixture should build");

		assert_eq!(event.event_type(), spec.dom_name);
		assert_eq!(event.payload().interface(), spec.primary_interface);
		assert_eq!(event.base().bubbles, spec.behavior.bubbles);
		assert_eq!(event.base().cancelable, spec.behavior.cancelable);
		assert_eq!(event.base().composed, spec.behavior.composed);
	}
}

macro_rules! assert_catalog_wrapper_fixture_parity {
	(
		$(
			$kind:ident,
			$dom_name:literal,
			$payload:ident,
			$interface:ident,
			[$($fallback:ident),* $(,)?],
			[$($capability:ident),* $(,)?],
			$bubbles:literal,
			$cancelable:literal,
			$composed:literal,
			$fixture_defaults:ident;
		)*
	) => {
		// Catalog coverage intentionally includes deprecated compatibility payloads.
		#[allow(deprecated)]
		#[rstest]
		fn every_catalog_fixture_converts_to_its_generated_wrapper() {
			$(
				let raw = EventFixture::new(crate::event::KnownEvent::$kind)
					.build()
					.expect("catalog fixture must build");
				let payload = crate::event::$payload::try_from_raw(raw)
					.expect("catalog fixture must convert to its generated payload");

				assert_eq!(payload.event_type(), $dom_name);
			)*
		}
	};
}

reinhardt_event_catalog::__reinhardt_event_catalog!(assert_catalog_wrapper_fixture_parity);

#[rstest]
fn checked_and_keyboard_shortcuts_preserve_the_try_api_convention() {
	let checked = Arc::new(Mutex::new(None));
	let checked_for_handler = Arc::clone(&checked);
	let key = Arc::new(Mutex::new(None));
	let key_for_handler = Arc::clone(&key);
	let screen = render(Page::fragment([
		PageElement::new("input")
			.attr("aria-label", "Enabled")
			.attr("type", "checkbox")
			.on(
				ChangeEvent::EVENT,
				typed_event_handler::<ChangeEvent, _>(move |event: ChangeEvent| {
					*checked_for_handler.lock().unwrap() = Some(event.checked());
				}),
			)
			.into_page(),
		PageElement::new("input")
			.attr("aria-label", "Search")
			.on(
				KeyDownEvent::EVENT,
				typed_event_handler::<KeyDownEvent, _>(move |event: KeyDownEvent| {
					*key_for_handler.lock().unwrap() = Some(event.key());
				}),
			)
			.into_page(),
	]));

	screen.get_by_label("Enabled").change_checked(true);
	screen
		.get_by_label("Search")
		.try_key_down("Enter")
		.expect("key-down shortcut should dispatch");

	assert_eq!(
		checked.lock().unwrap().as_ref(),
		Some(&Ok::<bool, crate::event::EventTargetError>(true))
	);
	assert_eq!(key.lock().unwrap().as_deref(), Some("Enter"));
}

#[rstest]
fn select_file_and_contenteditable_target_snapshots_are_owned() {
	let selected = Arc::new(Mutex::new(None));
	let selected_for_handler = Arc::clone(&selected);
	let files = Arc::new(Mutex::new(None));
	let files_for_handler = Arc::clone(&files);
	let editable = Arc::new(Mutex::new(None));
	let editable_for_handler = Arc::clone(&editable);
	let screen = render(Page::fragment([
		PageElement::new("select")
			.attr("aria-label", "Roles")
			.attr("multiple", "multiple")
			.on(
				ChangeEvent::EVENT,
				typed_event_handler::<ChangeEvent, _>(move |event: ChangeEvent| {
					*selected_for_handler.lock().unwrap() = Some(event.selected_values());
				}),
			)
			.child(PageElement::new("option").attr("value", "admin"))
			.child(PageElement::new("option").attr("value", "editor"))
			.into_page(),
		PageElement::new("input")
			.attr("aria-label", "Upload")
			.attr("type", "file")
			.on(
				ChangeEvent::EVENT,
				typed_event_handler::<ChangeEvent, _>(move |event: ChangeEvent| {
					*files_for_handler.lock().unwrap() = Some(event.files());
				}),
			)
			.into_page(),
		PageElement::new("div")
			.attr("contenteditable", "true")
			.on(
				InputEvent::EVENT,
				typed_event_handler::<InputEvent, _>(move |event: InputEvent| {
					*editable_for_handler.lock().unwrap() = Some(event.value());
				}),
			)
			.child("Draft")
			.into_page(),
	]));

	screen
		.get_by_label("Roles")
		.dispatch(EventFixture::change().selected_values(["admin", "editor"]))
		.expect("select fixture should dispatch");
	screen
		.get_by_label("Upload")
		.dispatch(EventFixture::change().file("notes.txt", "text/plain", 12, 1_000))
		.expect("file fixture should dispatch");
	screen
		.get_by_text("Draft")
		.dispatch(EventFixture::input().value("Edited"))
		.expect("contenteditable fixture should dispatch");

	assert_eq!(
		selected.lock().unwrap().as_ref(),
		Some(&Ok::<Vec<String>, crate::event::EventTargetError>(vec![
			"admin".to_string(),
			"editor".to_string(),
		]))
	);
	let file_snapshot = files.lock().unwrap();
	let file_snapshot = file_snapshot
		.as_ref()
		.expect("files handler should run")
		.as_ref()
		.expect("file target should be supported");
	assert_eq!(file_snapshot.len(), 1);
	assert_eq!(file_snapshot[0].name(), "notes.txt");
	assert_eq!(file_snapshot[0].media_type(), "text/plain");
	assert_eq!(file_snapshot[0].size(), 12);
	assert_eq!(file_snapshot[0].last_modified(), 1_000);
	assert_eq!(
		editable.lock().unwrap().as_ref(),
		Some(&Ok::<String, crate::event::EventTargetError>(
			"Edited".to_string()
		))
	);
}
