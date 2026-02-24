//! Host API Implementation
//!
//! This module provides the host state and functions that are exposed to WASM plugins.
//! It implements the `host` interface defined in the WIT file.
//!
//! # Capabilities
//!
//! Some host functions require specific capabilities:
//! - HTTP functions: Require `NetworkAccess` capability
//! - Database functions: Require `DatabaseAccess` capability
//! - SSR rendering: Requires `Verified` or `Trusted` trust level
//! - JavaScript execution: Requires `Trusted` trust level only
//!
//! The capability-based access control pattern ensures plugins can only access
//! resources they have been explicitly granted. Capabilities are preserved
//! across `Clone` operations so that WASM store creation does not lose state.

use crate::capability::{Capability, TrustLevel};
use crate::error::PluginResult;

use parking_lot::RwLock;
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};

use super::events::{Event, EventBus, SharedEventBus};
use super::models::{ModelRegistry, ModelSchema, SharedModelRegistry, SqlMigration};
use super::ssr::{RenderOptions, RenderResult, SharedSsrProxy, SsrError, SsrProxy};
use super::types::{ConfigValue, WitHttpResponse, WitPluginError};

/// Shared default HTTP client reused across all `HostState` instances.
///
/// Creating a `reqwest::Client` per `HostState` wastes connection pools and
/// may lead to file descriptor exhaustion under high load. This shared client
/// ensures connection pools are reused across all plugins.
static DEFAULT_HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(reqwest::Client::new);

#[cfg(feature = "ts")]
use super::ts_runtime::SharedTsRuntime;

#[cfg(feature = "wasm")]
use reinhardt_db::backends::connection::DatabaseConnection;

#[cfg(feature = "wasm")]
use wasmtime::component::ResourceTable;
#[cfg(feature = "wasm")]
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

/// Validate a URL to prevent Server-Side Request Forgery (SSRF) attacks.
///
/// Performs the following checks:
/// 1. Only allows `http://` and `https://` schemes
/// 2. Resolves the hostname to an IP address
/// 3. Rejects requests to private, loopback, and link-local IP addresses
fn validate_url_for_ssrf(url: &str) -> Result<(), WitPluginError> {
	use std::net::ToSocketAddrs;

	// Parse the URL
	let parsed = url::Url::parse(url)
		.map_err(|e| WitPluginError::with_details(400, "Invalid URL", e.to_string()))?;

	// Validate scheme: only http and https are allowed
	match parsed.scheme() {
		"http" | "https" => {}
		scheme => {
			return Err(WitPluginError::new(
				403,
				format!(
					"URL scheme '{}' is not allowed; only http and https are permitted",
					scheme
				),
			));
		}
	}

	// Extract host
	let host = parsed
		.host_str()
		.ok_or_else(|| WitPluginError::new(400, "URL must contain a host"))?;

	// Determine port (default to 80 for http, 443 for https)
	let port = parsed.port_or_known_default().unwrap_or(80);

	// Resolve hostname to IP addresses for DNS rebinding protection
	let socket_addrs: Vec<_> = format!("{}:{}", host, port)
		.to_socket_addrs()
		.map_err(|e| {
			WitPluginError::with_details(400, "Failed to resolve hostname", e.to_string())
		})?
		.collect();

	if socket_addrs.is_empty() {
		return Err(WitPluginError::new(
			400,
			"Hostname resolved to no addresses",
		));
	}

	// Check each resolved IP address against blocklists
	for addr in &socket_addrs {
		if is_private_ip(&addr.ip()) {
			return Err(WitPluginError::new(
				403,
				format!(
					"Requests to private/internal IP address {} are not allowed",
					addr.ip()
				),
			));
		}
	}

	Ok(())
}

/// Check if an IP address is private, loopback, or link-local.
fn is_private_ip(ip: &std::net::IpAddr) -> bool {
	use std::net::IpAddr;

	match ip {
		IpAddr::V4(ipv4) => {
			// 127.0.0.0/8 - Loopback
			ipv4.is_loopback()
			// 10.0.0.0/8 - Private
			|| ipv4.octets()[0] == 10
			// 172.16.0.0/12 - Private
			|| (ipv4.octets()[0] == 172 && (ipv4.octets()[1] & 0xf0) == 16)
			// 192.168.0.0/16 - Private
			|| (ipv4.octets()[0] == 192 && ipv4.octets()[1] == 168)
			// 169.254.0.0/16 - Link-local
			|| (ipv4.octets()[0] == 169 && ipv4.octets()[1] == 254)
			// 0.0.0.0/8 - Current network
			|| ipv4.octets()[0] == 0
		}
		IpAddr::V6(ipv6) => {
			// ::1 - IPv6 loopback
			ipv6.is_loopback()
			// fc00::/7 - IPv6 unique local address (private)
			|| (ipv6.segments()[0] & 0xfe00) == 0xfc00
			// fe80::/10 - IPv6 link-local
			|| (ipv6.segments()[0] & 0xffc0) == 0xfe80
			// :: - IPv6 unspecified
			|| ipv6.is_unspecified()
			// ::ffff:0:0/96 - IPv4-mapped IPv6 (check the embedded IPv4)
			|| is_ipv4_mapped_private(ipv6)
		}
	}
}

/// Check if an IPv4-mapped IPv6 address contains a private IPv4 address.
fn is_ipv4_mapped_private(ipv6: &std::net::Ipv6Addr) -> bool {
	// Check for ::ffff:x.x.x.x pattern (IPv4-mapped IPv6)
	let segments = ipv6.segments();
	if segments[0] == 0
		&& segments[1] == 0
		&& segments[2] == 0
		&& segments[3] == 0
		&& segments[4] == 0
		&& segments[5] == 0xffff
	{
		let ipv4 = std::net::Ipv4Addr::new(
			(segments[6] >> 8) as u8,
			(segments[6] & 0xff) as u8,
			(segments[7] >> 8) as u8,
			(segments[7] & 0xff) as u8,
		);
		return is_private_ip(&std::net::IpAddr::V4(ipv4));
	}
	false
}

/// Host state that is passed to WASM instances.
///
/// This struct contains all the state that plugins can access through host functions.
pub struct HostState {
	/// Plugin name (for logging context)
	pub plugin_name: String,
	/// Trust level for this plugin (determines security restrictions)
	trust_level: TrustLevel,
	/// Configuration values accessible to the plugin
	config: RwLock<HashMap<String, ConfigValue>>,
	/// Registered services (name -> MessagePack-serialized data)
	services: RwLock<HashMap<String, Vec<u8>>>,
	/// Type-safe services (for internal use)
	typed_services: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
	/// Granted capabilities for this plugin
	capabilities: HashSet<Capability>,
	/// HTTP client for making external requests (optional)
	http_client: Option<reqwest::Client>,
	/// Database connection for SQL queries (optional, requires DatabaseAccess capability)
	#[cfg(feature = "wasm")]
	db_connection: Option<Arc<DatabaseConnection>>,
	#[cfg(not(feature = "wasm"))]
	db_connection: Option<()>,
	/// Event bus for inter-plugin communication
	event_bus: SharedEventBus,
	/// Model registry for schema and migration registration
	model_registry: SharedModelRegistry,
	/// SSR proxy for server-side rendering (requires TypeScript runtime)
	ssr_proxy: SharedSsrProxy,
	/// WASI context for WASI P2 interface support
	/// Wrapped in Mutex to allow interior mutability while maintaining Sync
	#[cfg(feature = "wasm")]
	wasi_ctx: parking_lot::Mutex<WasiCtx>,
	/// Resource table for WASI P2 interface support
	/// Wrapped in Mutex to allow interior mutability while maintaining Sync
	#[cfg(feature = "wasm")]
	resource_table: parking_lot::Mutex<ResourceTable>,
}

impl HostState {
	/// Create a new host state with default configuration.
	pub fn new(plugin_name: impl Into<String>) -> Self {
		Self::with_shared_resources(
			plugin_name,
			Arc::new(EventBus::new()),
			Arc::new(ModelRegistry::new()),
			Arc::new(SsrProxy::new()),
		)
	}

	/// Create a new host state with a shared event bus.
	///
	/// This allows multiple plugin instances to share the same event bus
	/// for inter-plugin communication.
	pub fn with_event_bus(plugin_name: impl Into<String>, event_bus: SharedEventBus) -> Self {
		Self::with_shared_resources(
			plugin_name,
			event_bus,
			Arc::new(ModelRegistry::new()),
			Arc::new(SsrProxy::new()),
		)
	}

