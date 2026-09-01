//! Authentication State Management for Client-side WASM
//!
//! This module provides reactive authentication state management for client-side
//! WASM applications. It integrates with the `reinhardt-auth` session system
//! and provides a Django-like interface for accessing user information.
//!
//! ## Usage
//!
//! ```ignore
//! use reinhardt_pages::auth::{AuthState, auth_state};
//!
//! // Get the global auth state
//! let auth = auth_state();
//!
//! // Check authentication status (reactive)
//! if auth.is_authenticated() {
//!     println!("User: {}", auth.username().unwrap_or_default());
//! }
//!
//! // React to authentication changes
//! Effect::new(move || {
//!     if auth.is_authenticated() {
//!         // Show user dashboard
//!     } else {
//!         // Show login form
//!     }
//! });
//! ```

use crate::reactive::{ReactiveScope, Signal};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

thread_local! {
	static AUTHENTICATION_INVALIDATION_SCHEDULED: Cell<Option<u64>> = const { Cell::new(None) };
}

struct AuthenticationInvalidationScheduleGuard {
	generation: u64,
}

impl Drop for AuthenticationInvalidationScheduleGuard {
	fn drop(&mut self) {
		AUTHENTICATION_INVALIDATION_SCHEDULED.with(|scheduled| {
			if scheduled.get() == Some(self.generation) {
				scheduled.set(None);
			}
		});
	}
}

/// Authentication context captured when a generated server-function request starts.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServerFnAuthenticationContext {
	established: bool,
	generation: u64,
}

/// Deserializes a user ID that may be either a JSON string or a JSON number.
///
/// This provides backward compatibility: existing clients that send `"user_id": 42`
/// (integer) will have it converted to `Some("42")`, while new clients sending
/// `"user_id": "550e8400-..."` (UUID string) work directly.
fn deserialize_user_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
	D: serde::Deserializer<'de>,
{
	use serde::Deserialize;
	let value = Option::<serde_json::Value>::deserialize(deserializer)?;
	Ok(value.and_then(|v| match v {
		serde_json::Value::String(s) if s.is_empty() => None,
		serde_json::Value::String(s) => Some(s),
		serde_json::Value::Number(n) => Some(n.to_string()),
		serde_json::Value::Null => None,
		_ => None,
	}))
}

/// Session key for user ID (matches reinhardt-auth).
pub const SESSION_KEY_USER_ID: &str = "_auth_user_id";

/// Session key for username.
pub const SESSION_KEY_USERNAME: &str = "_auth_username";

/// Cookie name for session ID (matches reinhardt-auth).
pub const SESSION_COOKIE_NAME: &str = "sessionid";

/// Clears authentication-dependent client state and revalidates the active route.
///
/// Duplicate notifications are coalesced only while they refer to the current
/// [`AuthState`] generation. A login, update, or state-changing logout advances
/// that generation and supersedes any older replacement navigation.
pub fn invalidate_authentication() {
	let generation = auth_state().authentication_generation();
	invalidate_authentication_generation(generation);
}

fn invalidate_authentication_generation(generation: u64) {
	let coordinator = crate::app::try_with_navigation_coordinator(Rc::clone);
	if let Some(coordinator) = coordinator {
		let already_scheduled = AUTHENTICATION_INVALIDATION_SCHEDULED.with(|scheduled| {
			if scheduled.get() == Some(generation) {
				true
			} else {
				scheduled.set(Some(generation));
				false
			}
		});
		if already_scheduled {
			return;
		}

		coordinator.clear_for_authentication_change();
		let schedule_guard = AuthenticationInvalidationScheduleGuard { generation };
		crate::platform::spawn_task(async move {
			crate::platform::defer_yield().await;
			let path = coordinator.current_path();
			let _ = coordinator.navigate_with_completion_guard(
				path,
				crate::app::NavigationIntent::Replace,
				schedule_guard,
			);
		});
		return;
	}

	if let Some(client) = crate::reactive::query::current_query_client() {
		client.clear_for_authentication_change();
	}
	AUTHENTICATION_INVALIDATION_SCHEDULED.with(|scheduled| {
		if scheduled.get() == Some(generation) {
			scheduled.set(None);
		}
	});
}

