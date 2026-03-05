//! Page types for component rendering.
//!
//! This module provides the core types for representing renderable content
//! in the Reinhardt framework.
//!
//! ## Overview
//!
//! The `Page` enum is the core abstraction for all UI elements in the component system.
//! It can represent DOM elements, text nodes, fragments, or reactive content.
//!
//! ## Example
//!
//! ```ignore
//! use reinhardt_core::types::page::{Page, PageElement, IntoPage};
//!
//! let view = PageElement::new("div")
//!     .attr("class", "container")
//!     .child("Hello, World!")
//!     .into_page();
//!
//! let html = view.render_to_string();
//! ```

pub mod event;
pub mod head;
mod util;

pub use event::EventType;
pub use head::{Head, LinkTag, MetaTag, ScriptTag, StyleTag};
pub(crate) use util::html_escape;
pub use util::{BOOLEAN_ATTRS, is_boolean_attr_truthy};

use std::borrow::Cow;
use std::sync::Arc;

/// Type alias for event handler functions.
#[cfg(target_arch = "wasm32")]
pub type PageEventHandler = Arc<dyn Fn(web_sys::Event) + 'static>;

/// Dummy event type for non-WASM environments.
///
/// This type exists to maintain API compatibility between WASM and non-WASM builds.
/// In non-WASM environments, event handlers still accept an argument (this dummy type)
/// so that user code doesn't need conditional compilation for event handler signatures.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Default)]
pub struct DummyEvent;

#[cfg(not(target_arch = "wasm32"))]
impl DummyEvent {
	/// No-op method for API compatibility with web_sys::Event.
	///
	/// This method exists to maintain API compatibility between WASM and non-WASM builds.
	/// In non-WASM environments, this is a no-op.
	pub fn prevent_default(&self) {}
}

/// Type alias for event handler functions (non-WASM placeholder).
///
/// Uses `DummyEvent` to maintain API compatibility with the WASM version,
/// allowing the same event handler signatures (e.g., `|_| { ... }`) to work
/// in both WASM and non-WASM environments.
#[cfg(not(target_arch = "wasm32"))]
pub type PageEventHandler = Arc<dyn Fn(DummyEvent) + 'static>;

/// Error type for mounting views to the DOM.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MountError {
	/// Window object not available.
	NoWindow,
	/// Document object not available.
	NoDocument,
	/// Failed to create an element.
	CreateElementFailed,
	/// Failed to set an attribute.
	SetAttributeFailed,
	/// Failed to append a child element.
	AppendChildFailed,
}

impl std::fmt::Display for MountError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			MountError::NoWindow => write!(f, "Window object not available"),
			MountError::NoDocument => write!(f, "Document object not available"),
			MountError::CreateElementFailed => write!(f, "Failed to create element"),
			MountError::SetAttributeFailed => write!(f, "Failed to set attribute"),
			MountError::AppendChildFailed => write!(f, "Failed to append child"),
		}
	}
}

impl std::error::Error for MountError {}

/// Reactive conditional rendering.
///
/// This struct holds closures for condition evaluation and view generation,
/// enabling automatic DOM updates when the condition's dependencies change.
pub struct ReactiveIf {
	/// Condition closure that returns bool when called.
	/// This closure is re-evaluated whenever its Signal dependencies change.
	condition: Box<dyn Fn() -> bool + 'static>,
	/// Page to render when condition is true.
	then_view: Box<dyn Fn() -> Page + 'static>,
	/// Page to render when condition is false.
	else_view: Box<dyn Fn() -> Page + 'static>,
}

/// Reactive view that re-evaluates when Signal dependencies change.
///
/// This struct holds a single closure that generates a Page, enabling
/// automatic DOM updates when any Signal accessed within the closure changes.
pub struct Reactive {
	/// Page generation closure that returns a Page when called.
	/// This closure is re-evaluated whenever its Signal dependencies change.
	render: Box<dyn Fn() -> Page + 'static>,
}

impl std::fmt::Debug for Reactive {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Reactive")
			.field("render", &"<closure>")
			.finish()
	}
}

impl Reactive {
	/// Returns the rendered view.
	pub fn render(&self) -> Page {
		(self.render)()
	}

	/// Consumes the Reactive and returns the render closure.
	pub fn into_render(self) -> Box<dyn Fn() -> Page + 'static> {
		self.render
	}
}

