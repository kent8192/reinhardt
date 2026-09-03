#![cfg(all(native, feature = "testing"))]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::{cell::Cell, rc::Rc};

use reinhardt_pages::component::{ControlBinding, ControlBindingError, NumberParseErrorKind};
use reinhardt_pages::event::{
	ChangeEvent, CompositionEndEvent, CompositionStartEvent, EventPayload, typed_event_handler,
};
use reinhardt_pages::prelude::spawn_task;
use reinhardt_pages::reactive::hooks::use_layout_effect;
use reinhardt_pages::reactive::{ReactiveScope, Signal};
use reinhardt_pages::testing::component::{EventError, EventFixture, render};
use reinhardt_pages::{IntoPage, Page, PageElement, deps, page};
use rstest::{fixture, rstest};
use serial_test::serial;

#[fixture]
fn reactive_scope() -> ReactiveScope {
	ReactiveScope::new()
}

fn signal_in_scope<T: 'static>(scope: &ReactiveScope, value: T) -> Signal<T> {
	scope.enter(|| Signal::new(value))
}

#[rstest]
#[tokio::test]
async fn bound_input_updates_before_explicit_handler(reactive_scope: ReactiveScope) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, "old".to_owned());
	let observed = Arc::new(Mutex::new(String::new()));
	let observed_handler = Arc::clone(&observed);
	let value_handler = value.clone();
	let screen = render(page!({
		input {
			aria_label: "Name",
			bind: value,
			@input: move |_| *observed_handler.lock().unwrap() = value_handler.get(),
		}
	}));
	let input = screen.get_by_label("Name");

	// Act
	input.input("new");
	screen.settle().await;

	// Assert
	assert_eq!(value.get(), "new");
	assert_eq!(*observed.lock().unwrap(), "new");
	assert_eq!(input.value().as_deref(), Some("new"));
}

#[rstest]
fn native_bound_textarea_omits_suppressed_children(reactive_scope: ReactiveScope) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, "current".to_owned());

	// Act
	let screen = render(
		PageElement::new("textarea")
			.attr("aria-label", "Description")
			.control_binding(ControlBinding::text(value))
			.child("stale child"),
	);

	// Assert
	assert_eq!(
		screen.get_by_label("Description").value().as_deref(),
		Some("current")
	);
	assert!(screen.try_get_by_text("stale child").is_err());
}

#[rstest]
fn native_select_treats_falsy_multiple_as_single_select(reactive_scope: ReactiveScope) {
	// Arrange
	let selected = signal_in_scope(&reactive_scope, "rust".to_owned());

	// Act
	let screen = render(
		PageElement::new("select")
			.attr("aria-label", "Language")
			.attr("multiple", "false")
			.control_binding(ControlBinding::select_one(selected))
			.child(
				PageElement::new("option")
					.attr("value", "rust")
					.child("Rust"),
			),
	);

	// Assert
	assert_eq!(
		screen.get_by_label("Language").value().as_deref(),
		Some("rust")
	);
}

#[rstest]
#[case("false")]
#[case("0")]
fn native_select_with_falsy_multiple_projects_only_the_first_duplicate(
	reactive_scope: ReactiveScope,
	#[case] multiple: &str,
) {
	// Arrange
	let selected = signal_in_scope(&reactive_scope, "rust".to_owned());

	// Act
	let screen = render(
		PageElement::new("select")
			.attr("aria-label", "Language")
			.attr("multiple", multiple.to_owned())
			.control_binding(ControlBinding::select_one(selected))
			.child(
				PageElement::new("option")
					.attr("value", "rust")
					.child("First"),
			)
			.child(
				PageElement::new("option")
					.attr("value", "rust")
					.child("Second"),
			),
	);

	// Assert
	assert_eq!(
		screen
			.pretty()
			.match_indices("selected=\"selected\"")
			.count(),
		1
	);
}

#[rstest]
#[serial(controlled_binding_effect)]
fn binding_write_layout_effect_can_read_the_same_screen(reactive_scope: ReactiveScope) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, "old".to_owned());
	let screen = render(page!({
		input {
			aria_label: "Name",
			bind: value,
		}
	}));
	let input = screen.get_by_label("Name");
	let observed = Arc::new(Mutex::new(Vec::new()));
	let effect_input = input.clone();
	let effect_value = value.clone();
	let effect_observed = Arc::clone(&observed);
	let _effect = reactive_scope.enter(|| {
		use_layout_effect(
			move || {
				effect_observed
					.lock()
					.unwrap()
					.push((effect_value.get(), effect_input.value()));
				None::<fn()>
			},
			deps![value.clone()],
		)
	});

	// Act
	input.input("new");

	// Assert
	assert_eq!(
		*observed.lock().unwrap(),
		vec![
			("old".to_owned(), Some("old".to_owned())),
			("new".to_owned(), Some("new".to_owned())),
		]
	);
}