	/// Create a new host state with shared event bus and model registry.
	///
	/// This allows multiple plugin instances to share the same event bus,
	/// model registry, and SSR proxy for inter-plugin communication,
	/// schema management, and server-side rendering.
	pub fn with_shared_resources(
		plugin_name: impl Into<String>,
		event_bus: SharedEventBus,
		model_registry: SharedModelRegistry,
		ssr_proxy: SharedSsrProxy,
	) -> Self {
		Self {
			plugin_name: plugin_name.into(),
			trust_level: TrustLevel::default(),
			config: RwLock::new(HashMap::new()),
			services: RwLock::new(HashMap::new()),
			typed_services: RwLock::new(HashMap::new()),
			capabilities: HashSet::new(),
			http_client: Some(DEFAULT_HTTP_CLIENT.clone()),
			db_connection: None,
			event_bus,
			model_registry,
			ssr_proxy,
			#[cfg(feature = "wasm")]
			wasi_ctx: parking_lot::Mutex::new(WasiCtxBuilder::new().build()),
			#[cfg(feature = "wasm")]
			resource_table: parking_lot::Mutex::new(ResourceTable::new()),
		}
	}

	/// Create a builder for host state.
	pub fn builder(plugin_name: impl Into<String>) -> HostStateBuilder {
		HostStateBuilder::new(plugin_name)
	}

	// ===== Configuration API =====

	/// Get a configuration value by key.
	pub fn get_config(&self, key: &str) -> Option<ConfigValue> {
		self.config.read().get(key).cloned()
	}

	/// Set a configuration value.
	pub fn set_config(&self, key: &str, value: ConfigValue) -> PluginResult<()> {
		self.config.write().insert(key.to_string(), value);
		Ok(())
	}

	/// Set multiple configuration values at once.
	pub fn set_config_all(&self, values: HashMap<String, ConfigValue>) {
		let mut config = self.config.write();
		for (key, value) in values {
			config.insert(key, value);
		}
	}

	/// Get all configuration values as a cloned HashMap.
	///
	/// This method is primarily used for serializing configuration to pass
	/// to WASM plugins during lifecycle events.
	pub fn get_config_all(&self) -> HashMap<String, ConfigValue> {
		self.config.read().clone()
	}

	// ===== Logging API =====

	/// Log a debug message.
	pub fn log_debug(&self, message: &str) {
		tracing::debug!(plugin = %self.plugin_name, "{}", message);
	}

	/// Log an info message.
	pub fn log_info(&self, message: &str) {
		tracing::info!(plugin = %self.plugin_name, "{}", message);
	}

	/// Log a warning message.
	pub fn log_warn(&self, message: &str) {
		tracing::warn!(plugin = %self.plugin_name, "{}", message);
	}

	/// Log an error message.
	pub fn log_error(&self, message: &str) {
		tracing::error!(plugin = %self.plugin_name, "{}", message);
	}

	// ===== Service Registration API =====

	/// Register a service with MessagePack-serialized data.
	pub fn register_service(&self, name: &str, data: Vec<u8>) -> PluginResult<()> {
		self.services.write().insert(name.to_string(), data);
		Ok(())
	}

	/// Get a registered service by name.
	pub fn get_service(&self, name: &str) -> Option<Vec<u8>> {
		self.services.read().get(name).cloned()
	}

	/// Unregister a service by name.
	pub fn unregister_service(&self, name: &str) -> PluginResult<()> {
		self.services.write().remove(name);
		Ok(())
	}

	/// Register a typed service (for internal use).
	pub fn register_typed_service<T: Any + Send + Sync>(&self, service: Arc<T>) {
		let type_id = TypeId::of::<T>();
		self.typed_services.write().insert(type_id, service);
	}

