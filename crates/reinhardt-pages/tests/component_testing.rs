#![cfg(all(native, feature = "testing"))]

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reinhardt_core::reactive::ReactiveScope;
use reinhardt_core::types::page::{
	DeferredNode, EventType, IntoPage, Outlet, Page, PageElement, SuspenseNode,
};
use reinhardt_pages::callback::async_handler;
use reinhardt_pages::component::suspense::SuspenseBoundary;
use reinhardt_pages::deps;
use reinhardt_pages::event::{ClickEvent, EventPayload, FocusEvent, typed_event_handler};
use reinhardt_pages::prelude::spawn_task;
use reinhardt_pages::reactive::hooks::use_action;
#[cfg(feature = "msw")]
use reinhardt_pages::reactive::{QueryOptions, use_query};
use reinhardt_pages::reactive::{ResourceState, Signal, use_resource};
#[cfg(feature = "msw")]
use reinhardt_pages::server_fn::{ServerFnError, server_fn};
use reinhardt_pages::testing::component::{EventError, EventFixture, Role, render};
use reinhardt_pages::{Callback, NativeEvent, page};
use rstest::rstest;
use serial_test::serial;

static TYPED_ASYNC_CALLED: AtomicBool = AtomicBool::new(false);

struct AtomicReset(&'static AtomicBool);

impl Drop for AtomicReset {
	fn drop(&mut self) {
		self.0.store(false, Ordering::SeqCst);
	}
}

async fn record_typed_async_click(event: ClickEvent) {
	assert_eq!(event.event_type(), "click");
	tokio::task::yield_now().await;
	TYPED_ASYNC_CALLED.store(true, Ordering::SeqCst);
}

fn text_page(text: impl Into<String>) -> Page {
	PageElement::new("div").child(text.into()).into_page()
}

fn string_resource_page(state: ResourceState<String, String>) -> Page {
	match state {
		ResourceState::Loading => text_page("Loading"),
		ResourceState::Success(value) => text_page(value),
		ResourceState::Error(error) => text_page(error),
	}
}

fn index_job_component() -> Page {
	let resource = use_resource(
		|| async { Ok::<String, String>("Index job".to_string()) },
		deps![],
	);
	Page::reactive(move || string_resource_page(resource.get()))
}

fn ready_component() -> Page {
	let resource = use_resource(
		|| async { Ok::<String, String>("Ready".to_string()) },
		deps![],
	);
	Page::reactive(move || string_resource_page(resource.get()))
}

fn suspense_resource_component() -> Page {
	let resource = use_resource(
		|| async { Ok::<String, String>("Ready".to_string()) },
		deps![],
	);
	let content_resource = resource.clone();
	SuspenseBoundary::new()
		.fallback(|| text_page("Loading"))
		.track(resource)
		.content(move || string_resource_page(content_resource.get()))
		.into_page()
}

fn mixed_resource_component() -> Page {
	let pending = use_resource(std::future::pending::<Result<String, String>>, deps![]);
	let ready = use_resource(
		|| async { Ok::<String, String>("Ready".to_string()) },
		deps![],
	);
	Page::reactive(move || {
		let _ = pending.get();
		string_resource_page(ready.get())
	})
}

fn save_component() -> Page {
	let action = use_action(|_: ()| async { Ok::<String, String>("Saved".to_string()) });
	let button_action = action.clone();
	PageElement::new("div")
		.child(
			PageElement::new("button")
				.listener("click", move |_| button_action.dispatch(()))
				.child("Save"),
		)
		.child(Page::reactive(move || match action.result() {
			Some(value) => text_page(value),
			None => text_page("Idle"),
		}))
		.into_page()
}

fn action_dispatching_component() -> Page {
	let action = use_action(|value: i32| async move { Ok::<i32, String>(value) });
	let dispatch: Callback<NativeEvent, ()> = action.dispatching(5);
	let dispatch_with: Callback<NativeEvent, ()> = action.dispatching_with(|| 6);
	let result_action = action.clone();

	PageElement::new("div")
		.child(
			PageElement::new("button")
				.listener("click", move |event| dispatch.call(event))
				.child("Dispatch fixed payload"),
		)
		.child(
			PageElement::new("button")
				.listener("click", move |event| dispatch_with.call(event))
				.child("Dispatch computed payload"),
		)
		.child(Page::reactive(move || match result_action.result() {
			Some(value) => text_page(value.to_string()),
			None => text_page("Idle"),
		}))
		.into_page()
}

fn async_click_component() -> Page {
	let message = Signal::new("Idle".to_string());
	let click_message = message.clone();
	PageElement::new("div")
		.child(
			PageElement::new("button")
				.on(
					EventType::Click,
					async_handler(move |_| {
						let click_message = click_message.clone();
						async move {
							click_message.set("Clicked".to_string());
						}
					}),
				)
				.child("Run"),
		)
		.child(Page::reactive(move || text_page(message.get())))
		.into_page()
}

fn delayed_async_click_component() -> Page {
	let message = Signal::new("Idle".to_string());
	let click_message = message.clone();
	PageElement::new("div")
		.child(
			PageElement::new("button")
				.on(
					EventType::Click,
					async_handler(move |_| {
						let click_message = click_message.clone();
						async move {
							tokio::time::sleep(Duration::from_millis(1)).await;
							click_message.set("Delayed".to_string());
						}
					}),
				)
				.child("Run"),
		)
		.child(Page::reactive(move || text_page(message.get())))
		.into_page()
}

#[test]
fn native_component_testing_public_surface_is_available() {
	let screen = render(
		PageElement::new("main")
			.attr("aria-label", "Dashboard")
			.child(PageElement::new("h1").child("Dashboard"))
			.child(PageElement::new("button").child("Refresh")),
	);

	assert_eq!(
		screen.get_by_role(Role::Main, "Dashboard").tag_name(),
		"main"
	);
	assert_eq!(
		screen.get_by_role(Role::Button, "Refresh").text(),
		"Refresh"
	);
}

#[test]
fn label_query_does_not_match_placeholder_only_inputs() {
	let screen = render(PageElement::new("input").attr("placeholder", "Email"));

	assert!(screen.try_get_by_label("Email").is_err());
	assert_eq!(screen.get_by_placeholder("Email").tag_name(), "input");
}

#[test]
fn presentation_role_suppresses_implicit_role_queries() {
	let screen = render(
		PageElement::new("button")
			.attr("role", "presentation")
			.child("Save"),
	);

	assert!(screen.try_get_by_role(Role::Button, "Save").is_err());
	assert!(screen.query_by_text("Save").is_some());
}

#[test]
fn text_queries_ignore_hidden_descendant_text() {
	let screen = render(
		PageElement::new("div").child(PageElement::new("span").attr("hidden", "").child("Secret")),
	);

	assert!(screen.try_get_by_text("Secret").is_err());
}

#[test]
fn outlet_pages_render_inline_children_and_placeholders() {
	let inline = render(Page::outlet(Outlet::inline(text_page("Nested"))));
	assert!(inline.query_by_text("Nested").is_some());

	let placeholder = render(Page::outlet(Outlet::placeholder("main")));
	let pretty = placeholder.pretty();
	assert!(pretty.contains("<reinhardt-outlet"));
	assert!(pretty.contains("data-rh-outlet-id=\"main\""));
}

#[test]
fn suspense_and_deferred_pages_render_component_test_branches() {
	let pending = render(Page::Suspense(SuspenseNode::new(
		Some("pending".to_string()),
		|| true,
		|| text_page("Loading"),
		|| text_page("Ready"),
	)));
	assert!(pending.query_by_text("Loading").is_some());
	assert!(pending.query_by_text("Ready").is_none());

	let resolved = render(Page::Suspense(SuspenseNode::new(
		Some("resolved".to_string()),
		|| false,
		|| text_page("Loading"),
		|| text_page("Ready"),
	)));
	assert!(resolved.query_by_text("Ready").is_some());
	assert!(resolved.query_by_text("Loading").is_none());

	let deferred = render(Page::Deferred(DeferredNode::new(
		"deferred",
		|| text_page("Deferred fallback"),
		|| text_page("Deferred content"),
	)));
	assert!(deferred.query_by_text("Deferred content").is_some());
	assert!(deferred.query_by_text("Deferred fallback").is_none());
}

#[tokio::test]
async fn settle_rerenders_suspense_branch_changes() {
	let pending = Rc::new(Cell::new(true));
	let pending_for_boundary = Rc::clone(&pending);
	let screen = render(Page::Suspense(SuspenseNode::new(
		Some("async-boundary".to_string()),
		move || pending_for_boundary.get(),
		|| text_page("Loading"),
		|| text_page("Ready"),
	)));

	assert!(screen.query_by_text("Loading").is_some());
	assert!(screen.query_by_text("Ready").is_none());

	pending.set(false);
	screen.settle().await;

	assert!(screen.query_by_text("Ready").is_some());
	assert!(screen.query_by_text("Loading").is_none());
}

#[tokio::test]
async fn suspense_pages_rerender_resolved_resource_after_settle() {
	let screen = render(suspense_resource_component);

	assert!(screen.query_by_text("Loading").is_some());
	assert!(screen.query_by_text("Ready").is_none());

	screen.settle().await;

	assert!(screen.query_by_text("Ready").is_some());
	assert!(screen.query_by_text("Loading").is_none());
}

#[tokio::test]
async fn action_dispatching_callbacks_schedule_native_actions() {
	let screen = render(action_dispatching_component);

	screen
		.get_by_role(Role::Button, "Dispatch fixed payload")
		.click();
	screen.settle().await;
	assert!(screen.query_by_text("5").is_some());

	screen
		.get_by_role(Role::Button, "Dispatch computed payload")
		.click();
	screen.settle().await;
	assert!(screen.query_by_text("6").is_some());
}

#[test]
fn role_queries_follow_fallback_tokens_and_input_rules() {
	let screen = render(Page::fragment([
		PageElement::new("div")
			.attr("role", "foo button")
			.child("Fallback button")
			.into_page(),
		PageElement::new("div")
			.attr("role", "switch checkbox")
			.attr("aria-label", "Power")
			.into_page(),
		PageElement::new("label")
			.child("Password")
			.child(PageElement::new("input").attr("type", "password"))
			.into_page(),
		PageElement::new("input")
			.attr("type", "submit")
			.attr("value", "Save")
			.into_page(),
	]));

	assert_eq!(
		screen
			.get_by_role(Role::Button, "Fallback button")
			.tag_name(),
		"div"
	);
	assert_eq!(
		screen.get_by_role(Role::Checkbox, "Power").tag_name(),
		"div"
	);
	assert!(screen.try_get_by_role(Role::Textbox, "Password").is_err());
	assert_eq!(screen.get_by_label("Password").tag_name(), "input");
	assert_eq!(screen.get_by_role(Role::Button, "Save").tag_name(), "input");
}

#[tokio::test]
async fn settle_runs_use_resource_on_native() {
	let screen = render(index_job_component);

	screen.settle().await;

	assert!(screen.query_by_text("Index job").is_some());
}

#[tokio::test]
async fn click_action_settles_to_success() {
	let screen = render(save_component);

	screen.get_by_role(Role::Button, "Save").click();
	screen.settle().await;

	assert!(screen.query_by_text("Saved").is_some());
}

#[tokio::test]
async fn click_action_uses_own_screen_scheduler() {
	let first = render(save_component);
	let second = render(save_component);

	first.get_by_role(Role::Button, "Save").click();
	first.settle().await;

	assert!(first.query_by_text("Saved").is_some());
	assert!(second.query_by_text("Saved").is_none());
	assert!(second.query_by_text("Idle").is_some());
}

#[tokio::test]
async fn async_click_handler_settles_to_updated_ui() {
	let screen = render(async_click_component);

	screen.get_by_role(Role::Button, "Run").click();
	screen.settle().await;

	assert!(screen.query_by_text("Clicked").is_some());
}

#[tokio::test]
async fn settle_waits_for_timer_backed_tasks() {
	let screen = render(delayed_async_click_component);

	screen.get_by_role(Role::Button, "Run").click();
	screen.settle().await;

	assert!(screen.query_by_text("Delayed").is_some());
}

#[tokio::test]
async fn settle_continues_when_rerender_mounts_async_work() {
	let scope = ReactiveScope::new();
	let (show_child, message) =
		scope.enter(|| (Signal::new(false), Signal::new("Idle".to_string())));
	let spawned = Rc::new(Cell::new(false));
	let screen = render({
		let show_child = show_child.clone();
		let message = message.clone();
		let spawned = Rc::clone(&spawned);
		move || {
			let show_child = show_child.clone();
			let message = message.clone();
			let spawned = Rc::clone(&spawned);
			Page::reactive(move || {
				if show_child.get() && !spawned.replace(true) {
					let spawned_message = message.clone();
					spawn_task(async move {
						spawned_message.set("Mounted work".to_string());
					});
				}
				text_page(message.get())
			})
		}
	});

	show_child.set(true);
	screen.settle().await;

	assert!(screen.query_by_text("Mounted work").is_some());
}

#[tokio::test]
async fn settle_preserves_tasks_spawned_by_polled_tasks() {
	let scope = ReactiveScope::new();
	let message = scope.enter(|| Signal::new("Idle".to_string()));
	let click_message = message.clone();
	let screen = render(move || {
		let message = message.clone();
		let click_message = click_message.clone();
		PageElement::new("div")
			.child(
				PageElement::new("button")
					.on(
						EventType::Click,
						async_handler(move |_| {
							let click_message = click_message.clone();
							async move {
								tokio::task::yield_now().await;
								spawn_task(async move {
									click_message.set("Nested".to_string());
								});
							}
						}),
					)
					.child("Run nested"),
			)
			.child(Page::reactive(move || text_page(message.get())))
			.into_page()
	});

	screen.get_by_role(Role::Button, "Run nested").click();
	screen.settle().await;

	assert!(screen.query_by_text("Nested").is_some());
}

#[tokio::test]
async fn disabled_controls_suppress_click_handlers() {
	let scope = ReactiveScope::new();
	let message = scope.enter(|| Signal::new("Idle".to_string()));
	let click_message = message.clone();
	let screen = render(move || {
		let message = message.clone();
		let click_message = click_message.clone();
		PageElement::new("div")
			.child(
				PageElement::new("button")
					.attr("disabled", "")
					.listener("click", move |_| click_message.set("Clicked".to_string()))
					.child("Save"),
			)
			.child(Page::reactive(move || text_page(message.get())))
			.into_page()
	});

	screen.get_by_role(Role::Button, "Save").click();
	screen.settle().await;

	assert!(screen.query_by_text("Idle").is_some());
	assert!(screen.query_by_text("Clicked").is_none());
}

#[tokio::test]
async fn click_events_bubble_from_descendant_handles() {
	let scope = ReactiveScope::new();
	let message = scope.enter(|| Signal::new("Idle".to_string()));
	let click_message = message.clone();
	let screen = render(move || {
		let message = message.clone();
		let click_message = click_message.clone();
		PageElement::new("div")
			.child(
				PageElement::new("button")
					.listener("click", move |_| click_message.set("Clicked".to_string()))
					.child(PageElement::new("span").child("Nested label")),
			)
			.child(Page::reactive(move || text_page(message.get())))
			.into_page()
	});

	screen.get_by_text("Nested label").click();
	screen.settle().await;

	assert!(screen.query_by_text("Clicked").is_some());
}

#[test]
fn click_events_invoke_each_handler_in_bubbling_path() {
	let calls = Rc::new(RefCell::new(Vec::new()));
	let outer_calls = Rc::clone(&calls);
	let button_calls = Rc::clone(&calls);
	let screen = render(
		PageElement::new("div")
			.listener("click", move |_| outer_calls.borrow_mut().push("outer"))
			.child(
				PageElement::new("button")
					.listener("click", move |_| button_calls.borrow_mut().push("button"))
					.child(
						PageElement::new("span")
							.attr("role", "status")
							.attr("aria-label", "Nested status")
							.child("Nested label"),
					),
			),
	);

	screen.get_by_role(Role::Status, "Nested status").click();

	assert_eq!(calls.borrow().as_slice(), ["button", "outer"]);
}

#[test]
fn submit_helper_dispatches_submit_event() {
	let submitted = Rc::new(Cell::new(false));
	let submitted_for_handler = Rc::clone(&submitted);
	let screen = render(
		PageElement::new("form")
			.attr("aria-label", "Job form")
			.listener("submit", move |_| submitted_for_handler.set(true))
			.child(PageElement::new("input").attr("name", "job")),
	);

	screen.get_by_role(Role::Form, "Job form").submit();

	assert!(submitted.get());
}

#[rstest]
fn typed_page_handler_preserves_originating_and_listener_targets() {
	let observed = Arc::new(Mutex::new(None));
	let screen = render(page!(|observed: Arc<Mutex<Option<(String, String)>>>| {
		button {
			aria_label: "Parent button",
			@click: move |event: ClickEvent| {
				let target = event.target().expect("click target should exist");
				let current_target = event
					.current_target()
					.expect("click current target should exist");
				*observed.lock().unwrap() = Some((
					target.tag_name().to_string(),
					current_target.tag_name().to_string(),
				));
			},
			span { "Nested target" }
		}
	})(Arc::clone(&observed)));

	screen.get_by_text("Nested target").click();

	assert_eq!(
		observed.lock().unwrap().as_ref(),
		Some(&("span".to_string(), "button".to_string()))
	);
}

#[rstest]
fn non_bubbling_fixture_does_not_reach_an_ancestor_listener() {
	let called = Arc::new(AtomicBool::new(false));
	let called_for_handler = Arc::clone(&called);
	let screen = render(
		PageElement::new("div")
			.on(
				FocusEvent::EVENT,
				typed_event_handler::<FocusEvent, _>(move |_event: FocusEvent| {
					called_for_handler.store(true, Ordering::SeqCst);
				}),
			)
			.child(PageElement::new("span").child("Focusable child")),
	);

	let result = screen
		.get_by_text("Focusable child")
		.dispatch(EventFixture::new(EventType::Focus));

	assert_eq!(result, Err(EventError::MissingHandler));
	assert!(!called.load(Ordering::SeqCst));
}

#[rstest]
fn stop_propagation_keeps_same_target_handlers_and_skips_ancestors() {
	let calls = Arc::new(Mutex::new(Vec::new()));
	let outer_calls = Arc::clone(&calls);
	let first_calls = Arc::clone(&calls);
	let second_calls = Arc::clone(&calls);
	let screen = render(
		PageElement::new("div")
			.on(
				ClickEvent::EVENT,
				typed_event_handler::<ClickEvent, _>(move |_event: ClickEvent| {
					outer_calls.lock().unwrap().push("outer");
				}),
			)
			.child(
				PageElement::new("button")
					.on(
						ClickEvent::EVENT,
						typed_event_handler::<ClickEvent, _>(move |event: ClickEvent| {
							first_calls.lock().unwrap().push("first");
							event.stop_propagation();
						}),
					)
					.on(
						ClickEvent::EVENT,
						typed_event_handler::<ClickEvent, _>(move |_event: ClickEvent| {
							second_calls.lock().unwrap().push("second");
						}),
					)
					.child("Stop propagation"),
			),
	);

	screen.get_by_role(Role::Button, "Stop propagation").click();

	assert_eq!(calls.lock().unwrap().as_slice(), ["first", "second"]);
}

#[rstest]
fn stop_immediate_propagation_skips_same_target_and_ancestor_handlers() {
	let calls = Arc::new(Mutex::new(Vec::new()));
	let outer_calls = Arc::clone(&calls);
	let first_calls = Arc::clone(&calls);
	let second_calls = Arc::clone(&calls);
	let screen = render(
		PageElement::new("div")
			.on(
				ClickEvent::EVENT,
				typed_event_handler::<ClickEvent, _>(move |_event: ClickEvent| {
					outer_calls.lock().unwrap().push("outer");
				}),
			)
			.child(
				PageElement::new("button")
					.on(
						ClickEvent::EVENT,
						typed_event_handler::<ClickEvent, _>(move |event: ClickEvent| {
							first_calls.lock().unwrap().push("first");
							event.stop_immediate_propagation();
						}),
					)
					.on(
						ClickEvent::EVENT,
						typed_event_handler::<ClickEvent, _>(move |_event: ClickEvent| {
							second_calls.lock().unwrap().push("second");
						}),
					)
					.child("Stop immediately"),
			),
	);

	screen.get_by_role(Role::Button, "Stop immediately").click();

	assert_eq!(calls.lock().unwrap().as_slice(), ["first"]);
}

#[rstest]
fn prevent_default_state_is_shared_with_ancestor_listeners() {
	let default_prevented = Arc::new(AtomicBool::new(false));
	let default_prevented_for_handler = Arc::clone(&default_prevented);
	let screen = render(
		PageElement::new("div")
			.on(
				ClickEvent::EVENT,
				typed_event_handler::<ClickEvent, _>(move |event: ClickEvent| {
					default_prevented_for_handler
						.store(event.default_prevented(), Ordering::SeqCst);
				}),
			)
			.child(
				PageElement::new("button")
					.on(
						ClickEvent::EVENT,
						typed_event_handler::<ClickEvent, _>(move |event: ClickEvent| {
							event.prevent_default();
						}),
					)
					.child("Prevent default"),
			),
	);

	screen.get_by_role(Role::Button, "Prevent default").click();

	assert!(default_prevented.load(Ordering::SeqCst));
}

#[rstest]
#[tokio::test]
#[serial(component_typed_async)]
async fn typed_async_page_handler_settles_deterministically() {
	TYPED_ASYNC_CALLED.store(false, Ordering::SeqCst);
	let _reset = AtomicReset(&TYPED_ASYNC_CALLED);
	let screen = render(page!(|| {
		button {
			@click: async |event: ClickEvent| {
				crate::record_typed_async_click(event).await;
			},
			"Run typed async"
		}
	}));

	screen.get_by_role(Role::Button, "Run typed async").click();
	screen.settle().await;

	assert!(TYPED_ASYNC_CALLED.load(Ordering::SeqCst));
}

#[rstest]
#[tokio::test]
async fn typed_sync_page_handler_rerenders_after_settle() {
	let scope = ReactiveScope::new();
	let message = scope.enter(|| Signal::new("Idle".to_string()));
	let handler_message = message.clone();
	let rendered_message = message.clone();
	let screen = render(
		page!(|handler_message: Signal<String>, rendered_message: Signal<String>| {
			div {
				button {
					@click: move |event: ClickEvent| {
						assert_eq!(event.event_type(), "click");
						handler_message.set("Typed sync complete".to_string());
					},
					"Run typed sync"
				}
				p { { rendered_message.get() } }
			}
		})(handler_message, rendered_message),
	);

	screen.get_by_role(Role::Button, "Run typed sync").click();
	screen.settle().await;

	assert!(screen.query_by_text("Typed sync complete").is_some());
}

#[tokio::test]
async fn parent_rerender_skips_removed_child_anchors() {
	let scope = ReactiveScope::new();
	let show_child = scope.enter(|| Signal::new(true));
	let child_renders = Rc::new(Cell::new(0));
	let screen = {
		let show_child = show_child.clone();
		let child_renders = Rc::clone(&child_renders);
		render(move || {
			let show_child = show_child.clone();
			let child_renders = Rc::clone(&child_renders);
			Page::reactive(move || {
				if show_child.get() {
					let child_renders = Rc::clone(&child_renders);
					Page::reactive(move || {
						child_renders.set(child_renders.get() + 1);
						text_page("Child")
					})
				} else {
					text_page("Gone")
				}
			})
		})
	};

	assert_eq!(child_renders.get(), 1);
	show_child.set(false);
	screen.settle().await;

	assert_eq!(child_renders.get(), 1);
	assert!(screen.query_by_text("Gone").is_some());
	assert!(screen.query_by_text("Child").is_none());
}

#[tokio::test]
async fn find_by_text_waits_for_resource() {
	let screen = render(ready_component);

	let element = screen.find_by_text("Ready").await;

	assert_eq!(element.text(), "Ready");
}

#[tokio::test]
async fn find_by_text_rerenders_completed_work_with_pending_tasks() {
	let screen = render(mixed_resource_component);

	let element = screen.find_by_text("Ready").await;

	assert_eq!(element.text(), "Ready");
}

#[test]
fn pretty_dom_snapshot_is_stable() {
	let screen = render(page!(|| {
		main {
			aria_label: "Jobs",
			button { "Refresh" }
		}
	}));

	insta::assert_snapshot!(screen.pretty());
}

#[cfg(feature = "msw")]
#[server_fn]
async fn load_jobs() -> Result<Vec<String>, ServerFnError> {
	Ok(vec!["real job".to_string()])
}

#[cfg(feature = "msw")]
#[derive(Clone)]
struct JobsDatabase;

#[cfg(feature = "msw")]
struct JobsDatabaseKey;

#[cfg(feature = "msw")]
impl reinhardt_di::InjectableKey for JobsDatabaseKey {}

#[cfg(feature = "msw")]
#[server_fn]
async fn load_injected_jobs(
	#[inject] _database: reinhardt_di::KeyedDepends<JobsDatabaseKey, JobsDatabase>,
) -> Result<Vec<String>, ServerFnError> {
	Ok(vec!["real injected job".to_string()])
}

#[cfg(feature = "msw")]
type JobsResult<T> = Result<T, ServerFnError>;

#[cfg(feature = "msw")]
#[server_fn]
async fn load_injected_alias_jobs(
	#[inject] _database: reinhardt_di::KeyedDepends<JobsDatabaseKey, JobsDatabase>,
) -> JobsResult<Vec<String>> {
	Ok(vec!["real injected alias job".to_string()])
}

#[cfg(feature = "msw")]
fn jobs_resource_page(state: ResourceState<Vec<String>, ServerFnError>) -> Page {
	match state {
		ResourceState::Loading => text_page("Loading"),
		ResourceState::Success(values) => text_page(values.join(", ")),
		ResourceState::Error(error) => text_page(error.to_string()),
	}
}

#[cfg(feature = "msw")]
fn jobs_component() -> Page {
	let jobs = use_resource(|| async { load_jobs().await }, deps![]);
	Page::reactive(move || jobs_resource_page(jobs.get()))
}

#[cfg(feature = "msw")]
fn jobs_query_component() -> Page {
	let jobs = use_query(load_jobs::query(), QueryOptions::default());
	let refetch_jobs = jobs.clone();
	PageElement::new("div")
		.child(
			PageElement::new("button")
				.listener("click", move |_| refetch_jobs.refetch())
				.child("Refresh"),
		)
		.child(Page::reactive(move || jobs_resource_page(jobs.get())))
		.into_page()
}

#[cfg(feature = "msw")]
fn injected_jobs_query_component() -> Page {
	let jobs = use_query(load_injected_jobs::query(), QueryOptions::default());
	Page::reactive(move || jobs_resource_page(jobs.get()))
}

#[cfg(feature = "msw")]
fn injected_alias_jobs_query_component() -> Page {
	let jobs = use_query(load_injected_alias_jobs::query(), QueryOptions::default());
	Page::reactive(move || jobs_resource_page(jobs.get()))
}

#[cfg(feature = "msw")]
#[tokio::test]
async fn server_fn_mock_feeds_resource() {
	let screen = render(jobs_component);
	screen.mock_server_fn::<load_jobs::marker>(|_args| Ok(vec!["Index job".to_string()]));

	screen.settle().await;

	assert!(screen.query_by_text("Index job").is_some());
	assert_eq!(screen.calls_to_server_fn::<load_jobs::marker>().len(), 1);
}

#[cfg(feature = "msw")]
#[tokio::test]
async fn server_fn_mocks_are_scoped_per_screen() {
	let first = render(jobs_component);
	first.mock_server_fn::<load_jobs::marker>(|_args| Ok(vec!["First job".to_string()]));
	let second = render(jobs_component);
	second.mock_server_fn::<load_jobs::marker>(|_args| Ok(vec!["Second job".to_string()]));

	first.settle().await;
	second.settle().await;

	assert!(first.query_by_text("First job").is_some());
	assert!(first.query_by_text("Second job").is_none());
	assert!(second.query_by_text("Second job").is_some());
	assert!(second.query_by_text("First job").is_none());
}

#[cfg(feature = "msw")]
#[tokio::test]
async fn server_fn_query_cache_is_scoped_per_screen() {
	let first = render(jobs_query_component);
	first.mock_server_fn::<load_jobs::marker>(|_args| Ok(vec!["First job".to_string()]));
	let second = render(jobs_query_component);
	second.mock_server_fn::<load_jobs::marker>(|_args| Ok(vec!["Second job".to_string()]));

	first.get_by_role(Role::Button, "Refresh").click();
	second.get_by_role(Role::Button, "Refresh").click();
	first.settle().await;
	second.settle().await;

	assert!(first.query_by_text("First job").is_some());
	assert!(first.query_by_text("Second job").is_none());
	assert!(second.query_by_text("Second job").is_some());
	assert!(second.query_by_text("First job").is_none());
}

#[cfg(feature = "msw")]
#[tokio::test]
async fn server_fn_query_cache_does_not_leak_after_screen_drop() {
	{
		let first = render(jobs_query_component);
		first.mock_server_fn::<load_jobs::marker>(|_args| Ok(vec!["First job".to_string()]));

		first.get_by_role(Role::Button, "Refresh").click();
		first.settle().await;

		assert!(first.query_by_text("First job").is_some());
		assert_eq!(first.calls_to_server_fn::<load_jobs::marker>().len(), 2);
	}

	let second = render(jobs_query_component);
	second.mock_server_fn::<load_jobs::marker>(|_args| Ok(vec!["Second job".to_string()]));

	second.get_by_role(Role::Button, "Refresh").click();
	second.settle().await;

	assert!(second.query_by_text("Second job").is_some());
	assert!(second.query_by_text("First job").is_none());
	assert_eq!(second.calls_to_server_fn::<load_jobs::marker>().len(), 2);
}

#[cfg(feature = "msw")]
#[rstest]
#[tokio::test]
async fn injected_server_fn_query_mock_errors_render_query_errors() {
	// Arrange
	let screen = render(injected_jobs_query_component);
	screen.mock_server_fn::<load_injected_jobs::marker>(|_args| {
		Err(ServerFnError::application("injected query failed"))
	});

	// Act
	screen.settle().await;

	// Assert
	screen.get_by_text("injected query failed");
	assert!(
		screen
			.query_by_text("Application error: injected query failed")
			.is_none()
	);
	assert_eq!(
		screen
			.calls_to_server_fn::<load_injected_jobs::marker>()
			.len(),
		1
	);
}

#[cfg(feature = "msw")]
#[tokio::test]
async fn injected_alias_server_fn_query_uses_native_mock() {
	// Arrange
	let screen = render(injected_alias_jobs_query_component);
	screen.mock_server_fn::<load_injected_alias_jobs::marker>(|_args| {
		Ok(vec!["Injected alias job".to_string()])
	});

	// Act
	screen.settle().await;

	// Assert
	assert!(screen.query_by_text("Injected alias job").is_some());
	assert_eq!(
		screen
			.calls_to_server_fn::<load_injected_alias_jobs::marker>()
			.len(),
		1
	);
}

#[cfg(feature = "msw")]
#[rstest]
#[tokio::test]
async fn injected_server_fn_query_without_mock_renders_query_error() {
	// Arrange
	let screen = render(injected_jobs_query_component);

	// Act
	screen.settle().await;

	// Assert
	screen.get_by_text("no mock registered for active server function");
	assert!(
		screen
			.query_by_text("Application error: no mock registered for active server function")
			.is_none()
	);
	assert_eq!(
		screen
			.calls_to_server_fn::<load_injected_jobs::marker>()
			.len(),
		1
	);
}

#[cfg(feature = "msw")]
#[tokio::test]
async fn server_fn_mock_errors_render_resource_errors() {
	let screen = render(jobs_component);
	screen.mock_server_fn::<load_jobs::marker>(|_args| {
		Err(ServerFnError::application("mock failed"))
	});

	screen.settle().await;

	screen.get_by_text("mock failed");
	assert!(
		screen
			.query_by_text("Application error: mock failed")
			.is_none()
	);
	assert_eq!(screen.calls_to_server_fn::<load_jobs::marker>().len(), 1);
}

#[cfg(feature = "msw")]
#[tokio::test]
async fn active_server_fn_mock_scope_requires_registered_handler() {
	let screen = render(jobs_component);

	screen.settle().await;

	screen.get_by_text("no mock registered for active server function");
	assert!(
		screen
			.query_by_text("Application error: no mock registered for active server function")
			.is_none()
	);
	assert_eq!(screen.calls_to_server_fn::<load_jobs::marker>().len(), 1);
}

#[cfg(feature = "msw")]
#[tokio::test]
async fn server_fn_mock_handler_can_inspect_recorded_calls() {
	let screen = render(jobs_component);
	let screen_for_handler = screen.clone();
	screen.mock_server_fn::<load_jobs::marker>(move |_args| {
		assert_eq!(
			screen_for_handler
				.calls_to_server_fn::<load_jobs::marker>()
				.len(),
			1
		);
		Ok(vec!["Inspectable job".to_string()])
	});

	screen.settle().await;

	assert!(screen.query_by_text("Inspectable job").is_some());
}
