//! Reinhardt Pages - WASM-based Frontend Framework
//!
//! A Django-inspired frontend framework for Reinhardt that preserves the benefits of
//! Django templates while leveraging WebAssembly for modern interactivity.
//!
//! ## Features
//!
//! - **Fine-grained Reactivity**: Leptos/Solid.js-style Signal system with automatic dependency tracking
//! - **Hybrid Rendering**: SSR + Client-side Hydration for optimal performance and SEO
//! - **Django-like API**: Familiar patterns for Reinhardt developers
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
//! - [`component`]: Component system with IntoPage trait, Head management
//! - [`form`](mod@form): Django Form integration
//! - [`csrf`]: CSRF protection
//! - [`auth`]: Authentication integration
//! - [`api`]: API client with Django QuerySet-like interface
//! - [`server_fn`]: Server Functions (RPC)
//! - [`ssr`]: Server-side rendering with Head support
//! - [`hydration`]: Client-side hydration
//! - [`router`]: Client-side routing (reinhardt-urls compatible)
//! - [`static_resolver`]: Static file URL resolution (collectstatic support)
//!
//! ## Macros
//!
//! - [`page!`]: JSX-like macro for defining view components
//! - [`head!`]: JSX-like macro for defining HTML head sections
//!
//! ## Example
//!
//! ### Basic Component
//!
//! ```ignore
//! use reinhardt_pages::{Signal, Page, page};
//!
//! fn counter() -> Page {
//!     let count = Signal::new(0);
//!
//!     page!(|| {
//!         div {
//!             p { "Count: " }
//!             button {
//!                 @click: |_| count.update(|n| *n += 1),
//!                 "Increment"
//!             }
//!         }
//!     })()
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
//!     use_effect({
//!         let ws = ws.clone();
//!         move || {
//!             match ws.connection_state().get() {
//!                 ConnectionState::Open => log!("Connected to chat"),
//!                 ConnectionState::Closed => log!("Disconnected from chat"),
//!                 ConnectionState::Error(e) => log!("Connection error: {}", e),
//!                 _ => {}
//!             }
//!             None::<fn()>
//!         }
//!     });
//!
//!     // Handle incoming messages
//!     use_effect({
//!         let ws = ws.clone();
//!         move || {
//!             if let Some(WebSocketMessage::Text(text)) = ws.latest_message().get() {
//!                 log!("Received: {}", text);
//!             }
//!             None::<fn()>
//!         }
//!     });
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
pub mod spawn;

// Platform abstraction (unified types for WASM and native)
pub mod platform;

// Unified prelude for simplified imports
pub mod prelude;

// Component system
pub mod component;

// Form and security
pub mod auth;
pub mod csrf;
// Static form metadata types for form! macro (WASM-compatible)
pub mod form_generated;
// FormComponent requires reinhardt-forms which is not WASM-compatible yet
// For now, client-side forms should use PageElement
#[cfg(not(target_arch = "wasm32"))]
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

// Integration modules (runtime support for special macros)
pub mod integ;

// Testing utilities (available on both WASM and server)
// Layer 1: server_fn unit tests (server-side only)
// Layer 2: Component + server_fn mock tests (WASM)
// Layer 3: E2E tests (both platforms)
pub mod testing;

// Static file URL resolver
pub mod static_resolver;

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
#[cfg(not(target_arch = "wasm32"))]
pub use component::DummyEvent;
pub use component::{
	Component, Head, IntoPage, LinkTag, MetaTag, Page, PageElement, PageExt, Props, ScriptTag,
	StyleTag,
};
pub use csrf::{CsrfManager, get_csrf_token};
pub use dom::{Document, Element, EventHandle, EventType, document};
#[cfg(not(target_arch = "wasm32"))]
pub use form::{FormBinding, FormComponent};
// Static form metadata types (always available, used by form! macro)
pub use form_generated::{StaticFieldMetadata, StaticFormMetadata};
pub use hydration::{HydrationContext, HydrationError, hydrate};
pub use reactive::{Effect, Memo, Resource, ResourceState, Signal};
#[cfg(target_arch = "wasm32")]
pub use reactive::{create_resource, create_resource_with_deps};
// Re-export Context system
pub use reactive::{
	Context, ContextGuard, create_context, get_context, provide_context, remove_context,
};
// Re-export Hooks API
pub use reactive::{
	ActionState, Dispatch, OptimisticState, Ref, SetState, SharedSetState, SharedSignal,
	TransitionState, use_action_state, use_callback, use_context, use_debug_value,
	use_deferred_value, use_effect, use_effect_event, use_id, use_layout_effect, use_memo,
	use_optimistic, use_reducer, use_ref, use_shared_state, use_state, use_sync_external_store,
	use_transition,
};
#[cfg(not(target_arch = "wasm32"))]
pub use reinhardt_forms::{
	Widget,
	wasm_compat::{FieldMetadata, FormMetadata},
};
pub use router::{Link, PathPattern, Route, Router, RouterOutlet};
pub use server_fn::{ServerFn, ServerFnError};
pub use ssr::{SsrOptions, SsrRenderer, SsrState};
pub use static_resolver::{init_static_resolver, is_initialized, resolve_static};

// Re-export procedural macros
pub use reinhardt_pages_macros::form;
pub use reinhardt_pages_macros::head;
pub use reinhardt_pages_macros::page;

// Logging macros are automatically exported via #[macro_export]
// Users can access them as: reinhardt_pages::debug_log!, reinhardt_pages::info_log!, etc.
