//! Element Wrapper
//!
//! Provides a safe, ergonomic wrapper around `web_sys::Element`.
//!
//! ## Key Features
//!
//! - **Attribute Management**: Type-safe attribute operations
//! - **Event Listeners**: RAII-based automatic cleanup via `EventHandle`
//! - **Reactive Binding**: Integration with Signal system for automatic updates
//!
//! ## RAII Pattern for Event Listeners
//!
//! Event listeners are automatically removed when `EventHandle` is dropped:
//!
//! ```ignore
//! let element = document().create_element("button")?;
//! {
//!     let handle = element.add_event_listener("click", || {
//!         console::log_1(&"Clicked!".into());
//!     });
//!     // handle.drop() is called here, removing the event listener
//! }
//! ```

use wasm_bindgen::{JsCast, closure::Closure};
use web_sys;

use crate::reactive::{Effect, Signal};

/// Thin wrapper around `web_sys::Element`
///
/// This struct provides a safe, ergonomic API for DOM manipulation while
/// maintaining compatibility with the underlying web-sys types.
pub struct Element {
	/// The underlying web-sys Element
	inner: web_sys::Element,
	/// Event handles for RAII cleanup
	///
	/// These handles keep event listeners alive as long as the Element exists.
	/// When the Element is dropped, the handles are dropped too, automatically
	/// removing the event listeners from the DOM.
	event_handles: Vec<EventHandle>,
}

impl Clone for Element {
	/// Clone the element reference without cloning event handles.
	///
	/// Cloned elements share the same underlying DOM element but do not
	/// take ownership of event handles. The original Element retains
	/// ownership of all event handles.
	fn clone(&self) -> Self {
		Self {
			inner: self.inner.clone(),
			event_handles: Vec::new(),
		}
	}
}

impl Element {
	/// Create a new Element wrapper from a web-sys Element
	///
	/// # Arguments
	///
	/// * `element` - The web-sys Element to wrap
	///
	/// # Example
	///
	/// ```ignore
	/// use web_sys;
	/// use reinhardt_pages::dom::Element;
	///
	/// let web_element: web_sys::Element = /* ... */;
	/// let element = Element::new(web_element);
	/// ```
	pub fn new(element: web_sys::Element) -> Self {
		Self {
			inner: element,
			event_handles: Vec::new(),
		}
	}

	/// Get a reference to the underlying web-sys Element
	///
	/// This is useful when you need to pass the element to web-sys APIs directly.
	pub fn as_web_sys(&self) -> &web_sys::Element {
		&self.inner
	}

	/// Consume self and return the underlying web-sys Element
	pub fn into_web_sys(self) -> web_sys::Element {
		self.inner
	}

	/// Store event handles to keep event listeners alive.
	///
	/// Event handles use RAII to automatically remove event listeners when
	/// dropped. By storing them in the Element, the listeners remain active
	/// as long as the Element itself is alive.
	pub fn store_event_handles(&mut self, handles: Vec<EventHandle>) {
		self.event_handles.extend(handles);
	}

	/// Get the number of stored event handles.
	///
	/// Useful for verifying that event handles have been properly transferred
	/// to the element.
	pub fn event_handle_count(&self) -> usize {
		self.event_handles.len()
	}

	/// Set an attribute on this element
	///
	/// # Arguments
	///
	/// * `name` - Attribute name
	/// * `value` - Attribute value
	///
	/// # Example
	///
	/// ```ignore
	/// element.set_attribute("class", "btn btn-primary")?;
	/// element.set_attribute("id", "submit-button")?;
	/// ```
	pub fn set_attribute(&self, name: &str, value: &str) -> Result<(), String> {
		self.inner
			.set_attribute(name, value)
			.map_err(|e| format!("Failed to set attribute '{}': {:?}", name, e))
	}

	/// Get an attribute from this element
	///
	/// Returns `None` if the attribute doesn't exist.
	///
	/// # Arguments
	///
	/// * `name` - Attribute name
	///
	/// # Example
	///
	/// ```ignore
	/// if let Some(class) = element.get_attribute("class")? {
	///     println!("Element has class: {}", class);
	/// }
	/// ```
	pub fn get_attribute(&self, name: &str) -> Option<String> {
		self.inner.get_attribute(name)
	}

