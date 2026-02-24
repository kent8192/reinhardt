#![cfg(target_arch = "wasm32")]

//! DOM Query API for WASM Frontend Testing
//!
//! This module provides Testing Library-style DOM queries for WASM tests.
//! It prioritizes accessibility-based queries (role, label) over implementation
//! details (test IDs, CSS selectors).
//!
//! Uses `escape_css_selector` from reinhardt-core for safe CSS selector escaping.
//!
//! # Query Priority (Recommended Order)
//!
//! 1. **Role-based** - `get_by_role()`, `get_by_role_with_name()`
//! 2. **Label-based** - `get_by_label_text()`
//! 3. **Text-based** - `get_by_text()`
//! 4. **Placeholder** - `get_by_placeholder_text()`
//! 5. **Test ID** - `get_by_test_id()` (fallback)
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_test::wasm::{Screen, QueryResult};
//!
//! let screen = Screen::new();
//!
//! // Role-based query (preferred)
//! let button = screen.get_by_role("button").get();
//! let submit = screen.get_by_role_with_name("button", "Submit").get();
//!
//! // Text-based query
//! let heading = screen.get_by_text("Welcome").get();
//!
//! // Async query (waits for element)
//! let loaded = screen.find_by_text("Loaded").await;
//! ```

use reinhardt_core::security::escape_css_selector;
use web_sys::{Document, Element, NodeList, window};

/// Result of a DOM query operation.
///
/// Contains zero or more elements matching the query criteria.
/// Provides methods for accessing elements and making assertions.
#[derive(Debug, Clone)]
pub struct QueryResult {
	elements: Vec<Element>,
	query_description: String,
}

impl QueryResult {
	/// Create a new QueryResult from a list of elements.
	pub fn new(elements: Vec<Element>, description: impl Into<String>) -> Self {
		Self {
			elements,
			query_description: description.into(),
		}
	}

	/// Create an empty QueryResult.
	pub fn empty(description: impl Into<String>) -> Self {
		Self {
			elements: Vec::new(),
			query_description: description.into(),
		}
	}

	/// Get the first matching element.
	///
	/// # Panics
	///
	/// Panics if no elements match the query. Use `query()` for a non-panicking
	/// alternative.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let button = screen.get_by_role("button").get();
	/// ```
	pub fn get(&self) -> Element {
		self.elements
			.first()
			.cloned()
			.unwrap_or_else(|| panic!("No element found for query: {}", self.query_description))
	}

	/// Get the first matching element, or `None` if no match.
	///
	/// Use this when you want to check if an element exists without panicking.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// if let Some(button) = screen.get_by_role("button").query() {
	///     // Element exists
	/// }
	/// ```
	pub fn query(&self) -> Option<Element> {
		self.elements.first().cloned()
	}

	/// Get all matching elements.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let buttons = screen.get_by_role("button").get_all();
	/// assert_eq!(buttons.len(), 3);
	/// ```
	pub fn get_all(&self) -> Vec<Element> {
		self.elements.clone()
	}

	/// Get the number of matching elements.
	pub fn count(&self) -> usize {
		self.elements.len()
	}

	/// Check if any elements matched the query.
	pub fn exists(&self) -> bool {
		!self.elements.is_empty()
	}

	/// Assert that exactly one element was found and return it.
	///
	/// # Panics
	///
	/// Panics if zero or more than one element is found.
	pub fn get_only(&self) -> Element {
		match self.elements.len() {
			0 => panic!("No element found for query: {}", self.query_description),
			1 => self.elements[0].clone(),
			n => panic!(
				"Expected exactly one element for query '{}', but found {}",
				self.query_description, n
			),
		}
	}

	/// Assert that the element exists.
	///
	/// # Panics
	///
	/// Panics if no elements match.
	pub fn should_exist(&self) {
		if self.elements.is_empty() {
			panic!(
				"Expected element to exist for query: {}",
				self.query_description
			);
		}
	}

	/// Assert that no elements match.
	///
	/// # Panics
	///
	/// Panics if any elements match.
	pub fn should_not_exist(&self) {
		if !self.elements.is_empty() {
			panic!(
				"Expected no elements for query '{}', but found {}",
				self.query_description,
				self.elements.len()
			);
		}
	}

	/// Get the query description.
	pub fn description(&self) -> &str {
		&self.query_description
	}
}

