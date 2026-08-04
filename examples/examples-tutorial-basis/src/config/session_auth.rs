//! Account-validating session authentication for the tutorial server.

use crate::apps::users::models::User;
use reinhardt::core::async_trait;
use reinhardt::di::InjectionContext;
use reinhardt::http::{AuthState, IsActive, IsAdmin, IsAuthenticated};
use reinhardt::middleware::session::{SessionData, USER_ID_SESSION_KEY};
use reinhardt::{BaseUser, DatabaseConnection, Handler, Middleware, Model, Request, Response};
use std::sync::Arc;

/// Resolves the session identity against the current tutorial user record.
///
/// `SessionMiddleware` owns cookie/session storage. This middleware runs after
/// it, loads the referenced user through the request DI connection, and only
/// then publishes authentication and authorization state.
#[derive(Debug, Default)]
pub struct TutorialSessionAuthMiddleware;

impl TutorialSessionAuthMiddleware {
	/// Create account-validating session authentication middleware.
	pub fn new() -> Self {
		Self
	}

	async fn validated_auth_state(&self, request: &Request) -> AuthState {
		let Some(session) = request.extensions.get::<SessionData>() else {
			return AuthState::anonymous();
		};
		let Some(user_id) = session.get::<i64>(USER_ID_SESSION_KEY) else {
			return AuthState::anonymous();
		};
		let Some(context) = request.get_di_context::<InjectionContext>() else {
			tracing::warn!("Tutorial session authentication has no DI context");
			return AuthState::anonymous();
		};
		let Some(db) = context
			.get_singleton::<DatabaseConnection>()
			.or_else(|| context.get_request::<DatabaseConnection>())
		else {
			tracing::warn!("Tutorial session authentication has no database connection");
			return AuthState::anonymous();
		};

		match User::objects().get(user_id).first_with_db(&db).await {
			Ok(Some(user)) if user.is_active() => AuthState::authenticated(
				user.id().to_string(),
				user.is_superuser,
				true,
			),
			Ok(Some(_)) | Ok(None) => AuthState::anonymous(),
			Err(error) => {
				tracing::warn!(?error, "Tutorial session account validation failed");
				AuthState::anonymous()
			}
		}
	}
}

#[async_trait]
impl Middleware for TutorialSessionAuthMiddleware {
	async fn process(
		&self,
		request: Request,
		next: Arc<dyn Handler>,
	) -> reinhardt::Result<Response> {
		let auth_state = self.validated_auth_state(&request).await;
		request
			.extensions
			.insert(IsAuthenticated(auth_state.is_authenticated()));
		request.extensions.insert(IsAdmin(auth_state.is_admin()));
		request.extensions.insert(IsActive(auth_state.is_active()));
		request.extensions.insert(auth_state);
		next.handle(request).await
	}
}

#[cfg(test)]
mod tests {
	use super::TutorialSessionAuthMiddleware;
	use reinhardt::core::async_trait;
	use reinhardt::di::{InjectionContext, SingletonScope};
	use reinhardt::http::AuthState;
	use reinhardt::middleware::session::{SessionData, USER_ID_SESSION_KEY};
	use reinhardt::{DatabaseConnection, Handler, Middleware, Request, Response};
	use sqlx::SqlitePool;
	use std::sync::{Arc, Mutex};
	use std::time::Duration;
	use tempfile::NamedTempFile;

	struct CaptureAuthState(Arc<Mutex<Option<AuthState>>>);

	#[async_trait]
	impl Handler for CaptureAuthState {
		async fn handle(&self, request: Request) -> reinhardt::Result<Response> {
			*self.0.lock().expect("capture lock should remain available") =
				request.extensions.get::<AuthState>();
			Ok(Response::ok())
		}
	}

	async fn request_for_user(user_id: i64, is_active: bool) -> (NamedTempFile, Request) {
		let database_file = NamedTempFile::new().expect("temporary database should be created");
		let database_path = database_file
			.path()
			.to_str()
			.expect("temporary database path should be UTF-8");
		let sqlx_url = format!("sqlite://{database_path}?mode=rwc");
		let orm_url = format!("sqlite:///{database_path}");
		let pool = SqlitePool::connect(&sqlx_url)
			.await
			.expect("SQLite pool should connect");
		sqlx::query(
			"CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT NOT NULL, password_hash TEXT, is_active BOOLEAN NOT NULL, is_superuser BOOLEAN NOT NULL, last_login TEXT, created_at TEXT NOT NULL)",
		)
		.execute(&pool)
		.await
		.expect("users table should be created");
		sqlx::query(
			"INSERT INTO users (id, username, password_hash, is_active, is_superuser, last_login, created_at) VALUES (?, ?, NULL, ?, 0, NULL, '2026-08-04T00:00:00Z')",
		)
		.bind(user_id)
		.bind("tutorial-user")
		.bind(is_active)
		.execute(&pool)
		.await
		.expect("tutorial user should be inserted");

		let db = DatabaseConnection::connect_sqlite(&orm_url)
			.await
			.expect("ORM connection should connect");
		let singleton = Arc::new(SingletonScope::new());
		singleton.set(db);
		let context = InjectionContext::builder(singleton).build();

		let mut session = SessionData::new(Duration::from_secs(3600));
		session
			.set(USER_ID_SESSION_KEY.to_string(), user_id)
			.expect("session user ID should serialize");
		let request = Request::builder()
			.uri("/")
			.body(Vec::new().into())
			.build()
			.unwrap();
		request.extensions.insert(session);
		request.extensions.insert(Arc::new(context));

		(database_file, request)
	}

	#[tokio::test]
	async fn active_session_user_populates_validated_auth_state() {
		let (_database_file, request) = request_for_user(7, true).await;
		let captured = Arc::new(Mutex::new(None));
		let handler = Arc::new(CaptureAuthState(Arc::clone(&captured)));

		TutorialSessionAuthMiddleware::new()
			.process(request, handler)
			.await
			.expect("authentication middleware should continue");

		let auth_state = captured
			.lock()
			.expect("capture lock should remain available")
			.clone()
			.expect("active account should produce AuthState");
		assert!(auth_state.is_authenticated());
		assert!(auth_state.is_active());
		assert_eq!(auth_state.user_id(), "7");
	}

	#[tokio::test]
	async fn inactive_session_user_is_anonymous() {
		let (_database_file, request) = request_for_user(8, false).await;
		let captured = Arc::new(Mutex::new(None));
		let handler = Arc::new(CaptureAuthState(Arc::clone(&captured)));

		TutorialSessionAuthMiddleware::new()
			.process(request, handler)
			.await
			.expect("authentication middleware should fail closed and continue");

		let auth_state = captured
			.lock()
			.expect("capture lock should remain available")
			.clone()
			.expect("middleware should always populate AuthState");
		assert!(auth_state.is_anonymous());
	}
}
