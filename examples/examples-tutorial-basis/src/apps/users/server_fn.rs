//! User authentication server functions
//!
//! Provides session-cookie-based login/logout and current-user lookup.
//! Follows the session-auth pattern: `SessionData` + keyed session-store
//! dependency are injected, the session ID is regenerated on successful login
//! (fixation prevention), and `user_id` is persisted in the session map.

use crate::apps::users::models::UserInfo;
#[cfg(server)]
use crate::shared::types::{LoginRequest, RegisterRequest};
use reinhardt::pages::server_fn::{ServerFnError, server_fn};

#[cfg(server)]
use {
	crate::apps::users::models::{AuthUserManager, AuthUserManagerKey, User},
	reinhardt::BaseUser,
	reinhardt::DatabaseConnection,
	reinhardt::Validate,
	reinhardt::db::orm::Model,
	reinhardt::di::Depends,
	reinhardt::middleware::session::{
		SessionAuthExt, SessionData, SessionStore, SessionStoreKey, USER_ID_SESSION_KEY,
	},
	reinhardt::reinhardt_auth::BaseUserManager,
	std::collections::HashMap,
	std::result::Result,
	std::sync::Arc,
};

/// Authenticate a user by username/password and persist the session.
///
/// CSRF is supplied by the `#[server_fn]` client stub through `X-CSRFToken`
/// and verified by middleware before this handler runs.
#[server_fn]
pub async fn login(
	username: String,
	password: String,
	#[inject] _db: DatabaseConnection,
	#[inject] session: SessionData,
	#[inject] store: Depends<SessionStoreKey, Arc<SessionStore>>,
) -> Result<UserInfo, ServerFnError> {
	let mut session = session;

	let request = LoginRequest { username, password };

	// Run the field-level validators declared on `LoginRequest` before
	// touching the database — empty/oversized credentials should reject
	// at the request boundary rather than slip through to the password
	// comparison below.
	request
		.validate()
		.map_err(|e| ServerFnError::application(format!("Validation failed: {}", e)))?;

	let manager = User::objects();
	let user = manager
		.filter(User::field_username().eq(request.username.trim().to_string()))
		.first()
		.await
		.map_err(|e| ServerFnError::application(format!("Database error: {}", e)))?
		.ok_or_else(|| ServerFnError::server(401, "Invalid credentials"))?;

	let valid = user
		.check_password(&request.password)
		.map_err(|e| ServerFnError::application(format!("Password check failed: {}", e)))?;

	if !valid {
		return Err(ServerFnError::server(401, "Invalid credentials"));
	}

	if !user.is_active() {
		return Err(ServerFnError::server(403, "User account is inactive"));
	}

	// Session fixation prevention: `SessionAuthExt::login` rotates the session
	// ID, writes the authenticated user's primary key under
	// `USER_ID_SESSION_KEY`, deletes the old store entry, and persists the
	// rotated session in one step. See issue #4446.
	session
		.login(&store, user.id())
		.map_err(|e| ServerFnError::application(format!("Session error: {}", e)))?;

	Ok(UserInfo::from(user))
}

/// Register a new account and start an authenticated session.
///
/// Mirrors `login`'s session-handling: on success the session id is rotated
/// (fixation prevention) and `user_id` is persisted so the caller is logged
/// in immediately — typical "sign-up then continue" UX for tutorials. The
/// CSRF is supplied by the `#[server_fn]` client stub through `X-CSRFToken`
/// and verified by middleware before this runs.
///
/// We invoke `request.validate()` manually rather than using
/// `#[server_fn(pre_validate = true)]` because that flag only triggers when
/// each parameter is an extractor type whose inner DTO derives `Validate`
/// (e.g. `body: Json<RegisterRequest>` — see
/// `tests/integration/src/pre_validate.rs`). The `form!` macro sends the
/// HTML form's fields as individual `String` params to keep its
/// `form!` flow working, so the macro-generated synthetic
/// `Args` struct only derives `Deserialize` — there is nothing on the
/// auto-path that knows the field-level `#[validate(...)]` attributes on
/// `RegisterRequest`. Building the DTO by hand and validating it
/// recovers the same guarantees without giving up the `form!` ergonomics.
#[server_fn]
pub async fn register(
	username: String,
	password: String,
	password_confirmation: String,
	#[inject] user_manager: Depends<AuthUserManagerKey, AuthUserManager>,
	#[inject] session: SessionData,
	#[inject] store: Depends<SessionStoreKey, Arc<SessionStore>>,
) -> Result<UserInfo, ServerFnError> {
	let mut session = session;

	let request = RegisterRequest {
		username,
		password,
		password_confirmation,
	};

	// Field-level validators declared on `RegisterRequest` (length on
	// username + password, non-empty password_confirmation). Run them
	// before the password-equality check so a too-short password does
	// not silently match a too-short confirmation.
	request
		.validate()
		.map_err(|e| ServerFnError::application(format!("Validation failed: {}", e)))?;

	// Password confirmation lives outside the derived `Validate` —
	// see the `validate_passwords_match` rationale on `RegisterRequest`.
	request
		.validate_passwords_match()
		.map_err(ServerFnError::application)?;

	// Delegate to `AuthUserManager` — it owns the "validate + hash +
	// persist" pipeline so this server function stays focused on session
	// handling. Username length, uniqueness, and password strength are all
	// enforced inside `create_user`; any failure surfaces as a
	// `reinhardt::Error` that maps to a 400 via `ServerFnError::application`.
	//
	// `BaseUserManager::create_user` takes `&mut self`, but DI hands us a
	// shared keyed `Depends`. Clone the inner manager — its only field is a
	// database handle — so this is cheap and gives us the `&mut` access the
	// trait method needs.
	let mut user_manager: AuthUserManager = (*user_manager).clone();
	let saved = user_manager
		.create_user(
			request.username.trim(),
			Some(&request.password),
			HashMap::new(),
		)
		.await
		.map_err(|e| ServerFnError::application(e.to_string()))?;

	// Match `login`: rotate via `SessionAuthExt::login` so the account is
	// signed in immediately on successful registration and the `"user_id"`
	// key stays centralised behind `USER_ID_SESSION_KEY`.
	session
		.login(&store, saved.id())
		.map_err(|e| ServerFnError::application(format!("Session error: {}", e)))?;

	Ok(UserInfo::from(saved))
}

/// Clear the active session.
///
/// CSRF is supplied by the `#[server_fn]` client stub; see [`login`].
#[server_fn]
pub async fn logout(
	#[inject] session: SessionData,
	#[inject] store: Depends<SessionStoreKey, Arc<SessionStore>>,
) -> Result<(), ServerFnError> {
	let mut session = session;

	// `SessionAuthExt::logout` rotates the session id, drops the user-id
	// key, and persists the rotated session — see the docstring on
	// `SessionAuthExt` for the rationale on rotation-before-clear.
	session.logout(&store);
	Ok(())
}

/// Return the currently authenticated user, if any.
#[server_fn]
pub async fn current_user(
	#[inject] _db: DatabaseConnection,
	#[inject] session: SessionData,
) -> Result<Option<UserInfo>, ServerFnError> {
	let Some(user_id) = session.get::<i64>(USER_ID_SESSION_KEY) else {
		return Ok(None);
	};

	let user = User::objects()
		.filter(User::field_id().eq(user_id))
		.first()
		.await
		.map_err(|e| ServerFnError::application(format!("Database error: {}", e)))?;

	Ok(user.map(UserInfo::from))
}
