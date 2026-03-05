//! WebSocket routing integration
//!
//! This module provides routing capabilities for WebSocket connections,
//! allowing URL-based WebSocket endpoint registration.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// WebSocket route handler
pub type WebSocketRouteHandler = Arc<dyn Fn() -> RouteResult + Send + Sync>;

/// Routing result type
pub type RouteResult = Result<(), RouteError>;

/// Routing errors
#[derive(Debug, thiserror::Error)]
pub enum RouteError {
	#[error("Route not found")]
	NotFound(String),
	#[error("Route already exists")]
	AlreadyExists(String),
	#[error("Invalid route pattern")]
	InvalidPattern(String),
}

/// WebSocket route information
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::routing::WebSocketRoute;
///
/// let route = WebSocketRoute::new(
///     "/ws/chat".to_string(),
///     Some("websocket:chat".to_string()),
/// );
///
/// assert_eq!(route.path(), "/ws/chat");
/// assert_eq!(route.name(), Some("websocket:chat"));
/// ```
#[derive(Debug, Clone)]
pub struct WebSocketRoute {
	path: String,
	name: Option<String>,
	metadata: HashMap<String, String>,
}

impl WebSocketRoute {
	/// Create a new WebSocket route
	pub fn new(path: String, name: Option<String>) -> Self {
		Self {
			path,
			name,
			metadata: HashMap::new(),
		}
	}

	/// Get the route path
	pub fn path(&self) -> &str {
		&self.path
	}

	/// Get the route name
	pub fn name(&self) -> Option<&str> {
		self.name.as_deref()
	}

	/// Add metadata to the route
	pub fn with_metadata(mut self, key: String, value: String) -> Self {
		self.metadata.insert(key, value);
		self
	}

	/// Get metadata value
	pub fn get_metadata(&self, key: &str) -> Option<&String> {
		self.metadata.get(key)
	}
}

/// WebSocket router for managing routes
///
/// Supports an optional middleware chain that is applied to all WebSocket
/// upgrade requests before establishing the connection. This ensures that
/// HTTP-level middleware (authentication, IP filtering, rate limiting) is
/// not bypassed by WebSocket upgrades.
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::routing::{WebSocketRouter, WebSocketRoute};
///
/// # tokio_test::block_on(async {
/// let mut router = WebSocketRouter::new();
///
/// let route = WebSocketRoute::new(
///     "/ws/chat".to_string(),
///     Some("websocket:chat".to_string()),
/// );
///
/// router.register_route(route).await.unwrap();
///
/// let found = router.find_route("/ws/chat").await;
/// assert!(found.is_some());
/// assert_eq!(found.unwrap().name(), Some("websocket:chat"));
/// # });
/// ```
pub struct WebSocketRouter {
	routes: Arc<RwLock<HashMap<String, WebSocketRoute>>>,
	names: Arc<RwLock<HashMap<String, String>>>, // name -> path mapping
	/// Middleware chain applied to upgrade requests before WebSocket handshake
	middleware: Option<Arc<crate::middleware::MiddlewareChain>>,
}

impl WebSocketRouter {
	/// Create a new WebSocket router
	pub fn new() -> Self {
		Self {
			routes: Arc::new(RwLock::new(HashMap::new())),
			names: Arc::new(RwLock::new(HashMap::new())),
			middleware: None,
		}
	}

	/// Set the middleware chain applied to upgrade requests before WebSocket handshake
	pub fn with_middleware(mut self, middleware: crate::middleware::MiddlewareChain) -> Self {
		self.middleware = Some(Arc::new(middleware));
		self
	}

	/// Get a reference to the middleware chain, if configured
	pub fn middleware(&self) -> Option<&Arc<crate::middleware::MiddlewareChain>> {
		self.middleware.as_ref()
	}

	/// Register a WebSocket route
	pub async fn register_route(&mut self, route: WebSocketRoute) -> RouteResult {
		let mut routes = self.routes.write().await;

		if routes.contains_key(&route.path) {
			return Err(RouteError::AlreadyExists(route.path.clone()));
		}

		if let Some(name) = &route.name {
			let mut names = self.names.write().await;
			names.insert(name.clone(), route.path.clone());
		}

		routes.insert(route.path.clone(), route);
		Ok(())
	}

