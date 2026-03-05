//! WASM Plugin Instance
//!
//! This module provides the `WasmPluginInstance` struct which represents
//! a loaded and instantiated WASM plugin.

use crate::capability::Capability;
use crate::context::PluginContext;
use crate::error::{PluginError, PluginResult, PluginState};
use crate::manifest::WasmPluginConfig;
use crate::metadata::PluginMetadata;
use crate::plugin::{Plugin, PluginLifecycle};

use async_trait::async_trait;
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wasmtime::Store;
use wasmtime::component::Component;

use super::host::HostState;
use super::runtime::WasmRuntime;
use super::types::WitCapability;

/// A loaded WASM plugin instance.
///
/// This struct represents a WASM plugin that has been loaded and is ready
/// to execute. It implements the `Plugin` and `PluginLifecycle` traits.
pub struct WasmPluginInstance {
	/// Plugin name.
	name: String,
	/// Path to the WASM file.
	wasm_path: PathBuf,
	/// Plugin metadata (initialized at creation).
	metadata: PluginMetadata,
	/// Plugin capabilities (initialized at creation).
	capabilities: Vec<Capability>,
	/// Current plugin state.
	state: RwLock<PluginState>,
	/// The compiled WASM component.
	#[allow(dead_code)] // Used for future WASM function calls
	component: Component,
	/// Host state for this instance.
	host_state: RwLock<HostState>,
	/// Reference to the runtime.
	#[allow(dead_code)] // Used for future store creation
	runtime: Arc<WasmRuntime>,
	/// WASM-specific configuration.
	wasm_config: Option<WasmPluginConfig>,
}

impl WasmPluginInstance {
	/// Create a new WASM plugin instance.
	///
	/// # Arguments
	///
	/// * `name` - Plugin name
	/// * `wasm_path` - Path to the WASM file
	/// * `component` - Compiled WASM component
	/// * `host_state` - Host state for this instance
	/// * `runtime` - Reference to the WASM runtime
	/// * `wasm_config` - Optional WASM-specific configuration
	pub(crate) fn new(
		name: String,
		wasm_path: PathBuf,
		component: Component,
		host_state: HostState,
		runtime: Arc<WasmRuntime>,
		wasm_config: Option<WasmPluginConfig>,
	) -> PluginResult<Self> {
		// Initialize metadata
		let metadata = PluginMetadata::builder(&name, "0.1.0")
			.description(format!("WASM plugin: {}", name))
			.build()?;

		// Initialize capabilities from config
		let capabilities = if let Some(ref config) = wasm_config {
			config
				.capabilities
				.iter()
				.map(|s| match s.to_lowercase().as_str() {
					"middleware" => WitCapability::Middleware,
					"commands" => WitCapability::Commands,
					"view-sets" | "viewsets" => WitCapability::ViewSets,
					"signals" => WitCapability::Signals,
					"services" => WitCapability::Services,
					"auth" => WitCapability::Auth,
					"templates" => WitCapability::Templates,
					"static-files" | "staticfiles" => WitCapability::StaticFiles,
					"routing" => WitCapability::Routing,
					"signal-receivers" | "signalreceivers" => WitCapability::SignalReceivers,
					"handlers" => WitCapability::Handlers,
					other => WitCapability::Custom(other.to_string()),
				})
				.map(|wc| wc.to_capability())
				.collect()
		} else {
			Vec::new()
		};

		Ok(Self {
			name,
			wasm_path,
			metadata,
			capabilities,
			state: RwLock::new(PluginState::Registered),
			component,
			host_state: RwLock::new(host_state),
			runtime,
			wasm_config,
		})
	}

	/// Get the path to the WASM file.
	pub fn wasm_path(&self) -> &Path {
		&self.wasm_path
	}

	/// Get the current plugin state.
	pub fn state(&self) -> PluginState {
		*self.state.read()
	}

	/// Get the WASM-specific configuration.
	pub fn wasm_config(&self) -> Option<&WasmPluginConfig> {
		self.wasm_config.as_ref()
	}

