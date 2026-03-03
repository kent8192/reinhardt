//! WebSocket test client and utilities for integration testing
//!
//! Provides WebSocket test client for end-to-end WebSocket testing with
//! support for authentication, connection management, and message assertions.
//!
//! ## Usage Examples
//!
//! ### Basic WebSocket Connection
//!
//! ```rust,no_run
//! use reinhardt_test::websocket::WebSocketTestClient;
//! use rstest::*;
//!
//! #[rstest]
//! #[tokio::test]
//! async fn test_websocket_connection() {
//!     let client = WebSocketTestClient::connect("ws://localhost:8080/ws").await.unwrap();
//!     client.send_text("Hello").await.unwrap();
//!     let response = client.receive_text().await.unwrap();
//!     assert_eq!(response, "Hello");
//! }
//! ```
//!
//! ### WebSocket with Authentication
//!
//! ```rust,no_run
//! use reinhardt_test::websocket::WebSocketTestClient;
//!
//! #[tokio::test]
//! async fn test_websocket_auth() {
//!     let client = WebSocketTestClient::connect_with_token(
//!         "ws://localhost:8080/ws",
//!         "my-auth-token"
//!     ).await.unwrap();
//!     // ...
//! }
//! ```

use futures::{SinkExt, StreamExt};
use std::io::{Error as IoError, ErrorKind};
use tokio::time::{Duration, timeout};
use tokio_tungstenite::{
	MaybeTlsStream, WebSocketStream, connect_async,
	tungstenite::{Error as WsError, Message},
};

/// WebSocket test client for integration testing
///
/// Provides high-level API for WebSocket connection management, message sending/receiving,
/// and authentication.
pub struct WebSocketTestClient {
	/// WebSocket connection stream
	stream: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
	/// WebSocket URL
	url: String,
}

impl WebSocketTestClient {
	/// Connect to WebSocket server
	///
	/// # Example
	/// ```rust,no_run
	/// use reinhardt_test::websocket::WebSocketTestClient;
	///
	/// #[tokio::test]
	/// async fn test_connect() {
	///     let client = WebSocketTestClient::connect("ws://localhost:8080/ws")
	///         .await
	///         .unwrap();
	/// }
	/// ```
	pub async fn connect(url: &str) -> Result<Self, WsError> {
		let (stream, _response) = connect_async(url).await?;
		Ok(Self {
			stream,
			url: url.to_string(),
		})
	}

	/// Connect to WebSocket server with Bearer token authentication
	///
	/// Adds `Authorization: Bearer <token>` header to the WebSocket handshake request.
	///
	/// # Example
	/// ```rust,no_run
	/// use reinhardt_test::websocket::WebSocketTestClient;
	///
	/// #[tokio::test]
	/// async fn test_auth() {
	///     let client = WebSocketTestClient::connect_with_token(
	///         "ws://localhost:8080/ws",
	///         "my-secret-token"
	///     )
	///     .await
	///     .unwrap();
	/// }
	/// ```
	pub async fn connect_with_token(url: &str, token: &str) -> Result<Self, WsError> {
		use tokio_tungstenite::tungstenite::http::Request;

		let request = Request::builder()
			.uri(url)
			.header("Authorization", format!("Bearer {}", token))
			.body(())
			.expect("Failed to build WebSocket request");

		let (stream, _response) = connect_async(request).await?;
		Ok(Self {
			stream,
			url: url.to_string(),
		})
	}

	/// Connect to WebSocket server with query parameter authentication
	///
	/// Appends `?token=<token>` to the URL.
	///
	/// # Example
	/// ```rust,no_run
	/// use reinhardt_test::websocket::WebSocketTestClient;
	///
	/// #[tokio::test]
	/// async fn test_query_auth() {
	///     let client = WebSocketTestClient::connect_with_query_token(
	///         "ws://localhost:8080/ws",
	///         "my-token"
	///     )
	///     .await
	///     .unwrap();
	/// }
	/// ```
	// Fixes #880: URL-encode token to prevent injection via query parameter
	pub async fn connect_with_query_token(url: &str, token: &str) -> Result<Self, WsError> {
		let url_with_token = format!("{}?token={}", url, urlencoding::encode(token));
		Self::connect(&url_with_token).await
	}