impl std::fmt::Debug for ReactiveIf {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ReactiveIf")
			.field("condition", &"<closure>")
			.field("then_view", &"<closure>")
			.field("else_view", &"<closure>")
			.finish()
	}
}

impl ReactiveIf {
	/// Evaluates the condition closure.
	pub fn condition(&self) -> bool {
		(self.condition)()
	}

	/// Calls the then_view closure and returns the view.
	pub fn then_view(&self) -> Page {
		(self.then_view)()
	}

	/// Calls the else_view closure and returns the view.
	pub fn else_view(&self) -> Page {
		(self.else_view)()
	}

	/// Consumes the ReactiveIf and returns its parts.
	///
	/// Returns a tuple of (condition, then_view, else_view) closures.
	#[allow(clippy::type_complexity)] // Tuple decomposition is intentional for destructuring
	pub fn into_parts(
		self,
	) -> (
		Box<dyn Fn() -> bool + 'static>,
		Box<dyn Fn() -> Page + 'static>,
		Box<dyn Fn() -> Page + 'static>,
	) {
		(self.condition, self.then_view, self.else_view)
	}
}

/// A unified representation of renderable content.
///
/// Page is the core abstraction for all UI elements in the component system.
/// It can represent DOM elements, text nodes, fragments, or reactive content.
#[derive(Debug)]
pub enum Page {
	/// A DOM element.
	Element(PageElement),
	/// A text node.
	Text(Cow<'static, str>),
	/// A fragment containing multiple views (no wrapper element).
	Fragment(Vec<Page>),
	/// An empty view (renders nothing).
	Empty,
	/// A view with associated head section.
	///
	/// This variant allows components to declare their own `<head>` requirements
	/// (title, meta tags, stylesheets, etc.) that will be collected during SSR.
	WithHead {
		/// The head section for this view.
		head: Head,
		/// The actual view content.
		view: Box<Page>,
	},
	/// A reactive conditional view.
	///
	/// This variant enables automatic DOM updates when the condition's
	/// Signal dependencies change. The condition is re-evaluated and
	/// the appropriate branch is rendered.
	ReactiveIf(ReactiveIf),
	/// A reactive view that re-renders when Signal dependencies change.
	///
	/// This variant wraps any expression in a reactive context, enabling
	/// automatic DOM updates when Signal values accessed within the
	/// closure change.
	Reactive(Reactive),
}

/// Represents a DOM element in the view tree.
pub struct PageElement {
	/// The tag name (e.g., "div", "span").
	tag: Cow<'static, str>,
	/// HTML attributes.
	attrs: Vec<(Cow<'static, str>, Cow<'static, str>)>,
	/// Child views.
	children: Vec<Page>,
	/// Whether this is a void element (no closing tag).
	is_void: bool,
	/// Event handlers attached to this element.
	event_handlers: Vec<(EventType, PageEventHandler)>,
}

impl std::fmt::Debug for PageElement {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("PageElement")
			.field("tag", &self.tag)
			.field("attrs", &self.attrs)
			.field("children", &self.children)
			.field("is_void", &self.is_void)
			.field("event_handlers_count", &self.event_handlers.len())
			.finish()
	}
}

impl PageElement {
	/// Creates a new element view.
	pub fn new(tag: impl Into<Cow<'static, str>>) -> Self {
		let tag = tag.into();
		let is_void = matches!(
			tag.as_ref(),
			"area"
				| "base" | "br"
				| "col" | "embed"
				| "hr" | "img"
				| "input" | "link"
				| "meta" | "source"
				| "track" | "wbr"
		);
		Self {
			tag,
			attrs: Vec::new(),
			children: Vec::new(),
			is_void,
			event_handlers: Vec::new(),
		}
	}

	/// Adds an attribute.
	pub fn attr(
		mut self,
		name: impl Into<Cow<'static, str>>,
		value: impl Into<Cow<'static, str>>,
	) -> Self {
		self.attrs.push((name.into(), value.into()));
		self
	}

