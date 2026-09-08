//! Reinhardt Pages - WASM-based Frontend Framework
//!
//! A Django-inspired frontend framework for Reinhardt that preserves the benefits of
//! Django templates while leveraging WebAssembly for modern interactivity.
//!
//! Use [`reactive::batch`] to group related reactive writes into one update
//! cycle. Async [`Action`] handles can be connected to [`OptimisticState`] with
//! [`Action::with_optimistic`] so failed mutations automatically roll back
//! optimistic UI state.
//!
//! ## Features
//!
//! - **Fine-grained Reactivity**: Leptos/Solid.js-style Signal system with React-aligned hooks
//! - **Hybrid Rendering**: SSR + Client-side Hydration for optimal performance and SEO
//! - **Django-like API**: Familiar patterns for Reinhardt developers
//! - **Boundaries**: Suspense and error boundaries for async UI states
//!
//! ## React-aligned hook signatures (v0.2, Refs #4195)
//!
//! `use_effect`, `use_layout_effect`, `use_memo`, `use_callback`, and
//! `use_callback_with` take an explicit dependency tuple as the second
//! argument:
//!
//! ```ignore
//! use reinhardt_pages::prelude::*;
//!
//! let count = Signal::new(0_i32);
//! let count_for_effect = count.clone();
//! let _eff = use_effect(
//!     move || {
//!         println!("count = {}", count_for_effect.get());
//!         None::<fn()>
//!     },
//!     (count.clone(),),
//! );
//! ```
//!
//! Closures run with no active reactive Observer ("Option A"), so
//! `Signal::get` inside does NOT auto-subscribe — subscriptions derive
//! exclusively from the deps tuple. Pass `()` for mount-only effects.
//!
//! For a concept-by-concept mapping from React to Reinhardt Pages, see
//! `docs/react_to_reinhardt.md` in this crate.
//! - **Low-level Only**: Built on wasm-bindgen, web-sys, and js-sys (no high-level framework dependencies)
//! - **Security First**: Built-in CSRF protection, XSS prevention, and session management
//!
//! ## Architecture
//!
//! This framework consists of several key modules:
//!
//! - [`reactive`]: Fine-grained reactivity system (Signal, Effect, Memo)
//! - [`dom`]: DOM abstraction layer
//! - [`builder`]: HTML element builder API
//! - [`component`](mod@component): Component system with IntoPage trait, Head management
//! - [`form`](mod@form): Django Form integration
//! - [`form_state`]: Typed `use_form` runtime state
//! - [`csrf`]: CSRF protection
//! - [`auth`]: Authentication integration
//! - [`api`]: API client with Django QuerySet-like interface
//! - [`server_fn`]: Server Functions (RPC)
//! - [`ssr`]: Server-side rendering with Head support
//! - [`hydration`]: Client-side hydration
//! - [`router`]: Client-side routing (reinhardt-urls compatible)
//! - [`portal`]: Explicit portal mounting into existing DOM targets
//! - [`static_resolver`]: Static file URL resolution (collectstatic support)
//!
//! ## Forms
//!
//! `form!` owns static form definition: field names, widgets, labels,
//! rendering metadata, and server function binding. [`use_form`] owns typed
//! runtime behavior: value signals, dirty/touched state, validation errors,
//! loading/success state, reset, and submit orchestration.
//!
//! Create a `form!` generated form and attach runtime behavior with
//! [`use_form`]:
//!
//! ```ignore
//! use reinhardt_pages::{form, use_form};
//!
//! let login_form = form! {
//!     name: LoginForm,
//!     action: "/login",
//!     fields: {
//!         username: CharField { initial: String::new() }
//!         password: CharField { initial: String::new() }
//!     }
//! };
//!
//! let runtime = use_form(&login_form).build();
//! runtime.set_value(login_form.username_field(), "ada".to_string());
//! ```
//!
//! `FileField` and `ImageField` participate in this runtime contract as
//! `Option<web_sys::File>` values. File values are browser-owned and are
//! tracked for dirty/touched state without treating the file payload as a
//! serializable scalar.
//!
//! Generated native HTML includes current field values, checkbox state, textarea
//! content, and selected options. Bound controls synchronize signal updates and
//! runtime reset through their existing DOM nodes. `bind: false` renders only
//! a value snapshot without ongoing synchronization. File inputs remain clear-only for
//! programmatic updates. Optional temporal/UUID/IP values render empty for
//! `None`; datetime-local uses `T`, and JSON values use compact serialization.
//! Hydration preserves browser edits and selected files, while explicit runtime
//! setters and field resets take precedence for their own field or collection path.
//! Unchanged multiple selections retain source order. Leading textarea line feeds
//! survive HTML parsing, including selective hydration. Native reset clears touched
//! state and errors without emitting edit or validation events, and preserves
//! subscriptions to later custom widget errors and field edits.
//! Static choice values are evaluated once per option. Textarea hydration compares
//! normalized HTML line endings so parsing alone does not create an edit.
//! Unbound textarea snapshots preserve whitespace and validate parsed default text.
//! Reconciled pristine defaults remain preferred only for their updated fields and paths.
//!
//! Stable native widget coverage includes the following `form!` DSL items:
//!
//! | DSL item | HTML output | Value state |
//! |---|---|---|
//! | `MonthInput` | `<input type="month">` | string field |
//! | `WeekInput` | `<input type="week">` | string field |
//! | `RadioInput` | one `<input type="radio">` | `ChoiceField<String>` |
//! | `RadioSelect` | a group of `<input type="radio">` controls | choice field |
//! | `ResetButton` | `<button type="reset">` | none |
//! | `Button` | `<button type="button">` | none |
//! | `ImageInput` | `<input type="image">` | none |
//! | `Datalist` | `<datalist>` | option source only |
//! | `OptGroup` | `<optgroup>` | choice grouping only |
//! | `Output` | `<output>` | none |
//! | `Meter` | `<meter>` | none |
//! | `Progress` | `<progress>` | none |
//!
//! `RadioInput` renders one string-valued choice, with the field's name and ID.
//! Its fixed option value defaults to `"on"`; exactly one static option such as
//! `choices: [("yes", "Yes")]` supplies another value and an optional label.
//! An explicit field label takes precedence, and an option marked `disabled`
//! disables the input. Multiple, empty, grouped, or dynamic options are rejected;
//! use `RadioSelect` with `choices_from` for a group and `CheckboxInput` for booleans.
//!
//! The radio is checked when its field value equals its option value. Selecting
//! it updates the field, and programmatic values and runtime resets update the
//! checked state. Forms with a bound scalar radio restore all bound scalar and
//! collection fields on native reset, including defaults loaded after mounting.
//! Collection defaults follow item keys; rows without persisted loader defaults
//! use their values at page construction, or field defaults for new rows. Reset
//! preserves collection keys, order, and unbound values, and clears file inputs.
//! DOM and signal values are restored before runtime touched state and errors are
//! cleared without change or validation events. Later reset listeners or immediate
//! source writes after `reset()` supersede the pending reset, even for equal values.
//! Custom widget errors remain
//! reactive after reset. Scalar and collection
//! radios preserve `autocomplete` and retain focus within their reactive subtree.
//! For scalar fields, `bind: false` snapshots the current value without binding.
//!
//! Typed native attributes are accepted for the controls that support them:
//!
//! | Attribute | Compatible controls |
//! |---|---|
//! | `min` / `max` / `step` | number, range, date, time, datetime-local, month, week |
//! | `size` | text-like inputs |
//! | `accept` / `capture` | file-like inputs |
//! | `multiple` | file-like inputs and multi-select |
//! | `list` | datalist-compatible text-like inputs |
//!
//! `FieldGroup` renders as semantic `<fieldset>` output. When `label` is
//! present, the label is rendered as a `<legend>` inside the fieldset.
//!
//! Dynamic `RadioSelect` fields also use `<fieldset>` / `<legend>` to name the
//! radio group. Each option keeps its own label and indexed input ID. Field,
//! label, and input CSS classes are preserved. Custom wrappers keep their tag and
//! attributes, with `aria-labelledby` naming the group from a caption `<span>`
//! instead of a legend. The default role is `group`; an explicit wrapper role is
//! preserved.
//!
//! `CustomWidget` is experimental and must opt in explicitly:
//!
//! ```rust,ignore
//! date_range: CharField {
//!     widget: CustomWidget(crate::widgets::DateRangePicker) {
//!         experimental,
//!         adapter: crate::widgets::DateRangeAdapter,
//!     },
//! }
//! ```
//!
//! The adapter API may change in a minor release with a documented migration
//! path.
//!
//! Use `ambient_arguments` for non-field values supplied from surrounding
//! context. The old `strip_arguments` DSL name remains as a deprecated alias.
//! CSRF should be supplied by `#[server_fn]` client stubs through the
//! `X-CSRFToken` header rather than as a server function business argument.
//!
//! ## Macros
//!
//! - [`page!`]: JSX-like macro for defining view components
//! - [`head!`]: JSX-like macro for defining HTML head sections
//! - [`form!`]: Type-safe form component macro
//! - [`client_page`]: Client page function macro with native route-table stubs
//! - [`wasm_server_api`]: WASM/server API parity macro
//!
//! See `docs/wasm_server_api.md` for the target-specific API parity contract.
//!
//! ## Example
//!
//! ### Basic Component
//!
//! ```no_run
//! use reinhardt_pages::{Signal, Page, page};
//!
//! fn counter() -> Page {
//!     let count = Signal::new(0);
//!
//!     page!(|count: Signal<i32>| {
//!         div {
//!             p { { format!("Count: {}", count.get()) } }
//!         }
//!     })(count)
//! }
//! ```
//!
//! ### With Head Section
//!
//! ```ignore
//! use reinhardt_pages::{head, page, Page, resolve_static};
//!
//! fn home_page() -> Page {
//!     let page_head = head!(|| {
//!         title { "Home - My App" }
//!         meta { name: "description", content: "Welcome to my app" }
//!         link { rel: "stylesheet", href: resolve_static("css/main.css") }
//!     });
//!
//!     page! {
//!         #head: page_head,
//!         || {
//!             div { class: "container",
//!                 h1 { "Welcome Home" }
//!             }
//!         }
//!     }()
//! }
//! ```
//!
//! ### WebSocket Integration
//!
//! The `use_websocket` hook provides reactive WebSocket connections:
//!
//! ```ignore
//! use reinhardt_pages::reactive::hooks::{use_websocket, use_effect, UseWebSocketOptions};
//! use reinhardt_pages::reactive::hooks::{ConnectionState, WebSocketMessage};
//!
//! fn chat_component() -> Page {
//!     // Establish WebSocket connection
//!     let ws = use_websocket("ws://localhost:8000/ws/chat", UseWebSocketOptions::default());
//!
//!     // Monitor connection state reactively
//!     use_effect(
//!         {
//!             let ws = ws.clone();
//!             move || {
//!                 match ws.connection_state().get() {
//!                     ConnectionState::Open => log!("Connected to chat"),
//!                     ConnectionState::Closed => log!("Disconnected from chat"),
//!                     ConnectionState::Error(e) => log!("Connection error: {}", e),
//!                     _ => {}
//!                 }
//!                 None::<fn()>
//!             }
//!         },
//!         (),
//!     );
//!
//!     // Handle incoming messages
//!     use_effect(
//!         {
//!             let ws = ws.clone();
//!             move || {
//!                 if let Some(WebSocketMessage::Text(text)) = ws.latest_message().get() {
//!                     log!("Received: {}", text);
//!                 }
//!                 None::<fn()>
//!             }
//!         },
//!         (),
//!     );
//!
//!     page!(|| {
//!         div {
//!             button {
//!                 @click: move |_| {
//!                     ws.send_text("Hello, server!".to_string()).ok();
//!                 },
//!                 "Send Message"
//!             }
//!         }
//!     })()
//! }
//! ```
//!
//! **Note**: WebSocket functionality is WASM-only. On the server side (SSR),
//! `use_websocket` returns a no-op handle with connection state always set to `Closed`.