	/// Remove an attribute from this element
	///
	/// # Arguments
	///
	/// * `name` - Attribute name
	///
	/// # Example
	///
	/// ```ignore
	/// element.remove_attribute("disabled")?;
	/// ```
	pub fn remove_attribute(&self, name: &str) -> Result<(), String> {
		self.inner
			.remove_attribute(name)
			.map_err(|e| format!("Failed to remove attribute '{}': {:?}", name, e))
	}

	/// Set a reactive attribute that automatically updates when a Signal changes
	///
	/// This creates an Effect that tracks the Signal and updates the attribute
	/// whenever the Signal's value changes.
	///
	/// # Arguments
	///
	/// * `name` - Attribute name
	/// * `signal` - Signal to bind to
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_pages::{reactive::Signal, dom::Element};
	///
	/// let count = Signal::new(0);
	/// let element = document().create_element("div")?;
	///
	/// // Attribute automatically updates when count changes
	/// element.set_reactive_attribute("data-count", count.clone());
	///
	/// count.set(42); // Attribute is now "42"
	/// ```
	///
	/// # Note
	///
	/// The Effect is not returned, so it will live for the duration of the program.
	/// For fine-grained control, use Effect::new() directly.
	pub fn set_reactive_attribute<T>(&self, name: &str, signal: Signal<T>)
	where
		T: ToString + Clone + 'static,
	{
		let element = self.clone();
		let name = name.to_string();

		Effect::new(move || {
			let value = signal.get().to_string();
			let _ = element.set_attribute(&name, &value);
		});
	}

