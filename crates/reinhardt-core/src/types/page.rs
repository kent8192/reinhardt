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
//! ```rust
//! use reinhardt_core::types::page::{Page, PageElement, IntoPage};
//!
//! let view = PageElement::new("div")
//!     .attr("class", "container")
//!     .child("Hello, World!")
//!     .into_page();
//!
//! let html = view.render_to_string();
//! ```

/// Controlled form-element binding descriptors.
pub mod control_binding;
pub mod event;
pub mod head;
#[cfg(feature = "page-hot-reload")]
pub mod hot_reload;
#[cfg(native)]
pub mod native_event;
mod util;

pub use control_binding::{
	ControlBinding, ControlBindingError, ControlBindingSnapshot, ControlKind, ControlValue,
	ControlWriteOutcome, NumberParseError, NumberParseErrorKind, NumberValue,
};
pub use event::{EventInterface, EventName, EventType};
pub use head::{Head, LinkTag, MetaTag, ScriptTag, StyleTag};
#[cfg(feature = "page-hot-reload")]
pub use hot_reload::DevTemplateMetadata;
#[cfg(native)]
pub use native_event::*;
pub(crate) use util::html_escape;
pub use util::{BOOLEAN_ATTRS, is_boolean_attr, is_boolean_attr_truthy};

use std::borrow::Cow;
use std::sync::Arc;

/// Type alias for event handler functions.
#[cfg(wasm)]
pub type PageEventHandler = Arc<dyn Fn(web_sys::Event) + 'static>;

/// Type alias for event handler functions on native targets.
#[cfg(native)]
pub type PageEventHandler = Arc<dyn Fn(NativeEvent) + 'static>;

/// An attribute whose value is evaluated from reactive state when rendered.
#[derive(Clone)]
pub struct ReactiveAttribute {
	name: Cow<'static, str>,
	render: Arc<dyn Fn() -> Option<Cow<'static, str>> + 'static>,
}

impl std::fmt::Debug for ReactiveAttribute {
	fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter
			.debug_struct("ReactiveAttribute")
			.field("name", &self.name)
			.finish_non_exhaustive()
	}
}

impl ReactiveAttribute {
	/// Returns the attribute name.
	pub fn name(&self) -> &str {
		&self.name
	}

	/// Evaluates and returns the current attribute value.
	pub fn value(&self) -> Option<Cow<'static, str>> {
		(self.render)().filter(|value| is_safe_html_attribute(&self.name, value))
	}
}

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
	/// Failed to install a controlled form-element binding.
	ControlBinding(ControlBindingError),
}

impl std::fmt::Display for MountError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			MountError::NoWindow => write!(f, "Window object not available"),
			MountError::NoDocument => write!(f, "Document object not available"),
			MountError::CreateElementFailed => write!(f, "Failed to create element"),
			MountError::SetAttributeFailed => write!(f, "Failed to set attribute"),
			MountError::AppendChildFailed => write!(f, "Failed to append child"),
			MountError::ControlBinding(error) => write!(f, "Failed to bind control: {error}"),
		}
	}
}

impl std::error::Error for MountError {
	fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
		match self {
			Self::ControlBinding(error) => Some(error),
			_ => None,
		}
	}
}

impl From<ControlBindingError> for MountError {
	fn from(error: ControlBindingError) -> Self {
		Self::ControlBinding(error)
	}
}

/// Reactive conditional rendering.
///
/// This struct holds closures for condition evaluation and view generation,
/// enabling automatic DOM updates when the condition's dependencies change.
///
/// The closures are stored as `Arc<dyn Fn>` so the entire `ReactiveIf` (and
/// therefore the enclosing `Page`) is `Clone`. Cloning duplicates only the
/// Arc handle; both clones invoke the same underlying render closure.
#[derive(Clone)]
pub struct ReactiveIf {
	/// Condition closure that returns bool when called.
	/// This closure is re-evaluated whenever its Signal dependencies change.
	condition: std::sync::Arc<dyn Fn() -> bool + 'static>,
	/// Page to render when condition is true.
	then_view: std::sync::Arc<dyn Fn() -> Page + 'static>,
	/// Page to render when condition is false.
	else_view: std::sync::Arc<dyn Fn() -> Page + 'static>,
}

/// Reactive view that re-evaluates when Signal dependencies change.
///
/// This struct holds a single closure that generates a Page, enabling
/// automatic DOM updates when any Signal accessed within the closure changes.
///
/// The closure is stored as `Arc<dyn Fn>` so the entire `Reactive` (and
/// therefore the enclosing `Page`) is `Clone`. Cloning duplicates only the
/// Arc handle; both clones invoke the same underlying render closure.
#[derive(Clone)]
pub struct Reactive {
	/// Page generation closure that returns a Page when called.
	/// This closure is re-evaluated whenever its Signal dependencies change.
	render: std::sync::Arc<dyn Fn() -> Page + 'static>,
}

/// Suspense view node with lazy branch factories.
///
/// The branch factories are stored as `Arc<dyn Fn>` so the enclosing `Page`
/// remains cloneable while each traversal can render a fresh branch.
pub struct SuspenseNode {
	/// Optional boundary identifier for matching SSR and hydration boundaries.
	boundary_id: Option<String>,
	/// Resource hydration keys explicitly tracked by this boundary.
	tracked_resource_ids: Vec<String>,
	/// Pending-state closure used to choose the active branch on the client.
	is_pending: Arc<dyn Fn() -> bool + 'static>,
	/// Fallback view factory invoked while the boundary is pending.
	fallback: Arc<dyn Fn() -> Page + 'static>,
	/// Content view factory invoked after the boundary has resolved.
	content: Arc<dyn Fn() -> Page + 'static>,
}

/// Deferred view node with lazy fallback and content factories.
///
/// Deferred nodes preserve both branches for async SSR orchestration while
/// normal page traversal renders the content branch.
#[derive(Clone)]
pub struct DeferredNode {
	/// Stable node identifier for SSR and hydration coordination.
	node_id: String,
	/// Fallback view factory reserved for deferred streaming boundaries.
	fallback: Arc<dyn Fn() -> Page + 'static>,
	/// Content view factory rendered by normal traversal.
	content: Arc<dyn Fn() -> Page + 'static>,
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
	pub fn into_render(self) -> std::sync::Arc<dyn Fn() -> Page + 'static> {
		self.render
	}
}

impl std::fmt::Debug for SuspenseNode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("SuspenseNode")
			.field("boundary_id", &self.boundary_id)
			.field("tracked_resource_ids", &self.tracked_resource_ids)
			.field("is_pending", &"<closure>")
			.field("fallback", &"<closure>")
			.field("content", &"<closure>")
			.finish()
	}
}

impl Clone for SuspenseNode {
	fn clone(&self) -> Self {
		Self {
			boundary_id: self.boundary_id.clone(),
			tracked_resource_ids: self.tracked_resource_ids.clone(),
			is_pending: Arc::clone(&self.is_pending),
			fallback: Arc::clone(&self.fallback),
			content: Arc::clone(&self.content),
		}
	}
}

impl SuspenseNode {
	/// Creates a new suspense node.
	pub fn new(
		boundary_id: Option<String>,
		is_pending: impl Fn() -> bool + 'static,
		fallback: impl Fn() -> Page + 'static,
		content: impl Fn() -> Page + 'static,
	) -> Self {
		Self::new_with_tracked_resources(boundary_id, Vec::new(), is_pending, fallback, content)
	}

	/// Creates a new suspense node with tracked SSR resource keys.
	pub fn new_with_tracked_resources(
		boundary_id: Option<String>,
		tracked_resource_ids: Vec<String>,
		is_pending: impl Fn() -> bool + 'static,
		fallback: impl Fn() -> Page + 'static,
		content: impl Fn() -> Page + 'static,
	) -> Self {
		Self {
			boundary_id,
			tracked_resource_ids,
			is_pending: Arc::new(is_pending),
			fallback: Arc::new(fallback),
			content: Arc::new(content),
		}
	}

	/// Returns the optional boundary identifier.
	pub fn boundary_id(&self) -> Option<&str> {
		self.boundary_id.as_deref()
	}

