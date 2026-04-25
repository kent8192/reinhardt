//! Configuration module for {{ project_name }}.

#[cfg(server)]
pub mod apps;
#[cfg(server)]
pub mod settings;
pub mod urls;
#[cfg(server)]
pub mod wasm;
