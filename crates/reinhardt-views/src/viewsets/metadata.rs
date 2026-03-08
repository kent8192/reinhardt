use async_trait::async_trait;
use hyper::Method;
use reinhardt_http::{Request, Response, Result};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Type alias for async handler function
type AsyncHandlerFn =
	dyn Fn(Request) -> Pin<Box<dyn Future<Output = Result<Response>> + Send>> + Send + Sync;

/// Custom action handler trait
#[async_trait]
pub trait ActionHandler: Send + Sync {
	/// Handle the incoming request and return a response.
	async fn handle(&self, request: Request) -> Result<Response>;
}

/// Function pointer-based ActionHandler implementation
pub struct FunctionActionHandler {
	handler: Arc<AsyncHandlerFn>,
}

impl FunctionActionHandler {
	/// Create a new `FunctionActionHandler` wrapping the given async function.
	pub fn new<F>(handler: F) -> Self
	where
		F: Fn(Request) -> Pin<Box<dyn Future<Output = Result<Response>> + Send>>
			+ Send
			+ Sync
			+ 'static,
	{
		Self {
			handler: Arc::new(handler),
		}
	}
}

#[async_trait]
impl ActionHandler for FunctionActionHandler {
	async fn handle(&self, request: Request) -> Result<Response> {
		(self.handler)(request).await
	}
}

/// Action metadata
pub struct ActionMetadata {
	/// Function name (default identifier)
	pub name: String,

	/// Whether this is a detail action (single object) or list action
	pub detail: bool,

	/// Custom display name
	pub custom_name: Option<String>,

	/// Custom suffix
	pub suffix: Option<String>,

	/// Custom URL path
	pub url_path: Option<String>,

	/// Custom URL name (for reverse routing)
	pub url_name: Option<String>,

	/// Allowed HTTP methods
	pub methods: Vec<Method>,

	/// Actual handler function
	pub handler: Arc<dyn ActionHandler>,
}

impl ActionMetadata {
	/// Create a new ActionMetadata
	pub fn new(name: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			detail: false,
			custom_name: None,
			suffix: None,
			url_path: None,
			url_name: None,
			methods: vec![Method::GET],
			handler: Arc::new(FunctionActionHandler::new(|_| {
				Box::pin(async { Response::ok().with_json(&serde_json::json!({})) })
			})),
		}
	}

	/// Set as detail action
	pub fn with_detail(mut self, detail: bool) -> Self {
		self.detail = detail;
		self
	}

	/// Set custom name
	pub fn with_custom_name(mut self, name: impl Into<String>) -> Self {
		self.custom_name = Some(name.into());
		self
	}

	/// Set suffix
	pub fn with_suffix(mut self, suffix: impl Into<String>) -> Self {
		self.suffix = Some(suffix.into());
		self
	}

	/// Set URL path
	pub fn with_url_path(mut self, path: impl Into<String>) -> Self {
		self.url_path = Some(path.into());
		self
	}

	/// Set URL name
	pub fn with_url_name(mut self, name: impl Into<String>) -> Self {
		self.url_name = Some(name.into());
		self
	}

	/// Set HTTP methods
	pub fn with_methods(mut self, methods: Vec<Method>) -> Self {
		self.methods = methods;
		self
	}

	/// Set handler
	pub fn with_handler<H: ActionHandler + 'static>(mut self, handler: H) -> Self {
		self.handler = Arc::new(handler);
		self
	}

	/// Get display name (priority: custom_name > name + suffix > name)
	pub fn display_name(&self) -> String {
		if let Some(ref custom_name) = self.custom_name {
			custom_name.clone()
		} else if let Some(ref suffix) = self.suffix {
			format!("{} {}", self.format_name(&self.name), suffix)
		} else {
			self.format_name(&self.name)
		}
	}

	/// Get URL name (priority: url_name > name)
	pub fn get_url_name(&self) -> String {
		self.url_name
			.clone()
			.unwrap_or_else(|| self.name.replace('_', "-"))
	}

	/// Get URL path (priority: url_path > default generation)
	pub fn get_url_path(&self) -> String {
		self.url_path
			.clone()
			.unwrap_or_else(|| self.name.replace('_', "-"))
	}

	/// Convert snake_case to Title Case
	fn format_name(&self, name: &str) -> String {
		name.split('_')
			.map(|word| {
				let mut chars = word.chars();
				match chars.next() {
					Some(first) => first.to_uppercase().chain(chars).collect(),
					None => String::new(),
				}
			})
			.collect::<Vec<_>>()
			.join(" ")
	}
}

impl Clone for ActionMetadata {
	fn clone(&self) -> Self {
		Self {
			name: self.name.clone(),
			detail: self.detail,
			custom_name: self.custom_name.clone(),
			suffix: self.suffix.clone(),
			url_path: self.url_path.clone(),
			url_name: self.url_name.clone(),
			methods: self.methods.clone(),
			handler: self.handler.clone(),
		}
	}
}

impl fmt::Debug for ActionMetadata {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ActionMetadata")
			.field("name", &self.name)
			.field("detail", &self.detail)
			.field("custom_name", &self.custom_name)
			.field("suffix", &self.suffix)
			.field("url_path", &self.url_path)
			.field("url_name", &self.url_name)
			.field("methods", &self.methods)
			.finish()
	}
}

/// Action registry entry (collected by inventory)
pub struct ActionRegistryEntry {
	/// Fully qualified type name of the viewset.
	pub viewset_type: &'static str,
	/// Name of the custom action.
	pub action_name: &'static str,
	/// Function that returns the action metadata.
	pub metadata_fn: fn() -> ActionMetadata,
}

impl ActionRegistryEntry {
	/// Create a new `ActionRegistryEntry`.
	pub const fn new(
		viewset_type: &'static str,
		action_name: &'static str,
		metadata_fn: fn() -> ActionMetadata,
	) -> Self {
		Self {
			viewset_type,
			action_name,
			metadata_fn,
		}
	}
}

inventory::collect!(ActionRegistryEntry);

/// Get actions associated with a ViewSet type
pub fn get_actions_for_viewset(viewset_type: &str) -> Vec<ActionMetadata> {
	inventory::iter::<ActionRegistryEntry>()
		.filter(|entry| entry.viewset_type == viewset_type)
		.map(|entry| (entry.metadata_fn)())
		.collect()
}
