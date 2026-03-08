use async_trait::async_trait;
use dashmap::DashMap;
use serde_json::Value as JsonValue;
use std::cell::RefCell;
use std::sync::Arc;

/// Event hook result - continue or stop propagation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
	/// Continue processing other hooks
	Continue,
	/// Stop processing hooks (veto operation)
	Veto,
}

/// Mapper lifecycle events (SQLAlchemy MapperEvents)
#[async_trait]
pub trait MapperEvents: Send + Sync {
	/// Called before an INSERT is emitted for an instance
	async fn before_insert(&self, _instance_id: &str, _values: &JsonValue) -> EventResult {
		EventResult::Continue
	}

	/// Called after an INSERT is emitted for an instance
	async fn after_insert(&self, _instance_id: &str) -> EventResult {
		EventResult::Continue
	}

	/// Called before an UPDATE is emitted for an instance
	async fn before_update(&self, _instance_id: &str, _values: &JsonValue) -> EventResult {
		EventResult::Continue
	}

	/// Called after an UPDATE is emitted for an instance
	async fn after_update(&self, _instance_id: &str) -> EventResult {
		EventResult::Continue
	}

	/// Called before a DELETE is emitted for an instance
	async fn before_delete(&self, _instance_id: &str) -> EventResult {
		EventResult::Continue
	}

	/// Called after a DELETE is emitted for an instance
	async fn after_delete(&self, _instance_id: &str) -> EventResult {
		EventResult::Continue
	}

	/// Called when an object is loaded from the database
	async fn load(&self, _instance_id: &str, _data: &JsonValue) -> EventResult {
		EventResult::Continue
	}

	/// Called when an object is refreshed from the database
	async fn refresh(&self, _instance_id: &str) -> EventResult {
		EventResult::Continue
	}

	/// Called when an object's attributes are expired
	async fn expire(&self, _instance_id: &str, _attribute_names: &[String]) -> EventResult {
		EventResult::Continue
	}
}

/// Session lifecycle events (SQLAlchemy SessionEvents)
#[async_trait]
pub trait SessionEvents: Send + Sync {
	/// Called before the flush process starts
	async fn before_flush(&self, _session_id: &str, _instances: &[String]) -> EventResult {
		EventResult::Continue
	}

	/// Called after the flush process completes
	async fn after_flush(&self, _session_id: &str) -> EventResult {
		EventResult::Continue
	}

	/// Called after session.flush() is called, but before commit is called
	async fn after_flush_postexec(&self, _session_id: &str) -> EventResult {
		EventResult::Continue
	}

	/// Called before a transaction is committed
	async fn before_commit(&self, _session_id: &str) -> EventResult {
		EventResult::Continue
	}

	/// Called after a transaction is committed
	async fn after_commit(&self, _session_id: &str) -> EventResult {
		EventResult::Continue
	}

	/// Called after a transaction is rolled back
	async fn after_rollback(&self, _session_id: &str) -> EventResult {
		EventResult::Continue
	}

	/// Called before a transaction begins
	async fn after_begin(&self, _session_id: &str) -> EventResult {
		EventResult::Continue
	}

	/// Called when session is soft-closed
	async fn after_soft_rollback(&self, _session_id: &str) -> EventResult {
		EventResult::Continue
	}

	/// Called before bulk insert
	async fn before_bulk_insert(&self, _values: &[JsonValue]) -> EventResult {
		EventResult::Continue
	}

	/// Called after bulk insert
	async fn after_bulk_insert(&self, _count: usize) -> EventResult {
		EventResult::Continue
	}

	/// Called before bulk update
	async fn before_bulk_update(&self, _filter: &JsonValue, _values: &JsonValue) -> EventResult {
		EventResult::Continue
	}

	/// Called after bulk update
	async fn after_bulk_update(&self, _count: usize) -> EventResult {
		EventResult::Continue
	}

	/// Called before bulk delete
	async fn before_bulk_delete(&self, _filter: &JsonValue) -> EventResult {
		EventResult::Continue
	}

	/// Called after bulk delete
	async fn after_bulk_delete(&self, _count: usize) -> EventResult {
		EventResult::Continue
	}
}

/// Attribute-level events (SQLAlchemy AttributeEvents)
#[async_trait]
pub trait AttributeEvents: Send + Sync {
	/// Called when an attribute value is set
	async fn set(
		&self,
		_instance_id: &str,
		_attribute: &str,
		_value: &JsonValue,
		_old_value: Option<&JsonValue>,
	) -> EventResult {
		EventResult::Continue
	}

