//! HTML Element Builder
//!
//! This module provides a fluent API for constructing HTML elements with type-safe operations.
//!
//! ## Design Pattern
//!
//! - **Fluent API**: Method chaining for readable construction
//! - **RAII Integration**: EventHandle management for automatic cleanup
//! - **Reactive Binding**: Seamless integration with `Signal<T>`

use crate::Signal;
use crate::dom::{Document, Element, EventHandle};

/// Most elements have 0-2 event listeners in practice
const TYPICAL_EVENT_COUNT: usize = 2;

/// HTML element builder with fluent API
///
/// This builder allows constructing HTML elements with method chaining.
/// Event listeners are automatically managed through EventHandle.
///
/// ## Example
///
/// ```ignore
/// let button = button()
///     .class("btn btn-primary")
///     .id("submit-button")
///     .text("Submit")
///     .on_click(|| console::log_1(&"Clicked!".into()))
///     .build();
/// ```
pub struct ElementBuilder {
	/// The underlying DOM element
	element: Element,
	/// Event handles for RAII cleanup
	event_handles: Vec<EventHandle>,
}

impl ElementBuilder {
	/// Create a new builder from an element
	pub fn new(element: Element) -> Self {
		Self {
			element,
			event_handles: Vec::with_capacity(TYPICAL_EVENT_COUNT),
		}
	}

	/// Set the class attribute
	///
	/// Multiple calls will overwrite the previous value.
	/// Use space-separated values for multiple classes.
	///
	/// ## Example
	///
	/// ```ignore
	/// div().class("container flex-row").build()
	/// ```
	pub fn class(self, class: &str) -> Self {
		let _ = self.element.set_attribute("class", class);
		self
	}

	/// Set the id attribute
	///
	/// ## Example
	///
	/// ```ignore
	/// div().id("main-content").build()
	/// ```
	pub fn id(self, id: &str) -> Self {
		let _ = self.element.set_attribute("id", id);
		self
	}

	/// Set the style attribute
	///
	/// ## Example
	///
	/// ```ignore
	/// div().style("color: red; font-size: 16px").build()
	/// ```
	pub fn style(self, style: &str) -> Self {
		let _ = self.element.set_attribute("style", style);
		self
	}

	/// Set a custom attribute
	///
	/// ## Example
	///
	/// ```ignore
	/// div().attr("data-test-id", "my-div").build()
	/// ```
	pub fn attr(self, name: &str, value: &str) -> Self {
		let _ = self.element.set_attribute(name, value);
		self
	}

	/// Remove an attribute
	///
	/// ## Example
	///
	/// ```ignore
	/// div().remove_attr("disabled").build()
	/// ```
	pub fn remove_attr(self, name: &str) -> Self {
		let _ = self.element.remove_attribute(name);
		self
	}

	/// Set a reactive attribute bound to a Signal
	///
	/// The attribute will automatically update when the Signal changes.
	///
	/// ## Example
	///
	/// ```ignore
	/// let disabled = Signal::new(false);
	/// button()
	///     .reactive_attr("disabled", disabled)
	///     .build()
	/// ```
	pub fn reactive_attr<T>(self, name: &str, signal: Signal<T>) -> Self
	where
		T: ToString + Clone + 'static,
	{
		self.element.set_reactive_attribute(name, signal);
		self
	}

	/// Set text content
	///
	/// This will replace all children of the element.
	///
	/// ## Example
	///
	/// ```ignore
	/// p().text("Hello, world!").build()
	/// ```
	pub fn text(self, text: &str) -> Self {
		self.element.set_text_content(text);
		self
	}

	/// Append a child element
	///
	/// ## Example
	///
	/// ```ignore
	/// div()
	///     .child(p().text("First paragraph").build())
	///     .child(p().text("Second paragraph").build())
	///     .build()
	/// ```
	pub fn child(self, child: Element) -> Self {
		let _ = self.element.append_child(child);
		self
	}

	/// Add an event listener for any event type
	///
	/// This is a generic method that can handle any DOM event.
	/// For common events, use the convenience methods (`on_click`, `on_input`, etc.).
	///
	/// # Arguments
	///
	/// * `event_type` - DOM event type (e.g., "click", "mouseenter", "touchstart")
	/// * `callback` - Event handler closure
	///
	/// # Example
	///
	/// ```ignore
	/// button()
	///     .on("mouseenter", || console::log_1(&"Mouse entered".into()))
	///     .on("mouseleave", || console::log_1(&"Mouse left".into()))
	///     .build()
	/// ```
	pub fn on<F>(mut self, event_type: &str, callback: F) -> Self
	where
		F: FnMut() + 'static,
	{
		let handle = self.element.add_event_listener(event_type, callback);
		self.event_handles.push(handle);
		self
	}