#![warn(missing_docs)]

// Re-export AST definitions from reinhardt-pages-ast
// This is deprecated but kept for backward compatibility
#[allow(deprecated)] // Intentional: maintaining backward compatibility with existing code
pub use reinhardt_pages_ast as ast;

// Core modules
pub mod builder;
pub mod callback;
pub mod dom;
pub mod logging;
pub mod reactive;

// Platform abstraction (unified types and task spawning for WASM and native)
pub mod platform;
pub mod portal;

/// Backward-compatibility re-export of task-spawning utilities.
///
/// Task spawning moved into `platform` (#4362). This deprecated module
/// keeps existing `reinhardt_pages::spawn::*` imports working for the
/// remainder of the 0.x line.
pub mod spawn {
	/// Deprecated: use `spawn_task` from `reinhardt_pages::prelude` instead.
	#[deprecated(
		note = "moved to reinhardt_pages::platform; use spawn_task from reinhardt_pages::prelude instead"
	)]
	pub use crate::platform::spawn_task;

	/// Deprecated: use `defer_yield` from `reinhardt_pages::prelude` instead.
	#[deprecated(
		note = "moved to reinhardt_pages::platform; use defer_yield from reinhardt_pages::prelude instead"
	)]
	pub use crate::platform::defer_yield;
}

