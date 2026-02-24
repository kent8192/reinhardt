//! Tweet server module
//!
//! Contains server-side tweet functions.
//! This module is only compiled for non-WebAssembly targets.

pub mod server_fn;

pub use server_fn::*;