	/// Called when an item is appended to a collection attribute
	async fn append(
		&self,
		_instance_id: &str,
		_attribute: &str,
		_value: &JsonValue,
	) -> EventResult {
		EventResult::Continue
	}

	/// Called when an item is removed from a collection attribute
	async fn remove(
		&self,
		_instance_id: &str,
		_attribute: &str,
		_value: &JsonValue,
	) -> EventResult {
		EventResult::Continue
	}

	/// Called when a scalar attribute is initialized
	async fn init_scalar(
		&self,
		_instance_id: &str,
		_attribute: &str,
		_value: &JsonValue,
	) -> EventResult {
		EventResult::Continue
	}

	/// Called when a collection attribute is initialized
	async fn init_collection(&self, _instance_id: &str, _attribute: &str) -> EventResult {
		EventResult::Continue
	}
}

/// Instance-level events (SQLAlchemy InstanceEvents)
#[async_trait]
pub trait InstanceEvents: Send + Sync {
	/// Called when a new instance is constructed
	async fn init(&self, _instance_id: &str) -> EventResult {
		EventResult::Continue
	}

	/// Called after instance is fully loaded
	async fn load(&self, _instance_id: &str) -> EventResult {
		EventResult::Continue
	}

	/// Called before instance is refreshed
	async fn refresh(&self, _instance_id: &str) -> EventResult {
		EventResult::Continue
	}

	/// Called after instance is refreshed
	async fn refresh_flush(&self, _instance_id: &str, _flush_context: &str) -> EventResult {
		EventResult::Continue
	}

	/// Called before instance is expired
	async fn expire(&self, _instance_id: &str, _attrs: &[String]) -> EventResult {
		EventResult::Continue
	}

	/// Called before instance is pickled (serialized)
	async fn pickle(&self, _instance_id: &str, _state_dict: &JsonValue) -> EventResult {
		EventResult::Continue
	}

	/// Called after instance is unpickled (deserialized)
	async fn unpickle(&self, _instance_id: &str, _state_dict: &JsonValue) -> EventResult {
		EventResult::Continue
	}
}

/// Event listener container
#[derive(Clone)]
pub enum EventListener {
	/// Mapper variant.
	Mapper(Arc<dyn MapperEvents>),
	/// Session variant.
	Session(Arc<dyn SessionEvents>),
	/// Attribute variant.
	Attribute(Arc<dyn AttributeEvents>),
	/// Instance variant.
	Instance(Arc<dyn InstanceEvents>),
}

/// Event registry for managing all event listeners
pub struct EventRegistry {
	mapper_listeners: DashMap<String, Vec<Arc<dyn MapperEvents>>>,
	session_listeners: DashMap<String, Vec<Arc<dyn SessionEvents>>>,
	attribute_listeners: DashMap<String, Vec<Arc<dyn AttributeEvents>>>,
	instance_listeners: DashMap<String, Vec<Arc<dyn InstanceEvents>>>,
}

