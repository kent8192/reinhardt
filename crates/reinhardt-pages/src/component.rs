//! Component System for reinhardt-pages
//!
//! This module provides a component abstraction for building reusable UI elements.
//! Components can be rendered both on the server (SSR) and client (WASM).
//!
//! ## Features
//!
//! - **IntoPage trait**: Convert any type into a renderable Page
//! - **Component trait**: Define reusable UI components
//! - **Page enum**: Unified representation of DOM elements, text, and fragments
//! - **Props system**: Type-safe component properties
//! - **Boundaries**: Suspense, error, activity, and view-transition components
//!
//! ## Usage
//!
//! ```ignore
//! use reinhardt_pages::component::{Component, IntoPage, Page};
//!
//! // Simple component as a function
//! fn greeting(name: &str) -> impl IntoPage {
//!     div()
//!         .class("greeting")
//!         .child(p().text(&format!("Hello, {}!", name)).build())
//!         .build()
//! }
//!
//! // Using the component
//! let page = greeting("World").into_page();
//! let html = page.render_to_string();
//! ```

pub mod activity;
pub mod error_boundary;
mod into_page;
mod props;
pub(crate) mod reactive_if;
pub mod suspense;
mod r#trait;
pub mod view_transition;

// Re-export Page types (originally from into_page, now from reinhardt-types via into_page)
pub use activity::{ActivityBoundary, ActivityMode};
pub use error_boundary::{BoundaryError, ErrorBoundary, ErrorTracker};
#[cfg(native)]
pub use into_page::DummyEvent;
pub use into_page::PageExt;
pub use into_page::{
	Head, IntoPage, LinkTag, MetaTag, MountError, Page, PageElement, PageEventHandler, Reactive,
	ReactiveIf, ScriptTag, StyleTag,
};
pub use props::Props;
#[cfg(wasm)]
pub use reactive_if::{ReactiveIfNode, ReactiveNode, cleanup_reactive_nodes, store_reactive_node};
#[doc(hidden)]
pub use reinhardt_core::types::page::{ControlBinding, ControlKind, ControlValue};
pub use suspense::{ResourceTracker, SuspenseBoundary};
pub use r#trait::Component;
pub use view_transition::{
	ViewTransitionBoundary, ViewTransitionHandle, ViewTransitionStatus, start_view_transition,
};
