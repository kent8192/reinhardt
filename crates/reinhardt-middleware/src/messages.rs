//! Messages middleware
//!
//! Provides Django-style flash messages for one-time notifications.
//! Messages can be stored in sessions or cookies and are displayed once.
//!
//! # Middleware Ordering
//!
//! For best results, ensure `SessionMiddleware` runs before `MessageMiddleware`:
//!
//! ```ignore
//! app.middleware(SessionMiddleware::new(config))
//!    .middleware(MessageMiddleware::new(storage));
//! ```
//!
//! This allows `MessageMiddleware` to use the session ID from
//! `SessionMiddleware` via request extensions.

use async_trait::async_trait;
use hyper::header::COOKIE;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::session::SessionData;

/// Message header for passing messages between middleware and handlers
pub const MESSAGE_HEADER: &str = "X-Messages";

/// Message severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageLevel {
	Debug,
	Info,
	Success,
	Warning,
	Error,
}

/// A single flash message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
	pub level: MessageLevel,
	pub text: String,
}

impl Message {
	/// Create a new message
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::messages::{Message, MessageLevel};
	///
	/// let msg = Message::new(MessageLevel::Success, "Saved successfully!".to_string());
	/// assert_eq!(msg.level, MessageLevel::Success);
	/// ```
	pub fn new(level: MessageLevel, text: String) -> Self {
		Self { level, text }
	}

	/// Create a debug message
	pub fn debug(text: String) -> Self {
		Self::new(MessageLevel::Debug, text)
	}

	/// Create an info message
	pub fn info(text: String) -> Self {
		Self::new(MessageLevel::Info, text)
	}

	/// Create a success message
	pub fn success(text: String) -> Self {
		Self::new(MessageLevel::Success, text)
	}

	/// Create a warning message
	pub fn warning(text: String) -> Self {
		Self::new(MessageLevel::Warning, text)
	}

	/// Create an error message
	pub fn error(text: String) -> Self {
		Self::new(MessageLevel::Error, text)
	}
}

/// Message storage trait
pub trait MessageStorage: Send + Sync {
	/// Add a message to storage
	fn add_message(&self, session_id: &str, message: Message);
	/// Get all messages for a session and clear them
	fn get_and_clear_messages(&self, session_id: &str) -> Vec<Message>;
	/// Get messages without clearing
	fn get_messages(&self, session_id: &str) -> Vec<Message>;
}

/// Session-based message storage
///
/// Stores messages in memory keyed by session ID.
/// In production, this should be backed by a persistent session store.
pub struct SessionStorage {
	messages: Arc<RwLock<HashMap<String, Vec<Message>>>>,
}

impl SessionStorage {
	/// Create a new SessionStorage
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::messages::SessionStorage;
	///
	/// let storage = SessionStorage::new();
	/// ```
	pub fn new() -> Self {
		Self {
			messages: Arc::new(RwLock::new(HashMap::new())),
		}
	}
}

impl Default for SessionStorage {
	fn default() -> Self {
		Self::new()
	}
}

impl MessageStorage for SessionStorage {
	fn add_message(&self, session_id: &str, message: Message) {
		let mut messages = self.messages.write().unwrap_or_else(|e| e.into_inner());
		messages
			.entry(session_id.to_string())
			.or_default()
			.push(message);
	}

	fn get_and_clear_messages(&self, session_id: &str) -> Vec<Message> {
		let mut messages = self.messages.write().unwrap_or_else(|e| e.into_inner());
		messages.remove(session_id).unwrap_or_default()
	}

	fn get_messages(&self, session_id: &str) -> Vec<Message> {
		let messages = self.messages.read().unwrap_or_else(|e| e.into_inner());
		messages.get(session_id).cloned().unwrap_or_default()
	}
}

/// Cookie-based message storage
///
/// Stores messages in memory similar to SessionStorage but designed
/// to be serialized to cookies in production.
pub struct CookieStorage {
	messages: Arc<RwLock<HashMap<String, Vec<Message>>>>,
}

impl CookieStorage {
	/// Create a new CookieStorage
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::messages::CookieStorage;
	///
	/// let storage = CookieStorage::new();
	/// ```
	pub fn new() -> Self {
		Self {
			messages: Arc::new(RwLock::new(HashMap::new())),
		}
	}
}

impl Default for CookieStorage {
	fn default() -> Self {
		Self::new()
	}
}

impl MessageStorage for CookieStorage {
	fn add_message(&self, session_id: &str, message: Message) {
		let mut messages = self.messages.write().unwrap_or_else(|e| e.into_inner());
		messages
			.entry(session_id.to_string())
			.or_default()
			.push(message);
	}