impl EventRegistry {
	/// Create a new event registry
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::EventRegistry;
	///
	/// let registry = EventRegistry::new();
	/// assert_eq!(registry.mapper_listener_count(), 0);
	/// ```
	pub fn new() -> Self {
		Self {
			mapper_listeners: DashMap::new(),
			session_listeners: DashMap::new(),
			attribute_listeners: DashMap::new(),
			instance_listeners: DashMap::new(),
		}
	}
	/// Register a mapper event listener for a specific model
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::{EventRegistry, MapperEvents, EventResult};
	/// use std::sync::Arc;
	/// use async_trait::async_trait;
	/// use serde_json::Value as JsonValue;
	///
	/// struct MyListener;
	///
	/// #[async_trait]
	/// impl MapperEvents for MyListener {
	///     async fn before_insert(&self, _id: &str, _values: &JsonValue) -> EventResult {
	///         EventResult::Continue
	///     }
	/// }
	///
	/// let registry = EventRegistry::new();
	/// assert_eq!(registry.mapper_listener_count(), 0);
	/// registry.register_mapper_listener("User".to_string(), Arc::new(MyListener));
	/// assert_eq!(registry.mapper_listener_count(), 1);
	/// ```
	pub fn register_mapper_listener(&self, model: String, listener: Arc<dyn MapperEvents>) {
		self.mapper_listeners
			.entry(model)
			.or_default()
			.push(listener);
	}
	/// Register a session event listener for a specific session
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::{EventRegistry, SessionEvents, EventResult};
	/// use std::sync::Arc;
	/// use async_trait::async_trait;
	///
	/// struct MySessionListener;
	///
	/// #[async_trait]
	/// impl SessionEvents for MySessionListener {
	///     async fn before_commit(&self, _session_id: &str) -> EventResult {
	///         EventResult::Continue
	///     }
	/// }
	///
	/// let registry = EventRegistry::new();
	/// assert_eq!(registry.session_listener_count(), 0);
	/// registry.register_session_listener("session-1".to_string(), Arc::new(MySessionListener));
	/// assert_eq!(registry.session_listener_count(), 1);
	/// ```
	pub fn register_session_listener(&self, session_id: String, listener: Arc<dyn SessionEvents>) {
		self.session_listeners
			.entry(session_id)
			.or_default()
			.push(listener);
	}
	/// Register an attribute event listener for a specific model attribute
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::{EventRegistry, AttributeEvents, EventResult};
	/// use std::sync::Arc;
	/// use async_trait::async_trait;
	/// use serde_json::Value as JsonValue;
	///
	/// struct MyAttributeListener;
	///
	/// #[async_trait]
	/// impl AttributeEvents for MyAttributeListener {
	///     async fn set(&self, _id: &str, _attr: &str, _value: &JsonValue, _old: Option<&JsonValue>) -> EventResult {
	///         EventResult::Continue
	///     }
	/// }
	///
	/// let registry = EventRegistry::new();
	/// let listener = Arc::new(MyAttributeListener);
	/// registry.register_attribute_listener("User.email".to_string(), listener.clone());
	/// // Verify registration succeeded (no panic)
	/// let _: &EventRegistry = &registry;
	/// ```
	pub fn register_attribute_listener(
		&self,
		model_attr: String,
		listener: Arc<dyn AttributeEvents>,
	) {
		self.attribute_listeners
			.entry(model_attr)
			.or_default()
			.push(listener);
	}
	/// Register an instance event listener for a specific instance
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::{EventRegistry, InstanceEvents, EventResult};
	/// use std::sync::Arc;
	/// use async_trait::async_trait;
	///
	/// struct MyInstanceListener;
	///
	/// #[async_trait]
	/// impl InstanceEvents for MyInstanceListener {
	///     async fn init(&self, _instance_id: &str) -> EventResult {
	///         EventResult::Continue
	///     }
	/// }
	///
	/// let registry = EventRegistry::new();
	/// let listener = Arc::new(MyInstanceListener);
	/// registry.register_instance_listener("user-1".to_string(), listener.clone());
	/// // Verify registration succeeded (no panic)
	/// let _: &EventRegistry = &registry;
	/// ```
	pub fn register_instance_listener(
		&self,
		instance_id: String,
		listener: Arc<dyn InstanceEvents>,
	) {
		self.instance_listeners
			.entry(instance_id)
			.or_default()
			.push(listener);
	}
	/// Dispatch before_insert event to all registered listeners for a model
	///
	/// Returns Veto if any listener vetoes the operation
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::{EventRegistry, EventResult};
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let registry = EventRegistry::new();
	/// let values = json!({"name": "John"});
	/// let result = registry.dispatch_before_insert("User", "user-1", &values).await;
	/// assert_eq!(result, EventResult::Continue);
	/// # });
	/// ```
	pub async fn dispatch_before_insert(
		&self,
		model: &str,
		instance_id: &str,
		values: &JsonValue,
	) -> EventResult {
		if let Some(listeners) = self.mapper_listeners.get(model) {
			for listener in listeners.value() {
				match listener.before_insert(instance_id, values).await {
					EventResult::Veto => return EventResult::Veto,
					EventResult::Continue => continue,
				}
			}
		}
		EventResult::Continue
	}
	/// Dispatch after_insert event to all registered listeners for a model
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::{EventRegistry, EventResult};
	///
	/// # tokio_test::block_on(async {
	/// let registry = EventRegistry::new();
	/// let result = registry.dispatch_after_insert("User", "user-1").await;
	/// assert_eq!(result, EventResult::Continue);
	/// # });
	/// ```
	pub async fn dispatch_after_insert(&self, model: &str, instance_id: &str) -> EventResult {
		if let Some(listeners) = self.mapper_listeners.get(model) {
			for listener in listeners.value() {
				listener.after_insert(instance_id).await;
			}
		}
		EventResult::Continue
	}
	/// Dispatch before_update event to all registered listeners for a model
	///
	/// Returns Veto if any listener vetoes the operation
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::{EventRegistry, EventResult};
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let registry = EventRegistry::new();
	/// let values = json!({"name": "Jane"});
	/// let result = registry.dispatch_before_update("User", "user-1", &values).await;
	/// assert_eq!(result, EventResult::Continue);
	/// # });
	/// ```
	pub async fn dispatch_before_update(
		&self,
		model: &str,
		instance_id: &str,
		values: &JsonValue,
	) -> EventResult {
		if let Some(listeners) = self.mapper_listeners.get(model) {
			for listener in listeners.value() {
				match listener.before_update(instance_id, values).await {
					EventResult::Veto => return EventResult::Veto,
					EventResult::Continue => continue,
				}
			}
		}
		EventResult::Continue
	}
	/// Dispatch after_update event to all registered listeners for a model
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::{EventRegistry, EventResult};
	///
	/// # tokio_test::block_on(async {
	/// let registry = EventRegistry::new();
	/// let result = registry.dispatch_after_update("User", "user-1").await;
	/// assert_eq!(result, EventResult::Continue);
	/// # });
	/// ```
	pub async fn dispatch_after_update(&self, model: &str, instance_id: &str) -> EventResult {
		if let Some(listeners) = self.mapper_listeners.get(model) {
			for listener in listeners.value() {
				listener.after_update(instance_id).await;
			}
		}
		EventResult::Continue
	}
	/// Dispatch before_delete event to all registered listeners for a model
	///
	/// Returns Veto if any listener vetoes the operation
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::{EventRegistry, EventResult};
	///
	/// # tokio_test::block_on(async {
	/// let registry = EventRegistry::new();
	/// let result = registry.dispatch_before_delete("User", "user-1").await;
	/// assert_eq!(result, EventResult::Continue);
	/// # });
	/// ```
	pub async fn dispatch_before_delete(&self, model: &str, instance_id: &str) -> EventResult {
		if let Some(listeners) = self.mapper_listeners.get(model) {
			for listener in listeners.value() {
				match listener.before_delete(instance_id).await {
					EventResult::Veto => return EventResult::Veto,
					EventResult::Continue => continue,
				}
			}
		}
		EventResult::Continue
	}
	/// Dispatch after_delete event to all registered listeners for a model
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::{EventRegistry, EventResult};
	///
	/// # tokio_test::block_on(async {
	/// let registry = EventRegistry::new();
	/// let result = registry.dispatch_after_delete("User", "user-1").await;
	/// assert_eq!(result, EventResult::Continue);
	/// # });
	/// ```
	pub async fn dispatch_after_delete(&self, model: &str, instance_id: &str) -> EventResult {
		if let Some(listeners) = self.mapper_listeners.get(model) {
			for listener in listeners.value() {
				listener.after_delete(instance_id).await;
			}
		}
		EventResult::Continue
	}
	/// Dispatch before_flush event to all registered listeners for a session
	///
	/// Returns Veto if any listener vetoes the operation
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::{EventRegistry, EventResult};
	///
	/// # tokio_test::block_on(async {
	/// let registry = EventRegistry::new();
	/// let instances = vec!["user-1".to_string(), "user-2".to_string()];
	/// let result = registry.dispatch_before_flush("session-1", &instances).await;
	/// assert_eq!(result, EventResult::Continue);
	/// # });
	/// ```
	pub async fn dispatch_before_flush(
		&self,
		session_id: &str,
		instances: &[String],
	) -> EventResult {
		if let Some(listeners) = self.session_listeners.get(session_id) {
			for listener in listeners.value() {
				match listener.before_flush(session_id, instances).await {
					EventResult::Veto => return EventResult::Veto,
					EventResult::Continue => continue,
				}
			}
		}
		EventResult::Continue
	}
	/// Dispatch after_flush event to all registered listeners for a session
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::{EventRegistry, EventResult};
	///
	/// # tokio_test::block_on(async {
	/// let registry = EventRegistry::new();
	/// let result = registry.dispatch_after_flush("session-1").await;
	/// assert_eq!(result, EventResult::Continue);
	/// # });
	/// ```
	pub async fn dispatch_after_flush(&self, session_id: &str) -> EventResult {
		if let Some(listeners) = self.session_listeners.get(session_id) {
			for listener in listeners.value() {
				listener.after_flush(session_id).await;
			}
		}
		EventResult::Continue
	}
	/// Dispatch before_commit event to all registered listeners for a session
	///
	/// Returns Veto if any listener vetoes the operation
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::{EventRegistry, EventResult};
	///
	/// # tokio_test::block_on(async {
	/// let registry = EventRegistry::new();
	/// let result = registry.dispatch_before_commit("session-1").await;
	/// assert_eq!(result, EventResult::Continue);
	/// # });
	/// ```
	pub async fn dispatch_before_commit(&self, session_id: &str) -> EventResult {
		if let Some(listeners) = self.session_listeners.get(session_id) {
			for listener in listeners.value() {
				match listener.before_commit(session_id).await {
					EventResult::Veto => return EventResult::Veto,
					EventResult::Continue => continue,
				}
			}
		}
		EventResult::Continue
	}
	/// Dispatch after_commit event to all registered listeners for a session
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::{EventRegistry, EventResult};
	///
	/// # tokio_test::block_on(async {
	/// let registry = EventRegistry::new();
	/// let result = registry.dispatch_after_commit("session-1").await;
	/// assert_eq!(result, EventResult::Continue);
	/// # });
	/// ```
	pub async fn dispatch_after_commit(&self, session_id: &str) -> EventResult {
		if let Some(listeners) = self.session_listeners.get(session_id) {
			for listener in listeners.value() {
				listener.after_commit(session_id).await;
			}
		}
		EventResult::Continue
	}
	/// Dispatch after_rollback event to all registered listeners for a session
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::{EventRegistry, EventResult};
	///
	/// # tokio_test::block_on(async {
	/// let registry = EventRegistry::new();
	/// let result = registry.dispatch_after_rollback("session-1").await;
	/// assert_eq!(result, EventResult::Continue);
	/// # });
	/// ```
	pub async fn dispatch_after_rollback(&self, session_id: &str) -> EventResult {
		if let Some(listeners) = self.session_listeners.get(session_id) {
			for listener in listeners.value() {
				listener.after_rollback(session_id).await;
			}
		}
		EventResult::Continue
	}
	/// Dispatch attribute set event to all registered listeners for a model attribute
	///
	/// Returns Veto if any listener vetoes the operation
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::{EventRegistry, EventResult};
	/// use serde_json::json;
	///
	/// # tokio_test::block_on(async {
	/// let registry = EventRegistry::new();
	/// let new_value = json!("new@example.com");
	/// let old_value = json!("old@example.com");
	/// let result = registry.dispatch_attribute_set(
	///     "User.email",
	///     "user-1",
	///     "email",
	///     &new_value,
	///     Some(&old_value)
	/// ).await;
	/// assert_eq!(result, EventResult::Continue);
	/// # });
	/// ```
	pub async fn dispatch_attribute_set(
		&self,
		model_attr: &str,
		instance_id: &str,
		attribute: &str,
		value: &JsonValue,
		old_value: Option<&JsonValue>,
	) -> EventResult {
		if let Some(listeners) = self.attribute_listeners.get(model_attr) {
			for listener in listeners.value() {
				match listener.set(instance_id, attribute, value, old_value).await {
					EventResult::Veto => return EventResult::Veto,
					EventResult::Continue => continue,
				}
			}
		}
		EventResult::Continue
	}
	/// Clear all registered event listeners
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::EventRegistry;
	///
	/// let registry = EventRegistry::new();
	/// registry.clear();
	/// assert_eq!(registry.mapper_listener_count(), 0);
	/// ```
	pub fn clear(&self) {
		self.mapper_listeners.clear();
		self.session_listeners.clear();
		self.attribute_listeners.clear();
		self.instance_listeners.clear();
	}
	/// Get count of registered mapper listeners
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::EventRegistry;
	///
	/// let registry = EventRegistry::new();
	/// assert_eq!(registry.mapper_listener_count(), 0);
	/// ```
	pub fn mapper_listener_count(&self) -> usize {
		self.mapper_listeners.len()
	}
	/// Get count of registered session listeners
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_db::orm::events::EventRegistry;
	///
	/// let registry = EventRegistry::new();
	/// assert_eq!(registry.session_listener_count(), 0);
	/// ```
	pub fn session_listener_count(&self) -> usize {
		self.session_listeners.len()
	}
}

