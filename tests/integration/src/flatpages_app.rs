//! Flatpages application builder for integration tests

use async_trait::async_trait;
use reinhardt_apps::{Handler, Request, Response, Result};
use reinhardt_auth::session::InMemorySessionStore;
use reinhardt_auth::{SimpleUser, User};
use reinhardt_middleware::AuthenticationMiddleware;
use reinhardt_middleware::csrf::{CsrfMiddleware, CsrfMiddlewareConfig};
use reinhardt_core::security::csrf::{CsrfMeta, check_token, get_secret};
use reinhardt_urls::routers::{DefaultRouter, Router, path};
use sqlx::{Pool, Postgres};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
	pub pool: Pool<Postgres>,
	pub csrf_meta: Option<Arc<Mutex<CsrfMeta>>>,
	pub authenticated: bool, // Simple auth flag for testing
}

/// CSRF-protected app with accessible state for testing
pub struct CsrfAppWithState {
	pub router: Arc<dyn Handler>,
	pub state: AppState,
}

/// Flatpage handler
struct FlatpageHandler {
	pool: Pool<Postgres>,
	authenticated: bool,
}

#[async_trait]
impl Handler for FlatpageHandler {
	async fn handle(&self, request: Request) -> Result<Response> {
		use sqlx::Row;

		let path = request
			.path_params
			.get("path")
			.map(|p| format!("/{}", p))
			.unwrap_or_else(|| "/".into());

		// Query flatpage from database
		let result = sqlx::query(
			"SELECT id, url, title, content, enable_comments, template_name, registration_required \
             FROM flatpages WHERE url = $1",
		)
		.bind(&path)
		.fetch_optional(&self.pool)
		.await;

		match result {
			Ok(Some(row)) => {
				let title: String = row.get("title");
				let content: String = row.get("content");
				let registration_required: bool = row.get("registration_required");

				// Check if registration is required
				if registration_required && !self.authenticated {
					return Ok(
						Response::new(hyper::StatusCode::FOUND).with_body("Redirect to login")
					);
				}

				let html = format!(
					r#"<!DOCTYPE html>
<html>
<head><title>{}</title></head>
<body><p>{}</p></body>
</html>"#,
					title, content
				);
				Ok(Response::ok().with_body(html))
			}
			Ok(None) => Ok(Response::not_found().with_body("Not Found")),
			Err(_) => Ok(Response::not_found().with_body("Not Found")),
		}
	}
}

/// Fallback flatpage handler (middleware-style)
struct FallbackFlatpageHandler {
	pool: Pool<Postgres>,
	authenticated: bool,
}

#[async_trait]
impl Handler for FallbackFlatpageHandler {
	async fn handle(&self, request: Request) -> Result<Response> {
		use sqlx::Row;

		let path = request
			.path_params
			.get("path")
			.map(|p| format!("/{}", p))
			.unwrap_or_else(|| "/".into());

		// Query flatpage from database (fallback middleware behavior)
		let result = sqlx::query(
			"SELECT id, url, title, content, enable_comments, template_name, registration_required \
             FROM flatpages WHERE url = $1",
		)
		.bind(&path)
		.fetch_optional(&self.pool)
		.await;

		match result {
			Ok(Some(row)) => {
				let title: String = row.get("title");
				let content: String = row.get("content");
				let registration_required: bool = row.get("registration_required");

				if registration_required && !self.authenticated {
					return Ok(
						Response::new(hyper::StatusCode::FOUND).with_body("Redirect to login")
					);
				}

				let html = format!(
					r#"<!DOCTYPE html>
<html>
<head><title>{}</title></head>
<body><p>{}</p></body>
</html>"#,
					title, content
				);
				Ok(Response::ok().with_body(html))
			}
			Ok(None) => Ok(Response::not_found().with_body("Not Found")),
			Err(_) => Ok(Response::not_found().with_body("Not Found")),
		}
	}
}

/// CSRF-protected flatpage handler
struct CsrfFlatpageHandler {
	pool: Pool<Postgres>,
	csrf_meta: Arc<Mutex<CsrfMeta>>,
	authenticated: bool,
}

#[async_trait]
impl Handler for CsrfFlatpageHandler {
	async fn handle(&self, request: Request) -> Result<Response> {
		use sqlx::Row;

		// For POST requests, check CSRF token
		if request.method == hyper::Method::POST {
			if let Some(token) = request
				.headers
				.get("X-CSRFToken")
				.and_then(|v| v.to_str().ok())
			{
				let meta = self.csrf_meta.lock().unwrap();
				let secret = &meta.token;

				if check_token(token, secret).is_err() {
					return Ok(Response::forbidden().with_body("CSRF validation failed"));
				}
			} else {
				return Ok(Response::forbidden().with_body("CSRF token missing"));
			}
		}

		// Handle the flatpage
		let path = request
			.path_params
			.get("path")
			.map(|p| format!("/{}", p))
			.unwrap_or_else(|| "/".into());

		// Query flatpage from database
		let result = sqlx::query(
			"SELECT id, url, title, content, enable_comments, template_name, registration_required \
             FROM flatpages WHERE url = $1",
		)
		.bind(&path)
		.fetch_optional(&self.pool)
		.await;

		match result {
			Ok(Some(row)) => {
				let title: String = row.get("title");
				let content: String = row.get("content");
				let registration_required: bool = row.get("registration_required");

				if registration_required && !self.authenticated {
					return Ok(
						Response::new(hyper::StatusCode::FOUND).with_body("Redirect to login")
					);
				}

				let html = format!(
					r#"<!DOCTYPE html>
<html>
<head><title>{}</title></head>
<body><p>{}</p></body>
</html>"#,
					title, content
				);
				Ok(Response::ok().with_body(html))
			}
			Ok(None) => Ok(Response::not_found().with_body("Not Found")),
			Err(_) => Ok(Response::not_found().with_body("Not Found")),
		}
	}
}

