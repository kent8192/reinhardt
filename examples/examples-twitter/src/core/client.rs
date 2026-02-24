//! WASM client module
//!
//! This module contains the WASM client entry point and shared client components.

pub mod components;
pub mod lib;
pub mod pages;
pub mod router;

// Re-export main entry point and utilities
pub use lib::main;
pub use router::{AppRoute, init_global_router, with_router};
