//! Plugin context for lifecycle operations.
//!
//! The plugin context provides plugins with access to framework services
//! during lifecycle hooks (load, enable, disable, unload).

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::RwLock;

/// Plugin context providing access to framework services.
///
/// The context is passed to plugins during lifecycle hooks and provides
/// access to:
/// - Plugin configuration
/// - Service registration
/// - Framework services (when integrated with Reinhardt)
///
/// # Thread Safety
///
/// The context is thread-safe and can be shared across async boundaries.
#[derive(Clone)]
pub struct PluginContext {
	/// Plugin configuration values.
	config: Arc<RwLock<HashMap<String, toml::Value>>>,

	/// Type-erased services registry.
	services: Arc<RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>>,

	/// Project root directory.
	project_root: PathBuf,

	/// Plugins data directory (.dentdelion/plugins).
	plugins_dir: PathBuf,
}

impl PluginContext {
	/// Creates a new plugin context.
	pub fn new(project_root: PathBuf) -> Self {
		let plugins_dir = project_root.join(".dentdelion").join("plugins");

		Self {
			config: Arc::new(RwLock::new(HashMap::new())),
			services: Arc::new(RwLock::new(HashMap::new())),
			project_root,
			plugins_dir,
		}
	}

	/// Creates a plugin context builder.
	pub fn builder(project_root: impl Into<PathBuf>) -> PluginContextBuilder {
		PluginContextBuilder::new(project_root)
	}

	/// Returns the project root directory.
	pub fn project_root(&self) -> &PathBuf {
		&self.project_root
	}

	/// Returns the plugins data directory.
	pub fn plugins_dir(&self) -> &PathBuf {
		&self.plugins_dir
	}

	/// Gets a configuration value.
	pub fn get_config(&self, key: &str) -> Option<toml::Value> {
		self.config.read().get(key).cloned()
	}

	/// Gets a configuration value as a string.
	pub fn get_config_string(&self, key: &str) -> Option<String> {
		self.get_config(key)
			.and_then(|v| v.as_str().map(String::from))
	}

	/// Gets a configuration value as an integer.
	pub fn get_config_int(&self, key: &str) -> Option<i64> {
		self.get_config(key).and_then(|v| v.as_integer())
	}

	/// Gets a configuration value as a boolean.
	pub fn get_config_bool(&self, key: &str) -> Option<bool> {
		self.get_config(key).and_then(|v| v.as_bool())
	}

	/// Sets a configuration value.
	pub fn set_config(&self, key: impl Into<String>, value: toml::Value) {
		self.config.write().insert(key.into(), value);
	}

	/// Sets all configuration values from a table.
	pub fn set_config_table(&self, table: HashMap<String, toml::Value>) {
		let mut config = self.config.write();
		for (key, value) in table {
			config.insert(key, value);
		}
	}

	/// Registers a service in the context.
	///
	/// Services are stored by their type and can be retrieved by any plugin.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_dentdelion::context::PluginContext;
	/// use std::sync::Arc;
	///
	/// # struct MyService;
	/// # impl MyService { fn new() -> Self { MyService } }
	/// let ctx = PluginContext::new(std::path::PathBuf::from("/tmp"));
	/// ctx.register_service(Arc::new(MyService::new()));
	/// ```
	pub fn register_service<T: Any + Send + Sync>(&self, service: Arc<T>) {
		let type_id = TypeId::of::<T>();
		self.services.write().insert(type_id, service);
	}

	/// Gets a registered service by type.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_dentdelion::context::PluginContext;
	/// use std::sync::Arc;
	///
	/// # struct MyService;
	/// # impl MyService { fn new() -> Self { MyService } }
	/// let ctx = PluginContext::new(std::path::PathBuf::from("/tmp"));
	/// ctx.register_service(Arc::new(MyService::new()));
	/// let service: Option<Arc<MyService>> = ctx.get_service();
	/// ```
	pub fn get_service<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
		let type_id = TypeId::of::<T>();
		self.services
			.read()
			.get(&type_id)
			.and_then(|service| service.clone().downcast::<T>().ok())
	}

	/// Checks if a service is registered.
	pub fn has_service<T: Any + Send + Sync>(&self) -> bool {
		let type_id = TypeId::of::<T>();
		self.services.read().contains_key(&type_id)
	}

	/// Removes a registered service.
	pub fn remove_service<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
		let type_id = TypeId::of::<T>();
		self.services
			.write()
			.remove(&type_id)
			.and_then(|service| service.downcast::<T>().ok())
	}
}