impl Default for EventRegistry {
	fn default() -> Self {
		Self::new()
	}
}

impl Clone for EventRegistry {
	fn clone(&self) -> Self {
		Self {
			mapper_listeners: self.mapper_listeners.clone(),
			session_listeners: self.session_listeners.clone(),
			attribute_listeners: self.attribute_listeners.clone(),
			instance_listeners: self.instance_listeners.clone(),
		}
	}
}

// Thread-local storage for the active EventRegistry within a scope.
thread_local! {
	static ACTIVE_REGISTRY: RefCell<Option<Arc<EventRegistry>>> = const { RefCell::new(None) };
}

/// Sets the active EventRegistry for the duration of the given closure.
///
/// This allows model operations to access the EventRegistry without requiring
/// explicit parameter passing.
///
/// # Examples
///
/// ```
/// use reinhardt_db::orm::events::{EventRegistry, with_event_registry};
/// use std::sync::Arc;
///
/// let registry = Arc::new(EventRegistry::new());
/// with_event_registry(registry.clone(), || {
///     // Model operations within this block will use this registry
/// });
/// ```
pub fn with_event_registry<F, R>(registry: Arc<EventRegistry>, f: F) -> R
where
	F: FnOnce() -> R,
{
	ACTIVE_REGISTRY.with(|r| {
		let prev = r.borrow_mut().replace(registry);
		let result = f();
		*r.borrow_mut() = prev;
		result
	})
}