	/// Adds a boolean attribute.
	///
	/// Boolean attributes in HTML are either present (true) or absent (false).
	/// When true, the attribute is added with the attribute name as its value
	/// (e.g., `disabled="disabled"`). When false, the attribute is not added.
	///
	/// # Example
	///
	/// ```ignore
	/// PageElement::new("button")
	///     .bool_attr("disabled", is_disabled)
	///     .child("Click me")
	/// ```
	pub fn bool_attr(self, name: impl Into<Cow<'static, str>>, value: bool) -> Self {
		if value {
			let name = name.into();
			// Boolean attributes use the attribute name as value (e.g., disabled="disabled")
			self.attr(name.clone(), name)
		} else {
			self
		}
	}

	/// Adds a child view.
	pub fn child(mut self, child: impl IntoPage) -> Self {
		self.children.push(child.into_page());
		self
	}

	/// Adds multiple child views.
	pub fn children(mut self, children: impl IntoIterator<Item = impl IntoPage>) -> Self {
		self.children
			.extend(children.into_iter().map(|c| c.into_page()));
		self
	}

	/// Adds an event handler.
	pub fn on(mut self, event_type: EventType, handler: PageEventHandler) -> Self {
		self.event_handlers.push((event_type, handler));
		self
	}

	/// Adds an event listener using string event name (convenience method).
	///
	/// This is a convenience wrapper around [`on`] that accepts a string event name
	/// and a closure. The event name is parsed to [`EventType`] at runtime.
	///
	/// # Arguments
	///
	/// * `event_name` - The event name (e.g., "click", "submit", "input")
	/// * `handler` - The event handler closure
	///
	/// # Panics
	///
	/// Panics if the event name is not a recognized event type.
	///
	/// # Example
	///
	/// ```ignore
	/// PageElement::new("button")
	///     .listener("click", |event| {
	///         console::log_1(&"Button clicked!".into());
	///     })
	/// ```
	#[cfg(target_arch = "wasm32")]
	pub fn listener<F>(self, event_name: &str, handler: F) -> Self
	where
		F: Fn(web_sys::Event) + 'static,
	{
		use std::str::FromStr;
		let event_type = EventType::from_str(event_name)
			.unwrap_or_else(|_| panic!("Unknown event type: {}", event_name));
		self.on(event_type, Arc::new(handler))
	}

	/// Adds an event listener using string event name (non-WASM stub).
	///
	/// In non-WASM environments, this is a stub that stores the handler
	/// for API compatibility but won't actually attach to DOM events.
	#[cfg(not(target_arch = "wasm32"))]
	pub fn listener<F>(self, event_name: &str, handler: F) -> Self
	where
		F: Fn(DummyEvent) + Send + Sync + 'static,
	{
		use std::str::FromStr;
		let event_type = EventType::from_str(event_name)
			.unwrap_or_else(|_| panic!("Unknown event type: {}", event_name));
		self.on(event_type, Arc::new(handler))
	}

	/// Returns the tag name.
	pub fn tag_name(&self) -> &str {
		&self.tag
	}

	/// Returns the attributes.
	pub fn attrs(&self) -> &[(Cow<'static, str>, Cow<'static, str>)] {
		&self.attrs
	}

	/// Returns the child views.
	pub fn child_views(&self) -> &[Page] {
		&self.children
	}

	/// Returns whether this is a void element.
	pub fn is_void(&self) -> bool {
		self.is_void
	}

	/// Adds an attribute mutably (for parser use).
	pub fn add_attr(
		&mut self,
		name: impl Into<Cow<'static, str>>,
		value: impl Into<Cow<'static, str>>,
	) {
		self.attrs.push((name.into(), value.into()));
	}

	/// Adds a child mutably (for parser use).
	pub fn add_child(&mut self, child: impl IntoPage) {
		self.children.push(child.into_page());
	}

	/// Adds an event handler mutably (for parser use).
	pub fn add_event_handler(&mut self, event_type: EventType, handler: PageEventHandler) {
		self.event_handlers.push((event_type, handler));
	}

	/// Returns the event handlers.
	pub fn event_handlers(&self) -> &[(EventType, PageEventHandler)] {
		&self.event_handlers
	}

	/// Consumes the element view and returns the children.
	pub fn into_children(self) -> Vec<Page> {
		self.children
	}

	/// Consumes the element view and returns the event handlers.
	pub fn into_event_handlers(self) -> Vec<(EventType, PageEventHandler)> {
		self.event_handlers
	}

