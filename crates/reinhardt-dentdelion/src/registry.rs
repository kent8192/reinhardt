//! Plugin registry for managing installed plugins.
//!
//! The registry maintains the state of all registered plugins and manages
//! their lifecycle transitions.

use crate::capability::Capability;
use crate::context::PluginContext;
use crate::error::{PluginError, PluginResult, PluginState};
use crate::plugin::{ArcPlugin, ArcPluginLifecycle, PluginLifecycle};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Plugin registry entry containing plugin and its state.
struct PluginEntry {
	/// The plugin instance.
	plugin: ArcPlugin,
	/// The lifecycle-capable plugin reference (if available).
	/// This allows calling lifecycle hooks without losing type information.
	lifecycle: Option<ArcPluginLifecycle>,
	/// Current lifecycle state.
	state: PluginState,
}

/// Central registry for all plugins.
///
/// The registry manages plugin registration, dependency resolution,
/// and lifecycle state transitions.
///
/// # Thread Safety
///
/// The registry is thread-safe and can be accessed from multiple threads.
pub struct PluginRegistry {
	/// All registered plugins by name.
	plugins: RwLock<HashMap<String, PluginEntry>>,

	/// Capability to plugin mapping.
	capability_map: RwLock<HashMap<Capability, Vec<String>>>,

	/// Dependency graph (plugin -> its dependencies).
	dependency_graph: RwLock<HashMap<String, Vec<String>>>,

	/// Reverse dependency graph (plugin -> plugins that depend on it).
	dependents: RwLock<HashMap<String, Vec<String>>>,
}

impl PluginRegistry {
	/// Creates a new empty registry.
	pub fn new() -> Self {
		Self {
			plugins: RwLock::new(HashMap::new()),
			capability_map: RwLock::new(HashMap::new()),
			dependency_graph: RwLock::new(HashMap::new()),
			dependents: RwLock::new(HashMap::new()),
		}
	}

	/// Registers a plugin.
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - A plugin with the same name but different version is already registered
	pub fn register(&self, plugin: ArcPlugin) -> PluginResult<()> {
		let metadata = plugin.metadata();
		let name = metadata.name.clone();

		// Check for duplicate registration
		{
			let plugins = self.plugins.read();
			if let Some(existing) = plugins.get(&name) {
				let existing_version = existing.plugin.metadata().version.clone();
				if metadata.version != existing_version {
					return Err(PluginError::VersionConflict {
						plugin: name,
						existing: existing_version,
						new: metadata.version.clone(),
					});
				}
				// Already registered with same version
				return Ok(());
			}
		}

		// Register capabilities
		{
			let mut cap_map = self.capability_map.write();
			for capability in plugin.capabilities() {
				cap_map
					.entry(capability.clone())
					.or_default()
					.push(name.clone());
			}
		}

		// Build dependency graph
		{
			let mut deps = self.dependency_graph.write();
			let mut reverse = self.dependents.write();

			for dep in &metadata.dependencies {
				if !dep.optional {
					deps.entry(name.clone()).or_default().push(dep.name.clone());

					reverse
						.entry(dep.name.clone())
						.or_default()
						.push(name.clone());
				}
			}
		}

		// Add plugin entry
		{
			let mut plugins = self.plugins.write();
			plugins.insert(
				name,
				PluginEntry {
					plugin,
					lifecycle: None,
					state: PluginState::Registered,
				},
			);
		}

		Ok(())
	}

	/// Registers a plugin with lifecycle support.
	///
	/// Unlike `register()`, this method preserves the `PluginLifecycle` trait
	/// allowing lifecycle hooks (on_load, on_enable, on_disable, on_unload)
	/// to be called during plugin state transitions.
	pub fn register_with_lifecycle<P>(&self, plugin: Arc<P>) -> PluginResult<()>
	where
		P: PluginLifecycle + 'static,
	{
		let metadata = plugin.metadata();
		let name = metadata.name.clone();

		// Check for duplicate registration
		{
			let plugins = self.plugins.read();
			if let Some(existing) = plugins.get(&name) {
				let existing_version = existing.plugin.metadata().version.clone();
				if metadata.version != existing_version {
					return Err(PluginError::VersionConflict {
						plugin: name,
						existing: existing_version,
						new: metadata.version.clone(),
					});
				}
				// Already registered with same version
				return Ok(());
			}
		}

		// Register capabilities
		{
			let mut cap_map = self.capability_map.write();
			for capability in plugin.capabilities() {
				cap_map
					.entry(capability.clone())
					.or_default()
					.push(name.clone());
			}
		}

		// Build dependency graph
		{
			let mut deps = self.dependency_graph.write();
			let mut reverse = self.dependents.write();

			for dep in &metadata.dependencies {
				if !dep.optional {
					deps.entry(name.clone()).or_default().push(dep.name.clone());

					reverse
						.entry(dep.name.clone())
						.or_default()
						.push(name.clone());
				}
			}
		}

		// Store both ArcPlugin and ArcPluginLifecycle
		let arc_plugin: ArcPlugin = plugin.clone();
		let arc_lifecycle: ArcPluginLifecycle = plugin;

		{
			let mut plugins = self.plugins.write();
			plugins.insert(
				name,
				PluginEntry {
					plugin: arc_plugin,
					lifecycle: Some(arc_lifecycle),
					state: PluginState::Registered,
				},
			);
		}

		Ok(())
	}

	/// Gets a plugin by name.
	pub fn get(&self, name: &str) -> Option<ArcPlugin> {
		self.plugins.read().get(name).map(|e| e.plugin.clone())
	}