/// Gets the currently active EventRegistry, if one has been set.
///
/// Returns `None` if no registry has been set via `with_event_registry`.
///
/// # Examples
///
/// ```
/// use reinhardt_db::orm::events::{EventRegistry, get_active_registry, with_event_registry};
/// use std::sync::Arc;
///
/// // Outside any scope, returns None
/// assert!(get_active_registry().is_none());
///
/// let registry = Arc::new(EventRegistry::new());
/// with_event_registry(registry.clone(), || {
///     // Inside the scope, returns the active registry
///     assert!(get_active_registry().is_some());
/// });
/// ```
pub fn get_active_registry() -> Option<Arc<EventRegistry>> {
	ACTIVE_REGISTRY.with(|r| r.borrow().clone())
}

/// RAII guard for active EventRegistry scope.
///
/// When the guard is dropped, the previous registry (if any) is restored.
/// This is useful for async code where the closure-based approach doesn't work.
///
/// # Examples
///
/// ```
/// use reinhardt_db::orm::events::{EventRegistry, set_active_registry, get_active_registry};
/// use std::sync::Arc;
///
/// async fn example() {
///     let registry = Arc::new(EventRegistry::new());
///     let _guard = set_active_registry(registry.clone());
///
///     // Registry is active for the lifetime of the guard
///     assert!(get_active_registry().is_some());
///
///     // Async code works correctly
///     tokio::task::yield_now().await;
///     assert!(get_active_registry().is_some());
/// } // Guard dropped here, registry cleared
/// ```
pub struct ActiveRegistryGuard {
	prev: Option<Arc<EventRegistry>>,
}

