//! Mobile deep linking module.
//!
//! This module provides iOS Universal Links, Android App Links,
//! and custom URL scheme support for mobile app integration.
//!
//! # Examples
//!
//! ```rust,no_run
//! use reinhardt::deeplink::{DeeplinkConfig, DeeplinkRouter};
//! ```

#[cfg(feature = "deeplink")]
pub use reinhardt_deeplink::*;
