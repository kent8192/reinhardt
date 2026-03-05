//! Injection context for dependency resolution

use crate::function_handle::FunctionHandle;
use crate::override_registry::OverrideRegistry;
use crate::scope::{RequestScope, SingletonScope};
use std::any::Any;
use std::sync::Arc;

// Re-export ParamContext and Request types for convenience
#[cfg(feature = "params")]
pub use crate::params::{ParamContext, Request};

/// The main injection context for dependency resolution.
///
/// `InjectionContext` manages both request-scoped and singleton-scoped dependencies,
/// as well as dependency overrides for testing.
///
/// # Override Support
///
/// The context supports dependency overrides, which take precedence over normal
/// dependency resolution. This is particularly useful for testing:
///
/// ```rust,no_run
/// use reinhardt_di::{InjectionContext, SingletonScope};
/// use std::sync::Arc;
///
/// # #[derive(Clone)]
/// # struct Database;
/// # impl Database {
/// #     fn connect(_url: &str) -> Self { Database }
/// #     fn mock() -> Self { Database }
/// # }
/// # fn create_database() -> Database { Database::connect("production://db") }
///
/// let singleton = Arc::new(SingletonScope::new());
/// let ctx = InjectionContext::builder(singleton).build();
///
/// // Set override for testing
/// ctx.dependency(create_database).override_with(Database::mock());
/// ```
pub struct InjectionContext {
	request_scope: RequestScope,
	singleton_scope: Arc<SingletonScope>,
	/// Override registry for dependency substitution (e.g., for testing)
	override_registry: Arc<OverrideRegistry>,
	/// HTTP request for parameter extraction
	#[cfg(feature = "params")]
	request: Option<Arc<Request>>,
	/// Parameter context for path/header/cookie extraction
	#[cfg(feature = "params")]
	param_context: Option<Arc<ParamContext>>,
}

/// Builder for constructing `InjectionContext` instances.
///
/// Provides a fluent API for building injection contexts with optional HTTP request support.
///
/// # Examples
///
/// ```
/// use reinhardt_di::{InjectionContext, SingletonScope};
///
/// let singleton_scope = SingletonScope::new();
/// let ctx = InjectionContext::builder(singleton_scope).build();
/// ```
pub struct InjectionContextBuilder {
	singleton_scope: Arc<SingletonScope>,
	#[cfg(feature = "params")]
	request: Option<Request>,
	#[cfg(feature = "params")]
	param_context: Option<ParamContext>,
}

impl InjectionContextBuilder {
	/// Set the HTTP request for this context.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_di::{InjectionContext, SingletonScope, Request};
	///
	/// let singleton_scope = SingletonScope::new();
	/// let request = Request::builder()
	///     .method(hyper::Method::GET)
	///     .uri("/")
	///     .build()
	///     .unwrap();
	///
	/// let ctx = InjectionContext::builder(singleton_scope)
	///     .with_request(request)
	///     .build();
	/// ```
	#[cfg(feature = "params")]
	pub fn with_request(mut self, request: Request) -> Self {
		self.request = Some(request);
		self
	}

	/// Set the parameter context for this context.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_di::{InjectionContext, SingletonScope, ParamContext};
	///
	/// let singleton_scope = SingletonScope::new();
	/// let param_context = ParamContext::new();
	///
	/// let ctx = InjectionContext::builder(singleton_scope)
	///     .with_param_context(param_context)
	///     .build();
	/// ```
	#[cfg(feature = "params")]
	pub fn with_param_context(mut self, param_context: ParamContext) -> Self {
		self.param_context = Some(param_context);
		self
	}