	fn get_and_clear_messages(&self, session_id: &str) -> Vec<Message> {
		let mut messages = self.messages.write().unwrap_or_else(|e| e.into_inner());
		messages.remove(session_id).unwrap_or_default()
	}

	fn get_messages(&self, session_id: &str) -> Vec<Message> {
		let messages = self.messages.read().unwrap_or_else(|e| e.into_inner());
		messages.get(session_id).cloned().unwrap_or_default()
	}
}

/// Message framework middleware
///
/// Provides flash message functionality similar to Django's messages framework.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use reinhardt_middleware::messages::{MessageMiddleware, SessionStorage, Message, MessageLevel};
/// use reinhardt_http::{Handler, Middleware, Request, Response};
/// use hyper::{StatusCode, Method, Version, HeaderMap};
/// use bytes::Bytes;
///
/// struct TestHandler {
///     storage: Arc<dyn reinhardt_middleware::messages::MessageStorage>,
/// }
///
/// #[async_trait::async_trait]
/// impl Handler for TestHandler {
///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
///         // Add a message
///         self.storage.add_message("test-session", Message::success("Operation successful!".to_string()));
///         Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
///     }
/// }
///
/// # tokio_test::block_on(async {
/// let storage: Arc<dyn reinhardt_middleware::messages::MessageStorage> = Arc::new(SessionStorage::new());
/// let middleware = MessageMiddleware::new(storage.clone());
/// let handler = Arc::new(TestHandler { storage: storage.clone() });
///
/// let mut headers = HeaderMap::new();
/// headers.insert(hyper::header::COOKIE, "sessionid=test-session".parse().unwrap());
///
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/page")
///     .version(Version::HTTP_11)
///     .headers(headers)
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// let _response = middleware.process(request, handler).await.unwrap();
/// let messages = storage.get_and_clear_messages("test-session");
/// assert_eq!(messages.len(), 1);
/// assert_eq!(messages[0].level, MessageLevel::Success);
/// # });
/// ```
#[allow(dead_code)]
pub struct MessageMiddleware {
	storage: Arc<dyn MessageStorage>,
}

impl MessageMiddleware {
	/// Create a new MessageMiddleware with the given storage backend
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_middleware::messages::{MessageMiddleware, SessionStorage};
	///
	/// let storage = Arc::new(SessionStorage::new());
	/// let middleware = MessageMiddleware::new(storage);
	/// ```
	pub fn new(storage: Arc<dyn MessageStorage>) -> Self {
		Self { storage }
	}

	/// Extract session ID from request
	///
	/// This method first checks for `SessionData` in request extensions
	/// (set by `SessionMiddleware`), then falls back to cookie extraction.
	fn get_session_id(request: &Request) -> String {
		// Check for SessionData set by SessionMiddleware
		if let Some(session_data) = request.extensions.get::<SessionData>() {
			return session_data.id.clone();
		}

		// Fallback: extract from cookie
		request
			.headers
			.get(COOKIE)
			.and_then(|c| c.to_str().ok())
			.and_then(|cookies| {
				for cookie in cookies.split(';') {
					let cookie = cookie.trim();
					if let Some((name, value)) = cookie.split_once('=')
						&& name == "sessionid"
					{
						return Some(value.to_string());
					}
				}
				None
			})
			.unwrap_or_else(|| "default".to_string())
	}
}

#[async_trait]
impl Middleware for MessageMiddleware {
	async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
		let _session_id = Self::get_session_id(&request);

		// Process the request
		let response = handler.handle(request).await?;