	/// Consumes the element view and returns all parts.
	///
	/// Returns a tuple of (tag, attrs, children, is_void, event_handlers).
	#[allow(clippy::type_complexity)] // Tuple decomposition is intentional for destructuring
	pub fn into_parts(
		self,
	) -> (
		Cow<'static, str>,
		Vec<(Cow<'static, str>, Cow<'static, str>)>,
		Vec<Page>,
		bool,
		Vec<(EventType, PageEventHandler)>,
	) {
		(
			self.tag,
			self.attrs,
			self.children,
			self.is_void,
			self.event_handlers,
		)
	}
}

impl Page {
	/// Creates an element view.
	pub fn element(tag: impl Into<Cow<'static, str>>) -> PageElement {
		PageElement::new(tag)
	}

	/// Creates a text view.
	pub fn text(content: impl Into<Cow<'static, str>>) -> Self {
		Self::Text(content.into())
	}

	/// Creates a fragment view.
	pub fn fragment(children: impl IntoIterator<Item = impl IntoPage>) -> Self {
		Self::Fragment(children.into_iter().map(|c| c.into_page()).collect())
	}

	/// Creates an empty view.
	pub fn empty() -> Self {
		Self::Empty
	}

	/// Attaches a head section to this view.
	///
	/// The head section contains metadata like title, meta tags, stylesheets,
	/// and scripts that should be included in the HTML `<head>` element.
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_core::types::page::{Page, Head};
	///
	/// let view = Page::text("Hello, World!");
	/// let head = Head::new().title("My Page");
	/// let view_with_head = view.with_head(head);
	/// ```
	pub fn with_head(self, head: Head) -> Self {
		Page::WithHead {
			head,
			view: Box::new(self),
		}
	}

	/// Creates a reactive conditional view.
	///
	/// The condition is re-evaluated whenever its Signal dependencies change,
	/// and the DOM is automatically updated to show the appropriate branch.
	///
	/// # Arguments
	///
	/// * `condition` - A closure that returns `true` or `false`. This closure
	///   should call `.get()` on Signals to establish reactive dependencies.
	/// * `then_view` - A closure that returns the Page to render when condition is true.
	/// * `else_view` - A closure that returns the Page to render when condition is false.
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_core::types::page::Page;
	/// use reinhardt_pages::reactive::hooks::use_state;
	///
	/// let (show_error, set_show_error) = use_state(false);
	///
	/// let view = Page::reactive_if(
	///     move || show_error.get(),
	///     move || Page::text("Error occurred!"),
	///     move || Page::text("All good!"),
	/// );
	/// ```
	pub fn reactive_if<C, T, E>(condition: C, then_view: T, else_view: E) -> Self
	where
		C: Fn() -> bool + 'static,
		T: Fn() -> Page + 'static,
		E: Fn() -> Page + 'static,
	{
		Page::ReactiveIf(ReactiveIf {
			condition: Box::new(condition),
			then_view: Box::new(then_view),
			else_view: Box::new(else_view),
		})
	}

	/// Creates a reactive view that re-renders when Signal dependencies change.
	///
	/// This wraps any view-generating closure in a reactive context. When any
	/// Signal accessed within the closure changes, the closure is re-evaluated
	/// and the DOM is updated accordingly.
	///
	/// # Arguments
	///
	/// * `render` - A closure that returns a `Page`. This closure will be
	///   re-evaluated whenever its Signal dependencies change.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let (count, set_count) = use_state(0);
	///
	/// Page::reactive(move || {
	///     if count.get() > 0 {
	///         Page::text(format!("Count: {}", count.get()))
	///     } else {
	///         Page::text("No count yet")
	///     }
	/// })
	/// ```
	pub fn reactive<F>(render: F) -> Self
	where
		F: Fn() -> Page + 'static,
	{
		Page::Reactive(Reactive {
			render: Box::new(render),
		})
	}

	/// Extracts the head section from this view if it has one.
	///
	/// Returns `Some(&Head)` if this view is a `WithHead` variant,
	/// or `None` for other variants.
	pub fn extract_head(&self) -> Option<&Head> {
		match self {
			Page::WithHead { head, .. } => Some(head),
			_ => None,
		}
	}