	/// Register a singleton instance in the context.
	///
	/// This allows explicit registration of pre-configured instances
	/// that will be shared across all requests.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{InjectionContext, SingletonScope};
	/// use std::sync::Arc;
	///
	/// #[derive(Debug, Clone)]
	/// struct DatabaseConfig {
	///     url: String,
	/// }
	///
	/// let singleton_scope = Arc::new(SingletonScope::new());
	/// let config = DatabaseConfig { url: "postgres://localhost".to_string() };
	///
	/// let ctx = InjectionContext::builder(singleton_scope)
	///     .singleton(config)
	///     .build();
	/// ```
	pub fn singleton<T: std::any::Any + Send + Sync>(self, instance: T) -> Self {
		self.singleton_scope.set(instance);
		self
	}

	/// Build the final `InjectionContext` instance.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{InjectionContext, SingletonScope};
	///
	/// let singleton_scope = SingletonScope::new();
	/// let ctx = InjectionContext::builder(singleton_scope).build();
	/// ```
	pub fn build(self) -> InjectionContext {
		InjectionContext {
			request_scope: RequestScope::new(),
			singleton_scope: self.singleton_scope,
			override_registry: Arc::new(OverrideRegistry::new()),
			#[cfg(feature = "params")]
			request: self.request.map(Arc::new),
			#[cfg(feature = "params")]
			param_context: self.param_context.map(Arc::new),
		}
	}
}

impl Clone for InjectionContext {
	fn clone(&self) -> Self {
		Self {
			request_scope: self.request_scope.deep_clone(),
			singleton_scope: Arc::clone(&self.singleton_scope),
			override_registry: Arc::clone(&self.override_registry),
			#[cfg(feature = "params")]
			request: self.request.clone(),
			#[cfg(feature = "params")]
			param_context: self.param_context.clone(),
		}
	}
}

impl InjectionContext {
	/// Create a new `InjectionContextBuilder`.
	///
	/// This is the recommended way to construct an `InjectionContext`.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{InjectionContext, SingletonScope};
	///
	/// let singleton_scope = SingletonScope::new();
	/// let ctx = InjectionContext::builder(singleton_scope).build();
	/// ```
	pub fn builder(singleton_scope: impl Into<Arc<SingletonScope>>) -> InjectionContextBuilder {
		InjectionContextBuilder {
			singleton_scope: singleton_scope.into(),
			#[cfg(feature = "params")]
			request: None,
			#[cfg(feature = "params")]
			param_context: None,
		}
	}

	/// Gets the HTTP request from the context.
	///
	/// Returns `None` if no request was set (e.g., when testing without HTTP context).
	/// Returns a reference to the request.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_di::{InjectionContext, SingletonScope, Request, ParamContext};
	///
	/// let singleton_scope = SingletonScope::new();
	/// let request = Request::builder()
	///     .method(hyper::Method::GET)
	///     .uri("/")
	///     .build()
	///     .unwrap();
	/// let param_context = ParamContext::new();
	///
	/// let ctx = InjectionContext::builder(singleton_scope)
	///     .with_request(request)
	///     .with_param_context(param_context)
	///     .build();
	///
	/// assert!(ctx.get_http_request().is_some());
	/// ```
	#[cfg(feature = "params")]
	pub fn get_http_request(&self) -> Option<&Request> {
		self.request.as_ref().map(|arc| arc.as_ref())
	}

	/// Gets the parameter context from the context.
	///
	/// Returns `None` if no parameter context was set.
	/// Returns a reference to the parameter context.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_di::{InjectionContext, SingletonScope, Request, ParamContext};
	///
	/// let singleton_scope = SingletonScope::new();
	/// let request = Request::builder()
	///     .method(hyper::Method::GET)
	///     .uri("/")
	///     .build()
	///     .unwrap();
	/// let param_context = ParamContext::new();
	///
	/// let ctx = InjectionContext::builder(singleton_scope)
	///     .with_request(request)
	///     .with_param_context(param_context)
	///     .build();
	///
	/// assert!(ctx.get_param_context().is_some());
	/// ```
	#[cfg(feature = "params")]
	pub fn get_param_context(&self) -> Option<&ParamContext> {
		self.param_context.as_ref().map(|arc| arc.as_ref())
	}