/// Reports an HTTP status for a caller that already established authentication.
///
/// Generated server-function stubs use [`observe_server_fn_status_for_request`]
/// with a request-time context so an anonymous endpoint's expected 401 remains
/// an ordinary application error.
#[doc(hidden)]
pub fn observe_server_fn_status(status: u16) {
	if status == 401 {
		clear_jwt_token();
		auth_state().logout();
		invalidate_authentication();
	}
}

/// Captures whether a generated request carries an established authentication context.
#[doc(hidden)]
pub fn server_fn_authentication_context() -> ServerFnAuthenticationContext {
	let state = auth_state();
	ServerFnAuthenticationContext {
		established: state.is_authenticated_untracked() || get_jwt_token().is_some(),
		generation: state.authentication_generation(),
	}
}

/// Reports a generated server-function response against its request-time auth context.
#[doc(hidden)]
pub fn observe_server_fn_status_for_request(status: u16, context: ServerFnAuthenticationContext) {
	if status != 401 || !context.established {
		return;
	}

	let state = auth_state();
	let current_context_is_established =
		state.is_authenticated_untracked() || get_jwt_token().is_some();
	if context.generation != state.authentication_generation() || !current_context_is_established {
		return;
	}

	clear_jwt_token();
	state.logout();
	invalidate_authentication();
}

struct AuthStateStore {
	state: AuthState,
}

thread_local! {
	/// Global authentication state instance.
	static AUTH_STATE: RefCell<Option<AuthStateStore>> = const { RefCell::new(None) };
}

/// Returns the global authentication state.
///
/// This creates the state on first access and returns the same instance
/// for subsequent calls within the same thread.
pub fn auth_state() -> AuthState {
	AUTH_STATE.with(|state| {
		let mut state = state.borrow_mut();
		if state.is_none() {
			*state = Some(AuthStateStore {
				state: AuthState::new(),
			});
		}
		state
			.as_ref()
			.expect("authentication state must be initialized")
			.state
			.clone()
	})
}

/// Reactive authentication state for client-side applications.
///
/// This struct provides reactive signals that automatically update
/// when authentication state changes. It can be used to build
/// authentication-aware UI components.
#[derive(Clone)]
pub struct AuthState {
	/// Scope that owns this state's reactive signals.
	_scope: Rc<ReactiveScope>,
	/// Monotonic identity for authentication-bound cache and navigation work.
	authentication_generation: Rc<Cell<u64>>,
	/// Whether the user is authenticated.
	is_authenticated: Signal<bool>,
	/// The authenticated user's ID (string to support both integer and UUID PKs).
	user_id: Signal<Option<String>>,
	/// The authenticated user's username.
	username: Signal<Option<String>>,
	/// The authenticated user's email.
	email: Signal<Option<String>>,
	/// Whether the user is a staff member.
	is_staff: Signal<bool>,
	/// Whether the user is a superuser.
	is_superuser: Signal<bool>,
	/// User's permissions (cached from server).
	permissions: Signal<HashSet<String>>,
}

/// A signal exported from [`AuthState`] that retains the state's owner scope.
#[derive(Clone)]
pub struct AuthSignal<T: 'static> {
	_state: AuthState,
	signal: Signal<T>,
}

impl<T: 'static> std::ops::Deref for AuthSignal<T> {
	type Target = Signal<T>;

	fn deref(&self) -> &Self::Target {
		&self.signal
	}
}

impl<T: 'static> reinhardt_core::reactive::Trackable for AuthSignal<T> {
	fn node_id(&self) -> reinhardt_core::reactive::NodeId {
		self.signal.id()
	}
}

impl std::fmt::Debug for AuthState {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("AuthState").finish_non_exhaustive()
	}
}

impl Default for AuthState {
	fn default() -> Self {
		Self::new()
	}
}

impl AuthState {
	/// Creates a new authentication state with default (unauthenticated) values.
	pub fn new() -> Self {
		Self::from_data(AuthData::anonymous())
	}

	/// Creates an authentication state from server-provided data.
	///
	/// This is typically used during hydration when the server
	/// embeds authentication data in the initial HTML.
	pub fn from_server_data(data: AuthData) -> Self {
		Self::from_data(data)
	}

	fn from_data(data: AuthData) -> Self {
		let scope = Rc::new(ReactiveScope::new());
		scope.enter(|| Self {
			_scope: Rc::clone(&scope),
			authentication_generation: Rc::new(Cell::new(0)),
			is_authenticated: Signal::new(data.is_authenticated),
			user_id: Signal::new(data.user_id),
			username: Signal::new(data.username),
			email: Signal::new(data.email),
			is_staff: Signal::new(data.is_staff),
			is_superuser: Signal::new(data.is_superuser),
			permissions: Signal::new(data.permissions.into_iter().collect()),
		})
	}