#[rstest]
#[serial(controlled_binding_effect)]
#[tokio::test]
async fn binding_write_layout_effect_spawns_on_the_screen_scheduler(reactive_scope: ReactiveScope) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, "old".to_owned());
	let screen = render(page!({
		input {
			aria_label: "Name",
			bind: value,
		}
	}));
	let input = screen.get_by_label("Name");
	let completed = Arc::new(AtomicBool::new(false));
	let effect_value = value.clone();
	let effect_completed = Arc::clone(&completed);
	let _effect = reactive_scope.enter(|| {
		use_layout_effect(
			move || {
				if effect_value.get() == "new" {
					let completed = Arc::clone(&effect_completed);
					spawn_task(async move {
						completed.store(true, Ordering::SeqCst);
					});
				}
				None::<fn()>
			},
			deps![value.clone()],
		)
	});

	// Act
	input.input("new");
	screen.settle().await;

	// Assert
	assert!(completed.load(Ordering::SeqCst));
}

#[rstest]
#[tokio::test]
async fn checkbox_binding_tracks_fixture_and_external_signal_state(reactive_scope: ReactiveScope) {
	// Arrange
	let checked = signal_in_scope(&reactive_scope, false);
	let observed = Arc::new(Mutex::new(None));
	let observed_handler = Arc::clone(&observed);
	let screen = render(page!({
		input {
			aria_label: "Enabled",
			type: "checkbox",
			bind: checked,
			@change: move |event: ChangeEvent| {
				*observed_handler.lock().unwrap() = Some(event.checked());
			},
		}
	}));
	let checkbox = screen.get_by_label("Enabled");

	// Act
	checkbox.change_checked(true);
	checked.set(false);
	screen.settle().await;
	checkbox
		.dispatch(EventFixture::change())
		.expect("refreshed checkbox should dispatch");

	// Assert
	assert!(!checked.get());
	assert_eq!(
		observed.lock().unwrap().as_ref(),
		Some(&Ok::<bool, reinhardt_pages::event::EventTargetError>(false))
	);
}

#[rstest]
#[tokio::test]
async fn radio_binding_writes_only_the_checked_choice_and_refreshes_comparison(
	reactive_scope: ReactiveScope,
) {
	// Arrange
	let selected = signal_in_scope(&reactive_scope, "draft".to_owned());
	let screen = render(page!({
		input {
			aria_label: "Draft",
			type: "radio",
			value: "draft",
			bind: selected,
		}
		input {
			aria_label: "Published",
			type: "radio",
			value: "published",
			bind: selected,
		}
	}));
	let published = screen.get_by_label("Published");

	// Act
	published.change_checked(true);
	let selected_after_checked = selected.get();
	selected.set("draft".to_owned());
	screen.settle().await;
	published
		.dispatch(EventFixture::change())
		.expect("unchecked radio binding should ignore the change");

	// Assert
	assert_eq!(selected_after_checked, "published");
	assert_eq!(selected.get(), "draft");
}

#[rstest]
fn uppercase_radio_updates_binding_and_projects_checked_event_state(reactive_scope: ReactiveScope) {
	// Arrange
	let selected = signal_in_scope(&reactive_scope, "draft".to_owned());
	let observed = Arc::new(Mutex::new(None));
	let observed_handler = Arc::clone(&observed);
	let screen = render(
		PageElement::new("INPUT")
			.attr("aria-label", "Published uppercase")
			.attr("type", "RADIO")
			.attr("value", "published")
			.control_binding(ControlBinding::radio(
				selected.clone(),
				"published".to_owned(),
			))
			.on(
				ChangeEvent::EVENT,
				typed_event_handler::<ChangeEvent, _>(move |event: ChangeEvent| {
					*observed_handler.lock().unwrap() = Some(event.checked());
				}),
			),
	);
	let radio = screen.get_by_label("Published uppercase");

	// Act
	radio
		.try_change_checked(true)
		.expect("uppercase radio should accept checked target state");

	// Assert
	assert_eq!(selected.get(), "published");
	assert_eq!(
		observed.lock().unwrap().as_ref(),
		Some(&Ok::<bool, reinhardt_pages::event::EventTargetError>(true))
	);
}

#[rstest]
#[test]
fn radio_binding_evaluates_dynamic_value_once(reactive_scope: ReactiveScope) {
	// Arrange
	let selected = signal_in_scope(&reactive_scope, "first".to_owned());
	let evaluations = Rc::new(Cell::new(0));
	let value_evaluations = Rc::clone(&evaluations);

	// Act
	let screen = render(page!({
		input {
			aria_label: "Choice",
			type: "radio",
			value: {
				let count = value_evaluations.get() + 1;
				value_evaluations.set(count);
				if count == 1 { "first" } else { "second" }
			},
			bind: selected,
		}
	}));
	let input = screen.get_by_label("Choice");
	selected.set("other".to_owned());
	input.change_checked(true);

	// Assert
	assert_eq!(evaluations.get(), 1);
	assert_eq!(input.value().as_deref(), Some("first"));
	assert_eq!(selected.get(), "first");
}