// Unified prelude for simplified imports
pub mod prelude;

// Component system
pub mod component;

// Form and security
pub mod auth;
pub mod csrf;
#[doc(hidden)]
mod fetch;
// Static form metadata types for form! macro (WASM-compatible)
pub mod form_generated;
// Typed form runtime state (WASM-compatible)
pub mod form_state;
// FormComponent requires reinhardt-forms which is not WASM-compatible yet.
// Client-side forms use PageElement.
#[cfg(native)]
pub mod form;

// API and communication
pub mod api;
pub mod server_fn;

// Server-side rendering
pub mod ssr;

// Client-side hydration
pub mod hydration;

// Client-side routing
pub mod router;

// WASM application launcher
pub mod app;

// Integration modules (runtime support for special macros)
pub mod integ;

// Testing utilities (available on both WASM and server)
// Layer 1: server_fn unit tests (server-side only)
// Layer 2: Component + server_fn mock tests (WASM)
// Layer 3: E2E tests (both platforms)
#[cfg(any(test, feature = "testing"))]
pub mod testing;

// Static file URL resolver
pub mod static_resolver;

// Hot Module Replacement (server-side only, feature-gated)
#[cfg(all(native, feature = "hmr"))]
pub mod hmr;

// Table utilities (django-tables2 equivalent)
pub mod tables;

