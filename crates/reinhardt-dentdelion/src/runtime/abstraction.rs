//! Runtime abstraction layer for multi-runtime plugin support.
//!
//! This module defines the core abstractions that enable plugins to run on
//! different runtime backends (Static Rust, WASM, TypeScript/deno_core).
//!
//! See [`RuntimeType`] for the architecture diagram.
//!
//! # Example
//!
//! ```ignore
//! use reinhardt_dentdelion::runtime::{PluginRuntime, RuntimeType};
//!
//! async fn execute_plugin(runtime: &dyn PluginRuntime) {
//!     match runtime.runtime_type() {
//!         RuntimeType::Static => println!("Running as Rust code"),
//!         RuntimeType::Wasm => println!("Running in WASM sandbox"),
//!         RuntimeType::TypeScript => println!("Running in V8 (deno_core)"),
//!     }
//! }
//! ```

use crate::capability::{Capability, PluginTier};
use crate::error::PluginError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::sync::Arc;

#[cfg_attr(doc, aquamarine::aquamarine)]
/// Identifies the type of runtime a plugin uses.
///
/// This enum allows the framework to make runtime-specific decisions
/// about capability support, resource limits, and execution strategies.
///
/// # Architecture
///
/// The runtime abstraction provides a unified interface for executing plugin
/// code regardless of the underlying runtime:
///
/// ```mermaid
/// classDiagram
///     class PluginRuntime {
///         <<trait>>
///         +runtime_type() RuntimeType
///         +invoke() Result
///     }
///     class StaticRuntime {
///         Rust crates
///         compile-time integration
///     }
///     class WasmRuntime {
///         wasmtime
///         Component Model
///     }
///     class TsRuntime {
///         deno_core
///         TypeScript/JavaScript
///     }
///
///     PluginRuntime <|.. StaticRuntime
///     PluginRuntime <|.. WasmRuntime
///     PluginRuntime <|.. TsRuntime
/// ```
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeType {
	/// Static Rust plugin compiled with the host application.
	///
	/// Features:
	/// - Full access to Rust APIs
	/// - No sandboxing (runs in host process)
	/// - Compile-time integration (Models capability)
	/// - Highest performance
	Static,

	/// WebAssembly plugin running in wasmtime.
	///
	/// Features:
	/// - Sandboxed execution
	/// - Memory and CPU limits
	/// - Component Model interface (WIT)
	/// - Cross-platform compatibility
	Wasm,

	/// TypeScript/JavaScript plugin running in deno_core (V8).
	///
	/// Features:
	/// - Direct TypeScript execution (no transpilation)
	/// - SSR support for React/Vue
	/// - npm ecosystem access
	/// - V8 isolate sandboxing
	TypeScript,
}

impl RuntimeType {
	/// Returns a human-readable name for the runtime type.
	pub fn as_str(&self) -> &'static str {
		match self {
			Self::Static => "static",
			Self::Wasm => "wasm",
			Self::TypeScript => "typescript",
		}
	}

	/// Returns whether this runtime type supports hot module replacement (HMR).
	pub fn supports_hmr(&self) -> bool {
		match self {
			Self::Static => false,    // Requires recompilation
			Self::Wasm => false,      // Module reload possible but complex
			Self::TypeScript => true, // V8 supports dynamic module reload
		}
	}

	/// Returns whether this runtime type can run server-side rendering (SSR).
	pub fn supports_ssr(&self) -> bool {
		match self {
			Self::Static => false,    // Rust-native SSR via reinhardt-pages
			Self::Wasm => false,      // Limited JavaScript execution
			Self::TypeScript => true, // Full React/Vue SSR support
		}
	}
}

impl fmt::Display for RuntimeType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

impl std::str::FromStr for RuntimeType {
	type Err = String;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_lowercase().as_str() {
			"static" | "rust" => Ok(Self::Static),
			"wasm" | "webassembly" => Ok(Self::Wasm),
			"typescript" | "ts" | "javascript" | "js" | "deno" => Ok(Self::TypeScript),
			_ => Err(format!("unknown runtime type: {s}")),
		}
	}
}