		// Messages are stored in the storage and can be retrieved by handlers
		// In a complete implementation, we'd add messages to the response or template context
		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};

	#[test]
	fn test_message_creation() {
		let msg = Message::debug("Debug message".to_string());
		assert_eq!(msg.level, MessageLevel::Debug);

		let msg = Message::info("Info message".to_string());
		assert_eq!(msg.level, MessageLevel::Info);

		let msg = Message::success("Success message".to_string());
		assert_eq!(msg.level, MessageLevel::Success);

		let msg = Message::warning("Warning message".to_string());
		assert_eq!(msg.level, MessageLevel::Warning);

		let msg = Message::error("Error message".to_string());
		assert_eq!(msg.level, MessageLevel::Error);
	}

	#[test]
	fn test_session_storage_add_and_get() {
		let storage = SessionStorage::new();
		let session_id = "test-session";

		storage.add_message(session_id, Message::info("Message 1".to_string()));
		storage.add_message(session_id, Message::success("Message 2".to_string()));

		let messages = storage.get_messages(session_id);
		assert_eq!(messages.len(), 2);
		assert_eq!(messages[0].level, MessageLevel::Info);
		assert_eq!(messages[1].level, MessageLevel::Success);
	}

	#[test]
	fn test_session_storage_clear() {
		let storage = SessionStorage::new();
		let session_id = "test-session";

		storage.add_message(session_id, Message::info("Message 1".to_string()));
		storage.add_message(session_id, Message::info("Message 2".to_string()));

		let messages = storage.get_and_clear_messages(session_id);
		assert_eq!(messages.len(), 2);

		// Messages should be cleared
		let messages = storage.get_messages(session_id);
		assert_eq!(messages.len(), 0);
	}

	#[test]
	fn test_cookie_storage_add_and_get() {
		let storage = CookieStorage::new();
		let session_id = "test-session";

		storage.add_message(session_id, Message::warning("Warning 1".to_string()));
		storage.add_message(session_id, Message::error("Error 1".to_string()));

		let messages = storage.get_messages(session_id);
		assert_eq!(messages.len(), 2);
		assert_eq!(messages[0].level, MessageLevel::Warning);
		assert_eq!(messages[1].level, MessageLevel::Error);
	}

	#[test]
	fn test_cookie_storage_clear() {
		let storage = CookieStorage::new();
		let session_id = "test-session";

		storage.add_message(session_id, Message::info("Info 1".to_string()));

		let messages = storage.get_and_clear_messages(session_id);
		assert_eq!(messages.len(), 1);

		// Messages should be cleared
		let messages = storage.get_messages(session_id);
		assert_eq!(messages.len(), 0);
	}

	#[test]
	fn test_separate_sessions() {
		let storage = SessionStorage::new();

		storage.add_message("session1", Message::info("Session 1 message".to_string()));
		storage.add_message(
			"session2",
			Message::success("Session 2 message".to_string()),
		);

		let messages1 = storage.get_messages("session1");
		let messages2 = storage.get_messages("session2");

		assert_eq!(messages1.len(), 1);
		assert_eq!(messages2.len(), 1);
		assert_eq!(messages1[0].level, MessageLevel::Info);
		assert_eq!(messages2[0].level, MessageLevel::Success);
	}

	struct TestHandler {
		storage: Arc<dyn MessageStorage>,
	}

	#[async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, request: Request) -> Result<Response> {
			let session_id = MessageMiddleware::get_session_id(&request);
			self.storage
				.add_message(&session_id, Message::success("Test message".to_string()));
			Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
		}
	}

	#[tokio::test]
	async fn test_middleware_with_session_storage() {
		let storage: Arc<dyn MessageStorage> = Arc::new(SessionStorage::new());
		let middleware = MessageMiddleware::new(storage.clone());
		let handler = Arc::new(TestHandler {
			storage: storage.clone(),
		});

		let mut headers = HeaderMap::new();
		headers.insert(COOKIE, "sessionid=test-session".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/page")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		assert_eq!(response.status, StatusCode::OK);

		// Verify message was stored
		let messages = storage.get_and_clear_messages("test-session");
		assert_eq!(messages.len(), 1);
		assert_eq!(messages[0].level, MessageLevel::Success);
	}

	#[tokio::test]
	async fn test_middleware_default_session() {
		let storage: Arc<dyn MessageStorage> = Arc::new(SessionStorage::new());
		let middleware = MessageMiddleware::new(storage.clone());
		let handler = Arc::new(TestHandler {
			storage: storage.clone(),
		});

		// No session cookie
		let request = Request::builder()
			.method(Method::GET)
			.uri("/page")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		assert_eq!(response.status, StatusCode::OK);

		// Should use default session
		let messages = storage.get_messages("default");
		assert_eq!(messages.len(), 1);
	}

	#[tokio::test]
	async fn test_middleware_with_cookie_storage() {
		let storage: Arc<dyn MessageStorage> = Arc::new(CookieStorage::new());
		let middleware = MessageMiddleware::new(storage.clone());
		let handler = Arc::new(TestHandler {
			storage: storage.clone(),
		});

		let mut headers = HeaderMap::new();
		headers.insert(COOKIE, "sessionid=cookie-session".parse().unwrap());

		let request = Request::builder()
			.method(Method::GET)
			.uri("/page")
			.version(Version::HTTP_11)
			.headers(headers)
			.body(Bytes::new())
			.build()
			.unwrap();

		let response = middleware.process(request, handler).await.unwrap();
		assert_eq!(response.status, StatusCode::OK);

		// Verify message was stored
		let messages = storage.get_and_clear_messages("cookie-session");
		assert_eq!(messages.len(), 1);
		assert_eq!(messages[0].level, MessageLevel::Success);
	}
}
