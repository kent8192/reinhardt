//! Request dispatching module.
//!
//! This module provides HTTP request dispatching, handler composition,
//! and middleware chain execution.
//!
//! # Examples
//!
//! ```rust,no_run
//! use reinhardt::dispatch::{BaseHandler, Dispatcher};
//! ```

#[cfg(feature = "dispatch")]
pub use reinhardt_dispatch::*;