#[rstest]
#[tokio::test]
async fn invalid_number_raw_survives_settle_until_the_value_signal_changes(
	reactive_scope: ReactiveScope,
) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, 7_i32);
	let error = signal_in_scope(&reactive_scope, None);
	let screen = render(page!({
		input {
			aria_label: "Quantity",
			type: "number",
			bind: number(value, error),
		}
	}));
	let input = screen.get_by_label("Quantity");

	// Act
	input.input("-");
	screen.settle().await;
	let retained_raw = input.value();
	value.set(12);
	screen.settle().await;

	// Assert
	assert_eq!(value.get(), 12);
	assert_eq!(retained_raw.as_deref(), Some("-"));
	assert_eq!(input.value().as_deref(), Some("12"));
	let parse_error = error.get().expect("invalid number should set an error");
	assert_eq!(parse_error.raw(), "-");
	assert_eq!(parse_error.kind(), NumberParseErrorKind::Incomplete);
}

#[rstest]
#[tokio::test]
async fn invalid_number_raw_is_cleared_by_an_explicit_same_value_write(
	reactive_scope: ReactiveScope,
) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, 7_i32);
	let screen = render(page!({
		input {
			aria_label: "Quantity",
			type: "number",
			bind: value,
		}
	}));
	let input = screen.get_by_label("Quantity");

	// Act
	input.input("-");
	value.set(7);
	screen.settle().await;

	// Assert
	assert_eq!(input.value().as_deref(), Some("7"));
}

#[rstest]
#[tokio::test]
async fn select_one_binding_tracks_selected_value_in_both_directions(
	reactive_scope: ReactiveScope,
) {
	// Arrange
	let selected = signal_in_scope(&reactive_scope, "rust".to_owned());
	let observed = Arc::new(Mutex::new(None));
	let observed_handler = Arc::clone(&observed);
	let screen = render(page!({
		select {
			aria_label: "Language",
			bind: selected,
			@change: move |event: ChangeEvent| {
				*observed_handler.lock().unwrap() = Some(event.selected_values());
			},
			option {
				value: "rust",
				"Rust"
			}
			option {
				value: "wasm",
				"WebAssembly"
			}
		}
	}));
	let select = screen.get_by_label("Language");

	// Act
	select
		.dispatch(EventFixture::change().selected_values(["wasm"]))
		.expect("select-one fixture should dispatch");
	let wasm_dom = screen.pretty();
	selected.set("rust".to_owned());
	screen.settle().await;
	select
		.dispatch(EventFixture::change())
		.expect("refreshed select-one should dispatch");

	// Assert
	assert_eq!(selected.get(), "rust");
	assert_eq!(select.value().as_deref(), Some("rust"));
	assert_eq!(
		wasm_dom,
		concat!(
			"<select aria-label=\"Language\">\n",
			"  <option value=\"rust\">\n",
			"    Rust\n",
			"  </option>\n",
			"  <option value=\"wasm\" selected=\"selected\">\n",
			"    WebAssembly\n",
			"  </option>\n",
			"</select>\n",
		)
	);
	assert_eq!(
		screen.pretty(),
		concat!(
			"<select aria-label=\"Language\">\n",
			"  <option value=\"rust\" selected=\"selected\">\n",
			"    Rust\n",
			"  </option>\n",
			"  <option value=\"wasm\">\n",
			"    WebAssembly\n",
			"  </option>\n",
			"</select>\n",
		)
	);
	assert_eq!(
		observed.lock().unwrap().as_ref(),
		Some(&Ok::<Vec<String>, reinhardt_pages::event::EventTargetError>(vec!["rust".to_owned()]))
	);
}

