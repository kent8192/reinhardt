//! `SessionMiddleware`: cookie parsing, store wiring, and `Set-Cookie` writeback.

use async_trait::async_trait;
use reinhardt_http::{
	AuthState, Handler, IsActive, IsAdmin, IsAuthenticated, Middleware, MiddlewareDiRegistration,
	Request, Response, Result,
};
use std::any::TypeId;
use std::sync::Arc;

use super::config::SessionConfig;
use super::cookie::find_cookie_value;
use super::data::{SessionData, USER_ID_SESSION_KEY};
use super::id::{ActiveSessionId, SessionCookieName, SessionId};
use super::store::{SessionStore, SessionStoreKey};

/// Session middleware
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use std::time::Duration;
/// use reinhardt_middleware::session::{SessionMiddleware, SessionConfig};
/// use reinhardt_http::{Handler, Middleware, Request, Response};
/// use hyper::{StatusCode, Method, Version, HeaderMap};
/// use bytes::Bytes;
///
/// struct TestHandler;
///
/// #[async_trait::async_trait]
/// impl Handler for TestHandler {
///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
///         Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
///     }
/// }
///
/// # tokio_test::block_on(async {
/// let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600));
/// let middleware = SessionMiddleware::new(config);
/// let handler = Arc::new(TestHandler);
///
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/api/data")
///     .version(Version::HTTP_11)
///     .headers(HeaderMap::new())
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// let response = middleware.process(request, handler).await.unwrap();
/// assert_eq!(response.status, StatusCode::OK);
/// # });
/// ```
pub struct SessionMiddleware {
	config: SessionConfig,
	store: Arc<SessionStore>,
}

impl SessionMiddleware {
	/// Create a new session middleware
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::session::{SessionMiddleware, SessionConfig};
	///
	/// let config = SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600));
	/// let middleware = SessionMiddleware::new(config);
	/// ```
	pub fn new(config: SessionConfig) -> Self {
		Self {
			config,
			store: Arc::new(SessionStore::new()),
		}
	}

	/// Create with default configuration
	pub fn with_defaults() -> Self {
		Self::new(SessionConfig::default())
	}

	/// Create from an existing Arc-wrapped session store
	///
	/// This is provided for cases where you already have an `Arc<SessionStore>`.
	/// In most cases, you should use `new()` instead, which creates the store internally.
	pub fn from_arc(config: SessionConfig, store: Arc<SessionStore>) -> Self {
		Self { config, store }
	}

	/// Get a reference to the session store
	///
	/// # Examples
	///
	/// ```
	/// use std::time::Duration;
	/// use reinhardt_middleware::session::{SessionMiddleware, SessionConfig};
	///
	/// let middleware = SessionMiddleware::new(
	///     SessionConfig::new("sessionid".to_string(), Duration::from_secs(3600))
	/// );
	///
	/// // Access the store
	/// let store = middleware.store();
	/// assert_eq!(store.len(), 0);
	/// ```
	pub fn store(&self) -> &SessionStore {
		&self.store
	}

	/// Get a cloned Arc of the store (for cases where you need ownership)
	///
	/// In most cases, you should use `store()` instead to get a reference.
	pub fn store_arc(&self) -> Arc<SessionStore> {
		Arc::clone(&self.store)
	}

	/// Get session ID from request.
	///
	/// Delegates to the shared `find_cookie_value` helper so this stays in
	/// lock-step with the `Injectable` DI path that also parses session cookies.
	fn get_session_id(&self, request: &Request) -> Option<String> {
		find_cookie_value(request, &self.config.cookie_name)
	}

	/// Build Set-Cookie header
	fn build_cookie_header(&self, session_id: &str) -> String {
		let mut parts = vec![format!("{}={}", self.config.cookie_name, session_id)];

		parts.push(format!("Path={}", self.config.path));

		if let Some(domain) = &self.config.domain {
			parts.push(format!("Domain={}", domain));
		}

		if self.config.http_only {
			parts.push("HttpOnly".to_string());
		}

		if self.config.secure {
			parts.push("Secure".to_string());
		}

		if let Some(same_site) = &self.config.same_site {
			parts.push(format!("SameSite={}", same_site));
		}

		parts.push(format!("Max-Age={}", self.config.ttl.as_secs()));

		parts.join("; ")
	}

	fn user_id_from_session(session: &SessionData) -> Option<String> {
		let value = session.data.get(USER_ID_SESSION_KEY)?;
		let user_id = match value {
			serde_json::Value::String(s) => s.clone(),
			serde_json::Value::Number(n) => n.to_string(),
			serde_json::Value::Bool(b) => b.to_string(),
			_ => return None,
		};

		if user_id.is_empty() {
			None
		} else {
			Some(user_id)
		}
	}

