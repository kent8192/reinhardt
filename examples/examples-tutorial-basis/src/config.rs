//! Configuration module for examples-tutorial-basis

#[cfg(server)]
pub mod admin;
pub mod apps;
#[cfg(server)]
pub mod settings;
#[cfg(server)]
pub mod session_auth;
pub mod urls;
#[cfg(server)]
pub mod wasm;
