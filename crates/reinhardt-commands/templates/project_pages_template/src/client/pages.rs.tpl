//! Page components for {{ project_name }}.
//!
//! Each function returns a [`Page`] that the router can mount. Pages
//! typically delegate rendering to per-app component modules under
//! `super::components`.
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt::pages::component::Page;
//!
//! pub fn index_page() -> Page {
//!     crate::client::components::polls::polls_index()
//! }
//! ```

#[allow(unused_imports)] // `Page` will be used once page functions are added.
use reinhardt::pages::component::Page;
