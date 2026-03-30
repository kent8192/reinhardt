//! Dentdelion - Plugin System for Reinhardt Framework
//!
//! Dentdelion (dent de lion = lion's tooth = dandelion) is a plugin system
//! that makes it easy to create, distribute, and install plugins for the
//! Reinhardt web framework.
//!
//! # Features
//!
//! - **Static Plugins**: Rust crates compiled with your application
//! - **WASM Plugins**: Dynamic plugins loaded at runtime (with `wasm` feature)
//! - **TypeScript Plugins**: TypeScript/JavaScript plugins via deno_core (with `ts` feature)
//! - **Capability System**: Fine-grained control over what plugins can do
//! - **CLI Management**: Install and manage plugins via `reinhardt plugin` commands
//! - **Multi-Runtime**: Unified interface for Static, WASM, and TypeScript runtimes
//!
//! # Naming Convention
//!
//! Plugin names should follow the `xxx-delion` pattern:
//! - `auth-delion` - Authentication plugin
//! - `rate-limit-delion` - Rate limiting plugin
//! - `analytics-delion` - Analytics plugin
//!
//! # Quick Start
//!
//! ## Creating a Plugin
//!
//! ```rust
//! use reinhardt_dentdelion::prelude::*;
//! use std::sync::Arc;
//!
//! struct MyPlugin {
//!     metadata: PluginMetadata,
//! }
//!
//! impl MyPlugin {
//!     pub fn new() -> Self {
//!         Self {
//!             metadata: PluginMetadata::builder("my-delion", "1.0.0")
//!                 .description("My awesome plugin")
//!                 .provides(PluginCapability::Middleware)
//!                 .build()
//!                 .unwrap(),
//!         }
//!     }
//! }
//!
//! impl Plugin for MyPlugin {
//!     fn metadata(&self) -> &PluginMetadata {
//!         &self.metadata
//!     }
//!
//!     fn capabilities(&self) -> &[Capability] {
//!         &[Capability::Core(PluginCapability::Middleware)]
//!     }
//! }
//!
//! // Register the plugin for automatic discovery
//! register_plugin!(|| Arc::new(MyPlugin::new()));
//! ```
//!
//! ## Using the Registry
//!
//! ```rust
//! use reinhardt_dentdelion::prelude::*;
//! use std::sync::Arc;
//!
//! # struct MyPlugin { metadata: PluginMetadata }
//! # impl MyPlugin {
//! #     fn new() -> Self {
//! #         Self {
//! #             metadata: PluginMetadata::builder("my-delion", "1.0.0")
//! #                 .build().unwrap(),
//! #         }
//! #     }
//! # }
//! # impl Plugin for MyPlugin {
//! #     fn metadata(&self) -> &PluginMetadata { &self.metadata }
//! #     fn capabilities(&self) -> &[Capability] { &[] }
//! # }
//! let registry = PluginRegistry::new();
//!
//! // Register plugins
//! registry.register(Arc::new(MyPlugin::new())).unwrap();
//!
//! // Validate dependencies
//! registry.validate_dependencies().unwrap();
//!
//! // Get enable order
//! let order = registry.get_enable_order().unwrap();
//! ```
//!
//! ## Project Manifest (dentdelion.toml)
//!
//! ```toml
//! [dentdelion]
//! format_version = "1.0"
//! wasm_dir = ".dentdelion/plugins"
//!
//! [[plugins]]
//! name = "auth-delion"
//! type = "static"
//! version = "1.0.0"
//! enabled = true
//!
//! [plugin_config.auth-delion]
//! algorithm = "HS256"
//! ```
//!
//! # Feature Flags
//!
//! - `default` - Core plugin system only
//! - `wasm` - WASM plugin support with Component Model (requires wasmtime 39.x)
//! - `ts` - TypeScript plugin support via deno_core (V8 engine)
//! - `cli` - CLI support for crates.io integration
//! - `full` - All features enabled (wasm + ts + cli)
//!
//! # WASM Plugin Support
//!
//! When the `wasm` feature is enabled, plugins can be loaded dynamically at runtime
//! from WebAssembly Component Model files (.wasm).
//!
//! ## Key Features
//!
//! - **Component Model**: Uses WebAssembly Interface Types (WIT) for type-safe interfaces
//! - **Sandboxed Execution**: Memory limits, timeouts, and fuel-based CPU metering
//! - **Host API**: Config, logging, services, HTTP client, and database access
//! - **Capability-Based Security**: Fine-grained permission checks for sensitive operations
//!
//! ## Requirements
//!
//! - Plugins must implement the `dentdelion-plugin` WIT world (see `wit/dentdelion.wit`)
//! - Use `wit-bindgen` or `cargo-component` for code generation
//! - Data serialized with MessagePack for WASM boundary crossing
//!
//! ## Runtime Configuration
//!
//! ```ignore
//! use reinhardt_dentdelion::wasm::{WasmRuntime, WasmRuntimeConfigBuilder};
//! use std::time::Duration;
//!
//! let config = WasmRuntimeConfigBuilder::new()
//!     .memory_limit_mb(128)
//!     .timeout(Duration::from_secs(30))
//!     .fuel_metering(true)
//!     .initial_fuel(100_000_000)
//!     .build();
//!
//! let runtime = WasmRuntime::new(config)?;
//! ```
//!
//! ## Capabilities
//!
//! Two special capabilities are available for WASM plugins:
//! - `NetworkAccess` - Required for `http_get`/`http_post` host functions
//! - `DatabaseAccess` - Required for `db_query`/`db_execute` host functions
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────┐
//! │        PluginRegistry            │
//! │    - Plugin registration         │
//! │    - Dependency resolution       │
//! │    - Lifecycle management        │
//! └────────────────┬─────────────────┘
//!                  │
//!     ┌────────────┼────────────┐
//!     │            │            │
//! ┌───▼──┐     ┌───▼──┐     ┌───▼────┐
//! │Static│     │ WASM │     │Manifest│
//! │Plugin│     │Plugin│     │ Parser │
//! └──────┘     └──────┘     └────────┘
//! ```