#[rstest]
#[tokio::test]
async fn uppercase_select_updates_binding_event_state_and_option_projection(
	reactive_scope: ReactiveScope,
) {
	// Arrange
	let selected = signal_in_scope(&reactive_scope, "rust".to_owned());
	let observed = Arc::new(Mutex::new(Vec::new()));
	let observed_handler = Arc::clone(&observed);
	let screen = render(
		PageElement::new("SELECT")
			.attr("aria-label", "Uppercase language")
			.control_binding(ControlBinding::select_one(selected.clone()))
			.on(
				ChangeEvent::EVENT,
				typed_event_handler::<ChangeEvent, _>(move |event: ChangeEvent| {
					observed_handler
						.lock()
						.unwrap()
						.push(event.selected_values());
				}),
			)
			.child(
				PageElement::new("OPTION")
					.attr("value", "rust")
					.child("Rust"),
			)
			.child(
				PageElement::new("OPTION")
					.attr("value", "wasm")
					.child("WebAssembly"),
			),
	);
	let select = screen.get_by_label("Uppercase language");

	// Act
	select
		.dispatch(EventFixture::change().selected_values(["wasm"]))
		.expect("uppercase select should accept selected target state");
	let wasm_dom = screen.pretty();
	selected.set("rust".to_owned());
	screen.settle().await;
	select
		.dispatch(EventFixture::change())
		.expect("uppercase select should project refreshed selected state");

	// Assert
	assert_eq!(selected.get(), "rust");
	assert_eq!(select.value().as_deref(), Some("rust"));
	assert_eq!(
		wasm_dom,
		concat!(
			"<SELECT aria-label=\"Uppercase language\">\n",
			"  <OPTION value=\"rust\">\n",
			"    Rust\n",
			"  </OPTION>\n",
			"  <OPTION value=\"wasm\" selected=\"selected\">\n",
			"    WebAssembly\n",
			"  </OPTION>\n",
			"</SELECT>\n",
		)
	);
	assert_eq!(
		screen.pretty(),
		concat!(
			"<SELECT aria-label=\"Uppercase language\">\n",
			"  <OPTION value=\"rust\" selected=\"selected\">\n",
			"    Rust\n",
			"  </OPTION>\n",
			"  <OPTION value=\"wasm\">\n",
			"    WebAssembly\n",
			"  </OPTION>\n",
			"</SELECT>\n",
		)
	);
	assert_eq!(
		*observed.lock().unwrap(),
		vec![
			Ok::<Vec<String>, reinhardt_pages::event::EventTargetError>(vec!["wasm".to_owned()]),
			Ok::<Vec<String>, reinhardt_pages::event::EventTargetError>(vec!["rust".to_owned()]),
		]
	);
}

#[rstest]
#[tokio::test]
async fn select_one_projection_uses_first_matching_option_and_ignores_absent_values(
	reactive_scope: ReactiveScope,
) {
	// Arrange
	let selected = signal_in_scope(&reactive_scope, "duplicate".to_owned());
	let screen = render(page!({
		select {
			aria_label: "Duplicate",
			bind: selected,
			option {
				value: "duplicate",
				"First"
			}
			option {
				value: "duplicate",
				"Second"
			}
		}
	}));
	let select = screen.get_by_label("Duplicate");

	// Act
	let duplicate_projection = screen.pretty();
	selected.set("absent".to_owned());
	screen.settle().await;

	// Assert
	assert_eq!(select.value().as_deref(), Some(""));
	assert_eq!(
		duplicate_projection,
		concat!(
			"<select aria-label=\"Duplicate\">\n",
			"  <option value=\"duplicate\" selected=\"selected\">\n",
			"    First\n",
			"  </option>\n",
			"  <option value=\"duplicate\">\n",
			"    Second\n",
			"  </option>\n",
			"</select>\n",
		)
	);
	assert!(!screen.pretty().contains("selected=\"selected\""));
}

#[rstest]
fn select_many_projection_uses_option_dom_order_and_preserves_duplicates(
	reactive_scope: ReactiveScope,
) {
	// Arrange
	let selected = signal_in_scope(
		&reactive_scope,
		vec![
			"missing".to_owned(),
			"second".to_owned(),
			"first".to_owned(),
		],
	);
	let screen = render(page!({
		select {
			aria_label: "Ordered",
			multiple: true,
			bind: selected,
			option {
				value: "first",
				"First"
			}
			option {
				value: "second",
				"Second A"
			}
			option {
				value: "second",
				"Second B"
			}
		}
	}));
	let select = screen.get_by_label("Ordered");

	// Act
	select
		.dispatch(EventFixture::change())
		.expect("normalized select should dispatch");

	// Assert
	assert_eq!(select.value().as_deref(), Some("first"));
	assert_eq!(
		selected.get(),
		vec!["first".to_owned(), "second".to_owned(), "second".to_owned(),]
	);
	assert_eq!(screen.pretty().matches("selected=\"selected\"").count(), 3);
}

#[rstest]
fn select_one_empty_selection_commits_the_browser_empty_value(reactive_scope: ReactiveScope) {
	// Arrange
	let selected = signal_in_scope(&reactive_scope, "rust".to_owned());
	let screen = render(page!({
		select {
			aria_label: "Language",
			bind: selected,
			option {
				value: "rust",
				"Rust"
			}
		}
	}));
	let select = screen.get_by_label("Language");

	// Act
	select
		.dispatch(EventFixture::change().selected_values(Vec::<String>::new()))
		.expect("empty select-one fixture should dispatch");

	// Assert
	assert_eq!(selected.get(), "");
	assert_eq!(select.value().as_deref(), Some(""));
}