	/// Get a typed service (for internal use).
	pub fn get_typed_service<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
		let type_id = TypeId::of::<T>();
		self.typed_services
			.read()
			.get(&type_id)
			.and_then(|s| s.clone().downcast::<T>().ok())
	}

	// ===== HTTP Client API =====

	/// Check if HTTP access is allowed.
	fn has_http_capability(&self) -> bool {
		use crate::capability::PluginCapability;
		self.capabilities
			.contains(&Capability::Core(PluginCapability::NetworkAccess))
	}

	/// Perform an HTTP GET request.
	///
	/// Validates the URL against SSRF attacks before making the request.
	/// Only `http://` and `https://` schemes are allowed, and requests to
	/// private/loopback IP addresses are rejected.
	pub async fn http_get(
		&self,
		url: &str,
		headers: &[(String, String)],
	) -> Result<WitHttpResponse, WitPluginError> {
		if !self.has_http_capability() {
			return Err(WitPluginError::new(403, "HTTP access not permitted"));
		}

		validate_url_for_ssrf(url)?;

		let client = self
			.http_client
			.as_ref()
			.ok_or_else(|| WitPluginError::new(500, "HTTP client not available"))?;

		let mut request = client.get(url);

		for (name, value) in headers {
			request = request.header(name, value);
		}

		let response = request
			.send()
			.await
			.map_err(|e| WitPluginError::with_details(500, "HTTP request failed", e.to_string()))?;

		let status = response.status().as_u16();
		let headers: Vec<(String, String)> = response
			.headers()
			.iter()
			.map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
			.collect();
		let body = response.bytes().await.map_err(|e| {
			WitPluginError::with_details(500, "Failed to read response body", e.to_string())
		})?;

		Ok(WitHttpResponse {
			status,
			headers,
			body: body.to_vec(),
		})
	}

	/// Perform an HTTP POST request.
	///
	/// Validates the URL against SSRF attacks before making the request.
	/// Only `http://` and `https://` schemes are allowed, and requests to
	/// private/loopback IP addresses are rejected.
	pub async fn http_post(
		&self,
		url: &str,
		body: &[u8],
		headers: &[(String, String)],
	) -> Result<WitHttpResponse, WitPluginError> {
		if !self.has_http_capability() {
			return Err(WitPluginError::new(403, "HTTP access not permitted"));
		}

		validate_url_for_ssrf(url)?;

		let client = self
			.http_client
			.as_ref()
			.ok_or_else(|| WitPluginError::new(500, "HTTP client not available"))?;

		let mut request = client.post(url).body(body.to_vec());

		for (name, value) in headers {
			request = request.header(name, value);
		}

		let response = request
			.send()
			.await
			.map_err(|e| WitPluginError::with_details(500, "HTTP request failed", e.to_string()))?;

		let status = response.status().as_u16();
		let resp_headers: Vec<(String, String)> = response
			.headers()
			.iter()
			.map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
			.collect();
		let resp_body = response.bytes().await.map_err(|e| {
			WitPluginError::with_details(500, "Failed to read response body", e.to_string())
		})?;

		Ok(WitHttpResponse {
			status,
			headers: resp_headers,
			body: resp_body.to_vec(),
		})
	}

	// ===== Database API =====

	/// Check if database access is allowed.
	fn has_db_capability(&self) -> bool {
		use crate::capability::PluginCapability;
		self.capabilities
			.contains(&Capability::Core(PluginCapability::DatabaseAccess))
	}

	/// Execute a SQL query.
	#[cfg(feature = "wasm")]
	pub async fn db_query(&self, sql: &str, params: &[u8]) -> Result<Vec<u8>, WitPluginError> {
		if !self.has_db_capability() {
			return Err(WitPluginError::new(403, "Database access not permitted"));
		}

		// Validate SQL statement to prevent SQL injection
		use super::sql_validator::validate_sql;
		validate_sql(sql).map_err(|e| {
			WitPluginError::with_details(403, "SQL validation failed", e.to_string())
		})?;

		let conn = self
			.db_connection
			.as_ref()
			.ok_or_else(|| WitPluginError::new(500, "Database connection not available"))?;

		// Deserialize MessagePack parameters
		use reinhardt_db::backends::types::QueryValue;
		let params_vec: Vec<QueryValue> = rmp_serde::from_slice(params).map_err(|e| {
			WitPluginError::with_details(400, "Invalid query parameters", e.to_string())
		})?;

		// Execute query
		let rows = conn.fetch_all(sql, params_vec).await.map_err(|e| {
			WitPluginError::with_details(500, "Database query failed", e.to_string())
		})?;

		// Convert rows to serializable format (Vec<HashMap>)
		let serializable_rows: Vec<HashMap<String, QueryValue>> =
			rows.into_iter().map(|row| row.data).collect();

		// Serialize results to MessagePack
		rmp_serde::to_vec(&serializable_rows).map_err(|e| {
			WitPluginError::with_details(500, "Failed to serialize results", e.to_string())
		})
	}

	#[cfg(not(feature = "wasm"))]
	pub async fn db_query(&self, _sql: &str, _params: &[u8]) -> Result<Vec<u8>, WitPluginError> {
		if !self.has_db_capability() {
			return Err(WitPluginError::new(403, "Database access not permitted"));
		}
		Err(WitPluginError::new(
			501,
			"Database queries not available (wasm feature not enabled)",
		))
	}

	/// Execute a SQL statement (INSERT, UPDATE, DELETE).
	#[cfg(feature = "wasm")]
	pub async fn db_execute(&self, sql: &str, params: &[u8]) -> Result<u64, WitPluginError> {
		if !self.has_db_capability() {
			return Err(WitPluginError::new(403, "Database access not permitted"));
		}

		// Validate SQL statement to prevent SQL injection
		use super::sql_validator::validate_sql;
		validate_sql(sql).map_err(|e| {
			WitPluginError::with_details(403, "SQL validation failed", e.to_string())
		})?;

		let conn = self
			.db_connection
			.as_ref()
			.ok_or_else(|| WitPluginError::new(500, "Database connection not available"))?;

		// Deserialize MessagePack parameters
		use reinhardt_db::backends::types::QueryValue;
		let params_vec: Vec<QueryValue> = rmp_serde::from_slice(params).map_err(|e| {
			WitPluginError::with_details(400, "Invalid query parameters", e.to_string())
		})?;

		// Execute statement
		let result = conn.execute(sql, params_vec).await.map_err(|e| {
			WitPluginError::with_details(500, "Database execution failed", e.to_string())
		})?;

		Ok(result.rows_affected)
	}

	#[cfg(not(feature = "wasm"))]
	pub async fn db_execute(&self, _sql: &str, _params: &[u8]) -> Result<u64, WitPluginError> {
		if !self.has_db_capability() {
			return Err(WitPluginError::new(403, "Database access not permitted"));
		}
		Err(WitPluginError::new(
			501,
			"Database execution not available (wasm feature not enabled)",
		))
	}

	// ===== Trust Level API =====

	/// Get the trust level for this plugin.
	pub fn trust_level(&self) -> TrustLevel {
		self.trust_level
	}

	// ===== Capability Management =====

	/// Add a capability to this host state.
	pub fn add_capability(&mut self, capability: Capability) {
		self.capabilities.insert(capability);
	}

	/// Check if this host state has a specific capability.
	pub fn has_capability(&self, capability: &Capability) -> bool {
		self.capabilities.contains(capability)
	}

	/// Get all capabilities.
	pub fn capabilities(&self) -> &HashSet<Capability> {
		&self.capabilities
	}

	// ===== Event Bus API =====

	/// Get a reference to the shared event bus.
	pub fn event_bus(&self) -> &SharedEventBus {
		&self.event_bus
	}

	/// Emit an event to all matching subscribers.
	///
	/// # Arguments
	///
	/// * `name` - Event name (e.g., "user.created")
	/// * `payload` - MessagePack-serialized event data
	///
	/// # Returns
	///
	/// The number of subscriptions that received the event.
	pub fn emit_event(&self, name: &str, payload: Vec<u8>) -> usize {
		self.event_bus.emit(name, payload, &self.plugin_name)
	}

	/// Subscribe to events matching a pattern.
	///
	/// # Pattern Syntax
	///
	/// - `*` - Matches all events
	/// - `user.*` - Matches events starting with "user." (e.g., "user.created")
	/// - `user.created` - Matches only "user.created"
	///
	/// # Returns
	///
	/// A unique subscription ID for polling and unsubscribing.
	///
	/// # Errors
	///
	/// Returns an error if the subscription limit has been reached.
	pub fn subscribe_events(
		&self,
		pattern: &str,
	) -> Result<u64, crate::wasm::events::EventBusError> {
		self.event_bus.subscribe(pattern, &self.plugin_name)
	}

	/// Unsubscribe from a subscription.
	pub fn unsubscribe_events(&self, subscription_id: u64) -> bool {
		self.event_bus.unsubscribe(subscription_id)
	}

	/// Poll for pending events on a subscription.
	///
	/// Events are removed from the queue once polled.
	pub fn poll_pending_events(&self, subscription_id: u64, limit: usize) -> Vec<Event> {
		self.event_bus.poll(subscription_id, limit)
	}

	// ===== Model Registry API =====

	/// Get a reference to the shared model registry.
	pub fn model_registry(&self) -> &SharedModelRegistry {
		&self.model_registry
	}

	/// Register a model schema.
	///
	/// # Arguments
	///
	/// * `schema` - The model schema to register
	///
	/// # Returns
	///
	/// An error if a schema with the same table name is already registered
	/// by this plugin.
	pub fn register_model_schema(&self, schema: ModelSchema) -> Result<(), String> {
		self.model_registry
			.register_model(&self.plugin_name, schema)
	}

	/// Register a raw SQL migration.
	///
	/// # Arguments
	///
	/// * `migration` - The SQL migration to register
	///
	/// # Returns
	///
	/// An error if a migration with the same version is already registered
	/// by this plugin.
	pub fn register_sql_migration(&self, migration: SqlMigration) -> Result<(), String> {
		self.model_registry
			.register_migration(&self.plugin_name, migration)
	}

	/// List all model table names registered by this plugin.
	pub fn list_registered_models(&self) -> Vec<String> {
		self.model_registry.list_models(&self.plugin_name)
	}

	/// Get a model schema by table name.
	pub fn get_registered_model(&self, table_name: &str) -> Option<ModelSchema> {
		self.model_registry.get_model(&self.plugin_name, table_name)
	}

	/// List all migrations registered by this plugin.
	pub fn list_registered_migrations(&self) -> Vec<SqlMigration> {
		self.model_registry.list_migrations(&self.plugin_name)
	}

	// ===== SSR Proxy API =====

	/// Get a reference to the shared SSR proxy.
	pub fn ssr_proxy(&self) -> &SharedSsrProxy {
		&self.ssr_proxy
	}

	/// Check if SSR is available.
	///
	/// Returns `true` if the host has TypeScript runtime support enabled.
	pub fn is_ssr_available(&self) -> bool {
		self.ssr_proxy.is_available()
	}

	/// Render a React component to HTML.
	///
	/// # Arguments
	///
	/// * `component_path` - Path to the component file (relative to plugin assets)
	/// * `props` - MessagePack-serialized component props
	/// * `options` - Rendering options
	///
	/// # Returns
	///
	/// Rendered HTML and optional extracted assets, or an error if SSR is not available
	/// or the trust level is insufficient.
	///
	/// # Security
	///
	/// Only plugins with `Verified` or `Trusted` trust level can render components.
	pub async fn render_react(
		&self,
		component_path: &str,
		props: &[u8],
		options: RenderOptions,
	) -> Result<RenderResult, SsrError> {
		if !self.trust_level.allows_ssr() {
			return Err(SsrError::PermissionDenied(format!(
				"SSR rendering requires at least Verified trust level, but plugin '{}' has {:?} trust level",
				self.plugin_name, self.trust_level
			)));
		}
		self.ssr_proxy
			.render_react(component_path, props, options)
			.await
	}

	/// Execute arbitrary JavaScript code.
	///
	/// # Arguments
	///
	/// * `code` - JavaScript code to execute
	///
	/// # Returns
	///
	/// The result as UTF-8 bytes, or an error if the trust level is insufficient
	/// or SSR is not available.
	///
	/// # Security
	///
	/// Only plugins with `Trusted` trust level can execute arbitrary JavaScript.
	/// This prevents untrusted or verified plugins from accessing the JavaScript
	/// runtime directly.
	pub fn eval_js(&self, code: &str) -> Result<Vec<u8>, SsrError> {
		if !self.trust_level.allows_js_execution() {
			return Err(SsrError::PermissionDenied(format!(
				"JavaScript execution requires Trusted trust level, but plugin '{}' has {:?} trust level",
				self.plugin_name, self.trust_level
			)));
		}
		self.ssr_proxy.eval_js(code).map(|s| s.into_bytes())
	}

	/// Render a component with trust level enforcement.
	///
	/// # Arguments
	///
	/// * `component_code` - JavaScript code defining the component
	/// * `props_json` - JSON string representing component props
	/// * `options` - Rendering options
	///
	/// # Security
	///
	/// Only plugins with `Verified` or `Trusted` trust level can render components.
	/// `Untrusted` plugins cannot use SSR as it involves JavaScript execution.
	/// Component code is also validated for dangerous patterns.
	pub fn render_component(
		&self,
		component_code: &str,
		props_json: &str,
		options: RenderOptions,
	) -> Result<RenderResult, SsrError> {
		if !self.trust_level.allows_ssr() {
			return Err(SsrError::PermissionDenied(format!(
				"SSR rendering requires at least Verified trust level, but plugin '{}' has {:?} trust level",
				self.plugin_name, self.trust_level
			)));
		}
		self.ssr_proxy
			.render_component(component_code, props_json, options)
	}
}

