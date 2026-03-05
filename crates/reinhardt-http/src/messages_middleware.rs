//! Messages middleware for HTTP request processing.
//!
//! This module provides `MessagesMiddleware` for integrating Django-style
//! flash messages into the HTTP request/response cycle.
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_http::{Handler, Request, Response, MessagesMiddleware};
//! use reinhardt_core::messages::MemoryStorage;
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! struct MyHandler;
//!
//! #[async_trait]
//! impl Handler for MyHandler {
//!     async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
//!         Ok(Response::ok())
//!     }
//! }
//!
//! let storage = MemoryStorage::new();
//! let middleware = MessagesMiddleware::new(storage);
//! ```

use async_trait::async_trait;
use reinhardt_core::messages::{MessageStorage, middleware::MessagesContainer};
use std::sync::{Arc, Mutex};

use crate::{Handler, Middleware, Request, Response};

/// Middleware for managing flash messages across HTTP requests.
///
/// This middleware:
/// 1. Loads existing messages from storage into the request extensions
/// 2. Processes the request through the handler chain
/// 3. Persists any new messages back to storage
///
/// ## Usage
///
/// ```rust,no_run
/// use reinhardt_http::MessagesMiddleware;
/// use reinhardt_core::messages::MemoryStorage;
///
/// let storage = MemoryStorage::new();
/// let middleware = MessagesMiddleware::new(storage);
/// ```
pub struct MessagesMiddleware<S: MessageStorage> {
	storage: Arc<Mutex<S>>,
}

impl<S: MessageStorage + 'static> MessagesMiddleware<S> {
	/// Creates a new messages middleware with the given storage backend.
	///
	/// # Arguments
	///
	/// * `storage` - The storage backend to use for persisting messages
	///
	/// # Example
	///
	/// ```rust,no_run
	/// use reinhardt_http::MessagesMiddleware;
	/// use reinhardt_core::messages::MemoryStorage;
	///
	/// let storage = MemoryStorage::new();
	/// let middleware = MessagesMiddleware::new(storage);
	/// ```
	pub fn new(storage: S) -> Self {
		Self {
			storage: Arc::new(Mutex::new(storage)),
		}
	}

	/// Returns a reference to the internal storage.
	///
	/// This is useful for testing or advanced use cases where direct
	/// access to the storage is needed.
	pub fn storage(&self) -> Arc<Mutex<S>> {
		self.storage.clone()
	}
}

#[async_trait]
impl<S: MessageStorage + 'static> Middleware for MessagesMiddleware<S> {
	async fn process(
		&self,
		request: Request,
		next: Arc<dyn Handler>,
	) -> reinhardt_core::exception::Result<Response> {
		// Load existing messages from storage into the request extensions
		let initial_messages = {
			let storage = self.storage.lock().unwrap_or_else(|e| e.into_inner());
			storage.peek().to_vec()
		};
		let initial_count = initial_messages.len();

		// Create a container with the loaded messages and insert into request extensions
		let container = MessagesContainer::new(initial_messages);
		request.extensions.insert(container.clone());

		// Process the request through the handler chain
		let response = next.handle(request).await?;

		// Sync storage with container state after handler processing
		{
			let mut storage = self.storage.lock().unwrap_or_else(|e| e.into_inner());
			let current_messages = container.get_messages();

			// If messages were consumed/cleared during request processing, sync storage
			if current_messages.len() < initial_count || current_messages.is_empty() {
				// Clear storage and sync with container state
				storage.clear();
				for msg in current_messages {
					storage.add(msg);
				}
			} else {
				// Only add new messages (messages that weren't in the initial set)
				for msg in current_messages {
					let is_new = !storage
						.peek()
						.iter()
						.any(|m| m.text == msg.text && m.level == msg.level);
					if is_new {
						storage.add(msg);
					}
				}
			}
		}

		Ok(response)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::middleware::MiddlewareChain;
	use bytes::Bytes;
	use hyper::{HeaderMap, Method, StatusCode, Version};
	use reinhardt_core::messages::{Level, MemoryStorage, Message};

	struct AddMessageHandler;

	#[async_trait]
	impl Handler for AddMessageHandler {
		async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
			if let Some(container) = request.extensions.get::<MessagesContainer>() {
				container.add(Message::new(Level::Success, "Test message"));
			}
			Ok(Response::new(StatusCode::OK))
		}
	}

	fn create_test_request() -> Request {
		Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap()
	}

	#[tokio::test]
	async fn test_messages_middleware_injects_container() {
		let storage = MemoryStorage::new();
		let middleware = MessagesMiddleware::new(storage);

		struct CheckContainerHandler;

		#[async_trait]
		impl Handler for CheckContainerHandler {
			async fn handle(
				&self,
				request: Request,
			) -> reinhardt_core::exception::Result<Response> {
				assert!(
					request.extensions.get::<MessagesContainer>().is_some(),
					"MessagesContainer should be present in request extensions"
				);
				Ok(Response::new(StatusCode::OK))
			}
		}

		let handler = Arc::new(CheckContainerHandler) as Arc<dyn Handler>;
		let request = create_test_request();
		let result = middleware.process(request, handler).await;

		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_messages_middleware_persists_messages() {
		let storage = MemoryStorage::new();
		let middleware = MessagesMiddleware::new(storage);

		let handler = Arc::new(AddMessageHandler) as Arc<dyn Handler>;
		let request = create_test_request();
		let _ = middleware.process(request, handler).await;

		// Check that the message was persisted to storage
		let stored = {
			let storage = middleware.storage.lock().unwrap_or_else(|e| e.into_inner());
			storage.peek().to_vec()
		};

		assert_eq!(stored.len(), 1);
		assert_eq!(stored[0].text, "Test message");
		assert_eq!(stored[0].level, Level::Success);
	}

	#[tokio::test]
	async fn test_messages_middleware_loads_existing_messages() {
		let mut storage = MemoryStorage::new();
		storage.add(Message::new(Level::Info, "Existing message"));
		let middleware = MessagesMiddleware::new(storage);

		struct CheckExistingHandler;

		#[async_trait]
		impl Handler for CheckExistingHandler {
			async fn handle(
				&self,
				request: Request,
			) -> reinhardt_core::exception::Result<Response> {
				if let Some(container) = request.extensions.get::<MessagesContainer>() {
					let messages = container.get_messages();
					assert_eq!(messages.len(), 1);
					assert_eq!(messages[0].text, "Existing message");
				}
				Ok(Response::new(StatusCode::OK))
			}
		}

		let handler = Arc::new(CheckExistingHandler) as Arc<dyn Handler>;
		let request = create_test_request();
		let result = middleware.process(request, handler).await;

		assert!(result.is_ok());
	}

	#[tokio::test]
	async fn test_messages_middleware_chain_integration() {
		let storage = MemoryStorage::new();
		let middleware = Arc::new(MessagesMiddleware::new(storage));

		let handler = Arc::new(AddMessageHandler) as Arc<dyn Handler>;
		let chain = MiddlewareChain::new(handler).with_middleware(middleware);

		let request = create_test_request();
		let result = chain.handle(request).await;

		assert!(result.is_ok());
	}
}
