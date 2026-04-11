//! `MockableServerFn` trait for MSW-style testing.
//!
//! This trait exposes server function metadata (Args/Response types, codec)
//! for type-safe mocking in WASM tests. It is automatically implemented by
//! the `#[server_fn]` macro when the `msw` feature is enabled.

use serde::Serialize;
use serde::de::DeserializeOwned;

#[cfg(native)]
use super::registration::ServerFnRegistration;

/// Exposes server function metadata for MSW-style mocking.
///
/// Automatically implemented by `#[server_fn]` macro when the `msw` feature
/// is enabled. Provides compile-time access to Args/Response types and codec
/// information, enabling type-safe mock handler registration.
///
/// On native targets, this trait requires `ServerFnRegistration` as a
/// supertrait, inheriting `PATH`, `NAME`, and `CODEC` constants.
///
/// On WASM targets, `ServerFnRegistration` is unavailable, so this trait
/// duplicates those constants directly.
#[cfg(native)]
pub trait MockableServerFn: ServerFnRegistration {
	/// Client-side argument struct (excludes `#[inject]` parameters).
	type Args: Serialize + DeserializeOwned + 'static;

	/// Success response type (the `Ok` variant of the function's return type).
	type Response: Serialize + DeserializeOwned + 'static;

	/// Names of `#[inject]` parameters (for documentation/debugging).
	const INJECTED_PARAMS: &'static [&'static str] = &[];
}

/// WASM-side version of `MockableServerFn`.
///
/// On WASM targets, `ServerFnRegistration` is not available (it is
/// `#[cfg(native)]`-only). This version provides the same metadata
/// without the supertrait bound, duplicating `PATH`, `NAME`, and `CODEC`.
#[cfg(wasm)]
pub trait MockableServerFn: 'static {
	/// Client-side argument struct (excludes `#[inject]` parameters).
	type Args: Serialize + DeserializeOwned + 'static;

	/// Success response type (the `Ok` variant of the function's return type).
	type Response: Serialize + DeserializeOwned + 'static;

	/// The HTTP path for this server function.
	const PATH: &'static str;

	/// The name of the server function.
	const NAME: &'static str;

	/// The codec this server function uses.
	const CODEC: &'static str = "json";

	/// Names of `#[inject]` parameters (for documentation/debugging).
	const INJECTED_PARAMS: &'static [&'static str] = &[];
}