// Clone implementation for HostState (needed for store creation).
// Preserves all state (config, services, capabilities, shared resources).
// WASI context and resource table are created fresh since each store
// requires its own isolated WASI state.
impl Clone for HostState {
	fn clone(&self) -> Self {
		let mut builder = HostStateBuilder::new(&self.plugin_name)
			.config_all(self.config.read().clone())
			.capabilities(self.capabilities.clone())
			.event_bus(self.event_bus.clone())
			.model_registry(self.model_registry.clone())
			.ssr_proxy(self.ssr_proxy.clone());

		if let Some(ref client) = self.http_client {
			builder = builder.http_client(client.clone());
		} else {
			builder = builder.no_http_client();
		}

		#[cfg(feature = "wasm")]
		if let Some(ref conn) = self.db_connection {
			builder = builder.db_connection(conn.clone());
		}

		let state = builder.build();

		// Clone serialized services
		let services = self.services.read().clone();
		for (name, data) in services {
			let _ = state.register_service(&name, data);
		}

		// Clone typed services (Arc-based shared references)
		*state.typed_services.write() = self.typed_services.read().clone();

		state
	}
}

impl std::fmt::Debug for HostState {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("HostState")
			.field("plugin_name", &self.plugin_name)
			.field("trust_level", &self.trust_level)
			.field("config_count", &self.config.read().len())
			.field("service_count", &self.services.read().len())
			.field("capabilities", &self.capabilities)
			.finish_non_exhaustive()
	}
}

/// Builder for `HostState`.
pub struct HostStateBuilder {
	plugin_name: String,
	trust_level: TrustLevel,
	config: HashMap<String, ConfigValue>,
	capabilities: HashSet<Capability>,
	http_client: Option<reqwest::Client>,
	#[cfg(feature = "wasm")]
	db_connection: Option<Arc<DatabaseConnection>>,
	#[cfg(not(feature = "wasm"))]
	db_connection: Option<()>,
	event_bus: Option<SharedEventBus>,
	model_registry: Option<SharedModelRegistry>,
	ssr_proxy: Option<SharedSsrProxy>,
}

impl HostStateBuilder {
	/// Create a new builder.
	///
	/// Uses the shared default HTTP client to avoid creating redundant connection pools.
	pub fn new(plugin_name: impl Into<String>) -> Self {
		Self {
			plugin_name: plugin_name.into(),
			trust_level: TrustLevel::default(),
			config: HashMap::new(),
			capabilities: HashSet::new(),
			http_client: Some(DEFAULT_HTTP_CLIENT.clone()),
			db_connection: None,
			event_bus: None,
			model_registry: None,
			ssr_proxy: None,
		}
	}

	/// Set the trust level for this plugin.
	///
	/// Default is `TrustLevel::Untrusted`.
	pub fn trust_level(mut self, trust_level: TrustLevel) -> Self {
		self.trust_level = trust_level;
		self
	}

	/// Add a configuration value.
	pub fn config(mut self, key: impl Into<String>, value: ConfigValue) -> Self {
		self.config.insert(key.into(), value);
		self
	}

	/// Add multiple configuration values.
	pub fn config_all(mut self, config: HashMap<String, ConfigValue>) -> Self {
		self.config.extend(config);
		self
	}

	/// Add a capability.
	pub fn capability(mut self, capability: Capability) -> Self {
		self.capabilities.insert(capability);
		self
	}

	/// Add multiple capabilities.
	pub fn capabilities(mut self, capabilities: impl IntoIterator<Item = Capability>) -> Self {
		self.capabilities.extend(capabilities);
		self
	}

	/// Set a custom HTTP client.
	pub fn http_client(mut self, client: reqwest::Client) -> Self {
		self.http_client = Some(client);
		self
	}

	/// Disable HTTP client.
	pub fn no_http_client(mut self) -> Self {
		self.http_client = None;
		self
	}

	/// Set a database connection.
	#[cfg(feature = "wasm")]
	pub fn db_connection(mut self, connection: Arc<DatabaseConnection>) -> Self {
		self.db_connection = Some(connection);
		self
	}

	/// Set a shared event bus.
	///
	/// If not set, a new event bus will be created for this host state.
	/// To enable inter-plugin communication, share the same event bus
	/// across multiple host states.
	pub fn event_bus(mut self, event_bus: SharedEventBus) -> Self {
		self.event_bus = Some(event_bus);
		self
	}

	/// Set a shared model registry.
	///
	/// If not set, a new model registry will be created for this host state.
	/// To share model schemas across plugins, use the same model registry
	/// for multiple host states.
	pub fn model_registry(mut self, model_registry: SharedModelRegistry) -> Self {
		self.model_registry = Some(model_registry);
		self
	}

	/// Set a shared SSR proxy.
	///
	/// If not set, a new SSR proxy will be created for this host state.
	/// By default, the SSR proxy is unavailable (TypeScript runtime not enabled).
	/// Use [`SsrProxy::with_availability(true)`] to enable SSR.
	pub fn ssr_proxy(mut self, ssr_proxy: SharedSsrProxy) -> Self {
		self.ssr_proxy = Some(ssr_proxy);
		self
	}

	/// Set a TypeScript runtime for SSR.
	///
	/// This enables JavaScript/TypeScript SSR with full TypeScript support.
	/// The TypeScript runtime uses rustyscript (deno_core based) with
	/// Preact loaded from esm.sh CDN for React-compatible SSR.
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_dentdelion::wasm::{TsRuntime, HostStateBuilder};
	/// use std::sync::Arc;
	///
	/// let ts_runtime = Arc::new(TsRuntime::new()?);
	/// let host_state = HostStateBuilder::new("my-plugin")
	///     .ts_runtime(ts_runtime)
	///     .build();
	/// ```
	#[cfg(feature = "ts")]
	pub fn ts_runtime(mut self, runtime: SharedTsRuntime) -> Self {
		self.ssr_proxy = Some(Arc::new(SsrProxy::with_ts_runtime(runtime)));
		self
	}

	/// Build the host state.
	pub fn build(self) -> HostState {
		HostState {
			plugin_name: self.plugin_name,
			trust_level: self.trust_level,
			config: RwLock::new(self.config),
			services: RwLock::new(HashMap::new()),
			typed_services: RwLock::new(HashMap::new()),
			capabilities: self.capabilities,
			http_client: self.http_client,
			db_connection: self.db_connection,
			event_bus: self.event_bus.unwrap_or_else(|| Arc::new(EventBus::new())),
			model_registry: self
				.model_registry
				.unwrap_or_else(|| Arc::new(ModelRegistry::new())),
			ssr_proxy: self.ssr_proxy.unwrap_or_else(|| Arc::new(SsrProxy::new())),
			#[cfg(feature = "wasm")]
			wasi_ctx: parking_lot::Mutex::new(WasiCtxBuilder::new().build()),
			#[cfg(feature = "wasm")]
			resource_table: parking_lot::Mutex::new(ResourceTable::new()),
		}
	}
}

// ===== Type Aliases for Generated WASM Types =====

#[cfg(feature = "wasm")]
type GeneratedPluginError = crate::wasm::runtime::reinhardt::dentdelion::types::PluginError;

#[cfg(feature = "wasm")]
type GeneratedHttpResponse = crate::wasm::runtime::reinhardt::dentdelion::types::HttpResponse;

// ===== Type Conversion Functions =====

/// Convert WitPluginError to generated PluginError type
#[cfg(feature = "wasm")]
fn to_generated_error(err: WitPluginError) -> GeneratedPluginError {
	GeneratedPluginError {
		code: err.code,
		message: err.message,
		details: err.details,
	}
}

/// Convert WitHttpResponse to generated HttpResponse type
#[cfg(feature = "wasm")]
fn to_generated_http_response(response: WitHttpResponse) -> GeneratedHttpResponse {
	GeneratedHttpResponse {
		status: response.status,
		headers: response.headers,
		body: response.body,
	}
}

// ===== WasiView Trait Implementation =====
//
// Implement WasiView to enable WASI P2 interface support for plugins.
// This provides access to WasiCtx and ResourceTable required by wasmtime-wasi.

#[cfg(feature = "wasm")]
impl WasiView for HostState {
	fn ctx(&mut self) -> WasiCtxView<'_> {
		// Use get_mut() to obtain mutable references without locking
		// This is safe because WasiView::ctx() requires &mut self,
		// ensuring exclusive access to the Mutex internals
		WasiCtxView {
			ctx: self.wasi_ctx.get_mut(),
			table: self.resource_table.get_mut(),
		}
	}
}

// ===== Host Trait Implementation =====
//
// Implement the Host trait generated by bindgen! to expose host functions to WASM plugins.
// The trait is generated at crate::wasm::runtime::reinhardt::dentdelion::host::Host
//
// Note: The generated Host trait uses synchronous functions, but some of our host
// functions are async. We use tokio::runtime::Handle::current().block_on() to bridge them.

// Implement the empty marker trait from types module
#[cfg(feature = "wasm")]
impl crate::wasm::runtime::reinhardt::dentdelion::types::Host for HostState {}