impl Drop for ActiveRegistryGuard {
	fn drop(&mut self) {
		ACTIVE_REGISTRY.with(|r| {
			*r.borrow_mut() = self.prev.take();
		});
	}
}

/// Sets the active EventRegistry and returns a guard.
///
/// The guard restores the previous registry (if any) when dropped.
/// This is the preferred method for async code.
///
/// # Examples
///
/// ```
/// use reinhardt_db::orm::events::{EventRegistry, set_active_registry, get_active_registry};
/// use std::sync::Arc;
///
/// let registry = Arc::new(EventRegistry::new());
/// {
///     let _guard = set_active_registry(registry.clone());
///     assert!(get_active_registry().is_some());
/// } // Guard dropped, registry cleared
/// assert!(get_active_registry().is_none());
/// ```
pub fn set_active_registry(registry: Arc<EventRegistry>) -> ActiveRegistryGuard {
	let prev = ACTIVE_REGISTRY.with(|r| r.borrow_mut().replace(registry));
	ActiveRegistryGuard { prev }
}

/// Dependency Injection support for EventRegistry.
///
/// When the `di` feature is enabled, EventRegistry can be injected
/// from an InjectionContext.
#[cfg(feature = "di")]
mod di_support {
	use super::EventRegistry;
	use async_trait::async_trait;
	use reinhardt_di::{DiResult, Injectable, InjectionContext};

