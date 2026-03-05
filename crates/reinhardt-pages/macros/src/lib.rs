//! Procedural Macros for Reinhardt Pages
//!
//! This crate provides procedural macros for the reinhardt-pages WASM frontend framework.
//!
//! ## Available Macros
//!
//! - `page!` - Anonymous component DSL macro
//! - `head!` - HTML head section DSL macro
//! - `form!` - Type-safe form component macro with reactive bindings
//! - `#[server_fn]` - Server Functions (RPC) macro
//!
//! ## page! Macro Example
//!
//! ```ignore
//! use reinhardt_pages::page;
//!
//! // Define an anonymous component with closure-style props
//! let counter = page!(|initial: i32| {
//!     div {
//!         class: "counter",
//!         h1 { "Counter" }
//!         span { format!("Count: {}", initial) }
//!         button {
//!             @click: |_| { /* handler */ },
//!             "+"
//!         }
//!     }
//! });
//!
//! // Use like a function
//! let view = counter(42);
//! ```
//!
//! ## server_fn Example
//!
//! ```ignore
//! use reinhardt_pages_macros::server_fn;
//!
//! #[server_fn]
//! async fn get_user(id: u32) -> Result<User, ServerFnError> {
//!     // Server-side code (automatically removed on WASM build)
//!     let user = User::find_by_id(id).await?;
//!     Ok(user)
//! }
//!
//! // On client (WASM), this expands to an HTTP request
//! // On server, this expands to a route handler
//! ```

use proc_macro::TokenStream;

mod crate_paths;
mod form;
mod head;
mod page;
mod server_fn;

/// Server Function macro
///
/// This macro generates client-side stub (WASM) and server-side handler (non-WASM)
/// for seamless RPC communication between frontend and backend.
///
/// ## Basic Usage
///
/// ```ignore
/// #[server_fn]
/// async fn get_user(id: u32) -> Result<User, ServerFnError> {
///     // Server-side implementation
///     let user = User::find_by_id(id).await?;
///     Ok(user)
/// }
/// ```
///
/// ## Options
///
/// - `use_inject = true` - Enable dependency injection
/// - `endpoint = "/custom/path"` - Custom endpoint path
/// - `codec = "json"` - Serialization codec (json, url, msgpack)
///
/// ```ignore
/// #[server_fn(endpoint = "/api/users/get")]
/// async fn get_user(id: u32) -> Result<User, ServerFnError> {
///     // ...
/// }
/// ```
#[proc_macro_attribute]
pub fn server_fn(args: TokenStream, input: TokenStream) -> TokenStream {
	server_fn::server_fn_impl(args, input)
}