	/// Gets the state of a plugin.
	pub fn get_state(&self, name: &str) -> Option<PluginState> {
		self.plugins.read().get(name).map(|e| e.state)
	}

	/// Checks if a plugin is registered.
	pub fn is_registered(&self, name: &str) -> bool {
		self.plugins.read().contains_key(name)
	}

	/// Checks if a plugin is enabled.
	pub fn is_enabled(&self, name: &str) -> bool {
		self.plugins
			.read()
			.get(name)
			.is_some_and(|e| e.state == PluginState::Enabled)
	}

	/// Returns the names of all registered plugins.
	pub fn plugin_names(&self) -> Vec<String> {
		self.plugins.read().keys().cloned().collect()
	}

	/// Returns the number of registered plugins.
	pub fn len(&self) -> usize {
		self.plugins.read().len()
	}

	/// Returns true if no plugins are registered.
	pub fn is_empty(&self) -> bool {
		self.plugins.read().is_empty()
	}

	/// Gets plugins providing a specific capability.
	pub fn get_capability_providers(&self, capability: &Capability) -> Vec<ArcPlugin> {
		let cap_map = self.capability_map.read();
		let plugins = self.plugins.read();

		cap_map
			.get(capability)
			.map(|names| {
				names
					.iter()
					.filter_map(|name| {
						plugins
							.get(name)
							.filter(|e| e.state == PluginState::Enabled)
							.map(|e| e.plugin.clone())
					})
					.collect()
			})
			.unwrap_or_default()
	}

	/// Gets all enabled plugins with a specific capability.
	pub fn enabled_with_capability(&self, capability: &Capability) -> Vec<ArcPlugin> {
		self.get_capability_providers(capability)
	}

	/// Validates all dependencies are satisfied.
	pub fn validate_dependencies(&self) -> PluginResult<()> {
		let plugins = self.plugins.read();
		let deps = self.dependency_graph.read();

		for (plugin_name, dependencies) in deps.iter() {
			for dep_name in dependencies {
				if !plugins.contains_key(dep_name) {
					return Err(PluginError::MissingDependency {
						plugin: plugin_name.clone(),
						dependency: dep_name.clone(),
					});
				}

				// Validate version requirement
				// These lookups may fail if the dependency graph is inconsistent
				// with the plugins map (e.g., due to concurrent modification).
				let Some(plugin_entry) = plugins.get(plugin_name) else {
					continue;
				};
				let plugin_metadata = plugin_entry.plugin.metadata();
				let Some(dep_spec) = plugin_metadata
					.dependencies
					.iter()
					.find(|d| &d.name == dep_name)
				else {
					continue;
				};

				let actual_version = &plugins
					.get(dep_name)
					.ok_or_else(|| PluginError::MissingDependency {
						plugin: plugin_name.clone(),
						dependency: dep_name.clone(),
					})?
					.plugin
					.metadata()
					.version;
				if !dep_spec.version_req.matches(actual_version) {
					return Err(PluginError::IncompatibleVersion {
						plugin: plugin_name.clone(),
						dependency: dep_name.clone(),
						required: dep_spec.version_req.to_string(),
						actual: actual_version.clone(),
					});
				}
			}
		}

		Ok(())
	}

	/// Gets the topological order for enabling plugins.
	///
	/// Returns plugins in an order where dependencies come before dependents.
	/// Uses Kahn's algorithm: edges point from dependency to dependent,
	/// so in-degree counts represent the number of unsatisfied dependencies.
	pub fn get_enable_order(&self) -> PluginResult<Vec<String>> {
		let deps = self.dependency_graph.read();
		let reverse_deps = self.dependents.read();
		let plugins = self.plugins.read();

		// Kahn's algorithm for topological sort
		let mut in_degree: HashMap<String, usize> = HashMap::new();

		// Initialize in-degree for all plugins
		for name in plugins.keys() {
			in_degree.insert(name.clone(), 0);
		}

		// Count incoming edges: for each plugin, in-degree = number of dependencies
		for (plugin_name, dependencies) in deps.iter() {
			if let Some(degree) = in_degree.get_mut(plugin_name) {
				*degree = dependencies
					.iter()
					.filter(|d| plugins.contains_key(d.as_str()))
					.count();
			}
		}

		// Start with nodes that have no dependencies (in-degree = 0)
		let mut queue: Vec<String> = in_degree
			.iter()
			.filter(|&(_, &degree)| degree == 0)
			.map(|(name, _)| name.clone())
			.collect();

		let mut result = Vec::new();

		while let Some(name) = queue.pop() {
			result.push(name.clone());

			// For each plugin that depends on this one, decrement its in-degree
			if let Some(dependents) = reverse_deps.get(&name) {
				for dependent in dependents {
					if let Some(degree) = in_degree.get_mut(dependent) {
						*degree -= 1;
						if *degree == 0 {
							queue.push(dependent.clone());
						}
					}
				}
			}
		}

		if result.len() != plugins.len() {
			return Err(PluginError::CircularDependency);
		}

		Ok(result)
	}

	/// Gets plugins that depend on the given plugin.
	pub fn get_dependents(&self, name: &str) -> Vec<String> {
		self.dependents
			.read()
			.get(name)
			.cloned()
			.unwrap_or_default()
	}

	/// Gets dependencies of the given plugin.
	pub fn get_dependencies(&self, name: &str) -> Vec<String> {
		self.dependency_graph
			.read()
			.get(name)
			.cloned()
			.unwrap_or_default()
	}

