//! Auth server module
//!
//! Contains server-side authentication functions.
//! This module is only compiled for non-WebAssembly targets.

pub mod server_fn;

pub use server_fn::*;