/// Screen object for querying the DOM.
///
/// The Screen provides methods for finding elements in the DOM using
/// accessibility-friendly queries. It can be created for the entire document
/// or scoped to a specific element.
///
/// # Example
///
/// ```rust,ignore
/// // Query the entire document
/// let screen = Screen::new();
///
/// // Query within a specific element
/// let container = document.get_element_by_id("app").unwrap();
/// let scoped = Screen::within(&container);
/// ```
#[derive(Debug, Clone)]
pub struct Screen {
	root: Option<Element>,
}

impl Default for Screen {
	fn default() -> Self {
		Self::new()
	}
}

impl Screen {
	/// Create a Screen for the entire document.
	pub fn new() -> Self {
		Self { root: None }
	}

	/// Create a Screen scoped to a specific element.
	///
	/// Queries will only search within the given element and its descendants.
	pub fn within(element: &Element) -> Self {
		Self {
			root: Some(element.clone()),
		}
	}

	/// Get the document.
	fn document(&self) -> Option<Document> {
		window().and_then(|w| w.document())
	}

	/// Get the root element for queries (body if not scoped).
	fn query_root(&self) -> Option<Element> {
		if let Some(ref root) = self.root {
			Some(root.clone())
		} else {
			self.document().and_then(|d| d.body()).map(|b| b.into())
		}
	}

	/// Execute a CSS selector query.
	fn query_selector_all(&self, selector: &str) -> Vec<Element> {
		let root = match self.query_root() {
			Some(r) => r,
			None => return Vec::new(),
		};

		root.query_selector_all(selector)
			.ok()
			.map(|list| node_list_to_elements(&list))
			.unwrap_or_default()
	}

	// === Role-based Queries (Highest Priority) ===

	/// Query elements by their ARIA role.
	///
	/// This is the preferred query method as it reflects how assistive
	/// technologies perceive the page.
	///
	/// # Arguments
	///
	/// * `role` - The ARIA role (e.g., "button", "heading", "textbox")
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let buttons = screen.get_by_role("button").get_all();
	/// let heading = screen.get_by_role("heading").get();
	/// ```
	pub fn get_by_role(&self, role: &str) -> QueryResult {
		let selector = format!("[role=\"{}\"]", escape_css_selector(role));
		let mut elements = self.query_selector_all(&selector);

		// Also include implicit roles from HTML elements
		let implicit_elements = self.get_elements_with_implicit_role(role);
		for elem in implicit_elements {
			if !elements.iter().any(|e| e == &elem) {
				elements.push(elem);
			}
		}

		QueryResult::new(elements, format!("role=\"{}\"", role))
	}

	/// Query elements by ARIA role and accessible name.
	///
	/// The accessible name can come from aria-label, aria-labelledby,
	/// or the element's text content.
	///
	/// # Arguments
	///
	/// * `role` - The ARIA role
	/// * `name` - The accessible name (case-insensitive substring match)
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let submit = screen.get_by_role_with_name("button", "Submit").get();
	/// ```
	pub fn get_by_role_with_name(&self, role: &str, name: &str) -> QueryResult {
		let all_with_role = self.get_by_role(role);
		let name_lower = name.to_lowercase();

		let filtered: Vec<Element> = all_with_role
			.elements
			.into_iter()
			.filter(|elem| {
				let accessible_name = get_accessible_name(elem).to_lowercase();
				accessible_name.contains(&name_lower)
			})
			.collect();

		QueryResult::new(filtered, format!("role=\"{}\" name=\"{}\"", role, name))
	}

	/// Get elements that have an implicit ARIA role from their HTML tag.
	fn get_elements_with_implicit_role(&self, role: &str) -> Vec<Element> {
		let tags = match role {
			"button" => vec!["button", "input[type=\"button\"]", "input[type=\"submit\"]"],
			"textbox" => vec!["input[type=\"text\"]", "input:not([type])", "textarea"],
			"checkbox" => vec!["input[type=\"checkbox\"]"],
			"radio" => vec!["input[type=\"radio\"]"],
			"link" => vec!["a[href]"],
			"heading" => vec!["h1", "h2", "h3", "h4", "h5", "h6"],
			"list" => vec!["ul", "ol"],
			"listitem" => vec!["li"],
			"img" => vec!["img"],
			"navigation" => vec!["nav"],
			"main" => vec!["main"],
			"banner" => vec!["header"],
			"contentinfo" => vec!["footer"],
			"form" => vec!["form"],
			"search" => vec!["search"],
			"article" => vec!["article"],
			"region" => vec!["section[aria-label]", "section[aria-labelledby]"],
			"combobox" => vec!["select"],
			"option" => vec!["option"],
			_ => vec![],
		};

		let mut results = Vec::new();
		for tag_selector in tags {
			results.extend(self.query_selector_all(tag_selector));
		}
		results
	}