	/// Get a reference to the compiled component.
	pub fn component(&self) -> &Component {
		&self.component
	}

	/// Create a new store for executing plugin functions.
	#[allow(dead_code)] // Used for future WASM function calls
	fn create_store(&self) -> Store<HostState> {
		let host_state = self.host_state.read().clone();
		self.runtime.create_store(host_state)
	}

	/// Call the plugin's on_load function.
	async fn call_on_load(&self, config: &[u8]) -> PluginResult<()> {
		use crate::wasm::runtime::DentdelionPlugin;

		let mut store = self.create_store();

		// Instantiate the WASM component
		let plugin =
			DentdelionPlugin::instantiate_async(&mut store, &self.component, self.runtime.linker())
				.await
				.map_err(|e| {
					PluginError::WasmExecutionError(format!(
						"Failed to instantiate plugin {}: {}",
						self.name, e
					))
				})?;

		// Call the on_load exported function
		plugin
			.reinhardt_dentdelion_plugin()
			.call_on_load(&mut store, config)
			.await
			.map_err(|e| {
				PluginError::WasmExecutionError(format!(
					"WASM trap in on_load for {}: {}",
					self.name, e
				))
			})?
			.map_err(|e| PluginError::LifecycleError {
				plugin: self.name.clone(),
				phase: "on_load".to_string(),
				message: format!("[{}] {}", e.code, e.message),
			})?;

		tracing::info!("Successfully called on_load for plugin: {}", self.name);
		Ok(())
	}

	/// Call the plugin's on_enable function.
	async fn call_on_enable(&self) -> PluginResult<()> {
		use crate::wasm::runtime::DentdelionPlugin;

		let mut store = self.create_store();

		let plugin =
			DentdelionPlugin::instantiate_async(&mut store, &self.component, self.runtime.linker())
				.await
				.map_err(|e| {
					PluginError::WasmExecutionError(format!(
						"Failed to instantiate plugin {}: {}",
						self.name, e
					))
				})?;

		plugin
			.reinhardt_dentdelion_plugin()
			.call_on_enable(&mut store)
			.await
			.map_err(|e| {
				PluginError::WasmExecutionError(format!(
					"WASM trap in on_enable for {}: {}",
					self.name, e
				))
			})?
			.map_err(|e| PluginError::LifecycleError {
				plugin: self.name.clone(),
				phase: "on_enable".to_string(),
				message: format!("[{}] {}", e.code, e.message),
			})?;

		tracing::info!("Successfully called on_enable for plugin: {}", self.name);
		Ok(())
	}

	/// Call the plugin's on_disable function.
	async fn call_on_disable(&self) -> PluginResult<()> {
		use crate::wasm::runtime::DentdelionPlugin;

		let mut store = self.create_store();

		let plugin =
			DentdelionPlugin::instantiate_async(&mut store, &self.component, self.runtime.linker())
				.await
				.map_err(|e| {
					PluginError::WasmExecutionError(format!(
						"Failed to instantiate plugin {}: {}",
						self.name, e
					))
				})?;

		plugin
			.reinhardt_dentdelion_plugin()
			.call_on_disable(&mut store)
			.await
			.map_err(|e| {
				PluginError::WasmExecutionError(format!(
					"WASM trap in on_disable for {}: {}",
					self.name, e
				))
			})?
			.map_err(|e| PluginError::LifecycleError {
				plugin: self.name.clone(),
				phase: "on_disable".to_string(),
				message: format!("[{}] {}", e.code, e.message),
			})?;

		tracing::info!("Successfully called on_disable for plugin: {}", self.name);
		Ok(())
	}

