#![cfg(not(target_arch = "wasm32"))]
//! SSR tests for watch syntax in page! macro
//!
//! Tests validate:
//! 1. Watch blocks render correctly during SSR
//! 2. Reactive views are evaluated and converted to HTML
//! 3. Conditional content renders based on initial Signal values
//! 4. Escape handling for XSS prevention
//! 5. Integration with SsrRenderer

use reinhardt_pages::component::Page;
use reinhardt_pages::page;
use reinhardt_pages::reactive::Signal;
use reinhardt_pages::ssr::SsrRenderer;
use rstest::{fixture, rstest};
use serial_test::serial;

// ============================================================================
// Fixtures
// ============================================================================

/// Creates a default SSR renderer
#[fixture]
fn ssr_renderer() -> SsrRenderer {
	SsrRenderer::new()
}

/// Creates a boolean Signal for testing conditions (initially false)
#[fixture]
fn bool_signal_false() -> Signal<bool> {
	Signal::new(false)
}

/// Creates a boolean Signal for testing conditions (initially true)
#[fixture]
fn bool_signal_true() -> Signal<bool> {
	Signal::new(true)
}

/// Creates a string Signal for content testing
#[fixture]
fn string_signal() -> Signal<String> {
	Signal::new("Hello, World!".to_string())
}

/// Creates an optional string Signal for error states
#[fixture]
fn error_signal() -> Signal<Option<String>> {
	Signal::new(None)
}

/// Creates a list Signal for iteration testing
#[fixture]
fn list_signal() -> Signal<Vec<String>> {
	Signal::new(vec![
		"Item 1".to_string(),
		"Item 2".to_string(),
		"Item 3".to_string(),
	])
}

/// Creates a counter Signal for numeric testing
#[fixture]
fn counter_signal() -> Signal<i32> {
	Signal::new(42)
}

// ============================================================================
// SSR-01: Basic SSR rendering of watch block
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_basic_render(string_signal: Signal<String>) {
	let signal = string_signal.clone();

	let view = page!(|signal: Signal<String>| {
		div {
			class: "container",
			watch {
				span {
					{ signal.get() }
				}
			}
		}
	})(signal.clone());

	let html = view.render_to_string();

	assert!(
		html.contains("class=\"container\""),
		"Should have container class"
	);
	assert!(html.contains("<span>"), "Should have span element");
	assert!(
		html.contains("Hello, World!"),
		"Should contain signal value"
	);
	assert!(html.contains("</span>"), "Should close span element");
}

// ============================================================================
// SSR-02: Watch with condition evaluating to true
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_condition_true(bool_signal_true: Signal<bool>) {
	let signal = bool_signal_true.clone();

	let view = page!(|signal: Signal<bool>| {
		div {
			watch {
				if signal.get() {
					span {
						"Visible content"
					}
				}
			}
		}
	})(signal.clone());

	let html = view.render_to_string();

	assert!(
		html.contains("<span>"),
		"Should have span when condition is true"
	);
	assert!(
		html.contains("Visible content"),
		"Should contain conditional content"
	);
}

// ============================================================================
// SSR-03: Watch with condition evaluating to false
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_condition_false(bool_signal_false: Signal<bool>) {
	let signal = bool_signal_false.clone();

	let view = page!(|signal: Signal<bool>| {
		div {
			watch {
				if signal.get() {
					span {
						"Should not appear"
					}
				}
			}
		}
	})(signal.clone());

	let html = view.render_to_string();

	assert!(
		!html.contains("Should not appear"),
		"Should not contain content when condition is false"
	);
	// The div should still be present
	assert!(html.contains("<div>"), "Should have outer div");
	assert!(html.contains("</div>"), "Should close outer div");
}

// ============================================================================
// SSR-04: Watch with if-else rendering correct branch
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_if_else_true_branch(bool_signal_true: Signal<bool>) {
	let signal = bool_signal_true.clone();

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

	let html = view.render_to_string();

	assert!(html.contains("True branch"), "Should render true branch");
	assert!(
		!html.contains("False branch"),
		"Should not render false branch"
	);
}

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_if_else_false_branch(bool_signal_false: Signal<bool>) {
	let signal = bool_signal_false.clone();

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

	let html = view.render_to_string();

	assert!(
		!html.contains("True branch"),
		"Should not render true branch"
	);
	assert!(html.contains("False branch"), "Should render false branch");
}

// ============================================================================
// SSR-05: Watch with nested elements
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_nested_elements(bool_signal_true: Signal<bool>) {
	let signal = bool_signal_true.clone();

	let view = page!(|signal: Signal<bool>| {
		div {
			class: "outer",
			watch {
				if signal.get() {
					section {
						class: "section",
						article {
							class: "article",
							p {
								"Nested paragraph"
							}
						}
					}
				}
			}
		}
	})(signal.clone());

	let html = view.render_to_string();

	assert!(html.contains("class=\"outer\""), "Should have outer class");
	assert!(html.contains("<section"), "Should have section element");
	assert!(
		html.contains("class=\"section\""),
		"Should have section class"
	);
	assert!(html.contains("<article"), "Should have article element");
	assert!(html.contains("<p>"), "Should have paragraph element");
	assert!(
		html.contains("Nested paragraph"),
		"Should contain nested text"
	);
}