	fn populate_auth_extensions(request: &Request, session: &SessionData) {
		if request.extensions.contains::<AuthState>() {
			return;
		}

		let Some(user_id) = Self::user_id_from_session(session) else {
			request.extensions.insert(IsAuthenticated(false));
			request.extensions.insert(IsAdmin(false));
			request.extensions.insert(IsActive(false));
			request.extensions.insert(AuthState::anonymous());
			return;
		};

		let is_staff = session.get::<bool>("is_staff").unwrap_or(false);
		let is_superuser = session.get::<bool>("is_superuser").unwrap_or(false);
		let is_admin = is_staff || is_superuser;
		let is_active = true;

		request.extensions.insert(user_id.clone());
		request.extensions.insert(IsAuthenticated(true));
		request.extensions.insert(IsAdmin(is_admin));
		request.extensions.insert(IsActive(is_active));
		request
			.extensions
			.insert(AuthState::authenticated(user_id, is_admin, is_active));
	}
}

impl Default for SessionMiddleware {
	fn default() -> Self {
		Self::with_defaults()
	}
}

#[async_trait]
impl Middleware for SessionMiddleware {
	/// Exposes the middleware-owned `Arc<SessionStore>` as DI singletons.
	///
	/// The raw `SessionStore` key is retained for `SessionData::inject`.
	/// The keyed `FactoryOutput<SessionStoreKey, Arc<SessionStore>>` key is
	/// used by handlers that request
	/// `#[inject] store: Depends<SessionStoreKey, Arc<SessionStore>>`.
	fn di_registrations(&self) -> Vec<MiddlewareDiRegistration> {
		let store = Arc::clone(&self.store);
		let keyed_store = Arc::new(reinhardt_di::FactoryOutput::<
			SessionStoreKey,
			Arc<SessionStore>,
		>::new(Arc::clone(&self.store)));
		vec![
			(
				TypeId::of::<SessionStore>(),
				store as Arc<dyn std::any::Any + Send + Sync>,
			),
			(
				TypeId::of::<reinhardt_di::FactoryOutput<SessionStoreKey, Arc<SessionStore>>>(),
				keyed_store as Arc<dyn std::any::Any + Send + Sync>,
			),
		]
	}

	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		// Get or generate session ID
		let session_id = self.get_session_id(&request);
		let mut session = if let Some(id) = session_id.clone() {
			self.store
				.get(&id)
				.filter(|s| s.is_valid())
				.unwrap_or_else(|| SessionData::new(self.config.ttl))
		} else {
			SessionData::new(self.config.ttl)
		};

		// Touch the session
		session.touch(self.config.ttl);

		// Save the session
		self.store.save(session.clone());

		// Inject session ID and cookie name into request extensions
		// so downstream handlers and Injectable impls can access them
		request
			.extensions
			.insert(SessionId::new(session.id.clone()));
		request
			.extensions
			.insert(SessionCookieName::new(self.config.cookie_name.clone()));
		// Shared, mutable holder so handlers that rotate the session ID
		// (`SessionData::regenerate_id`) keep `Set-Cookie` in sync. See #3827.
		let active_id = ActiveSessionId::new(session.id.clone());
		request.extensions.insert(active_id.clone());
		Self::populate_auth_extensions(&request, &session);

		// Call the handler
		// Convert errors to responses so post-processing (e.g., security headers)
		// always runs, even when invoked outside MiddlewareChain. (#3244)
		let mut response = match handler.handle(request).await {
			Ok(resp) => resp,
			Err(e) => Response::from(e),
		};