	// === Text-based Queries ===

	/// Query elements by their text content.
	///
	/// Performs a case-insensitive substring match on the element's
	/// text content.
	///
	/// # Arguments
	///
	/// * `text` - The text to search for
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let welcome = screen.get_by_text("Welcome").get();
	/// ```
	pub fn get_by_text(&self, text: &str) -> QueryResult {
		let root = match self.query_root() {
			Some(r) => r,
			None => return QueryResult::empty(format!("text=\"{}\"", text)),
		};

		let text_lower = text.to_lowercase();
		let elements = find_elements_with_text(&root, &text_lower);

		QueryResult::new(elements, format!("text=\"{}\"", text))
	}

	/// Query elements by text content using a regex pattern.
	///
	/// # Arguments
	///
	/// * `pattern` - The regex pattern to match
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let count = screen.get_by_text_regex(r"Count: \d+").get();
	/// ```
	pub fn get_by_text_regex(&self, pattern: &str) -> QueryResult {
		let root = match self.query_root() {
			Some(r) => r,
			None => return QueryResult::empty(format!("text_regex=\"{}\"", pattern)),
		};

		let re = match regex::Regex::new(pattern) {
			Ok(r) => r,
			Err(_) => return QueryResult::empty(format!("text_regex=\"{}\" (invalid)", pattern)),
		};

		let elements = find_elements_matching_regex(&root, &re);

		QueryResult::new(elements, format!("text_regex=\"{}\"", pattern))
	}

	// === Label-based Queries ===

	/// Query form elements by their associated label text.
	///
	/// Finds elements that have a `<label>` with matching text, either via
	/// the `for` attribute or by nesting.
	///
	/// # Arguments
	///
	/// * `label` - The label text to search for
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let email = screen.get_by_label_text("Email").get();
	/// ```
	pub fn get_by_label_text(&self, label: &str) -> QueryResult {
		let label_lower = label.to_lowercase();
		let mut elements = Vec::new();

		// Find labels with matching text
		let labels = self.query_selector_all("label");
		for label_elem in labels {
			let label_text = label_elem.text_content().unwrap_or_default().to_lowercase();
			if label_text.contains(&label_lower) {
				// Check for `for` attribute
				if let Some(for_id) = label_elem.get_attribute("for") {
					if let Some(doc) = self.document() {
						if let Some(target) = doc.get_element_by_id(&for_id) {
							elements.push(target);
						}
					}
				}

				// Check for nested input
				if let Ok(Some(nested)) = label_elem.query_selector("input, select, textarea") {
					if !elements.contains(&nested) {
						elements.push(nested);
					}
				}
			}
		}

		// Also check aria-label and aria-labelledby
		let aria_labeled = self.query_selector_all(&format!(
			"[aria-label*=\"{}\" i]",
			escape_css_selector(label)
		));
		for elem in aria_labeled {
			if !elements.contains(&elem) {
				elements.push(elem);
			}
		}

		QueryResult::new(elements, format!("label=\"{}\"", label))
	}

	/// Query elements by their placeholder attribute.
	///
	/// # Arguments
	///
	/// * `placeholder` - The placeholder text to search for
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let search = screen.get_by_placeholder_text("Search...").get();
	/// ```
	pub fn get_by_placeholder_text(&self, placeholder: &str) -> QueryResult {
		let selector = format!("[placeholder*=\"{}\" i]", escape_css_selector(placeholder));
		let elements = self.query_selector_all(&selector);
		QueryResult::new(elements, format!("placeholder=\"{}\"", placeholder))
	}

	// === Attribute-based Queries ===

	/// Query elements by their `data-testid` attribute.
	///
	/// This is a fallback query method when accessibility-based queries
	/// are not practical.
	///
	/// # Arguments
	///
	/// * `test_id` - The test ID value
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let modal = screen.get_by_test_id("confirmation-modal").get();
	/// ```
	pub fn get_by_test_id(&self, test_id: &str) -> QueryResult {
		let selector = format!("[data-testid=\"{}\"]", escape_css_selector(test_id));
		let elements = self.query_selector_all(&selector);
		QueryResult::new(elements, format!("data-testid=\"{}\"", test_id))
	}

	/// Query elements by their `alt` attribute (for images).
	///
	/// # Arguments
	///
	/// * `alt` - The alt text to search for
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let logo = screen.get_by_alt_text("Company Logo").get();
	/// ```
	pub fn get_by_alt_text(&self, alt: &str) -> QueryResult {
		let selector = format!("[alt*=\"{}\" i]", escape_css_selector(alt));
		let elements = self.query_selector_all(&selector);
		QueryResult::new(elements, format!("alt=\"{}\"", alt))
	}

