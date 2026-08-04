//! Configuration module for examples-tutorial-basis

#[cfg(server)]
pub mod admin;
pub mod apps;
#[cfg(server)]
pub mod settings;
#[cfg(server)]
pub mod session_auth;
#[cfg(all(server, feature = "commands-shell"))]
pub mod shell;
pub mod urls;
#[cfg(server)]
pub mod wasm;