	/// Sets the HTTP request and parameter context.
	///
	/// This can be used to add HTTP context to an existing InjectionContext.
	/// The request and parameter context will be wrapped in Arc internally.
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_di::{InjectionContext, SingletonScope, Request, ParamContext};
	///
	/// let singleton_scope = SingletonScope::new();
	/// let mut ctx = InjectionContext::builder(singleton_scope).build();
	///
	/// let request = Request::builder()
	///     .method(hyper::Method::GET)
	///     .uri("/")
	///     .build()
	///     .unwrap();
	/// let param_context = ParamContext::new();
	///
	/// ctx.set_http_request(request, param_context);
	/// ```
	#[cfg(feature = "params")]
	pub fn set_http_request(&mut self, request: Request, param_context: ParamContext) {
		self.request = Some(Arc::new(request));
		self.param_context = Some(Arc::new(param_context));
	}
	/// Retrieves a request-scoped value from the context.
	///
	/// Request-scoped values are cached only for the duration of a single request.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{InjectionContext, SingletonScope};
	/// use std::sync::Arc;
	///
	/// let singleton_scope = Arc::new(SingletonScope::new());
	/// let ctx = InjectionContext::builder(singleton_scope).build();
	///
	/// ctx.set_request(42i32);
	/// let value = ctx.get_request::<i32>().unwrap();
	/// assert_eq!(*value, 42);
	/// ```
	pub fn get_request<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
		self.request_scope.get::<T>()
	}
	/// Stores a value in the request scope.
	///
	/// The value is cached for the duration of the current request only.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{InjectionContext, SingletonScope};
	/// use std::sync::Arc;
	///
	/// let singleton_scope = Arc::new(SingletonScope::new());
	/// let ctx = InjectionContext::builder(singleton_scope).build();
	///
	/// ctx.set_request("request-data".to_string());
	/// assert!(ctx.get_request::<String>().is_some());
	/// ```
	pub fn set_request<T: Any + Send + Sync>(&self, value: T) {
		self.request_scope.set(value);
	}

	/// Stores a pre-wrapped `Arc<T>` in the request scope.
	///
	/// This avoids the need to unwrap and re-wrap Arc values that are
	/// already in Arc form, such as those returned by factory functions.
	fn set_request_arc<T: Any + Send + Sync>(&self, value: Arc<T>) {
		self.request_scope.set_arc(value);
	}
	/// Retrieves a singleton value from the context.
	///
	/// Singleton values persist across all requests and are shared application-wide.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{InjectionContext, SingletonScope};
	/// use std::sync::Arc;
	///
	/// let singleton_scope = Arc::new(SingletonScope::new());
	/// singleton_scope.set(100u64);
	///
	/// let ctx = InjectionContext::builder(singleton_scope).build();
	/// let value = ctx.get_singleton::<u64>().unwrap();
	/// assert_eq!(*value, 100);
	/// ```
	pub fn get_singleton<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
		self.singleton_scope.get::<T>()
	}
	/// Stores a value in the singleton scope.
	///
	/// The value persists across all requests and is shared application-wide.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{InjectionContext, SingletonScope};
	/// use std::sync::Arc;
	///
	/// let singleton_scope = Arc::new(SingletonScope::new());
	/// let ctx = InjectionContext::builder(singleton_scope).build();
	///
	/// ctx.set_singleton("global-config".to_string());
	/// assert!(ctx.get_singleton::<String>().is_some());
	/// ```
	pub fn set_singleton<T: Any + Send + Sync>(&self, value: T) {
		self.singleton_scope.set(value);
	}

	/// Stores a pre-wrapped `Arc<T>` in the singleton scope.
	///
	/// This avoids the need to unwrap and re-wrap Arc values that are
	/// already in Arc form, such as those returned by factory functions.
	fn set_singleton_arc<T: Any + Send + Sync>(&self, value: Arc<T>) {
		self.singleton_scope.set_arc(value);
	}

	/// Returns a reference to the singleton scope.
	///
	/// This is useful for advanced scenarios where direct access to the
	/// singleton scope is needed.
	pub fn singleton_scope(&self) -> &Arc<SingletonScope> {
		&self.singleton_scope
	}

	/// Returns a reference to the override registry.
	///
	/// The override registry stores function-level overrides that take
	/// precedence over normal dependency resolution.
	pub fn overrides(&self) -> &OverrideRegistry {
		&self.override_registry
	}

	/// Creates a handle for the given injectable function.
	///
	/// This method provides a fluent API for setting and managing dependency
	/// overrides. The function pointer is used as a unique key to identify
	/// which injectable function should be overridden.
	///
	/// # Note
	///
	/// This method is designed to work with functions annotated with `#[injectable]`.
	/// The `#[injectable]` macro generates a 0-argument function regardless of
	/// the original function's parameter count, as all `#[inject]` parameters
	/// are resolved internally by the DI system.
	///
	/// # Type Parameters
	///
	/// * `O` - The output type of the function (the dependency type)
	///
	/// # Arguments
	///
	/// * `func` - A function pointer to the injectable function
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_di::{InjectionContext, SingletonScope};
	/// use std::sync::Arc;
	///
	/// # #[derive(Clone)]
	/// # struct Database;
	/// # impl Database {
	/// #     fn connect(_url: &str) -> Self { Database }
	/// #     fn mock() -> Self { Database }
	/// # }
	/// # struct Config { url: String }
	/// # fn create_database() -> Database {
	/// #     Database::connect("production://db")
	/// # }
	///
	/// let singleton = Arc::new(SingletonScope::new());
	/// let ctx = InjectionContext::builder(singleton).build();
	///
	/// // Set override - create_database is 0-argument after macro expansion
	/// ctx.dependency(create_database).override_with(Database::mock());
	///
	/// // Check if override exists
	/// assert!(ctx.dependency(create_database).has_override());
	///
	/// // Clear override
	/// ctx.dependency(create_database).clear_override();
	/// ```
	pub fn dependency<O>(&self, func: fn() -> O) -> FunctionHandle<'_, O>
	where
		O: Clone + Send + Sync + 'static,
	{
		let func_ptr = func as usize;
		FunctionHandle::new(self, func_ptr)
	}

	/// Gets an override value for a function pointer.
	///
	/// This is primarily used internally by the `#[injectable]` macro to check
	/// for overrides before executing the actual function.
	///
	/// # Arguments
	///
	/// * `func_ptr` - The function pointer address as usize
	///
	/// # Returns
	///
	/// `Some(value)` if an override is set, `None` otherwise.
	pub fn get_override<O: Clone + 'static>(&self, func_ptr: usize) -> Option<O> {
		self.override_registry.get(func_ptr)
	}

	/// Clears all overrides from the context.
	///
	/// This is useful for cleanup in tests to ensure a clean state.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_di::{InjectionContext, SingletonScope};
	/// use std::sync::Arc;
	///
	/// fn my_factory() -> i32 { 42 }
	///
	/// let singleton = Arc::new(SingletonScope::new());
	/// let ctx = InjectionContext::builder(singleton).build();
	///
	/// ctx.dependency(my_factory).override_with(100);
	/// assert!(ctx.dependency(my_factory).has_override());
	///
	/// ctx.clear_overrides();
	/// assert!(!ctx.dependency(my_factory).has_override());
	/// ```
	pub fn clear_overrides(&self) {
		self.override_registry.clear();
	}

	/// Resolve a dependency from the global registry
	///
	/// This method implements the core dependency resolution logic:
	/// 1. Check cache based on scope (Request or Singleton)
	/// 2. If not cached, create using the factory from the global registry
	/// 3. Cache the result according to the scope
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_di::{InjectionContext, SingletonScope};
	/// use std::sync::Arc;
	/// # use async_trait::async_trait;
	///
	/// # #[derive(Clone)]
	/// # struct Config;
	/// # #[async_trait]
	/// # impl reinhardt_di::Injectable for Config {
	/// #     async fn inject(_ctx: &InjectionContext) -> reinhardt_di::DiResult<Self> {
	/// #         Ok(Config)
	/// #     }
	/// # }
	/// # async fn example() -> reinhardt_di::DiResult<()> {
	/// let singleton_scope = Arc::new(SingletonScope::new());
	/// let ctx = InjectionContext::builder(singleton_scope).build();
	///
	/// let config = ctx.resolve::<Config>().await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn resolve<T: Any + Send + Sync + 'static>(&self) -> crate::DiResult<Arc<T>> {
		use crate::cycle_detection::{
			begin_resolution, register_type_name, with_cycle_detection_scope,
		};
		use crate::registry::{DependencyScope, global_registry};

		with_cycle_detection_scope(async {
			let type_id = std::any::TypeId::of::<T>();
			let type_name = std::any::type_name::<T>();
			let registry = global_registry();

			// Register type name (for error messages)
			register_type_name::<T>(type_name);

			// [Fast path] Skip circular detection on cache hit
			let scope = registry.get_scope::<T>().ok_or_else(|| {
				crate::DiError::DependencyNotRegistered {
					type_name: type_name.to_string(),
				}
			})?;
			match scope {
				DependencyScope::Singleton => {
					if let Some(cached) = self.get_singleton::<T>() {
						return Ok(cached); // < 5% overhead
					}
				}
				DependencyScope::Request => {
					if let Some(cached) = self.get_request::<T>() {
						return Ok(cached); // < 5% overhead
					}
				}
				_ => {}
			}

			// [Slow path] Execute circular detection only on cache miss
			let _guard = begin_resolution(type_id, type_name)
				.map_err(|e| crate::DiError::CircularDependency(e.to_string()))?;

			// Actual resolution processing (existing logic)
			self.resolve_internal::<T>(scope).await
			// Guard is automatically cleaned up when dropped
		})
		.await
	}

	async fn resolve_internal<T: Any + Send + Sync + 'static>(
		&self,
		scope: crate::registry::DependencyScope,
	) -> crate::DiResult<Arc<T>> {
		use crate::registry::{DependencyScope, global_registry};

		let registry = global_registry();

		match scope {
			DependencyScope::Singleton => {
				// Create new instance
				let instance = registry.create::<T>(self).await?;

				// Cache the Arc directly in singleton scope without unwrapping.
				// This avoids panics when the factory retains an Arc clone,
				// which causes Arc::try_unwrap to fail.
				self.set_singleton_arc(Arc::clone(&instance));
				Ok(instance)
			}
			DependencyScope::Request => {
				// Create new instance
				let instance = registry.create::<T>(self).await?;

				// Cache the Arc directly in request scope without unwrapping.
				// This avoids panics when the factory retains an Arc clone.
				self.set_request_arc(Arc::clone(&instance));
				Ok(instance)
			}
			DependencyScope::Transient => {
				// Never cache, always create new
				registry.create::<T>(self).await
			}
		}
	}
}

pub struct RequestContext {
	injection_ctx: InjectionContext,
}

impl RequestContext {
	/// Creates a new RequestContext with a shared singleton scope.
	///
	/// This is typically used to create a context for each incoming request.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{RequestContext, SingletonScope};
	///
	/// let singleton_scope = SingletonScope::new();
	/// let request_ctx = RequestContext::new(singleton_scope);
	/// ```
	pub fn new(singleton_scope: SingletonScope) -> Self {
		Self {
			injection_ctx: InjectionContext::builder(singleton_scope).build(),
		}
	}
	/// Returns a reference to the underlying injection context.
	///
	/// This allows access to the dependency injection context for resolving dependencies.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_di::{RequestContext, SingletonScope};
	///
	/// let singleton_scope = SingletonScope::new();
	/// let request_ctx = RequestContext::new(singleton_scope);
	///
	/// let ctx = request_ctx.injection_context();
	/// ctx.set_request(42i32);
	/// ```
	pub fn injection_context(&self) -> &InjectionContext {
		&self.injection_ctx
	}
}