	/// Returns whether the user is authenticated.
	pub fn is_authenticated(&self) -> bool {
		self.is_authenticated.get()
	}

	fn is_authenticated_untracked(&self) -> bool {
		self.is_authenticated.get_untracked()
	}

	fn authentication_generation(&self) -> u64 {
		self.authentication_generation.get()
	}

	fn advance_authentication_generation(&self) {
		self.authentication_generation
			.set(self.authentication_generation.get().wrapping_add(1));
	}

	/// Returns the authenticated user's ID.
	pub fn user_id(&self) -> Option<String> {
		self.user_id.get()
	}

	/// Returns the authenticated user's username.
	pub fn username(&self) -> Option<String> {
		self.username.get()
	}

	/// Returns the authenticated user's email.
	pub fn email(&self) -> Option<String> {
		self.email.get()
	}

	/// Returns whether the user is a staff member.
	pub fn is_staff(&self) -> bool {
		self.is_staff.get()
	}

	/// Returns whether the user is a superuser.
	pub fn is_superuser(&self) -> bool {
		self.is_superuser.get()
	}

	/// Returns the Signal for authentication status.
	///
	/// Use this for reactive UI updates.
	pub fn is_authenticated_signal(&self) -> AuthSignal<bool> {
		self.signal(self.is_authenticated)
	}

	/// Returns the Signal for user ID.
	pub fn user_id_signal(&self) -> AuthSignal<Option<String>> {
		self.signal(self.user_id)
	}

	/// Returns the Signal for username.
	pub fn username_signal(&self) -> AuthSignal<Option<String>> {
		self.signal(self.username)
	}

	/// Returns the Signal for email.
	pub fn email_signal(&self) -> AuthSignal<Option<String>> {
		self.signal(self.email)
	}

	/// Returns the Signal for staff status.
	pub fn is_staff_signal(&self) -> AuthSignal<bool> {
		self.signal(self.is_staff)
	}

	/// Returns the Signal for superuser status.
	pub fn is_superuser_signal(&self) -> AuthSignal<bool> {
		self.signal(self.is_superuser)
	}

	/// Updates the authentication state with new data.
	///
	/// This is typically called after a successful login or
	/// when session data is refreshed from the server.
	pub fn update(&self, data: AuthData) {
		self.advance_authentication_generation();
		self.is_authenticated.set(data.is_authenticated);
		self.user_id.set(data.user_id);
		self.username.set(data.username);
		self.email.set(data.email);
		self.is_staff.set(data.is_staff);
		self.is_superuser.set(data.is_superuser);
		self.permissions.set(data.permissions.into_iter().collect());
	}

	/// Sets the state to authenticated with the given user data.
	///
	/// Resets `email`, `is_staff`, `is_superuser`, and `permissions` to defaults
	/// to prevent stale data from a previous session.
	pub fn login(&self, user_id: impl Into<String>, username: impl Into<String>) {
		self.advance_authentication_generation();
		self.is_authenticated.set(true);
		self.user_id.set(Some(user_id.into()));
		self.username.set(Some(username.into()));
		self.email.set(None);
		self.is_staff.set(false);
		self.is_superuser.set(false);
		self.permissions.set(HashSet::new());
	}

	/// Sets the state to authenticated with full user data.
	pub fn login_full(
		&self,
		user_id: impl Into<String>,
		username: impl Into<String>,
		email: Option<String>,
		is_staff: bool,
		is_superuser: bool,
	) {
		self.advance_authentication_generation();
		self.is_authenticated.set(true);
		self.user_id.set(Some(user_id.into()));
		self.username.set(Some(username.into()));
		self.email.set(email);
		self.is_staff.set(is_staff);
		self.is_superuser.set(is_superuser);
	}