/// Error type for runtime operations.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
	/// The requested function was not found in the plugin.
	#[error("function not found: {0}")]
	FunctionNotFound(String),

	/// Function execution failed.
	#[error("execution error: {0}")]
	ExecutionError(String),

	/// The capability is not supported by this runtime.
	#[error("capability not supported: {0}")]
	CapabilityNotSupported(String),

	/// Timeout during execution.
	#[error("execution timeout after {0:?}")]
	Timeout(std::time::Duration),

	/// Memory limit exceeded.
	#[error("memory limit exceeded: {used} bytes > {limit} bytes")]
	MemoryLimitExceeded {
		/// Bytes used
		used: usize,
		/// Maximum allowed bytes
		limit: usize,
	},

	/// Module loading error.
	#[error("module load error: {0}")]
	ModuleLoadError(String),

	/// Serialization/deserialization error.
	#[error("serialization error: {0}")]
	SerializationError(String),

	/// Internal runtime error.
	#[error("internal error: {0}")]
	Internal(String),
}

impl From<RuntimeError> for PluginError {
	fn from(err: RuntimeError) -> Self {
		PluginError::RuntimeError(err.to_string())
	}
}

/// Core trait for plugin runtime implementations.
///
/// This trait provides a unified interface for executing plugin code
/// across different runtime backends.
///
/// # Implementors
///
/// - `StaticRuntime`: Direct Rust function calls
/// - `WasmRuntime`: wasmtime Component Model execution
/// - `TsRuntime`: deno_core V8 execution
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to support concurrent execution.
#[async_trait]
pub trait PluginRuntime: Send + Sync {
	/// Returns the type of this runtime.
	fn runtime_type(&self) -> RuntimeType;

	/// Executes a function in the plugin.
	///
	/// # Arguments
	///
	/// * `function` - The name of the function to execute
	/// * `args` - Arguments to pass to the function as JSON values
	///
	/// # Returns
	///
	/// The return value of the function as a JSON value.
	///
	/// # Errors
	///
	/// Returns an error if the function is not found, execution fails,
	/// or the runtime encounters an internal error.
	async fn execute(&self, function: &str, args: Vec<Value>) -> Result<Value, RuntimeError>;

	/// Checks if this runtime supports a specific capability.
	///
	/// Some capabilities may not be available on all runtimes.
	/// For example, the `Models` capability requires compile-time
	/// integration and is only available on `Static` runtime.
	fn supports_capability(&self, capability: &Capability) -> bool;

	/// Returns the current memory usage in bytes.
	///
	/// Returns `None` if memory tracking is not available.
	fn memory_usage(&self) -> Option<usize> {
		None
	}

	/// Returns the configured memory limit in bytes.
	///
	/// Returns `None` if no limit is configured.
	fn memory_limit(&self) -> Option<usize> {
		None
	}

	/// Resets the runtime state.
	///
	/// This is useful for clearing cached data or resetting
	/// execution context between requests.
	async fn reset(&self) -> Result<(), RuntimeError> {
		Ok(())
	}
}

/// Type alias for a boxed runtime.
pub type BoxedRuntime = Box<dyn PluginRuntime>;

/// Type alias for an Arc-wrapped runtime.
pub type ArcRuntime = Arc<dyn PluginRuntime>;

/// Configuration for runtime resource limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeLimits {
	/// Maximum memory in bytes (default: 128MB).
	pub memory_limit_bytes: usize,

	/// Maximum execution time (default: 30 seconds).
	pub timeout: std::time::Duration,

	/// Maximum CPU instructions for metered execution (WASM only).
	pub fuel_limit: Option<u64>,
}

impl Default for RuntimeLimits {
	fn default() -> Self {
		Self {
			memory_limit_bytes: 128 * 1024 * 1024, // 128MB
			timeout: std::time::Duration::from_secs(30),
			fuel_limit: Some(100_000_000), // ~100M instructions
		}
	}
}

