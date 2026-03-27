//! WASM-based reactive frontend framework with SSR
//!
//! This module provides access to reinhardt-pages, a reactive UI framework
//! for building full-stack applications with Rust and WebAssembly.
//!
//! ## Architecture
//!
//! - **Component System**: Reactive components with Signal-based state management
//! - **Server Functions**: RPC-style communication between client and server
//! - **SSR Support**: Server-side rendering with client-side hydration
//! - **Routing**: History API-based client-side routing
//!
//! ## Example
//!
//! ```rust,ignore
//! use reinhardt::pages::prelude::*;
//! use reinhardt::pages::component::{PageElement, IntoPage, Page};
//! use reinhardt::pages::Signal;
//!
//! // Define a reactive component
//! pub fn counter() -> Page {
//!     let count = Signal::new(0);
//!
//!     let on_click = {
//!         let count = count.clone();
//!         move |_event: web_sys::Event| {
//!             count.set(count.get() + 1);
//!         }
//!     };
//!
//!     PageElement::new("div")
//!         .child(
//!             PageElement::new("button")
//!                 .attr("type", "button")
//!                 .listener("click", on_click)
//!                 .child(format!("Count: {}", count.get()))
//!         )
//!         .into_page()
//! }
//!
//! // Define a server function
//! #[server_fn]
//! pub async fn get_data(
//!     #[inject] db: DatabaseConnection,
//! ) -> std::result::Result<String, ServerFnError> {
//!     // Server-side logic here
//!     Ok("Hello from server!".to_string())
//! }
//! ```

// Re-export all reinhardt-pages functionality
pub use reinhardt_pages::*;
