//! Client-side (WASM) modules for the {{ app_name }} application.
//!
//! Reached only on the WASM target via `#[cfg(client)] pub mod client;`
//! in the parent app aggregator; contents below need no additional gates.
//!
//! The freshly generated app contains a route-backed placeholder component
//! under `components/placeholder.rs`. Register additional components from
//! `../urls/client_router.rs` as the app grows. Add component-scoped styles in
//! `style.rs`.

pub mod components;
pub mod hooks;
pub mod style;