// Implement the actual host functions from host module
// bindgen generates traits with native async fn (RPITIT - Return Position Impl Trait In Traits)
// so we don't use async_trait macro here
#[cfg(feature = "wasm")]
impl crate::wasm::runtime::reinhardt::dentdelion::host::Host for HostState {
	// ===== Configuration Access =====

	async fn get_config(&mut self, key: String) -> Result<Option<Vec<u8>>, anyhow::Error> {
		// Get config value and serialize to MessagePack
		let result = Self::get_config(self, &key).and_then(|v| rmp_serde::to_vec(&v).ok());
		Ok(result)
	}

	async fn set_config(
		&mut self,
		key: String,
		value: Vec<u8>,
	) -> Result<Result<(), GeneratedPluginError>, anyhow::Error> {
		// Deserialize MessagePack to ConfigValue
		let config_value: ConfigValue = rmp_serde::from_slice(&value)
			.map_err(|e| anyhow::anyhow!("Invalid config value: {}", e))?;

		let result = Self::set_config(self, &key, config_value)
			.map_err(|e| to_generated_error(WitPluginError::from_plugin_error(&e)));
		Ok(result)
	}

	// ===== Logging =====

	async fn log_debug(&mut self, message: String) -> Result<(), anyhow::Error> {
		Self::log_debug(self, &message);
		Ok(())
	}

	async fn log_info(&mut self, message: String) -> Result<(), anyhow::Error> {
		Self::log_info(self, &message);
		Ok(())
	}

	async fn log_warn(&mut self, message: String) -> Result<(), anyhow::Error> {
		Self::log_warn(self, &message);
		Ok(())
	}

	async fn log_error(&mut self, message: String) -> Result<(), anyhow::Error> {
		Self::log_error(self, &message);
		Ok(())
	}

	// ===== Service Registration =====

	async fn register_service(
		&mut self,
		name: String,
		data: Vec<u8>,
	) -> Result<Result<(), GeneratedPluginError>, anyhow::Error> {
		let result = Self::register_service(self, &name, data)
			.map_err(|e| to_generated_error(WitPluginError::from_plugin_error(&e)));
		Ok(result)
	}

	async fn get_service(&mut self, name: String) -> Result<Option<Vec<u8>>, anyhow::Error> {
		Ok(Self::get_service(self, &name))
	}

	async fn unregister_service(
		&mut self,
		name: String,
	) -> Result<Result<(), GeneratedPluginError>, anyhow::Error> {
		let result = Self::unregister_service(self, &name)
			.map_err(|e| to_generated_error(WitPluginError::from_plugin_error(&e)));
		Ok(result)
	}

	// ===== HTTP Client =====

	async fn http_get(
		&mut self,
		url: String,
		headers: Vec<(String, String)>,
	) -> Result<Result<GeneratedHttpResponse, GeneratedPluginError>, anyhow::Error> {
		// Call async function directly (no longer need to block)
		let result = Self::http_get(self, &url, &headers)
			.await
			.map(to_generated_http_response)
			.map_err(to_generated_error);
		Ok(result)
	}

	async fn http_post(
		&mut self,
		url: String,
		body: Vec<u8>,
		headers: Vec<(String, String)>,
	) -> Result<Result<GeneratedHttpResponse, GeneratedPluginError>, anyhow::Error> {
		// Call async function directly (no longer need to block)
		let result = Self::http_post(self, &url, &body, &headers)
			.await
			.map(to_generated_http_response)
			.map_err(to_generated_error);
		Ok(result)
	}

	// ===== Database Access =====

	async fn db_query(
		&mut self,
		sql: String,
		params: Vec<u8>,
	) -> Result<Result<Vec<u8>, GeneratedPluginError>, anyhow::Error> {
		// Call async function directly (no longer need to block)
		let result = Self::db_query(self, &sql, &params)
			.await
			.map_err(to_generated_error);
		Ok(result)
	}

	async fn db_execute(
		&mut self,
		sql: String,
		params: Vec<u8>,
	) -> Result<Result<u64, GeneratedPluginError>, anyhow::Error> {
		// Call async function directly (no longer need to block)
		let result = Self::db_execute(self, &sql, &params)
			.await
			.map_err(to_generated_error);
		Ok(result)
	}
}

// ===== Events Host Trait Implementation =====
//
// Implement the events Host trait for inter-plugin communication.
// The trait is generated at crate::wasm::runtime::reinhardt::dentdelion::events::Host

#[cfg(feature = "wasm")]
type GeneratedEvent = crate::wasm::runtime::reinhardt::dentdelion::events::Event;

#[cfg(feature = "wasm")]
fn to_generated_event(event: Event) -> GeneratedEvent {
	GeneratedEvent {
		name: event.name,
		payload: event.payload,
		source: event.source,
		timestamp: event.timestamp,
	}
}

#[cfg(feature = "wasm")]
impl crate::wasm::runtime::reinhardt::dentdelion::events::Host for HostState {
	async fn emit(
		&mut self,
		name: String,
		payload: Vec<u8>,
	) -> Result<Result<(), GeneratedPluginError>, anyhow::Error> {
		let _delivered = self.emit_event(&name, payload);
		Ok(Ok(()))
	}

	async fn subscribe(
		&mut self,
		pattern: String,
	) -> Result<Result<u64, GeneratedPluginError>, anyhow::Error> {
		match self.subscribe_events(&pattern) {
			Ok(subscription_id) => Ok(Ok(subscription_id)),
			Err(e) => Ok(Err(to_generated_error(WitPluginError::new(
				429,
				e.to_string(),
			)))),
		}
	}

	async fn unsubscribe(
		&mut self,
		id: u64,
	) -> Result<Result<(), GeneratedPluginError>, anyhow::Error> {
		let removed = self.unsubscribe_events(id);
		if removed {
			Ok(Ok(()))
		} else {
			Ok(Err(to_generated_error(WitPluginError::new(
				404,
				"Subscription not found",
			))))
		}
	}

	async fn poll_events(
		&mut self,
		id: u64,
		limit: u32,
	) -> Result<Vec<GeneratedEvent>, anyhow::Error> {
		let events = self.poll_pending_events(id, limit as usize);
		Ok(events.into_iter().map(to_generated_event).collect())
	}
}

// ===== Models Host Trait Implementation =====
//
// Implement the models Host trait for schema and migration registration.
// The trait is generated at crate::wasm::runtime::reinhardt::dentdelion::models::Host

#[cfg(feature = "wasm")]
type GeneratedModelSchema = crate::wasm::runtime::reinhardt::dentdelion::models::ModelSchema;
#[cfg(feature = "wasm")]
type GeneratedColumnDef = crate::wasm::runtime::reinhardt::dentdelion::models::ColumnDef;
#[cfg(feature = "wasm")]
type GeneratedColumnType = crate::wasm::runtime::reinhardt::dentdelion::models::ColumnType;
#[cfg(feature = "wasm")]
type GeneratedIndexDef = crate::wasm::runtime::reinhardt::dentdelion::models::IndexDef;
#[cfg(feature = "wasm")]
type GeneratedSqlMigration = crate::wasm::runtime::reinhardt::dentdelion::models::SqlMigration;

use super::models::{ColumnDef, ColumnType, IndexDef};

/// Convert generated ColumnType to internal ColumnType
#[cfg(feature = "wasm")]
fn from_generated_column_type(col_type: GeneratedColumnType) -> ColumnType {
	use crate::wasm::runtime::reinhardt::dentdelion::models::ColumnType as Gen;
	match col_type {
		Gen::Integer => ColumnType::Integer,
		Gen::BigInteger => ColumnType::BigInteger,
		Gen::Text => ColumnType::Text,
		Gen::Varchar(len) => ColumnType::Varchar(len),
		Gen::Boolean => ColumnType::Boolean,
		Gen::Timestamp => ColumnType::Timestamp,
		Gen::Uuid => ColumnType::Uuid,
		Gen::Json => ColumnType::Json,
		Gen::Decimal((precision, scale)) => ColumnType::Decimal { precision, scale },
		Gen::ForeignKey(table) => ColumnType::ForeignKey(table),
	}
}

/// Convert generated ColumnDef to internal ColumnDef
#[cfg(feature = "wasm")]
fn from_generated_column_def(col: GeneratedColumnDef) -> ColumnDef {
	ColumnDef {
		name: col.name,
		column_type: from_generated_column_type(col.column_type),
		nullable: col.nullable,
		primary_key: col.primary_key,
		unique_value: col.unique_value,
		default_value: col.default_value,
	}
}

/// Convert generated IndexDef to internal IndexDef
#[cfg(feature = "wasm")]
fn from_generated_index_def(idx: GeneratedIndexDef) -> IndexDef {
	IndexDef {
		name: idx.name,
		columns: idx.columns,
		unique_value: idx.unique_value,
	}
}