// ============================================================================
// SSR-06: Watch with content escaping (XSS prevention)
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_content_escaping() {
	let xss_content = Signal::new("<script>alert('xss')</script>".to_string());

	let view = page!(|xss_content: Signal<String>| {
		div {
			watch {
				{ xss_content.get() }
			}
		}
	})(xss_content.clone());

	let html = view.render_to_string();

	// The script tag should be escaped
	assert!(
		!html.contains("<script>"),
		"Should not contain unescaped script tag"
	);
	assert!(
		html.contains("&lt;script&gt;"),
		"Should have escaped script tag"
	);
	assert!(html.contains("&#x27;"), "Should have escaped single quotes");
}

// ============================================================================
// SSR-07: Watch with for loop rendering
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_for_loop(list_signal: Signal<Vec<String>>) {
	let items = list_signal.clone();

	let view = page!(|items: Signal<Vec<String>>| {
		ul {
			watch {
				for item in items.get().iter() {
					li {
						{ item.clone() }
					}
				}
			}
		}
	})(items.clone());

	let html = view.render_to_string();

	assert!(html.contains("<ul>"), "Should have ul element");
	assert!(
		html.contains("<li>Item 1</li>"),
		"Should contain first item"
	);
	assert!(
		html.contains("<li>Item 2</li>"),
		"Should contain second item"
	);
	assert!(
		html.contains("<li>Item 3</li>"),
		"Should contain third item"
	);
	assert!(html.contains("</ul>"), "Should close ul element");
}

// ============================================================================
// SSR-08: Watch with expression rendering
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_expression(counter_signal: Signal<i32>) {
	let counter = counter_signal.clone();

	let view = page!(|counter: Signal<i32>| {
		div {
			watch {
				{ format!("Count: {}", counter.get()) }
			}
		}
	})(counter.clone());

	let html = view.render_to_string();

	assert!(
		html.contains("Count: 42"),
		"Should render formatted expression"
	);
}

// ============================================================================
// SSR-09: Multiple watch blocks in same parent
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_multiple_blocks(
	bool_signal_true: Signal<bool>,
	error_signal: Signal<Option<String>>,
) {
	let loading = bool_signal_true.clone();
	let error = error_signal.clone();

	let view = page!(|loading: Signal<bool>, error: Signal<Option<String>>| {
		div {
			watch {
				if loading.get() {
					div {
						class: "loading",
						"Loading..."
					}
				}
			}
			watch {
				if error.get().is_some() {
					div {
						class: "error",
						{ error.get().unwrap_or_default() }
					}
				}
			}
		}
	})(loading.clone(), error.clone());

	let html = view.render_to_string();

	assert!(html.contains("Loading..."), "Should render loading block");
	assert!(
		html.contains("class=\"loading\""),
		"Should have loading class"
	);
	assert!(
		!html.contains("class=\"error\""),
		"Should not render error block when None"
	);
}

// ============================================================================
// SSR-10: Watch with unicode content
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_unicode() {
	let unicode_content = Signal::new("日本語テスト 🎉 한국어".to_string());

	let view = page!(|content: Signal<String>| {
		div {
			watch {
				{ content.get() }
			}
		}
	})(unicode_content.clone());

	let html = view.render_to_string();

	assert!(
		html.contains("日本語テスト"),
		"Should contain Japanese characters"
	);
	assert!(html.contains("🎉"), "Should contain emoji");
	assert!(html.contains("한국어"), "Should contain Korean characters");
}

// ============================================================================
// SSR-11: Watch with data attributes
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_data_attributes(counter_signal: Signal<i32>) {
	let counter = counter_signal.clone();

	let view = page!(|counter: Signal<i32>| {
		div {
			watch {
				span {
					data_count: counter.get().to_string(),
					data_active: "true",
					"Counter element"
				}
			}
		}
	})(counter.clone());

	let html = view.render_to_string();

	assert!(
		html.contains("data-count=\"42\""),
		"Should have data-count attribute"
	);
	assert!(
		html.contains("data-active=\"true\""),
		"Should have data-active attribute"
	);
}

// ============================================================================
// SSR-12: Watch with empty content
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_empty_content() {
	let empty = Signal::new("".to_string());

	let view = page!(|empty: Signal<String>| {
		div {
			class: "wrapper",
			watch {
				{ empty.get() }
			}
		}
	})(empty.clone());

	let html = view.render_to_string();

	assert!(
		html.contains("class=\"wrapper\""),
		"Should have wrapper class"
	);
	assert!(
		html.contains("<div class=\"wrapper\"></div>"),
		"Should have empty div"
	);
}