impl RuntimeLimits {
	/// Creates runtime limits from a plugin tier.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_dentdelion::capability::PluginTier;
	/// use reinhardt_dentdelion::runtime::RuntimeLimits;
	///
	/// let limits = RuntimeLimits::from_tier(PluginTier::Premium);
	/// assert_eq!(limits.memory_limit_bytes, 512 * 1024 * 1024);
	/// ```
	pub fn from_tier(tier: PluginTier) -> Self {
		Self {
			memory_limit_bytes: tier.memory_limit_bytes(),
			timeout: std::time::Duration::from_secs(tier.timeout_secs()),
			fuel_limit: Some(tier.fuel_limit()),
		}
	}

	/// Creates a builder for custom runtime limits.
	pub fn builder() -> RuntimeLimitsBuilder {
		RuntimeLimitsBuilder::default()
	}
}

/// Builder for creating custom runtime limits.
///
/// The fuel limit uses a nested `Option` to distinguish between:
/// - `None` = not set, use default
/// - `Some(None)` = explicitly disabled fuel metering
/// - `Some(Some(value))` = explicitly set fuel limit
#[derive(Debug, Clone, Default)]
pub struct RuntimeLimitsBuilder {
	memory_limit_bytes: Option<usize>,
	timeout: Option<std::time::Duration>,
	fuel_limit: Option<Option<u64>>,
}

impl RuntimeLimitsBuilder {
	/// Sets the memory limit in bytes.
	pub fn memory_limit_bytes(mut self, bytes: usize) -> Self {
		self.memory_limit_bytes = Some(bytes);
		self
	}

	/// Sets the memory limit in megabytes.
	pub fn memory_limit_mb(self, mb: usize) -> Self {
		self.memory_limit_bytes(mb * 1024 * 1024)
	}

	/// Sets the execution timeout.
	pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
		self.timeout = Some(timeout);
		self
	}

	/// Sets the execution timeout in seconds.
	pub fn timeout_secs(self, secs: u64) -> Self {
		self.timeout(std::time::Duration::from_secs(secs))
	}

	/// Sets the fuel limit (CPU instructions).
	pub fn fuel_limit(mut self, fuel: u64) -> Self {
		self.fuel_limit = Some(Some(fuel));
		self
	}

	/// Disables fuel metering.
	pub fn no_fuel_limit(mut self) -> Self {
		self.fuel_limit = Some(None);
		self
	}

	/// Builds the runtime limits, using defaults for unset values.
	pub fn build(self) -> RuntimeLimits {
		let defaults = RuntimeLimits::default();
		RuntimeLimits {
			memory_limit_bytes: self
				.memory_limit_bytes
				.unwrap_or(defaults.memory_limit_bytes),
			timeout: self.timeout.unwrap_or(defaults.timeout),
			fuel_limit: self.fuel_limit.unwrap_or(defaults.fuel_limit),
		}
	}

	/// Builds the runtime limits based on a tier, overriding specified values.
	pub fn build_from_tier(self, tier: PluginTier) -> RuntimeLimits {
		let tier_limits = RuntimeLimits::from_tier(tier);
		RuntimeLimits {
			memory_limit_bytes: self
				.memory_limit_bytes
				.unwrap_or(tier_limits.memory_limit_bytes),
			timeout: self.timeout.unwrap_or(tier_limits.timeout),
			fuel_limit: self.fuel_limit.unwrap_or(tier_limits.fuel_limit),
		}
	}
}