	/// Sets the state of a plugin.
	///
	/// This is an internal method used by lifecycle operations.
	pub(crate) fn set_state(&self, name: &str, state: PluginState) -> PluginResult<()> {
		let mut plugins = self.plugins.write();
		if let Some(entry) = plugins.get_mut(name) {
			entry.state = state;
			Ok(())
		} else {
			Err(PluginError::NotFound(name.to_string()))
		}
	}

	/// Loads all registered plugins.
	pub async fn load_all(&self, ctx: &PluginContext) -> PluginResult<()> {
		let order = self.get_enable_order()?;

		for name in order {
			self.load_plugin(&name, ctx).await?;
		}

		Ok(())
	}

	/// Loads a single plugin.
	async fn load_plugin(&self, name: &str, ctx: &PluginContext) -> PluginResult<()> {
		let lifecycle = {
			let plugins = self.plugins.read();
			let entry = plugins
				.get(name)
				.ok_or_else(|| PluginError::NotFound(name.to_string()))?;
			entry.lifecycle.clone()
		};

		// Call lifecycle hook if available
		if let Some(lifecycle) = lifecycle {
			lifecycle.on_load(ctx).await?;
		}

		self.set_state(name, PluginState::Loaded)?;
		tracing::info!("Loaded plugin: {}", name);

		Ok(())
	}

	/// Enables all loaded plugins.
	pub async fn enable_all(&self, ctx: &PluginContext) -> PluginResult<()> {
		let order = self.get_enable_order()?;

		for name in order {
			if self.get_state(&name) == Some(PluginState::Loaded) {
				self.enable_plugin(&name, ctx).await?;
			}
		}

		Ok(())
	}

	/// Enables a single plugin.
	async fn enable_plugin(&self, name: &str, ctx: &PluginContext) -> PluginResult<()> {
		// Acquire all needed locks simultaneously to get a consistent snapshot
		// and prevent TOCTOU issues between dependency check and lifecycle access
		let lifecycle = {
			let plugins = self.plugins.read();
			let deps = self.dependency_graph.read();

			// Check dependencies are enabled (using already-held plugins lock)
			if let Some(dependencies) = deps.get(name) {
				for dep in dependencies {
					let is_dep_enabled = plugins
						.get(dep)
						.is_some_and(|e| e.state == PluginState::Enabled);
					if !is_dep_enabled {
						return Err(PluginError::MissingDependency {
							plugin: name.to_string(),
							dependency: dep.clone(),
						});
					}
				}
			}

			// Get lifecycle reference while still holding the lock
			plugins.get(name).and_then(|e| e.lifecycle.clone())
		};

		// Call lifecycle hook if available (outside locks to avoid holding
		// locks across async calls)
		if let Some(lifecycle) = lifecycle {
			lifecycle.on_enable(ctx).await?;
		}

		self.set_state(name, PluginState::Enabled)?;
		tracing::info!("Enabled plugin: {}", name);

		Ok(())
	}

	/// Disables a plugin and all plugins that depend on it.
	///
	/// This method recursively disables dependent plugins. It tracks visited
	/// nodes to prevent infinite recursion if a cycle exists in the dependents
	/// graph, and enforces a maximum recursion depth as an additional safeguard.
	pub async fn disable_plugin(&self, name: &str, ctx: &PluginContext) -> PluginResult<()> {
		use std::collections::HashSet;

		/// Maximum recursion depth for disable_plugin to prevent stack overflow.
		const MAX_DISABLE_DEPTH: usize = 64;

		fn disable_inner<'a>(
			registry: &'a PluginRegistry,
			name: &'a str,
			ctx: &'a PluginContext,
			visited: &'a mut HashSet<String>,
			depth: usize,
		) -> std::pin::Pin<Box<dyn std::future::Future<Output = PluginResult<()>> + 'a>> {
			Box::pin(async move {
				// Guard against cycles and excessive depth
				if !visited.insert(name.to_string()) {
					tracing::warn!(
						"Cycle detected in dependents graph while disabling '{}', skipping",
						name
					);
					return Ok(());
				}
				if depth > MAX_DISABLE_DEPTH {
					return Err(PluginError::Custom(format!(
						"maximum recursion depth ({}) exceeded while disabling plugin '{}'",
						MAX_DISABLE_DEPTH, name
					)));
				}

				// Acquire both locks simultaneously to get a consistent snapshot
				// of dependents and their enabled status, plus lifecycle reference
				let (enabled_dependents, lifecycle) = {
					let plugins = registry.plugins.read();
					let reverse = registry.dependents.read();

					let enabled_deps: Vec<String> = reverse
						.get(name)
						.map(|deps| {
							deps.iter()
								.filter(|dep| {
									plugins
										.get(dep.as_str())
										.is_some_and(|e| e.state == PluginState::Enabled)
								})
								.cloned()
								.collect()
						})
						.unwrap_or_default();

					let lc = plugins.get(name).and_then(|e| e.lifecycle.clone());

					(enabled_deps, lc)
				};

				// Disable all enabled dependents first
				for dep in enabled_dependents {
					disable_inner(registry, &dep, ctx, visited, depth + 1).await?;
				}

				// Use the lifecycle reference obtained from the consistent snapshot
				let lifecycle = lifecycle;

				// Call lifecycle hook if available
				if let Some(lifecycle) = lifecycle {
					// Log but don't fail if on_disable returns an error
					if let Err(e) = lifecycle.on_disable(ctx).await {
						tracing::warn!("Plugin {} on_disable hook returned error: {}", name, e);
					}
				}

				registry.set_state(name, PluginState::Disabled)?;
				tracing::info!("Disabled plugin: {}", name);

				Ok(())
			})
		}

		let mut visited = HashSet::new();
		disable_inner(self, name, ctx, &mut visited, 0).await
	}

	/// Unregisters a plugin.
	///
	/// The plugin must be disabled first. If the plugin has lifecycle support,
	/// the `on_unload` hook will be called before removal.
	pub async fn unregister(&self, name: &str, ctx: &PluginContext) -> PluginResult<()> {
		let (state, lifecycle) = {
			let plugins = self.plugins.read();
			let entry = plugins
				.get(name)
				.ok_or_else(|| PluginError::NotFound(name.to_string()))?;
			(entry.state, entry.lifecycle.clone())
		};

		if state == PluginState::Enabled {
			return Err(PluginError::InvalidStateTransition {
				plugin: name.to_string(),
				from: state,
				to: PluginState::Registered,
			});
		}

		// Call on_unload lifecycle hook if available
		if let Some(lifecycle) = lifecycle {
			// Log but don't fail if on_unload returns an error
			if let Err(e) = lifecycle.on_unload(ctx).await {
				tracing::warn!("Plugin {} on_unload hook returned error: {}", name, e);
			}
		}

		// Remove from capability map
		{
			let mut cap_map = self.capability_map.write();
			for providers in cap_map.values_mut() {
				providers.retain(|n| n != name);
			}
		}

		// Remove from dependency graphs
		{
			let mut deps = self.dependency_graph.write();
			let mut reverse = self.dependents.write();

			deps.remove(name);
			reverse.remove(name);

			for providers in deps.values_mut() {
				providers.retain(|n| n != name);
			}
			for providers in reverse.values_mut() {
				providers.retain(|n| n != name);
			}
		}

		// Remove plugin
		self.plugins.write().remove(name);
		tracing::info!("Unregistered plugin: {}", name);

		Ok(())
	}
}

