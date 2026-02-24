//! Auth client module (WASM)
//!
//! Contains authentication-related UI components and state management.
//! This module is only compiled for WebAssembly target.

pub mod components;
pub mod state;

pub use components::*;
pub use state::*;
