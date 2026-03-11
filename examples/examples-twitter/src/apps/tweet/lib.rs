//! Tweet application module

#[cfg(server)]
pub mod admin;
#[cfg(server)]
pub mod models;
pub mod shared;
pub mod urls;

#[cfg(client)]
pub mod client;

#[cfg(server)]
pub mod server;

#[cfg(test)]
pub mod tests;