#[rstest]
#[tokio::test]
async fn select_many_binding_tracks_all_selected_values_in_both_directions(
	reactive_scope: ReactiveScope,
) {
	// Arrange
	let selected = signal_in_scope(&reactive_scope, vec!["rust".to_owned()]);
	let observed = Arc::new(Mutex::new(None));
	let observed_handler = Arc::clone(&observed);
	let screen = render(page!({
		select {
			aria_label: "Targets",
			multiple: true,
			bind: selected,
			@change: move |event: ChangeEvent| {
				*observed_handler.lock().unwrap() = Some(event.selected_values());
			},
			option {
				value: "rust",
				"Rust"
			}
			option {
				value: "wasm",
				"WebAssembly"
			}
		}
	}));
	let select = screen.get_by_label("Targets");

	// Act
	select
		.dispatch(EventFixture::change().selected_values(["rust", "wasm"]))
		.expect("select-many fixture should dispatch");
	let both_selected_dom = screen.pretty();
	selected.set(vec!["wasm".to_owned()]);
	screen.settle().await;
	select
		.dispatch(EventFixture::change())
		.expect("refreshed select-many should dispatch");

	// Assert
	assert_eq!(selected.get(), vec!["wasm".to_owned()]);
	assert_eq!(select.value().as_deref(), Some("wasm"));
	assert_eq!(
		both_selected_dom,
		concat!(
			"<select aria-label=\"Targets\" multiple=\"multiple\">\n",
			"  <option value=\"rust\" selected=\"selected\">\n",
			"    Rust\n",
			"  </option>\n",
			"  <option value=\"wasm\" selected=\"selected\">\n",
			"    WebAssembly\n",
			"  </option>\n",
			"</select>\n",
		)
	);
	assert_eq!(
		screen.pretty(),
		concat!(
			"<select aria-label=\"Targets\" multiple=\"multiple\">\n",
			"  <option value=\"rust\">\n",
			"    Rust\n",
			"  </option>\n",
			"  <option value=\"wasm\" selected=\"selected\">\n",
			"    WebAssembly\n",
			"  </option>\n",
			"</select>\n",
		)
	);
	assert_eq!(
		observed.lock().unwrap().as_ref(),
		Some(&Ok::<Vec<String>, reinhardt_pages::event::EventTargetError>(vec!["wasm".to_owned()]))
	);
}

#[rstest]
fn select_binding_uses_flattened_option_text_when_value_is_omitted(reactive_scope: ReactiveScope) {
	// Arrange
	let selected = signal_in_scope(
		&reactive_scope,
		vec![
			"Rust ignored & WebAssembly".to_owned(),
			"Nested\u{a0}<Choice>ignored".to_owned(),
		],
	);
	let screen = render(
		PageElement::new("select")
			.attr("aria-label", "Targets")
			.bool_attr("multiple", true)
			.control_binding(ControlBinding::select_many(selected))
			.child(
				PageElement::new("optgroup")
					.child(
						PageElement::new("option")
							.child(" \tRust\n")
							.child(PageElement::new("script").child("ignored"))
							.child("  &\r\nWebAssembly\x0c "),
					)
					.child(PageElement::new("option").child(Page::Fragment(vec![
						Page::text(" Nested\u{a0}"),
						PageElement::new("span").child("<Choice>").into_page(),
						PageElement::new("script").child("ignored").into_page(),
						Page::text(" "),
					]))),
			),
	);

	// Act
	let html = screen.pretty();

	// Assert
	assert_eq!(
		html,
		concat!(
			"<select aria-label=\"Targets\" multiple=\"multiple\">\n",
			"  <optgroup>\n",
			"    <option selected=\"selected\">\n",
			"       \tRust\n\n",
			"      <span>\n",
			"        ignored\n",
			"      </span>\n",
			"        &\r\n",
			"WebAssembly\x0c \n",
			"    </option>\n",
			"    <option selected=\"selected\">\n",
			"       Nested\u{a0}\n",
			"      <span>\n",
			"        <Choice>\n",
			"      </span>\n",
			"      <span>\n",
			"        ignored\n",
			"      </span>\n",
			"       \n",
			"    </option>\n",
			"  </optgroup>\n",
			"</select>\n",
		)
	);
}

#[rstest]
fn inferred_option_value_uses_one_reactive_render(reactive_scope: ReactiveScope) {
	// Arrange
	let renders = Rc::new(Cell::new(0));
	let render_count = Rc::clone(&renders);
	let selected = signal_in_scope(&reactive_scope, "Static".to_owned());

	// Act
	let screen = render(
		PageElement::new("select")
			.attr("aria-label", "Target")
			.control_binding(ControlBinding::select_one(selected))
			.child(
				PageElement::new("option")
					.child("Static")
					.child(Page::reactive(move || {
						render_count.set(render_count.get() + 1);
						Page::text(" Dynamic")
					})),
			),
	);

	// Assert
	assert_eq!(renders.get(), 1);
	assert!(screen.pretty().contains("<option selected=\"selected\">"));
}