/// Convert generated ModelSchema to internal ModelSchema
#[cfg(feature = "wasm")]
fn from_generated_model_schema(schema: GeneratedModelSchema) -> ModelSchema {
	ModelSchema {
		table_name: schema.table_name,
		columns: schema
			.columns
			.into_iter()
			.map(from_generated_column_def)
			.collect(),
		indexes: schema
			.indexes
			.into_iter()
			.map(from_generated_index_def)
			.collect(),
	}
}

/// Convert generated SqlMigration to internal SqlMigration
#[cfg(feature = "wasm")]
fn from_generated_sql_migration(migration: GeneratedSqlMigration) -> SqlMigration {
	SqlMigration {
		version: migration.version,
		description: migration.description,
		up_sql: migration.up_sql,
		down_sql: migration.down_sql,
	}
}

#[cfg(feature = "wasm")]
impl crate::wasm::runtime::reinhardt::dentdelion::models::Host for HostState {
	async fn register_model(
		&mut self,
		schema: GeneratedModelSchema,
	) -> Result<Result<(), GeneratedPluginError>, anyhow::Error> {
		let internal_schema = from_generated_model_schema(schema);
		let result = self
			.register_model_schema(internal_schema)
			.map_err(|e| to_generated_error(WitPluginError::new(400, &e)));
		Ok(result)
	}

	async fn register_migration(
		&mut self,
		migration: GeneratedSqlMigration,
	) -> Result<Result<(), GeneratedPluginError>, anyhow::Error> {
		let internal_migration = from_generated_sql_migration(migration);
		let result = self
			.register_sql_migration(internal_migration)
			.map_err(|e| to_generated_error(WitPluginError::new(400, &e)));
		Ok(result)
	}

	async fn list_models(&mut self) -> Result<Vec<String>, anyhow::Error> {
		Ok(self.list_registered_models())
	}
}

// ===== SSR Host Trait Implementation =====
//
// Implement the ssr Host trait for server-side rendering support.
// The trait is generated at crate::wasm::runtime::reinhardt::dentdelion::ssr::Host

#[cfg(feature = "wasm")]
type GeneratedRenderOptions = crate::wasm::runtime::reinhardt::dentdelion::ssr::RenderOptions;
#[cfg(feature = "wasm")]
type GeneratedRenderResult = crate::wasm::runtime::reinhardt::dentdelion::ssr::RenderResult;

/// Convert generated RenderOptions to internal RenderOptions
#[cfg(feature = "wasm")]
fn from_generated_render_options(options: GeneratedRenderOptions) -> RenderOptions {
	RenderOptions {
		include_hydration: options.include_hydration,
		extract_css: options.extract_css,
		extract_meta: options.extract_meta,
	}
}

/// Convert internal RenderResult to generated RenderResult
#[cfg(feature = "wasm")]
fn to_generated_render_result(result: RenderResult) -> GeneratedRenderResult {
	GeneratedRenderResult {
		html: result.html,
		css: result.css,
		meta: result.meta,
		hydration_script: result.hydration_script,
	}
}

/// Convert internal SsrError to generated PluginError
#[cfg(feature = "wasm")]
fn ssr_error_to_plugin_error(err: SsrError) -> GeneratedPluginError {
	let code = match &err {
		SsrError::NotAvailable => 503,
		SsrError::ComponentNotFound(_) => 404,
		SsrError::PropsSerialization(_) => 400,
		SsrError::RenderFailed(_) => 500,
		SsrError::EvalFailed(_) => 500,
		SsrError::PermissionDenied(_) => 403,
		SsrError::DangerousPattern(_) => 403,
	};
	GeneratedPluginError {
		code,
		message: err.to_string(),
		details: None,
	}
}

#[cfg(feature = "wasm")]
impl crate::wasm::runtime::reinhardt::dentdelion::ssr::Host for HostState {
	async fn render_react(
		&mut self,
		component_path: String,
		props: Vec<u8>,
		options: GeneratedRenderOptions,
	) -> Result<Result<GeneratedRenderResult, GeneratedPluginError>, anyhow::Error> {
		let internal_options = from_generated_render_options(options);
		let result = self
			.ssr_proxy
			.render_react(&component_path, &props, internal_options)
			.await
			.map(to_generated_render_result)
			.map_err(ssr_error_to_plugin_error);
		Ok(result)
	}

	async fn eval_js(
		&mut self,
		code: String,
	) -> Result<Result<Vec<u8>, GeneratedPluginError>, anyhow::Error> {
		let result = Self::eval_js(self, &code).map_err(ssr_error_to_plugin_error);
		Ok(result)
	}