/// Page component macro
///
/// Creates an anonymous component with a closure-style DSL for defining views.
/// The component is returned as a callable function that takes props and returns a View.
///
/// ## Syntax
///
/// ```text
/// // Basic syntax
/// page!(|prop1: Type1, prop2: Type2| {
///     element {
///         attr: "value",
///         @event: |e| { handler(e) },
///         child_element { ... }
///         "text content"
///     }
/// })
///
/// // With head directive (for SSR)
/// page! {
///     #head: my_head,
///     |prop1: Type1| {
///         element { ... }
///     }
/// }
/// ```
///
/// ## Closure Parameters
///
/// Define props using closure syntax:
///
/// | Pattern | Example | Description |
/// |---------|---------|-------------|
/// | No parameters | `page!(\|\| { ... })` | Static view |
/// | Single parameter | `page!(\|name: String\| { ... })` | One prop |
/// | Multiple parameters | `page!(\|a: T1, b: T2\| { ... })` | Multiple props |
/// | Signal parameter | `page!(\|sig: Signal<T>\| { ... })` | Reactive signal |
///
/// ## HTML Elements
///
/// HTML elements are written as `tag { ... }`. The macro supports 70+ HTML elements
/// with compile-time validation.
///
/// ### Structural Elements
///
/// | Element | Description |
/// |---------|-------------|
/// | `div` | Generic container |
/// | `span` | Inline container |
/// | `p` | Paragraph |
/// | `header` | Header section |
/// | `footer` | Footer section |
/// | `main` | Main content |
/// | `nav` | Navigation |
/// | `section` | Generic section |
/// | `article` | Article content |
///
/// ### Headings
///
/// | Element | Description |
/// |---------|-------------|
/// | `h1` - `h6` | Heading levels 1-6 |
///
/// ### Text-Level Elements
///
/// | Element | Description |
/// |---------|-------------|
/// | `em`, `strong` | Emphasis, strong emphasis |
/// | `small`, `mark` | Small text, highlighted text |
/// | `b`, `i`, `u`, `s` | Bold, italic, underline, strikethrough |
/// | `code`, `kbd`, `samp`, `var` | Code, keyboard input, sample output, variable |
/// | `sub`, `sup` | Subscript, superscript |
/// | `br`, `wbr` | Line break, word break opportunity |
/// | `cite`, `abbr`, `time`, `dfn` | Citation, abbreviation, time, definition |
/// | `ins`, `del` | Inserted, deleted text |
/// | `q`, `blockquote` | Inline quote, block quote |
///
/// ### List Elements
///
/// | Element | Description |
/// |---------|-------------|
/// | `ul`, `ol` | Unordered list, ordered list |
/// | `li` | List item |
/// | `dl`, `dt`, `dd` | Definition list, term, description |
///
/// ### Table Elements
///
/// | Element | Description |
/// |---------|-------------|
/// | `table` | Table container |
/// | `thead`, `tbody`, `tfoot` | Table sections |
/// | `tr` | Table row |
/// | `th`, `td` | Header cell, data cell |
/// | `caption` | Table caption |
/// | `colgroup`, `col` | Column group, column |
///
/// ### Form Elements
///
/// | Element | Description |
/// |---------|-------------|
/// | `form` | Form container |
/// | `input` | Input field (void element) |
/// | `button` | Button |
/// | `label` | Form label |
/// | `select`, `option`, `optgroup` | Dropdown, option, option group |
/// | `textarea` | Multi-line text input |
///
/// ### Embedded Content
///
/// | Element | Description |
/// |---------|-------------|
/// | `img` | Image (requires `src` and `alt`) |
/// | `iframe` | Inline frame |
/// | `video`, `audio` | Video, audio player |
/// | `source`, `track` | Media source, text track |
/// | `canvas` | Drawing canvas |
/// | `picture` | Responsive image container |
/// | `script`, `noscript` | Script, fallback for no script |
/// | `object`, `param` | Embedded object, object parameters |
/// | `embed` | Embedded content |
///
/// ### Other Elements
///
/// | Element | Description |
/// |---------|-------------|
/// | `a` | Anchor/link |
/// | `hr` | Horizontal rule (void element) |
/// | `pre` | Preformatted text |
/// | `figure`, `figcaption` | Figure, figure caption |
/// | `details`, `summary` | Collapsible section |
/// | `dialog` | Dialog box |
/// | `data` | Machine-readable data value |
/// | `ruby`, `rt`, `rp` | Ruby annotations for East Asian typography |
/// | `bdi`, `bdo` | Bidirectional text isolation/override |
/// | `address` | Contact information |
/// | `template`, `slot` | HTML template, Web Components slot |
///
/// ### Void Elements
///
/// These elements cannot have children:
///
/// `br`, `col`, `embed`, `hr`, `img`, `input`, `param`, `source`, `track`, `wbr`
///
/// ## Attributes
///
/// Attributes use `key: value` syntax with automatic underscore-to-hyphen conversion.
///
/// ### Global Attributes
///
/// Available on all elements:
///
/// | Attribute | Description |
/// |-----------|-------------|
/// | `id` | Unique identifier |
/// | `class` | CSS class names |
/// | `style` | Inline CSS |
/// | `title` | Advisory title |
/// | `lang` | Language code |
/// | `dir` | Text direction (`ltr`, `rtl`) |
/// | `tabindex` | Tab order (integer) |
/// | `hidden` | Hidden state (boolean expression) |
/// | `role` | ARIA role |
/// | `data_*` | Custom data attributes (converted to `data-*`) |
/// | `aria_*` | Accessibility attributes (converted to `aria-*`) |
///
/// ### Attribute Value Types
///
/// | Type | Syntax | Example |
/// |------|--------|---------|
/// | String literal | `attr: "value"` | `class: "container"` |
/// | Expression | `attr: expr` | `class: css_class` |
/// | Integer literal | `attr: number` | `tabindex: 1` |
/// | Boolean expression | `attr: expr` | `disabled: is_disabled` |
///
/// ### Boolean Attributes
///
/// **Important**: Boolean attributes require expressions, not literals.
///
/// ```ignore
/// // CORRECT:
/// button { disabled: is_disabled }
/// input { checked: is_checked }
///
/// // INCORRECT (compile error):
/// button { disabled: true }      // ❌ Boolean literal not allowed
/// button { disabled: "disabled" } // ❌ String literal not allowed
/// ```
///
/// Boolean attributes: `disabled`, `required`, `readonly`, `checked`, `selected`,
/// `autofocus`, `autoplay`, `controls`, `loop`, `muted`, `default`, `defer`,
/// `formnovalidate`, `hidden`, `ismap`, `multiple`, `novalidate`, `open`, `reversed`
///
/// ### Numeric Attributes
///
/// Must be integer literals or expressions:
///
/// `size`, `min`, `max`, `step`, `rows`, `cols`, `colspan`, `rowspan`,
/// `tabindex`, `maxlength`, `minlength`
///
/// ### Enumerated Attributes
///
/// | Element | Attribute | Allowed Values |
/// |---------|-----------|----------------|
/// | `input` | `type` | `text`, `password`, `email`, `number`, `tel`, `url`, `search`, `checkbox`, `radio`, `submit`, `button`, `reset`, `file`, `hidden`, `date`, `datetime-local`, `time`, `week`, `month`, `color`, `range`, `image` |
/// | `button` | `type` | `submit`, `button`, `reset` |
/// | `form` | `method` | `get`, `post`, `dialog` |
/// | `form` | `enctype` | `application/x-www-form-urlencoded`, `multipart/form-data`, `text/plain` |
/// | `script` | `type` | `module`, `text/javascript`, `application/javascript` |
///
/// ### Example
///
/// ```ignore
/// page!(|| {
///     div {
///         class: "container",
///         id: "main",
///         data_testid: "test",  // Converts to data-testid
///         aria_label: "Main content",  // Converts to aria-label
///     }
/// })
/// ```
///
/// ## Event Handlers
///
/// Events use `@event: handler` syntax.
///
/// ### Mouse Events
///
/// | Event | Description |
/// |-------|-------------|
/// | `@click` | Mouse click |
/// | `@dblclick` | Double click |
/// | `@mousedown` | Mouse button pressed |
/// | `@mouseup` | Mouse button released |
/// | `@mouseenter` | Mouse enters element |
/// | `@mouseleave` | Mouse leaves element |
/// | `@mousemove` | Mouse moves over element |
/// | `@mouseover` | Mouse over element (bubbles) |
/// | `@mouseout` | Mouse out of element (bubbles) |
///
/// ### Keyboard Events
///
/// | Event | Description |
/// |-------|-------------|
/// | `@keydown` | Key pressed |
/// | `@keyup` | Key released |
/// | `@keypress` | Key pressed (character input) |
///
/// ### Form Events
///
/// | Event | Description |
/// |-------|-------------|
/// | `@input` | Input value changed |
/// | `@change` | Value changed and committed |
/// | `@submit` | Form submitted |
/// | `@focus` | Element focused |
/// | `@blur` | Element lost focus |
///
/// ### Touch Events
///
/// | Event | Description |
/// |-------|-------------|
/// | `@touchstart` | Touch started |
/// | `@touchend` | Touch ended |
/// | `@touchmove` | Touch moved |
/// | `@touchcancel` | Touch cancelled |
///
/// ### Drag Events
///
/// | Event | Description |
/// |-------|-------------|
/// | `@dragstart` | Drag started |
/// | `@drag` | Dragging |
/// | `@drop` | Dropped |
/// | `@dragenter` | Drag entered element |
/// | `@dragleave` | Drag left element |
/// | `@dragover` | Drag over element |
/// | `@dragend` | Drag ended |
///
/// ### Other Events
///
/// | Event | Description |
/// |-------|-------------|
/// | `@load` | Resource loaded |
/// | `@error` | Error occurred |
/// | `@scroll` | Element scrolled |
/// | `@resize` | Element/window resized |
///
/// ### Handler Syntax
///
/// ```ignore
/// // Inline closure with event parameter
/// button { @click: |e| { handle_click(e); } }
///
/// // Closure ignoring event
/// button { @click: |_| { do_something(); } }
///
/// // Function reference
/// fn handle_click(_event: Event) { ... }
/// button { @click: handle_click }
/// ```
///
/// **Note**: Closures must have 0 or 1 parameter (compile error if more).
///
/// ## Child Nodes
///
/// ### Text Content
///
/// ```ignore
/// div { "Hello, World!" }
/// p { "Paragraph text" }
/// ```
///
/// ### Expressions
///
/// ```ignore
/// div { name }                    // Variable
/// div { name.to_string() }        // Method call
/// div { format!("{}", count) }    // Macro
/// div { { complex_expr } }        // Braced expression
/// ```
///
/// ### Nested Elements
///
/// ```ignore
/// div {
///     h1 { "Title" }
///     p { "Content" }
/// }
/// ```
///
/// ## Conditional Rendering
///
/// ### Basic if
///
/// ```ignore
/// div {
///     if condition {
///         span { "Visible when true" }
///     }
/// }
/// ```
///
/// ### if-else
///
/// ```ignore
/// div {
///     if condition {
///         span { "True branch" }
///     } else {
///         span { "False branch" }
///     }
/// }
/// ```
///
/// ### if-else if-else
///
/// ```ignore
/// div {
///     if count > 10 {
///         span { "Greater" }
///     } else if count == 10 {
///         span { "Equal" }
///     } else {
///         span { "Less" }
///     }
/// }
/// ```
///
/// ## List Rendering
///
/// ### Simple for Loop
///
/// ```ignore
/// ul {
///     for item in items {
///         li { item }
///     }
/// }
/// ```
///
/// ### With Destructuring
///
/// ```ignore
/// ul {
///     for (index, item) in items.iter().enumerate() {
///         li { { index.to_string() } ": " { item } }
///     }
/// }
/// ```
///
/// ### Nested Loops
///
/// ```ignore
/// div {
///     for row in matrix {
///         div {
///             for cell in row {
///                 span { { cell.to_string() } }
///             }
///         }
///     }
/// }
/// ```
///
/// ## Reactive Features
///
/// ### watch Blocks
///
/// Use `watch` for Signal-dependent reactive rendering:
///
/// ```ignore
/// page!(|error: Signal<Option<String>>| {
///     div {
///         watch {
///             if error.get().is_some() {
///                 div {
///                     class: "alert",
///                     { error.get().unwrap_or_default() }
///                 }
///             }
///         }
///     }
/// })(error.clone())
/// ```
///
/// ### When to Use watch
///
/// | Scenario | Solution |
/// |----------|----------|
/// | Static condition on Copy type | Plain `if` |
/// | Dynamic Signal-dependent condition | `watch { if signal.get() { ... } }` |
/// | Multiple reactive branches | `watch { match state.get() { ... } }` |
///
/// ### watch with match
///
/// ```ignore
/// watch {
///     match state.get() {
///         State::Loading => div { "Loading..." },
///         State::Ready(data) => div { { data } },
///         State::Error(msg) => div { class: "error", { msg } },
///     }
/// }
/// ```
///
/// ## Components
///
/// ### Component Calls
///
/// Call components with named arguments:
///
/// ```ignore
/// MyButton(label: "Click me")
///
/// MyCard(title: "Card", content: "Content", class: "custom")
/// ```
///
/// ### Component with Children
///
/// ```ignore
/// MyWrapper(class: "container") {
///     p { "Child content" }
///     span { "More content" }
/// }
/// ```
///
/// ## Head Directive (SSR)
///
/// For Server-Side Rendering, inject head content using `#head`:
///
/// ```ignore
/// page! {
///     #head: my_head,
///     || {
///         div { "Page content" }
///     }
/// }
/// ```
///
/// The head expression is called with `.with_head(head_expr)` on the resulting view.
///
/// ## Validation Rules
///
/// The macro performs compile-time validation:
///
/// ### Accessibility Requirements
///
/// | Element | Requirement |
/// |---------|-------------|
/// | `img` | Must have `src` (string literal) and `alt` attributes |
/// | `button` | Must have text content or `aria-label`/`aria-labelledby` |
///
/// ### Security Validation
///
/// URL attributes block dangerous schemes to prevent XSS:
///
/// - ❌ `javascript:`
/// - ❌ `data:`
/// - ❌ `vbscript:`
///
/// Applies to: `href`, `src`, `action`, `formaction`
///
/// ### Element Nesting Rules
///
/// | Rule | Description |
/// |------|-------------|
/// | Void elements | Cannot have children (e.g., `img`, `input`, `br`) |
/// | Interactive elements | Cannot nest inside each other (`button`, `a`, `label`, `select`, `textarea`) |
/// | `select` | Can only contain `option` and `optgroup` |
/// | `ul`, `ol` | Can only contain `li` |
/// | `dl` | Can only contain `dt`, `dd`, and `div` |
///
/// ## Generated Code Structure
///
/// The `page!` macro generates a closure returning a `View`:
///
/// ```ignore
/// // page!(|name: String| { div { class: "greeting", { name } } })
/// // Generates approximately:
/// |name: String| -> View {
///     ElementView::new("div")
///         .attr("class", "greeting")
///         .child(name)
///         .into_view()
/// }
/// ```
///
/// ### Event Handler Generation
///
/// ```ignore
/// // button { @click: |_| { handle() } }
/// // Generates (on WASM):
/// ElementView::new("button")
///     .on(EventType::Click, Arc::new(move |_| { handle() }))
/// ```
///
/// ## SSR/CSR Considerations
///
/// - **Event handlers**: Active on WASM (client), no-op on server (native)
/// - **head directive**: Enables SSR metadata injection
/// - **Same source code**: Works for both WASM and native targets
/// - **Conditional compilation**: Events are type-checked but ignored on server
///
/// ## Complete Example
///
/// ```ignore
/// use reinhardt_pages::prelude::*;
///
/// fn todo_app(todos: Signal<Vec<String>>, filter: Signal<String>) -> View {
///     page!(|todos: Signal<Vec<String>>, filter: Signal<String>| {
///         div {
///             class: "todo-app",
///
///             header {
///                 h1 { "My Todo App" }
///                 input {
///                     type: "text",
///                     placeholder: "Add a todo...",
///                     @input: |e| { /* handle input */ },
///                 }
///             }
///
///             nav {
///                 for filter_type in vec!["all", "active", "completed"] {
///                     button {
///                         @click: move |_| { /* set filter */ },
///                         { filter_type }
///                     }
///                 }
///             }
///
///             ul {
///                 class: "todo-list",
///                 watch {
///                     if todos.get().is_empty() {
///                         li { class: "empty", "No todos yet" }
///                     }
///                 }
///             }
///
///             footer {
///                 aria_label: "Todo stats",
///                 data_testid: "footer",
///                 { format!("{} items", todos.get().len()) }
///             }
///         }
///     })(todos, filter)
/// }
/// ```
#[proc_macro]
pub fn page(input: TokenStream) -> TokenStream {
	page::page_impl(input)
}