	/// Connect to WebSocket server with cookie authentication
	///
	/// Adds `Cookie: <cookie_name>=<cookie_value>` header to the WebSocket handshake request.
	///
	/// # Example
	/// ```rust,no_run
	/// use reinhardt_test::websocket::WebSocketTestClient;
	///
	/// #[tokio::test]
	/// async fn test_cookie_auth() {
	///     let client = WebSocketTestClient::connect_with_cookie(
	///         "ws://localhost:8080/ws",
	///         "session_id",
	///         "abc123"
	///     )
	///     .await
	///     .unwrap();
	/// }
	/// ```
	pub async fn connect_with_cookie(
		url: &str,
		cookie_name: &str,
		cookie_value: &str,
	) -> Result<Self, WsError> {
		use tokio_tungstenite::tungstenite::http::Request;

		let request = Request::builder()
			.uri(url)
			.header("Cookie", format!("{}={}", cookie_name, cookie_value))
			.body(())
			.expect("Failed to build WebSocket request");

		let (stream, _response) = connect_async(request).await?;
		Ok(Self {
			stream,
			url: url.to_string(),
		})
	}

	/// Send text message to WebSocket server
	///
	/// # Example
	/// ```rust,no_run
	/// use reinhardt_test::websocket::WebSocketTestClient;
	///
	/// #[tokio::test]
	/// async fn test_send() {
	///     let mut client = WebSocketTestClient::connect("ws://localhost:8080/ws")
	///         .await
	///         .unwrap();
	///     client.send_text("Hello").await.unwrap();
	/// }
	/// ```
	pub async fn send_text(&mut self, text: &str) -> Result<(), WsError> {
		self.stream.send(Message::text(text)).await
	}

	/// Send binary message to WebSocket server
	pub async fn send_binary(&mut self, data: &[u8]) -> Result<(), WsError> {
		self.stream.send(Message::binary(data.to_vec())).await
	}

	/// Send ping message to WebSocket server
	pub async fn send_ping(&mut self, payload: &[u8]) -> Result<(), WsError> {
		self.stream
			.send(Message::Ping(payload.to_vec().into()))
			.await
	}

	/// Send pong message to WebSocket server
	pub async fn send_pong(&mut self, payload: &[u8]) -> Result<(), WsError> {
		self.stream
			.send(Message::Pong(payload.to_vec().into()))
			.await
	}

	/// Receive next message from WebSocket server
	///
	/// Returns `None` if connection is closed.
	pub async fn receive(&mut self) -> Option<Result<Message, WsError>> {
		self.stream.next().await
	}

	/// Receive text message from WebSocket server with timeout
	///
	/// # Example
	/// ```rust,no_run
	/// use reinhardt_test::websocket::WebSocketTestClient;
	///
	/// #[tokio::test]
	/// async fn test_receive() {
	///     let mut client = WebSocketTestClient::connect("ws://localhost:8080/ws")
	///         .await
	///         .unwrap();
	///     let text = client.receive_text().await.unwrap();
	///     assert_eq!(text, "Welcome");
	/// }
	/// ```
	pub async fn receive_text(&mut self) -> Result<String, WsError> {
		self.receive_text_with_timeout(Duration::from_secs(5)).await
	}

	/// Receive text message with custom timeout
	pub async fn receive_text_with_timeout(
		&mut self,
		duration: Duration,
	) -> Result<String, WsError> {
		match timeout(duration, self.stream.next()).await {
			Ok(Some(Ok(Message::Text(text)))) => Ok(text.to_string()),
			Ok(Some(Ok(msg))) => Err(WsError::Io(IoError::new(
				ErrorKind::InvalidData,
				format!("Expected text message, got {:?}", msg),
			))),
			Ok(Some(Err(e))) => Err(e),
			Ok(None) => Err(WsError::ConnectionClosed),
			Err(_) => Err(WsError::Io(IoError::new(
				ErrorKind::TimedOut,
				"Receive timeout",
			))),
		}
	}