#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

pub mod capability;
pub mod context;
pub mod error;
pub mod installer;
pub mod manifest;
pub mod metadata;
pub mod plugin;
pub mod registry;
pub mod runtime;

#[cfg(feature = "cli")]
pub mod crates_io;

#[cfg(feature = "wasm")]
pub mod wasm;

/// Re-export commonly used types.
pub mod prelude {
	pub use crate::capability::{Capability, PluginCapability, PluginTier, TrustLevel};
	pub use crate::context::{PluginContext, PluginContextBuilder};
	pub use crate::error::{PluginError, PluginResult, PluginState};
	pub use crate::installer::PluginInstaller;
	pub use crate::manifest::{
		InstalledPlugin, MANIFEST_FILENAME, PluginType, ProjectManifest, WasmPluginConfig,
	};
	pub use crate::metadata::{PluginDependency, PluginMetadata, PluginMetadataBuilder};
	pub use crate::plugin::{
		ArcPlugin, ArcPluginLifecycle, BoxedPlugin, Plugin, PluginFactory, PluginLifecycle,
		PluginRegistration,
	};
	pub use crate::register_plugin;
	pub use crate::registry::PluginRegistry;
	pub use crate::runtime::{
		ArcRuntime, BoxedRuntime, PluginRuntime, RuntimeError, RuntimeLimits, RuntimeLimitsBuilder,
		RuntimeType,
	};

	#[cfg(feature = "cli")]
	pub use crate::crates_io::CratesIoClient;

	#[cfg(feature = "wasm")]
	pub use crate::wasm::{
		ColumnDef, ColumnType, Event, EventBus, IndexDef, ModelRegistry, ModelSchema,
		SharedEventBus, SharedModelRegistry, SqlMigration,
	};

	pub use async_trait::async_trait;
}

// Re-export inventory for plugin registration
pub use inventory;

#[cfg(test)]
mod tests {
	use super::prelude::*;
	use std::sync::Arc;

	struct TestPlugin {
		metadata: PluginMetadata,
		capabilities: Vec<Capability>,
	}

	impl TestPlugin {
		fn new() -> Self {
			Self {
				metadata: PluginMetadata::builder("test-delion", "1.0.0")
					.description("Test plugin for integration tests")
					.author("Test Author")
					.license("MIT")
					.provides(PluginCapability::Middleware)
					.build()
					.unwrap(),
				capabilities: vec![Capability::Core(PluginCapability::Middleware)],
			}
		}
	}

	impl Plugin for TestPlugin {
		fn metadata(&self) -> &PluginMetadata {
			&self.metadata
		}

		fn capabilities(&self) -> &[Capability] {
			&self.capabilities
		}
	}

	#[async_trait]
	impl PluginLifecycle for TestPlugin {
		async fn on_load(&self, _ctx: &PluginContext) -> Result<(), PluginError> {
			tracing::info!("TestPlugin loaded");
			Ok(())
		}

		async fn on_enable(&self, _ctx: &PluginContext) -> Result<(), PluginError> {
			tracing::info!("TestPlugin enabled");
			Ok(())
		}
	}

	#[test]
	fn test_integration() {
		let registry = PluginRegistry::new();
		let plugin = Arc::new(TestPlugin::new());

		registry.register(plugin.clone()).unwrap();

		assert!(registry.is_registered("test-delion"));
		assert_eq!(registry.len(), 1);

		let retrieved = registry.get("test-delion").unwrap();
		assert_eq!(retrieved.name(), "test-delion");
		assert_eq!(retrieved.version().to_string(), "1.0.0");
	}

	#[test]
	fn test_capability_query() {
		let registry = PluginRegistry::new();
		let plugin = Arc::new(TestPlugin::new());

		registry.register(plugin).unwrap();
		registry
			.set_state("test-delion", PluginState::Enabled)
			.unwrap();

		let providers =
			registry.get_capability_providers(&Capability::Core(PluginCapability::Middleware));
		assert_eq!(providers.len(), 1);
	}
}