#[rstest]
#[tokio::test]
async fn composition_defers_writes_and_deduplicates_the_final_input(reactive_scope: ReactiveScope) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, "old".to_owned());
	let observed = Arc::new(Mutex::new(Vec::new()));
	let observed_input = Arc::clone(&observed);
	let input_value = value.clone();
	let observed_end = Arc::clone(&observed);
	let end_value = value.clone();
	let screen = render(page!({
		input {
			aria_label: "Name",
			bind: value,
			@input: move |_| observed_input.lock().unwrap().push(input_value.get()),
			@compositionend: move |_| {
				observed_end.lock().unwrap().push(end_value.get());
				end_value.set("after-end".to_owned());
			},
		}
	}));
	let input = screen.get_by_label("Name");

	// Act
	input
		.dispatch(EventFixture::new(CompositionStartEvent::EVENT))
		.expect("composition start should dispatch");
	input
		.dispatch(EventFixture::input().value("k").is_composing(true))
		.expect("first composing input should dispatch");
	input
		.dispatch(EventFixture::input().value("かな").is_composing(true))
		.expect("second composing input should dispatch");
	input
		.dispatch(EventFixture::new(CompositionEndEvent::EVENT).value("かな"))
		.expect("composition end should dispatch");
	input
		.dispatch(EventFixture::input().value("かな"))
		.expect("duplicate final input should dispatch");
	screen.settle().await;

	// Assert
	assert_eq!(
		*observed.lock().unwrap(),
		vec![
			"old".to_owned(),
			"old".to_owned(),
			"かな".to_owned(),
			"after-end".to_owned(),
		]
	);
	assert_eq!(value.get(), "after-end");
	assert_eq!(input.value().as_deref(), Some("after-end"));
}

#[rstest]
fn isolated_composing_input_skips_only_that_event(reactive_scope: ReactiveScope) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, "old".to_owned());
	let observed = Arc::new(Mutex::new(Vec::new()));
	let observed_input = Arc::clone(&observed);
	let input_value = value.clone();
	let screen = render(page!({
		input {
			aria_label: "Name",
			bind: value,
			@input: move |_| observed_input.lock().unwrap().push(input_value.get()),
		}
	}));
	let input = screen.get_by_label("Name");

	// Act
	input
		.dispatch(EventFixture::input().value("pending").is_composing(true))
		.expect("isolated composing input should dispatch");
	let value_after_composing_input = value.get();
	let raw_after_composing_input = input.value();
	input
		.dispatch(EventFixture::input().value("committed"))
		.expect("normal input after an isolated composing input should dispatch");

	// Assert
	assert_eq!(value_after_composing_input, "old");
	assert_eq!(raw_after_composing_input.as_deref(), Some("pending"));
	assert_eq!(value.get(), "committed");
	assert_eq!(
		*observed.lock().unwrap(),
		vec!["old".to_owned(), "committed".to_owned()]
	);
}

#[rstest]
fn isolated_composing_input_invalidates_stale_composition_dedupe(reactive_scope: ReactiveScope) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, "old".to_owned());
	let observed = Arc::new(Mutex::new(Vec::new()));
	let observed_input = Arc::clone(&observed);
	let input_value = value.clone();
	let end_value = value.clone();
	let screen = render(page!({
		input {
			aria_label: "Name",
			bind: value,
			@input: move |_| observed_input.lock().unwrap().push(input_value.get()),
			@compositionend: move |_| end_value.set("after-end".to_owned()),
		}
	}));
	let input = screen.get_by_label("Name");

	// Act
	input
		.dispatch(EventFixture::new(CompositionStartEvent::EVENT))
		.expect("composition start should dispatch");
	input
		.dispatch(EventFixture::new(CompositionEndEvent::EVENT).value("same"))
		.expect("composition end should dispatch");
	input
		.dispatch(EventFixture::input().value("same").is_composing(true))
		.expect("isolated composing input should dispatch");
	input
		.dispatch(EventFixture::input().value("same"))
		.expect("normal input should dispatch");

	// Assert
	assert_eq!(
		*observed.lock().unwrap(),
		vec!["after-end".to_owned(), "same".to_owned()]
	);
	assert_eq!(value.get(), "same");
}

#[rstest]
fn binding_failures_are_structured_event_errors(reactive_scope: ReactiveScope) {
	// Arrange
	let selected = signal_in_scope(&reactive_scope, Vec::<String>::new());
	let screen = render(
		PageElement::new("input")
			.attr("aria-label", "Invalid")
			.control_binding(ControlBinding::select_many(selected)),
	);
	let input = screen.get_by_label("Invalid");

	// Act
	let error = input
		.dispatch(EventFixture::change().value("unexpected"))
		.expect_err("invalid binding target should fail");

	// Assert
	assert_eq!(
		error,
		EventError::ControlBinding(ControlBindingError::UnsupportedElement {
			control: reinhardt_pages::component::ControlKind::SelectMany,
			actual_tag: "input".to_owned(),
		})
	);
}