		// Append Set-Cookie header (use append to preserve existing Set-Cookie headers).
		// Read the final session ID from the shared holder rather than the
		// local `session` clone, since handlers may have rotated the ID via
		// `SessionData::regenerate_id`. See #3827.
		let final_id = active_id.get();
		let cookie = self.build_cookie_header(&final_id);
		response.headers.append(
			hyper::header::SET_COOKIE,
			hyper::header::HeaderValue::from_str(&cookie).map_err(|e| {
				reinhardt_core::exception::Error::Internal(format!(
					"Failed to create cookie header: {}",
					e
				))
			})?,
		);

		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, Version};
	use reinhardt_http::{AuthState, Handler, Request};
	use rstest::rstest;
	use std::sync::Mutex;
	use std::time::Duration;

	#[derive(Default)]
	struct CapturedAuth {
		auth_state: Option<AuthState>,
		user_id: Option<String>,
		is_authenticated: Option<IsAuthenticated>,
		is_admin: Option<IsAdmin>,
		is_active: Option<IsActive>,
	}

	struct CaptureAuthHandler {
		captured: Arc<Mutex<CapturedAuth>>,
	}

	#[async_trait]
	impl Handler for CaptureAuthHandler {
		async fn handle(&self, request: Request) -> Result<Response> {
			*self.captured.lock().unwrap() = CapturedAuth {
				auth_state: request.extensions.get::<AuthState>(),
				user_id: request.extensions.get::<String>(),
				is_authenticated: request.extensions.get::<IsAuthenticated>(),
				is_admin: request.extensions.get::<IsAdmin>(),
				is_active: request.extensions.get::<IsActive>(),
			};
			Ok(Response::ok())
		}
	}

	/// Returns a `SessionMiddleware` with a fixed cookie name and TTL for
	/// deterministic tests.
	fn make_middleware() -> SessionMiddleware {
		SessionMiddleware::new(SessionConfig::new(
			"sessionid".to_string(),
			Duration::from_secs(3600),
		))
	}

	fn request_with_cookie(session_id: &str) -> Request {
		let mut headers = HeaderMap::new();
		headers.insert(
			hyper::header::COOKIE,
			hyper::header::HeaderValue::from_str(&format!("sessionid={}", session_id)).unwrap(),
		);
		Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	fn request_without_cookie() -> Request {
		Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	fn capture_handler() -> (Arc<Mutex<CapturedAuth>>, Arc<dyn Handler>) {
		let captured = Arc::new(Mutex::new(CapturedAuth::default()));
		let handler = Arc::new(CaptureAuthHandler {
			captured: Arc::clone(&captured),
		});
		(captured, handler)
	}

	#[rstest]
	fn test_session_middleware_di_registrations_returns_store() {
		// Arrange: build a middleware (which internally creates an Arc<SessionStore>).
		let middleware = make_middleware();
		let store_arc = middleware.store_arc();

		// Act: ask the middleware for its DI registrations.
		let registrations = middleware.di_registrations();

		assert_eq!(registrations.len(), 2);
		let (type_id, value) = &registrations[0];
		assert_eq!(*type_id, TypeId::of::<SessionStore>());
		let downcast = value
			.clone()
			.downcast::<SessionStore>()
			.expect("registered Arc must downcast to SessionStore");
		assert!(
			Arc::ptr_eq(&downcast, &store_arc),
			"middleware DI registration must expose the same Arc<SessionStore> the middleware writes to"
		);

		let (type_id, value) = &registrations[1];
		assert_eq!(
			*type_id,
			TypeId::of::<reinhardt_di::FactoryOutput<SessionStoreKey, Arc<SessionStore>>>()
		);
		let downcast = value
			.clone()
			.downcast::<reinhardt_di::FactoryOutput<SessionStoreKey, Arc<SessionStore>>>()
			.expect("registered Arc must downcast to keyed SessionStore factory output");
		assert!(
			Arc::ptr_eq(downcast.as_ref(), &store_arc),
			"keyed DI registration must expose the same Arc<SessionStore> the middleware writes to"
		);
	}

	#[rstest]
	fn test_session_middleware_di_registrations_apply_to_singleton_scope() {
		// Arrange: middleware + an empty SingletonScope.
		let middleware = make_middleware();
		let store_arc = middleware.store_arc();
		let scope = reinhardt_di::SingletonScope::new();

		// Act: feed the middleware's registrations into a DiRegistrationList and
		// apply it to the scope (mirroring what `with_middleware` does internally).
		let mut list = reinhardt_di::DiRegistrationList::new();
		for (type_id, value) in middleware.di_registrations() {
			list.register_arc_any(type_id, value);
		}
		list.apply_to(&scope);

		// Assert: the scope now resolves `SessionStore` (keyed by its own
		// TypeId after the #4437 migration) to the same Arc the middleware
		// uses, mirroring what `SessionData::inject` does via
		// `get_singleton::<SessionStore>()`, so handlers see the same store.
		let resolved = scope
			.get::<SessionStore>()
			.expect("SingletonScope must resolve SessionStore after applying middleware DI");
		assert!(
			Arc::ptr_eq(&resolved, &store_arc),
			"resolved Arc<SessionStore> must point at the same allocation the middleware owns"
		);
	}

	#[tokio::test]
	async fn session_middleware_populates_auth_state_from_session_user_id() {
		let middleware = make_middleware();
		let store = middleware.store_arc();
		let mut session = SessionData::new(Duration::from_secs(3600));
		session
			.set(USER_ID_SESSION_KEY.to_string(), 42_i64)
			.unwrap();
		session.set("is_staff".to_string(), true).unwrap();
		let session_id = session.id.clone();
		store.save(session);

		let (captured, handler) = capture_handler();
		middleware
			.process(request_with_cookie(&session_id), handler)
			.await
			.unwrap();

		let captured = captured.lock().unwrap();
		let auth_state = captured
			.auth_state
			.as_ref()
			.expect("SessionMiddleware must insert AuthState");
		assert!(auth_state.is_authenticated());
		assert_eq!(auth_state.user_id(), "42");
		assert!(auth_state.is_admin());
		assert!(auth_state.is_active());
		assert_eq!(captured.user_id.as_deref(), Some("42"));
		assert_eq!(captured.is_authenticated, Some(IsAuthenticated(true)));
		assert_eq!(captured.is_admin, Some(IsAdmin(true)));
		assert_eq!(captured.is_active, Some(IsActive(true)));
	}

	#[tokio::test]
	async fn session_middleware_populates_anonymous_auth_state_without_user_id() {
		let middleware = make_middleware();
		let (captured, handler) = capture_handler();

		middleware
			.process(request_without_cookie(), handler)
			.await
			.unwrap();

		let captured = captured.lock().unwrap();
		let auth_state = captured
			.auth_state
			.as_ref()
			.expect("SessionMiddleware must insert anonymous AuthState");
		assert!(auth_state.is_anonymous());
		assert!(captured.user_id.is_none());
		assert_eq!(captured.is_authenticated, Some(IsAuthenticated(false)));
		assert_eq!(captured.is_admin, Some(IsAdmin(false)));
		assert_eq!(captured.is_active, Some(IsActive(false)));
	}

	#[tokio::test]
	async fn session_middleware_preserves_existing_auth_state() {
		let middleware = make_middleware();
		let request = request_without_cookie();
		request
			.extensions
			.insert(AuthState::authenticated("jwt-user", true, true));
		let (captured, handler) = capture_handler();

		middleware.process(request, handler).await.unwrap();

		let captured = captured.lock().unwrap();
		let auth_state = captured
			.auth_state
			.as_ref()
			.expect("pre-existing AuthState must remain visible");
		assert_eq!(auth_state.user_id(), "jwt-user");
		assert!(auth_state.is_authenticated());
		assert!(auth_state.is_admin());
		assert!(auth_state.is_active());
		assert!(captured.is_authenticated.is_none());
		assert!(captured.is_admin.is_none());
		assert!(captured.is_active.is_none());
	}

	/// End-to-end injection test: drives the same path a handler with
	/// `#[inject] session: SessionData` would take. Catches `TypeId` /
	/// shape regressions in `di_registrations` that `SingletonScope::get`-only
	/// tests would miss, by going through `InjectionContext` and the real
	/// `Injectable for SessionData` implementation. See PR #4435 Copilot review.
	#[tokio::test]
	async fn test_session_data_inject_resolves_via_middleware_di_registrations() {
		use crate::session::data::SessionData;
		use bytes::Bytes;
		use hyper::{Method, Version};
		use reinhardt_di::{Injectable, InjectionContext, SingletonScope};
		use reinhardt_http::Request;

		// Arrange: middleware contributes its Arc<SessionStore> via DI; pre-seed
		// the store with a valid session so the inject path can load it.
		let middleware = make_middleware();
		let store_arc = middleware.store_arc();
		let mut seeded = SessionData::new(Duration::from_secs(3600));
		seeded
			.set("user_id".to_string(), "alice".to_string())
			.unwrap();
		let seeded_id = seeded.id.clone();
		store_arc.save(seeded.clone());

		let scope = SingletonScope::new();
		let mut list = reinhardt_di::DiRegistrationList::new();
		for (type_id, value) in middleware.di_registrations() {
			list.register_arc_any(type_id, value);
		}
		list.apply_to(&scope);

		// Build a request that carries the SessionId extension the middleware
		// would normally inject during `process`. This bypasses Cookie parsing
		// but exercises the exact branch `SessionData::inject` takes when
		// `SessionMiddleware` is upstream.
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.body(Bytes::new())
			.build()
			.unwrap();
		request.extensions.insert(SessionId::new(seeded_id.clone()));

		// `SessionData::inject` reads the request from the per-request scope
		// via `ctx.get_request::<Request>()`, so the request must be stored
		// with `set_request` (request scope) rather than the builder's
		// `with_request` (which populates the HTTP-request slot accessed by
		// `get_http_request`).
		let ctx = InjectionContext::builder(Arc::new(scope)).build();
		ctx.set_request(request);

		// Act: resolve `SessionData` through the real `#[inject]` code path.
		let resolved = SessionData::inject(&ctx)
			.await
			.expect("SessionData::inject must succeed when middleware DI is registered");

		// Assert: the resolved session is the one the store holds.
		assert_eq!(resolved.id, seeded_id);
		assert_eq!(resolved.get::<String>("user_id").as_deref(), Some("alice"));
	}
}