	/// Add an event listener to this element
	///
	/// Returns an `EventHandle` that automatically removes the listener when dropped.
	///
	/// # Arguments
	///
	/// * `event_type` - Event type (e.g., "click", "input")
	/// * `callback` - Closure to call when event fires
	///
	/// # Example
	///
	/// ```ignore
	/// let handle = element.add_event_listener("click", || {
	///     console::log_1(&"Button clicked!".into());
	/// });
	///
	/// // Keep `handle` alive as long as you want the listener active
	/// // When `handle` is dropped, the listener is automatically removed
	/// ```
	pub fn add_event_listener<F>(&self, event_type: &str, mut callback: F) -> EventHandle
	where
		F: FnMut() + 'static,
	{
		let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
			callback();
		}) as Box<dyn FnMut(web_sys::Event)>);

		self.inner
			.add_event_listener_with_callback(event_type, closure.as_ref().unchecked_ref())
			.expect("Failed to add event listener");

		EventHandle {
			element: self.inner.clone(),
			event_type: event_type.to_string(),
			closure: Some(closure),
		}
	}

	/// Add an event listener that receives the event object
	///
	/// Returns an `EventHandle` that automatically removes the listener when dropped.
	///
	/// # Arguments
	///
	/// * `event_type` - Event type (e.g., "click", "input")
	/// * `callback` - Closure to call when event fires, receives the event object
	///
	/// # Example
	///
	/// ```ignore
	/// let handle = element.add_event_listener_with_event("click", |event| {
	///     console::log_2(&"Event:".into(), &event);
	/// });
	/// ```
	pub fn add_event_listener_with_event<F>(&self, event_type: &str, mut callback: F) -> EventHandle
	where
		F: FnMut(web_sys::Event) + 'static,
	{
		let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
			callback(event);
		}) as Box<dyn FnMut(web_sys::Event)>);

		self.inner
			.add_event_listener_with_callback(event_type, closure.as_ref().unchecked_ref())
			.expect("Failed to add event listener");

		EventHandle {
			element: self.inner.clone(),
			event_type: event_type.to_string(),
			closure: Some(closure),
		}
	}

	/// Set text content of this element
	///
	/// # Arguments
	///
	/// * `text` - Text content to set
	///
	/// # Example
	///
	/// ```ignore
	/// element.set_text_content("Hello, World!");
	/// ```
	pub fn set_text_content(&self, text: &str) {
		self.inner.set_text_content(Some(text));
	}

	/// Get text content of this element
	///
	/// # Example
	///
	/// ```ignore
	/// let text = element.text_content();
	/// println!("Element text: {:?}", text);
	/// ```
	pub fn text_content(&self) -> Option<String> {
		self.inner.text_content()
	}

	/// Append a child element to this element
	///
	/// # Arguments
	///
	/// * `child` - Child element to append
	///
	/// # Example
	///
	/// ```ignore
	/// let parent = document().create_element("div")?;
	/// let child = document().create_element("p")?;
	/// parent.append_child(child)?;
	/// ```
	pub fn append_child(&self, child: Element) -> Result<(), String> {
		self.inner
			.append_child(&child.inner)
			.map_err(|e| format!("Failed to append child: {:?}", e))?;
		Ok(())
	}

	/// Get all child elements
	///
	/// Returns a vector of child elements, excluding text nodes.
	///
	/// # Returns
	///
	/// A vector of child elements. Returns an empty vector if there are no children.
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_pages::dom::document;
	///
	/// let doc = document();
	/// let parent = doc.create_element("div")?;
	/// let child = doc.create_element("span")?;
	/// parent.append_child(child)?;
	///
	/// let children = parent.children();
	/// assert_eq!(children.len(), 1);
	/// ```
	pub fn children(&self) -> Vec<Element> {
		let collection = self.inner.children();
		(0..collection.length())
			.filter_map(|i| collection.item(i))
			.map(Element::new)
			.collect()
	}

	/// Get the tag name of this element
	///
	/// Returns the tag name in uppercase (e.g., "DIV", "SPAN").
	/// Use `.to_lowercase()` on the result if you need lowercase.
	///
	/// # Returns
	///
	/// The tag name as a String.
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_pages::dom::document;
	///
	/// let doc = document();
	/// let element = doc.create_element("div")?;
	/// assert_eq!(element.tag_name().to_lowercase(), "div");
	/// ```
	pub fn tag_name(&self) -> String {
		self.inner.tag_name()
	}

	/// Get a reference to the underlying web-sys Element
	///
	/// This method provides direct access to the wrapped `web_sys::Element`
	/// for cases where you need to use web-sys APIs directly.
	///
	/// # Returns
	///
	/// A reference to the underlying `web_sys::Element`.
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_pages::dom::document;
	///
	/// let doc = document();
	/// let element = doc.create_element("button")?;
	///
	/// // Access web_sys API directly
	/// let web_element = element.inner();
	/// ```
	///
	/// # Note
	///
	/// This method is identical to `as_web_sys()` but provided for API consistency.
	pub fn inner(&self) -> &web_sys::Element {
		&self.inner
	}

	/// Get the parent element of this element.
	///
	/// Returns `None` if there is no parent element.
	///
	/// # Returns
	///
	/// An optional parent `Element`.
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_pages::dom::document;
	///
	/// let doc = document();
	/// let parent = doc.create_element("div")?;
	/// let child = doc.create_element("span")?;
	/// parent.append_child(child.clone())?;
	///
	/// let parent_elem = child.parent_element();
	/// assert!(parent_elem.is_some());
	/// ```
	pub fn parent_element(&self) -> Option<Element> {
		self.inner.parent_element().map(Element::new)
	}

	/// Get the next sibling element.
	///
	/// Returns `None` if there is no next sibling element.
	///
	/// # Returns
	///
	/// An optional next sibling `Element`.
	///
	/// # Example
	///
	/// ```ignore
	/// let sibling = element.next_element_sibling();
	/// if let Some(next) = sibling {
	///     println!("Next sibling: {}", next.tag_name());
	/// }
	/// ```
	pub fn next_element_sibling(&self) -> Option<Element> {
		self.inner.next_element_sibling().map(Element::new)
	}

	/// Get the previous sibling element.
	///
	/// Returns `None` if there is no previous sibling element.
	///
	/// # Returns
	///
	/// An optional previous sibling `Element`.
	///
	/// # Example
	///
	/// ```ignore
	/// let sibling = element.previous_element_sibling();
	/// if let Some(prev) = sibling {
	///     println!("Previous sibling: {}", prev.tag_name());
	/// }
	/// ```
	pub fn previous_element_sibling(&self) -> Option<Element> {
		self.inner.previous_element_sibling().map(Element::new)
	}

	/// Get the first child element.
	///
	/// Returns `None` if there are no child elements.
	///
	/// # Returns
	///
	/// An optional first child `Element`.
	///
	/// # Example
	///
	/// ```ignore
	/// if let Some(first_child) = element.first_element_child() {
	///     println!("First child: {}", first_child.tag_name());
	/// }
	/// ```
	pub fn first_element_child(&self) -> Option<Element> {
		self.inner.first_element_child().map(Element::new)
	}

	/// Get the last child element.
	///
	/// Returns `None` if there are no child elements.
	///
	/// # Returns
	///
	/// An optional last child `Element`.
	///
	/// # Example
	///
	/// ```ignore
	/// if let Some(last_child) = element.last_element_child() {
	///     println!("Last child: {}", last_child.tag_name());
	/// }
	/// ```
	pub fn last_element_child(&self) -> Option<Element> {
		self.inner.last_element_child().map(Element::new)
	}

	/// Get the number of child elements.
	///
	/// # Returns
	///
	/// The count of child elements (not including text nodes).
	///
	/// # Example
	///
	/// ```ignore
	/// let count = element.child_element_count();
	/// println!("Element has {} children", count);
	/// ```
	pub fn child_element_count(&self) -> u32 {
		self.inner.child_element_count()
	}

	/// Query for the first element matching a CSS selector.
	///
	/// Returns the first descendant element that matches the specified CSS selector,
	/// or `None` if no matches are found.
	///
	/// # Arguments
	///
	/// * `selector` - A CSS selector string
	///
	/// # Returns
	///
	/// * `Ok(Some(Element))` - First matching element
	/// * `Ok(None)` - No matches found
	/// * `Err(String)` - Invalid selector or query failed
	///
	/// # Example
	///
	/// ```ignore
	/// let button = element.query_selector(".btn-primary")?;
	/// if let Some(btn) = button {
	///     btn.set_attribute("disabled", "true")?;
	/// }
	/// ```
	pub fn query_selector(&self, selector: &str) -> Result<Option<Element>, String> {
		self.inner
			.query_selector(selector)
			.map(|opt| opt.map(Element::new))
			.map_err(|e| format!("Failed to query selector '{}': {:?}", selector, e))
	}

	/// Query for all elements matching a CSS selector.
	///
	/// Returns all descendant elements that match the specified CSS selector.
	///
	/// # Arguments
	///
	/// * `selector` - A CSS selector string
	///
	/// # Returns
	///
	/// * `Ok(Vec<Element>)` - Vector of matching elements (empty if no matches)
	/// * `Err(String)` - Invalid selector or query failed
	///
	/// # Example
	///
	/// ```ignore
	/// let items = element.query_selector_all(".list-item")?;
	/// for item in items {
	///     item.add_class("processed")?;
	/// }
	/// ```
	pub fn query_selector_all(&self, selector: &str) -> Result<Vec<Element>, String> {
		use wasm_bindgen::JsCast;

		let node_list = self
			.inner
			.query_selector_all(selector)
			.map_err(|e| format!("Failed to query selector all '{}': {:?}", selector, e))?;

		Ok((0..node_list.length())
			.filter_map(|i| node_list.item(i))
			.filter_map(|node| node.dyn_into::<web_sys::Element>().ok())
			.map(Element::new)
			.collect())
	}

	/// Find the closest ancestor element matching a CSS selector.
	///
	/// Traverses the element and its parents (heading toward the document root)
	/// until it finds a node that matches the specified CSS selector.
	///
	/// # Arguments
	///
	/// * `selector` - A CSS selector string
	///
	/// # Returns
	///
	/// * `Ok(Some(Element))` - First matching ancestor (or self if it matches)
	/// * `Ok(None)` - No matches found
	/// * `Err(String)` - Invalid selector or query failed
	///
	/// # Example
	///
	/// ```ignore
	/// let form = element.closest("form")?;
	/// if let Some(form_elem) = form {
	///     form_elem.set_attribute("novalidate", "true")?;
	/// }
	/// ```
	pub fn closest(&self, selector: &str) -> Result<Option<Element>, String> {
		self.inner
			.closest(selector)
			.map(|opt| opt.map(Element::new))
			.map_err(|e| format!("Failed to find closest '{}': {:?}", selector, e))
	}

	/// Get the element's ID.
	///
	/// Returns the value of the element's `id` attribute.
	///
	/// # Returns
	///
	/// The ID as a String (empty string if no ID is set).
	///
	/// # Example
	///
	/// ```ignore
	/// let id = element.id();
	/// if !id.is_empty() {
	///     println!("Element ID: {}", id);
	/// }
	/// ```
	pub fn id(&self) -> String {
		self.inner.id()
	}

	/// Set the element's ID.
	///
	/// Sets the value of the element's `id` attribute.
	///
	/// # Arguments
	///
	/// * `id` - The ID to set
	///
	/// # Example
	///
	/// ```ignore
	/// element.set_id("my-element");
	/// assert_eq!(element.id(), "my-element");
	/// ```
	pub fn set_id(&self, id: &str) {
		self.inner.set_id(id);
	}

	/// Get the element's class list.
	///
	/// Returns a `DomTokenList` representing the element's `class` attribute.
	/// This provides methods for manipulating individual classes.
	///
	/// # Returns
	///
	/// A `web_sys::DomTokenList` for class manipulation.
	///
	/// # Example
	///
	/// ```ignore
	/// let class_list = element.class_list();
	/// class_list.add_1("active").unwrap();
	/// ```
	pub fn class_list(&self) -> web_sys::DomTokenList {
		self.inner.class_list()
	}

	/// Add a class to the element.
	///
	/// Adds the specified class to the element's class list if not already present.
	///
	/// # Arguments
	///
	/// * `class` - The class name to add
	///
	/// # Returns
	///
	/// * `Ok(())` - Class was added successfully
	/// * `Err(String)` - Failed to add class
	///
	/// # Example
	///
	/// ```ignore
	/// element.add_class("active")?;
	/// element.add_class("highlight")?;
	/// ```
	pub fn add_class(&self, class: &str) -> Result<(), String> {
		self.inner
			.class_list()
			.add_1(class)
			.map_err(|e| format!("Failed to add class '{}': {:?}", class, e))
	}

	/// Remove a class from the element.
	///
	/// Removes the specified class from the element's class list if present.
	///
	/// # Arguments
	///
	/// * `class` - The class name to remove
	///
	/// # Returns
	///
	/// * `Ok(())` - Class was removed successfully
	/// * `Err(String)` - Failed to remove class
	///
	/// # Example
	///
	/// ```ignore
	/// element.remove_class("active")?;
	/// ```
	pub fn remove_class(&self, class: &str) -> Result<(), String> {
		self.inner
			.class_list()
			.remove_1(class)
			.map_err(|e| format!("Failed to remove class '{}': {:?}", class, e))
	}

	/// Check if the element has a specific class.
	///
	/// Returns `true` if the element's class list contains the specified class.
	///
	/// # Arguments
	///
	/// * `class` - The class name to check
	///
	/// # Returns
	///
	/// `true` if the class is present, `false` otherwise.
	///
	/// # Example
	///
	/// ```ignore
	/// if element.has_class("active") {
	///     println!("Element is active");
	/// }
	/// ```
	pub fn has_class(&self, class: &str) -> bool {
		self.inner.class_list().contains(class)
	}

	/// Toggle a class on the element.
	///
	/// If the class is present, removes it. If the class is absent, adds it.
	///
	/// # Arguments
	///
	/// * `class` - The class name to toggle
	///
	/// # Returns
	///
	/// * `Ok(true)` - Class was added
	/// * `Ok(false)` - Class was removed
	/// * `Err(String)` - Failed to toggle class
	///
	/// # Example
	///
	/// ```ignore
	/// let is_active = element.toggle_class("active")?;
	/// if is_active {
	///     println!("Class was added");
	/// } else {
	///     println!("Class was removed");
	/// }
	/// ```
	pub fn toggle_class(&self, class: &str) -> Result<bool, String> {
		self.inner
			.class_list()
			.toggle(class)
			.map_err(|e| format!("Failed to toggle class '{}': {:?}", class, e))
	}
}