	/// Query elements by their `title` attribute.
	///
	/// # Arguments
	///
	/// * `title` - The title text to search for
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let tooltip = screen.get_by_title("More information").get();
	/// ```
	pub fn get_by_title(&self, title: &str) -> QueryResult {
		let selector = format!("[title*=\"{}\" i]", escape_css_selector(title));
		let elements = self.query_selector_all(&selector);
		QueryResult::new(elements, format!("title=\"{}\"", title))
	}

	/// Query elements by CSS selector (escape hatch).
	///
	/// Use accessibility-based queries when possible. This is provided
	/// as a fallback for complex selection scenarios.
	///
	/// # Arguments
	///
	/// * `selector` - The CSS selector
	pub fn query_selector(&self, selector: &str) -> QueryResult {
		let elements = self.query_selector_all(selector);
		QueryResult::new(elements, format!("selector=\"{}\"", selector))
	}

	// === Async Queries (find_by_*) ===

	/// Wait for an element with the given role to appear.
	///
	/// This is useful for testing asynchronous content that loads after
	/// the initial render.
	///
	/// # Arguments
	///
	/// * `role` - The ARIA role to search for
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let dialog = screen.find_by_role("dialog").await;
	/// ```
	pub async fn find_by_role(&self, role: &str) -> QueryResult {
		self.find_by_role_timeout(role, 1000).await
	}

	/// Wait for an element with the given role to appear, with custom timeout.
	///
	/// # Arguments
	///
	/// * `role` - The ARIA role to search for
	/// * `timeout_ms` - Maximum time to wait in milliseconds
	pub async fn find_by_role_timeout(&self, role: &str, timeout_ms: u32) -> QueryResult {
		wait_for_query(|| self.get_by_role(role), timeout_ms).await
	}

	/// Wait for an element with the given text to appear.
	///
	/// # Arguments
	///
	/// * `text` - The text to search for
	pub async fn find_by_text(&self, text: &str) -> QueryResult {
		self.find_by_text_timeout(text, 1000).await
	}

	/// Wait for an element with the given text to appear, with custom timeout.
	///
	/// # Arguments
	///
	/// * `text` - The text to search for
	/// * `timeout_ms` - Maximum time to wait in milliseconds
	pub async fn find_by_text_timeout(&self, text: &str, timeout_ms: u32) -> QueryResult {
		let text_owned = text.to_string();
		wait_for_query(|| self.get_by_text(&text_owned), timeout_ms).await
	}

	/// Wait for an element with the given label to appear.
	///
	/// # Arguments
	///
	/// * `label` - The label text to search for
	pub async fn find_by_label_text(&self, label: &str) -> QueryResult {
		self.find_by_label_text_timeout(label, 1000).await
	}

	/// Wait for an element with the given label to appear, with custom timeout.
	pub async fn find_by_label_text_timeout(&self, label: &str, timeout_ms: u32) -> QueryResult {
		let label_owned = label.to_string();
		wait_for_query(|| self.get_by_label_text(&label_owned), timeout_ms).await
	}

	/// Wait for an element with the given test ID to appear.
	///
	/// # Arguments
	///
	/// * `test_id` - The test ID to search for
	pub async fn find_by_test_id(&self, test_id: &str) -> QueryResult {
		self.find_by_test_id_timeout(test_id, 1000).await
	}

	/// Wait for an element with the given test ID to appear, with custom timeout.
	pub async fn find_by_test_id_timeout(&self, test_id: &str, timeout_ms: u32) -> QueryResult {
		let test_id_owned = test_id.to_string();
		wait_for_query(|| self.get_by_test_id(&test_id_owned), timeout_ms).await
	}
}

// === Helper Functions ===

/// Convert a NodeList to a Vec of Elements.
fn node_list_to_elements(list: &NodeList) -> Vec<Element> {
	let mut elements = Vec::new();
	for i in 0..list.length() {
		if let Some(node) = list.get(i) {
			if let Ok(elem) = node.dyn_into::<Element>() {
				elements.push(elem);
			}
		}
	}
	elements
}