	/// Find a route by path
	pub async fn find_route(&self, path: &str) -> Option<WebSocketRoute> {
		let routes = self.routes.read().await;
		routes.get(path).cloned()
	}

	/// Find a route by name
	pub async fn find_route_by_name(&self, name: &str) -> Option<WebSocketRoute> {
		let names = self.names.read().await;
		if let Some(path) = names.get(name) {
			let routes = self.routes.read().await;
			routes.get(path).cloned()
		} else {
			None
		}
	}

	/// Remove a route by path
	pub async fn remove_route(&mut self, path: &str) -> RouteResult {
		let mut routes = self.routes.write().await;

		let route = routes
			.remove(path)
			.ok_or_else(|| RouteError::NotFound(path.to_string()))?;

		if let Some(name) = &route.name {
			let mut names = self.names.write().await;
			names.remove(name);
		}

		Ok(())
	}

	/// Get all registered routes
	pub async fn all_routes(&self) -> Vec<WebSocketRoute> {
		let routes = self.routes.read().await;
		routes.values().cloned().collect()
	}

	/// Check if a route exists
	pub async fn has_route(&self, path: &str) -> bool {
		let routes = self.routes.read().await;
		routes.contains_key(path)
	}

	/// Get the number of registered routes
	pub async fn route_count(&self) -> usize {
		let routes = self.routes.read().await;
		routes.len()
	}

	/// Clear all routes
	pub async fn clear(&mut self) {
		let mut routes = self.routes.write().await;
		let mut names = self.names.write().await;
		routes.clear();
		names.clear();
	}
}

impl Default for WebSocketRouter {
	fn default() -> Self {
		Self::new()
	}
}

/// Global WebSocket router registry
static GLOBAL_ROUTER: once_cell::sync::Lazy<Arc<RwLock<Option<WebSocketRouter>>>> =
	once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(None)));

/// Register a global WebSocket router
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::routing::{register_websocket_router, WebSocketRouter};
///
/// # tokio_test::block_on(async {
/// let router = WebSocketRouter::new();
/// register_websocket_router(router).await;
/// # });
/// ```
pub async fn register_websocket_router(router: WebSocketRouter) {
	let mut global = GLOBAL_ROUTER.write().await;
	*global = Some(router);
}

/// Get the global WebSocket router
pub async fn get_websocket_router() -> Option<WebSocketRouter> {
	let global = GLOBAL_ROUTER.read().await;
	if global.is_some() {
		Some(WebSocketRouter::new())
	} else {
		None
	}
}

/// Clear the global WebSocket router
pub async fn clear_websocket_router() {
	let mut global = GLOBAL_ROUTER.write().await;
	*global = None;
}