	/// Finds the topmost head section in the view tree.
	///
	/// This method searches the view tree from the root and returns the first
	/// head section found. This ensures that the outermost (page-level) head
	/// takes precedence over inner component heads.
	///
	/// # Search Order
	///
	/// 1. If this view is a `WithHead`, returns its head
	/// 2. For `Fragment` views, searches children in order and returns the first found
	/// 3. For other variants, returns `None`
	pub fn find_topmost_head(&self) -> Option<&Head> {
		match self {
			Page::WithHead { head, .. } => Some(head),
			Page::Fragment(children) => children.iter().find_map(|v| v.find_topmost_head()),
			_ => None,
		}
	}

	/// Renders the view to an HTML string.
	///
	/// This is the core SSR method that converts the view tree to HTML.
	pub fn render_to_string(&self) -> String {
		let mut output = String::new();
		self.render_to_string_inner(&mut output);
		output
	}

	fn render_to_string_inner(&self, output: &mut String) {
		match self {
			Page::Element(el) => {
				output.push('<');
				output.push_str(el.tag_name());

				for (name, value) in el.attrs() {
					// Skip boolean attributes with falsy values (empty, "false", "0")
					let name_str: &str = name.as_ref();
					if BOOLEAN_ATTRS.contains(&name_str) && !is_boolean_attr_truthy(value) {
						continue;
					}

					output.push(' ');
					output.push_str(name);
					output.push_str("=\"");
					output.push_str(&html_escape(value));
					output.push('"');
				}

				if el.is_void() {
					output.push_str(" />");
				} else {
					output.push('>');
					for child in el.child_views() {
						child.render_to_string_inner(output);
					}
					output.push_str("</");
					output.push_str(el.tag_name());
					output.push('>');
				}
			}
			Page::Text(text) => {
				output.push_str(&html_escape(text));
			}
			Page::Fragment(children) => {
				for child in children {
					child.render_to_string_inner(output);
				}
			}
			Page::Empty => {}
			Page::WithHead { view, .. } => {
				// The head is extracted separately during SSR; here we just render the content
				view.render_to_string_inner(output);
			}
			Page::ReactiveIf(reactive_if) => {
				// For SSR, evaluate condition once and render the appropriate branch
				let condition_result = (reactive_if.condition)();
				let view = if condition_result {
					(reactive_if.then_view)()
				} else {
					(reactive_if.else_view)()
				};
				view.render_to_string_inner(output);
			}
			Page::Reactive(reactive) => {
				// For SSR, evaluate render once and render the result
				let view = reactive.render();
				view.render_to_string_inner(output);
			}
		}
	}
}

/// Trait for types that can be converted into a Page.
///
/// This is the primary abstraction for renderable content.
/// Implementing this trait allows any type to be used in the view tree.
pub trait IntoPage {
	/// Converts self into a Page.
	fn into_page(self) -> Page;
}

// Core implementations

impl IntoPage for Page {
	fn into_page(self) -> Page {
		self
	}
}

impl IntoPage for PageElement {
	fn into_page(self) -> Page {
		Page::Element(self)
	}
}

impl IntoPage for String {
	fn into_page(self) -> Page {
		Page::Text(Cow::Owned(self))
	}
}

impl IntoPage for &String {
	fn into_page(self) -> Page {
		Page::Text(Cow::Owned(self.clone()))
	}
}

impl IntoPage for &'static str {
	fn into_page(self) -> Page {
		Page::Text(Cow::Borrowed(self))
	}
}

impl<T: IntoPage> IntoPage for Option<T> {
	fn into_page(self) -> Page {
		match self {
			Some(v) => v.into_page(),
			None => Page::Empty,
		}
	}
}

impl<T: IntoPage> IntoPage for Vec<T> {
	fn into_page(self) -> Page {
		Page::Fragment(self.into_iter().map(|v| v.into_page()).collect())
	}
}

impl IntoPage for () {
	fn into_page(self) -> Page {
		Page::Empty
	}
}

// Tuple implementations for fragments

impl<A: IntoPage, B: IntoPage> IntoPage for (A, B) {
	fn into_page(self) -> Page {
		Page::Fragment(vec![self.0.into_page(), self.1.into_page()])
	}
}

impl<A: IntoPage, B: IntoPage, C: IntoPage> IntoPage for (A, B, C) {
	fn into_page(self) -> Page {
		Page::Fragment(vec![
			self.0.into_page(),
			self.1.into_page(),
			self.2.into_page(),
		])
	}
}