/// Get the accessible name of an element.
fn get_accessible_name(element: &Element) -> String {
	// Check aria-label first
	if let Some(label) = element.get_attribute("aria-label") {
		return label;
	}

	// Check aria-labelledby
	if let Some(labelledby) = element.get_attribute("aria-labelledby") {
		if let Some(window) = window() {
			if let Some(doc) = window.document() {
				let mut names = Vec::new();
				for id in labelledby.split_whitespace() {
					if let Some(label_elem) = doc.get_element_by_id(id) {
						if let Some(text) = label_elem.text_content() {
							names.push(text);
						}
					}
				}
				if !names.is_empty() {
					return names.join(" ");
				}
			}
		}
	}

	// Fall back to text content
	element.text_content().unwrap_or_default()
}

/// Find elements that contain the given text (case-insensitive).
fn find_elements_with_text(root: &Element, text_lower: &str) -> Vec<Element> {
	let mut results = Vec::new();
	find_text_recursive(root, text_lower, &mut results);
	results
}

/// Recursively find elements with matching text.
fn find_text_recursive(element: &Element, text_lower: &str, results: &mut Vec<Element>) {
	// Check if this element's direct text content matches
	let element_text = element.text_content().unwrap_or_default().to_lowercase();
	if element_text.contains(text_lower) {
		// Only add leaf-ish elements (those with the most specific match)
		let children = element.children();
		let mut child_has_text = false;
		for i in 0..children.length() {
			if let Some(child) = children.get_with_index(i) {
				let child_text = child.text_content().unwrap_or_default().to_lowercase();
				if child_text.contains(text_lower) {
					child_has_text = true;
					find_text_recursive(&child, text_lower, results);
				}
			}
		}
		if !child_has_text {
			results.push(element.clone());
		}
	}
}

/// Find elements with text matching a regex.
fn find_elements_matching_regex(root: &Element, re: &regex::Regex) -> Vec<Element> {
	let mut results = Vec::new();
	find_regex_recursive(root, re, &mut results);
	results
}

/// Recursively find elements with text matching a regex.
fn find_regex_recursive(element: &Element, re: &regex::Regex, results: &mut Vec<Element>) {
	let element_text = element.text_content().unwrap_or_default();
	if re.is_match(&element_text) {
		let children = element.children();
		let mut child_matches = false;
		for i in 0..children.length() {
			if let Some(child) = children.get_with_index(i) {
				let child_text = child.text_content().unwrap_or_default();
				if re.is_match(&child_text) {
					child_matches = true;
					find_regex_recursive(&child, re, results);
				}
			}
		}
		if !child_matches {
			results.push(element.clone());
		}
	}
}

/// Wait for a query to return results.
async fn wait_for_query<F>(query_fn: F, timeout_ms: u32) -> QueryResult
where
	F: Fn() -> QueryResult,
{
	use gloo_timers::future::TimeoutFuture;

	let interval_ms = 50u32;
	let max_attempts = timeout_ms / interval_ms;

	for _ in 0..max_attempts {
		let result = query_fn();
		if result.exists() {
			return result;
		}
		TimeoutFuture::new(interval_ms).await;
	}

	// Return final attempt (may be empty)
	query_fn()
}

/// Fixture function for creating a Screen.
///
/// This can be used with rstest's `#[fixture]` attribute.
pub fn screen() -> Screen {
	Screen::new()
}

/// Fixture function for creating a scoped Screen.
pub fn scoped_screen(root: Element) -> Screen {
	Screen::within(&root)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_query_result_empty() {
		let result = QueryResult::empty("test");
		assert!(!result.exists());
		assert_eq!(result.count(), 0);
	}

	#[test]
	fn test_query_result_description() {
		let result = QueryResult::empty("role=\"button\"");
		assert_eq!(result.description(), "role=\"button\"");
	}

	#[test]
	#[should_panic(expected = "No element found")]
	fn test_query_result_get_panics_when_empty() {
		let result = QueryResult::empty("test");
		result.get();
	}

	#[test]
	fn test_query_result_query_returns_none_when_empty() {
		let result = QueryResult::empty("test");
		assert!(result.query().is_none());
	}

	#[test]
	fn test_screen_default() {
		let screen = Screen::default();
		assert!(screen.root.is_none());
	}

	#[test]
	fn test_escape_css_selector_no_special_chars() {
		assert_eq!(escape_css_selector("button"), "button");
	}

	#[test]
	fn test_escape_css_selector_with_metacharacters() {
		assert_eq!(
			escape_css_selector("a\"] , body *{display:none}"),
			r#"a\"\] \, body \*\{display\:none\}"#
		);
	}

	#[test]
	fn test_escape_css_selector_quotes() {
		assert_eq!(
			escape_css_selector(r#"it's "quoted""#),
			r#"it\'s \"quoted\""#
		);
	}
}
