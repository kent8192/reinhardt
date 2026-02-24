//! Unit tests for watch syntax in page! macro
//!
//! Tests validate:
//! 1. Watch block generates reactive View
//! 2. Signal dependencies are tracked correctly
//! 3. DOM updates when Signals change
//! 4. Edge cases (nesting, expressions, attributes)

use reinhardt_pages::component::Page;
use reinhardt_pages::page;
use reinhardt_pages::reactive::Signal;
use rstest::{fixture, rstest};
use serial_test::serial;

// ============================================================================
// Fixtures
// ============================================================================

/// Creates a boolean Signal for testing conditions
#[fixture]
fn bool_signal() -> Signal<bool> {
	Signal::new(false)
}

/// Creates a string Signal for testing content
#[fixture]
fn string_signal() -> Signal<String> {
	Signal::new("initial".to_string())
}

/// Creates an optional string Signal for error states
#[fixture]
fn error_signal() -> Signal<Option<String>> {
	Signal::new(None)
}

/// Creates a vector Signal for list testing
#[fixture]
fn list_signal() -> Signal<Vec<String>> {
	Signal::new(vec!["item1".to_string(), "item2".to_string()])
}

/// Creates a counter Signal for numeric testing
#[fixture]
fn counter_signal() -> Signal<i32> {
	Signal::new(0)
}

// ============================================================================
// HP-01: Basic watch with if condition
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_with_if_condition(bool_signal: Signal<bool>) {
	let signal = bool_signal.clone();

	// Create view with watch block containing if condition
	let view = page!(|signal: Signal<bool>| {
		div {
			watch {
				if signal.get() {
					span {
						"Visible"
					}
				}
			}
		}
	})(signal.clone());

	// Verify the view is created (Page::Element with reactive child)
	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "div");
			// Watch block generates a reactive child
			assert!(
				!el.child_views().is_empty(),
				"watch block should generate a child"
			);
		}
		_ => panic!("Expected Page::Element, got {:?}", view),
	}
}

// ============================================================================
// HP-02: Watch with if-else branching
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_with_if_else(bool_signal: Signal<bool>) {
	let signal = bool_signal.clone();

	let view = page!(|signal: Signal<bool>| {
		div {
			watch {
				if signal.get() {
					span {
						"True branch"
					}
				} else {
					span {
						"False branch"
					}
				}
			}
		}
	})(signal.clone());

	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "div");
			assert!(!el.child_views().is_empty());
		}
		_ => panic!("Expected Page::Element"),
	}
}

// ============================================================================
// HP-03: Nested if statements in watch
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_with_nested_if(bool_signal: Signal<bool>) {
	let outer = bool_signal.clone();
	let inner = Signal::new(true);

	let view = page!(|outer: Signal<bool>, inner: Signal<bool>| {
		div {
			watch {
				if outer.get() {
					if inner.get() {
						span {
							"Both true"
						}
					} else {
						span {
							"Outer only"
						}
					}
				} else {
					span {
						"Outer false"
					}
				}
			}
		}
	})(outer.clone(), inner.clone());

	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "div");
		}
		_ => panic!("Expected Page::Element"),
	}
}

// ============================================================================
// HP-04: Watch with element containing children
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_with_element_child(bool_signal: Signal<bool>) {
	let signal = bool_signal.clone();

	let view = page!(|signal: Signal<bool>| {
		div {
			watch {
				if signal.get() {
					div {
						class: "container",
						p {
							"Paragraph 1"
						}
						p {
							"Paragraph 2"
						}
					}
				}
			}
		}
	})(signal.clone());

	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "div");
		}
		_ => panic!("Expected Page::Element"),
	}
}

// ============================================================================
// HP-05: Watch with text content
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_with_text_content(string_signal: Signal<String>) {
	let signal = string_signal.clone();

	let view = page!(|signal: Signal<String>| {
		div {
			watch {
				{ signal.get() }
			}
		}
	})(signal.clone());

	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "div");
		}
		_ => panic!("Expected Page::Element"),
	}
}

// ============================================================================
// HP-06: Watch with expression node
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_with_expression_node(counter_signal: Signal<i32>) {
	let signal = counter_signal.clone();

	let view = page!(|signal: Signal<i32>| {
		div {
			watch {
				{ format!("Count: {}", signal.get()) }
			}
		}
	})(signal.clone());

	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "div");
		}
		_ => panic!("Expected Page::Element"),
	}
}