impl<A: IntoPage, B: IntoPage, C: IntoPage, D: IntoPage> IntoPage for (A, B, C, D) {
	fn into_page(self) -> Page {
		Page::Fragment(vec![
			self.0.into_page(),
			self.1.into_page(),
			self.2.into_page(),
			self.3.into_page(),
		])
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_element_view_creation() {
		let el = PageElement::new("div");
		assert_eq!(el.tag, "div");
		assert!(!el.is_void);
		assert!(el.attrs.is_empty());
		assert!(el.children.is_empty());
	}

	#[test]
	fn test_void_element_detection() {
		assert!(PageElement::new("br").is_void);
		assert!(PageElement::new("img").is_void);
		assert!(PageElement::new("input").is_void);
		assert!(!PageElement::new("div").is_void);
		assert!(!PageElement::new("span").is_void);
	}

	#[test]
	fn test_element_with_attrs() {
		let el = PageElement::new("div")
			.attr("class", "container")
			.attr("id", "main");
		assert_eq!(el.attrs.len(), 2);
	}

	#[test]
	fn test_element_with_children() {
		let el = PageElement::new("div").child("Hello").child("World");
		assert_eq!(el.children.len(), 2);
	}

	#[test]
	fn test_render_simple_element() {
		let view = PageElement::new("div").into_page();
		assert_eq!(view.render_to_string(), "<div></div>");
	}

	#[test]
	fn test_render_element_with_attrs() {
		let view = PageElement::new("div")
			.attr("class", "container")
			.attr("id", "main")
			.into_page();
		let html = view.render_to_string();
		assert!(html.contains("class=\"container\""));
		assert!(html.contains("id=\"main\""));
	}

	#[test]
	fn test_render_void_element() {
		let view = PageElement::new("br").into_page();
		assert_eq!(view.render_to_string(), "<br />");
	}

	#[test]
	fn test_render_element_with_children() {
		let view = PageElement::new("div")
			.child("Hello, ")
			.child(PageElement::new("strong").child("World"))
			.into_page();
		assert_eq!(
			view.render_to_string(),
			"<div>Hello, <strong>World</strong></div>"
		);
	}

	#[test]
	fn test_render_text() {
		let view = Page::text("Hello");
		assert_eq!(view.render_to_string(), "Hello");
	}

	#[test]
	fn test_render_text_with_escaping() {
		let view = Page::text("<script>alert('xss')</script>");
		assert_eq!(
			view.render_to_string(),
			"&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
		);
	}

	#[test]
	fn test_render_fragment() {
		let view = Page::fragment(["One", "Two", "Three"]);
		assert_eq!(view.render_to_string(), "OneTwoThree");
	}

	#[test]
	fn test_render_empty() {
		let view = Page::empty();
		assert_eq!(view.render_to_string(), "");
	}

	#[test]
	fn test_into_page_string() {
		let view = "Hello".into_page();
		assert_eq!(view.render_to_string(), "Hello");
	}

	#[test]
	fn test_into_page_option_some() {
		let view: Page = Some("Hello").into_page();
		assert_eq!(view.render_to_string(), "Hello");
	}

	#[test]
	fn test_into_page_option_none() {
		let view: Page = None::<String>.into_page();
		assert_eq!(view.render_to_string(), "");
	}

	#[test]
	fn test_into_page_vec() {
		let view = vec!["A", "B", "C"].into_page();
		assert_eq!(view.render_to_string(), "ABC");
	}

	#[test]
	fn test_into_page_tuple() {
		let view = ("Hello, ", "World!").into_page();
		assert_eq!(view.render_to_string(), "Hello, World!");
	}

	#[test]
	fn test_html_escape() {
		assert_eq!(html_escape("Hello"), Cow::Borrowed("Hello"));
		assert_eq!(
			html_escape("<div>"),
			Cow::<str>::Owned("&lt;div&gt;".to_string())
		);
		assert_eq!(
			html_escape("a & b"),
			Cow::<str>::Owned("a &amp; b".to_string())
		);
	}

	#[test]
	fn test_nested_elements() {
		let view = PageElement::new("html")
			.child(PageElement::new("head").child(PageElement::new("title").child("Test Page")))
			.child(
				PageElement::new("body")
					.child(PageElement::new("h1").child("Hello"))
					.child(PageElement::new("p").child("World")),
			)
			.into_page();

		let html = view.render_to_string();
		assert!(html.starts_with("<html>"));
		assert!(html.contains("<title>Test Page</title>"));
		assert!(html.contains("<h1>Hello</h1>"));
		assert!(html.ends_with("</html>"));
	}

	// Boolean attribute handling tests

	#[test]
	fn test_is_boolean_attr_truthy() {
		// Truthy values
		assert!(is_boolean_attr_truthy("true"));
		assert!(is_boolean_attr_truthy("1"));
		assert!(is_boolean_attr_truthy("disabled"));
		assert!(is_boolean_attr_truthy("yes"));

		// Falsy values
		assert!(!is_boolean_attr_truthy(""));
		assert!(!is_boolean_attr_truthy("false"));
		assert!(!is_boolean_attr_truthy("0"));
	}

	#[test]
	fn test_boolean_attr_disabled_empty_string_not_rendered() {
		// Empty string should NOT render the disabled attribute
		let view = PageElement::new("button").attr("disabled", "").into_page();
		let html = view.render_to_string();
		assert_eq!(html, "<button></button>");
		assert!(!html.contains("disabled"));
	}

	#[test]
	fn test_boolean_attr_disabled_false_not_rendered() {
		// "false" should NOT render the disabled attribute
		let view = PageElement::new("button")
			.attr("disabled", "false")
			.into_page();
		let html = view.render_to_string();
		assert_eq!(html, "<button></button>");
		assert!(!html.contains("disabled"));
	}

	#[test]
	fn test_boolean_attr_disabled_zero_not_rendered() {
		// "0" should NOT render the disabled attribute
		let view = PageElement::new("button").attr("disabled", "0").into_page();
		let html = view.render_to_string();
		assert_eq!(html, "<button></button>");
		assert!(!html.contains("disabled"));
	}

	#[test]
	fn test_boolean_attr_disabled_true_rendered() {
		// "true" should render the disabled attribute
		let view = PageElement::new("button")
			.attr("disabled", "true")
			.into_page();
		let html = view.render_to_string();
		assert!(html.contains("disabled=\"true\""));
	}

	#[test]
	fn test_boolean_attr_checked_empty_not_rendered() {
		// Empty string should NOT render the checked attribute
		let view = PageElement::new("input")
			.attr("type", "checkbox")
			.attr("checked", "")
			.into_page();
		let html = view.render_to_string();
		assert!(html.contains("type=\"checkbox\""));
		assert!(!html.contains("checked"));
	}

	#[test]
	fn test_boolean_attr_checked_true_rendered() {
		// "true" should render the checked attribute
		let view = PageElement::new("input")
			.attr("type", "checkbox")
			.attr("checked", "true")
			.into_page();
		let html = view.render_to_string();
		assert!(html.contains("checked=\"true\""));
	}

	#[test]
	fn test_non_boolean_attr_empty_string_rendered() {
		// Non-boolean attributes with empty string SHOULD be rendered
		let view = PageElement::new("input")
			.attr("placeholder", "")
			.into_page();
		let html = view.render_to_string();
		assert!(html.contains("placeholder=\"\""));
	}

	#[test]
	fn test_non_boolean_attr_false_rendered() {
		// Non-boolean attributes with "false" SHOULD be rendered as-is
		let view = PageElement::new("div")
			.attr("data-active", "false")
			.into_page();
		let html = view.render_to_string();
		assert!(html.contains("data-active=\"false\""));
	}

	#[test]
	fn test_multiple_boolean_attrs_mixed() {
		// Mix of boolean and non-boolean attributes
		let view = PageElement::new("input")
			.attr("type", "text")
			.attr("disabled", "")      // Should NOT be rendered
			.attr("readonly", "true")  // Should be rendered
			.attr("required", "false") // Should NOT be rendered
			.attr("placeholder", "")   // Should be rendered (non-boolean)
			.into_page();
		let html = view.render_to_string();

		assert!(html.contains("type=\"text\""));
		assert!(!html.contains("disabled"));
		assert!(html.contains("readonly=\"true\""));
		assert!(!html.contains("required"));
		assert!(html.contains("placeholder=\"\""));
	}
}