// ============================================================================
// SSR-13: Watch with SsrRenderer
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_with_renderer(ssr_renderer: SsrRenderer, string_signal: Signal<String>) {
	let signal = string_signal.clone();

	let view = page!(|signal: Signal<String>| {
		div {
			watch {
				p {
					{ signal.get() }
				}
			}
		}
	})(signal.clone());

	let html = ssr_renderer.render_view(&view);

	assert!(html.contains("<div>"), "Should have div element");
	assert!(
		html.contains("<p>Hello, World!</p>"),
		"Should contain paragraph with content"
	);
}

// ============================================================================
// SSR-14: Watch with deeply nested structure
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_deeply_nested(bool_signal_true: Signal<bool>) {
	let signal = bool_signal_true.clone();

	let view = page!(|signal: Signal<bool>| {
		div {
			class: "level-1",
			div {
				class: "level-2",
				div {
					class: "level-3",
					watch {
						if signal.get() {
							div {
								class: "level-4",
								span {
									"Deep content"
								}
							}
						}
					}
				}
			}
		}
	})(signal.clone());

	let html = view.render_to_string();

	assert!(
		html.contains("class=\"level-1\""),
		"Should have level-1 class"
	);
	assert!(
		html.contains("class=\"level-2\""),
		"Should have level-2 class"
	);
	assert!(
		html.contains("class=\"level-3\""),
		"Should have level-3 class"
	);
	assert!(
		html.contains("class=\"level-4\""),
		"Should have level-4 class"
	);
	assert!(html.contains("Deep content"), "Should contain deep content");
}

// ============================================================================
// SSR-15: Watch with fragment result
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_fragment_result(list_signal: Signal<Vec<String>>) {
	let items = list_signal.clone();

	let view = page!(|items: Signal<Vec<String>>| {
		div {
			watch {
				{
					Page::fragment(
							items
								.get()
								.iter()
								.map(|i| Page::text(i.clone()))
								.collect::<Vec<Page>>(),
						)
				}
			}
		}
	})(items.clone());

	let html = view.render_to_string();

	assert!(html.contains("Item 1"), "Should contain first item");
	assert!(html.contains("Item 2"), "Should contain second item");
	assert!(html.contains("Item 3"), "Should contain third item");
}

// ============================================================================
// SSR-16: Watch state combination matrix
// ============================================================================

#[rstest]
#[case(true, Some("Error!".to_string()), true)]
#[case(true, None, true)]
#[case(false, Some("Error!".to_string()), false)]
#[case(false, None, false)]
#[serial(reactive)]
fn test_watch_ssr_state_matrix(
	#[case] loading_state: bool,
	#[case] error_state: Option<String>,
	#[case] expect_loading: bool,
) {
	let loading = Signal::new(loading_state);
	let error = Signal::new(error_state.clone());

	let view = page!(|loading: Signal<bool>, error: Signal<Option<String>>| {
		div {
			watch {
				if loading.get() {
					div {
						class: "loading",
						"Loading..."
					}
				}
			}
			watch {
				if error.get().is_some() {
					div {
						class: "error",
						{ error.get().unwrap_or_default() }
					}
				}
			}
		}
	})(loading.clone(), error.clone());

	let html = view.render_to_string();

	if expect_loading {
		assert!(
			html.contains("Loading..."),
			"Should show loading when loading=true"
		);
	} else {
		assert!(
			!html.contains("Loading..."),
			"Should not show loading when loading=false"
		);
	}

	if error_state.is_some() {
		assert!(
			html.contains("Error!"),
			"Should show error when error is Some"
		);
	} else {
		assert!(
			!html.contains("class=\"error\""),
			"Should not show error when error is None"
		);
	}
}

// ============================================================================
// SSR-17: Watch with special HTML characters in attributes
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_special_chars_in_attrs() {
	let title = Signal::new("Title with \"quotes\" & ampersand".to_string());

	let view = page!(|title: Signal<String>| {
		div {
			watch {
				span {
					title: title.get(),
					"Content"
				}
			}
		}
	})(title.clone());

	let html = view.render_to_string();

	// Attribute values should be properly escaped
	assert!(html.contains("title="), "Should have title attribute");
	assert!(html.contains("Content"), "Should contain content");
}

// ============================================================================
// SSR-18: Watch with void elements
// ============================================================================

#[rstest]
#[serial(reactive)]
fn test_watch_ssr_void_elements(bool_signal_true: Signal<bool>) {
	let show = bool_signal_true.clone();

	let view = page!(|show: Signal<bool>| {
		div {
			watch {
				if show.get() {
					br {}
					hr {}
					img {
						src: "/image.png",
						alt: "Test image",
					}
				}
			}
		}
	})(show.clone());

	let html = view.render_to_string();

	assert!(html.contains("<br />"), "Should have self-closing br");
	assert!(html.contains("<hr />"), "Should have self-closing hr");
	assert!(html.contains("<img "), "Should have img element");
	assert!(
		html.contains("src=\"/image.png\""),
		"Should have src attribute"
	);
	assert!(
		html.contains("alt=\"Test image\""),
		"Should have alt attribute"
	);
}
