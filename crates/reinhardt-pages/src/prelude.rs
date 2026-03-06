//! Unified prelude for reinhardt-pages.
//!
//! This module provides all commonly used types with a single import statement,
//! eliminating the need for conditional compilation in most user code.
//!
//! # Example
//!
//! ```rust,ignore
//! // Instead of multiple imports with cfg guards:
//! // #[cfg(target_arch = "wasm32")]
//! // use reinhardt_pages::{Signal, Page, use_state, ...};
//! // #[cfg(not(target_arch = "wasm32"))]
//! // use reinhardt::pages::{Signal, Page, use_state, ...};
//!
//! // Use the unified prelude:
//! use reinhardt_pages::prelude::*;
//! // or
//! use reinhardt::pages::prelude::*;
//!
//! fn my_component() -> Page {
//!     let (count, set_count) = use_state(|| 0);
//!     // Component logic...
//!     Page::empty()
//! }
//! ```
//!
//! # Included Types
//!
//! ## Reactive System
//! - [`Signal`], [`Effect`], [`Memo`], [`Resource`], [`ResourceState`]
//! - Context: [`Context`], [`ContextGuard`], [`create_context`], [`get_context`],
//!   [`provide_context`], [`remove_context`]
//!
//! ## Hooks
//! - [`use_state`], [`use_effect`], [`use_memo`], [`use_callback`], [`use_context`]
//! - [`use_ref`], [`use_reducer`], [`use_transition`], [`use_deferred_value`]
//! - [`use_id`], [`use_layout_effect`], [`use_effect_event`], [`use_debug_value`]
//! - [`use_optimistic`], [`use_action_state`], [`use_shared_state`]
//! - [`use_sync_external_store`]
//!
//! ## Component System
//! - [`Component`], [`PageElement`], [`IntoPage`], [`Page`], [`Props`]
//! - [`PageEventHandler`]
//!
//! ## Events and Callbacks
//! - [`Callback`], [`IntoEventHandler`], [`into_event_handler`]
//! - [`Event`] (platform-agnostic event type)
//!
//! ## DOM
//! - [`Document`], [`Element`], [`EventHandle`], [`EventType`], [`document`](fn@document)
//!
//! ## Routing
//! - [`Link`], [`Router`], [`Route`], [`RouterOutlet`], [`PathPattern`]
//!
//! ## Macros
//! - [`page`] - Component DSL for defining views
//! - [`head`] - HTML head section DSL
//!
//! ## Static Files
//! - [`resolve_static`] - Resolve static file URLs
//! - [`init_static_resolver`] - Initialize the static resolver
//! - [`is_initialized`] - Check if static resolver is initialized

// ============================================================================
// Reactive System
// ============================================================================

pub use crate::reactive::{Effect, Memo, Resource, ResourceState, Signal};

// Context system
pub use crate::reactive::{
	Context, ContextGuard, create_context, get_context, provide_context, remove_context,
};

// Hooks API
pub use crate::reactive::{
	ActionState, Dispatch, OptimisticState, Ref, SetState, SharedSetState, SharedSignal,
	TransitionState, use_action_state, use_callback, use_context, use_debug_value,
	use_deferred_value, use_effect, use_effect_event, use_id, use_layout_effect, use_memo,
	use_optimistic, use_reducer, use_ref, use_shared_state, use_state, use_sync_external_store,
	use_transition,
};

// WASM-only resource creation functions
#[cfg(target_arch = "wasm32")]
pub use crate::reactive::{create_resource, create_resource_with_deps};

// ============================================================================
// Component System
// ============================================================================

pub use crate::component::{
	Component, Head, IntoPage, LinkTag, MetaTag, Page, PageElement, PageEventHandler, PageExt,
	Props, ScriptTag, StyleTag,
};

// ============================================================================
// Events and Callbacks
// ============================================================================

pub use crate::callback::{Callback, IntoEventHandler, into_event_handler};

// Platform-agnostic Event type
pub use crate::platform::Event;

// ============================================================================
// DOM
// ============================================================================

pub use crate::dom::{Document, Element, EventHandle, EventType, document};

// ============================================================================
// Routing
// ============================================================================

pub use crate::router::{Link, PathPattern, Route, Router, RouterOutlet};

// ============================================================================
// API and Server Functions
// ============================================================================

pub use crate::api::{ApiModel, ApiQuerySet, Filter, FilterOp};
pub use crate::server_fn::{ServerFn, ServerFnError};

// ============================================================================
// Authentication and Security
// ============================================================================

pub use crate::auth::{AuthData, AuthError, AuthState, auth_state};
pub use crate::csrf::{CsrfManager, get_csrf_token};

// ============================================================================
// SSR and Hydration
// ============================================================================

pub use crate::hydration::{
	HydrationContext, HydrationError, hydrate, init_hydration_state, is_hydration_complete,
	on_hydration_complete,
};

#[cfg(target_arch = "wasm32")]
pub use crate::hydration::mark_hydration_complete;
pub use crate::ssr::{SsrOptions, SsrRenderer, SsrState};

// ============================================================================
// Static File URL Resolver
// ============================================================================

pub use crate::static_resolver::{init_static_resolver, is_initialized, resolve_static};

// ============================================================================
// Forms (native only)
// ============================================================================

#[cfg(not(target_arch = "wasm32"))]
pub use crate::form::{FormBinding, FormComponent};

#[cfg(not(target_arch = "wasm32"))]
pub use reinhardt_forms::{
	Widget,
	wasm_compat::{FieldMetadata, FormMetadata},
};

// ============================================================================
// Macros
// ============================================================================

pub use crate::form;
pub use crate::head;
pub use crate::page;

// ============================================================================
// WASM-specific utilities
// ============================================================================

/// Spawn a local async task (WASM only).
///
/// This is a convenience re-export from `wasm_bindgen_futures`.
#[cfg(target_arch = "wasm32")]
pub use wasm_bindgen_futures::spawn_local;