impl std::fmt::Debug for PluginContext {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("PluginContext")
			.field("project_root", &self.project_root)
			.field("plugins_dir", &self.plugins_dir)
			.field(
				"config_keys",
				&self.config.read().keys().collect::<Vec<_>>(),
			)
			.field("service_count", &self.services.read().len())
			.finish()
	}
}

/// Builder for PluginContext.
pub struct PluginContextBuilder {
	project_root: PathBuf,
	plugins_dir: Option<PathBuf>,
	config: HashMap<String, toml::Value>,
}

impl PluginContextBuilder {
	/// Creates a new builder.
	pub fn new(project_root: impl Into<PathBuf>) -> Self {
		Self {
			project_root: project_root.into(),
			plugins_dir: None,
			config: HashMap::new(),
		}
	}

	/// Sets a custom plugins directory.
	pub fn plugins_dir(mut self, dir: impl Into<PathBuf>) -> Self {
		self.plugins_dir = Some(dir.into());
		self
	}

	/// Adds a configuration value.
	pub fn config(mut self, key: impl Into<String>, value: toml::Value) -> Self {
		self.config.insert(key.into(), value);
		self
	}

	/// Adds multiple configuration values.
	pub fn config_table(mut self, table: HashMap<String, toml::Value>) -> Self {
		self.config.extend(table);
		self
	}

	/// Builds the PluginContext.
	pub fn build(self) -> PluginContext {
		let plugins_dir = self
			.plugins_dir
			.unwrap_or_else(|| self.project_root.join(".dentdelion").join("plugins"));

		PluginContext {
			config: Arc::new(RwLock::new(self.config)),
			services: Arc::new(RwLock::new(HashMap::new())),
			project_root: self.project_root,
			plugins_dir,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_context_creation() {
		let ctx = PluginContext::new(PathBuf::from("/test/project"));

		assert_eq!(ctx.project_root(), &PathBuf::from("/test/project"));
		assert_eq!(
			ctx.plugins_dir(),
			&PathBuf::from("/test/project/.dentdelion/plugins")
		);
	}

	#[test]
	fn test_config_operations() {
		let ctx = PluginContext::new(PathBuf::from("/test"));

		ctx.set_config("test_key", toml::Value::String("test_value".to_string()));
		assert_eq!(
			ctx.get_config_string("test_key"),
			Some("test_value".to_string())
		);

		ctx.set_config("int_key", toml::Value::Integer(42));
		assert_eq!(ctx.get_config_int("int_key"), Some(42));

		ctx.set_config("bool_key", toml::Value::Boolean(true));
		assert_eq!(ctx.get_config_bool("bool_key"), Some(true));
	}

	#[test]
	fn test_service_registration() {
		let ctx = PluginContext::new(PathBuf::from("/test"));

		struct TestService {
			value: i32,
		}

		let service = Arc::new(TestService { value: 42 });
		ctx.register_service(service);

		assert!(ctx.has_service::<TestService>());

		let retrieved = ctx.get_service::<TestService>().unwrap();
		assert_eq!(retrieved.value, 42);

		let removed = ctx.remove_service::<TestService>().unwrap();
		assert_eq!(removed.value, 42);
		assert!(!ctx.has_service::<TestService>());
	}

	#[test]
	fn test_builder() {
		let mut config = HashMap::new();
		config.insert(
			"key1".to_string(),
			toml::Value::String("value1".to_string()),
		);

		let ctx = PluginContext::builder("/test/project")
			.plugins_dir("/custom/plugins")
			.config_table(config)
			.config("key2", toml::Value::Integer(123))
			.build();

		assert_eq!(ctx.plugins_dir(), &PathBuf::from("/custom/plugins"));
		assert_eq!(ctx.get_config_string("key1"), Some("value1".to_string()));
		assert_eq!(ctx.get_config_int("key2"), Some(123));
	}
}