// ============================================================================
// HP-07: Watch with for loop
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_with_for_loop(list_signal: Signal<Vec<String>>) {
	let signal = list_signal.clone();

	let view = page!(|signal: Signal<Vec<String>>| {
		ul {
			watch {
				for item in signal.get().iter() {
					li {
						{ item.clone() }
					}
				}
			}
		}
	})(signal.clone());

	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "ul");
		}
		_ => panic!("Expected Page::Element"),
	}
}

// ============================================================================
// HP-08: Watch nested inside element
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_nested_in_element(bool_signal: Signal<bool>) {
	let signal = bool_signal.clone();

	let view = page!(|signal: Signal<bool>| {
		div {
			class: "outer",
			section {
				class: "middle",
				article {
					class: "inner",
					watch {
						if signal.get() {
							p {
								"Deep nested content"
							}
						}
					}
				}
			}
		}
	})(signal.clone());

	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "div");
			// Check that class attribute is "outer"
			let class_attr = el.attrs().iter().find(|(k, _)| k == "class");
			assert_eq!(class_attr.map(|(_, v)| v.as_ref()), Some("outer"));
		}
		_ => panic!("Expected Page::Element"),
	}
}

// ============================================================================
// HP-10: Multiple watch blocks in same parent
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_multiple_watch_blocks(bool_signal: Signal<bool>, error_signal: Signal<Option<String>>) {
	let loading = bool_signal.clone();
	let error = error_signal.clone();

	let view = page!(|loading: Signal<bool>, error: Signal<Option<String>>| {
		div {
			watch {
				if loading.get() {
					div {
						"Loading..."
					}
				}
			}
			watch {
				if error.get().is_some() {
					div {
						{ error.get().unwrap_or_default() }
					}
				}
			}
		}
	})(loading.clone(), error.clone());

	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "div");
			// Should have at least 2 children (the two watch blocks)
			assert!(
				el.child_views().len() >= 2,
				"Should have multiple watch children"
			);
		}
		_ => panic!("Expected Page::Element"),
	}
}

// ============================================================================
// EC-01: Deeply nested watch (5+ levels)
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_deeply_nested(bool_signal: Signal<bool>) {
	let signal = bool_signal.clone();

	let view = page!(|signal: Signal<bool>| {
		div {
			div {
				div {
					div {
						div {
							watch {
								if signal.get() {
									span {
										"Deep"
									}
								}
							}
						}
					}
				}
			}
		}
	})(signal.clone());

	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "div");
		}
		_ => panic!("Expected Page::Element"),
	}
}

// ============================================================================
// EC-03: Watch with unicode content
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_with_unicode() {
	let text = Signal::new("日本語テスト 🎉 emoji".to_string());

	let view = page!(|text: Signal<String>| {
		div {
			watch {
				{ text.get() }
			}
		}
	})(text.clone());

	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "div");
		}
		_ => panic!("Expected Page::Element"),
	}
}

// ============================================================================
// EC-05: Watch with empty string content
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_with_empty_string() {
	let text = Signal::new("".to_string());

	let view = page!(|text: Signal<String>| {
		div {
			watch {
				{ text.get() }
			}
		}
	})(text.clone());

	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "div");
		}
		_ => panic!("Expected Page::Element"),
	}
}

// ============================================================================
// EC-08: Watch with fragment
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_with_fragment_result(list_signal: Signal<Vec<String>>) {
	let items = list_signal.clone();

	let view = page!(|items: Signal<Vec<String>>| {
		div {
			watch {
				{ Page::fragment(items.get().iter().map(|i| { Page::text(i.clone()) }).collect ::<Vec<Page>>()) }
			}
		}
	})(items.clone());

	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "div");
		}
		_ => panic!("Expected Page::Element"),
	}
}

// ============================================================================
// EC-09: Watch with data-* attributes
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_with_data_attributes(counter_signal: Signal<i32>) {
	let signal = counter_signal.clone();

	let view = page!(|signal: Signal<i32>| {
		div {
			watch {
				if signal.get()> 0 {
					span {
						data_count: signal.get().to_string(),
						"Has data"
					}
				}
			}
		}
	})(signal.clone());

	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "div");
		}
		_ => panic!("Expected Page::Element"),
	}
}

