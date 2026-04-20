//! Configuration module for examples-twitter

#[cfg(native)]
pub mod admin;
// `installed_apps!` macro is server-only (the facade re-exports it under
// `cfg(all(feature = "core", native))` and WASM builds disable `core`).
// See #3825.
#[cfg(native)]
pub mod apps;
#[cfg(native)]
pub mod middleware;
#[cfg(native)]
pub mod settings;
pub mod urls;
#[cfg(native)]
pub mod wasm;
