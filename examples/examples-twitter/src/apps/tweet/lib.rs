//! Tweet application module

#[cfg(native)]
use reinhardt::app_config;

#[cfg(native)]
pub mod admin;
#[cfg(native)]
pub mod models;
pub mod shared;
pub mod urls;

#[cfg(wasm)]
pub mod client;

#[cfg(native)]
pub mod server;

#[cfg(test)]
pub mod tests;

#[cfg(native)]
#[app_config(name = "tweet", label = "tweet", verbose_name = "Tweets")]
pub struct TweetConfig;