// Re-export commonly used types
pub use api::{ApiModel, ApiQuerySet, Filter, FilterOp};
pub use auth::{AuthData, AuthError, AuthState, auth_state};
pub use builder::{
	attributes::{AriaAttributes, BooleanAttributes},
	html::{
		a, button, div, form, h1, h2, h3, img, input, li, ol, option, p, select, span, textarea, ul,
	},
};
pub use callback::{Callback, IntoEventHandler, event_handler, into_event_handler};
#[cfg(native)]
pub use component::DummyEvent;
#[cfg(wasm)]
pub use component::cleanup_reactive_nodes;
pub use component::{
	ActivityBoundary, ActivityMode, BoundaryError, Component, ErrorBoundary, ErrorTracker, Head,
	IntoPage, LinkTag, MetaTag, Page, PageElement, PageExt, Props, ResourceTracker, ScriptTag,
	StyleTag, SuspenseBoundary, ViewTransitionBoundary, ViewTransitionHandle, ViewTransitionStatus,
	start_view_transition,
};
pub use csrf::{CsrfManager, get_csrf_token};
pub use dom::{CustomEventOptions, Document, Element, EventHandle, EventType, document};
#[cfg(native)]
pub use form::{FormBinding, FormComponent};
// Static form metadata types (always available, used by form! macro)
pub use form_generated::{StaticFieldMetadata, StaticFormMetadata};
pub use form_state::{
	CollectionItem, CollectionItemKey, CollectionState, CustomWidgetContext, CustomWidgetRawValue,
	FieldError, FieldPathState, FieldState, FocusError, FormCollectionRuntimeSource, FormEvent,
	FormRuntimeSource, FormState, FormSubscription, FormValidationError, FormWidgetAdapter,
	FormWidgetError, FormWidgetValueKind, NoDeps, ResetOnDeps, RevalidateOn, UseFormBuilder,
	UseFormReturn, UseFormSubmitOutcome, use_form,
};
pub use hydration::{HydrationContext, HydrationError, hydrate};
pub use portal::{Portal, PortalError, PortalHandle, PortalTarget, mount_portal};
pub use reactive::{Effect, Memo, Resource, ResourceState, Signal, use_resource};
// Re-export Context system
pub use reactive::{
	Context, ContextGuard, create_context, get_context, provide_context, remove_context,
};
// Re-export Hooks API
pub use app::{ClientLauncher, LaunchCtx, PathCtx, PathParams};
pub use reactive::{Action, ActionPhase, use_action};
pub use reactive::{
	Dispatch, OptimisticState, Ref, SetState, SharedSetState, SharedSignal, TransitionState,
	use_callback, use_context, use_debug_value, use_deferred_value, use_effect, use_id,
	use_layout_effect, use_memo, use_optimistic, use_reducer, use_ref, use_shared_state, use_state,
	use_sync_external_store, use_transition,
};
#[cfg(native)]
pub use reinhardt_forms::{
	Widget,
	wasm_compat::{FieldMetadata, FormMetadata},
};
pub use router::Link;
// Imperative SPA navigation API (Issue #4610). `navigate` is the free
// function; `use_router` returns a `RouterHandle` for use inside hooks /
// components. `NavigateError` is the public error returned by both paths.
pub use reactive::hooks::router::{NavigateError, RouterHandle, use_router};
pub use router::{NavigationType, navigate};
pub use router::{Path, Query};
pub use server_fn::{ServerFn, ServerFnError, parse_server_error_message};
pub use ssr::SsrState;
#[cfg(native)]
pub use ssr::{SsrOptions, SsrRenderer};
pub use static_resolver::{init_static_resolver, is_initialized, resolve_static};

// Re-export procedural macros
pub use reinhardt_pages_macros::form;
pub use reinhardt_pages_macros::head;
pub use reinhardt_pages_macros::page;
pub use reinhardt_pages_macros::wasm_server_api;
pub use reinhardt_pages_macros::{FromRequest, client_page, component, page_props};

// Private re-exports used by macro-generated code. Not part of the public API.
#[doc(hidden)]
pub mod __private {
	pub mod fetch {
		pub use crate::fetch::{
			FetchCredentials, FetchResponse, request, request_with_credentials,
		};
	}
	pub use bon;
	pub use inventory;
	pub use reinhardt_urls;

	// `tracing` is enabled for all targets *except* browser wasm (wasm32-unknown-unknown).
	// Browser wasm uses a different logging mechanism, so tracing is intentionally excluded there.
	// The `native` cfg alias (defined in build.rs as
	// `not(all(target_family = "wasm", target_os = "unknown"))`) precisely targets every
	// non-browser-wasm platform.
	#[cfg(native)]
	pub use tracing;
}

// Logging macros are automatically exported via #[macro_export]
// Users can access them as: reinhardt_pages::debug_log!, reinhardt_pages::info_log!, etc.