impl From<PluginTier> for RuntimeLimits {
	fn from(tier: PluginTier) -> Self {
		Self::from_tier(tier)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_runtime_type_display() {
		assert_eq!(RuntimeType::Static.to_string(), "static");
		assert_eq!(RuntimeType::Wasm.to_string(), "wasm");
		assert_eq!(RuntimeType::TypeScript.to_string(), "typescript");
	}

	#[test]
	fn test_runtime_type_as_str() {
		assert_eq!(RuntimeType::Static.as_str(), "static");
		assert_eq!(RuntimeType::Wasm.as_str(), "wasm");
		assert_eq!(RuntimeType::TypeScript.as_str(), "typescript");
	}

	#[test]
	fn test_runtime_type_from_str() {
		assert_eq!(
			"static".parse::<RuntimeType>().unwrap(),
			RuntimeType::Static
		);
		assert_eq!("rust".parse::<RuntimeType>().unwrap(), RuntimeType::Static);
		assert_eq!("wasm".parse::<RuntimeType>().unwrap(), RuntimeType::Wasm);
		assert_eq!(
			"webassembly".parse::<RuntimeType>().unwrap(),
			RuntimeType::Wasm
		);
		assert_eq!(
			"typescript".parse::<RuntimeType>().unwrap(),
			RuntimeType::TypeScript
		);
		assert_eq!(
			"ts".parse::<RuntimeType>().unwrap(),
			RuntimeType::TypeScript
		);
		assert_eq!(
			"javascript".parse::<RuntimeType>().unwrap(),
			RuntimeType::TypeScript
		);
		assert_eq!(
			"js".parse::<RuntimeType>().unwrap(),
			RuntimeType::TypeScript
		);
		assert_eq!(
			"deno".parse::<RuntimeType>().unwrap(),
			RuntimeType::TypeScript
		);
		assert!("unknown".parse::<RuntimeType>().is_err());
	}

	#[test]
	fn test_runtime_type_from_str_case_insensitive() {
		assert_eq!(
			"STATIC".parse::<RuntimeType>().unwrap(),
			RuntimeType::Static
		);
		assert_eq!(
			"TypeScript".parse::<RuntimeType>().unwrap(),
			RuntimeType::TypeScript
		);
		assert_eq!("WASM".parse::<RuntimeType>().unwrap(), RuntimeType::Wasm);
	}

	#[test]
	fn test_runtime_type_capabilities() {
		assert!(!RuntimeType::Static.supports_hmr());
		assert!(!RuntimeType::Wasm.supports_hmr());
		assert!(RuntimeType::TypeScript.supports_hmr());

		assert!(!RuntimeType::Static.supports_ssr());
		assert!(!RuntimeType::Wasm.supports_ssr());
		assert!(RuntimeType::TypeScript.supports_ssr());
	}

	#[test]
	fn test_runtime_type_clone() {
		let rt = RuntimeType::Static;
		let cloned = rt;
		assert_eq!(rt, cloned);
	}

	#[test]
	fn test_runtime_type_eq() {
		assert_eq!(RuntimeType::Static, RuntimeType::Static);
		assert_ne!(RuntimeType::Static, RuntimeType::Wasm);
		assert_ne!(RuntimeType::Wasm, RuntimeType::TypeScript);
	}

	#[test]
	fn test_runtime_type_hash() {
		use std::collections::HashSet;
		let mut set = HashSet::new();
		set.insert(RuntimeType::Static);
		set.insert(RuntimeType::Wasm);
		set.insert(RuntimeType::TypeScript);
		assert_eq!(set.len(), 3);

		// Duplicate should not increase size
		set.insert(RuntimeType::Static);
		assert_eq!(set.len(), 3);
	}

	#[test]
	fn test_runtime_type_serde() {
		let rt = RuntimeType::Wasm;
		let json = serde_json::to_string(&rt).unwrap();
		assert_eq!(json, r#""wasm""#);

		let deserialized: RuntimeType = serde_json::from_str(&json).unwrap();
		assert_eq!(deserialized, rt);
	}

	#[test]
	fn test_runtime_type_serde_all_variants() {
		// Verify all variants serialize correctly
		// Note: serde rename_all = "snake_case" converts TypeScript -> type_script
		assert_eq!(
			serde_json::to_string(&RuntimeType::Static).unwrap(),
			r#""static""#
		);
		assert_eq!(
			serde_json::to_string(&RuntimeType::Wasm).unwrap(),
			r#""wasm""#
		);
		assert_eq!(
			serde_json::to_string(&RuntimeType::TypeScript).unwrap(),
			r#""type_script""#
		);
	}

	#[test]
	fn test_runtime_limits_default() {
		let limits = RuntimeLimits::default();
		assert_eq!(limits.memory_limit_bytes, 128 * 1024 * 1024);
		assert_eq!(limits.timeout, std::time::Duration::from_secs(30));
		assert_eq!(limits.fuel_limit, Some(100_000_000));
	}

	#[test]
	fn test_runtime_limits_clone() {
		let limits = RuntimeLimits::default();
		let cloned = limits.clone();
		assert_eq!(limits.memory_limit_bytes, cloned.memory_limit_bytes);
		assert_eq!(limits.timeout, cloned.timeout);
		assert_eq!(limits.fuel_limit, cloned.fuel_limit);
	}

	#[test]
	fn test_runtime_limits_serde() {
		let limits = RuntimeLimits {
			memory_limit_bytes: 1024 * 1024,
			timeout: std::time::Duration::from_secs(10),
			fuel_limit: Some(50_000_000),
		};
		let json = serde_json::to_string(&limits).unwrap();
		let deserialized: RuntimeLimits = serde_json::from_str(&json).unwrap();
		assert_eq!(deserialized.memory_limit_bytes, 1024 * 1024);
		assert_eq!(deserialized.timeout, std::time::Duration::from_secs(10));
		assert_eq!(deserialized.fuel_limit, Some(50_000_000));
	}

	#[test]
	fn test_runtime_limits_debug() {
		let limits = RuntimeLimits::default();
		let debug_str = format!("{:?}", limits);
		assert!(debug_str.contains("RuntimeLimits"));
		assert!(debug_str.contains("memory_limit_bytes"));
	}

	#[test]
	fn test_runtime_error_function_not_found() {
		let err = RuntimeError::FunctionNotFound("test_fn".to_string());
		assert_eq!(err.to_string(), "function not found: test_fn");
	}

	#[test]
	fn test_runtime_error_execution_error() {
		let err = RuntimeError::ExecutionError("panic occurred".to_string());
		assert_eq!(err.to_string(), "execution error: panic occurred");
	}

	#[test]
	fn test_runtime_error_capability_not_supported() {
		let err = RuntimeError::CapabilityNotSupported("models".to_string());
		assert_eq!(err.to_string(), "capability not supported: models");
	}

	#[test]
	fn test_runtime_error_timeout() {
		let err = RuntimeError::Timeout(std::time::Duration::from_secs(30));
		assert_eq!(err.to_string(), "execution timeout after 30s");
	}

	#[test]
	fn test_runtime_error_memory_limit_exceeded() {
		let err = RuntimeError::MemoryLimitExceeded {
			used: 200 * 1024 * 1024,
			limit: 128 * 1024 * 1024,
		};
		let msg = err.to_string();
		assert!(msg.contains("memory limit exceeded"));
		assert!(msg.contains("209715200 bytes"));
		assert!(msg.contains("134217728 bytes"));
	}

	#[test]
	fn test_runtime_error_module_load_error() {
		let err = RuntimeError::ModuleLoadError("file not found".to_string());
		assert_eq!(err.to_string(), "module load error: file not found");
	}

	#[test]
	fn test_runtime_error_serialization_error() {
		let err = RuntimeError::SerializationError("invalid json".to_string());
		assert_eq!(err.to_string(), "serialization error: invalid json");
	}

	#[test]
	fn test_runtime_error_internal() {
		let err = RuntimeError::Internal("unexpected state".to_string());
		assert_eq!(err.to_string(), "internal error: unexpected state");
	}

	#[test]
	fn test_runtime_error_to_plugin_error() {
		let runtime_err = RuntimeError::FunctionNotFound("missing".to_string());
		let plugin_err: PluginError = runtime_err.into();
		let msg = plugin_err.to_string();
		assert!(msg.contains("function not found: missing"));
	}

	// =========================================================================
	// RuntimeLimits from_tier and Builder Tests
	// =========================================================================

	#[test]
	fn test_runtime_limits_from_tier_standard() {
		let limits = RuntimeLimits::from_tier(PluginTier::Standard);
		assert_eq!(limits.memory_limit_bytes, 128 * 1024 * 1024);
		assert_eq!(limits.timeout, std::time::Duration::from_secs(30));
		assert_eq!(limits.fuel_limit, Some(100_000_000));
	}

	#[test]
	fn test_runtime_limits_from_tier_premium() {
		let limits = RuntimeLimits::from_tier(PluginTier::Premium);
		assert_eq!(limits.memory_limit_bytes, 512 * 1024 * 1024);
		assert_eq!(limits.timeout, std::time::Duration::from_secs(60));
		assert_eq!(limits.fuel_limit, Some(500_000_000));
	}

	#[test]
	fn test_runtime_limits_from_tier_enterprise() {
		let limits = RuntimeLimits::from_tier(PluginTier::Enterprise);
		assert_eq!(limits.memory_limit_bytes, 1024 * 1024 * 1024);
		assert_eq!(limits.timeout, std::time::Duration::from_secs(120));
		assert_eq!(limits.fuel_limit, Some(1_000_000_000));
	}

	#[test]
	fn test_runtime_limits_from_plugin_tier_trait() {
		let tier = PluginTier::Premium;
		let limits: RuntimeLimits = tier.into();
		assert_eq!(limits.memory_limit_bytes, 512 * 1024 * 1024);
	}

	#[test]
	fn test_runtime_limits_builder_defaults() {
		let limits = RuntimeLimits::builder().build();
		assert_eq!(limits.memory_limit_bytes, 128 * 1024 * 1024);
		assert_eq!(limits.timeout, std::time::Duration::from_secs(30));
		assert_eq!(limits.fuel_limit, Some(100_000_000));
	}

	#[test]
	fn test_runtime_limits_builder_memory_bytes() {
		let limits = RuntimeLimits::builder()
			.memory_limit_bytes(256 * 1024 * 1024)
			.build();
		assert_eq!(limits.memory_limit_bytes, 256 * 1024 * 1024);
	}

	#[test]
	fn test_runtime_limits_builder_memory_mb() {
		let limits = RuntimeLimits::builder().memory_limit_mb(256).build();
		assert_eq!(limits.memory_limit_bytes, 256 * 1024 * 1024);
	}

	#[test]
	fn test_runtime_limits_builder_timeout() {
		let limits = RuntimeLimits::builder()
			.timeout(std::time::Duration::from_secs(60))
			.build();
		assert_eq!(limits.timeout, std::time::Duration::from_secs(60));
	}

	#[test]
	fn test_runtime_limits_builder_timeout_secs() {
		let limits = RuntimeLimits::builder().timeout_secs(45).build();
		assert_eq!(limits.timeout, std::time::Duration::from_secs(45));
	}

	#[test]
	fn test_runtime_limits_builder_fuel() {
		let limits = RuntimeLimits::builder().fuel_limit(200_000_000).build();
		assert_eq!(limits.fuel_limit, Some(200_000_000));
	}

	#[test]
	fn test_runtime_limits_builder_no_fuel() {
		let limits = RuntimeLimits::builder().no_fuel_limit().build();
		assert_eq!(limits.fuel_limit, None);
	}

	#[test]
	fn test_runtime_limits_builder_combined() {
		let limits = RuntimeLimits::builder()
			.memory_limit_mb(512)
			.timeout_secs(90)
			.fuel_limit(300_000_000)
			.build();
		assert_eq!(limits.memory_limit_bytes, 512 * 1024 * 1024);
		assert_eq!(limits.timeout, std::time::Duration::from_secs(90));
		assert_eq!(limits.fuel_limit, Some(300_000_000));
	}

	#[test]
	fn test_runtime_limits_builder_from_tier() {
		let limits = RuntimeLimits::builder()
			.timeout_secs(90) // Override timeout only
			.build_from_tier(PluginTier::Premium);
		// Memory from Premium tier
		assert_eq!(limits.memory_limit_bytes, 512 * 1024 * 1024);
		// Timeout overridden
		assert_eq!(limits.timeout, std::time::Duration::from_secs(90));
		// Fuel from Premium tier
		assert_eq!(limits.fuel_limit, Some(500_000_000));
	}
}
