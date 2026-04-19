//! Core plugin traits.
//!
//! This module defines the fundamental traits that all plugins must implement.
//!
//! # Architecture
//!
//! - [`Plugin`]: Base trait providing metadata and capability information
//! - [`PluginLifecycle`]: Lifecycle hooks for plugin initialization and cleanup
//!
//! # Example
//!
//! ```rust
//! use reinhardt_dentdelion::prelude::*;
//!
//! struct MyPlugin {
//!     metadata: PluginMetadata,
//! }
//!
//! # impl MyPlugin {
//! #     fn new() -> Self {
//! #         Self {
//! #             metadata: PluginMetadata::builder("my-delion", "1.0.0")
//! #                 .build().unwrap(),
//! #         }
//! #     }
//! # }
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
//! #[async_trait]
//! impl PluginLifecycle for MyPlugin {
//!     async fn on_enable(&self, ctx: &PluginContext) -> Result<(), PluginError> {
//!         // Register middleware, services, etc.
//!         Ok(())
//!     }
//! }
//! ```

use crate::capability::Capability;
use crate::context::PluginContext;
use crate::error::PluginError;
use crate::metadata::PluginMetadata;
use async_trait::async_trait;
use std::sync::Arc;

/// Core plugin trait - all plugins must implement this.
///
/// This is the fundamental interface that all plugins (static and dynamic)
/// must implement. It provides metadata and capability discovery.
///
/// # Thread Safety
///
/// Plugins must be `Send + Sync` to support concurrent access from
/// multiple request handlers.
pub trait Plugin: Send + Sync {
	/// Returns the plugin's metadata (name, version, author, etc.).
	fn metadata(&self) -> &PluginMetadata;

	/// Returns the capabilities this plugin provides.
	///
	/// Only declared capabilities will be activated at runtime.
	/// This allows the framework to efficiently manage plugin features
	/// and ensure type safety.
	fn capabilities(&self) -> &[Capability];

	/// Checks if this plugin provides a specific capability.
	fn has_capability(&self, capability: &Capability) -> bool {
		self.capabilities().contains(capability)
	}

	/// Returns whether this is a dynamic (WASM) plugin.
	///
	/// Static plugins return `false` (default), WASM plugins return `true`.
	fn is_dynamic(&self) -> bool {
		false
	}

	/// Returns the plugin's name for convenience.
	fn name(&self) -> &str {
		&self.metadata().name
	}

	/// Returns the plugin's version for convenience.
	fn version(&self) -> &semver::Version {
		&self.metadata().version
	}
}

/// Plugin lifecycle hooks.
///
/// Plugins implement this trait to receive lifecycle events.
/// All methods have default no-op implementations, allowing plugins
/// to only implement the hooks they need.
///
/// # Lifecycle Phases
///
/// 1. **Load**: Plugin is loaded into memory (resources allocated)
/// 2. **Enable**: Plugin is activated (services registered)
/// 3. **Disable**: Plugin is deactivated (services unregistered)
/// 4. **Unload**: Plugin is removed from memory (resources released)
///
/// This pattern matches Reinhardt's existing `AppReadyHook` lifecycle.
#[async_trait]
pub trait PluginLifecycle: Plugin {
	/// Called when the plugin is being loaded.
	///
	/// Use this to initialize resources, validate configuration,
	/// and prepare the plugin for activation.
	///
	/// # Arguments
	///
	/// * `ctx` - The plugin context providing access to framework services
	///
	/// # Errors
	///
	/// Return an error if the plugin cannot be loaded. The plugin will
	/// transition to the `Failed` state.
	async fn on_load(&self, _ctx: &PluginContext) -> Result<(), PluginError> {
		Ok(())
	}

	/// Called when the plugin is enabled.
	///
	/// At this point, all dependencies are already enabled.
	/// Register services, middleware, and other capabilities here.
	///
	/// # Arguments
	///
	/// * `ctx` - The plugin context providing access to framework services
	///
	/// # Errors
	///
	/// Return an error if the plugin cannot be enabled. The plugin will
	/// transition to the `Failed` state.
	async fn on_enable(&self, _ctx: &PluginContext) -> Result<(), PluginError> {
		Ok(())
	}