	/// Receive binary message from WebSocket server with timeout
	pub async fn receive_binary(&mut self) -> Result<Vec<u8>, WsError> {
		self.receive_binary_with_timeout(Duration::from_secs(5))
			.await
	}

	/// Receive binary message with custom timeout
	pub async fn receive_binary_with_timeout(
		&mut self,
		duration: Duration,
	) -> Result<Vec<u8>, WsError> {
		match timeout(duration, self.stream.next()).await {
			Ok(Some(Ok(Message::Binary(data)))) => Ok(data.to_vec()),
			Ok(Some(Ok(msg))) => Err(WsError::Io(IoError::new(
				ErrorKind::InvalidData,
				format!("Expected binary message, got {:?}", msg),
			))),
			Ok(Some(Err(e))) => Err(e),
			Ok(None) => Err(WsError::ConnectionClosed),
			Err(_) => Err(WsError::Io(IoError::new(
				ErrorKind::TimedOut,
				"Receive timeout",
			))),
		}
	}

	/// Close WebSocket connection
	pub async fn close(mut self) -> Result<(), WsError> {
		self.stream.close(None).await
	}

	/// Get WebSocket URL
	pub fn url(&self) -> &str {
		&self.url
	}
}

/// WebSocket message assertion utilities
pub mod assertions {
	use tokio_tungstenite::tungstenite::Message;

	/// Assert that WebSocket message is text with expected content
	///
	/// # Example
	/// ```rust,no_run
	/// use reinhardt_test::websocket::assertions::assert_message_text;
	/// use tokio_tungstenite::tungstenite::Message;
	///
	/// let msg = Message::text("Hello");
	/// assert_message_text(&msg, "Hello");
	/// ```
	pub fn assert_message_text(msg: &Message, expected: &str) {
		match msg {
			Message::Text(text) => assert_eq!(text.as_str(), expected),
			_ => panic!("Expected text message, got {:?}", msg),
		}
	}

	/// Assert that WebSocket message is text containing substring
	pub fn assert_message_contains(msg: &Message, substring: &str) {
		match msg {
			Message::Text(text) => assert!(
				text.contains(substring),
				"Message '{}' does not contain '{}'",
				text,
				substring
			),
			_ => panic!("Expected text message, got {:?}", msg),
		}
	}

	/// Assert that WebSocket message is binary with expected data
	pub fn assert_message_binary(msg: &Message, expected: &[u8]) {
		match msg {
			Message::Binary(data) => assert_eq!(data.as_ref(), expected),
			_ => panic!("Expected binary message, got {:?}", msg),
		}
	}

	/// Assert that WebSocket message is ping
	pub fn assert_message_ping(msg: &Message) {
		match msg {
			Message::Ping(_) => {}
			_ => panic!("Expected ping message, got {:?}", msg),
		}
	}

	/// Assert that WebSocket message is pong
	pub fn assert_message_pong(msg: &Message) {
		match msg {
			Message::Pong(_) => {}
			_ => panic!("Expected pong message, got {:?}", msg),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_url_with_query_token() {
		let url = "ws://localhost:8080/ws";
		let token = "my-token";
		let expected = "ws://localhost:8080/ws?token=my-token";

		let url_with_token = format!("{}?token={}", url, urlencoding::encode(token));
		assert_eq!(url_with_token, expected);
	}

	#[test]
	fn test_url_with_query_token_special_chars() {
		let url = "ws://localhost:8080/ws";
		let token = "token with spaces&special=chars";
		let url_with_token = format!("{}?token={}", url, urlencoding::encode(token));
		assert_eq!(
			url_with_token,
			"ws://localhost:8080/ws?token=token%20with%20spaces%26special%3Dchars"
		);
	}

	#[test]
	fn test_message_assertions() {
		use assertions::*;

		let text_msg = Message::text("Hello");
		assert_message_text(&text_msg, "Hello");
		assert_message_contains(&text_msg, "ell");

		let binary_msg = Message::Binary(vec![1, 2, 3].into());
		assert_message_binary(&binary_msg, &[1, 2, 3]);

		let ping_msg = Message::Ping(vec![].into());
		assert_message_ping(&ping_msg);

		let pong_msg = Message::Pong(vec![].into());
		assert_message_pong(&pong_msg);
	}
}