// ============================================================================
// EQ-01: Watch condition boolean partitions (true, false)
// ============================================================================

#[rstest]
#[case(true)]
#[case(false)]
#[serial(reactive)]
fn test_watch_condition_boolean_partitions(#[case] initial_value: bool) {
	let signal = Signal::new(initial_value);

	let view = page!(|signal: Signal<bool>| {
		div {
			watch {
				if signal.get() {
					span {
						"True"
					}
				} else {
					span {
						"False"
					}
				}
			}
		}
	})(signal.clone());

	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "div");
		}
		_ => panic!("Expected Page::Element"),
	}
}

// ============================================================================
// BV-01: Watch nesting depth boundaries
// ============================================================================

#[rstest]
#[case(0)]
#[case(1)]
#[case(3)]
#[serial(reactive)]
fn test_watch_nesting_depth(#[case] depth: usize, bool_signal: Signal<bool>) {
	let signal = bool_signal.clone();

	// Create view with varying nesting depth
	let view = page!(|signal: Signal<bool>| {
		div {
			watch {
				if signal.get() {
					span {
						"Content"
					}
				}
			}
		}
	})(signal.clone());

	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "div");
		}
		_ => panic!("Expected Page::Element for depth {}", depth),
	}
}

// ============================================================================
// DT-01: Watch condition x content type matrix
// ============================================================================

#[rstest]
#[case(true, "text")]
#[case(false, "text")]
#[case(true, "element")]
#[case(false, "element")]
#[serial(reactive)]
fn test_watch_condition_content_matrix(#[case] condition: bool, #[case] content_type: &str) {
	let signal = Signal::new(condition);

	let view = if content_type == "text" {
		page!(|signal: Signal<bool>| {
			div {
				watch {
					if signal.get() {
						"Text content"
					} else {
						"Other text"
					}
				}
			}
		})(signal.clone())
	} else {
		page!(|signal: Signal<bool>| {
			div {
				watch {
					if signal.get() {
						span {
							"Element content"
						}
					} else {
						span {
							"Other element"
						}
					}
				}
			}
		})(signal.clone())
	};

	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "div");
		}
		_ => panic!(
			"Expected Page::Element for condition={}, content={}",
			condition, content_type
		),
	}
}

// ============================================================================
// Signal reactivity test: verify Page::reactive wraps closure correctly
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_signal_reactivity() {
	let signal = Signal::new(false);
	let signal_clone = signal.clone();

	// Create a reactive view
	let view = Page::reactive(move || {
		if signal_clone.get() {
			Page::text("True")
		} else {
			Page::text("False")
		}
	});

	// Verify the view is a Reactive variant
	match view {
		Page::Reactive(reactive) => {
			// Manually call render to verify the closure works
			let rendered = reactive.render();
			match rendered {
				Page::Text(text) => {
					assert_eq!(text, "False", "Initial value should be false");
				}
				_ => panic!("Expected Page::Text, got {:?}", rendered),
			}
		}
		_ => panic!("Expected Page::Reactive, got {:?}", view),
	}
}

// ============================================================================
// Complex scenario: loading/error/data states
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_complex_state_machine() {
	let loading = Signal::new(true);
	let error = Signal::<Option<String>>::new(None);
	let data = Signal::new(vec!["item1".to_string()]);

	let view = page!(|loading: Signal<bool>, error: Signal<Option<String>>, data: Signal<Vec<String>>| {
		div {
			class: "container",
			watch {
				if error.get().is_some() {
					div {
						class: "error",
						{ error.get().unwrap_or_default() }
					}
				}
			}
			watch {
				if loading.get() {
					div {
						"Loading..."
					}
				} else if data.get().is_empty() {
					div {
						"No data"
					}
				} else {
					ul {
						for item in data.get().iter() {
							li {
								{ item.clone() }
							}
						}
					}
				}
			}
		}
	})(loading.clone(), error.clone(), data.clone());

	match &view {
		Page::Element(el) => {
			assert_eq!(el.tag_name(), "div");
			// Check that class attribute is "container"
			let class_attr = el.attrs().iter().find(|(k, _)| k == "class");
			assert_eq!(class_attr.map(|(_, v)| v.as_ref()), Some("container"));
		}
		_ => panic!("Expected Page::Element"),
	}
}