	/// Clears the authentication state (logout).
	pub fn logout(&self) {
		let changed = self.is_authenticated.get_untracked()
			|| self.user_id.get_untracked().is_some()
			|| self.username.get_untracked().is_some()
			|| self.email.get_untracked().is_some()
			|| self.is_staff.get_untracked()
			|| self.is_superuser.get_untracked()
			|| !self.permissions.get_untracked().is_empty();
		if changed {
			self.advance_authentication_generation();
		}
		self.is_authenticated.set(false);
		self.user_id.set(None);
		self.username.set(None);
		self.email.set(None);
		self.is_staff.set(false);
		self.is_superuser.set(false);
		self.permissions.set(HashSet::new());
	}

	/// Checks if the user has the given permission.
	///
	/// Note: This is a client-side check only. Always verify
	/// permissions on the server for security.
	pub fn has_permission(&self, permission: &str) -> bool {
		// Superusers have all permissions
		if self.is_superuser() {
			return true;
		}

		// Check permission cache
		self.permissions.get().contains(permission)
	}

	/// Checks if the user has any of the given permissions.
	///
	/// Note: This is a client-side check only. Always verify
	/// permissions on the server for security.
	pub fn has_any_permission(&self, permissions: &[&str]) -> bool {
		if self.is_superuser() {
			return true;
		}
		let cached = self.permissions.get();
		permissions.iter().any(|p| cached.contains(*p))
	}

	/// Checks if the user has all of the given permissions.
	///
	/// Note: This is a client-side check only. Always verify
	/// permissions on the server for security.
	pub fn has_all_permissions(&self, permissions: &[&str]) -> bool {
		if self.is_superuser() {
			return true;
		}
		let cached = self.permissions.get();
		permissions.iter().all(|p| cached.contains(*p))
	}

	/// Updates the permission cache.
	pub fn set_permissions(&self, permissions: HashSet<String>) {
		self.permissions.set(permissions);
	}

	/// Returns the Signal for permissions (for reactive updates).
	pub fn permissions_signal(&self) -> AuthSignal<HashSet<String>> {
		self.signal(self.permissions)
	}

	fn signal<T: 'static>(&self, signal: Signal<T>) -> AuthSignal<T> {
		AuthSignal {
			_state: self.clone(),
			signal,
		}
	}

	/// Fetches permissions from the server and updates the cache.
	///
	/// Default endpoint: `/api/auth/permissions`
	#[cfg(wasm)]
	pub async fn fetch_permissions(&self, endpoint: Option<&str>) -> Result<(), AuthError> {
		use crate::csrf::csrf_headers;
		use crate::fetch;

		let endpoint = endpoint.unwrap_or("/api/auth/permissions");
		let mut headers = Vec::new();
		if let Some((header_name, header_value)) = csrf_headers() {
			headers.push((header_name.to_string(), header_value));
		}

		let response = fetch::request("GET", endpoint, None, headers)
			.await
			.map_err(|e| AuthError::Network(e.to_string()))?;

		if !response.is_success() {
			return Err(AuthError::Server {
				status: response.status(),
				message: response.into_text(),
			});
		}

		let permissions: Vec<String> = response
			.json()
			.map_err(|e| AuthError::Parse(e.to_string()))?;

		self.permissions.set(permissions.into_iter().collect());
		Ok(())
	}

	/// Fetches permissions from the server (non-WASM stub).
	#[cfg(native)]
	pub async fn fetch_permissions(&self, _endpoint: Option<&str>) -> Result<(), AuthError> {
		Ok(())
	}

	/// Initializes the auth state from embedded data in the page.
	///
	/// This looks for a `<script id="auth-data">` element containing
	/// JSON-encoded authentication data.
	#[cfg(wasm)]
	pub fn init_from_page(&self) {
		use web_sys::window;

		let Some(window) = window() else { return };
		let Some(document) = window.document() else {
			return;
		};

		// Try to find embedded auth data
		let Ok(Some(element)) = document.query_selector("#auth-data") else {
			return;
		};

		let Some(json_str) = element.text_content() else {
			return;
		};

		if let Ok(data) = serde_json::from_str::<AuthData>(&json_str) {
			self.update(data);
		}
	}

	/// Initializes the auth state (non-WASM stub).
	#[cfg(native)]
	pub fn init_from_page(&self) {
		// No-op on non-WASM targets
	}

	/// Fetches the current auth state from the server.
	///
	/// This makes an AJAX request to the auth status endpoint
	/// and updates the state with the response.
	#[cfg(wasm)]
	pub async fn fetch_from_server(&self, endpoint: &str) -> Result<(), AuthError> {
		use crate::csrf::csrf_headers;
		use crate::fetch;

		let mut headers = Vec::new();
		if let Some((header_name, header_value)) = csrf_headers() {
			headers.push((header_name.to_string(), header_value));
		}

		let response = fetch::request("GET", endpoint, None, headers)
			.await
			.map_err(|e| AuthError::Network(e.to_string()))?;

		if !response.is_success() {
			return Err(AuthError::Server {
				status: response.status(),
				message: response.into_text(),
			});
		}

		let data: AuthData = response
			.json()
			.map_err(|e| AuthError::Parse(e.to_string()))?;

		self.update(data);
		Ok(())
	}

	/// Fetches the current auth state (non-WASM stub).
	#[cfg(native)]
	pub async fn fetch_from_server(&self, _endpoint: &str) -> Result<(), AuthError> {
		Ok(())
	}
}

