//! WASM stub types for dependency injection
//!
//! These types are only used for type checking on WASM targets.
//! They provide dummy implementations of server-side types that appear
//! in Server Function signatures but are automatically injected and
//! filtered out by the `#[server_fn]` macro on the client side.

#[cfg(client)]
pub use wasm_only::*;

#[cfg(client)]
mod wasm_only {
	/// Dummy AdminSite type for WASM type checking
	///
	/// This type is never actually used in WASM code, as the `#[server_fn]`
	/// macro removes all dependency injection parameters from client stubs.
	/// It exists purely for type checking purposes.
	pub struct AdminSite;

	/// Dummy AdminDatabase type for WASM type checking
	///
	/// This type is never actually used in WASM code, as the `#[server_fn]`
	/// macro removes all dependency injection parameters from client stubs.
	/// It exists purely for type checking purposes.
	pub struct AdminDatabase;

	/// Dummy AdminRecord type for WASM type checking
	///
	/// This type is never actually used in WASM code.
	pub struct AdminRecord;

	/// Dummy ModelAdmin type for WASM type checking
	///
	/// This type is never actually used in WASM code.
	pub struct ModelAdmin;

	/// Dummy ExportFormat type for WASM type checking
	///
	/// This type is never actually used in WASM code.
	#[derive(serde::Serialize, serde::Deserialize)]
	pub struct ExportFormat;

	/// Dummy ImportBuilder type for WASM type checking
	///
	/// This type is never actually used in WASM code.
	pub struct ImportBuilder;

	/// Dummy ImportError type for WASM type checking
	///
	/// This type is never actually used in WASM code.
	pub struct ImportError;

	/// Dummy ImportFormat type for WASM type checking
	///
	/// This type is never actually used in WASM code.
	#[derive(serde::Serialize, serde::Deserialize)]
	pub struct ImportFormat;

	/// Dummy ImportResult type for WASM type checking
	///
	/// This type is never actually used in WASM code.
	pub struct ImportResult;
}