/// Head section macro
///
/// Creates an HTML head section with a concise DSL.
/// The macro returns a `Head` struct that can be used with SSR rendering.
///
/// ## Syntax
///
/// ```text
/// head!(|| {
///     title { "Page Title" }
///     meta { name: "description", content: "..." }
///     link { rel: "stylesheet", href: "..." }
///     script { src: "...", defer }
/// })
/// ```
///
/// ## Elements
///
/// ### Title
///
/// ```ignore
/// head!(|| {
///     title { "My Page Title" }
/// })
/// ```
///
/// ### Meta Tags
///
/// ```ignore
/// head!(|| {
///     meta { name: "description", content: "Page description" }
///     meta { property: "og:title", content: "Open Graph Title" }
///     meta { charset: "UTF-8" }
/// })
/// ```
///
/// ### Link Tags
///
/// ```ignore
/// head!(|| {
///     link { rel: "stylesheet", href: "/static/style.css" }
///     link { rel: "icon", href: "/favicon.png", type: "image/png" }
///     link { rel: "preload", href: "/static/app.js", as_: "script" }
/// })
/// ```
///
/// ### Script Tags
///
/// ```ignore
/// head!(|| {
///     script { src: "/static/app.js", defer }
///     script { src: "/static/analytics.js", async_ }
///     script { type: "module", src: "/static/main.js" }
///     script { "console.log('inline script');" }
/// })
/// ```
///
/// ### Style Tags
///
/// ```ignore
/// head!(|| {
///     style { "body { margin: 0; }" }
/// })
/// ```
///
/// ## Example
///
/// ```ignore
/// use reinhardt_pages::head;
///
/// let my_head = head!(|| {
///     title { "My Application" }
///     meta { name: "description", content: "A great application" }
///     meta { name: "viewport", content: "width=device-width, initial-scale=1.0" }
///     link { rel: "icon", href: "/favicon.png", type: "image/png" }
///     link { rel: "stylesheet", href: "/static/css/style.css" }
///     script { src: "/static/js/app.js", defer }
/// });
///
/// // Use with SSR
/// let html = my_head.to_html();
/// ```
#[proc_macro]
pub fn head(input: TokenStream) -> TokenStream {
	head::head_impl(input)
}