/// Authentication data that can be serialized/deserialized.
///
/// This is used for:
/// - Embedding auth data in SSR HTML
/// - Server responses to auth status requests
/// - Hydration of client-side auth state
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct AuthData {
	/// Whether the user is authenticated.
	pub is_authenticated: bool,
	/// The authenticated user's ID (string to support both integer and UUID PKs).
	#[serde(default, deserialize_with = "deserialize_user_id")]
	pub user_id: Option<String>,
	/// The authenticated user's username.
	#[serde(default)]
	pub username: Option<String>,
	/// The authenticated user's email.
	#[serde(default)]
	pub email: Option<String>,
	/// Whether the user is a staff member.
	#[serde(default)]
	pub is_staff: bool,
	/// Whether the user is a superuser.
	#[serde(default)]
	pub is_superuser: bool,
	/// User's permissions.
	#[serde(default)]
	pub permissions: Vec<String>,
}

impl AuthData {
	/// Creates anonymous (unauthenticated) auth data.
	pub fn anonymous() -> Self {
		Self::default()
	}

	/// Creates authenticated auth data with minimal info.
	pub fn authenticated(user_id: impl Into<String>, username: impl Into<String>) -> Self {
		Self {
			is_authenticated: true,
			user_id: Some(user_id.into()),
			username: Some(username.into()),
			..Default::default()
		}
	}

	/// Creates authenticated auth data with full info.
	pub fn full(
		user_id: impl Into<String>,
		username: impl Into<String>,
		email: Option<String>,
		is_staff: bool,
		is_superuser: bool,
	) -> Self {
		Self {
			is_authenticated: true,
			user_id: Some(user_id.into()),
			username: Some(username.into()),
			email,
			is_staff,
			is_superuser,
			permissions: Vec::new(),
		}
	}
}

/// Errors that can occur during authentication operations.
#[derive(Debug, Clone)]
pub enum AuthError {
	/// Network error during request.
	Network(String),
	/// Server returned an error response.
	Server {
		/// HTTP status code.
		status: u16,
		/// Error message.
		message: String,
	},
	/// Failed to parse response.
	Parse(String),
}

impl std::fmt::Display for AuthError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			AuthError::Network(msg) => write!(f, "Network error: {}", msg),
			AuthError::Server { status, message } => {
				write!(f, "Server error ({}): {}", status, message)
			}
			AuthError::Parse(msg) => write!(f, "Parse error: {}", msg),
		}
	}
}

impl std::error::Error for AuthError {}

// ============================================================================
// JWT Token Management for Client-side WASM
// ============================================================================

/// The HTTP header name for JWT Bearer token authentication.
pub const AUTH_HEADER_NAME: &str = "Authorization";

/// The sessionStorage key for the admin JWT token.
///
/// sessionStorage is preferred over localStorage for admin panels because:
/// - Tokens are scoped per tab and cleared when the tab closes
/// - Reduces exposure from XSS attacks on other tabs
pub const JWT_STORAGE_KEY: &str = "__admin_jwt";

/// Creates HTTP headers with JWT Bearer token for authenticated requests.
///
/// Returns a tuple of (header_name, header_value) if a JWT token is available
/// in sessionStorage. This follows the same pattern as [`crate::csrf::csrf_headers`].
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::auth::auth_headers;
///
/// if let Some((header_name, header_value)) = auth_headers() {
///     // header_name = "Authorization"
///     // header_value = "Bearer eyJhbGciOi..."
/// }
/// ```
#[cfg(wasm)]
pub fn auth_headers() -> Option<(&'static str, String)> {
	get_jwt_token().map(|token| (AUTH_HEADER_NAME, format!("Bearer {}", token)))
}