	/// Called when the plugin is disabled.
	///
	/// Clean up active resources while maintaining state for potential re-enable.
	/// Unregister services, middleware, etc.
	///
	/// # Arguments
	///
	/// * `ctx` - The plugin context providing access to framework services
	///
	/// # Errors
	///
	/// Return an error if cleanup fails. The error will be logged but
	/// the plugin will still transition to the `Disabled` state.
	async fn on_disable(&self, _ctx: &PluginContext) -> Result<(), PluginError> {
		Ok(())
	}

	/// Called when the plugin is unloaded.
	///
	/// Final cleanup - release all resources.
	///
	/// # Arguments
	///
	/// * `ctx` - The plugin context providing access to framework services
	///
	/// # Errors
	///
	/// Return an error if cleanup fails. The error will be logged but
	/// the plugin will still be unloaded.
	async fn on_unload(&self, _ctx: &PluginContext) -> Result<(), PluginError> {
		Ok(())
	}
}

/// Type alias for a boxed plugin.
pub type BoxedPlugin = Box<dyn Plugin>;

/// Type alias for an Arc-wrapped plugin.
pub type ArcPlugin = Arc<dyn Plugin>;

/// Type alias for a boxed plugin with lifecycle support.
pub type BoxedPluginLifecycle = Box<dyn PluginLifecycle>;

/// Type alias for an Arc-wrapped plugin with lifecycle support.
pub type ArcPluginLifecycle = Arc<dyn PluginLifecycle>;

/// Plugin factory function type.
///
/// Used for compile-time registration of static plugins via `inventory`.
pub type PluginFactory = fn() -> ArcPlugin;

// Distributed slice for static plugin registration.
// Static plugins register themselves here at compile time using
// the `inventory` crate.
inventory::collect!(PluginRegistration);

/// Plugin registration entry for compile-time discovery.
pub struct PluginRegistration {
	/// Factory function to create the plugin.
	pub factory: PluginFactory,
}

impl PluginRegistration {
	/// Creates a new plugin registration.
	pub const fn new(factory: PluginFactory) -> Self {
		Self { factory }
	}
}

/// Returns an iterator over all registered static plugins.
pub fn registered_plugins() -> impl Iterator<Item = ArcPlugin> {
	inventory::iter::<PluginRegistration>
		.into_iter()
		.map(|reg| (reg.factory)())
}

/// Macro for registering a static plugin.
///
/// This macro registers a plugin factory function with the `inventory` crate,
/// enabling automatic discovery of static plugins at compile time.
///
/// # Example
///
/// ```ignore
/// use reinhardt_dentdelion::register_plugin;
///
/// register_plugin!(|| Arc::new(MyPlugin::new()));
/// ```
#[macro_export]
macro_rules! register_plugin {
	($factory:expr) => {
		::inventory::submit! {
			$crate::plugin::PluginRegistration::new($factory)
		}
	};
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::capability::PluginCapability;
	use crate::metadata::PluginMetadata;

	struct TestPlugin {
		metadata: PluginMetadata,
		capabilities: Vec<Capability>,
	}

	impl TestPlugin {
		fn new() -> Self {
			Self {
				metadata: PluginMetadata::builder("test-delion", "1.0.0")
					.description("Test plugin")
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
	impl PluginLifecycle for TestPlugin {}

	#[test]
	fn test_plugin_trait() {
		let plugin = TestPlugin::new();

		assert_eq!(plugin.name(), "test-delion");
		assert_eq!(plugin.version().to_string(), "1.0.0");
		assert!(!plugin.is_dynamic());
		assert!(plugin.has_capability(&Capability::Core(PluginCapability::Middleware)));
		assert!(!plugin.has_capability(&Capability::Core(PluginCapability::Models)));
	}
}