/// Form component macro
///
/// Creates a type-safe form with reactive bindings and validation support.
/// The macro generates a form struct with Signal-bound fields and view generation.
///
/// # Table of Contents
///
/// - [Overview](#overview) - Quick introduction and getting started
/// - [Core Concepts](#core-concepts) - Syntax, attributes, HTTP methods
/// - [Fields](#fields) - Field types, widgets, properties, and groups
/// - [Validation](#validation) - Server-side and client-side validators
/// - [Examples](#examples) - Practical usage examples
/// - [Integration](#integration) - Integration with server_fn and page! macro
/// - [Reference](#reference) - Generated code and SSR/CSR considerations
/// - [Reactive Features](#reactive-features) - State management and bindings
/// - [Customization](#customization) - Wrappers, icons, attributes, and slots
/// - [Advanced Features](#advanced-features) - Callbacks, redirects, initial loading
///
/// # Overview
///
/// The `form!` macro provides a declarative DSL for creating type-safe forms
/// with automatic Signal binding, validation, and server integration.
///
/// Key features:
/// - **Type-safe fields**: Each field type maps to appropriate Rust and HTML types
/// - **Reactive bindings**: Automatic Signal creation for two-way data binding
/// - **Built-in validation**: Server-side and client-side validation support
/// - **Server integration**: Works with `#[server_fn]` for seamless RPC
/// - **SSR compatible**: Generates both server and client code
///
/// ## Quick Start
///
/// ```ignore
/// use reinhardt_pages::form;
///
/// // Create a simple login form
/// let form = form! {
///     name: LoginForm,
///     action: "/api/login",
///
///     fields: {
///         username: CharField { required, label: "Username" },
///         password: CharField { required, widget: PasswordInput, label: "Password" },
///     },
/// };
///
/// // Render the form
/// let view = form.into_view();
/// ```
///
/// # Core Concepts
///
/// ## Syntax
///
/// ```text
/// form! {
///     name: FormName,
///     action: "/api/endpoint",    // OR server_fn: function_name
///     method: Post,               // Optional, defaults to Post
///     class: "form-class",        // Optional, form CSS class
///
///     fields: {
///         field_name: FieldType {
///             required,           // Validation
///             max_length: 150,    // Constraints
///             label: "Label",     // Display
///             class: "input",     // Styling
///         },
///     },
///
///     validators: {               // Optional server-side validators
///         field_name: [
///             |v| !v.is_empty() => "Error message",
///         ],
///     },
///
///     client_validators: {        // Optional client-side validators
///         field_name: [
///             "value.length > 0" => "Error message",
///         ],
///     },
/// }
/// ```
///
/// ## Form-Level Attributes
///
/// | Attribute | Type | Required | Description |
/// |-----------|------|----------|-------------|
/// | `name` | Ident | Yes | Form struct name |
/// | `action` | String | One of action/server_fn | URL endpoint for form submission |
/// | `server_fn` | Path | One of action/server_fn | Server function for type-safe RPC |
/// | `method` | Method | No | HTTP method (default: `Post`) |
/// | `class` | String | No | Form CSS class (default: `"reinhardt-form"`) |
/// | `initial_loader` | Path | No | Server function for loading initial values |
/// | `redirect_on_success` | String | No | URL to redirect to after successful submission |
///
/// ## HTTP Methods
///
/// | Method | Syntax | HTML Output | Notes |
/// |--------|--------|-------------|-------|
/// | GET | `method: Get` | `method="get"` | Standard HTML |
/// | POST | `method: Post` | `method="post"` | Default |
/// | PUT | `method: Put` | `method="post"` + hidden `_method` field | JS required |
/// | PATCH | `method: Patch` | `method="post"` + hidden `_method` field | JS required |
/// | DELETE | `method: Delete` | `method="post"` + hidden `_method` field | JS required |
///
/// # Fields
///
/// ## Field Types
///
/// ### String Fields
///
/// | Field Type | Rust Type | Default Widget | Description |
/// |------------|-----------|----------------|-------------|
/// | `CharField` | `String` | `TextInput` | General text input |
/// | `TextField` | `String` | `Textarea` | Multi-line text |
/// | `EmailField` | `String` | `EmailInput` | Email with validation |
/// | `PasswordField` | `String` | `PasswordInput` | Password input (masked) |
/// | `UrlField` | `String` | `UrlInput` | URL input |
/// | `SlugField` | `String` | `TextInput` | URL-safe slug |
/// | `UuidField` | `String` | `TextInput` | UUID input |
/// | `IpAddressField` | `String` | `TextInput` | IP address |
/// | `JsonField` | `String` | `Textarea` | JSON data |
/// | `HiddenField` | `String` | `HiddenInput` | Hidden field |
///
/// ### Numeric Fields
///
/// | Field Type | Rust Type | Default Widget | Description |
/// |------------|-----------|----------------|-------------|
/// | `IntegerField` | `i64` | `NumberInput` | Integer input |
/// | `FloatField` | `f64` | `NumberInput` | Floating point |
/// | `DecimalField` | `f64` | `NumberInput` | Decimal number |
///
/// ### Boolean Fields
///
/// | Field Type | Rust Type | Default Widget | Description |
/// |------------|-----------|----------------|-------------|
/// | `BooleanField` | `bool` | `CheckboxInput` | True/false checkbox |
///
/// ### Date/Time Fields
///
/// | Field Type | Rust Type | Default Widget | Description |
/// |------------|-----------|----------------|-------------|
/// | `DateField` | `Option<NaiveDate>` | `DateInput` | Date picker |
/// | `TimeField` | `Option<NaiveTime>` | `TimeInput` | Time picker |
/// | `DateTimeField` | `Option<NaiveDateTime>` | `DateTimeInput` | Date and time |
///
/// ### Choice Fields
///
/// | Field Type | Rust Type | Default Widget | Description |
/// |------------|-----------|----------------|-------------|
/// | `ChoiceField` | `String` | `Select` | Single-select dropdown |
/// | `MultipleChoiceField` | `Vec<String>` | `SelectMultiple` | Multi-select |
///
/// ### File Fields
///
/// | Field Type | Rust Type | Default Widget | Description |
/// |------------|-----------|----------------|-------------|
/// | `FileField` | `Option<web_sys::File>` | `FileInput` | File upload |
/// | `ImageField` | `Option<web_sys::File>` | `FileInput` | Image upload |
///
/// ## Widget Types
///
/// ### Text Widgets
///
/// | Widget | HTML Element | Input Type | Description |
/// |--------|--------------|------------|-------------|
/// | `TextInput` | `<input>` | `text` | Single-line text |
/// | `EmailInput` | `<input>` | `email` | Email with browser validation |
/// | `PasswordInput` | `<input>` | `password` | Masked password |
/// | `UrlInput` | `<input>` | `url` | URL input |
/// | `TelInput` | `<input>` | `tel` | Telephone number |
/// | `SearchInput` | `<input>` | `search` | Search field |
/// | `Textarea` | `<textarea>` | - | Multi-line text |
///
/// ### Numeric Widgets
///
/// | Widget | HTML Element | Input Type | Description |
/// |--------|--------------|------------|-------------|
/// | `NumberInput` | `<input>` | `number` | Numeric input |
/// | `RangeInput` | `<input>` | `range` | Slider |
///
/// ### Date/Time Widgets
///
/// | Widget | HTML Element | Input Type | Description |
/// |--------|--------------|------------|-------------|
/// | `DateInput` | `<input>` | `date` | Date picker |
/// | `TimeInput` | `<input>` | `time` | Time picker |
/// | `DateTimeInput` | `<input>` | `datetime-local` | Date and time |
///
/// ### Selection Widgets
///
/// | Widget | HTML Element | Input Type | Description |
/// |--------|--------------|------------|-------------|
/// | `CheckboxInput` | `<input>` | `checkbox` | Single checkbox |
/// | `RadioInput` | `<input>` | `radio` | Single radio button |
/// | `RadioSelect` | multiple `<input>` | `radio` | Radio button group |
/// | `Select` | `<select>` | - | Dropdown select |
/// | `SelectMultiple` | `<select multiple>` | - | Multi-select list |
///
/// ### Other Widgets
///
/// | Widget | HTML Element | Input Type | Description |
/// |--------|--------------|------------|-------------|
/// | `FileInput` | `<input>` | `file` | File chooser |
/// | `HiddenInput` | `<input>` | `hidden` | Hidden field |
/// | `ColorInput` | `<input>` | `color` | Color picker |
///
/// ## Field Properties
///
/// ### Validation Properties
///
/// | Property | Type | Syntax | Description |
/// |----------|------|--------|-------------|
/// | `required` | flag/bool | `required` or `required: true` | Field must have value |
/// | `min_length` | i64 | `min_length: 3` | Minimum string length |
/// | `max_length` | i64 | `max_length: 150` | Maximum string length |
/// | `min_value` | i64 | `min_value: 0` | Minimum numeric value |
/// | `max_value` | i64 | `max_value: 100` | Maximum numeric value |
/// | `pattern` | String | `pattern: "[0-9]+"` | Regex pattern |
///
/// ### Display Properties
///
/// | Property | Type | Syntax | Description |
/// |----------|------|--------|-------------|
/// | `label` | String | `label: "Username"` | Field label text |
/// | `placeholder` | String | `placeholder: "Enter..."` | Input placeholder |
/// | `help_text` | String | `help_text: "Max 150 chars"` | Help text below field |
/// | `disabled` | flag/bool | `disabled` or `disabled: true` | Disable input |
/// | `readonly` | flag/bool | `readonly` or `readonly: true` | Read-only input |
/// | `autofocus` | flag/bool | `autofocus` or `autofocus: true` | Auto-focus on load |
///
/// ### Data Properties
///
/// | Property | Type | Syntax | Description |
/// |----------|------|--------|-------------|
/// | `initial_from` | String | `initial_from: "field_name"` | Map to initial data field |
/// | `bind` | bool | `bind: true` | Enable/disable auto two-way binding (default: true) |
///
/// ### Styling Properties
///
/// | Property | Type | Default | Description |
/// |----------|------|---------|-------------|
/// | `class` | String | `"reinhardt-input"` | Input element CSS class |
/// | `wrapper_class` | String | `"reinhardt-field"` | Field wrapper CSS class |
/// | `label_class` | String | `"reinhardt-label"` | Label CSS class |
/// | `error_class` | String | `"reinhardt-error"` | Error message CSS class |
///
/// ### Widget Override
///
/// ```text
/// widget: WidgetType
/// ```
///
/// Override the default widget for a field type.
///
/// ## Field Groups
///
/// Organize related fields into logical groups using `FieldGroup`.
/// Groups render as `<fieldset>` with an optional `<legend>`.
///
/// ### Field Group Syntax
///
/// ```text
/// fields: {
///     group_name: FieldGroup {
///         label: "Group Label",    // Optional: Renders as <legend>
///         class: "group-class",    // Optional: CSS class for fieldset
///         fields: {
///             field1: FieldType { ... },
///             field2: FieldType { ... },
///         },
///     },
/// }
/// ```
///
/// ### Field Group Example
///
/// ```ignore
/// let form = form! {
///     name: AddressForm,
///     action: "/api/address",
///
///     fields: {
///         name: CharField { required, label: "Full Name" },
///
///         address_group: FieldGroup {
///             label: "Address",
///             class: "address-section",
///             fields: {
///                 street: CharField { required, label: "Street" },
///                 city: CharField { required, label: "City" },
///                 zip: CharField { required, label: "ZIP Code", max_length: 10 },
///             },
///         },
///     },
/// };
/// ```
///
/// ### Field Group Generated HTML
///
/// ```html
/// <form class="reinhardt-form">
///   <div class="reinhardt-field">
///     <label>Full Name</label>
///     <input name="name" type="text">
///   </div>
///
///   <fieldset class="address-section">
///     <legend>Address</legend>
///     <div class="reinhardt-field">
///       <label>Street</label>
///       <input name="street" type="text">
///     </div>
///     <!-- ... more fields -->
///   </fieldset>
/// </form>
/// ```
///
/// ### Field Group Properties
///
/// | Property | Type | Required | Description |
/// |----------|------|----------|-------------|
/// | `label` | String | No | Legend text for the fieldset |
/// | `class` | String | No | CSS class for the fieldset element |
/// | `fields` | Block | Yes | Nested field definitions |
///
/// ### Accessing Group Fields
///
/// Group fields are flattened for accessor methods:
///
/// ```ignore
/// // Access fields directly (groups are transparent)
/// form.street();  // Returns &Signal<String>
/// form.city();
/// form.zip();
/// ```
///
/// # Validation
///
/// ## Server-Side Validators
///
/// Server-side validators run during form validation on the server.
///
/// ```ignore
/// form! {
///     name: MyForm,
///     action: "/api/submit",
///     fields: { username: CharField { required } },
///
///     validators: {
///         username: [
///             |v| !v.trim().is_empty() => "Username cannot be empty",
///             |v| v.len() >= 3 => "Username must be at least 3 characters",
///             |v| v.chars().all(|c| c.is_alphanumeric() || c == '_')
///                 => "Username can only contain letters, numbers, and underscores",
///         ],
///     },
/// }
/// ```
///
/// ## Client-Side Validators
///
/// Client-side validators run in the browser using JavaScript expressions.
///
/// ```ignore
/// form! {
///     name: MyForm,
///     action: "/api/submit",
///     fields: { email: EmailField { required } },
///
///     client_validators: {
///         email: [
///             "value.length > 0" => "Email is required",
///             "value.includes('@')" => "Invalid email format",
///         ],
///     },
/// }
/// ```
///
/// # Examples
///
/// ## Basic Form Example
///
/// ```ignore
/// use reinhardt_pages::form;
///
/// let login_form = form! {
///     name: LoginForm,
///     action: "/api/login",
///     method: Post,
///     class: "login-form",
///
///     fields: {
///         username: CharField {
///             required,
///             max_length: 150,
///             label: "Username",
///             placeholder: "Enter username",
///             class: "form-input",
///         },
///         password: CharField {
///             required,
///             min_length: 8,
///             widget: PasswordInput,
///             label: "Password",
///         },
///         remember_me: BooleanField {
///             label: "Remember me",
///         },
///     },
///
///     validators: {
///         username: [
///             |v| !v.trim().is_empty() => "Username is required",
///         ],
///     },
///
///     client_validators: {
///         password: [
///             "value.length >= 8" => "Password must be at least 8 characters",
///         ],
///     },
/// };
///
/// // Type-safe field access via Signal
/// let username_signal = login_form.username();
///
/// // Convert to View for rendering
/// let view = login_form.into_view();
/// ```
///
/// # Integration
///
/// ## With server_fn
///
/// ```ignore
/// #[server_fn]
/// async fn submit_login(request: LoginRequest) -> Result<(), ServerFnError> {
///     // Server-side handling
/// }
///
/// let form = form! {
///     name: LoginForm,
///     server_fn: submit_login,  // Uses server_fn instead of URL
///     fields: {
///         username: CharField { required },
///         password: CharField { required, widget: PasswordInput },
///     },
/// };
///
/// // On WASM, submit() calls the server_fn directly
/// form.submit().await?;
/// ```
///
/// ## Integration with page! Macro
///
/// The `form!` macro generates components that integrate seamlessly with `page!`.
///
/// ### Basic Integration
///
/// Use `into_view()` to embed a form in a page:
///
/// ```ignore
/// use reinhardt_pages::{form, page};
///
/// let login = form! {
///     name: LoginForm,
///     action: "/api/login",
///     fields: {
///         username: CharField { required },
///         password: CharField { required, widget: PasswordInput },
///     },
/// };
///
/// let page_view = page!(|| {
///     div {
///         class: "login-container",
///         h1 { "Sign In" }
///         login.into_view()  // Embeds form as View
///     }
/// });
/// ```
///
/// ### Signal Integration
///
/// Clone field Signals to use in page! for reactive updates:
///
/// ```ignore
/// let form = form! {
///     name: ProfileForm,
///     action: "/api/profile",
///     fields: {
///         name: CharField { required, max_length: 100 },
///         bio: TextField { max_length: 500 },
///     },
/// };
///
/// // Clone signals for use in page!
/// let name_signal = form.name().clone();
///
/// let page_view = page!(|name: Signal<String>| {
///     div {
///         form.into_view(),
///         // Reactive preview using Signal
///         div {
///             class: "preview",
///             h2 { "Preview" }
///             p { format!("Name: {}", name.get()) }
///         }
///     }
/// })(name_signal);
/// ```
///
/// ### Conditional Rendering
///
/// Use form Signals for conditional rendering in page!:
///
/// ```ignore
/// let form = form! {
///     name: RegistrationForm,
///     action: "/api/register",
///     fields: {
///         email: EmailField { required },
///         password: CharField { required, min_length: 8, widget: PasswordInput },
///     },
/// };
///
/// let email = form.email().clone();
/// let password = form.password().clone();
///
/// let page_view = page!(|email: Signal<String>, password: Signal<String>| {
///     div {
///         form.into_view(),
///
///         // Email validation feedback
///         if email.get().contains("@") {
///             span { class: "valid", "✓ Valid email" }
///         } else if !email.get().is_empty() {
///             span { class: "error", "✗ Invalid email format" }
///         }
///
///         // Password strength indicator
///         div {
///             class: "password-strength",
///             if password.get().len() >= 12 {
///                 span { class: "strong", "Strong password" }
///             } else if password.get().len() >= 8 {
///                 span { class: "medium", "Medium password" }
///             } else if !password.get().is_empty() {
///                 span { class: "weak", "Weak password" }
///             }
///         }
///     }
/// })(email, password);
/// ```
///
/// # Reference
///
/// ## Generated Code Structure
///
/// The `form!` macro generates the following code:
///
/// ```ignore
/// // Generated struct with Signal fields
/// struct FormName {
///     field_name: Signal<FieldType>,
///     // ... for each field
/// }
///
/// impl FormName {
///     /// Creates a new form instance with default values
///     pub fn new() -> Self { ... }
///
///     /// Field accessor returning Signal reference
///     pub fn field_name(&self) -> &Signal<FieldType> { ... }
///
///     /// Returns static metadata for SSR
///     pub fn metadata(&self) -> StaticFormMetadata { ... }
///
///     /// Validates all fields
///     pub fn validate(&self) -> Result<(), Vec<String>> { ... }
///
///     /// Submits the form (async on WASM, no-op on server)
///     pub fn submit(&self) -> impl Future<Output = Result<(), ServerFnError>> { ... }
///
///     /// Converts form to View for rendering
///     pub fn into_view(self) -> View { ... }
/// }
/// ```
///
/// ## SSR/CSR Considerations
///
/// - **`metadata()`**: Returns `StaticFormMetadata` for server-side rendering,
///   containing field names, types, widgets, and styling information.
///
/// - **`submit()`**: On WASM (client), performs actual HTTP request or server_fn call.
///   On server (non-WASM), returns `Ok(())` as a no-op.
///
/// - **Signal fields**: Work identically in both contexts, enabling shared code
///   between server and client.
///
/// - **`into_view()`**: Generates `ElementView` structures using the same API
///   as `page!` macro, ensuring seamless integration.
///
/// # Reactive Features
///
/// ## UI State Management
///
/// The `state` block provides automatic management of loading, error, and success states
/// during form submission.
///
/// ```text
/// state: { loading, error, success }
/// ```
///
/// | State | Signal Type | Description |
/// |-------|-------------|-------------|
/// | `loading` | `Signal<bool>` | True during submission |
/// | `error` | `Signal<Option<String>>` | Error message on failure |
/// | `success` | `Signal<bool>` | True after successful submission |
///
/// ### Example
///
/// ```ignore
/// let form = form! {
///     name: LoginForm,
///     server_fn: login_user,
///
///     state: { loading, error, success },
///
///     fields: {
///         username: CharField { required },
///         password: CharField { required, widget: PasswordInput },
///     },
/// };
///
/// // Access state signals
/// let is_loading = form.loading().get();
/// let error_msg = form.error().get();
/// let succeeded = form.success().get();
/// ```
///
/// ## Two-way Binding
///
/// Control automatic @input handler generation with the `bind` property.
/// Default is `true` (automatic binding enabled).
///
/// ```ignore
/// fields: {
///     // Automatic binding (default)
///     username: CharField { required },
///
///     // Explicit binding control
///     password: CharField { required, bind: true },
///
///     // Manual binding (no auto-generated handler)
///     custom: CharField { bind: false },
/// }
/// ```
///
/// ## Computed Values (derived block)
///
/// The `derived` block creates computed values from form fields. Each derived item
/// becomes a method on the form struct that returns a computed value based on
/// field signals. The values are computed each time they are accessed.
///
/// | Syntax | Description |
/// |--------|-------------|
/// | `name: \|form\| expr` | Computes value from form fields |
///
/// ### Character Counter Example
///
/// ```ignore
/// let form = form! {
///     name: TweetForm,
///     server_fn: create_tweet,
///
///     derived: {
///         char_count: |form| form.content().get().len(),
///         is_over_limit: |form| form.char_count() > 280,
///         progress_percent: |form| (form.char_count() as f32 / 280.0 * 100.0).min(100.0),
///     },
///
///     fields: {
///         content: CharField { required, bind: true, max_length: 280 },
///     },
/// };
///
/// // Access computed values
/// let count = form.char_count();      // Returns usize
/// let over = form.is_over_limit();    // Returns bool
/// let pct = form.progress_percent();  // Returns f32
/// ```
///
/// ### Shopping Cart Example
///
/// ```ignore
/// let form = form! {
///     name: OrderForm,
///     server_fn: place_order,
///
///     derived: {
///         subtotal: |form| {
///             let price = form.price().get().parse::<f64>().unwrap_or(0.0);
///             let qty = form.quantity().get().parse::<i32>().unwrap_or(0);
///             price * qty as f64
///         },
///         tax: |form| form.subtotal() * 0.1,
///         total: |form| form.subtotal() + form.tax(),
///     },
///
///     fields: {
///         price: DecimalField { required, bind: true },
///         quantity: IntegerField { required, bind: true },
///     },
/// };
/// ```
///
/// ### Usage Notes
///
/// - Derived values are computed on each access (not cached)
/// - Derived items can reference other derived items in the same block
/// - Use with `watch` blocks for reactive UI updates
///
/// ## Watch Integration
///
/// The `watch` block creates reactive computed views that re-render when
/// form state changes.
///
/// ```ignore
/// let form = form! {
///     name: LoginForm,
///     server_fn: login,
///
///     state: { error },
///
///     watch: {
///         error_display: |form| {
///             if let Some(err) = form.error().get() {
///                 div { class: "error-message", err }
///             }
///         },
///         username_preview: |form| {
///             let username = form.username().get();
///             if !username.is_empty() {
///                 p { format!("Hello, {}!", username) }
///             }
///         },
///     },
///
///     fields: {
///         username: CharField { required },
///         password: CharField { required, widget: PasswordInput },
///     },
/// };
///
/// // Use watch methods in page!
/// page!(|| {
///     div {
///         form.error_display()
///         form.into_view()
///         form.username_preview()
///     }
/// })
/// ```
///
/// ### Watch with Match Expressions
///
/// Watch closures support full Rust expressions, including `match` for
/// multi-condition rendering. This is useful for displaying different UI
/// states based on computed values.
///
/// ```ignore
/// let form = form! {
///     name: TweetForm,
///     server_fn: create_tweet,
///
///     derived: {
///         char_count: |form| form.content().get().len(),
///     },
///
///     watch: {
///         // Multi-color character counter using match
///         counter: |form| {
///             let count = form.char_count();
///             match count {
///                 c if c > 280 => div { class: "text-red-500 font-bold", { format!("{}/280", c) } },
///                 c if c > 250 => div { class: "text-yellow-500", { format!("{}/280", c) } },
///                 c if c > 0 => div { class: "text-blue-500", { format!("{}/280", c) } },
///                 _ => div { class: "text-gray-400", "0/280" },
///             }
///         },
///     },
///
///     fields: {
///         content: CharField { required, bind: true },
///     },
/// };
/// ```
///
/// ### Watch with If-Else Chains
///
/// For simpler conditions, if-else chains work well within watch closures:
///
/// ```ignore
/// watch: {
///     status_indicator: |form| {
///         let count = form.char_count();
///         if count > 280 {
///             span { class: "badge badge-error", "Over limit!" }
///         } else if count > 250 {
///             span { class: "badge badge-warning", "Almost full" }
///         } else {
///             span { class: "badge badge-success", "OK" }
///         }
///     },
/// }
/// ```
///
/// # Customization
///
/// ## Custom Wrapper Elements
///
/// Use the `wrapper` property to wrap an input field with custom HTML structure.
///
/// ```ignore
/// fields: {
///     email: EmailField {
///         required,
///         label: "Email",
///         wrapper: div {
///             class: "relative flex items-center",
///         },
///     },
/// }
/// ```
///
/// ## SVG Icons
///
/// Add SVG icons to form fields using the `icon` and `icon_position` properties.
///
/// | Position | Description |
/// |----------|-------------|
/// | `left` | Icon on the left side of input |
/// | `right` | Icon on the right side of input |
/// | `label` | Icon within the label |
///
/// ### Icon Example
///
/// ```ignore
/// fields: {
///     username: CharField {
///         required,
///         label: "Username",
///         icon: svg {
///             class: "w-5 h-5 text-gray-400",
///             viewBox: "0 0 24 24",
///             fill: "none",
///             stroke: "currentColor",
///             path {
///                 d: "M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z",
///                 stroke_linecap: "round",
///                 stroke_linejoin: "round",
///             }
///         },
///         icon_position: "left",
///     },
/// }
/// ```
///
/// ## Custom Attributes
///
/// Add ARIA and data attributes for accessibility and custom data.
///
/// ```ignore
/// fields: {
///     search: CharField {
///         label: "Search",
///         attrs: {
///             aria_label: "Search input",
///             aria_required: "true",
///             data_testid: "search-input",
///         },
///     },
/// }
/// ```
///
/// ## Slots
///
/// Insert custom content before, after, or between form fields using slots.
///
/// | Slot | Description |
/// |------|-------------|
/// | `before_fields` | Content rendered before all fields |
/// | `after_fields` | Content rendered after all fields |
///
/// ### Slots Example
///
/// ```ignore
/// let form = form! {
///     name: ContactForm,
///     server_fn: submit_contact,
///
///     slots: {
///         before_fields: || {
///             div {
///                 class: "form-header",
///                 h2 { "Contact Us" }
///                 p { "Fill out the form below to get in touch." }
///             }
///         },
///         after_fields: || {
///             div {
///                 class: "form-footer",
///                 p {
///                     class: "privacy-notice",
///                     "Your information will be kept confidential."
///                 }
///                 button { type: "submit", "Send Message" }
///             }
///         },
///     },
///
///     fields: {
///         name: CharField { required, label: "Your Name" },
///         email: EmailField { required, label: "Email Address" },
///         message: TextField { required, label: "Message" },
///     },
/// };
/// ```
///
/// ### Slots Generated Structure
///
/// ```html
/// <form class="reinhardt-form">
///   <!-- before_fields slot -->
///   <div class="form-header">
///     <h2>Contact Us</h2>
///     <p>Fill out the form below to get in touch.</p>
///   </div>
///
///   <!-- Form fields -->
///   <div class="reinhardt-field">...</div>
///   <div class="reinhardt-field">...</div>
///   <div class="reinhardt-field">...</div>
///
///   <!-- after_fields slot -->
///   <div class="form-footer">
///     <p class="privacy-notice">Your information will be kept confidential.</p>
///     <button type="submit">Send Message</button>
///   </div>
/// </form>
/// ```
///
/// # Advanced Features
///
/// ## Callbacks
///
/// Define callbacks to handle form submission lifecycle events.
///
/// | Callback | Signature | When Called |
/// |----------|-----------|-------------|
/// | `on_submit` | `\|&Form\|` | Before submission starts |
/// | `on_loading` | `\|bool\|` | When loading state changes |
/// | `on_success` | `\|Result\|` | After successful submission |
/// | `on_error` | `\|ServerFnError\|` | After submission error |
///
/// ### Example
///
/// ```ignore
/// let form = form! {
///     name: ContactForm,
///     server_fn: submit_contact,
///
///     on_submit: |form| {
///         console::log_1(&"Submitting...".into());
///     },
///     on_success: |result| {
///         console::log_1(&"Success!".into());
///     },
///     on_error: |e| {
///         console::error_1(&format!("Error: {:?}", e).into());
///     },
///     on_loading: |is_loading| {
///         console::log_1(&format!("Loading: {}", is_loading).into());
///     },
///
///     fields: {
///         message: TextField { required },
///     },
/// };
/// ```
///
/// ## Redirect on Success
///
/// Automatically redirect to a URL after successful form submission.
///
/// | Syntax | Example |
/// |--------|---------|
/// | Static path | `redirect_on_success: "/dashboard"` |
/// | Full URL | `redirect_on_success: "https://example.com/success"` |
///
/// ### Example
///
/// ```ignore
/// let form = form! {
///     name: LoginForm,
///     server_fn: login,
///     redirect_on_success: "/dashboard",
///
///     fields: {
///         username: CharField { required },
///         password: CharField { required, widget: PasswordInput },
///     },
/// };
/// ```
///
/// Note: Redirect is only executed on WASM (client-side) after `on_success` callback.
///
/// ## Initial Value Loading
///
/// Load initial form values from a server function using `initial_loader`.
/// Map loaded data to specific fields using `initial_from`.
///
/// | Attribute | Description |
/// |-----------|-------------|
/// | `initial_loader` | Server function that returns initial data |
/// | `initial_from` | Field property to map data field to form field |
///
/// ### Example
///
/// ```ignore
/// #[server_fn]
/// async fn get_profile() -> Result<ProfileData, ServerFnError> {
///     // Fetch user profile from database
///     Ok(ProfileData {
///         username: "john_doe".to_string(),
///         email: "john@example.com".to_string(),
///         bio: "Hello!".to_string(),
///     })
/// }
///
/// let form = form! {
///     name: ProfileEditForm,
///     server_fn: update_profile,
///     initial_loader: get_profile,  // Server function for initial data
///
///     fields: {
///         username: CharField {
///             required,
///             initial_from: "username",  // Maps to ProfileData.username
///         },
///         email: EmailField {
///             required,
///             initial_from: "email",     // Maps to ProfileData.email
///         },
///         bio: TextField {
///             initial_from: "bio",       // Maps to ProfileData.bio
///         },
///     },
/// };
/// ```
///
/// ### Generated Behavior
///
/// - On form creation, calls `initial_loader` asynchronously
/// - Populates each field with the corresponding value from the returned data
/// - Fields without `initial_from` use their default values
/// - Loading state (`loading` in state block) is set during data fetch
///
/// ## Dynamic Choice Loading
///
/// Load choice options dynamically from a server function using `choices_loader`.
/// Map loaded data to radio buttons or selects using field properties.
///
/// | Attribute | Level | Description |
/// |-----------|-------|-------------|
/// | `choices_loader` | Form | Server function that returns choice data |
/// | `choices_from` | Field | Data field containing choice array |
/// | `choice_value` | Field | Property path for option value (default: "value") |
/// | `choice_label` | Field | Property path for option label (default: "label") |
///
/// ### Voting Form Example
///
/// ```ignore
/// #[server_fn]
/// async fn get_poll_data(poll_id: i64) -> Result<PollData, ServerFnError> {
///     Ok(PollData {
///         question: "What is your favorite color?".to_string(),
///         choices: vec![
///             Choice { id: 1, choice_text: "Red".to_string() },
///             Choice { id: 2, choice_text: "Blue".to_string() },
///             Choice { id: 3, choice_text: "Green".to_string() },
///         ],
///     })
/// }
///
/// let form = form! {
///     name: VotingForm,
///     server_fn: submit_vote,
///     choices_loader: get_poll_data,  // Server function for choice data
///
///     fields: {
///         choice: ChoiceField {
///             required,
///             widget: RadioSelect,
///             choices_from: "choices",      // Maps to PollData.choices
///             choice_value: "id",           // Each choice's value: Choice.id
///             choice_label: "choice_text",  // Each choice's label: Choice.choice_text
///         },
///     },
/// };
/// ```
///
/// ### Filter Form Example
///
/// ```ignore
/// let form = form! {
///     name: FilterForm,
///     server_fn: apply_filter,
///     choices_loader: get_filter_options,
///
///     fields: {
///         category: ChoiceField {
///             label: "Category",
///             choices_from: "categories",
///             choice_value: "id",
///             choice_label: "name",
///         },
///         status: ChoiceField {
///             label: "Status",
///             widget: Select,
///             choices_from: "statuses",
///             choice_value: "code",
///             choice_label: "description",
///         },
///     },
/// };
/// ```
///
/// ### Generated Behavior
///
/// - Creates `{field}_choices` Signal for each dynamic choice field
/// - Generates `load_choices()` async method to fetch and populate choices
/// - Choice options are stored as `Vec<(String, String)>` (value, label) tuples
/// - Radio buttons or select options are rendered dynamically from the signal
///
/// ### External Signal Population (Without choices_loader)
///
/// When `choices_loader` cannot pass parameters (e.g., `question_id`), you can
/// populate the `{field}_choices` Signal externally. Define the field with
/// `choices_from` but omit `choices_loader`, then set the Signal manually.
///
/// ```ignore
/// // Define form without choices_loader
/// let voting_form = form! {
///     name: VotingForm,
///     server_fn: submit_vote,
///     method: Post,
///
///     fields: {
///         question_id: HiddenField { initial: question_id_str },
///         choice_id: ChoiceField {
///             widget: RadioSelect,
///             required,
///             choices_from: "choices",      // Generates choice_id_choices Signal
///             choice_value: "id",
///             choice_label: "choice_text",
///         },
///     },
/// };
///
/// // Fetch data externally and populate the Signal
/// #[cfg(target_arch = "wasm32")]
/// {
///     let form_clone = voting_form.clone();
///     spawn_local(async move {
///         match get_question_detail(question_id).await {
///             Ok((question, choices)) => {
///                 // Convert to (value, label) tuples
///                 let choice_options: Vec<(String, String)> = choices
///                     .iter()
///                     .map(|c| (c.id.to_string(), c.choice_text.clone()))
///                     .collect();
///
///                 // Set the Signal externally
///                 form_clone.choice_id_choices().set(choice_options);
///             }
///             Err(e) => { /* handle error */ }
///         }
///     });
/// }
/// ```
///
/// This pattern is useful when:
/// - The data loader requires parameters (e.g., `question_id`, `user_id`)
/// - You need to load choices from multiple sources
/// - You want more control over the loading logic
///
/// ## Server Function Parameter Expansion
///
/// When using `server_fn`, the form submits field values as **individual arguments**,
/// not as a struct. Design your server function accordingly.
///
/// ### Server Function Signature
///
/// ```ignore
/// // Form definition
/// let form = form! {
///     name: VotingForm,
///     server_fn: submit_vote,
///
///     fields: {
///         question_id: HiddenField { initial: "1" },
///         choice_id: ChoiceField { required },
///     },
/// };
///
/// // Server function must accept individual String parameters
/// #[server_fn(use_inject = true)]
/// pub async fn submit_vote(
///     question_id: String,   // From HiddenField
///     choice_id: String,     // From ChoiceField
///     #[inject] db: DatabaseConnection,
/// ) -> Result<ChoiceInfo, ServerFnError> {
///     // Parse String values to required types
///     let question_id: i64 = question_id.parse()
///         .map_err(|_| ServerFnError::application("Invalid question_id"))?;
///     let choice_id: i64 = choice_id.parse()
///         .map_err(|_| ServerFnError::application("Invalid choice_id"))?;
///
///     // Process the vote...
///     Ok(result)
/// }
/// ```
///
/// ### Wrapper Function Pattern
///
/// If your existing server function accepts a struct, create a wrapper:
///
/// ```ignore
/// // Original function expecting struct
/// #[server_fn]
/// pub async fn vote(request: VoteRequest) -> Result<ChoiceInfo, ServerFnError> {
///     vote_internal(request).await
/// }
///
/// // Wrapper for form! compatibility
/// #[server_fn(use_inject = true)]
/// pub async fn submit_vote(
///     question_id: String,
///     choice_id: String,
///     #[inject] db: DatabaseConnection,
/// ) -> Result<ChoiceInfo, ServerFnError> {
///     let request = VoteRequest {
///         question_id: question_id.parse().map_err(|_| ServerFnError::application("Invalid question_id"))?,
///         choice_id: choice_id.parse().map_err(|_| ServerFnError::application("Invalid choice_id"))?,
///     };
///     vote_internal(request, db).await
/// }
///
/// // Shared implementation
/// async fn vote_internal(request: VoteRequest, db: DatabaseConnection) -> Result<ChoiceInfo, ServerFnError> {
///     // Common logic
/// }
/// ```
///
/// ### Real-World Example
///
/// See `examples/examples-tutorial-basis` for a complete voting form implementation:
/// - Client: `src/client/components/polls.rs` - `polls_detail` function
/// - Server: `src/server_fn/polls.rs` - `submit_vote` wrapper function
///
/// ## CSRF Protection
///
/// Forms with non-GET methods (POST, PUT, PATCH, DELETE) automatically include
/// CSRF protection. A hidden input field with the CSRF token is injected as the
/// first child element of the form.
///
/// ### Automatic Injection
///
/// ```ignore
/// use reinhardt_pages::form;
/// # fn main() {
/// // This POST form automatically includes CSRF token
/// let contact_form = form! {
///     name: ContactForm,
///     action: "/api/contact",
///     method: Post,
///
///     fields: {
///         message: CharField { required },
///     },
/// };
/// # }
/// ```
///
/// The generated form will include:
/// ```html
/// <form action="/api/contact" method="post">
///     <input type="hidden" name="csrfmiddlewaretoken" value="[token]">
///     <!-- field elements -->
/// </form>
/// ```
///
/// ### Token Retrieval
///
/// The CSRF token is retrieved at render time from (in order):
/// 1. Cookie: `csrftoken`
/// 2. Meta tag: `<meta name="csrf-token">`
/// 3. Hidden input: `<input name="csrfmiddlewaretoken">`
///
/// ### GET Forms (No CSRF)
///
/// GET forms do not include CSRF tokens since they are safe methods:
///
/// ```ignore
/// use reinhardt_pages::form;
/// # fn main() {
/// // This GET form does NOT include CSRF token
/// let search_form = form! {
///     name: SearchForm,
///     action: "/search",
///     method: Get,
///
///     fields: {
///         query: CharField { required },
///     },
/// };
/// # }
/// ```
#[proc_macro]
pub fn form(input: TokenStream) -> TokenStream {
	form::form_impl(input)
}

// Note: For dependency injection parameters, use the tool attribute #[reinhardt::inject]
// instead of a bare #[inject]. This is because proc_macro_attribute doesn't support
// helper attributes (unlike proc_macro_derive). Tool attributes provide namespace
// clarity and prevent "cannot find attribute in scope" compiler errors.
// The #[server_fn] macro detects and processes #[reinhardt::inject] during expansion.