	async fn is_available(&mut self) -> Result<bool, anyhow::Error> {
		Ok(self.is_ssr_available())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_host_state_config() {
		let state = HostState::new("test-plugin");

		// Set and get config
		state
			.set_config("key1", ConfigValue::StringVal("value1".to_string()))
			.unwrap();
		assert_eq!(
			state.get_config("key1"),
			Some(ConfigValue::StringVal("value1".to_string()))
		);

		// Non-existent key
		assert!(state.get_config("nonexistent").is_none());
	}

	#[test]
	fn test_host_state_services() {
		let state = HostState::new("test-plugin");

		// Register and get service
		let data = vec![1, 2, 3, 4];
		state.register_service("my-service", data.clone()).unwrap();
		assert_eq!(state.get_service("my-service"), Some(data));

		// Unregister service
		state.unregister_service("my-service").unwrap();
		assert!(state.get_service("my-service").is_none());
	}

	#[test]
	fn test_host_state_builder() {
		let state = HostStateBuilder::new("test-plugin")
			.config("key1", ConfigValue::IntVal(42))
			.config("key2", ConfigValue::BoolVal(true))
			.build();

		assert_eq!(state.plugin_name, "test-plugin");
		assert_eq!(state.get_config("key1"), Some(ConfigValue::IntVal(42)));
		assert_eq!(state.get_config("key2"), Some(ConfigValue::BoolVal(true)));
	}

	#[test]
	fn test_host_state_capabilities() {
		use crate::capability::PluginCapability;

		let mut state = HostState::new("test-plugin");
		let cap = Capability::Core(PluginCapability::Middleware);

		assert!(!state.has_capability(&cap));

		state.add_capability(cap.clone());
		assert!(state.has_capability(&cap));
	}

	#[test]
	fn test_host_state_event_bus() {
		// Create two host states sharing the same event bus
		let event_bus = Arc::new(EventBus::new());

		let producer = HostStateBuilder::new("producer-plugin")
			.event_bus(event_bus.clone())
			.build();

		let consumer = HostStateBuilder::new("consumer-plugin")
			.event_bus(event_bus.clone())
			.build();

		// Consumer subscribes to user events
		let sub_id = consumer.subscribe_events("user.*").unwrap();

		// Producer emits an event
		let delivered = producer.emit_event("user.created", vec![1, 2, 3]);
		assert_eq!(delivered, 1);

		// Consumer polls events
		let events = consumer.poll_pending_events(sub_id, 10);
		assert_eq!(events.len(), 1);
		assert_eq!(events[0].name, "user.created");
		assert_eq!(events[0].source, "producer-plugin");
		assert_eq!(events[0].payload, vec![1, 2, 3]);
	}

	#[test]
	fn test_host_state_event_bus_isolation() {
		// Without sharing an event bus, plugins are isolated
		let plugin_a = HostState::new("plugin-a");
		let plugin_b = HostState::new("plugin-b");

		let sub_id = plugin_b.subscribe_events("*").unwrap();
		plugin_a.emit_event("test.event", vec![]);

		// plugin_b won't see the event because they have different event buses
		let events = plugin_b.poll_pending_events(sub_id, 10);
		assert!(events.is_empty());
	}

	#[test]
	fn test_host_state_event_unsubscribe() {
		let state = HostState::new("test-plugin");

		let sub_id = state.subscribe_events("*").unwrap();
		assert!(state.unsubscribe_events(sub_id));
		assert!(!state.unsubscribe_events(sub_id)); // Already unsubscribed
	}

	#[test]
	fn test_host_state_model_registry() {
		let state = HostState::new("test-plugin");

		// Register a model schema
		let schema = ModelSchema::new("users")
			.column(ColumnDef {
				name: "id".to_string(),
				column_type: ColumnType::Integer,
				nullable: false,
				primary_key: true,
				unique_value: true,
				default_value: None,
			})
			.column(ColumnDef {
				name: "email".to_string(),
				column_type: ColumnType::Varchar(255),
				nullable: false,
				primary_key: false,
				unique_value: true,
				default_value: None,
			});

		assert!(state.register_model_schema(schema).is_ok());

		// List models
		let models = state.list_registered_models();
		assert_eq!(models.len(), 1);
		assert!(models.contains(&"users".to_string()));

		// Get model
		let retrieved = state.get_registered_model("users");
		assert!(retrieved.is_some());
		assert_eq!(retrieved.unwrap().table_name, "users");

		// Non-existent model
		assert!(state.get_registered_model("nonexistent").is_none());
	}

	#[test]
	fn test_host_state_model_registry_duplicate() {
		let state = HostState::new("test-plugin");

		let schema = ModelSchema::new("products");
		assert!(state.register_model_schema(schema.clone()).is_ok());

		// Duplicate registration fails
		assert!(state.register_model_schema(schema).is_err());
	}

	#[test]
	fn test_host_state_migration_registry() {
		let state = HostState::new("test-plugin");

		let migration = SqlMigration::new(
			"0001_initial",
			"Create initial tables",
			"CREATE TABLE users (id INT PRIMARY KEY);",
			"DROP TABLE users;",
		);

		assert!(state.register_sql_migration(migration).is_ok());

		// List migrations
		let migrations = state.list_registered_migrations();
		assert_eq!(migrations.len(), 1);
		assert_eq!(migrations[0].version, "0001_initial");
	}

	#[test]
	fn test_host_state_shared_model_registry() {
		// Create two host states sharing the same model registry
		let model_registry = Arc::new(ModelRegistry::new());

		let plugin_a = HostStateBuilder::new("plugin-a")
			.model_registry(model_registry.clone())
			.build();

		let plugin_b = HostStateBuilder::new("plugin-b")
			.model_registry(model_registry.clone())
			.build();

		// Register models from both plugins
		plugin_a
			.register_model_schema(ModelSchema::new("users"))
			.unwrap();
		plugin_b
			.register_model_schema(ModelSchema::new("products"))
			.unwrap();

		// Each plugin can only see its own models
		assert_eq!(plugin_a.list_registered_models().len(), 1);
		assert!(
			plugin_a
				.list_registered_models()
				.contains(&"users".to_string())
		);

		assert_eq!(plugin_b.list_registered_models().len(), 1);
		assert!(
			plugin_b
				.list_registered_models()
				.contains(&"products".to_string())
		);

		// But the shared registry contains both
		assert_eq!(model_registry.schema_count(), 2);
	}

	#[test]
	fn test_host_state_builder_with_model_registry() {
		let model_registry = Arc::new(ModelRegistry::new());

		let state = HostStateBuilder::new("test-plugin")
			.model_registry(model_registry.clone())
			.build();

		// Verify the registry is shared
		assert!(Arc::ptr_eq(state.model_registry(), &model_registry));
	}

	#[test]
	fn test_host_state_ssr_proxy_default() {
		let state = HostState::new("test-plugin");

		// SSR is not available by default
		assert!(!state.is_ssr_available());
	}

	#[test]
	fn test_host_state_ssr_proxy_with_availability() {
		let ssr_proxy = Arc::new(SsrProxy::with_availability(true));

		let state = HostStateBuilder::new("test-plugin")
			.ssr_proxy(ssr_proxy.clone())
			.build();

		// SSR is available when explicitly enabled
		assert!(state.is_ssr_available());

		// Verify the proxy is shared
		assert!(Arc::ptr_eq(state.ssr_proxy(), &ssr_proxy));
	}

	#[tokio::test]
	async fn test_host_state_render_react_not_available() {
		// Verified plugin without SSR runtime should return NotAvailable
		let state = HostStateBuilder::new("test-plugin")
			.trust_level(TrustLevel::Verified)
			.build();

		let result = state
			.render_react("test.jsx", &[], RenderOptions::default())
			.await;

		// Should return NotAvailable error since SSR is disabled
		assert!(matches!(result, Err(SsrError::NotAvailable)));
	}

	#[test]
	fn test_host_state_eval_js_not_available() {
		// Trusted plugin without SSR runtime should return NotAvailable
		let state = HostStateBuilder::new("test-plugin")
			.trust_level(TrustLevel::Trusted)
			.build();

		let result = state.eval_js("console.log('test')");

		// Should return NotAvailable error since SSR is disabled
		assert!(matches!(result, Err(SsrError::NotAvailable)));
	}

	#[test]
	fn test_host_state_builder_with_ssr_proxy() {
		let ssr_proxy = Arc::new(SsrProxy::with_availability(true));

		let state = HostStateBuilder::new("test-plugin")
			.ssr_proxy(ssr_proxy.clone())
			.build();

		// Verify the proxy is shared
		assert!(Arc::ptr_eq(state.ssr_proxy(), &ssr_proxy));
		assert!(state.is_ssr_available());
	}

	// =========================================================================
	// Trust Level Enforcement Tests (Issue #675 + #677)
	// =========================================================================

	#[test]
	fn test_host_state_default_trust_level() {
		// Arrange
		let state = HostState::new("test-plugin");

		// Act & Assert
		assert_eq!(state.trust_level(), TrustLevel::Untrusted);
	}

	#[test]
	fn test_host_state_builder_trust_level() {
		// Arrange & Act
		let state = HostStateBuilder::new("test-plugin")
			.trust_level(TrustLevel::Trusted)
			.build();

		// Assert
		assert_eq!(state.trust_level(), TrustLevel::Trusted);
	}

	#[test]
	fn test_eval_js_denied_for_untrusted() {
		// Arrange
		let state = HostStateBuilder::new("untrusted-plugin")
			.trust_level(TrustLevel::Untrusted)
			.build();

		// Act
		let result = state.eval_js("1 + 1");

		// Assert
		assert!(matches!(result, Err(SsrError::PermissionDenied(_))));
	}

	#[test]
	fn test_eval_js_denied_for_verified() {
		// Arrange
		let state = HostStateBuilder::new("verified-plugin")
			.trust_level(TrustLevel::Verified)
			.build();

		// Act
		let result = state.eval_js("1 + 1");

		// Assert
		assert!(matches!(result, Err(SsrError::PermissionDenied(_))));
	}

	#[test]
	fn test_eval_js_allowed_for_trusted_returns_not_available() {
		// Arrange
		let state = HostStateBuilder::new("trusted-plugin")
			.trust_level(TrustLevel::Trusted)
			.build();

		// Act
		let result = state.eval_js("1 + 1");

		// Assert - passes trust check, fails with NotAvailable (no runtime)
		assert!(matches!(result, Err(SsrError::NotAvailable)));
	}

	#[test]
	fn test_render_component_denied_for_untrusted() {
		// Arrange
		let state = HostStateBuilder::new("untrusted-plugin")
			.trust_level(TrustLevel::Untrusted)
			.build();

		// Act
		let result = state.render_component(
			"function Component() { return h('div', null, 'test'); }",
			"{}",
			RenderOptions::default(),
		);

		// Assert
		assert!(matches!(result, Err(SsrError::PermissionDenied(_))));
	}

	#[test]
	fn test_render_component_allowed_for_verified() {
		// Arrange
		let state = HostStateBuilder::new("verified-plugin")
			.trust_level(TrustLevel::Verified)
			.build();

		// Act
		let result = state.render_component(
			"function Component() { return h('div', null, 'test'); }",
			"{}",
			RenderOptions::default(),
		);

		// Assert - passes trust check, fails with NotAvailable (no runtime)
		assert!(matches!(result, Err(SsrError::NotAvailable)));
	}

	#[test]
	fn test_render_component_allowed_for_trusted() {
		// Arrange
		let state = HostStateBuilder::new("trusted-plugin")
			.trust_level(TrustLevel::Trusted)
			.build();

		// Act
		let result = state.render_component(
			"function Component() { return h('div', null, 'test'); }",
			"{}",
			RenderOptions::default(),
		);

		// Assert - passes trust check, fails with NotAvailable (no runtime)
		assert!(matches!(result, Err(SsrError::NotAvailable)));
	}

	#[tokio::test]
	async fn test_render_react_denied_for_untrusted() {
		// Arrange
		let state = HostStateBuilder::new("untrusted-plugin")
			.trust_level(TrustLevel::Untrusted)
			.build();

		// Act
		let result = state
			.render_react("test.jsx", &[], RenderOptions::default())
			.await;

		// Assert
		assert!(matches!(result, Err(SsrError::PermissionDenied(_))));
	}

	#[test]
	fn test_render_component_rejects_dangerous_code_at_host_level() {
		// Arrange - Verified plugin with SSR available
		let ssr_proxy = Arc::new(SsrProxy::with_availability(true));
		let state = HostStateBuilder::new("verified-plugin")
			.trust_level(TrustLevel::Verified)
			.ssr_proxy(ssr_proxy)
			.build();

		// Act - attempt to render with dangerous code
		let result = state.render_component(
			"var fs = require('fs'); function Component() { return h('div'); }",
			"{}",
			RenderOptions::default(),
		);

		// Assert - dangerous pattern detected by SsrProxy
		assert!(matches!(result, Err(SsrError::DangerousPattern(_))));
	}

	// ===== SSRF Validation Tests =====

	#[test]
	fn test_ssrf_rejects_file_scheme() {
		// Arrange
		let url = "file:///etc/passwd";

		// Act
		let result = validate_url_for_ssrf(url);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.code, 403);
	}

