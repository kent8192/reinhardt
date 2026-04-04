//! WASM UI Application for Reinhardt Admin Panel
//!
//! This crate provides the client-side UI application for the admin panel,
//! built with reinhardt-pages and compiled to WebAssembly.
//!
//! # Architecture
//!
//! The application is organized into:
//! - `components/layout` - Layout components (header, sidebar, footer)
//! - `components/common` - Common reusable components (buttons, modals, etc.)
//! - `components/features` - Feature-specific components (dashboard, list, form, etc.)
//! - `router` - Client-side routing
//!
//! # Components
//!
//! All components are built using reinhardt-pages' reactive system with:
//! - `Signal<T>` for reactive state
//! - `create_resource` for Server Function calls
//! - `view!` macro for declarative UI
//!
//! # Example
//!
//! ```ignore
//! use reinhardt_admin_app::components::features::dashboard::dashboard_view;
//!
//! // In your app
//! view! {
//!     <div>
//!         {dashboard_view()}
//!     </div>
//! }
//! ```

#[cfg(client)]
mod main;

pub mod components;
pub mod router;

// Re-exports
pub use components::*;
pub use router::*;