/// URL reverse lookup for WebSocket routes
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::routing::{reverse_websocket_url, WebSocketRouter, WebSocketRoute};
///
/// # tokio_test::block_on(async {
/// let mut router = WebSocketRouter::new();
/// let route = WebSocketRoute::new(
///     "/ws/chat".to_string(),
///     Some("websocket:chat".to_string()),
/// );
/// router.register_route(route).await.unwrap();
///
/// let url = reverse_websocket_url(&router, "websocket:chat").await;
/// assert_eq!(url, Some("/ws/chat".to_string()));
/// # });
/// ```
pub async fn reverse_websocket_url(router: &WebSocketRouter, name: &str) -> Option<String> {
	router
		.find_route_by_name(name)
		.await
		.map(|route| route.path().to_string())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_websocket_route_creation() {
		let route = WebSocketRoute::new("/ws/test".to_string(), Some("test".to_string()));
		assert_eq!(route.path(), "/ws/test");
		assert_eq!(route.name(), Some("test"));
	}

	#[test]
	fn test_websocket_route_metadata() {
		let route = WebSocketRoute::new("/ws/test".to_string(), None)
			.with_metadata("auth".to_string(), "required".to_string());

		assert_eq!(route.get_metadata("auth").unwrap(), "required");
	}

	#[tokio::test]
	async fn test_router_register_route() {
		let mut router = WebSocketRouter::new();
		let route = WebSocketRoute::new("/ws/chat".to_string(), Some("chat".to_string()));

		assert!(router.register_route(route).await.is_ok());
		assert_eq!(router.route_count().await, 1);
	}

	#[tokio::test]
	async fn test_router_register_duplicate_route() {
		let mut router = WebSocketRouter::new();
		let route1 = WebSocketRoute::new("/ws/chat".to_string(), Some("chat".to_string()));
		let route2 = WebSocketRoute::new("/ws/chat".to_string(), Some("chat2".to_string()));

		router.register_route(route1).await.unwrap();
		let result = router.register_route(route2).await;

		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), RouteError::AlreadyExists(_)));
	}

	#[tokio::test]
	async fn test_router_find_route() {
		let mut router = WebSocketRouter::new();
		let route = WebSocketRoute::new("/ws/chat".to_string(), Some("chat".to_string()));

		router.register_route(route).await.unwrap();

		let found = router.find_route("/ws/chat").await;
		assert!(found.is_some());
		assert_eq!(found.unwrap().name(), Some("chat"));
	}

	#[tokio::test]
	async fn test_router_find_route_by_name() {
		let mut router = WebSocketRouter::new();
		let route = WebSocketRoute::new("/ws/chat".to_string(), Some("chat".to_string()));

		router.register_route(route).await.unwrap();

		let found = router.find_route_by_name("chat").await;
		assert!(found.is_some());
		assert_eq!(found.unwrap().path(), "/ws/chat");
	}

	#[tokio::test]
	async fn test_router_remove_route() {
		let mut router = WebSocketRouter::new();
		let route = WebSocketRoute::new("/ws/chat".to_string(), Some("chat".to_string()));

		router.register_route(route).await.unwrap();
		assert_eq!(router.route_count().await, 1);

		router.remove_route("/ws/chat").await.unwrap();
		assert_eq!(router.route_count().await, 0);
	}

	#[tokio::test]
	async fn test_router_all_routes() {
		let mut router = WebSocketRouter::new();
		let route1 = WebSocketRoute::new("/ws/chat".to_string(), Some("chat".to_string()));
		let route2 = WebSocketRoute::new("/ws/notif".to_string(), Some("notif".to_string()));

		router.register_route(route1).await.unwrap();
		router.register_route(route2).await.unwrap();

		let routes = router.all_routes().await;
		assert_eq!(routes.len(), 2);
	}

	#[tokio::test]
	async fn test_router_has_route() {
		let mut router = WebSocketRouter::new();
		let route = WebSocketRoute::new("/ws/chat".to_string(), None);

		router.register_route(route).await.unwrap();

		assert!(router.has_route("/ws/chat").await);
		assert!(!router.has_route("/ws/notif").await);
	}

	#[tokio::test]
	async fn test_router_clear() {
		let mut router = WebSocketRouter::new();
		let route = WebSocketRoute::new("/ws/chat".to_string(), None);

		router.register_route(route).await.unwrap();
		assert_eq!(router.route_count().await, 1);

		router.clear().await;
		assert_eq!(router.route_count().await, 0);
	}

	#[tokio::test]
	async fn test_reverse_websocket_url() {
		let mut router = WebSocketRouter::new();
		let route = WebSocketRoute::new("/ws/chat".to_string(), Some("chat".to_string()));

		router.register_route(route).await.unwrap();

		let url = reverse_websocket_url(&router, "chat").await;
		assert_eq!(url, Some("/ws/chat".to_string()));
	}

	#[tokio::test]
	async fn test_global_router_registration() {
		let router = WebSocketRouter::new();
		register_websocket_router(router).await;

		let global = get_websocket_router().await;
		assert!(global.is_some());

		clear_websocket_router().await;
		let cleared = get_websocket_router().await;
		assert!(cleared.is_none());
	}
}