	#[async_trait]
	impl Injectable for EventRegistry {
		async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
			// Check if EventRegistry is available in singleton scope
			if let Some(registry) = ctx.get_singleton::<EventRegistry>() {
				return Ok((*registry).clone());
			}
			// Create a new default instance
			Ok(EventRegistry::new())
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::atomic::{AtomicUsize, Ordering};

	struct TestMapperListener {
		before_insert_count: Arc<AtomicUsize>,
		after_insert_count: Arc<AtomicUsize>,
		before_update_count: Arc<AtomicUsize>,
		should_veto: bool,
	}

	#[async_trait]
	impl MapperEvents for TestMapperListener {
		async fn before_insert(&self, _instance_id: &str, _values: &JsonValue) -> EventResult {
			self.before_insert_count.fetch_add(1, Ordering::SeqCst);
			if self.should_veto {
				EventResult::Veto
			} else {
				EventResult::Continue
			}
		}

		async fn after_insert(&self, _instance_id: &str) -> EventResult {
			self.after_insert_count.fetch_add(1, Ordering::SeqCst);
			EventResult::Continue
		}

		async fn before_update(&self, _instance_id: &str, _values: &JsonValue) -> EventResult {
			self.before_update_count.fetch_add(1, Ordering::SeqCst);
			EventResult::Continue
		}
	}

	struct TestSessionListener {
		before_flush_count: Arc<AtomicUsize>,
		after_commit_count: Arc<AtomicUsize>,
	}

	#[async_trait]
	impl SessionEvents for TestSessionListener {
		async fn before_flush(&self, _session_id: &str, _instances: &[String]) -> EventResult {
			self.before_flush_count.fetch_add(1, Ordering::SeqCst);
			EventResult::Continue
		}

		async fn after_commit(&self, _session_id: &str) -> EventResult {
			self.after_commit_count.fetch_add(1, Ordering::SeqCst);
			EventResult::Continue
		}
	}

	#[tokio::test]
	async fn test_mapper_event_dispatch() {
		let registry = EventRegistry::new();

		let before_insert = Arc::new(AtomicUsize::new(0));
		let after_insert = Arc::new(AtomicUsize::new(0));
		let before_update = Arc::new(AtomicUsize::new(0));

		let listener = Arc::new(TestMapperListener {
			before_insert_count: before_insert.clone(),
			after_insert_count: after_insert.clone(),
			before_update_count: before_update.clone(),
			should_veto: false,
		});

		registry.register_mapper_listener("User".to_string(), listener);

		let values = serde_json::json!({"name": "John"});

		let result = registry
			.dispatch_before_insert("User", "user-1", &values)
			.await;
		assert_eq!(result, EventResult::Continue);
		assert_eq!(before_insert.load(Ordering::SeqCst), 1);

		registry.dispatch_after_insert("User", "user-1").await;
		assert_eq!(after_insert.load(Ordering::SeqCst), 1);

		let update_values = serde_json::json!({"name": "Jane"});
		registry
			.dispatch_before_update("User", "user-1", &update_values)
			.await;
		assert_eq!(before_update.load(Ordering::SeqCst), 1);
	}

	#[tokio::test]
	async fn test_mapper_event_veto() {
		let registry = EventRegistry::new();

		let before_insert = Arc::new(AtomicUsize::new(0));

		let listener = Arc::new(TestMapperListener {
			before_insert_count: before_insert.clone(),
			after_insert_count: Arc::new(AtomicUsize::new(0)),
			before_update_count: Arc::new(AtomicUsize::new(0)),
			should_veto: true,
		});

		registry.register_mapper_listener("User".to_string(), listener);

		let values = serde_json::json!({"name": "John"});

		let result = registry
			.dispatch_before_insert("User", "user-1", &values)
			.await;
		assert_eq!(result, EventResult::Veto);
		assert_eq!(before_insert.load(Ordering::SeqCst), 1);
	}

	#[tokio::test]
	async fn test_session_event_dispatch() {
		let registry = EventRegistry::new();

		let before_flush = Arc::new(AtomicUsize::new(0));
		let after_commit = Arc::new(AtomicUsize::new(0));

		let listener = Arc::new(TestSessionListener {
			before_flush_count: before_flush.clone(),
			after_commit_count: after_commit.clone(),
		});

		registry.register_session_listener("session-1".to_string(), listener);

		let instances = vec!["user-1".to_string(), "user-2".to_string()];

		registry
			.dispatch_before_flush("session-1", &instances)
			.await;
		assert_eq!(before_flush.load(Ordering::SeqCst), 1);

		registry.dispatch_after_commit("session-1").await;
		assert_eq!(after_commit.load(Ordering::SeqCst), 1);
	}