/// RAII wrapper for event listeners
///
/// When dropped, this handle automatically removes the event listener from the element.
/// This prevents memory leaks from forgotten event listeners.
///
/// ## Example
///
/// ```ignore
/// {
///     let handle = element.add_event_listener("click", || {
///         console::log_1(&"Clicked!".into());
///     });
///     // Listener is active here
/// } // handle.drop() called here - listener automatically removed
/// ```
#[derive(Debug)]
pub struct EventHandle {
	/// The element this listener is attached to
	element: web_sys::Element,
	/// Event type ("click", "input", etc.)
	event_type: String,
	/// The closure that handles the event
	/// Wrapped in Option so we can take() it in Drop
	closure: Option<Closure<dyn FnMut(web_sys::Event)>>,
}

impl Drop for EventHandle {
	fn drop(&mut self) {
		if let Some(closure) = self.closure.take() {
			// Remove the event listener
			let _ = self.element.remove_event_listener_with_callback(
				&self.event_type,
				closure.as_ref().unchecked_ref(),
			);
			// Closure is dropped here, cleaning up the JavaScript callback
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use wasm_bindgen_test::*;

	wasm_bindgen_test_configure!(run_in_browser);

	#[wasm_bindgen_test]
	fn test_element_set_attribute() {
		let document = web_sys::window().unwrap().document().unwrap();
		let web_element = document.create_element("div").unwrap();
		let element = Element::new(web_element);

		element.set_attribute("id", "test-div").unwrap();
		assert_eq!(element.get_attribute("id"), Some("test-div".to_string()));
	}

	#[wasm_bindgen_test]
	fn test_element_remove_attribute() {
		let document = web_sys::window().unwrap().document().unwrap();
		let web_element = document.create_element("div").unwrap();
		let element = Element::new(web_element);

		element.set_attribute("data-test", "value").unwrap();
		assert!(element.get_attribute("data-test").is_some());

		element.remove_attribute("data-test").unwrap();
		assert!(element.get_attribute("data-test").is_none());
	}

	#[wasm_bindgen_test]
	fn test_element_text_content() {
		let document = web_sys::window().unwrap().document().unwrap();
		let web_element = document.create_element("p").unwrap();
		let element = Element::new(web_element);

		element.set_text_content("Hello, World!");
		assert_eq!(element.text_content(), Some("Hello, World!".to_string()));
	}

	#[wasm_bindgen_test]
	fn test_children() {
		let document = web_sys::window().unwrap().document().unwrap();
		let parent = document.create_element("div").unwrap();
		let child1 = document.create_element("span").unwrap();
		let child2 = document.create_element("p").unwrap();

		parent.append_child(&child1).unwrap();
		parent.append_child(&child2).unwrap();

		let element = Element::new(parent);
		let children = element.children();

		assert_eq!(children.len(), 2);
		assert_eq!(children[0].tag_name().to_lowercase(), "span");
		assert_eq!(children[1].tag_name().to_lowercase(), "p");
	}

	#[wasm_bindgen_test]
	fn test_tag_name() {
		let document = web_sys::window().unwrap().document().unwrap();
		let web_element = document.create_element("div").unwrap();
		let element = Element::new(web_element);

		assert_eq!(element.tag_name().to_lowercase(), "div");
	}

	#[wasm_bindgen_test]
	fn test_inner() {
		let document = web_sys::window().unwrap().document().unwrap();
		let web_element = document.create_element("button").unwrap();
		let element = Element::new(web_element.clone());

		// inner() and as_web_sys() should return the same reference
		assert_eq!(element.inner().tag_name(), element.as_web_sys().tag_name());
	}

	#[wasm_bindgen_test]
	fn test_parent_element() {
		let document = web_sys::window().unwrap().document().unwrap();
		let parent_web = document.create_element("div").unwrap();
		let child_web = document.create_element("span").unwrap();

		parent_web.append_child(&child_web).unwrap();

		let child_element = Element::new(child_web);
		let parent_element = child_element.parent_element();

		assert!(parent_element.is_some());
		assert_eq!(parent_element.unwrap().tag_name().to_lowercase(), "div");
	}

	#[wasm_bindgen_test]
	fn test_siblings() {
		let document = web_sys::window().unwrap().document().unwrap();
		let parent = document.create_element("div").unwrap();
		let child1 = document.create_element("span").unwrap();
		let child2 = document.create_element("p").unwrap();
		let child3 = document.create_element("a").unwrap();

		parent.append_child(&child1).unwrap();
		parent.append_child(&child2).unwrap();
		parent.append_child(&child3).unwrap();

		let middle = Element::new(child2);

		let next = middle.next_element_sibling();
		assert!(next.is_some());
		assert_eq!(next.unwrap().tag_name().to_lowercase(), "a");

		let prev = middle.previous_element_sibling();
		assert!(prev.is_some());
		assert_eq!(prev.unwrap().tag_name().to_lowercase(), "span");
	}

	#[wasm_bindgen_test]
	fn test_first_and_last_child() {
		let document = web_sys::window().unwrap().document().unwrap();
		let parent_web = document.create_element("div").unwrap();
		let child1 = document.create_element("span").unwrap();
		let child2 = document.create_element("p").unwrap();
		let child3 = document.create_element("a").unwrap();

		parent_web.append_child(&child1).unwrap();
		parent_web.append_child(&child2).unwrap();
		parent_web.append_child(&child3).unwrap();

		let parent = Element::new(parent_web);

		let first = parent.first_element_child();
		assert!(first.is_some());
		assert_eq!(first.unwrap().tag_name().to_lowercase(), "span");

		let last = parent.last_element_child();
		assert!(last.is_some());
		assert_eq!(last.unwrap().tag_name().to_lowercase(), "a");
	}

	#[wasm_bindgen_test]
	fn test_child_element_count() {
		let document = web_sys::window().unwrap().document().unwrap();
		let parent_web = document.create_element("div").unwrap();
		let child1 = document.create_element("span").unwrap();
		let child2 = document.create_element("p").unwrap();

		let parent = Element::new(parent_web.clone());
		assert_eq!(parent.child_element_count(), 0);

		parent_web.append_child(&child1).unwrap();
		assert_eq!(parent.child_element_count(), 1);

		parent_web.append_child(&child2).unwrap();
		assert_eq!(parent.child_element_count(), 2);
	}

	#[wasm_bindgen_test]
	fn test_query_selector() {
		let document = web_sys::window().unwrap().document().unwrap();
		let parent_web = document.create_element("div").unwrap();
		let child = document.create_element("p").unwrap();
		child.set_class_name("test-class");

		parent_web.append_child(&child).unwrap();

		let parent = Element::new(parent_web);
		let found = parent.query_selector(".test-class").unwrap();

		assert!(found.is_some());
		assert_eq!(found.unwrap().tag_name().to_lowercase(), "p");
	}

	#[wasm_bindgen_test]
	fn test_query_selector_all() {
		let document = web_sys::window().unwrap().document().unwrap();
		let parent_web = document.create_element("div").unwrap();

		for _ in 0..3 {
			let child = document.create_element("span").unwrap();
			child.set_class_name("item");
			parent_web.append_child(&child).unwrap();
		}

		let parent = Element::new(parent_web);
		let found = parent.query_selector_all(".item").unwrap();

		assert_eq!(found.len(), 3);
		for elem in found {
			assert_eq!(elem.tag_name().to_lowercase(), "span");
		}
	}

	#[wasm_bindgen_test]
	fn test_closest() {
		let document = web_sys::window().unwrap().document().unwrap();
		let grandparent = document.create_element("div").unwrap();
		grandparent.set_class_name("container");

		let parent = document.create_element("section").unwrap();
		let child_web = document.create_element("span").unwrap();

		grandparent.append_child(&parent).unwrap();
		parent.append_child(&child_web).unwrap();

		let child = Element::new(child_web);
		let container = child.closest(".container").unwrap();

		assert!(container.is_some());
		assert_eq!(container.unwrap().tag_name().to_lowercase(), "div");
	}

	#[wasm_bindgen_test]
	fn test_id_operations() {
		let document = web_sys::window().unwrap().document().unwrap();
		let web_element = document.create_element("div").unwrap();
		let element = Element::new(web_element);

		assert_eq!(element.id(), "");

		element.set_id("my-element");
		assert_eq!(element.id(), "my-element");
	}

	#[wasm_bindgen_test]
	fn test_class_operations() {
		let document = web_sys::window().unwrap().document().unwrap();
		let web_element = document.create_element("div").unwrap();
		let element = Element::new(web_element);

		element.add_class("foo").unwrap();
		assert!(element.has_class("foo"));

		element.add_class("bar").unwrap();
		assert!(element.has_class("bar"));

		element.remove_class("foo").unwrap();
		assert!(!element.has_class("foo"));
		assert!(element.has_class("bar"));

		let toggled = element.toggle_class("baz").unwrap();
		assert!(toggled);
		assert!(element.has_class("baz"));

		let toggled_off = element.toggle_class("baz").unwrap();
		assert!(!toggled_off);
		assert!(!element.has_class("baz"));
	}
}