	/// Returns resource hydration keys explicitly tracked by this boundary.
	pub fn tracked_resource_ids(&self) -> &[String] {
		&self.tracked_resource_ids
	}

	/// Returns `true` when the fallback branch should render.
	pub fn is_pending(&self) -> bool {
		(self.is_pending)()
	}

	/// Renders the fallback branch.
	pub fn fallback(&self) -> Page {
		(self.fallback)()
	}

	/// Renders the fallback branch.
	pub fn render_fallback(&self) -> Page {
		self.fallback()
	}

	/// Renders the content branch.
	pub fn content(&self) -> Page {
		(self.content)()
	}

	/// Renders the content branch.
	pub fn render_content(&self) -> Page {
		self.content()
	}

	/// Renders the currently active branch.
	pub fn render_branch(&self) -> Page {
		if self.is_pending() {
			self.fallback()
		} else {
			self.content()
		}
	}

	fn find_topmost_content_head_owned(&self) -> Option<Head> {
		self.content().find_topmost_head_owned()
	}
}

impl std::fmt::Debug for DeferredNode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("DeferredNode")
			.field("node_id", &self.node_id)
			.field("fallback", &"<closure>")
			.field("content", &"<closure>")
			.finish()
	}
}

impl DeferredNode {
	/// Creates a new deferred node.
	pub fn new(
		node_id: impl Into<String>,
		fallback: impl Fn() -> Page + 'static,
		content: impl Fn() -> Page + 'static,
	) -> Self {
		Self {
			node_id: node_id.into(),
			fallback: Arc::new(fallback),
			content: Arc::new(content),
		}
	}

	/// Returns the stable node identifier.
	pub fn node_id(&self) -> &str {
		&self.node_id
	}

	/// Renders the fallback branch.
	pub fn fallback(&self) -> Page {
		(self.fallback)()
	}

	/// Renders the fallback branch.
	pub fn render_fallback(&self) -> Page {
		self.fallback()
	}

	/// Renders the content branch.
	pub fn content(&self) -> Page {
		(self.content)()
	}

	/// Renders the content branch.
	pub fn render_content(&self) -> Page {
		self.content()
	}

	fn find_topmost_content_head_owned(&self) -> Option<Head> {
		self.content().find_topmost_head_owned()
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
		std::sync::Arc<dyn Fn() -> bool + 'static>,
		std::sync::Arc<dyn Fn() -> Page + 'static>,
		std::sync::Arc<dyn Fn() -> Page + 'static>,
	) {
		(self.condition, self.then_view, self.else_view)
	}
}

/// Router-managed outlet content used by layout routes.
#[derive(Debug, Clone)]
pub struct Outlet {
	id: Option<String>,
	child: Option<Box<Page>>,
}

impl Outlet {
	/// Creates an inline outlet for stateless native and SSR rendering.
	pub fn inline(child: impl IntoPage) -> Self {
		Self {
			id: None,
			child: Some(Box::new(child.into_page())),
		}
	}

	/// Creates a placeholder outlet for browser mount managers.
	pub fn placeholder(id: impl Into<String>) -> Self {
		Self {
			id: Some(id.into()),
			child: None,
		}
	}

	/// Returns the placeholder id, if this outlet is a browser placeholder.
	pub fn id(&self) -> Option<&str> {
		self.id.as_deref()
	}

	/// Returns the inline child page, if present.
	pub fn child(&self) -> Option<&Page> {
		self.child.as_deref()
	}

	/// Consumes the outlet and returns the inline child page.
	pub fn into_child(self) -> Option<Page> {
		self.child.map(|child| *child)
	}
}

/// A unified representation of renderable content.
///
/// Page is the core abstraction for all UI elements in the component system.
/// It can represent DOM elements, text nodes, fragments, or reactive content.
///
/// `Page` is `Clone`: the `Reactive` and `ReactiveIf` variants share their
/// render closures via `Arc<dyn Fn>`, so cloning is O(1) and both clones
/// invoke the same render closure. This makes `Page` usable as a `page!`
/// parameter (spec §3.7) where the auto-wrap (spec §4.1) needs to capture
/// it from a `Fn`-callable closure.
#[derive(Debug, Clone)]
pub enum Page {
	/// A DOM element.
	Element(PageElement),
	/// A text node.
	Text(Cow<'static, str>),
	/// A fragment containing multiple views (no wrapper element).
	Fragment(Vec<Page>),
	/// A fragment whose children have stable identity keys.
	KeyedFragment(Vec<(String, Page)>),
	/// A router-managed outlet used by layout routes.
	Outlet(Outlet),
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
	/// A development template descriptor paired with its renderable view.
	#[cfg(feature = "page-hot-reload")]
	DevTemplate {
		/// Opaque descriptor emitted by the template macro.
		metadata: DevTemplateMetadata,
		/// The renderable template view.
		view: Box<Page>,
	},
	/// A development dynamic-slot marker paired with its renderable view.
	#[cfg(feature = "page-hot-reload")]
	DevSlot {
		/// Stable slot identity within the containing template.
		slot_id: u32,
		/// The renderable dynamic slot view.
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
	/// A suspense boundary with pending and resolved branch factories.
	Suspense(SuspenseNode),
	/// A deferred node with fallback and content branch factories.
	Deferred(DeferredNode),
}

/// Represents a DOM element in the view tree.
///
/// `PageElement` is `Clone` to support `Page::Clone` (which in turn enables
/// using `Page` as a `page!` parameter under spec §3.7 / §4.1). Event
/// handlers are `Arc<dyn Fn>`, so cloning the element duplicates only Arc
/// handles.
#[derive(Clone)]
pub struct PageElement {
	/// The tag name (e.g., "div", "span").
	tag: Cow<'static, str>,
	/// HTML attributes.
	attrs: Vec<(Cow<'static, str>, Cow<'static, str>)>,
	reactive_attrs: Vec<ReactiveAttribute>,
	/// Child views.
	children: Vec<Page>,
	/// Whether this is a void element (no closing tag).
	is_void: bool,
	/// Event handlers attached to this element.
	event_handlers: Vec<(EventName, PageEventHandler)>,
	/// Optional controlled form-element binding.
	control_binding: Option<ControlBinding>,
}

impl std::fmt::Debug for PageElement {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("PageElement")
			.field("tag", &self.tag)
			.field("attrs", &self.attrs)
			.field("children", &self.children)
			.field("is_void", &self.is_void)
			.field("event_handlers_count", &self.event_handlers.len())
			.field("control_binding", &self.control_binding)
			.finish()
	}
}

#[cfg(feature = "reactive")]
#[allow(
	clippy::arc_with_non_send_sync,
	reason = "PageEventHandler stays Arc-backed for cloneable Page trees while allowing handlers to capture thread-affine reactive state."
)]
fn scoped_event_handler(handler: PageEventHandler) -> PageEventHandler {
	let Some(scope) = crate::reactive::scope::current_scope_id() else {
		return handler;
	};

	Arc::new(move |event| {
		let _ = crate::reactive::scope::enter_scope(scope, || handler(event));
	})
}

#[cfg(not(feature = "reactive"))]
fn scoped_event_handler(handler: PageEventHandler) -> PageEventHandler {
	handler
}

impl PageElement {
	/// Creates a new element view.
	pub fn new(tag: impl Into<Cow<'static, str>>) -> Self {
		let tag = tag.into();
		let tag = if is_safe_html_element_name(&tag) {
			tag
		} else {
			Cow::Borrowed("span")
		};
		let is_void = [
			"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source",
			"track", "wbr",
		]
		.iter()
		.any(|void_tag| tag.eq_ignore_ascii_case(void_tag));
		Self {
			tag,
			attrs: Vec::new(),
			reactive_attrs: Vec::new(),
			children: Vec::new(),
			is_void,
			event_handlers: Vec::new(),
			control_binding: None,
		}
	}

	/// Adds an attribute.
	pub fn attr(
		mut self,
		name: impl Into<Cow<'static, str>>,
		value: impl Into<Cow<'static, str>>,
	) -> Self {
		let name = name.into();
		let value = value.into();
		if is_safe_html_attribute(&name, &value) {
			self.attrs.push((name, value));
		}
		self
	}

