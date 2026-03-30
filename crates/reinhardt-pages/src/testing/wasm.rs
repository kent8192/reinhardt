//! WASM Test Utilities
//!
//! This module provides utilities for testing WASM-based functionality
//! in browser environments. It includes helpers for setting up test fixtures
//! like CSRF tokens in cookies, meta tags, and hidden form inputs.
//!
//! These utilities are designed to work with `wasm-bindgen-test` and
//! enable testing of CSRF protection, authentication flows, and DOM manipulation.

use wasm_bindgen::JsCast;
use web_sys::{Document, Element, HtmlDocument, HtmlInputElement, window};

/// Sets up a test CSRF token in the browser cookie.
///
/// This function creates a `csrftoken` cookie that can be read by
/// `get_csrf_token_from_cookie()` during tests.
///
/// # Arguments
///
/// * `token` - The CSRF token value to set
///
/// # Example
///
/// ```no_run
/// use reinhardt_pages::testing::wasm::setup_csrf_cookie;
///
/// setup_csrf_cookie("test_token_abc123");
/// ```
pub fn setup_csrf_cookie(token: &str) {
	if let Some(window) = window() {
		if let Some(document) = window.document() {
			if let Some(html_doc) = document.dyn_ref::<HtmlDocument>() {
				let cookie = format!("csrftoken={}", token);
				let _ = html_doc.set_cookie(&cookie);
			}
		}
	}
}

/// Sets up a test CSRF token in a `<meta>` tag.
///
/// This function creates or updates a `<meta name="csrf-token">` element
/// in the document head that can be read by `get_csrf_token_from_meta()`.
///
/// # Arguments
///
/// * `token` - The CSRF token value to set
///
/// # Example
///
/// ```no_run
/// use reinhardt_pages::testing::wasm::setup_csrf_meta_tag;
///
/// setup_csrf_meta_tag("test_token_abc123");
/// ```
pub fn setup_csrf_meta_tag(token: &str) {
	if let Some(window) = window() {
		if let Some(document) = window.document() {
			// Remove existing meta tag if present
			if let Ok(Some(existing)) = document.query_selector("meta[name=\"csrf-token\"]") {
				if let Some(parent) = existing.parent_node() {
					let _ = parent.remove_child(&existing);
				}
			}

			// Create new meta tag
			if let Ok(meta) = document.create_element("meta") {
				let _ = meta.set_attribute("name", "csrf-token");
				let _ = meta.set_attribute("content", token);

				// Append to head (or body if head doesn't exist)
				if let Some(head) = document.head() {
					let _ = head.append_child(&meta);
				} else if let Some(body) = document.body() {
					let _ = body.append_child(&meta);
				}
			}
		}
	}
}

/// Sets up a test CSRF token in a hidden form input.
///
/// This function creates or updates an `<input name="csrfmiddlewaretoken">`
/// element that can be read by `get_csrf_token_from_input()`.
///
/// # Arguments
///
/// * `token` - The CSRF token value to set
///
/// # Example
///
/// ```no_run
/// use reinhardt_pages::testing::wasm::setup_csrf_input;
///
/// setup_csrf_input("test_token_abc123");
/// ```
pub fn setup_csrf_input(token: &str) {
	if let Some(window) = window() {
		if let Some(document) = window.document() {
			// Remove existing input if present
			if let Ok(Some(existing)) =
				document.query_selector("input[name=\"csrfmiddlewaretoken\"]")
			{
				if let Some(parent) = existing.parent_node() {
					let _ = parent.remove_child(&existing);
				}
			}

			// Create new hidden input
			if let Ok(input) = document.create_element("input") {
				if let Some(input_elem) = input.dyn_ref::<HtmlInputElement>() {
					input_elem.set_type("hidden");
					input_elem.set_name("csrfmiddlewaretoken");
					input_elem.set_value(token);
				}

				// Append to body
				if let Some(body) = document.body() {
					let _ = body.append_child(&input);
				}
			}
		}
	}
}

/// Cleans up all test CSRF fixtures from the DOM.
///
/// This function removes:
/// - `csrftoken` cookie (by setting expiry to past)
/// - `<meta name="csrf-token">` element
/// - `<input name="csrfmiddlewaretoken">` element
///
/// Should be called at the end of each test to ensure clean state.
///
/// # Example
///
/// ```no_run
/// use reinhardt_pages::testing::wasm::{setup_csrf_cookie, cleanup_csrf_fixtures};
///
/// setup_csrf_cookie("test_token");
/// // ... run test ...
/// cleanup_csrf_fixtures();
/// ```
pub fn cleanup_csrf_fixtures() {
	if let Some(window) = window() {
		if let Some(document) = window.document() {
			// Clear cookie by setting expiry to past
			if let Some(html_doc) = document.dyn_ref::<HtmlDocument>() {
				let _ = html_doc
					.set_cookie("csrftoken=; expires=Thu, 01 Jan 1970 00:00:00 UTC; path=/");
			}

			// Remove meta tag
			if let Ok(Some(meta)) = document.query_selector("meta[name=\"csrf-token\"]") {
				if let Some(parent) = meta.parent_node() {
					let _ = parent.remove_child(&meta);
				}
			}

			// Remove hidden input
			if let Ok(Some(input)) = document.query_selector("input[name=\"csrfmiddlewaretoken\"]")
			{
				if let Some(parent) = input.parent_node() {
					let _ = parent.remove_child(&input);
				}
			}
		}
	}
}

/// Creates a test DOM element with the given ID and tag name.
///
/// Useful for setting up DOM structures needed by component tests.
///
/// # Arguments
///
/// * `document` - The DOM document to create the element in
/// * `tag_name` - The HTML tag name (e.g., "div", "form")
/// * `id` - The element ID to set
///
/// # Returns
///
/// The created element, or `None` if creation failed.
pub fn create_test_element(document: &Document, tag_name: &str, id: &str) -> Option<Element> {
	let element = document.create_element(tag_name).ok()?;
	element.set_id(id);
	document.body()?.append_child(&element).ok()?;
	Some(element)
}

/// Removes a test element by ID.
///
/// # Arguments
///
/// * `document` - The DOM document containing the element
/// * `id` - The ID of the element to remove
pub fn remove_test_element(document: &Document, id: &str) {
	if let Some(element) = document.get_element_by_id(id) {
		if let Some(parent) = element.parent_node() {
			let _ = parent.remove_child(&element);
		}
	}
}

/// Cleans up all test fixtures created during WASM tests.
///
/// This is a convenience function that calls all cleanup helpers.
/// Call this at the end of tests to ensure a clean DOM state.
pub fn cleanup_all_test_fixtures() {
	cleanup_csrf_fixtures();

	// Clean up any elements with "test-" prefix IDs
	if let Some(window) = window() {
		if let Some(document) = window.document() {
			if let Ok(elements) = document.query_selector_all("[id^=\"test-\"]") {
				for i in 0..elements.length() {
					if let Some(element) = elements.get(i) {
						if let Some(parent) = element.parent_node() {
							let _ = parent.remove_child(&element);
						}
					}
				}
			}
		}
	}
}