#[rstest]
#[case("file")]
#[case("range")]
fn text_binding_rejects_unsupported_input_types(
	#[case] input_type: &str,
	reactive_scope: ReactiveScope,
) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, "bound".to_owned());
	let screen = render(
		PageElement::new("input")
			.attr("aria-label", "Invalid text target")
			.attr("type", input_type.to_owned())
			.control_binding(ControlBinding::text(value)),
	);
	let input = screen.get_by_label("Invalid text target");

	// Act
	let error = input
		.dispatch(EventFixture::input().value("edited"))
		.expect_err("unsupported input type should fail");

	// Assert
	assert_eq!(
		error,
		EventError::ControlBinding(ControlBindingError::UnsupportedElement {
			control: reinhardt_pages::component::ControlKind::Text,
			actual_tag: "input".to_owned(),
		})
	);
	if input_type == "file" {
		assert_eq!(input.value(), None);
	}
}

#[rstest]
#[case(PageElement::new("input"))]
#[case(PageElement::new("input").attr("type", "text"))]
#[case(PageElement::new("textarea"))]
fn text_binding_accepts_exact_text_controls(
	#[case] element: PageElement,
	reactive_scope: ReactiveScope,
) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, "bound".to_owned());
	let screen = render(
		element
			.attr("aria-label", "Text target")
			.control_binding(ControlBinding::text(value.clone())),
	);
	let input = screen.get_by_label("Text target");

	// Act
	input.input("edited");

	// Assert
	assert_eq!(value.get(), "edited");
	assert_eq!(input.value().as_deref(), Some("edited"));
}

#[rstest]
#[case("search", "bound", "edited")]
#[case("tel", "bound", "edited")]
#[case("url", "bound", "edited")]
#[case("email", "bound", "edited")]
#[case("password", "bound", "edited")]
#[case("color", "#112233", "#abcdef")]
#[case("date", "2026-08-30", "2026-08-31")]
#[case("datetime-local", "2026-08-30T09:15", "2026-08-31T10:30")]
#[case("month", "2026-08", "2026-09")]
#[case("week", "2026-W35", "2026-W36")]
#[case("time", "09:15", "10:30")]
fn text_binding_accepts_supported_input_types(
	#[case] input_type: &str,
	#[case] initial: &str,
	#[case] edited: &str,
	reactive_scope: ReactiveScope,
) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, initial.to_owned());
	let screen = render(
		PageElement::new("input")
			.attr("aria-label", "Text target")
			.attr("type", input_type.to_owned())
			.control_binding(ControlBinding::text(value.clone())),
	);
	let input = screen.get_by_label("Text target");

	// Act
	input.input(edited);

	// Assert
	assert_eq!(value.get(), edited);
	assert_eq!(input.value().as_deref(), Some(edited));
}

#[rstest]
fn number_binding_accepts_range_input_type(reactive_scope: ReactiveScope) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, 10_i32);
	let screen = render(
		PageElement::new("input")
			.attr("aria-label", "Range target")
			.attr("type", "range")
			.control_binding(ControlBinding::number(value.clone())),
	);
	let input = screen.get_by_label("Range target");

	// Act
	input.input("42");

	// Assert
	assert_eq!(value.get(), 42);
	assert_eq!(input.value().as_deref(), Some("42"));
}

#[rstest]
fn native_text_binding_applies_browser_value_sanitization(reactive_scope: ReactiveScope) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, "  https://example.test\n".to_owned());

	// Act
	let screen = render(
		PageElement::new("input")
			.attr("aria-label", "URL")
			.attr("type", "url")
			.control_binding(ControlBinding::text(value.clone())),
	);

	// Assert
	assert_eq!(value.get(), "https://example.test");
	assert_eq!(
		screen.get_by_label("URL").value().as_deref(),
		Some("https://example.test")
	);
}

#[rstest]
#[case("date", "2026-02-28", "2026-02-30")]
#[case("datetime-local", "2026-02-28T10:30", "2026-02-30T10:30")]
#[case("month", "2026-02", "2026-13")]
#[case("week", "2025-W52", "2025-W53")]
#[case("time", "10:30", "24:00")]
#[case("date", "2026-01-01", "2026-00-01")]
#[case("month", "2026-02", "2026-00")]
#[case("week", "2026-W53", "2026-W54")]
#[case("time", "10:30:00", "10:30:00.1234")]
#[case("date", "2026-01-01", "2026-\n08-31")]
fn native_temporal_binding_sanitizes_invalid_values(
	#[case] input_type: &str,
	#[case] initial: &str,
	#[case] edited: &str,
	reactive_scope: ReactiveScope,
) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, initial.to_owned());
	let screen = render(
		PageElement::new("input")
			.attr("aria-label", "Temporal target")
			.attr("type", input_type.to_owned())
			.control_binding(ControlBinding::text(value.clone())),
	);
	let input = screen.get_by_label("Temporal target");

	// Act
	input.input(edited);

	// Assert
	assert_eq!(value.get(), "");
	assert_eq!(input.value().as_deref(), Some(""));
}

