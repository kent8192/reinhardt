//! WASM Plugin Support
//!
//! This module provides WebAssembly plugin support for the Dentdelion plugin system
//! using the WASM Component Model and WIT interfaces.
//!
//! # Features
//!
//! - Dynamic plugin loading from `.wasm` component files
//! - Sandboxed execution with capability-based permissions
//! - Full host API (config, logging, services, HTTP, database)
//! - Resource limits (memory: 128MB, timeout: 30s by default)
//!
//! # Constraints
//!
//! - Memory limit: 128MB default (configurable via `WasmPluginConfig`)
//! - Execution timeout: 30 seconds default (configurable)
//! - `Models` capability is NOT supported (requires compile-time integration)
//! - HTTP/DB access requires capability-based permission checks
//!
//! # Example
//!
//! ```ignore
//! use reinhardt_dentdelion::wasm::{WasmRuntime, WasmPluginLoader};
//!
//! // Create runtime with default configuration
//! let runtime = WasmRuntime::new(Default::default())?;
//!
//! // Create loader for plugin directory
//! let loader = WasmPluginLoader::new(".dentdelion/plugins", runtime);
//!
//! // Discover and load plugins
//! let plugins = loader.discover().await?;
//! for path in plugins {
//!     let instance = loader.load(&path).await?;
//!     instance.on_load(&config).await?;
//! }
//! ```

mod events;
mod host;
mod instance;
mod loader;
mod models;
mod runtime;
mod sql_validator;
pub mod ssr;
#[cfg(feature = "ts")]
mod ts_runtime;
mod types;

pub use events::{Event, EventBus, EventBusError, SharedEventBus};
pub use host::{HostState, HostStateBuilder};
pub use instance::WasmPluginInstance;
pub use loader::WasmPluginLoader;
pub use models::{
	ColumnDef, ColumnType, IndexDef, ModelRegistry, ModelSchema, SharedModelRegistry, SqlMigration,
};
pub use runtime::{WasmRuntime, WasmRuntimeConfig, WasmRuntimeConfigBuilder};
pub use sql_validator::{
	SqlStatementType, SqlValidationError, SqlValidator, default_validator, validate_sql,
};
pub use ssr::{RenderOptions, RenderResult, SharedSsrProxy, SsrError, SsrProxy, escape_for_script};
#[cfg(feature = "ts")]
pub use ts_runtime::{SharedTsRuntime, TsError, TsResourceLimits, TsRuntime};
pub use types::{ConfigValue, WitCapability, WitPluginError, WitPluginMetadata};

/// WASM binary magic bytes (`\0asm`).
///
/// Every valid WebAssembly binary starts with these four bytes as defined
/// by the [WebAssembly specification](https://webassembly.github.io/spec/core/binary/modules.html#binary-magic).
pub const WASM_MAGIC: [u8; 4] = [0x00, 0x61, 0x73, 0x6D];

/// Validate that the given bytes represent a valid WASM binary by checking magic bytes.
///
/// This is a basic integrity check that verifies the WASM magic header (`\0asm`).
/// It provides early rejection of obviously invalid files, reducing attack surface
/// by preventing non-WASM files from reaching the WASM runtime.
///
/// **Note**: This is not a security boundary. It only checks the 4-byte magic header
/// and does not validate the WASM version field (bytes 4-7) or module structure.
/// Full validation is performed by the WASM runtime during compilation.
///
/// # Arguments
///
/// * `bytes` - The bytes to validate
///
/// # Returns
///
/// `true` if the bytes start with the WASM magic bytes, `false` otherwise.
pub fn is_valid_wasm(bytes: &[u8]) -> bool {
	bytes.len() >= 4 && bytes[..4] == WASM_MAGIC
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_wasm_magic_validation() {
		// Valid WASM magic
		let valid = [0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];
		assert!(is_valid_wasm(&valid));

		// Invalid magic
		let invalid = [0x00, 0x00, 0x00, 0x00];
		assert!(!is_valid_wasm(&invalid));

		// Too short
		let short = [0x00, 0x61, 0x73];
		assert!(!is_valid_wasm(&short));

		// Empty
		assert!(!is_valid_wasm(&[]));
	}
}