	/// Add a click event listener
	///
	/// ## Example
	///
	/// ```ignore
	/// button()
	///     .text("Click me")
	///     .on_click(|| console::log_1(&"Clicked!".into()))
	///     .build()
	/// ```
	#[inline]
	pub fn on_click<F>(self, callback: F) -> Self
	where
		F: FnMut() + 'static,
	{
		self.on("click", callback)
	}

	/// Add an input event listener
	///
	/// Commonly used with `<input>` and `<textarea>` elements.
	///
	/// ## Example
	///
	/// ```ignore
	/// input()
	///     .attr("type", "text")
	///     .on_input(|| console::log_1(&"Input changed".into()))
	///     .build()
	/// ```
	#[inline]
	pub fn on_input<F>(self, callback: F) -> Self
	where
		F: FnMut() + 'static,
	{
		self.on("input", callback)
	}

	/// Add a change event listener
	///
	/// Commonly used with `<select>`, `<input type="checkbox">`, etc.
	///
	/// ## Example
	///
	/// ```ignore
	/// input()
	///     .attr("type", "checkbox")
	///     .on_change(|| console::log_1(&"Checkbox toggled".into()))
	///     .build()
	/// ```
	#[inline]
	pub fn on_change<F>(self, callback: F) -> Self
	where
		F: FnMut() + 'static,
	{
		self.on("change", callback)
	}

	/// Add a submit event listener
	///
	/// Commonly used with `<form>` elements.
	///
	/// ## Example
	///
	/// ```ignore
	/// form()
	///     .on_submit(|| console::log_1(&"Form submitted".into()))
	///     .build()
	/// ```
	#[inline]
	pub fn on_submit<F>(self, callback: F) -> Self
	where
		F: FnMut() + 'static,
	{
		self.on("submit", callback)
	}

	/// Add a keydown event listener
	///
	/// ## Example
	///
	/// ```ignore
	/// input()
	///     .on_keydown(|| console::log_1(&"Key pressed".into()))
	///     .build()
	/// ```
	#[inline]
	pub fn on_keydown<F>(self, callback: F) -> Self
	where
		F: FnMut() + 'static,
	{
		self.on("keydown", callback)
	}

	/// Add a focus event listener
	///
	/// ## Example
	///
	/// ```ignore
	/// input()
	///     .on_focus(|| console::log_1(&"Input focused".into()))
	///     .build()
	/// ```
	#[inline]
	pub fn on_focus<F>(self, callback: F) -> Self
	where
		F: FnMut() + 'static,
	{
		self.on("focus", callback)
	}

	/// Add a blur event listener
	///
	/// ## Example
	///
	/// ```ignore
	/// input()
	///     .on_blur(|| console::log_1(&"Input blurred".into()))
	///     .build()
	/// ```
	#[inline]
	pub fn on_blur<F>(self, callback: F) -> Self
	where
		F: FnMut() + 'static,
	{
		self.on("blur", callback)
	}

	/// Finalize the builder and return the Element
	///
	/// Event handles are transferred to the element, ensuring proper cleanup.
	///
	/// ## Example
	///
	/// ```ignore
	/// let element = div().class("container").build();
	/// ```
	pub fn build(self) -> Element {
		// Event handles are dropped here, but they're owned by the element
		// through the closure's captured state. This is safe.
		self.element
	}
}

// ============================================================================
// Helper functions for common HTML elements
// ============================================================================

/// Internal helper for creating element builders (DRY principle)
///
/// # Errors
///
/// Returns an error string if the DOM element cannot be created (e.g., invalid
/// tag name or unavailable DOM environment).
#[inline]
fn try_create_element_builder(tag: &str) -> Result<ElementBuilder, String> {
	let doc = Document::global();
	let element = doc.create_element(tag)?;
	Ok(ElementBuilder::new(element))
}

/// Internal helper for creating element builders (DRY principle)
///
/// # Panics
///
/// Panics if the DOM element cannot be created. This indicates the browser
/// environment is unavailable or severely broken, as creating standard HTML
/// elements should always succeed in a valid DOM context.
#[inline]
fn create_element_builder(tag: &str) -> ElementBuilder {
	try_create_element_builder(tag)
		.unwrap_or_else(|e| panic!("failed to create <{tag}> element: {e}"))
}

