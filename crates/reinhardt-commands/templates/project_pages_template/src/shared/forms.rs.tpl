//! Server-side `Form` definitions for {{ project_name }}.
//!
//! Forms here are used server-side to generate `FormMetadata` (carrying
//! CSRF tokens and field definitions) that the WASM client reads via
//! server functions.
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt::forms::field::Widget;
//! use reinhardt::forms::{CharField, Form};
//!
//! pub fn create_login_form() -> Form {
//!     let mut form = Form::new();
//!     form.add_field(Box::new(
//!         CharField::new("username".to_string())
//!             .with_label("Username")
//!             .required(),
//!     ));
//!     form
//! }
//! ```