	/// Adds multiple attributes.
	pub fn with_attrs<N, V>(mut self, attrs: impl IntoIterator<Item = (N, V)>) -> Self
	where
		N: Into<Cow<'static, str>>,
		V: Into<Cow<'static, str>>,
	{
		for (name, value) in attrs {
			self = self.attr(name, value);
		}
		self
	}

	/// Adds an attribute whose value is evaluated from reactive state.
	pub fn reactive_attr<F>(mut self, name: impl Into<Cow<'static, str>>, render: F) -> Self
	where
		F: Fn() -> Option<Cow<'static, str>> + 'static,
	{
		let name = name.into();
		if is_safe_html_name(&name)
			&& !name.eq_ignore_ascii_case("srcdoc")
			&& !name
				.get(..2)
				.is_some_and(|prefix| prefix.eq_ignore_ascii_case("on"))
		{
			self.reactive_attrs.push(ReactiveAttribute {
				name,
				render: Arc::new(render),
			});
		}
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
	/// ```rust
	/// use reinhardt_core::types::page::PageElement;
	///
	/// let is_disabled = true;
	/// PageElement::new("button")
	///     .bool_attr("disabled", is_disabled)
	///     .child("Click me");
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

	/// Adds multiple boolean attributes.
	pub fn with_bool_attrs<N>(mut self, attrs: impl IntoIterator<Item = (N, bool)>) -> Self
	where
		N: Into<Cow<'static, str>>,
	{
		for (name, value) in attrs {
			self = self.bool_attr(name, value);
		}
		self
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
	pub fn on(mut self, event_type: impl Into<EventName>, handler: PageEventHandler) -> Self {
		self.event_handlers
			.push((event_type.into(), scoped_event_handler(handler)));
		self
	}

	/// Attaches a controlled form-element binding.
	pub fn control_binding(mut self, binding: ControlBinding) -> Self {
		self.control_binding = Some(binding);
		self
	}

	/// Adds an event listener using string event name (convenience method).
	///
	/// This is a convenience wrapper around [`on`] that accepts a string event name
	/// and a closure. Catalog names are stored as known events, while all other
	/// names are preserved as explicit custom events.
	///
	/// # Arguments
	///
	/// * `event_name` - The event name (e.g., "click", "submit", "input")
	/// * `handler` - The event handler closure
	///
	/// # Example
	///
	/// ```ignore
	/// PageElement::new("button")
	///     .listener("click", |event| {
	///         console::log_1(&"Button clicked!".into());
	///     })
	/// ```
	#[cfg(wasm)]
	pub fn listener<F>(self, event_name: &str, handler: F) -> Self
	where
		F: Fn(web_sys::Event) + 'static,
	{
		self.on(classify_event_name(event_name), Arc::new(handler))
	}

	/// Adds a native event listener using a string event name.
	#[cfg(native)]
	pub fn listener<F>(self, event_name: &str, handler: F) -> Self
	where
		F: Fn(NativeEvent) + 'static,
	{
		self.on(classify_event_name(event_name), Arc::new(handler))
	}

	/// Returns the tag name.
	pub fn tag_name(&self) -> &str {
		&self.tag
	}

	/// Returns the attributes.
	pub fn attrs(&self) -> &[(Cow<'static, str>, Cow<'static, str>)] {
		&self.attrs
	}

	/// Returns the reactive attributes.
	pub fn reactive_attrs(&self) -> &[ReactiveAttribute] {
		&self.reactive_attrs
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
		let name = name.into();
		let value = value.into();
		if is_safe_html_attribute(&name, &value) {
			self.attrs.push((name, value));
		}
	}

	/// Adds a child mutably (for parser use).
	pub fn add_child(&mut self, child: impl IntoPage) {
		self.children.push(child.into_page());
	}

	/// Adds an event handler mutably (for parser use).
	pub fn add_event_handler(
		&mut self,
		event_type: impl Into<EventName>,
		handler: PageEventHandler,
	) {
		self.event_handlers
			.push((event_type.into(), scoped_event_handler(handler)));
	}

	/// Returns the event handlers.
	pub fn event_handlers(&self) -> &[(EventName, PageEventHandler)] {
		&self.event_handlers
	}

	/// Returns the controlled form-element binding, if present.
	pub fn bound_control(&self) -> Option<&ControlBinding> {
		self.control_binding.as_ref()
	}

	/// Consumes the element view and returns the children.
	pub fn into_children(self) -> Vec<Page> {
		self.children
	}

	/// Consumes the element view and returns the event handlers.
	pub fn into_event_handlers(self) -> Vec<(EventName, PageEventHandler)> {
		self.event_handlers
	}

	/// Consumes the element view and returns its original public parts.
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
		Vec<(EventName, PageEventHandler)>,
	) {
		(
			self.tag,
			self.attrs,
			self.children,
			self.is_void,
			self.event_handlers,
		)
	}

	/// Consumes the element view and returns all parts, including its control binding.
	///
	/// Returns a tuple of (tag, attrs, reactive_attrs, children, is_void, event_handlers, control_binding).
	#[allow(clippy::type_complexity)] // Tuple decomposition is intentional for destructuring
	pub fn into_parts_with_control_binding(
		self,
	) -> (
		Cow<'static, str>,
		Vec<(Cow<'static, str>, Cow<'static, str>)>,
		Vec<ReactiveAttribute>,
		Vec<Page>,
		bool,
		Vec<(EventName, PageEventHandler)>,
		Option<ControlBinding>,
	) {
		(
			self.tag,
			self.attrs,
			self.reactive_attrs,
			self.children,
			self.is_void,
			self.event_handlers,
			self.control_binding,
		)
	}
}

fn classify_event_name(event_name: &str) -> EventName {
	match event::event_spec(event_name) {
		Some(spec) => EventName::Known(spec.kind),
		None => EventName::Custom(Cow::Owned(event_name.to_owned())),
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

	/// Creates a keyed fragment view.
	pub fn keyed_fragment<K, V>(children: impl IntoIterator<Item = (K, V)>) -> Self
	where
		K: Into<String>,
		V: IntoPage,
	{
		Self::KeyedFragment(
			children
				.into_iter()
				.map(|(key, child)| (key.into(), child.into_page()))
				.collect(),
		)
	}

	/// Creates an empty view.
	pub fn empty() -> Self {
		Self::Empty
	}

	/// Creates an outlet page node.
	pub fn outlet(outlet: Outlet) -> Self {
		Page::Outlet(outlet)
	}

	/// Attaches a head section to this view.
	///
	/// The head section contains metadata like title, meta tags, stylesheets,
	/// and scripts that should be included in the HTML `<head>` element.
	///
	/// # Example
	///
	/// ```rust
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

	/// Attaches opaque development template metadata to this view.
	#[cfg(feature = "page-hot-reload")]
	pub fn with_dev_template_metadata<T>(self, metadata: T) -> Self
	where
		T: std::any::Any + Send + Sync,
	{
		Self::DevTemplate {
			metadata: DevTemplateMetadata::new(metadata),
			view: Box::new(self),
		}
	}

	/// Marks this view as a development dynamic slot.
	#[cfg(feature = "page-hot-reload")]
	pub fn with_dev_slot(self, slot_id: u32) -> Self {
		Self::DevSlot {
			slot_id,
			view: Box::new(self),
		}
	}

	/// Returns development template metadata without consuming the view.
	#[cfg(feature = "page-hot-reload")]
	pub fn dev_template_metadata(&self) -> Option<&DevTemplateMetadata> {
		match self {
			Self::DevTemplate { metadata, .. } => Some(metadata),
			_ => None,
		}
	}

	/// Consumes a development template wrapper and returns its parts.
	#[cfg(feature = "page-hot-reload")]
	pub fn into_dev_template_parts(self) -> Option<(DevTemplateMetadata, Page)> {
		match self {
			Self::DevTemplate { metadata, view } => Some((metadata, *view)),
			_ => None,
		}
	}

	/// Returns the development slot identity without consuming the view.
	#[cfg(feature = "page-hot-reload")]
	pub fn dev_slot_id(&self) -> Option<u32> {
		match self {
			Self::DevSlot { slot_id, .. } => Some(*slot_id),
			_ => None,
		}
	}

	/// Consumes a development slot wrapper and returns its parts.
	#[cfg(feature = "page-hot-reload")]
	pub fn into_dev_slot_parts(self) -> Option<(u32, Page)> {
		match self {
			Self::DevSlot { slot_id, view } => Some((slot_id, *view)),
			_ => None,
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
			condition: std::sync::Arc::new(condition),
			then_view: std::sync::Arc::new(then_view),
			else_view: std::sync::Arc::new(else_view),
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
	/// # Nested `watch { }` footgun (issue #4515)
	///
	/// The `F: Fn() -> Page + 'static` bound matters: the runtime invokes the
	/// body once per signal change, so the body MUST NOT consume its captures.
	/// This becomes a footgun when two `watch { }` blocks (which lower to
	/// `Page::reactive`) are nested and share the same `Signal<T>` capture.
	/// `Signal<T>` is intentionally `!Copy` (it is `Rc`/`Arc`-backed and
	/// cheap to `Clone`), so the outer `move` closure cannot transfer the
	/// same signal into the inner `move` closure twice. The compiler reports
	/// this as `E0507: cannot move out of value, a captured variable in an
	/// Fn closure` at the inner `watch { }` site, and rustc itself suggests
	/// cloning the value before the inner move.
	///
	/// In addition to rustc's `clone` suggestion, the framework offers a
	/// second fix that is often preferable:
	///
	/// 1. **Flatten**: a single `watch { }` already subscribes to every
	///    signal it reads, so nested `watch { }` blocks are almost never
	///    required for reactivity reasons. Collapse them into one.
	/// 2. **Clone inside the outer body** before constructing the inner
	///    `watch { }`: `let s = s.clone();` (Signal clone is cheap). This is
	///    the path rustc's own diagnostic also suggests.
	///
	/// See `crates/reinhardt-pages/docs/watch_semantics.md` for worked
	/// examples and the rationale behind each fix.
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
			render: std::sync::Arc::new(render),
		})
	}

	/// Extracts the head section from this view if it has one.
	///
	/// Returns `Some(&Head)` if this view is a `WithHead` variant or a
	/// development wrapper around one, or `None` for other variants.
	pub fn extract_head(&self) -> Option<&Head> {
		match self {
			Page::WithHead { head, .. } => Some(head),
			#[cfg(feature = "page-hot-reload")]
			Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => view.extract_head(),
			_ => None,
		}
	}

	/// Finds the first structural `Head` for compatibility-oriented inspection.
	///
	/// This is not the document-head resolution API. SSR, hydration, and browser
	/// mounting compose every active declaration through `reinhardt-pages`.
	///
	/// # Search Order
	///
	/// 1. If this view is a `WithHead`, returns its head
	/// 2. For element and fragment views, searches children in order and returns the first found
	/// 3. For inline `Outlet` views, searches the child page
	/// 4. For other variants, returns `None`
	///
	/// Use [`Page::find_topmost_head_owned`] when lazy Suspense/Deferred content
	/// should participate in the lookup.
	pub fn find_topmost_head(&self) -> Option<&Head> {
		match self {
			Page::WithHead { head, .. } => Some(head),
			Page::Element(element) => element
				.child_views()
				.iter()
				.find_map(|view| view.find_topmost_head()),
			Page::Fragment(children) => children.iter().find_map(|v| v.find_topmost_head()),
			Page::KeyedFragment(children) => {
				children.iter().find_map(|(_, v)| v.find_topmost_head())
			}
			Page::Outlet(outlet) => outlet.child().and_then(Page::find_topmost_head),
			#[cfg(feature = "page-hot-reload")]
			Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => view.find_topmost_head(),
			_ => None,
		}
	}

	/// Finds the first structural `Head` and returns an owned copy for
	/// compatibility-oriented inspection.
	///
	/// This is not the document-head resolution API. SSR, hydration, and browser
	/// mounting compose every active declaration through `reinhardt-pages`.
	/// Unlike [`Page::find_topmost_head`], this method can evaluate lazy
	/// Suspense/Deferred content without storing request state on the `Page`.
	pub fn find_topmost_head_owned(&self) -> Option<Head> {
		match self {
			Page::WithHead { head, .. } => Some(head.clone()),
			Page::Element(element) => element
				.child_views()
				.iter()
				.find_map(Page::find_topmost_head_owned),
			Page::Fragment(children) => children.iter().find_map(Page::find_topmost_head_owned),
			Page::KeyedFragment(children) => children
				.iter()
				.find_map(|(_, v)| v.find_topmost_head_owned()),
			Page::Outlet(outlet) => outlet.child().and_then(Page::find_topmost_head_owned),
			#[cfg(feature = "page-hot-reload")]
			Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => view.find_topmost_head_owned(),
			Page::Suspense(node) => node.find_topmost_content_head_owned(),
			Page::Deferred(node) => node.find_topmost_content_head_owned(),
			_ => None,
		}
	}

	/// Renders the view to an HTML string.
	///
	/// This is the core SSR method that converts the view tree to HTML.
	pub fn render_to_string(&self) -> String {
		let mut output = String::new();
		self.render_to_string_inner(&mut output, None);
		output
	}

	fn render_to_string_inner(
		&self,
		output: &mut String,
		selection: Option<&StringRenderSelection>,
	) {
		match self {
			Page::Element(el) => {
				if !is_safe_html_element_name(el.tag_name()) {
					for child in el.child_views() {
						child.render_to_string_inner(output, selection);
					}
					return;
				}
				output.push('<');
				output.push_str(el.tag_name());
				let binding = el.bound_control();
				let binding_value = binding.map(ControlBinding::read);
				let reactive_input_type = el
					.reactive_attrs()
					.iter()
					.enumerate()
					.rfind(|(_, attribute)| attribute.name().eq_ignore_ascii_case("type"))
					.map(|(index, attribute)| (index, attribute.value()));
				let reactive_input_type_is_supported =
					reactive_input_type.as_ref().is_none_or(|(_, value)| {
						binding.is_none_or(|binding| {
							control_binding::controlled_attribute_update_is_supported(
								el.tag_name(),
								binding.kind(),
								"type",
								value.as_deref(),
							)
						})
					});
				let reactive_select_multiple = el
					.reactive_attrs()
					.iter()
					.enumerate()
					.rfind(|(_, attribute)| attribute.name().eq_ignore_ascii_case("multiple"))
					.map(|(index, attribute)| (index, attribute.value()));
				let reactive_select_multiple_is_supported =
					reactive_select_multiple.as_ref().is_none_or(|(_, value)| {
						binding.is_none_or(|binding| {
							control_binding::controlled_attribute_update_is_supported(
								el.tag_name(),
								binding.kind(),
								"multiple",
								value.as_deref(),
							)
						})
					});
				let mut input_type_is_password = el
					.attrs()
					.iter()
					.find(|(name, _)| name.eq_ignore_ascii_case("type"))
					.is_some_and(|(_, value)| value.eq_ignore_ascii_case("password"));
				if reactive_input_type_is_supported
					&& let Some((_, value)) = reactive_input_type.as_ref()
				{
					input_type_is_password = value
						.as_deref()
						.is_some_and(|value| value.eq_ignore_ascii_case("password"));
				}
				let omits_bound_password_value = el.tag_name().eq_ignore_ascii_case("input")
					&& input_type_is_password
					&& binding.is_some_and(|binding| binding.kind() == ControlKind::Text);
				let projected_input_value = if omits_bound_password_value {
					None
				} else {
					binding.and_then(|binding| match (binding.kind(), binding_value.as_ref()) {
						(
							ControlKind::Text | ControlKind::Number,
							Some(ControlValue::Text(value)),
						) => Some(value.as_str()),
						(ControlKind::Radio, _) => binding.radio_value(),
						_ => None,
					})
				};
				let projects_value =
					el.tag_name().eq_ignore_ascii_case("input") && projected_input_value.is_some();
				let projects_checked = matches!(binding_value, Some(ControlValue::Checked(true)));
				let projects_selected = el.tag_name().eq_ignore_ascii_case("option")
					&& selection.is_some_and(|selection| selection.matches(el));

				for (name, value) in el.attrs() {
					if !is_safe_html_attribute(name, value) {
						continue;
					}
					let has_reactive_attribute = el
						.reactive_attrs()
						.iter()
						.any(|attribute| attribute.name().eq_ignore_ascii_case(name))
						&& (!(name.eq_ignore_ascii_case("type") && reactive_input_type.is_some())
							|| reactive_input_type_is_supported)
						&& (!(name.eq_ignore_ascii_case("multiple")
							&& reactive_select_multiple.is_some())
							|| reactive_select_multiple_is_supported);
					// Skip boolean attributes with falsy values (empty, "false", "0")
					let name_str: &str = name.as_ref();
					if (name_str.eq_ignore_ascii_case("value")
						&& (projects_value || omits_bound_password_value))
						|| (name_str.eq_ignore_ascii_case("checked") && binding.is_some())
						|| (name_str.eq_ignore_ascii_case("selected") && selection.is_some())
						|| (is_boolean_attr(name_str) && !is_boolean_attr_truthy(value))
						|| has_reactive_attribute
					{
						continue;
					}

					output.push(' ');
					output.push_str(name);
					output.push_str("=\"");
					output.push_str(&html_escape(value));
					output.push('"');
				}
				for (index, attribute) in el.reactive_attrs().iter().enumerate() {
					let name = attribute.name();
					if (name.eq_ignore_ascii_case("value")
						&& (projects_value || omits_bound_password_value))
						|| (name.eq_ignore_ascii_case("checked") && binding.is_some())
						|| (name.eq_ignore_ascii_case("selected") && selection.is_some())
					{
						continue;
					}
					if el.reactive_attrs()[index + 1..]
						.iter()
						.any(|later| later.name().eq_ignore_ascii_case(attribute.name()))
					{
						continue;
					}
					let value = match reactive_input_type.as_ref() {
						Some((type_index, value)) if *type_index == index => value.clone(),
						_ => match reactive_select_multiple.as_ref() {
							Some((multiple_index, value)) if *multiple_index == index => {
								value.clone()
							}
							_ => attribute.value(),
						},
					};
					if (reactive_input_type
						.as_ref()
						.is_some_and(|(type_index, _)| *type_index == index)
						&& !reactive_input_type_is_supported)
						|| (reactive_select_multiple
							.as_ref()
							.is_some_and(|(multiple_index, _)| *multiple_index == index)
							&& !reactive_select_multiple_is_supported)
					{
						continue;
					}
					if let Some(value) = value
						&& is_safe_html_attribute(attribute.name(), &value)
						&& (!is_boolean_attr(attribute.name()) || is_boolean_attr_truthy(&value))
					{
						output.push(' ');
						output.push_str(attribute.name());
						output.push_str("=\"");
						output.push_str(&html_escape(&value));
						output.push('"');
					}
				}
				if let Some(value) = projected_input_value
					&& el.tag_name().eq_ignore_ascii_case("input")
				{
					output.push_str(" value=\"");
					output.push_str(&html_escape(value));
					output.push('"');
				}
				if projects_checked {
					output.push_str(" checked=\"checked\"");
				}
				if projects_selected {
					output.push_str(" selected=\"selected\"");
				}

				if el.is_void() {
					output.push_str(" />");
				} else {
					output.push('>');
					if el.tag_name().eq_ignore_ascii_case("textarea")
						&& let Some(ControlValue::Text(value)) = &binding_value
					{
						output.push_str(&html_escape(value));
					} else {
						let child_selection = if el.tag_name().eq_ignore_ascii_case("select") {
							StringRenderSelection::from_binding(binding, binding_value.as_ref())
						} else {
							None
						};
						for child in el.child_views() {
							child.render_to_string_inner(
								output,
								child_selection.as_ref().or(selection),
							);
						}
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
					child.render_to_string_inner(output, selection);
				}
			}
			Page::KeyedFragment(children) => {
				for (_, child) in children {
					child.render_to_string_inner(output, selection);
				}
			}
			Page::Outlet(outlet) => {
				if let Some(child) = outlet.child() {
					child.render_to_string_inner(output, selection);
				}
			}
			Page::Empty => {}
			Page::WithHead { view, .. } => {
				// The head is extracted separately during SSR; here we just render the content
				view.render_to_string_inner(output, selection);
			}
			#[cfg(feature = "page-hot-reload")]
			Page::DevTemplate { view, .. } | Page::DevSlot { view, .. } => {
				view.render_to_string_inner(output, selection);
			}
			Page::ReactiveIf(reactive_if) => {
				// For SSR, evaluate condition once and render the appropriate branch
				let condition_result = (reactive_if.condition)();
				let view = if condition_result {
					(reactive_if.then_view)()
				} else {
					(reactive_if.else_view)()
				};
				view.render_to_string_inner(output, selection);
			}
			Page::Reactive(reactive) => {
				// For SSR, evaluate render once and render the result
				let view = reactive.render();
				view.render_to_string_inner(output, selection);
			}
			Page::Suspense(node) => {
				let view = node.render_branch();
				view.render_to_string_inner(output, selection);
			}
			Page::Deferred(node) => {
				let view = node.content();
				view.render_to_string_inner(output, selection);
			}
		}
	}
}

#[derive(Clone)]
enum StringRenderSelection {
	One {
		value: String,
		matched: std::cell::Cell<bool>,
	},
	Many(Vec<String>),
}

impl StringRenderSelection {
	fn from_binding(
		binding: Option<&ControlBinding>,
		value: Option<&ControlValue>,
	) -> Option<Self> {
		match (binding.map(ControlBinding::kind), value) {
			(Some(ControlKind::SelectOne), Some(ControlValue::Text(value))) => Some(Self::One {
				value: value.clone(),
				matched: std::cell::Cell::new(false),
			}),
			(Some(ControlKind::SelectMany), Some(ControlValue::SelectedValues(values))) => {
				Some(Self::Many(values.clone()))
			}
			_ => None,
		}
	}

	fn matches(&self, option: &PageElement) -> bool {
		let value = option
			.attrs()
			.iter()
			.find(|(name, _)| name.eq_ignore_ascii_case("value"))
			.map(|(_, value)| value.as_ref().to_owned())
			.or_else(|| {
				(!Self::has_dynamic_option_content(option))
					.then(|| Self::normalize_option_text(option))
			});
		let Some(value) = value else {
			return false;
		};
		match self {
			Self::One {
				value: selected,
				matched,
			} => selected == &value && !matched.replace(true),
			Self::Many(selected) => selected.iter().any(|selected| selected == &value),
		}
	}

	fn normalize_option_text(option: &PageElement) -> String {
		Self::text_content_without_script(&Page::Element(option.clone()))
			.split_ascii_whitespace()
			.collect::<Vec<_>>()
			.join(" ")
	}

	fn has_dynamic_option_content(option: &PageElement) -> bool {
		option
			.child_views()
			.iter()
			.any(Self::page_has_dynamic_content)
	}

	fn is_script_element(element: &PageElement) -> bool {
		let tag = element.tag_name();
		tag.eq_ignore_ascii_case("script") || tag.eq_ignore_ascii_case("svg:script")
	}

	fn page_has_dynamic_content(page: &Page) -> bool {
		match page {
			Page::Element(element) if Self::is_script_element(element) => false,
			Page::Element(element) => element
				.child_views()
				.iter()
				.any(Self::page_has_dynamic_content),
			Page::Text(_) | Page::Empty => false,
			Page::Fragment(children) => children.iter().any(Self::page_has_dynamic_content),
			Page::KeyedFragment(children) => children
				.iter()
				.any(|(_, child)| Self::page_has_dynamic_content(child)),
			Page::Outlet(outlet) => outlet.child().is_some_and(Self::page_has_dynamic_content),
			Page::WithHead { view, .. } => Self::page_has_dynamic_content(view),
			#[cfg(feature = "page-hot-reload")]
			Page::DevTemplate { view, .. } => Self::page_has_dynamic_content(view),
			#[cfg(feature = "page-hot-reload")]
			Page::DevSlot { .. } => true,
			Page::ReactiveIf(_) | Page::Reactive(_) | Page::Suspense(_) | Page::Deferred(_) => true,
		}
	}

	fn text_content_without_script(page: &Page) -> String {
		match page {
			Page::Element(element) if Self::is_script_element(element) => String::new(),
			Page::Element(element) => element
				.child_views()
				.iter()
				.map(Self::text_content_without_script)
				.collect(),
			Page::Text(text) => text.to_string(),
			Page::Fragment(children) => children
				.iter()
				.map(Self::text_content_without_script)
				.collect(),
			Page::KeyedFragment(children) => children
				.iter()
				.map(|(_, child)| Self::text_content_without_script(child))
				.collect(),
			Page::Outlet(outlet) => outlet
				.child()
				.map(Self::text_content_without_script)
				.unwrap_or_default(),
			Page::Empty => String::new(),
			Page::WithHead { view, .. } => Self::text_content_without_script(view),
			#[cfg(feature = "page-hot-reload")]
			Page::DevTemplate { view, .. } => Self::text_content_without_script(view),
			#[cfg(feature = "page-hot-reload")]
			Page::DevSlot { .. } => String::new(),
			Page::ReactiveIf(_) | Page::Reactive(_) | Page::Suspense(_) | Page::Deferred(_) => {
				String::new()
			}
		}
	}
}

fn is_safe_html_name(name: &str) -> bool {
	!name.is_empty()
		&& name
			.chars()
			.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.'))
}

#[doc(hidden)]
pub fn is_safe_html_element_name(name: &str) -> bool {
	const BLOCKED_ELEMENTS: &[&str] = &[
		"animate",
		"animatemotion",
		"animatetransform",
		"annotation-xml",
		"base",
		"discard",
		"embed",
		"iframe",
		"link",
		"math",
		"meta",
		"object",
		"script",
		"set",
		"style",
	];
	is_safe_html_name(name)
		&& !name.contains(':')
		&& !BLOCKED_ELEMENTS
			.iter()
			.any(|blocked| name.eq_ignore_ascii_case(blocked))
}

#[doc(hidden)]
pub fn is_safe_html_attribute(name: &str, value: &str) -> bool {
	if !is_safe_html_name(name)
		|| name.eq_ignore_ascii_case("srcdoc")
		|| name
			.get(..2)
			.is_some_and(|prefix| prefix.eq_ignore_ascii_case("on"))
	{
		return false;
	}

	const URL_ATTRIBUTES: &[&str] = &[
		"action",
		"background",
		"cite",
		"formaction",
		"href",
		"poster",
		"src",
		"xlink:href",
	];
	!URL_ATTRIBUTES
		.iter()
		.any(|attribute| name.eq_ignore_ascii_case(attribute))
		|| crate::security::xss::is_safe_url(value)
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

impl IntoPage for Outlet {
	fn into_page(self) -> Page {
		Page::Outlet(self)
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
	use std::cell::Cell;
	use std::rc::Rc;

	use super::*;
	use crate::reactive::{ReactiveScope, Signal};
	use rstest::rstest;

	#[cfg(feature = "page-hot-reload")]
	#[test]
	fn dev_page_metadata_round_trips() {
		// Arrange
		let page = Page::text("body")
			.with_dev_slot(7)
			.with_dev_template_metadata(String::from("descriptor"));

		// Act
		let (metadata, view) = page.into_dev_template_parts().expect("metadata");

		// Assert
		assert_eq!(
			metadata.downcast_ref::<String>(),
			Some(&String::from("descriptor"))
		);
		assert!(matches!(view, Page::DevSlot { slot_id: 7, .. }));
	}

	#[test]
	fn mount_error_preserves_control_binding_failure() {
		let binding_error = ControlBindingError::UnsupportedElement {
			control: ControlKind::Checkbox,
			actual_tag: "select".to_owned(),
		};

		let mount_error = MountError::from(binding_error.clone());

		assert_eq!(mount_error, MountError::ControlBinding(binding_error));
		assert!(std::error::Error::source(&mount_error).is_some());
	}

	#[test]
	fn event_type_reexports_the_complete_catalog() {
		let event_type: EventType = EventType::PointerDown;

		assert_eq!(event_type.as_str(), "pointerdown");
	}

	#[cfg(all(native, feature = "reactive"))]
	#[test]
	fn page_element_preserves_known_and_custom_event_names() {
		let element = PageElement::new("button")
			.on(EventType::Click, Arc::new(|_| {}))
			.listener("editor:commit", |_| {});

		assert_eq!(
			element.event_handlers()[0].0,
			EventName::Known(EventType::Click)
		);
		assert_eq!(
			element.event_handlers()[1].0,
			EventName::Custom(Cow::Owned("editor:commit".to_owned()))
		);
	}

	#[cfg(native)]
	#[test]
	fn page_event_handlers_reenter_their_creation_scope() {
		use crate::reactive::{ReactiveScope, Signal};
		use std::cell::Cell;
		use std::rc::Rc;

		let scope = ReactiveScope::new();
		let calls = Rc::new(Cell::new(0));
		let handlers = scope.enter(|| {
			let on_calls = Rc::clone(&calls);
			let mut element = PageElement::new("button").on(
				EventType::Click,
				Arc::new(move |_| {
					let signal = Signal::new(1);
					assert_eq!(signal.get(), 1);
					on_calls.set(on_calls.get() + 1);
				}),
			);
			let added_calls = Rc::clone(&calls);
			element.add_event_handler(
				EventType::Input,
				Arc::new(move |_| {
					let signal = Signal::new(2);
					assert_eq!(signal.get(), 2);
					added_calls.set(added_calls.get() + 1);
				}),
			);
			element.into_event_handlers()
		});

		let event = NativeEvent::for_known(EventType::Click, NativeEventPayload::default());
		for (_, handler) in handlers {
			handler(event.clone());
		}

		assert_eq!(calls.get(), 2);
	}

	#[cfg(native)]
	#[test]
	fn native_target_owns_control_state_snapshot() {
		let target = NativeEventTarget::new("INPUT")
			.with_attribute("type", "checkbox")
			.with_value("enabled")
			.with_checked(true)
			.with_selected_values(["primary", "secondary"])
			.with_file(NativeEventFile::new("avatar.png", "image/png", 128, 42))
			.with_text_content("Enabled")
			.with_content_editable(true);

		assert_eq!(target.tag_name(), "input");
		assert_eq!(target.attribute("type"), Some("checkbox"));
		assert_eq!(target.value(), Some("enabled"));
		assert_eq!(target.checked(), Some(true));
		assert_eq!(target.selected_values(), &["primary", "secondary"]);
		assert_eq!(target.files()[0].name(), "avatar.png");
		assert_eq!(target.text_content(), Some("Enabled"));
		assert!(target.is_content_editable());
	}

	#[cfg(native)]
	#[test]
	fn native_payload_exposes_its_interface_family_data() {
		assert_eq!(
			NativeEventPayload::for_interface(EventInterface::Keyboard).interface(),
			EventInterface::Keyboard
		);
		let payload = NativeEventPayload::Pointer(PointerEventData {
			mouse: MouseEventData {
				client_x: 120.0,
				client_y: 80.0,
				button: 0,
				buttons: 1,
				modifiers: ModifierState {
					shift: true,
					..ModifierState::default()
				},
				..MouseEventData::default()
			},
			pointer_id: 7,
			pointer_kind: "pen".to_owned(),
			pressure: 0.5,
			..PointerEventData::default()
		});

		assert_eq!(payload.interface(), EventInterface::Pointer);
		let NativeEventPayload::Pointer(pointer) = payload else {
			panic!("pointer payload must retain its family data");
		};
		assert_eq!(
			(pointer.mouse.client_x, pointer.mouse.client_y),
			(120.0, 80.0)
		);
		assert_eq!(pointer.pointer_id, 7);
		assert_eq!(pointer.pointer_kind, "pen");
		assert_eq!(pointer.pressure, 0.5);
		assert!(pointer.mouse.modifiers.shift);
	}

	#[cfg(native)]
	#[test]
	fn native_event_snapshots_share_cancelation_and_propagation_state() {
		let target = NativeEventTarget::new("span").with_text_content("Save");
		let button = NativeEventTarget::new("button").with_attribute("type", "submit");
		let ancestor = NativeEventTarget::new("form");
		let event = NativeEvent::for_known(
			EventType::Click,
			NativeEventPayload::Pointer(PointerEventData::default()),
		)
		.with_target(target.clone())
		.with_current_target(button.clone());
		let ancestor_event = event.with_current_target(ancestor.clone());

		assert_eq!(event.target(), Some(&target));
		assert_eq!(event.current_target(), Some(&button));
		assert_eq!(ancestor_event.target(), Some(&target));
		assert_eq!(ancestor_event.current_target(), Some(&ancestor));
		assert!(event.base().cancelable);
		event.prevent_default();
		ancestor_event.stop_propagation();

		assert!(ancestor_event.default_prevented());
		assert!(event.propagation_stopped());
		assert!(!event.immediate_propagation_stopped());
	}

	#[cfg(native)]
	#[test]
	fn native_event_respects_cancelable_and_immediate_propagation_semantics() {
		let event = NativeEvent::new(
			EventName::Known(EventType::Input),
			BaseEventData {
				bubbles: true,
				cancelable: false,
				composed: true,
				time_stamp: 12.5,
				is_trusted: false,
			},
			NativeEventPayload::Input(InputEventData {
				data: Some("x".to_owned()),
				input_type: Some("insertText".to_owned()),
				is_composing: false,
			}),
		);

		event.prevent_default();
		event.stop_immediate_propagation();

		assert!(!event.default_prevented());
		assert!(event.propagation_stopped());
		assert!(event.immediate_propagation_stopped());
		assert_eq!(event.base().time_stamp, 12.5);
		assert_eq!(event.name(), &EventName::Known(EventType::Input));
	}

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
		assert!(PageElement::new("INPUT").is_void);
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
	fn test_element_with_batch_attrs() {
		let el = PageElement::new("div").with_attrs([("class", "container"), ("id", "main")]);
		assert_eq!(el.attrs.len(), 2);
		assert_eq!(el.attrs[0].0, "class");
		assert_eq!(el.attrs[0].1, "container");
		assert_eq!(el.attrs[1].0, "id");
		assert_eq!(el.attrs[1].1, "main");
	}

	#[test]
	fn test_element_with_batch_bool_attrs() {
		let el =
			PageElement::new("button").with_bool_attrs([("disabled", true), ("hidden", false)]);
		assert_eq!(el.attrs.len(), 1);
		assert_eq!(el.attrs[0].0, "disabled");
		assert_eq!(el.attrs[0].1, "disabled");
	}

	#[test]
	fn batch_bool_attrs_reject_unsafe_attribute_names() {
		let page = PageElement::new("button")
			.with_bool_attrs([("disabled", true), ("x=\" onmouseover=\"alert(1)", true)])
			.into_page();

		assert_eq!(
			page.render_to_string(),
			"<button disabled=\"disabled\"></button>"
		);
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
	fn render_to_string_prefers_reactive_attributes() {
		let view = PageElement::new("div")
			.attr("class", "stale")
			.reactive_attr("CLASS", || Some("current".into()))
			.into_page();

		assert_eq!(view.render_to_string(), "<div CLASS=\"current\"></div>");
	}

	#[test]
	fn render_to_string_omits_falsy_reactive_boolean_attributes() {
		// Arrange
		let view = PageElement::new("button")
			.reactive_attr("DISABLED", || Some("false".into()))
			.into_page();

		// Act
		let html = view.render_to_string();

		// Assert
		assert_eq!(html, "<button></button>");
	}

	#[test]
	fn render_to_string_omits_unsafe_reactive_attributes() {
		let view = PageElement::new("a")
			.reactive_attr("href", || Some("javascript:alert(1)".into()))
			.reactive_attr("onclick", || Some("alert(1)".into()))
			.into_page();

		assert_eq!(view.render_to_string(), "<a></a>");
	}

	#[test]
	fn render_to_string_projects_bound_input_and_textarea_values() {
		ReactiveScope::run(|| {
			let input = PageElement::new("input")
				.attr("type", "text")
				.attr("value", "stale")
				.control_binding(ControlBinding::text(Signal::new("current".to_owned())))
				.into_page();
			let textarea = PageElement::new("textarea")
				.control_binding(ControlBinding::text(Signal::new("current".to_owned())))
				.into_page();

			assert_eq!(
				input.render_to_string(),
				"<input type=\"text\" value=\"current\" />"
			);
			assert_eq!(textarea.render_to_string(), "<textarea>current</textarea>");
		});
	}

	#[test]
	fn render_to_string_omits_bound_password_values() {
		ReactiveScope::run(|| {
			let input = PageElement::new("input")
				.attr("type", "password")
				.attr("value", "stale")
				.control_binding(ControlBinding::text(Signal::new("secret".to_owned())))
				.into_page();

			assert_eq!(input.render_to_string(), "<input type=\"password\" />");
		});
	}

	#[rstest]
	fn render_to_string_uses_the_first_static_password_type() {
		ReactiveScope::run(|| {
			let input = PageElement::new("input")
				.attr("type", "password")
				.attr("type", "text")
				.control_binding(ControlBinding::text(Signal::new("secret".to_owned())))
				.into_page();

			assert_eq!(
				input.render_to_string(),
				"<input type=\"password\" type=\"text\" />"
			);
		});
	}

	#[rstest]
	fn render_to_string_omits_bound_password_values_for_reactive_types() {
		ReactiveScope::run(|| {
			// Arrange
			let type_evaluations = Rc::new(Cell::new(0));
			let reactive_type_evaluations = Rc::clone(&type_evaluations);
			let overridden_value_evaluations = Rc::new(Cell::new(0));
			let reactive_value_evaluations = Rc::clone(&overridden_value_evaluations);
			let superseded_class_evaluations = Rc::new(Cell::new(0));
			let first_class_evaluations = Rc::clone(&superseded_class_evaluations);
			let final_class_evaluations = Rc::new(Cell::new(0));
			let last_class_evaluations = Rc::clone(&final_class_evaluations);
			let input = PageElement::new("input")
				.attr("type", "text")
				.reactive_attr("type", move || {
					reactive_type_evaluations.set(reactive_type_evaluations.get() + 1);
					Some("password".into())
				})
				.reactive_attr("value", move || {
					reactive_value_evaluations.set(reactive_value_evaluations.get() + 1);
					Some("exposed".into())
				})
				.reactive_attr("class", move || {
					first_class_evaluations.set(first_class_evaluations.get() + 1);
					Some("superseded".into())
				})
				.reactive_attr("class", move || {
					last_class_evaluations.set(last_class_evaluations.get() + 1);
					Some("final".into())
				})
				.attr("value", "stale")
				.control_binding(ControlBinding::text(Signal::new("secret".to_owned())))
				.into_page();

			// Act
			let html = input.render_to_string();

			// Assert
			assert_eq!(html, "<input type=\"password\" class=\"final\" />");
			assert_eq!(type_evaluations.get(), 1);
			assert_eq!(overridden_value_evaluations.get(), 0);
			assert_eq!(superseded_class_evaluations.get(), 0);
			assert_eq!(final_class_evaluations.get(), 1);
		});
	}

	#[test]
	fn render_to_string_rejects_a_binding_incompatible_reactive_input_type() {
		ReactiveScope::run(|| {
			// Arrange
			let input = PageElement::new("input")
				.attr("type", "text")
				.reactive_attr("type", || Some("file".into()))
				.control_binding(ControlBinding::text(Signal::new("secret".to_owned())))
				.into_page();

			// Act
			let html = input.render_to_string();

			// Assert
			assert_eq!(html, "<input type=\"text\" value=\"secret\" />");
		});
	}

	#[test]
	fn render_to_string_rejects_a_binding_incompatible_reactive_select_cardinality() {
		ReactiveScope::run(|| {
			// Arrange
			let select = PageElement::new("select")
				.reactive_attr("multiple", || Some("multiple".into()))
				.control_binding(ControlBinding::select_one(Signal::new("draft".to_owned())))
				.into_page();

			// Act
			let html = select.render_to_string();

			// Assert
			assert_eq!(html, "<select></select>");
		});
	}

	#[test]
	fn render_to_string_projects_bound_radio_values() {
		ReactiveScope::run(|| {
			let input = PageElement::new("input")
				.attr("type", "radio")
				.attr("value", "stale")
				.control_binding(ControlBinding::radio(
					Signal::new("draft".to_owned()),
					"draft".to_owned(),
				))
				.into_page();

			assert_eq!(
				input.render_to_string(),
				"<input type=\"radio\" value=\"draft\" checked=\"checked\" />"
			);
		});
	}

	#[test]
	fn render_to_string_normalizes_controlled_attribute_names() {
		ReactiveScope::run(|| {
			let input = PageElement::new("INPUT")
				.attr("VALUE", "stale")
				.control_binding(ControlBinding::text(Signal::new("current".to_owned())))
				.into_page();

			assert_eq!(input.render_to_string(), "<INPUT value=\"current\" />");
		});
	}

	#[test]
	fn render_to_string_projects_bound_select_option_state() {
		ReactiveScope::run(|| {
			let single = PageElement::new("select")
				.control_binding(ControlBinding::select_one(Signal::new("wasm".to_owned())))
				.child(
					PageElement::new("option")
						.attr("value", "rust")
						.child("Rust"),
				)
				.child(
					PageElement::new("option")
						.attr("value", "wasm")
						.attr("selected", "selected")
						.child("WebAssembly"),
				)
				.into_page();
			let multiple = PageElement::new("select")
				.attr("multiple", "multiple")
				.control_binding(ControlBinding::select_many(Signal::new(vec![
					"rust".to_owned(),
					"wasm".to_owned(),
				])))
				.child(
					PageElement::new("option")
						.attr("value", "rust")
						.child("Rust"),
				)
				.child(PageElement::new("option").child("WebAssembly"))
				.into_page();

			assert_eq!(
				single.render_to_string(),
				"<select><option value=\"rust\">Rust</option><option value=\"wasm\" selected=\"selected\">WebAssembly</option></select>"
			);
			assert_eq!(
				multiple.render_to_string(),
				"<select multiple=\"multiple\"><option value=\"rust\" selected=\"selected\">Rust</option><option>WebAssembly</option></select>"
			);
		});
	}

	#[test]
	fn render_to_string_projects_bound_select_only_once() {
		ReactiveScope::run(|| {
			let select = PageElement::new("SELECT")
				.control_binding(ControlBinding::select_one(Signal::new("Rust".to_owned())))
				.child(
					PageElement::new("OPTION")
						.attr("value", "Rust")
						.child("First"),
				)
				.child(
					PageElement::new("option")
						.attr("value", "Rust")
						.child("Second"),
				)
				.into_page();

			assert_eq!(
				select.render_to_string(),
				"<SELECT><OPTION value=\"Rust\" selected=\"selected\">First</OPTION><option value=\"Rust\">Second</option></SELECT>"
			);
		});
	}

	#[test]
	fn render_to_string_normalizes_inferred_bound_option_value() {
		ReactiveScope::run(|| {
			let select = PageElement::new("select")
				.control_binding(ControlBinding::select_one(Signal::new("Rust".to_owned())))
				.child(PageElement::new("option").child(" \tRust\n"))
				.into_page();

			assert_eq!(
				select.render_to_string(),
				"<select><option selected=\"selected\"> \tRust\n</option></select>"
			);
		});
	}

	#[test]
	fn render_to_string_sanitizes_svg_script_when_inferring_bound_option_value() {
		ReactiveScope::run(|| {
			// Arrange
			let select = PageElement::new("select")
				.control_binding(ControlBinding::select_one(Signal::new("Rust".to_owned())))
				.child(
					PageElement::new("option")
						.child("Rust")
						.child(PageElement::new("svg:script").child("ignored")),
				)
				.into_page();

			// Act
			let html = select.render_to_string();

			// Assert
			assert_eq!(
				html,
				"<select><option>Rust<span>ignored</span></option></select>"
			);
		});
	}

	#[test]
	fn render_to_string_does_not_evaluate_dynamic_option_content_to_infer_value() {
		ReactiveScope::run(|| {
			// Arrange
			let renders = std::rc::Rc::new(std::cell::Cell::new(0));
			let render_count = std::rc::Rc::clone(&renders);
			let select = PageElement::new("select")
				.control_binding(ControlBinding::select_one(Signal::new(
					"Static Dynamic".to_owned(),
				)))
				.child(
					PageElement::new("option")
						.child("Static")
						.child(Page::reactive(move || {
							render_count.set(render_count.get() + 1);
							Page::text(" Dynamic")
						})),
				)
				.into_page();

			// Act
			let html = select.render_to_string();

			// Assert
			assert_eq!(renders.get(), 1);
			assert_eq!(html, "<select><option>Static Dynamic</option></select>");
		});
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
	fn render_omits_unsafe_attribute_names_and_urls() {
		let view = PageElement::new("a")
			.attr("href", " java\nscript:alert(1)")
			.attr("onclick", "alert(1)")
			.attr("data-safe", "kept")
			.into_page();

		assert_eq!(view.render_to_string(), "<a data-safe=\"kept\"></a>");
	}

	#[test]
	fn is_safe_html_attribute_rejects_javascript_and_data_urls() {
		assert!(!is_safe_html_attribute("href", "javascript:alert(1)"));
		assert!(!is_safe_html_attribute(
			"src",
			"data:text/html,<script>alert(1)</script>"
		));
		assert!(is_safe_html_attribute("href", "https://example.com"));
		assert!(is_safe_html_attribute("class", "container"));
	}

	#[test]
	fn render_omits_executable_elements() {
		let view = PageElement::new("script").child("alert(1)").into_page();

		assert_eq!(view.render_to_string(), "<span>alert(1)</span>");
	}

	#[test]
	fn render_omits_namespace_and_svg_animation_elements() {
		let view = PageElement::new("svg")
			.child(PageElement::new("circle"))
			.child(PageElement::new("animate").attr("values", "javascript:alert(1)"))
			.child(PageElement::new("svg:script").child("alert(2)"))
			.into_page();

		assert_eq!(
			view.render_to_string(),
			"<svg><circle></circle><span values=\"javascript:alert(1)\"></span><span>alert(2)</span></svg>"
		);
	}

	#[test]
	fn render_drops_invalid_tag_markup_but_preserves_children() {
		let view = PageElement::new("div><script")
			.child("safe text")
			.into_page();

		assert_eq!(view.render_to_string(), "<span>safe text</span>");
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
	fn test_render_keyed_fragment() {
		let view = Page::keyed_fragment([("first", "One"), ("second", "Two")]);
		assert_eq!(view.render_to_string(), "OneTwo");
	}

	#[test]
	fn test_render_empty() {
		let view = Page::empty();
		assert_eq!(view.render_to_string(), "");
	}

	#[test]
	fn outlet_inline_renders_child_page() {
		let view = Page::outlet(Outlet::inline(Page::text("Child")));

		assert_eq!(view.render_to_string(), "Child");
	}

	#[test]
	fn outlet_placeholder_renders_empty_on_string_render() {
		let view = Page::outlet(Outlet::placeholder("layout-0"));

		assert_eq!(view.render_to_string(), "");
	}

	#[test]
	fn outlet_inline_participates_in_head_lookup() {
		let view = Page::outlet(Outlet::inline(
			Page::text("Child").with_head(Head::new().title("Child")),
		));

		assert_eq!(
			view.find_topmost_head()
				.and_then(|head| head.title.as_deref()),
			Some("Child")
		);
	}

	#[test]
	fn suspense_head_lookup_uses_fresh_content_per_call() {
		let title = std::rc::Rc::new(std::cell::RefCell::new("first".to_string()));
		let content_title = std::rc::Rc::clone(&title);
		let node = SuspenseNode::new(
			Some("head-boundary".to_string()),
			|| false,
			|| Page::text("loading"),
			move || {
				Page::text("content").with_head(Head::new().title(content_title.borrow().clone()))
			},
		);

		let view = Page::Suspense(node);
		assert_eq!(
			view.find_topmost_head_owned()
				.and_then(|head| head.title.map(|title| title.into_owned())),
			Some("first".to_string())
		);

		*title.borrow_mut() = "second".to_string();
		assert_eq!(
			view.find_topmost_head_owned()
				.and_then(|head| head.title.map(|title| title.into_owned())),
			Some("second".to_string())
		);
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