/// Creates HTTP headers with JWT Bearer token (non-WASM stub).
#[cfg(native)]
pub fn auth_headers() -> Option<(&'static str, String)> {
	None
}

/// Retrieves the JWT token from sessionStorage.
///
/// Returns `None` if no token is stored or if sessionStorage is unavailable.
#[cfg(wasm)]
pub fn get_jwt_token() -> Option<String> {
	let window = web_sys::window()?;
	let storage = window.session_storage().ok()??;
	storage.get_item(JWT_STORAGE_KEY).ok()?
}

/// Retrieves the JWT token (non-WASM stub).
#[cfg(native)]
pub fn get_jwt_token() -> Option<String> {
	None
}

/// Stores a JWT token in sessionStorage.
///
/// The token persists for the lifetime of the browser tab.
#[cfg(wasm)]
pub fn set_jwt_token(token: &str) {
	if let Some(window) = web_sys::window()
		&& let Ok(Some(storage)) = window.session_storage()
	{
		let _ = storage.set_item(JWT_STORAGE_KEY, token);
	}
}

/// Stores a JWT token (non-WASM stub).
#[cfg(native)]
pub fn set_jwt_token(_token: &str) {
	// No-op on non-WASM targets
}

/// Removes the JWT token from sessionStorage.
///
/// This should be called on logout or when a 401 response is received.
#[cfg(wasm)]
pub fn clear_jwt_token() {
	if let Some(window) = web_sys::window()
		&& let Ok(Some(storage)) = window.session_storage()
	{
		let _ = storage.remove_item(JWT_STORAGE_KEY);
	}
}

/// Removes the JWT token (non-WASM stub).
#[cfg(native)]
pub fn clear_jwt_token() {
	// No-op on non-WASM targets
}

#[cfg(test)]
mod tests {
	use super::*;

	fn clear_global_auth_state() {
		let previous = AUTH_STATE.with(|state| state.borrow_mut().take());
		drop(previous);
	}

