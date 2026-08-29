//! Pluggable async session backend trait.

use async_trait::async_trait;
use reinhardt_http::Result;
use std::time::Duration;

use super::data::SessionData;

/// Async trait for pluggable session storage backends.
///
/// Implement this trait to integrate any async-capable session store
/// (e.g. Redis, DynamoDB, PostgreSQL) with the session middleware layer.
///
/// # Example
///
/// ```rust,ignore
/// use std::time::Duration;
/// use reinhardt_middleware::session::{AsyncSessionBackend, SessionData};
/// use reinhardt_http::Result;
///
/// struct MyBackend;
///
/// #[async_trait::async_trait]
/// impl AsyncSessionBackend for MyBackend {
///     async fn load(&self, id: &str) -> Result<Option<SessionData>> { Ok(None) }
///     async fn save(&self, session: &SessionData) -> Result<()> { Ok(()) }
///     async fn destroy(&self, id: &str) -> Result<()> { Ok(()) }
///     async fn touch(&self, id: &str, ttl: Duration) -> Result<()> { Ok(()) }
/// }
/// ```
#[async_trait]
pub trait AsyncSessionBackend: Send + Sync {
	/// Load a session by ID. Returns `None` if the session does not exist
	/// or has expired.
	async fn load(&self, id: &str) -> Result<Option<SessionData>>;

	/// Persist a session (insert or update).
	async fn save(&self, session: &SessionData) -> Result<()>;

	/// Remove a session by ID.
	async fn destroy(&self, id: &str) -> Result<()>;

	/// Refresh the TTL of an existing session without rewriting the full payload.
	async fn touch(&self, id: &str, ttl: Duration) -> Result<()>;
}

/// Optional atomic capability for session backends.
///
/// Implementations must return and delete one session in a single backend
/// operation. Missing and expired sessions are returned as `None`.
#[async_trait]
pub trait AtomicSessionBackend: AsyncSessionBackend {
	/// Atomically load and delete one unexpired session.
	async fn take(&self, id: &str) -> Result<Option<SessionData>>;
}