#[rstest]
fn native_datetime_local_normalizes_a_space_separator(reactive_scope: ReactiveScope) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, "2026-08-31 10:30".to_owned());

	// Act
	let screen = render(
		PageElement::new("input")
			.attr("aria-label", "Datetime target")
			.attr("type", "datetime-local")
			.control_binding(ControlBinding::text(value.clone())),
	);

	// Assert
	assert_eq!(value.get(), "2026-08-31T10:30");
	assert_eq!(
		screen.get_by_label("Datetime target").value().as_deref(),
		Some("2026-08-31T10:30")
	);
}

#[rstest]
#[case("2026-08-31 10:30:00", "2026-08-31T10:30")]
#[case("2026-08-31 10:30:00.010", "2026-08-31T10:30:00.01")]
fn native_datetime_local_uses_browser_shortest_serialization(
	#[case] initial: &str,
	#[case] expected: &str,
	reactive_scope: ReactiveScope,
) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, initial.to_owned());

	// Act
	let screen = render(
		PageElement::new("input")
			.attr("aria-label", "Datetime target")
			.attr("type", "datetime-local")
			.control_binding(ControlBinding::text(value.clone())),
	);

	// Assert
	assert_eq!(value.get(), expected);
	assert_eq!(
		screen.get_by_label("Datetime target").value().as_deref(),
		Some(expected)
	);
}

#[rstest]
fn native_range_binding_applies_declared_step(reactive_scope: ReactiveScope) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, 3_i32);

	// Act
	let screen = render(
		PageElement::new("input")
			.attr("aria-label", "Stepped range")
			.attr("type", "range")
			.attr("min", "0")
			.attr("max", "10")
			.attr("step", "2")
			.control_binding(ControlBinding::number(value.clone())),
	);

	// Assert
	assert_eq!(value.get(), 4);
	assert_eq!(
		screen.get_by_label("Stepped range").value().as_deref(),
		Some("4")
	);
}

#[rstest]
fn native_initial_range_reconciliation_uses_the_bound_value_as_the_step_base(
	reactive_scope: ReactiveScope,
) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, 3_i32);

	// Act
	let screen = render(
		PageElement::new("input")
			.attr("aria-label", "Implicit step base")
			.attr("type", "range")
			.attr("step", "2")
			.control_binding(ControlBinding::number(value)),
	);

	// Assert
	assert_eq!(value.get(), 3);
	assert_eq!(
		screen.get_by_label("Implicit step base").value().as_deref(),
		Some("3")
	);
}

#[rstest]
#[case::leading_plus_min("min", "+10")]
#[case::leading_plus_step("step", "+2")]
fn native_range_ignores_constraints_outside_the_html_number_grammar(
	reactive_scope: ReactiveScope,
	#[case] attribute: &str,
	#[case] invalid_value: &str,
) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, 3_i32);
	let range = PageElement::new("input")
		.attr("aria-label", "Invalid range constraint")
		.attr("type", "range")
		.attr("max", "10");
	let range = match attribute {
		"min" => range.attr("min", invalid_value.to_owned()),
		"step" => range
			.attr("min", "0")
			.attr("step", invalid_value.to_owned()),
		_ => unreachable!("the test cases cover range constraints"),
	}
	.control_binding(ControlBinding::number(value));

	// Act
	let screen = render(range);

	// Assert
	assert_eq!(value.get(), 3);
	assert_eq!(
		screen
			.get_by_label("Invalid range constraint")
			.value()
			.as_deref(),
		Some("3")
	);
}

#[rstest]
#[tokio::test]
async fn range_binding_reconciles_native_control_bounds(reactive_scope: ReactiveScope) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, 200_i32);
	let screen = render(
		PageElement::new("input")
			.attr("aria-label", "Range target")
			.attr("type", "range")
			.control_binding(ControlBinding::number(value.clone())),
	);

	// Assert
	assert_eq!(value.get(), 100);
	assert_eq!(
		screen.get_by_label("Range target").value().as_deref(),
		Some("100")
	);

	// Act
	value.set(-10);
	screen.settle().await;

	// Assert
	assert_eq!(value.get(), 0);
	assert_eq!(
		screen.get_by_label("Range target").value().as_deref(),
		Some("0")
	);
}

#[rstest]
fn text_binding_accepts_an_input_type_with_text_fallback_semantics(reactive_scope: ReactiveScope) {
	// Arrange
	let value = signal_in_scope(&reactive_scope, "old\r\nvalue".to_owned());
	let screen = render(
		PageElement::new("input")
			.attr("aria-label", "Fallback text target")
			.attr("type", "future-control")
			.control_binding(ControlBinding::text(value.clone())),
	);
	let input = screen.get_by_label("Fallback text target");
	assert_eq!(value.get(), "oldvalue");
	assert_eq!(input.value().as_deref(), Some("oldvalue"));

	// Act
	input
		.dispatch(EventFixture::input().value("edited\r\nvalue"))
		.expect("unknown input type should use text fallback semantics");

	// Assert
	assert_eq!(value.get(), "editedvalue");
	assert_eq!(input.value().as_deref(), Some("editedvalue"));
}