	#[tokio::test]
	async fn test_orm_events_multiple_listeners() {
		let registry = EventRegistry::new();

		let count1 = Arc::new(AtomicUsize::new(0));
		let count2 = Arc::new(AtomicUsize::new(0));

		let listener1 = Arc::new(TestMapperListener {
			before_insert_count: count1.clone(),
			after_insert_count: Arc::new(AtomicUsize::new(0)),
			before_update_count: Arc::new(AtomicUsize::new(0)),
			should_veto: false,
		});

		let listener2 = Arc::new(TestMapperListener {
			before_insert_count: count2.clone(),
			after_insert_count: Arc::new(AtomicUsize::new(0)),
			before_update_count: Arc::new(AtomicUsize::new(0)),
			should_veto: false,
		});

		registry.register_mapper_listener("User".to_string(), listener1);
		registry.register_mapper_listener("User".to_string(), listener2);

		let values = serde_json::json!({"name": "John"});

		registry
			.dispatch_before_insert("User", "user-1", &values)
			.await;

		assert_eq!(count1.load(Ordering::SeqCst), 1);
		assert_eq!(count2.load(Ordering::SeqCst), 1);
	}

	#[tokio::test]
	async fn test_clear_listeners() {
		let registry = EventRegistry::new();

		let listener = Arc::new(TestMapperListener {
			before_insert_count: Arc::new(AtomicUsize::new(0)),
			after_insert_count: Arc::new(AtomicUsize::new(0)),
			before_update_count: Arc::new(AtomicUsize::new(0)),
			should_veto: false,
		});

		registry.register_mapper_listener("User".to_string(), listener);
		assert_eq!(registry.mapper_listener_count(), 1);

		registry.clear();
		assert_eq!(registry.mapper_listener_count(), 0);
	}

	#[tokio::test]
	async fn test_active_registry_scoping() {
		// Outside any scope, get_active_registry returns None
		assert!(get_active_registry().is_none());

		let registry = Arc::new(EventRegistry::new());

		let count = Arc::new(AtomicUsize::new(0));

		let listener = Arc::new(TestMapperListener {
			before_insert_count: count.clone(),
			after_insert_count: Arc::new(AtomicUsize::new(0)),
			before_update_count: Arc::new(AtomicUsize::new(0)),
			should_veto: false,
		});

		registry.register_mapper_listener("ScopedTest".to_string(), listener);

		// Use with_event_registry to set the active registry
		with_event_registry(registry.clone(), || {
			// Inside the scope, get_active_registry returns the registry
			let active = get_active_registry();
			assert!(active.is_some());

			// The active registry is the same one we set
			assert_eq!(active.unwrap().mapper_listener_count(), 1);
		});

		// After the scope, get_active_registry returns None again
		assert!(get_active_registry().is_none());
	}

	#[tokio::test]
	async fn test_nested_registry_scoping() {
		let outer_registry = Arc::new(EventRegistry::new());
		let inner_registry = Arc::new(EventRegistry::new());

		// Register listeners in outer registry
		let outer_listener = Arc::new(TestMapperListener {
			before_insert_count: Arc::new(AtomicUsize::new(0)),
			after_insert_count: Arc::new(AtomicUsize::new(0)),
			before_update_count: Arc::new(AtomicUsize::new(0)),
			should_veto: false,
		});
		outer_registry.register_mapper_listener("OuterModel".to_string(), outer_listener);

		// Register different listeners in inner registry
		let inner_listener = Arc::new(TestMapperListener {
			before_insert_count: Arc::new(AtomicUsize::new(0)),
			after_insert_count: Arc::new(AtomicUsize::new(0)),
			before_update_count: Arc::new(AtomicUsize::new(0)),
			should_veto: false,
		});
		inner_registry.register_mapper_listener("InnerModel".to_string(), inner_listener);

		with_event_registry(outer_registry.clone(), || {
			assert_eq!(get_active_registry().unwrap().mapper_listener_count(), 1);

			// Nested scope replaces the active registry
			with_event_registry(inner_registry.clone(), || {
				assert_eq!(get_active_registry().unwrap().mapper_listener_count(), 1);
			});

			// After inner scope, outer registry is restored
			assert_eq!(get_active_registry().unwrap().mapper_listener_count(), 1);
		});
	}
}