	#[test]
	fn test_auth_state_creation() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let state = AuthState::new();
			assert!(!state.is_authenticated());
			assert!(state.user_id().is_none());
			assert!(state.username().is_none());
		});
	}

	#[test]
	fn auth_state_constructors_retain_a_scope_without_an_active_parent_scope() {
		let state = AuthState::new();
		let is_authenticated = state.is_authenticated_signal();
		state.login("42", "testuser");
		assert!(is_authenticated.get());

		let default_state = AuthState::default();
		assert!(!default_state.is_authenticated());

		let server_state = AuthState::from_server_data(AuthData::authenticated("7", "serveruser"));
		assert!(server_state.is_authenticated());
		assert_eq!(server_state.username(), Some("serveruser".to_string()));
	}

	#[rstest::rstest]
	#[serial_test::serial(reactive_runtime)]
	fn exported_auth_signal_retains_its_owner() {
		let is_authenticated = AuthState::new().is_authenticated_signal();

		assert!(!is_authenticated.get());
	}

	#[test]
	fn test_auth_state_login() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let state = AuthState::new();
			state.login("42", "testuser");

			assert!(state.is_authenticated());
			assert_eq!(state.user_id(), Some("42".to_string()));
			assert_eq!(state.username(), Some("testuser".to_string()));
		});
	}

	#[test]
	fn test_auth_state_logout() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let state = AuthState::new();
			state.login("42", "testuser");
			state.logout();

			assert!(!state.is_authenticated());
			assert!(state.user_id().is_none());
			assert!(state.username().is_none());
		});
	}

	#[test]
	fn test_auth_state_from_server_data() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let data = AuthData::full(
				"1",
				"admin",
				Some("admin@example.com".to_string()),
				true,
				true,
			);
			let state = AuthState::from_server_data(data);

			assert!(state.is_authenticated());
			assert_eq!(state.user_id(), Some("1".to_string()));
			assert_eq!(state.username(), Some("admin".to_string()));
			assert_eq!(state.email(), Some("admin@example.com".to_string()));
			assert!(state.is_staff());
			assert!(state.is_superuser());
		});
	}

	#[test]
	fn test_auth_data_anonymous() {
		let data = AuthData::anonymous();
		assert!(!data.is_authenticated);
		assert!(data.user_id.is_none());
	}

	#[test]
	fn test_auth_data_authenticated() {
		let data = AuthData::authenticated("1", "user");
		assert!(data.is_authenticated);
		assert_eq!(data.user_id, Some("1".to_string()));
		assert_eq!(data.username, Some("user".to_string()));
	}

	#[test]
	fn test_auth_state_update() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let state = AuthState::new();
			let data = AuthData::authenticated("99", "updated");
			state.update(data);

			assert!(state.is_authenticated());
			assert_eq!(state.user_id(), Some("99".to_string()));
			assert_eq!(state.username(), Some("updated".to_string()));
		});
	}

	#[test]
	fn test_global_auth_state() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let state1 = auth_state();
			let state2 = auth_state();

			state1.login("1", "test");
			assert!(state2.is_authenticated());
		});
	}

	#[test]
	fn global_auth_state_outlives_initializer_scope() {
		clear_global_auth_state();

		reinhardt_core::reactive::ReactiveScope::run(|| {
			auth_state().login("1", "test");
		});

		reinhardt_core::reactive::ReactiveScope::run(|| {
			let state = auth_state();
			assert!(state.is_authenticated());
			assert_eq!(state.username(), Some("test".to_string()));
		});

		clear_global_auth_state();
	}

	#[test]
	fn test_auth_error_display() {
		let network_err = AuthError::Network("timeout".to_string());
		assert_eq!(network_err.to_string(), "Network error: timeout");

		let server_err = AuthError::Server {
			status: 401,
			message: "Unauthorized".to_string(),
		};
		assert_eq!(server_err.to_string(), "Server error (401): Unauthorized");

		let parse_err = AuthError::Parse("invalid json".to_string());
		assert_eq!(parse_err.to_string(), "Parse error: invalid json");
	}

	#[test]
	fn test_has_permission_with_cache() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let state = AuthState::new();
			let mut perms = HashSet::new();
			perms.insert("blog.add_post".to_string());
			perms.insert("blog.edit_post".to_string());
			state.set_permissions(perms);

			assert!(state.has_permission("blog.add_post"));
			assert!(!state.has_permission("blog.delete_post"));
		});
	}

	#[test]
	fn test_superuser_has_all_permissions() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let state = AuthState::new();
			state.login_full("1", "admin", None, true, true);

			assert!(state.has_permission("any.permission"));
			assert!(state.has_permission("another.permission"));
		});
	}

	#[test]
	fn test_has_any_permission() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let state = AuthState::new();
			let mut perms = HashSet::new();
			perms.insert("blog.view".to_string());
			state.set_permissions(perms);

			assert!(state.has_any_permission(&["blog.view", "blog.edit"]));
			assert!(!state.has_any_permission(&["blog.delete", "blog.edit"]));
		});
	}

	#[test]
	fn test_has_all_permissions() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let state = AuthState::new();
			let mut perms = HashSet::new();
			perms.insert("blog.view".to_string());
			perms.insert("blog.edit".to_string());
			state.set_permissions(perms);

			assert!(state.has_all_permissions(&["blog.view", "blog.edit"]));
			assert!(!state.has_all_permissions(&["blog.view", "blog.delete"]));
		});
	}

	#[test]
	fn test_permissions_cleared_on_logout() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let state = AuthState::new();
			let mut perms = HashSet::new();
			perms.insert("blog.add_post".to_string());
			state.set_permissions(perms);
			state.login("1", "user");

			state.logout();

			assert!(!state.has_permission("blog.add_post"));
			assert_eq!(state.permissions.get().len(), 0);
		});
	}

	#[test]
	fn test_permissions_from_auth_data() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let data = AuthData {
				is_authenticated: true,
				user_id: Some("1".to_string()),
				username: Some("user".to_string()),
				email: None,
				is_staff: false,
				is_superuser: false,
				permissions: vec!["blog.view".to_string(), "blog.edit".to_string()],
			};
			let state = AuthState::from_server_data(data);

			assert!(state.has_permission("blog.view"));
			assert!(state.has_permission("blog.edit"));
			assert!(!state.has_permission("blog.delete"));
		});
	}

	#[test]
	fn test_permissions_update() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let state = AuthState::new();
			state.login("1", "user");

			let data = AuthData {
				is_authenticated: true,
				user_id: Some("1".to_string()),
				username: Some("user".to_string()),
				email: None,
				is_staff: false,
				is_superuser: false,
				permissions: vec!["blog.view".to_string()],
			};
			state.update(data);

			assert!(state.has_permission("blog.view"));
			assert!(!state.has_permission("blog.edit"));
		});
	}

	#[test]
	fn test_auth_headers_non_wasm() {
		// On non-WASM targets, auth_headers always returns None
		assert!(auth_headers().is_none());
	}

	#[test]
	#[cfg(not(target_arch = "wasm32"))]
	fn authentication_invalidation_clears_current_query_client_and_ignores_non_auth_statuses() {
		use crate::reactive::query::{
			QueryClient, QueryDefaults, QueryFamily, QueryOptions, TestQueryRuntime,
			provide_query_client,
		};

		let runtime = TestQueryRuntime::new();
		let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
		let family = QueryFamily::<(), String, String>::new("auth.invalidation-status");
		let query = client.observe(
			family.query((), || async { Ok("authenticated".to_owned()) }),
			QueryOptions::default(),
		);
		runtime.run_until_stalled();
		assert_eq!(query.data(), Some("authenticated".to_owned()));

		let _client_guard = provide_query_client(client);
		let state = auth_state();
		state.login("1", "authenticated");
		observe_server_fn_status(403);
		assert_eq!(query.data(), Some("authenticated".to_owned()));
		observe_server_fn_status(500);
		assert_eq!(query.data(), Some("authenticated".to_owned()));
		observe_server_fn_status(401);
		assert_eq!(query.data(), None);
		assert!(!state.is_authenticated());
		assert_eq!(state.user_id(), None);
		assert_eq!(state.username(), None);
		state.logout();
	}

	#[rstest::rstest]
	#[case(false, false)]
	#[case(true, true)]
	#[cfg(not(target_arch = "wasm32"))]
	fn request_scoped_401_requires_the_current_established_authentication(
		#[case] authenticated: bool,
		#[case] expect_invalidation: bool,
	) {
		use crate::reactive::query::{
			QueryClient, QueryDefaults, QueryFamily, QueryOptions, TestQueryRuntime,
			provide_query_client,
		};

		// Arrange
		clear_global_auth_state();
		let runtime = TestQueryRuntime::new();
		let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
		let family = QueryFamily::<(), String, String>::new("auth.request-context");
		let query = client.observe(
			family.query((), || async { Ok("authenticated".to_owned()) }),
			QueryOptions::default(),
		);
		runtime.run_until_stalled();
		let _client_guard = provide_query_client(client);
		let state = auth_state();
		if authenticated {
			state.login("1", "authenticated");
		}
		let context = server_fn_authentication_context();

		// Act
		observe_server_fn_status_for_request(401, context);

		// Assert
		assert_eq!(query.data().is_none(), expect_invalidation);
		assert_eq!(
			state.is_authenticated(),
			authenticated && !expect_invalidation
		);
		clear_global_auth_state();
	}

	#[cfg(native)]
	#[test]
	fn authentication_invalidation_resets_coalescing_when_native_scheduler_is_unavailable() {
		reinhardt_core::reactive::ReactiveScope::run(|| {
			let router = reinhardt_urls::routers::ClientRouter::new()
				.route("root", "/", || crate::Page::text("root"));
			crate::app::__install_client_router_for_test(router);
			AUTHENTICATION_INVALIDATION_SCHEDULED.with(|scheduled| scheduled.set(None));

			invalidate_authentication();

			assert_eq!(AUTHENTICATION_INVALIDATION_SCHEDULED.with(Cell::get), None);
			crate::app::__clear_spa_router_for_test();
		});
	}

	#[test]
	fn test_get_jwt_token_non_wasm() {
		// On non-WASM targets, get_jwt_token always returns None
		assert!(get_jwt_token().is_none());
	}

	#[test]
	fn test_set_jwt_token_non_wasm() {
		// On non-WASM targets, set_jwt_token is a no-op (should not panic)
		set_jwt_token("test-token");
	}

	#[test]
	fn test_clear_jwt_token_non_wasm() {
		// On non-WASM targets, clear_jwt_token is a no-op (should not panic)
		clear_jwt_token();
	}

	#[test]
	fn test_jwt_constants() {
		assert_eq!(AUTH_HEADER_NAME, "Authorization");
		assert_eq!(JWT_STORAGE_KEY, "__admin_jwt");
	}
}
