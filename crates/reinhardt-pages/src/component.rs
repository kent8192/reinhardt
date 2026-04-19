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

mod into_page;
mod props;
pub(crate) mod reactive_if;
mod r#trait;

// Re-export Page types (originally from into_page, now from reinhardt-types via into_page)
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
pub use r#trait::Component;