	/// Call the plugin's on_unload function.
	async fn call_on_unload(&self) -> PluginResult<()> {
		use crate::wasm::runtime::DentdelionPlugin;

		let mut store = self.create_store();

		let plugin =
			DentdelionPlugin::instantiate_async(&mut store, &self.component, self.runtime.linker())
				.await
				.map_err(|e| {
					PluginError::WasmExecutionError(format!(
						"Failed to instantiate plugin {}: {}",
						self.name, e
					))
				})?;

		plugin
			.reinhardt_dentdelion_plugin()
			.call_on_unload(&mut store)
			.await
			.map_err(|e| {
				PluginError::WasmExecutionError(format!(
					"WASM trap in on_unload for {}: {}",
					self.name, e
				))
			})?
			.map_err(|e| PluginError::LifecycleError {
				plugin: self.name.clone(),
				phase: "on_unload".to_string(),
				message: format!("[{}] {}", e.code, e.message),
			})?;

		tracing::info!("Successfully called on_unload for plugin: {}", self.name);
		Ok(())
	}
}

// Clone implementation for HostState is defined in host.rs where it has
// access to private fields.

impl Plugin for WasmPluginInstance {
	fn metadata(&self) -> &PluginMetadata {
		&self.metadata
	}

	fn capabilities(&self) -> &[Capability] {
		&self.capabilities
	}

	fn is_dynamic(&self) -> bool {
		true
	}
}

#[async_trait]
impl PluginLifecycle for WasmPluginInstance {
	async fn on_load(&self, _ctx: &PluginContext) -> Result<(), PluginError> {
		// Validate state transition
		let current = self.state();
		if current != PluginState::Registered {
			return Err(PluginError::InvalidStateTransition {
				plugin: self.name.clone(),
				from: current,
				to: PluginState::Loaded,
			});
		}

		// Serialize config for the plugin from host state
		let config = {
			let host = self.host_state.read();
			let config_map = host.get_config_all();
			rmp_serde::to_vec(&config_map).map_err(|e| {
				PluginError::ConfigError(format!("failed to serialize plugin config: {}", e))
			})?
		};

		// Call the plugin's on_load
		self.call_on_load(&config).await?;

		// Update state
		*self.state.write() = PluginState::Loaded;
		tracing::info!("WASM plugin loaded: {}", self.name);

		Ok(())
	}

	async fn on_enable(&self, _ctx: &PluginContext) -> Result<(), PluginError> {
		// Validate state transition
		let current = self.state();
		if current != PluginState::Loaded && current != PluginState::Disabled {
			return Err(PluginError::InvalidStateTransition {
				plugin: self.name.clone(),
				from: current,
				to: PluginState::Enabled,
			});
		}

		// Call the plugin's on_enable
		self.call_on_enable().await?;

		// Update state
		*self.state.write() = PluginState::Enabled;
		tracing::info!("WASM plugin enabled: {}", self.name);

		Ok(())
	}

	async fn on_disable(&self, _ctx: &PluginContext) -> Result<(), PluginError> {
		// Validate state transition
		let current = self.state();
		if current != PluginState::Enabled {
			return Err(PluginError::InvalidStateTransition {
				plugin: self.name.clone(),
				from: current,
				to: PluginState::Disabled,
			});
		}

		// Call the plugin's on_disable
		self.call_on_disable().await?;

		// Update state
		*self.state.write() = PluginState::Disabled;
		tracing::info!("WASM plugin disabled: {}", self.name);

		Ok(())
	}

	async fn on_unload(&self, _ctx: &PluginContext) -> Result<(), PluginError> {
		// Call the plugin's on_unload
		self.call_on_unload().await?;

		// Update state
		*self.state.write() = PluginState::Registered;
		tracing::info!("WASM plugin unloaded: {}", self.name);

		Ok(())
	}
}

impl std::fmt::Debug for WasmPluginInstance {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("WasmPluginInstance")
			.field("name", &self.name)
			.field("wasm_path", &self.wasm_path)
			.field("state", &self.state())
			.field("wasm_config", &self.wasm_config)
			.finish_non_exhaustive()
	}
}

// Integration tests are implemented in tests/wasm_integration.rs
// See tests/fixtures/ for sample WASM plugins (minimal, logging, config)