	#[test]
	fn test_ssrf_rejects_gopher_scheme() {
		// Arrange
		let url = "gopher://evil.com/";

		// Act
		let result = validate_url_for_ssrf(url);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.code, 403);
	}

	#[test]
	fn test_ssrf_rejects_ftp_scheme() {
		// Arrange
		let url = "ftp://internal-server/data";

		// Act
		let result = validate_url_for_ssrf(url);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.code, 403);
	}

	#[test]
	fn test_ssrf_rejects_loopback_ipv4() {
		// Arrange
		let url = "http://127.0.0.1/admin";

		// Act
		let result = validate_url_for_ssrf(url);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.code, 403);
	}

	#[test]
	fn test_ssrf_rejects_loopback_ipv4_other() {
		// Arrange
		let url = "http://127.0.0.2:8080/internal";

		// Act
		let result = validate_url_for_ssrf(url);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.code, 403);
	}

	#[test]
	fn test_ssrf_rejects_private_10_network() {
		// Arrange
		let url = "http://10.0.0.1/";

		// Act
		let result = validate_url_for_ssrf(url);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.code, 403);
	}

	#[test]
	fn test_ssrf_rejects_private_172_network() {
		// Arrange
		let url = "http://172.16.0.1/";

		// Act
		let result = validate_url_for_ssrf(url);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.code, 403);
	}

	#[test]
	fn test_ssrf_rejects_private_192_168_network() {
		// Arrange
		let url = "http://192.168.1.1/";

		// Act
		let result = validate_url_for_ssrf(url);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.code, 403);
	}

	#[test]
	fn test_ssrf_rejects_link_local() {
		// Arrange
		let url = "http://169.254.169.254/latest/meta-data/";

		// Act
		let result = validate_url_for_ssrf(url);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.code, 403);
	}

	#[test]
	fn test_ssrf_rejects_ipv6_loopback() {
		// Arrange
		let url = "http://[::1]/admin";

		// Act
		let result = validate_url_for_ssrf(url);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.code, 403);
	}

	#[test]
	fn test_ssrf_rejects_localhost() {
		// Arrange
		let url = "http://localhost/admin";

		// Act
		let result = validate_url_for_ssrf(url);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.code, 403);
	}

	#[test]
	fn test_ssrf_rejects_zero_ip() {
		// Arrange
		let url = "http://0.0.0.0/";

		// Act
		let result = validate_url_for_ssrf(url);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.code, 403);
	}

	#[test]
	fn test_ssrf_rejects_invalid_url() {
		// Arrange
		let url = "not-a-valid-url";

		// Act
		let result = validate_url_for_ssrf(url);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.code, 400);
	}

	#[test]
	fn test_ssrf_allows_public_http() {
		// Arrange
		let url = "http://example.com/api/data";

		// Act
		let result = validate_url_for_ssrf(url);

		// Assert
		assert!(result.is_ok());
	}

	#[test]
	fn test_ssrf_allows_public_https() {
		// Arrange
		let url = "https://api.github.com/repos";

		// Act
		let result = validate_url_for_ssrf(url);

		// Assert
		assert!(result.is_ok());
	}

	#[test]
	fn test_is_private_ip_loopback_v4() {
		// Arrange
		let ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));

		// Act & Assert
		assert!(is_private_ip(&ip));
	}

	#[test]
	fn test_is_private_ip_private_10() {
		// Arrange
		let ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(10, 1, 2, 3));

		// Act & Assert
		assert!(is_private_ip(&ip));
	}

	#[test]
	fn test_is_private_ip_private_172() {
		// Arrange
		let ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(172, 16, 0, 1));

		// Act & Assert
		assert!(is_private_ip(&ip));
	}

	#[test]
	fn test_is_private_ip_private_192_168() {
		// Arrange
		let ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 0, 1));

		// Act & Assert
		assert!(is_private_ip(&ip));
	}

	#[test]
	fn test_is_private_ip_link_local() {
		// Arrange
		let ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(169, 254, 1, 1));

		// Act & Assert
		assert!(is_private_ip(&ip));
	}

	#[test]
	fn test_is_private_ip_public() {
		// Arrange
		let ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(8, 8, 8, 8));

		// Act & Assert
		assert!(!is_private_ip(&ip));
	}

	#[test]
	fn test_is_private_ip_ipv6_loopback() {
		// Arrange
		let ip = std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST);

		// Act & Assert
		assert!(is_private_ip(&ip));
	}

	#[test]
	fn test_is_private_ip_ipv6_unique_local() {
		// Arrange - fc00::/7
		let ip = std::net::IpAddr::V6(std::net::Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 1));

		// Act & Assert
		assert!(is_private_ip(&ip));
	}

	#[test]
	fn test_is_private_ip_ipv6_link_local() {
		// Arrange - fe80::/10
		let ip = std::net::IpAddr::V6(std::net::Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1));

		// Act & Assert
		assert!(is_private_ip(&ip));
	}

	#[test]
	fn test_is_private_ip_172_not_in_range() {
		// Arrange - 172.32.0.1 is NOT in 172.16.0.0/12 range
		let ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(172, 32, 0, 1));

		// Act & Assert
		assert!(!is_private_ip(&ip));
	}

	// ==========================================================================
	// Clone Preservation Tests (#682)
	// ==========================================================================

	#[rstest::rstest]
	fn test_clone_preserves_config() {
		// Arrange
		let state = HostState::new("test-plugin");
		state
			.set_config(
				"db_url",
				ConfigValue::StringVal("postgres://localhost".to_string()),
			)
			.unwrap();
		state
			.set_config("max_retries", ConfigValue::IntVal(3))
			.unwrap();

		// Act
		let cloned = state.clone();

		// Assert
		assert_eq!(cloned.plugin_name, "test-plugin");
		assert_eq!(
			cloned.get_config("db_url"),
			Some(ConfigValue::StringVal("postgres://localhost".to_string()))
		);
		assert_eq!(
			cloned.get_config("max_retries"),
			Some(ConfigValue::IntVal(3))
		);
	}

	#[rstest::rstest]
	fn test_clone_preserves_capabilities() {
		// Arrange
		use crate::capability::PluginCapability;

		let mut state = HostState::new("test-plugin");
		let network_cap = Capability::Core(PluginCapability::NetworkAccess);
		let db_cap = Capability::Core(PluginCapability::DatabaseAccess);
		state.add_capability(network_cap.clone());
		state.add_capability(db_cap.clone());

		// Act
		let cloned = state.clone();

		// Assert
		assert!(cloned.has_capability(&network_cap));
		assert!(cloned.has_capability(&db_cap));
		assert_eq!(cloned.capabilities().len(), 2);
	}

	#[rstest::rstest]
	fn test_clone_preserves_services() {
		// Arrange
		let state = HostState::new("test-plugin");
		let service_data = vec![10, 20, 30, 40];
		state
			.register_service("cache-service", service_data.clone())
			.unwrap();

		// Act
		let cloned = state.clone();

		// Assert
		assert_eq!(cloned.get_service("cache-service"), Some(service_data));
	}

	#[rstest::rstest]
	fn test_clone_shares_event_bus() {
		// Arrange
		let event_bus = Arc::new(EventBus::new());
		let state = HostStateBuilder::new("test-plugin")
			.event_bus(event_bus.clone())
			.build();

		// Act
		let cloned = state.clone();

		// Assert - shared event bus should be the same Arc
		assert!(Arc::ptr_eq(state.event_bus(), cloned.event_bus()));
	}

	#[rstest::rstest]
	fn test_clone_shares_model_registry() {
		// Arrange
		let model_registry = Arc::new(ModelRegistry::new());
		let state = HostStateBuilder::new("test-plugin")
			.model_registry(model_registry.clone())
			.build();

		// Act
		let cloned = state.clone();

		// Assert - shared model registry should be the same Arc
		assert!(Arc::ptr_eq(state.model_registry(), cloned.model_registry()));
	}

	#[rstest::rstest]
	fn test_clone_shares_ssr_proxy() {
		// Arrange
		let ssr_proxy = Arc::new(SsrProxy::with_availability(true));
		let state = HostStateBuilder::new("test-plugin")
			.ssr_proxy(ssr_proxy.clone())
			.build();

		// Act
		let cloned = state.clone();

		// Assert
		assert!(Arc::ptr_eq(state.ssr_proxy(), cloned.ssr_proxy()));
		assert!(cloned.is_ssr_available());
	}

	#[rstest::rstest]
	fn test_clone_preserves_no_http_client() {
		// Arrange
		let state = HostStateBuilder::new("test-plugin")
			.no_http_client()
			.build();

		// Act
		let cloned = state.clone();

		// Assert - http_client should remain None
		assert!(!cloned.has_http_capability());
	}
}
