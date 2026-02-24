//! Code generation infrastructure.
//!
//! This module provides the `IRVisitor` trait that backends implement
//! for generating platform-specific code.
//!
//! ## Example Implementation
//!
//! ```rust,ignore
//! use reinhardt_manouche::codegen::IRVisitor;
//! use reinhardt_manouche::ir::*;
//! use proc_macro2::TokenStream;
//!
//! struct WebVisitor;
//!
//! impl IRVisitor for WebVisitor {
//!     type Output = TokenStream;
//!
//!     fn visit_element(&mut self, ir: &ElementIR) -> TokenStream {
//!         // Generate web-sys code
//!     }
//!     // ... implement other methods
//! }
//! ```

mod visitor;
mod walk;

pub use visitor::IRVisitor;
pub use walk::*;
