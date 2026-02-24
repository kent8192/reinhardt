//! Parsing logic for DSL macros.
//!
//! This module provides parsers for:
//! - `page!` macro → `PageMacro`
//! - `form!` macro → `FormMacro`
//! - `head!` macro → `HeadMacro`

mod form;
mod head;
mod page;

pub use form::*;
pub use head::*;
pub use page::*;