impl Default for PluginRegistry {
	fn default() -> Self {
		Self::new()
	}
}

impl std::fmt::Debug for PluginRegistry {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let plugins = self.plugins.read();
		let plugin_info: Vec<(&String, PluginState)> =
			plugins.iter().map(|(k, v)| (k, v.state)).collect();

		f.debug_struct("PluginRegistry")
			.field("plugins", &plugin_info)
			.field(
				"capabilities",
				&self.capability_map.read().keys().collect::<Vec<_>>(),
			)
			.finish()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::capability::PluginCapability;
	use crate::metadata::PluginMetadata;
	use crate::plugin::Plugin;

	struct TestPlugin {
		metadata: PluginMetadata,
		capabilities: Vec<Capability>,
	}

	impl TestPlugin {
		fn new(name: &str, version: &str) -> Self {
			Self {
				metadata: PluginMetadata::builder(name, version).build().unwrap(),
				capabilities: vec![],
			}
		}

		fn with_capability(mut self, cap: PluginCapability) -> Self {
			self.capabilities.push(Capability::Core(cap));
			self
		}

		fn with_dependency(mut self, dep_name: &str, version_req: &str) -> Self {
			let mut builder =
				PluginMetadata::builder(&self.metadata.name, &self.metadata.version.to_string());
			// Preserve existing dependencies
			for dep in &self.metadata.dependencies {
				builder = builder.depends_on(&dep.name, &dep.version_req.to_string());
			}
			builder = builder.depends_on(dep_name, version_req);
			self.metadata = builder.build().unwrap();
			self
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

	#[test]
	fn test_register_plugin() {
		let registry = PluginRegistry::new();
		let plugin = Arc::new(TestPlugin::new("test-delion", "1.0.0"));

		registry.register(plugin).unwrap();

		assert!(registry.is_registered("test-delion"));
		assert_eq!(registry.len(), 1);
	}

	#[test]
	fn test_capability_providers() {
		let registry = PluginRegistry::new();
		let plugin = Arc::new(
			TestPlugin::new("auth-delion", "1.0.0").with_capability(PluginCapability::Auth),
		);

		registry.register(plugin).unwrap();
		registry
			.set_state("auth-delion", PluginState::Enabled)
			.unwrap();

		let providers =
			registry.get_capability_providers(&Capability::Core(PluginCapability::Auth));
		assert_eq!(providers.len(), 1);
		assert_eq!(providers[0].name(), "auth-delion");
	}

	#[test]
	fn test_dependency_validation() {
		let registry = PluginRegistry::new();

		let core = Arc::new(TestPlugin::new("core-delion", "1.0.0"));
		let auth = Arc::new(
			TestPlugin::new("auth-delion", "1.0.0").with_dependency("core-delion", "^1.0.0"),
		);

		registry.register(core).unwrap();
		registry.register(auth).unwrap();

		assert!(registry.validate_dependencies().is_ok());
	}

	#[test]
	fn test_missing_dependency() {
		let registry = PluginRegistry::new();

		let auth = Arc::new(
			TestPlugin::new("auth-delion", "1.0.0").with_dependency("core-delion", "^1.0.0"),
		);

		registry.register(auth).unwrap();

		let result = registry.validate_dependencies();
		assert!(matches!(result, Err(PluginError::MissingDependency { .. })));
	}

	#[test]
	fn test_enable_order() {
		let registry = PluginRegistry::new();

		let core = Arc::new(TestPlugin::new("core-delion", "1.0.0"));
		let auth = Arc::new(
			TestPlugin::new("auth-delion", "1.0.0").with_dependency("core-delion", "^1.0.0"),
		);
		let api = Arc::new(
			TestPlugin::new("api-delion", "1.0.0").with_dependency("auth-delion", "^1.0.0"),
		);

		registry.register(core).unwrap();
		registry.register(auth).unwrap();
		registry.register(api).unwrap();

		let order = registry.get_enable_order().unwrap();

		// core should come before auth, auth should come before api
		let core_pos = order.iter().position(|n| n == "core-delion").unwrap();
		let auth_pos = order.iter().position(|n| n == "auth-delion").unwrap();
		let api_pos = order.iter().position(|n| n == "api-delion").unwrap();

		assert!(core_pos < auth_pos);
		assert!(auth_pos < api_pos);
	}

	// ==========================================================================
	// Circular Dependency Tests
	// ==========================================================================

	#[test]
	fn test_circular_dependency_detection() {
		let registry = PluginRegistry::new();

		let a =
			Arc::new(TestPlugin::new("a-delion", "1.0.0").with_dependency("b-delion", "^1.0.0"));
		let b =
			Arc::new(TestPlugin::new("b-delion", "1.0.0").with_dependency("a-delion", "^1.0.0"));

		registry.register(a).unwrap();
		registry.register(b).unwrap();

		let result = registry.get_enable_order();
		assert!(matches!(result, Err(PluginError::CircularDependency)));
	}

	#[test]
	fn test_circular_dependency_three_plugins() {
		let registry = PluginRegistry::new();

		// A -> B -> C -> A (cycle)
		let a =
			Arc::new(TestPlugin::new("a-delion", "1.0.0").with_dependency("c-delion", "^1.0.0"));
		let b =
			Arc::new(TestPlugin::new("b-delion", "1.0.0").with_dependency("a-delion", "^1.0.0"));
		let c =
			Arc::new(TestPlugin::new("c-delion", "1.0.0").with_dependency("b-delion", "^1.0.0"));

		registry.register(a).unwrap();
		registry.register(b).unwrap();
		registry.register(c).unwrap();

		let result = registry.get_enable_order();
		assert!(matches!(result, Err(PluginError::CircularDependency)));
	}

	// ==========================================================================
	// Incompatible Version Tests
	// ==========================================================================

	#[test]
	fn test_incompatible_version() {
		let registry = PluginRegistry::new();

		let core = Arc::new(TestPlugin::new("core-delion", "2.0.0"));
		let auth = Arc::new(
			TestPlugin::new("auth-delion", "1.0.0").with_dependency("core-delion", "^1.0.0"),
		);

		registry.register(core).unwrap();
		registry.register(auth).unwrap();

		let result = registry.validate_dependencies();
		assert!(matches!(
			result,
			Err(PluginError::IncompatibleVersion { .. })
		));
	}

	#[test]
	fn test_compatible_version_minor() {
		let registry = PluginRegistry::new();

		// core 1.5.0 should be compatible with ^1.0.0
		let core = Arc::new(TestPlugin::new("core-delion", "1.5.0"));
		let auth = Arc::new(
			TestPlugin::new("auth-delion", "1.0.0").with_dependency("core-delion", "^1.0.0"),
		);

		registry.register(core).unwrap();
		registry.register(auth).unwrap();

		assert!(registry.validate_dependencies().is_ok());
	}

	// ==========================================================================
	// Unregister Tests
	// ==========================================================================

	#[tokio::test]
	async fn test_unregister_nonexistent_fails() {
		use std::path::PathBuf;

		let registry = PluginRegistry::new();
		let ctx = PluginContext::new(PathBuf::from("/tmp/test-project"));
		let result = registry.unregister("nonexistent-delion", &ctx).await;
		assert!(matches!(result, Err(PluginError::NotFound(_))));
	}

	#[tokio::test]
	async fn test_unregister_enabled_plugin_fails() {
		use std::path::PathBuf;

		let registry = PluginRegistry::new();
		let plugin = Arc::new(TestPlugin::new("test-delion", "1.0.0"));
		registry.register(plugin).unwrap();
		registry
			.set_state("test-delion", PluginState::Enabled)
			.unwrap();

		let ctx = PluginContext::new(PathBuf::from("/tmp/test-project"));
		let result = registry.unregister("test-delion", &ctx).await;
		assert!(matches!(
			result,
			Err(PluginError::InvalidStateTransition { .. })
		));
	}

	#[tokio::test]
	async fn test_unregister_loaded_plugin_succeeds() {
		use std::path::PathBuf;

		let registry = PluginRegistry::new();
		let plugin = Arc::new(TestPlugin::new("test-delion", "1.0.0"));
		registry.register(plugin).unwrap();
		registry
			.set_state("test-delion", PluginState::Loaded)
			.unwrap();

		let ctx = PluginContext::new(PathBuf::from("/tmp/test-project"));
		assert!(registry.unregister("test-delion", &ctx).await.is_ok());
		assert!(!registry.is_registered("test-delion"));
	}

	#[tokio::test]
	async fn test_unregister_disabled_plugin_succeeds() {
		use std::path::PathBuf;

		let registry = PluginRegistry::new();
		let plugin = Arc::new(TestPlugin::new("test-delion", "1.0.0"));
		registry.register(plugin).unwrap();
		registry
			.set_state("test-delion", PluginState::Disabled)
			.unwrap();

		let ctx = PluginContext::new(PathBuf::from("/tmp/test-project"));
		assert!(registry.unregister("test-delion", &ctx).await.is_ok());
		assert!(!registry.is_registered("test-delion"));
	}

	// ==========================================================================
	// Register Tests
	// ==========================================================================

	#[test]
	fn test_register_same_version_twice_succeeds() {
		let registry = PluginRegistry::new();
		let plugin1 = Arc::new(TestPlugin::new("test-delion", "1.0.0"));
		let plugin2 = Arc::new(TestPlugin::new("test-delion", "1.0.0"));

		registry.register(plugin1).unwrap();
		// Same version should succeed (idempotent)
		registry.register(plugin2).unwrap();

		assert_eq!(registry.len(), 1);
	}

	#[test]
	fn test_register_different_version_fails() {
		let registry = PluginRegistry::new();
		let plugin1 = Arc::new(TestPlugin::new("test-delion", "1.0.0"));
		let plugin2 = Arc::new(TestPlugin::new("test-delion", "2.0.0"));

		registry.register(plugin1).unwrap();
		let result = registry.register(plugin2);

		assert!(matches!(result, Err(PluginError::VersionConflict { .. })));
	}

	// ==========================================================================
	// Default and Debug Trait Tests
	// ==========================================================================

	#[test]
	fn test_registry_default() {
		let registry = PluginRegistry::default();
		assert!(registry.is_empty());
		assert_eq!(registry.len(), 0);
	}

	#[test]
	fn test_registry_debug() {
		let registry = PluginRegistry::new();
		let debug_str = format!("{:?}", registry);
		assert!(debug_str.contains("PluginRegistry"));
	}

	// ==========================================================================
	// State Tests
	// ==========================================================================

	#[test]
	fn test_get_state_nonexistent() {
		let registry = PluginRegistry::new();
		assert!(registry.get_state("nonexistent").is_none());
	}

	#[test]
	fn test_get_state_registered() {
		let registry = PluginRegistry::new();
		let plugin = Arc::new(TestPlugin::new("test-delion", "1.0.0"));
		registry.register(plugin).unwrap();

		assert_eq!(
			registry.get_state("test-delion"),
			Some(PluginState::Registered)
		);
	}

	#[test]
	fn test_set_state_nonexistent_fails() {
		let registry = PluginRegistry::new();
		let result = registry.set_state("nonexistent", PluginState::Enabled);
		assert!(matches!(result, Err(PluginError::NotFound(_))));
	}

	#[test]
	fn test_is_enabled() {
		let registry = PluginRegistry::new();
		let plugin = Arc::new(TestPlugin::new("test-delion", "1.0.0"));
		registry.register(plugin).unwrap();

		assert!(!registry.is_enabled("test-delion"));

		registry
			.set_state("test-delion", PluginState::Enabled)
			.unwrap();
		assert!(registry.is_enabled("test-delion"));
	}

	// ==========================================================================
	// Plugin Names and Getters Tests
	// ==========================================================================

	#[test]
	fn test_plugin_names() {
		let registry = PluginRegistry::new();
		registry
			.register(Arc::new(TestPlugin::new("a-delion", "1.0.0")))
			.unwrap();
		registry
			.register(Arc::new(TestPlugin::new("b-delion", "1.0.0")))
			.unwrap();
		registry
			.register(Arc::new(TestPlugin::new("c-delion", "1.0.0")))
			.unwrap();

		let names = registry.plugin_names();
		assert_eq!(names.len(), 3);
		assert!(names.contains(&"a-delion".to_string()));
		assert!(names.contains(&"b-delion".to_string()));
		assert!(names.contains(&"c-delion".to_string()));
	}

	#[test]
	fn test_get_plugin() {
		let registry = PluginRegistry::new();
		registry
			.register(Arc::new(TestPlugin::new("test-delion", "1.0.0")))
			.unwrap();

		let plugin = registry.get("test-delion");
		assert!(plugin.is_some());
		assert_eq!(plugin.unwrap().name(), "test-delion");
	}

	#[test]
	fn test_get_nonexistent_plugin() {
		let registry = PluginRegistry::new();
		assert!(registry.get("nonexistent").is_none());
	}

	// ==========================================================================
	// Dependency Query Tests
	// ==========================================================================

	#[test]
	fn test_get_dependents() {
		let registry = PluginRegistry::new();

		let core = Arc::new(TestPlugin::new("core-delion", "1.0.0"));
		let auth = Arc::new(
			TestPlugin::new("auth-delion", "1.0.0").with_dependency("core-delion", "^1.0.0"),
		);

		registry.register(core).unwrap();
		registry.register(auth).unwrap();

		let dependents = registry.get_dependents("core-delion");
		assert_eq!(dependents.len(), 1);
		assert_eq!(dependents[0], "auth-delion");
	}

	#[test]
	fn test_get_dependencies() {
		let registry = PluginRegistry::new();

		let core = Arc::new(TestPlugin::new("core-delion", "1.0.0"));
		let auth = Arc::new(
			TestPlugin::new("auth-delion", "1.0.0").with_dependency("core-delion", "^1.0.0"),
		);

		registry.register(core).unwrap();
		registry.register(auth).unwrap();

		let deps = registry.get_dependencies("auth-delion");
		assert_eq!(deps.len(), 1);
		assert_eq!(deps[0], "core-delion");
	}

	#[test]
	fn test_get_dependencies_nonexistent() {
		let registry = PluginRegistry::new();
		let deps = registry.get_dependencies("nonexistent");
		assert!(deps.is_empty());
	}

	// ==========================================================================
	// Capability Tests
	// ==========================================================================

	#[test]
	fn test_enabled_with_capability() {
		let registry = PluginRegistry::new();
		let plugin = Arc::new(
			TestPlugin::new("auth-delion", "1.0.0").with_capability(PluginCapability::Auth),
		);

		registry.register(plugin).unwrap();
		registry
			.set_state("auth-delion", PluginState::Enabled)
			.unwrap();

		let auth_providers =
			registry.enabled_with_capability(&Capability::Core(PluginCapability::Auth));
		assert!(!auth_providers.is_empty());

		let services_providers =
			registry.enabled_with_capability(&Capability::Core(PluginCapability::Services));
		assert!(services_providers.is_empty());
	}

	#[test]
	fn test_capability_providers_empty() {
		let registry = PluginRegistry::new();
		let providers =
			registry.get_capability_providers(&Capability::Core(PluginCapability::Auth));
		assert!(providers.is_empty());
	}

	// ==========================================================================
	// Async Lifecycle Tests
	// ==========================================================================

	#[tokio::test]
	async fn test_load_all_plugins() {
		use crate::context::PluginContext;
		use std::path::PathBuf;

		let registry = PluginRegistry::new();
		let plugin = Arc::new(TestPlugin::new("test-delion", "1.0.0"));
		registry.register(plugin).unwrap();

		let ctx = PluginContext::new(PathBuf::from("/tmp/test-project"));
		registry.load_all(&ctx).await.unwrap();

		assert_eq!(registry.get_state("test-delion"), Some(PluginState::Loaded));
	}

	#[tokio::test]
	async fn test_enable_all_plugins() {
		use crate::context::PluginContext;
		use std::path::PathBuf;

		let registry = PluginRegistry::new();
		let plugin = Arc::new(TestPlugin::new("test-delion", "1.0.0"));
		registry.register(plugin).unwrap();

		let ctx = PluginContext::new(PathBuf::from("/tmp/test-project"));
		registry.load_all(&ctx).await.unwrap();
		registry.enable_all(&ctx).await.unwrap();

		assert!(registry.is_enabled("test-delion"));
	}

	#[tokio::test]
	async fn test_load_plugin_single() {
		use crate::context::PluginContext;
		use std::path::PathBuf;

		let registry = PluginRegistry::new();
		let plugin = Arc::new(TestPlugin::new("test-delion", "1.0.0"));
		registry.register(plugin).unwrap();

		let ctx = PluginContext::new(PathBuf::from("/tmp/test-project"));
		registry.load_plugin("test-delion", &ctx).await.unwrap();

		assert_eq!(registry.get_state("test-delion"), Some(PluginState::Loaded));
	}

	#[tokio::test]
	async fn test_load_plugin_nonexistent() {
		use crate::context::PluginContext;
		use std::path::PathBuf;

		let registry = PluginRegistry::new();
		let ctx = PluginContext::new(PathBuf::from("/tmp/test-project"));

		let result = registry.load_plugin("nonexistent", &ctx).await;
		assert!(matches!(result, Err(PluginError::NotFound(_))));
	}

	#[tokio::test]
	async fn test_enable_plugin_single() {
		use crate::context::PluginContext;
		use std::path::PathBuf;

		let registry = PluginRegistry::new();
		let plugin = Arc::new(TestPlugin::new("test-delion", "1.0.0"));
		registry.register(plugin).unwrap();

		let ctx = PluginContext::new(PathBuf::from("/tmp/test-project"));
		registry.load_plugin("test-delion", &ctx).await.unwrap();
		registry.enable_plugin("test-delion", &ctx).await.unwrap();

		assert!(registry.is_enabled("test-delion"));
	}

	#[tokio::test]
	async fn test_enable_plugin_nonexistent() {
		use crate::context::PluginContext;
		use std::path::PathBuf;

		let registry = PluginRegistry::new();
		let ctx = PluginContext::new(PathBuf::from("/tmp/test-project"));

		let result = registry.enable_plugin("nonexistent", &ctx).await;
		assert!(matches!(result, Err(PluginError::NotFound(_))));
	}

	#[tokio::test]
	async fn test_disable_plugin() {
		use crate::context::PluginContext;
		use std::path::PathBuf;

		let registry = PluginRegistry::new();
		let plugin = Arc::new(TestPlugin::new("test-delion", "1.0.0"));
		registry.register(plugin).unwrap();

		let ctx = PluginContext::new(PathBuf::from("/tmp/test-project"));
		registry.load_plugin("test-delion", &ctx).await.unwrap();
		registry.enable_plugin("test-delion", &ctx).await.unwrap();
		assert!(registry.is_enabled("test-delion"));

		registry.disable_plugin("test-delion", &ctx).await.unwrap();
		assert!(!registry.is_enabled("test-delion"));
		assert_eq!(
			registry.get_state("test-delion"),
			Some(PluginState::Disabled)
		);
	}

	#[tokio::test]
	async fn test_disable_plugin_nonexistent() {
		use crate::context::PluginContext;
		use std::path::PathBuf;

		let registry = PluginRegistry::new();
		let ctx = PluginContext::new(PathBuf::from("/tmp/test-project"));

		let result = registry.disable_plugin("nonexistent", &ctx).await;
		assert!(matches!(result, Err(PluginError::NotFound(_))));
	}

	#[tokio::test]
	async fn test_lifecycle_order_with_dependencies() {
		use crate::context::PluginContext;
		use std::path::PathBuf;

		let registry = PluginRegistry::new();

		let core = Arc::new(TestPlugin::new("core-delion", "1.0.0"));
		let auth = Arc::new(
			TestPlugin::new("auth-delion", "1.0.0").with_dependency("core-delion", "^1.0.0"),
		);

		registry.register(core).unwrap();
		registry.register(auth).unwrap();

		let ctx = PluginContext::new(PathBuf::from("/tmp/test-project"));
		registry.load_all(&ctx).await.unwrap();
		registry.enable_all(&ctx).await.unwrap();

		assert!(registry.is_enabled("core-delion"));
		assert!(registry.is_enabled("auth-delion"));
	}

	// ==========================================================================
	// Topological Sort Correctness Tests (#683)
	// ==========================================================================

	#[rstest::rstest]
	fn test_enable_order_diamond_dependency() {
		// Arrange: Diamond pattern A -> B, A -> C, B -> D, C -> D
		let registry = PluginRegistry::new();

		let d = Arc::new(TestPlugin::new("d-delion", "1.0.0"));
		let b =
			Arc::new(TestPlugin::new("b-delion", "1.0.0").with_dependency("d-delion", "^1.0.0"));
		let c =
			Arc::new(TestPlugin::new("c-delion", "1.0.0").with_dependency("d-delion", "^1.0.0"));
		let a = Arc::new(
			TestPlugin::new("a-delion", "1.0.0")
				.with_dependency("b-delion", "^1.0.0")
				.with_dependency("c-delion", "^1.0.0"),
		);

		registry.register(d).unwrap();
		registry.register(b).unwrap();
		registry.register(c).unwrap();
		registry.register(a).unwrap();

		// Act
		let order = registry.get_enable_order().unwrap();

		// Assert - dependencies must come before dependents
		let pos = |name: &str| order.iter().position(|n| n == name).unwrap();
		assert!(pos("d-delion") < pos("b-delion"));
		assert!(pos("d-delion") < pos("c-delion"));
		assert!(pos("b-delion") < pos("a-delion"));
		assert!(pos("c-delion") < pos("a-delion"));
	}

	#[rstest::rstest]
	fn test_enable_order_single_node() {
		// Arrange
		let registry = PluginRegistry::new();
		let plugin = Arc::new(TestPlugin::new("solo-delion", "1.0.0"));
		registry.register(plugin).unwrap();

		// Act
		let order = registry.get_enable_order().unwrap();

		// Assert
		assert_eq!(order.len(), 1);
		assert_eq!(order[0], "solo-delion");
	}

	#[rstest::rstest]
	fn test_enable_order_independent_plugins() {
		// Arrange - three independent plugins with no dependencies
		let registry = PluginRegistry::new();
		registry
			.register(Arc::new(TestPlugin::new("x-delion", "1.0.0")))
			.unwrap();
		registry
			.register(Arc::new(TestPlugin::new("y-delion", "1.0.0")))
			.unwrap();
		registry
			.register(Arc::new(TestPlugin::new("z-delion", "1.0.0")))
			.unwrap();

		// Act
		let order = registry.get_enable_order().unwrap();

		// Assert - all three should be present (any order is valid)
		assert_eq!(order.len(), 3);
		assert!(order.contains(&"x-delion".to_string()));
		assert!(order.contains(&"y-delion".to_string()));
		assert!(order.contains(&"z-delion".to_string()));
	}

	#[rstest::rstest]
	fn test_enable_order_complex_graph() {
		// Arrange: A -> B, B -> C, D -> C, E is independent
		let registry = PluginRegistry::new();

		let c = Arc::new(TestPlugin::new("c-delion", "1.0.0"));
		let b =
			Arc::new(TestPlugin::new("b-delion", "1.0.0").with_dependency("c-delion", "^1.0.0"));
		let a =
			Arc::new(TestPlugin::new("a-delion", "1.0.0").with_dependency("b-delion", "^1.0.0"));
		let d =
			Arc::new(TestPlugin::new("d-delion", "1.0.0").with_dependency("c-delion", "^1.0.0"));
		let e = Arc::new(TestPlugin::new("e-delion", "1.0.0"));

		registry.register(c).unwrap();
		registry.register(b).unwrap();
		registry.register(a).unwrap();
		registry.register(d).unwrap();
		registry.register(e).unwrap();

		// Act
		let order = registry.get_enable_order().unwrap();

		// Assert
		assert_eq!(order.len(), 5);
		let pos = |name: &str| order.iter().position(|n| n == name).unwrap();
		assert!(pos("c-delion") < pos("b-delion"));
		assert!(pos("b-delion") < pos("a-delion"));
		assert!(pos("c-delion") < pos("d-delion"));
	}

	#[rstest::rstest]
	fn test_enable_order_no_false_circular_detection() {
		// Arrange: A -> B, A -> C (fan-out, no cycle)
		let registry = PluginRegistry::new();

		let b = Arc::new(TestPlugin::new("b-delion", "1.0.0"));
		let c = Arc::new(TestPlugin::new("c-delion", "1.0.0"));
		let a = Arc::new(
			TestPlugin::new("a-delion", "1.0.0")
				.with_dependency("b-delion", "^1.0.0")
				.with_dependency("c-delion", "^1.0.0"),
		);

		registry.register(b).unwrap();
		registry.register(c).unwrap();
		registry.register(a).unwrap();

		// Act
		let result = registry.get_enable_order();

		// Assert - should succeed, not report false circular dependency
		assert!(result.is_ok());
		let order = result.unwrap();
		assert_eq!(order.len(), 3);
		let pos = |name: &str| order.iter().position(|n| n == name).unwrap();
		assert!(pos("b-delion") < pos("a-delion"));
		assert!(pos("c-delion") < pos("a-delion"));
	}
}
