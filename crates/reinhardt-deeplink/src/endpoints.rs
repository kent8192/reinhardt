//! HTTP endpoint handlers for deeplink files.
//!
//! This module provides handlers for serving the well-known files required by
//! mobile platforms for deep linking:
//!
//! - [`AasaHandler`] - Apple App Site Association (`/.well-known/apple-app-site-association`)
//! - [`AssetLinksHandler`] - Android Digital Asset Links (`/.well-known/assetlinks.json`)

mod aasa;
mod assetlinks;

pub use aasa::AasaHandler;
pub use assetlinks::AssetLinksHandler;