/// Macro for defining HTML element creation functions
macro_rules! define_element {
	($(#[$meta:meta])* $name:ident, $tag:literal) => {
		$(#[$meta])*
		pub fn $name() -> ElementBuilder {
			create_element_builder($tag)
		}
	};
}

define_element!(
	/// Create a `<div>` element
	///
	/// ## Example
	///
	/// ```ignore
	/// let container = div()
	///     .class("container")
	///     .child(p().text("Content").build())
	///     .build();
	/// ```
	div, "div"
);

define_element!(
	/// Create a `<span>` element
	///
	/// ## Example
	///
	/// ```ignore
	/// let label = span().text("Label").class("badge").build();
	/// ```
	span, "span"
);

define_element!(
	/// Create a `<p>` element (paragraph)
	///
	/// ## Example
	///
	/// ```ignore
	/// let paragraph = p().text("This is a paragraph.").build();
	/// ```
	p, "p"
);

define_element!(
	/// Create a `<button>` element
	///
	/// ## Example
	///
	/// ```ignore
	/// let button = button()
	///     .text("Click me")
	///     .on_click(|| console::log_1(&"Clicked!".into()))
	///     .build();
	/// ```
	button, "button"
);

define_element!(
	/// Create an `<input>` element
	///
	/// ## Example
	///
	/// ```ignore
	/// let text_input = input()
	///     .attr("type", "text")
	///     .attr("placeholder", "Enter text...")
	///     .on_input(|| console::log_1(&"Input changed".into()))
	///     .build();
	/// ```
	input, "input"
);

define_element!(
	/// Create a `<textarea>` element
	///
	/// ## Example
	///
	/// ```ignore
	/// let textarea = textarea()
	///     .attr("rows", "5")
	///     .attr("placeholder", "Enter long text...")
	///     .build();
	/// ```
	textarea, "textarea"
);

define_element!(
	/// Create a `<select>` element (dropdown)
	///
	/// ## Example
	///
	/// ```ignore
	/// let select = select()
	///     .child(option().attr("value", "1").text("Option 1").build())
	///     .child(option().attr("value", "2").text("Option 2").build())
	///     .build();
	/// ```
	select, "select"
);

define_element!(
	/// Create an `<option>` element (for use with `<select>`)
	///
	/// ## Example
	///
	/// ```ignore
	/// let option = option()
	///     .attr("value", "1")
	///     .text("Option 1")
	///     .build();
	/// ```
	option, "option"
);

define_element!(
	/// Create a `<form>` element
	///
	/// ## Example
	///
	/// ```ignore
	/// let form = form()
	///     .attr("method", "POST")
	///     .attr("action", "/submit")
	///     .on_submit(|| console::log_1(&"Form submitted".into()))
	///     .build();
	/// ```
	form, "form"
);

define_element!(
	/// Create an `<a>` element (hyperlink)
	///
	/// ## Example
	///
	/// ```ignore
	/// let link = a()
	///     .attr("href", "https://example.com")
	///     .text("Visit Example")
	///     .build();
	/// ```
	a, "a"
);

define_element!(
	/// Create an `<img>` element
	///
	/// ## Example
	///
	/// ```ignore
	/// let image = img()
	///     .attr("src", "/images/logo.png")
	///     .attr("alt", "Logo")
	///     .build();
	/// ```
	img, "img"
);

define_element!(
	/// Create a `<h1>` element (heading level 1)
	///
	/// ## Example
	///
	/// ```ignore
	/// let heading = h1().text("Page Title").build();
	/// ```
	h1, "h1"
);

define_element!(
	/// Create a `<h2>` element (heading level 2)
	h2, "h2"
);

define_element!(
	/// Create a `<h3>` element (heading level 3)
	h3, "h3"
);

define_element!(
	/// Create a `<ul>` element (unordered list)
	///
	/// ## Example
	///
	/// ```ignore
	/// let list = ul()
	///     .child(li().text("Item 1").build())
	///     .child(li().text("Item 2").build())
	///     .build();
	/// ```
	ul, "ul"
);

define_element!(
	/// Create an `<ol>` element (ordered list)
	ol, "ol"
);

define_element!(
	/// Create an `<li>` element (list item)
	li, "li"
);

// ============================================================================
// Content Sectioning Elements
// ============================================================================

define_element!(
	/// Create an `<article>` element
	///
	/// Represents a self-contained composition in a document, page, application,
	/// or site, which is intended to be independently distributable or reusable.
	article, "article"
);

define_element!(
	/// Create an `<aside>` element
	///
	/// Represents a portion of a document whose content is only indirectly related
	/// to the document's main content.
	aside, "aside"
);

define_element!(
	/// Create a `<footer>` element
	///
	/// Represents a footer for its nearest ancestor sectioning content or
	/// sectioning root element.
	footer, "footer"
);

define_element!(
	/// Create a `<h4>` element (heading level 4)
	h4, "h4"
);

define_element!(
	/// Create a `<h5>` element (heading level 5)
	h5, "h5"
);

define_element!(
	/// Create a `<h6>` element (heading level 6)
	h6, "h6"
);

define_element!(
	/// Create a `<header>` element
	///
	/// Represents introductory content, typically a group of introductory or
	/// navigational aids.
	header, "header"
);

define_element!(
	/// Create a `<main>` element
	///
	/// Represents the dominant content of the body of a document.
	main, "main"
);

define_element!(
	/// Create a `<nav>` element
	///
	/// Represents a section of a page whose purpose is to provide navigation links.
	nav, "nav"
);

define_element!(
	/// Create a `<section>` element
	///
	/// Represents a generic standalone section of a document.
	section, "section"
);

// ============================================================================
// Text Content Elements
// ============================================================================

define_element!(
	/// Create a `<blockquote>` element
	///
	/// Indicates that the enclosed text is an extended quotation.
	blockquote, "blockquote"
);

define_element!(
	/// Create a `<dd>` element
	///
	/// Provides the description, definition, or value for the preceding term in a
	/// description list.
	dd, "dd"
);

define_element!(
	/// Create a `<dl>` element
	///
	/// Represents a description list.
	dl, "dl"
);

define_element!(
	/// Create a `<dt>` element
	///
	/// Specifies a term in a description or definition list.
	dt, "dt"
);

define_element!(
	/// Create a `<figcaption>` element
	///
	/// Represents a caption or legend describing the rest of the contents of its
	/// parent `<figure>` element.
	figcaption, "figcaption"
);

define_element!(
	/// Create a `<figure>` element
	///
	/// Represents self-contained content, potentially with an optional caption.
	figure, "figure"
);

define_element!(
	/// Create an `<hr>` element
	///
	/// Represents a thematic break between paragraph-level elements.
	hr, "hr"
);

define_element!(
	/// Create a `<pre>` element
	///
	/// Represents preformatted text which is to be presented exactly as written
	/// in the HTML file.
	pre, "pre"
);

// ============================================================================
// Inline Text Semantics Elements
// ============================================================================

define_element!(
	/// Create a `<b>` element
	///
	/// Used to draw the reader's attention to the element's contents.
	b, "b"
);

define_element!(
	/// Create a `<br>` element
	///
	/// Produces a line break in text (carriage-return).
	br, "br"
);

define_element!(
	/// Create a `<code>` element
	///
	/// Displays its contents styled in a fashion intended to indicate that the
	/// text is a short fragment of computer code.
	code, "code"
);

define_element!(
	/// Create an `<em>` element
	///
	/// Marks text that has stress emphasis.
	em, "em"
);

define_element!(
	/// Create an `<i>` element
	///
	/// Represents a range of text that is set off from the normal text for some
	/// reason, such as idiomatic text or technical terms.
	i, "i"
);

define_element!(
	/// Create a `<kbd>` element
	///
	/// Represents a span of inline text denoting textual user input from a keyboard.
	kbd, "kbd"
);

define_element!(
	/// Create a `<mark>` element
	///
	/// Represents text which is marked or highlighted for reference or notation purposes.
	mark, "mark"
);

define_element!(
	/// Create a `<samp>` element
	///
	/// Used to enclose inline text which represents sample (or quoted) output from
	/// a computer program.
	samp, "samp"
);

define_element!(
	/// Create a `<small>` element
	///
	/// Represents side-comments and small print, like copyright and legal text.
	small, "small"
);

define_element!(
	/// Create a `<strong>` element
	///
	/// Indicates that its contents have strong importance, seriousness, or urgency.
	strong, "strong"
);

define_element!(
	/// Create a `<time>` element
	///
	/// Represents a specific period in time.
	time, "time"
);

define_element!(
	/// Create a `<u>` element
	///
	/// Represents a span of inline text which should be rendered in a way that
	/// indicates that it has a non-textual annotation.
	u, "u"
);

define_element!(
	/// Create a `<var>` element
	///
	/// Represents the name of a variable in a mathematical expression or a
	/// programming context.
	var, "var"
);

// ============================================================================
// Table Content Elements
// ============================================================================

define_element!(
	/// Create a `<caption>` element
	///
	/// Specifies the caption (or title) of a table.
	caption, "caption"
);

define_element!(
	/// Create a `<colgroup>` element
	///
	/// Defines a group of columns within a table.
	colgroup, "colgroup"
);

define_element!(
	/// Create a `<table>` element
	///
	/// Represents tabular data—that is, information presented in a two-dimensional
	/// table comprised of rows and columns of cells containing data.
	table, "table"
);

define_element!(
	/// Create a `<tbody>` element
	///
	/// Encapsulates a set of table rows, indicating that they comprise the body
	/// of a table's data.
	tbody, "tbody"
);

define_element!(
	/// Create a `<td>` element
	///
	/// Defines a cell of a table that contains data.
	td, "td"
);

define_element!(
	/// Create a `<tfoot>` element
	///
	/// Encapsulates a set of table rows, indicating that they comprise the foot
	/// of a table.
	tfoot, "tfoot"
);

define_element!(
	/// Create a `<th>` element
	///
	/// Defines a cell as the header of a group of table cells.
	th, "th"
);

define_element!(
	/// Create a `<thead>` element
	///
	/// Encapsulates a set of table rows, indicating that they comprise the head
	/// of a table.
	thead, "thead"
);

define_element!(
	/// Create a `<tr>` element
	///
	/// Defines a row of cells in a table.
	tr, "tr"
);

// ============================================================================
// Form Elements
// ============================================================================

define_element!(
	/// Create a `<datalist>` element
	///
	/// Contains a set of `<option>` elements that represent the permissible or
	/// recommended options available to choose from within other controls.
	datalist, "datalist"
);

define_element!(
	/// Create a `<fieldset>` element
	///
	/// Used to group several controls as well as labels within a web form.
	fieldset, "fieldset"
);

define_element!(
	/// Create a `<label>` element
	///
	/// Represents a caption for an item in a user interface.
	label, "label"
);

define_element!(
	/// Create a `<legend>` element
	///
	/// Represents a caption for the content of its parent `<fieldset>`.
	legend, "legend"
);

define_element!(
	/// Create an `<output>` element
	///
	/// Container element into which a site or app can inject the results of a
	/// calculation or the outcome of a user action.
	output, "output"
);

define_element!(
	/// Create a `<progress>` element
	///
	/// Displays an indicator showing the completion progress of a task.
	progress, "progress"
);

// ============================================================================
// Interactive Elements
// ============================================================================

define_element!(
	/// Create a `<details>` element
	///
	/// Creates a disclosure widget in which information is visible only when the
	/// widget is toggled into an "open" state.
	details, "details"
);

define_element!(
	/// Create a `<summary>` element
	///
	/// Specifies a summary, caption, or legend for a `<details>` element's
	/// disclosure box.
	summary, "summary"
);

// ============================================================================
// Image and Multimedia Elements
// ============================================================================

define_element!(
	/// Create an `<audio>` element
	///
	/// Used to embed sound content in documents.
	audio, "audio"
);

define_element!(
	/// Create a `<picture>` element
	///
	/// Contains zero or more `<source>` elements and one `<img>` element to offer
	/// alternative versions of an image for different display/device scenarios.
	picture, "picture"
);

define_element!(
	/// Create a `<source>` element
	///
	/// Specifies multiple media resources for the `<picture>`, `<audio>`, or
	/// `<video>` element.
	source, "source"
);

define_element!(
	/// Create a `<video>` element
	///
	/// Embeds a media player which supports video playback into the document.
	video, "video"
);

// ============================================================================
// Demarcating Edits Elements
// ============================================================================

define_element!(
	/// Create a `<del>` element
	///
	/// Represents a range of text that has been deleted from a document.
	del, "del"
);

define_element!(
	/// Create an `<ins>` element
	///
	/// Represents a range of text that has been added to a document.
	ins, "ins"
);

// ============================================================================
// Web Components Elements
// ============================================================================

define_element!(
	/// Create a `<template>` element
	///
	/// A mechanism for holding HTML that is not to be rendered immediately when
	/// a page is loaded but may be instantiated subsequently during runtime using
	/// JavaScript.
	template, "template"
);