/// Build a basic flatpages application
pub fn build_flatpages_app(pool: Pool<Postgres>) -> Arc<dyn Handler> {
	let flatpage_handler = Arc::new(FlatpageHandler {
		pool: pool.clone(),
		authenticated: false,
	});

	let fallback_handler = Arc::new(FallbackFlatpageHandler {
		pool: pool.clone(),
		authenticated: false,
	});

	let mut router = DefaultRouter::new();
	router.add_route(path("/flatpage_root/{path:.*}", flatpage_handler));
	router.add_route(path("/{path:.*}", fallback_handler));

	Arc::new(router)
}

/// Build flatpages app with CSRF protection
pub fn build_flatpages_app_with_csrf(pool: Pool<Postgres>) -> CsrfAppWithState {
	let csrf_meta = Arc::new(Mutex::new(CsrfMeta {
		token: get_secret(),
	}));
	let state = AppState {
		pool: pool.clone(),
		csrf_meta: Some(csrf_meta.clone()),
		authenticated: false,
	};

	let flatpage_handler = Arc::new(CsrfFlatpageHandler {
		pool: pool.clone(),
		csrf_meta: csrf_meta.clone(),
		authenticated: false,
	});

	let fallback_handler = Arc::new(CsrfFlatpageHandler {
		pool: pool.clone(),
		csrf_meta: csrf_meta.clone(),
		authenticated: false,
	});

	let mut router = DefaultRouter::new();
	router.add_route(path("/flatpage_root/{path:.*}", flatpage_handler.clone()));
	router.add_route(path("/{path:.*}", fallback_handler));

	let router = Arc::new(router) as Arc<dyn Handler>;

	CsrfAppWithState { router, state }
}

/// Get CSRF token for testing
pub fn get_csrf_token_for_testing(state: &AppState) -> Option<String> {
	state.csrf_meta.as_ref().map(|csrf_meta| {
		let meta = csrf_meta.lock().unwrap();
		meta.token.clone()
	})
}

/// Authentication backend for integration tests
struct TestAuthBackend {
	authenticated: bool,
	test_user: Option<SimpleUser>,
}

#[async_trait]
impl reinhardt_auth::AuthenticationBackend for TestAuthBackend {
	async fn authenticate(
		&self,
		_request: &Request,
	) -> std::result::Result<Option<Box<dyn User>>, reinhardt_auth::AuthenticationError> {
		if self.authenticated {
			if let Some(user) = &self.test_user {
				Ok(Some(Box::new(user.clone())))
			} else {
				Ok(Some(Box::new(SimpleUser {
					id: Uuid::now_v7(),
					username: "testuser".to_string(),
					email: "test@example.com".to_string(),
					is_active: true,
					is_admin: false,
				})))
			}
		} else {
			Ok(None)
		}
	}

	async fn get_user(
		&self,
		_user_id: &str,
	) -> std::result::Result<Option<Box<dyn User>>, reinhardt_auth::AuthenticationError> {
		if self.authenticated {
			if let Some(user) = &self.test_user {
				Ok(Some(Box::new(user.clone())))
			} else {
				Ok(Some(Box::new(SimpleUser {
					id: Uuid::now_v7(),
					username: "testuser".to_string(),
					email: "test@example.com".to_string(),
					is_active: true,
					is_admin: false,
				})))
			}
		} else {
			Ok(None)
		}
	}
}

/// Build flatpages app with authentication middleware (authenticated user)
pub fn build_flatpages_app_with_auth(pool: Pool<Postgres>) -> Arc<dyn Handler> {
	let session_store = Arc::new(InMemorySessionStore::new());
	let auth_backend = Arc::new(TestAuthBackend {
		authenticated: true,
		test_user: None,
	});

	let flatpage_handler = Arc::new(FlatpageHandler {
		pool: pool.clone(),
		authenticated: true,
	});

	let fallback_handler = Arc::new(FallbackFlatpageHandler {
		pool: pool.clone(),
		authenticated: true,
	});

	let mut router = DefaultRouter::new();
	router.add_route(path("/flatpage_root/{path:.*}", flatpage_handler));
	router.add_route(path("/{path:.*}", fallback_handler));

	let auth_middleware = AuthenticationMiddleware::new(session_store, auth_backend);
	let chain = reinhardt_http::MiddlewareChain::new(Arc::new(router))
		.with_middleware(Arc::new(auth_middleware));

	Arc::new(chain)
}

/// Build flatpages app without authentication (anonymous user)
pub fn build_flatpages_app_without_auth(pool: Pool<Postgres>) -> Arc<dyn Handler> {
	let session_store = Arc::new(InMemorySessionStore::new());
	let auth_backend = Arc::new(TestAuthBackend {
		authenticated: false,
		test_user: None,
	});

	let flatpage_handler = Arc::new(FlatpageHandler {
		pool: pool.clone(),
		authenticated: false,
	});

	let fallback_handler = Arc::new(FallbackFlatpageHandler {
		pool: pool.clone(),
		authenticated: false,
	});

	let mut router = DefaultRouter::new();
	router.add_route(path("/flatpage_root/{path:.*}", flatpage_handler));
	router.add_route(path("/{path:.*}", fallback_handler));

	let auth_middleware = AuthenticationMiddleware::new(session_store, auth_backend);
	let chain = reinhardt_http::MiddlewareChain::new(Arc::new(router))
		.with_middleware(Arc::new(auth_middleware));

	Arc::new(chain)
}
