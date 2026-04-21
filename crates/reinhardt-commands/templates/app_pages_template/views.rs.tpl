//! Views module for {{ app_name }} app (Pages)
//!
//! Define ViewSets here. Each `pub mod` declaration corresponds to a file
//! under the `views/` directory.
//!
//! For multi-file views that need re-exports for discovery, use:
//! ```rust,ignore
//! flatten_imports! {
//!     pub mod example;
//! }
//! ```
//!
//! # Example ViewSet
//!
//! ```rust,ignore
//! use reinhardt::prelude::*;
//! use reinhardt::viewset;
//!
//! // Import your model here
//! // use crate::models::{{ camel_case_app_name }};
//!
//! #[viewset]
//! pub struct {{ camel_case_app_name }}ViewSet;
//! ```
