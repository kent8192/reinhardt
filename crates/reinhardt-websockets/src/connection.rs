use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::sync::mpsc;

/// Ping/pong keepalive configuration for WebSocket connections.
///
/// Controls how frequently ping frames are sent and how long
/// the server waits for a pong response before considering
/// the connection dead.
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::connection::PingPongConfig;
/// use std::time::Duration;
///
/// // Use defaults (30s ping interval, 10s pong timeout)
/// let config = PingPongConfig::default();
/// assert_eq!(config.ping_interval(), Duration::from_secs(30));
/// assert_eq!(config.pong_timeout(), Duration::from_secs(10));
///
/// // Custom configuration
/// let config = PingPongConfig::new(
///     Duration::from_secs(15),
///     Duration::from_secs(5),
/// );
/// assert_eq!(config.ping_interval(), Duration::from_secs(15));
/// assert_eq!(config.pong_timeout(), Duration::from_secs(5));
/// ```
#[derive(Debug, Clone)]
pub struct PingPongConfig {
	/// Interval between ping frames sent to the client
	ping_interval: Duration,
	/// Maximum time to wait for a pong response before closing
	pong_timeout: Duration,
}

impl Default for PingPongConfig {
	fn default() -> Self {
		Self {
			ping_interval: Duration::from_secs(30),
			pong_timeout: Duration::from_secs(10),
		}
	}
}

impl PingPongConfig {
	/// Creates a new ping/pong configuration with the given intervals.
	///
	/// # Arguments
	///
	/// * `ping_interval` - How often to send ping frames
	/// * `pong_timeout` - How long to wait for a pong response
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::connection::PingPongConfig;
	/// use std::time::Duration;
	///
	/// let config = PingPongConfig::new(
	///     Duration::from_secs(20),
	///     Duration::from_secs(8),
	/// );
	/// assert_eq!(config.ping_interval(), Duration::from_secs(20));
	/// assert_eq!(config.pong_timeout(), Duration::from_secs(8));
	/// ```
	pub fn new(ping_interval: Duration, pong_timeout: Duration) -> Self {
		Self {
			ping_interval,
			pong_timeout,
		}
	}

	/// Returns the ping interval duration.
	pub fn ping_interval(&self) -> Duration {
		self.ping_interval
	}

	/// Returns the pong timeout duration.
	pub fn pong_timeout(&self) -> Duration {
		self.pong_timeout
	}

	/// Sets the ping interval duration.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::connection::PingPongConfig;
	/// use std::time::Duration;
	///
	/// let config = PingPongConfig::default()
	///     .with_ping_interval(Duration::from_secs(60));
	/// assert_eq!(config.ping_interval(), Duration::from_secs(60));
	/// ```
	pub fn with_ping_interval(mut self, interval: Duration) -> Self {
		self.ping_interval = interval;
		self
	}

	/// Sets the pong timeout duration.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::connection::PingPongConfig;
	/// use std::time::Duration;
	///
	/// let config = PingPongConfig::default()
	///     .with_pong_timeout(Duration::from_secs(15));
	/// assert_eq!(config.pong_timeout(), Duration::from_secs(15));
	/// ```
	pub fn with_pong_timeout(mut self, timeout: Duration) -> Self {
		self.pong_timeout = timeout;
		self
	}
}

/// Connection timeout configuration
///
/// This struct defines the timeout settings for WebSocket connections
/// to prevent resource exhaustion from idle connections.
///
/// # Fields
///
/// - `idle_timeout` - Maximum duration a connection can be idle before being closed (default: 5 minutes)
/// - `handshake_timeout` - Maximum duration for the WebSocket handshake to complete (default: 10 seconds)
/// - `cleanup_interval` - Interval for checking idle connections (default: 30 seconds)
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::connection::ConnectionConfig;
/// use std::time::Duration;
///
/// let config = ConnectionConfig::new()
///     .with_idle_timeout(Duration::from_secs(300))
///     .with_handshake_timeout(Duration::from_secs(10))
///     .with_cleanup_interval(Duration::from_secs(30));
///
/// assert_eq!(config.idle_timeout(), Duration::from_secs(300));
/// assert_eq!(config.handshake_timeout(), Duration::from_secs(10));
/// assert_eq!(config.cleanup_interval(), Duration::from_secs(30));
/// ```
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
	idle_timeout: Duration,
	handshake_timeout: Duration,
	cleanup_interval: Duration,
	/// Maximum number of concurrent connections allowed (None for unlimited)
	max_connections: Option<usize>,
	/// Ping/pong keepalive configuration
	ping_config: PingPongConfig,
}

impl Default for ConnectionConfig {
	fn default() -> Self {
		Self {
			idle_timeout: Duration::from_secs(300), // 5 minutes default
			handshake_timeout: Duration::from_secs(10), // 10 seconds default
			cleanup_interval: Duration::from_secs(30), // 30 seconds default
			max_connections: None,                  // Unlimited by default
			ping_config: PingPongConfig::default(),
		}
	}
}

impl ConnectionConfig {
	/// Create a new connection configuration with default values
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::connection::ConnectionConfig;
	/// use std::time::Duration;
	///
	/// let config = ConnectionConfig::new();
	/// assert_eq!(config.idle_timeout(), Duration::from_secs(300));
	/// assert_eq!(config.handshake_timeout(), Duration::from_secs(10));
	/// assert_eq!(config.cleanup_interval(), Duration::from_secs(30));
	/// ```
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the idle timeout duration
	///
	/// # Arguments
	///
	/// * `timeout` - Maximum duration a connection can be idle before being closed
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::connection::ConnectionConfig;
	/// use std::time::Duration;
	///
	/// let config = ConnectionConfig::new()
	///     .with_idle_timeout(Duration::from_secs(60));
	///
	/// assert_eq!(config.idle_timeout(), Duration::from_secs(60));
	/// ```
	pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
		self.idle_timeout = timeout;
		self
	}

	/// Set the handshake timeout duration
	///
	/// # Arguments
	///
	/// * `timeout` - Maximum duration for the WebSocket handshake to complete
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::connection::ConnectionConfig;
	/// use std::time::Duration;
	///
	/// let config = ConnectionConfig::new()
	///     .with_handshake_timeout(Duration::from_secs(5));
	///
	/// assert_eq!(config.handshake_timeout(), Duration::from_secs(5));
	/// ```
	pub fn with_handshake_timeout(mut self, timeout: Duration) -> Self {
		self.handshake_timeout = timeout;
		self
	}

	/// Set the cleanup interval for checking idle connections
	///
	/// # Arguments
	///
	/// * `interval` - How often to check for idle connections
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::connection::ConnectionConfig;
	/// use std::time::Duration;
	///
	/// let config = ConnectionConfig::new()
	///     .with_cleanup_interval(Duration::from_secs(15));
	///
	/// assert_eq!(config.cleanup_interval(), Duration::from_secs(15));
	/// ```
	pub fn with_cleanup_interval(mut self, interval: Duration) -> Self {
		self.cleanup_interval = interval;
		self
	}

	/// Get the idle timeout duration
	pub fn idle_timeout(&self) -> Duration {
		self.idle_timeout
	}

	/// Get the handshake timeout duration
	pub fn handshake_timeout(&self) -> Duration {
		self.handshake_timeout
	}

	/// Get the cleanup interval duration
	pub fn cleanup_interval(&self) -> Duration {
		self.cleanup_interval
	}

	/// Set the maximum number of concurrent connections
	///
	/// # Arguments
	///
	/// * `max` - Maximum number of connections allowed. Use `None` for unlimited.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::connection::ConnectionConfig;
	///
	/// let config = ConnectionConfig::new()
	///     .with_max_connections(Some(1000));
	///
	/// assert_eq!(config.max_connections(), Some(1000));
	/// ```
	pub fn with_max_connections(mut self, max: Option<usize>) -> Self {
		self.max_connections = max;
		self
	}

	/// Get the maximum number of concurrent connections
	pub fn max_connections(&self) -> Option<usize> {
		self.max_connections
	}

	/// Set the ping/pong keepalive configuration.
	///
	/// # Arguments
	///
	/// * `config` - The ping/pong configuration to use
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::connection::{ConnectionConfig, PingPongConfig};
	/// use std::time::Duration;
	///
	/// let ping_config = PingPongConfig::new(
	///     Duration::from_secs(15),
	///     Duration::from_secs(5),
	/// );
	/// let config = ConnectionConfig::new()
	///     .with_ping_config(ping_config);
	///
	/// assert_eq!(config.ping_config().ping_interval(), Duration::from_secs(15));
	/// assert_eq!(config.ping_config().pong_timeout(), Duration::from_secs(5));
	/// ```
	pub fn with_ping_config(mut self, config: PingPongConfig) -> Self {
		self.ping_config = config;
		self
	}

	/// Get the ping/pong keepalive configuration.
	pub fn ping_config(&self) -> &PingPongConfig {
		&self.ping_config
	}

	/// Create a configuration with no idle timeout (connections never time out)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::connection::ConnectionConfig;
	///
	/// let config = ConnectionConfig::no_timeout();
	/// assert_eq!(config.idle_timeout(), std::time::Duration::MAX);
	/// assert_eq!(config.handshake_timeout(), std::time::Duration::MAX);
	/// ```
	pub fn no_timeout() -> Self {
		Self {
			idle_timeout: Duration::MAX,
			handshake_timeout: Duration::MAX,
			cleanup_interval: Duration::from_secs(30),
			max_connections: None,
			ping_config: PingPongConfig::default(),
		}
	}

	/// Create a strict configuration with short timeouts
	///
	/// - Idle timeout: 30 seconds
	/// - Handshake timeout: 5 seconds
	/// - Cleanup interval: 10 seconds
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::connection::ConnectionConfig;
	/// use std::time::Duration;
	///
	/// let config = ConnectionConfig::strict();
	/// assert_eq!(config.idle_timeout(), Duration::from_secs(30));
	/// assert_eq!(config.handshake_timeout(), Duration::from_secs(5));
	/// assert_eq!(config.cleanup_interval(), Duration::from_secs(10));
	/// ```
	pub fn strict() -> Self {
		Self {
			idle_timeout: Duration::from_secs(30),
			handshake_timeout: Duration::from_secs(5),
			cleanup_interval: Duration::from_secs(10),
			max_connections: None,
			ping_config: PingPongConfig::new(Duration::from_secs(10), Duration::from_secs(5)),
		}
	}

	/// Create a permissive configuration with long timeouts
	///
	/// - Idle timeout: 1 hour
	/// - Handshake timeout: 30 seconds
	/// - Cleanup interval: 60 seconds
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::connection::ConnectionConfig;
	/// use std::time::Duration;
	///
	/// let config = ConnectionConfig::permissive();
	/// assert_eq!(config.idle_timeout(), Duration::from_secs(3600));
	/// assert_eq!(config.handshake_timeout(), Duration::from_secs(30));
	/// assert_eq!(config.cleanup_interval(), Duration::from_secs(60));
	/// ```
	pub fn permissive() -> Self {
		Self {
			idle_timeout: Duration::from_secs(3600),
			handshake_timeout: Duration::from_secs(30),
			cleanup_interval: Duration::from_secs(60),
			max_connections: None,
			ping_config: PingPongConfig::new(Duration::from_secs(60), Duration::from_secs(30)),
		}
	}
}

#[derive(Debug, thiserror::Error)]
pub enum WebSocketError {
	#[error("Connection error")]
	Connection(String),
	#[error("Send failed")]
	Send(String),
	#[error("Receive failed")]
	Receive(String),
	#[error("Protocol error")]
	Protocol(String),
	#[error("Internal error")]
	Internal(String),
	#[error("Connection timed out")]
	Timeout(Duration),
	#[error("Reconnection failed")]
	ReconnectFailed(u32),
	#[error("Invalid binary payload: {0}")]
	BinaryPayload(String),
	#[error("Heartbeat timeout: no pong received within {0:?}")]
	HeartbeatTimeout(Duration),
	#[error("Slow consumer: send timed out after {0:?}")]
	SlowConsumer(Duration),
}

impl WebSocketError {
	/// Returns a sanitized error message safe for client-facing communication.
	///
	/// Internal details (buffer sizes, queue depths, connection state, type names)
	/// are stripped to prevent information leakage.
	pub fn client_message(&self) -> &'static str {
		match self {
			Self::Connection(_) => "Connection error",
			Self::Send(_) => "Failed to send message",
			Self::Receive(_) => "Failed to receive message",
			Self::Protocol(_) => "Protocol error",
			Self::Internal(_) => "Internal server error",
			Self::Timeout(_) => "Connection timed out",
			Self::ReconnectFailed(_) => "Reconnection failed",
			Self::BinaryPayload(_) => "Invalid message format",
			Self::HeartbeatTimeout(_) => "Connection timed out",
			Self::SlowConsumer(_) => "Server overloaded",
		}
	}

	/// Returns the internal detail message for logging purposes.
	///
	/// This MUST NOT be sent to clients as it may contain sensitive
	/// internal state information.
	pub fn internal_detail(&self) -> String {
		match self {
			Self::Connection(msg) => format!("Connection error: {}", msg),
			Self::Send(msg) => format!("Send error: {}", msg),
			Self::Receive(msg) => format!("Receive error: {}", msg),
			Self::Protocol(msg) => format!("Protocol error: {}", msg),
			Self::Internal(msg) => format!("Internal error: {}", msg),
			Self::Timeout(d) => format!("Connection timeout: idle for {:?}", d),
			Self::ReconnectFailed(n) => format!("Reconnection failed after {} attempts", n),
			Self::BinaryPayload(msg) => format!("Invalid binary payload: {}", msg),
			Self::HeartbeatTimeout(d) => format!("Heartbeat timeout: no pong within {:?}", d),
			Self::SlowConsumer(d) => format!("Slow consumer: send timed out after {:?}", d),
		}
	}
}

pub type WebSocketResult<T> = Result<T, WebSocketError>;

/// WebSocket message types
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum Message {
	Text { data: String },
	Binary { data: Vec<u8> },
	Ping,
	Pong,
	Close { code: u16, reason: String },
}

impl Message {
	/// Creates a new text message.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::Message;
	///
	/// let msg = Message::text("Hello, World!".to_string());
	/// match msg {
	///     Message::Text { data } => assert_eq!(data, "Hello, World!"),
	///     _ => panic!("Expected text message"),
	/// }
	/// ```
	pub fn text(data: String) -> Self {
		Self::Text { data }
	}
	/// Creates a new binary message.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::Message;
	///
	/// let data = vec![0x48, 0x65, 0x6c, 0x6c, 0x6f]; // "Hello" in bytes
	/// let msg = Message::binary(data.clone());
	/// match msg {
	///     Message::Binary { data: d } => assert_eq!(d, data),
	///     _ => panic!("Expected binary message"),
	/// }
	/// ```
	pub fn binary(data: Vec<u8>) -> Self {
		Self::Binary { data }
	}
	/// Creates a text message containing JSON-serialized data.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::Message;
	/// use serde::Serialize;
	///
	/// #[derive(Serialize)]
	/// struct User {
	///     name: String,
	///     age: u32,
	/// }
	///
	/// let user = User {
	///     name: "Alice".to_string(),
	///     age: 30,
	/// };
	///
	/// let msg = Message::json(&user).unwrap();
	/// match msg {
	///     Message::Text { data } => {
	///         assert!(data.contains("Alice"));
	///         assert!(data.contains("30"));
	///     },
	///     _ => panic!("Expected text message"),
	/// }
	/// ```
	pub fn json<T: serde::Serialize>(data: &T) -> WebSocketResult<Self> {
		let json =
			serde_json::to_string(data).map_err(|e| WebSocketError::Protocol(e.to_string()))?;
		Ok(Self::text(json))
	}
	/// Parses the message content as JSON into the target type.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::Message;
	/// use serde::Deserialize;
	///
	/// #[derive(Deserialize, Debug, PartialEq)]
	/// struct User {
	///     name: String,
	///     age: u32,
	/// }
	///
	/// let msg = Message::text(r#"{"name":"Bob","age":25}"#.to_string());
	/// let user: User = msg.parse_json().unwrap();
	/// assert_eq!(user.name, "Bob");
	/// assert_eq!(user.age, 25);
	/// ```
	pub fn parse_json<T: serde::de::DeserializeOwned>(&self) -> WebSocketResult<T> {
		match self {
			Message::Text { data } => {
				serde_json::from_str(data).map_err(|e| WebSocketError::Protocol(e.to_string()))
			}
			_ => Err(WebSocketError::Protocol("Not a text message".to_string())),
		}
	}
}

/// WebSocket connection with activity tracking and timeout support
pub struct WebSocketConnection {
	id: String,
	tx: mpsc::UnboundedSender<Message>,
	closed: Arc<RwLock<bool>>,
	/// Subprotocol (negotiated protocol during WebSocket handshake)
	subprotocol: Option<String>,
	/// Timestamp of last activity on this connection
	last_activity: Arc<RwLock<Instant>>,
	/// Connection timeout configuration
	config: ConnectionConfig,
}

impl WebSocketConnection {
	/// Creates a new WebSocket connection with the given ID and sender.
	///
	/// Uses default [`ConnectionConfig`] for timeout settings.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use tokio::sync::mpsc;
	///
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let conn = WebSocketConnection::new("connection_1".to_string(), tx);
	/// assert_eq!(conn.id(), "connection_1");
	/// ```
	pub fn new(id: String, tx: mpsc::UnboundedSender<Message>) -> Self {
		Self {
			id,
			tx,
			closed: Arc::new(RwLock::new(false)),
			subprotocol: None,
			last_activity: Arc::new(RwLock::new(Instant::now())),
			config: ConnectionConfig::default(),
		}
	}

	/// Creates a new WebSocket connection with the given ID, sender, and configuration.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use reinhardt_websockets::connection::ConnectionConfig;
	/// use tokio::sync::mpsc;
	/// use std::time::Duration;
	///
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let config = ConnectionConfig::new()
	///     .with_idle_timeout(Duration::from_secs(60));
	/// let conn = WebSocketConnection::with_config("conn_1".to_string(), tx, config);
	/// assert_eq!(conn.id(), "conn_1");
	/// assert_eq!(conn.config().idle_timeout(), Duration::from_secs(60));
	/// ```
	pub fn with_config(
		id: String,
		tx: mpsc::UnboundedSender<Message>,
		config: ConnectionConfig,
	) -> Self {
		Self {
			id,
			tx,
			closed: Arc::new(RwLock::new(false)),
			subprotocol: None,
			last_activity: Arc::new(RwLock::new(Instant::now())),
			config,
		}
	}

	/// Creates a new WebSocket connection with the given ID, sender, and subprotocol.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use tokio::sync::mpsc;
	///
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let conn = WebSocketConnection::with_subprotocol(
	///     "connection_1".to_string(),
	///     tx,
	///     Some("chat".to_string())
	/// );
	/// assert_eq!(conn.id(), "connection_1");
	/// assert_eq!(conn.subprotocol(), Some("chat"));
	/// ```
	pub fn with_subprotocol(
		id: String,
		tx: mpsc::UnboundedSender<Message>,
		subprotocol: Option<String>,
	) -> Self {
		Self {
			id,
			tx,
			closed: Arc::new(RwLock::new(false)),
			subprotocol,
			last_activity: Arc::new(RwLock::new(Instant::now())),
			config: ConnectionConfig::default(),
		}
	}

	/// Gets the negotiated subprotocol, if any.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use tokio::sync::mpsc;
	///
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let conn = WebSocketConnection::with_subprotocol(
	///     "test".to_string(),
	///     tx,
	///     Some("chat".to_string())
	/// );
	/// assert_eq!(conn.subprotocol(), Some("chat"));
	/// ```
	pub fn subprotocol(&self) -> Option<&str> {
		self.subprotocol.as_deref()
	}

	/// Gets the connection ID.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use tokio::sync::mpsc;
	///
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let conn = WebSocketConnection::new("test_id".to_string(), tx);
	/// assert_eq!(conn.id(), "test_id");
	/// ```
	pub fn id(&self) -> &str {
		&self.id
	}

	/// Gets the connection timeout configuration.
	pub fn config(&self) -> &ConnectionConfig {
		&self.config
	}

	/// Records activity on the connection, resetting the idle timer.
	///
	/// This is called automatically when sending messages, but can also be called
	/// manually to indicate that the connection is still active (e.g., when
	/// receiving messages from the client).
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::WebSocketConnection;
	/// use tokio::sync::mpsc;
	///
	/// # tokio_test::block_on(async {
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let conn = WebSocketConnection::new("test".to_string(), tx);
	///
	/// conn.record_activity().await;
	/// assert!(!conn.is_idle().await);
	/// # });
	/// ```
	pub async fn record_activity(&self) {
		*self.last_activity.write().await = Instant::now();
	}

	/// Returns the duration since the last activity on this connection.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::WebSocketConnection;
	/// use tokio::sync::mpsc;
	/// use std::time::Duration;
	///
	/// # tokio_test::block_on(async {
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let conn = WebSocketConnection::new("test".to_string(), tx);
	///
	/// let idle = conn.idle_duration().await;
	/// assert!(idle < Duration::from_secs(1));
	/// # });
	/// ```
	pub async fn idle_duration(&self) -> Duration {
		self.last_activity.read().await.elapsed()
	}

	/// Checks whether this connection has exceeded its idle timeout.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::WebSocketConnection;
	/// use tokio::sync::mpsc;
	///
	/// # tokio_test::block_on(async {
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let conn = WebSocketConnection::new("test".to_string(), tx);
	///
	/// // A freshly created connection is not idle
	/// assert!(!conn.is_idle().await);
	/// # });
	/// ```
	pub async fn is_idle(&self) -> bool {
		self.idle_duration().await > self.config.idle_timeout
	}

	/// Sends a message through the WebSocket connection.
	///
	/// Records activity on the connection when a message is sent successfully.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use tokio::sync::mpsc;
	///
	/// # tokio_test::block_on(async {
	/// let (tx, mut rx) = mpsc::unbounded_channel();
	/// let conn = WebSocketConnection::new("test".to_string(), tx);
	///
	/// let message = Message::text("Hello".to_string());
	/// conn.send(message).await.unwrap();
	///
	/// let received = rx.recv().await.unwrap();
	/// assert!(matches!(received, Message::Text { .. }));
	/// # });
	/// ```
	pub async fn send(&self, message: Message) -> WebSocketResult<()> {
		if *self.closed.read().await {
			return Err(WebSocketError::Send("Connection closed".to_string()));
		}

		let result = self
			.tx
			.send(message)
			.map_err(|e| WebSocketError::Send(e.to_string()));

		if result.is_ok() {
			self.record_activity().await;
		}

		result
	}
	/// Sends a text message through the WebSocket connection.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use tokio::sync::mpsc;
	///
	/// # tokio_test::block_on(async {
	/// let (tx, mut rx) = mpsc::unbounded_channel();
	/// let conn = WebSocketConnection::new("test".to_string(), tx);
	///
	/// conn.send_text("Hello World".to_string()).await.unwrap();
	///
	/// let received = rx.recv().await.unwrap();
	/// match received {
	///     Message::Text { data } => assert_eq!(data, "Hello World"),
	///     _ => panic!("Expected text message"),
	/// }
	/// # });
	/// ```
	pub async fn send_text(&self, text: String) -> WebSocketResult<()> {
		self.send(Message::text(text)).await
	}
	/// Sends a binary message through the WebSocket connection.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use tokio::sync::mpsc;
	///
	/// # tokio_test::block_on(async {
	/// let (tx, mut rx) = mpsc::unbounded_channel();
	/// let conn = WebSocketConnection::new("test".to_string(), tx);
	///
	/// let binary_data = vec![0x48, 0x65, 0x6c, 0x6c, 0x6f]; // "Hello"
	/// conn.send_binary(binary_data.clone()).await.unwrap();
	///
	/// let received = rx.recv().await.unwrap();
	/// match received {
	///     Message::Binary { data } => assert_eq!(data, binary_data),
	///     _ => panic!("Expected binary message"),
	/// }
	/// # });
	/// ```
	pub async fn send_binary(&self, data: Vec<u8>) -> WebSocketResult<()> {
		self.send(Message::binary(data)).await
	}
	/// Sends a JSON message through the WebSocket connection.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use tokio::sync::mpsc;
	/// use serde::Serialize;
	///
	/// #[derive(Serialize)]
	/// struct User {
	///     name: String,
	///     age: u32,
	/// }
	///
	/// # tokio_test::block_on(async {
	/// let (tx, mut rx) = mpsc::unbounded_channel();
	/// let conn = WebSocketConnection::new("test".to_string(), tx);
	///
	/// let user = User { name: "Alice".to_string(), age: 30 };
	/// conn.send_json(&user).await.unwrap();
	///
	/// let received = rx.recv().await.unwrap();
	/// match received {
	///     Message::Text { data } => assert!(data.contains("Alice")),
	///     _ => panic!("Expected text message"),
	/// }
	/// # });
	/// ```
	pub async fn send_json<T: serde::Serialize>(&self, data: &T) -> WebSocketResult<()> {
		let message = Message::json(data)?;
		self.send(message).await
	}
	/// Closes the WebSocket connection.
	///
	/// The connection is always marked as closed regardless of whether the
	/// close frame could be sent. This ensures resource cleanup even when
	/// the underlying channel is already broken.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use tokio::sync::mpsc;
	///
	/// # tokio_test::block_on(async {
	/// let (tx, mut rx) = mpsc::unbounded_channel();
	/// let conn = WebSocketConnection::new("test".to_string(), tx);
	///
	/// conn.close().await.unwrap();
	/// assert!(conn.is_closed().await);
	/// # });
	/// ```
	pub async fn close(&self) -> WebSocketResult<()> {
		// Mark as closed first to prevent new sends
		*self.closed.write().await = true;

		// Best-effort send of close frame; connection is closed regardless
		self.tx
			.send(Message::Close {
				code: 1000,
				reason: "Normal closure".to_string(),
			})
			.map_err(|e| WebSocketError::Send(e.to_string()))
	}
	/// Closes the connection with a custom close code and reason.
	///
	/// The connection is always marked as closed regardless of whether the
	/// close frame could be sent. This ensures resource cleanup even when
	/// the underlying channel is already broken.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use tokio::sync::mpsc;
	///
	/// # tokio_test::block_on(async {
	/// let (tx, mut rx) = mpsc::unbounded_channel();
	/// let conn = WebSocketConnection::new("test".to_string(), tx);
	///
	/// conn.close_with_reason(1001, "Idle timeout".to_string()).await.unwrap();
	/// assert!(conn.is_closed().await);
	///
	/// let msg = rx.recv().await.unwrap();
	/// match msg {
	///     Message::Close { code, reason } => {
	///         assert_eq!(code, 1001);
	///         assert_eq!(reason, "Idle timeout");
	///     },
	///     _ => panic!("Expected close message"),
	/// }
	/// # });
	/// ```
	pub async fn close_with_reason(&self, code: u16, reason: String) -> WebSocketResult<()> {
		// Mark as closed first to prevent new sends
		*self.closed.write().await = true;

		// Best-effort send of close frame; connection is closed regardless
		self.tx
			.send(Message::Close { code, reason })
			.map_err(|e| WebSocketError::Send(e.to_string()))
	}

	/// Forces the connection closed without sending a close frame.
	///
	/// Use this for abnormal close paths where the underlying transport is
	/// already broken and sending a close frame would fail.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::WebSocketConnection;
	/// use tokio::sync::mpsc;
	///
	/// # tokio_test::block_on(async {
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let conn = WebSocketConnection::new("test".to_string(), tx);
	///
	/// conn.force_close().await;
	/// assert!(conn.is_closed().await);
	/// # });
	/// ```
	pub async fn force_close(&self) {
		*self.closed.write().await = true;
	}

	/// Checks if the WebSocket connection is closed.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::{WebSocketConnection, Message};
	/// use tokio::sync::mpsc;
	///
	/// # tokio_test::block_on(async {
	/// let (tx, _rx) = mpsc::unbounded_channel();
	/// let conn = WebSocketConnection::new("test".to_string(), tx);
	///
	/// assert!(!conn.is_closed().await);
	/// # });
	/// ```
	pub async fn is_closed(&self) -> bool {
		*self.closed.read().await
	}
}

/// Monitors WebSocket connections for idle timeouts and cleans them up.
///
/// The monitor periodically checks all registered connections and closes
/// those that have exceeded their idle timeout. This prevents resource
/// exhaustion from idle connection holding attacks.
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::connection::{ConnectionConfig, ConnectionTimeoutMonitor};
/// use reinhardt_websockets::WebSocketConnection;
/// use tokio::sync::mpsc;
/// use std::sync::Arc;
/// use std::time::Duration;
///
/// # tokio_test::block_on(async {
/// let config = ConnectionConfig::new()
///     .with_idle_timeout(Duration::from_secs(60))
///     .with_cleanup_interval(Duration::from_secs(10));
///
/// let monitor = ConnectionTimeoutMonitor::new(config);
///
/// let (tx, _rx) = mpsc::unbounded_channel();
/// let conn = Arc::new(WebSocketConnection::new("conn_1".to_string(), tx));
/// monitor.register(conn).await.unwrap();
///
/// assert_eq!(monitor.connection_count().await, 1);
/// # });
/// ```
pub struct ConnectionTimeoutMonitor {
	connections: Arc<RwLock<HashMap<String, Arc<WebSocketConnection>>>>,
	config: ConnectionConfig,
}

impl ConnectionTimeoutMonitor {
	/// Creates a new connection timeout monitor with the given configuration.
	pub fn new(config: ConnectionConfig) -> Self {
		Self {
			connections: Arc::new(RwLock::new(HashMap::new())),
			config,
		}
	}

	/// Registers a connection for timeout monitoring.
	///
	/// Returns `Ok(())` if the connection was registered, or `Err` if the
	/// maximum connection limit has been reached.
	pub async fn register(
		&self,
		connection: Arc<WebSocketConnection>,
	) -> Result<(), WebSocketError> {
		let mut connections = self.connections.write().await;

		if let Some(max) = self.config.max_connections
			&& connections.len() >= max
		{
			return Err(WebSocketError::Connection(format!(
				"maximum connection limit reached ({})",
				max
			)));
		}

		connections.insert(connection.id().to_string(), connection);
		Ok(())
	}

	/// Unregisters a connection from timeout monitoring.
	pub async fn unregister(&self, connection_id: &str) {
		self.connections.write().await.remove(connection_id);
	}

	/// Returns the number of currently monitored connections.
	pub async fn connection_count(&self) -> usize {
		self.connections.read().await.len()
	}

	/// Checks all connections and closes those that have exceeded their idle timeout.
	///
	/// Returns the IDs of connections that were closed due to timeout.
	pub async fn check_idle_connections(&self) -> Vec<String> {
		let connections = self.connections.read().await;
		let mut timed_out = Vec::new();

		for (id, conn) in connections.iter() {
			if conn.is_closed().await {
				timed_out.push(id.clone());
				continue;
			}

			let idle_duration = conn.idle_duration().await;
			if idle_duration > self.config.idle_timeout {
				let reason = format!(
					"Idle timeout: connection idle for {}s (limit: {}s)",
					idle_duration.as_secs(),
					self.config.idle_timeout.as_secs()
				);
				// Close with 1001 (Going Away) as per RFC 6455
				let _ = conn.close_with_reason(1001, reason).await;
				timed_out.push(id.clone());
			}
		}

		drop(connections);

		// Remove timed-out connections
		if !timed_out.is_empty() {
			let mut connections = self.connections.write().await;
			for id in &timed_out {
				connections.remove(id);
			}
		}

		timed_out
	}

	/// Gracefully shuts down all monitored connections.
	///
	/// Sends a Close frame (code 1001, "Going Away") to each active connection
	/// and removes it from monitoring. Already-closed connections are silently
	/// removed.
	///
	/// Returns the IDs of all connections that were shut down.
	pub async fn shutdown_all(&self) -> Vec<String> {
		let mut connections = self.connections.write().await;
		let mut shut_down = Vec::with_capacity(connections.len());

		for (id, conn) in connections.drain() {
			if !conn.is_closed().await {
				let _ = conn
					.close_with_reason(1001, "Server shutting down".to_string())
					.await;
			}
			shut_down.push(id);
		}

		shut_down
	}

	/// Starts the background monitoring task.
	///
	/// Returns a [`tokio::task::JoinHandle`] that can be used to abort the monitor.
	/// The monitor runs until the handle is aborted or the process exits.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_websockets::connection::{ConnectionConfig, ConnectionTimeoutMonitor};
	/// use std::time::Duration;
	///
	/// # tokio_test::block_on(async {
	/// let config = ConnectionConfig::new()
	///     .with_cleanup_interval(Duration::from_millis(100));
	/// let monitor = std::sync::Arc::new(ConnectionTimeoutMonitor::new(config));
	///
	/// let handle = monitor.start();
	///
	/// // Monitor is running in background...
	/// tokio::time::sleep(Duration::from_millis(50)).await;
	///
	/// // Stop the monitor
	/// handle.abort();
	/// # });
	/// ```
	pub fn start(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
		let monitor = Arc::clone(self);
		tokio::spawn(async move {
			let mut interval = tokio::time::interval(monitor.config.cleanup_interval);
			loop {
				interval.tick().await;
				monitor.check_idle_connections().await;
			}
		})
	}
}

/// Configuration for heartbeat (ping/pong) monitoring.
///
/// Defines how often pings are sent and how long to wait for a pong
/// response before considering the connection dead.
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::connection::HeartbeatConfig;
/// use std::time::Duration;
///
/// let config = HeartbeatConfig::new(
///     Duration::from_secs(30),
///     Duration::from_secs(10),
/// );
///
/// assert_eq!(config.ping_interval(), Duration::from_secs(30));
/// assert_eq!(config.pong_timeout(), Duration::from_secs(10));
/// ```
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
	/// Interval between outgoing pings
	ping_interval: Duration,
	/// Maximum time to wait for a pong response before closing
	pong_timeout: Duration,
}

impl HeartbeatConfig {
	/// Creates a new heartbeat configuration.
	pub fn new(ping_interval: Duration, pong_timeout: Duration) -> Self {
		Self {
			ping_interval,
			pong_timeout,
		}
	}

	/// Returns the ping interval.
	pub fn ping_interval(&self) -> Duration {
		self.ping_interval
	}

	/// Returns the pong timeout.
	pub fn pong_timeout(&self) -> Duration {
		self.pong_timeout
	}
}

impl Default for HeartbeatConfig {
	fn default() -> Self {
		Self {
			ping_interval: Duration::from_secs(30),
			pong_timeout: Duration::from_secs(10),
		}
	}
}

/// Monitors a WebSocket connection's heartbeat via ping/pong.
///
/// Sends periodic pings and tracks when the last pong was received.
/// When no pong arrives within the configured timeout, the connection
/// is force-closed and the monitor signals a heartbeat failure.
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::connection::{HeartbeatConfig, HeartbeatMonitor};
/// use reinhardt_websockets::WebSocketConnection;
/// use tokio::sync::mpsc;
/// use std::sync::Arc;
/// use std::time::Duration;
///
/// # tokio_test::block_on(async {
/// let (tx, _rx) = mpsc::unbounded_channel();
/// let conn = Arc::new(WebSocketConnection::new("hb_test".to_string(), tx));
/// let config = HeartbeatConfig::default();
///
/// let monitor = HeartbeatMonitor::new(conn, config);
/// assert!(!monitor.is_timed_out().await);
/// # });
/// ```
pub struct HeartbeatMonitor {
	connection: Arc<WebSocketConnection>,
	config: HeartbeatConfig,
	last_pong: Arc<RwLock<Instant>>,
	timed_out: Arc<RwLock<bool>>,
	pong_notify: Arc<tokio::sync::Notify>,
}

impl HeartbeatMonitor {
	/// Creates a new heartbeat monitor for the given connection.
	pub fn new(connection: Arc<WebSocketConnection>, config: HeartbeatConfig) -> Self {
		Self {
			connection,
			config,
			last_pong: Arc::new(RwLock::new(Instant::now())),
			timed_out: Arc::new(RwLock::new(false)),
			pong_notify: Arc::new(tokio::sync::Notify::new()),
		}
	}

	/// Records a pong response, resetting the timeout tracker.
	///
	/// This also wakes up the heartbeat monitor's sleep so it can
	/// proceed immediately instead of waiting for the full pong timeout.
	pub async fn record_pong(&self) {
		*self.last_pong.write().await = Instant::now();
		self.pong_notify.notify_one();
	}

	/// Returns the duration since the last pong was received.
	pub async fn time_since_last_pong(&self) -> Duration {
		self.last_pong.read().await.elapsed()
	}

	/// Returns whether the heartbeat has timed out.
	pub async fn is_timed_out(&self) -> bool {
		*self.timed_out.read().await
	}

	/// Checks whether the pong timeout has been exceeded.
	///
	/// If the timeout is exceeded, the connection is force-closed and
	/// the method returns `true`.
	pub async fn check_heartbeat(&self) -> bool {
		let since_pong = self.time_since_last_pong().await;

		if since_pong > self.config.pong_timeout {
			self.connection.force_close().await;
			*self.timed_out.write().await = true;
			return true;
		}

		false
	}

	/// Sends a ping message through the connection.
	///
	/// Returns `Ok(())` if the ping was sent, or an error if the
	/// connection is already closed.
	pub async fn send_ping(&self) -> WebSocketResult<()> {
		self.connection.send(Message::Ping).await
	}

	/// Returns a reference to the heartbeat configuration.
	pub fn config(&self) -> &HeartbeatConfig {
		&self.config
	}

	/// Returns a reference to the monitored connection.
	pub fn connection(&self) -> &Arc<WebSocketConnection> {
		&self.connection
	}

	/// Starts a background task that periodically sends pings and checks
	/// for pong timeouts.
	///
	/// Returns a [`tokio::task::JoinHandle`] that can be aborted to stop
	/// the monitor. The task ends automatically when a heartbeat timeout
	/// occurs.
	pub fn start(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
		let monitor = Arc::clone(self);
		tokio::spawn(async move {
			let mut interval = tokio::time::interval(monitor.config.ping_interval);
			loop {
				interval.tick().await;

				if monitor.connection.is_closed().await {
					break;
				}

				// Best-effort ping; if send fails, check_heartbeat will catch it
				let _ = monitor.send_ping().await;

				// Wait for pong or timeout, whichever comes first.
				// If pong arrives early, we skip the remaining sleep and
				// proceed to the next ping interval immediately.
				tokio::select! {
					() = tokio::time::sleep(monitor.config.pong_timeout) => {
						// Timeout elapsed without pong notification
						if monitor.check_heartbeat().await {
							break;
						}
					}
					() = monitor.pong_notify.notified() => {
						// Pong received early; no need to wait further
					}
				}
			}
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_message_text() {
		// Arrange
		let text = "Hello".to_string();

		// Act
		let msg = Message::text(text);

		// Assert
		match msg {
			Message::Text { data } => assert_eq!(data, "Hello"),
			_ => panic!("Expected text message"),
		}
	}

	#[rstest]
	fn test_message_json() {
		// Arrange
		#[derive(serde::Serialize)]
		struct TestData {
			value: i32,
		}
		let data = TestData { value: 42 };

		// Act
		let msg = Message::json(&data).unwrap();

		// Assert
		match msg {
			Message::Text { data } => assert!(data.contains("42")),
			_ => panic!("Expected text message"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_connection_send() {
		// Arrange
		let (tx, mut rx) = mpsc::unbounded_channel();
		let conn = WebSocketConnection::new("test".to_string(), tx);

		// Act
		conn.send_text("Hello".to_string()).await.unwrap();

		// Assert
		let received = rx.recv().await.unwrap();
		match received {
			Message::Text { data } => assert_eq!(data, "Hello"),
			_ => panic!("Expected text message"),
		}
	}

	#[rstest]
	fn test_connection_config_default() {
		// Arrange & Act
		let config = ConnectionConfig::new();

		// Assert
		assert_eq!(config.idle_timeout(), Duration::from_secs(300));
		assert_eq!(config.handshake_timeout(), Duration::from_secs(10));
		assert_eq!(config.cleanup_interval(), Duration::from_secs(30));
	}

	#[rstest]
	fn test_connection_config_strict() {
		// Arrange & Act
		let config = ConnectionConfig::strict();

		// Assert
		assert_eq!(config.idle_timeout(), Duration::from_secs(30));
		assert_eq!(config.handshake_timeout(), Duration::from_secs(5));
		assert_eq!(config.cleanup_interval(), Duration::from_secs(10));
	}

	#[rstest]
	fn test_connection_config_permissive() {
		// Arrange & Act
		let config = ConnectionConfig::permissive();

		// Assert
		assert_eq!(config.idle_timeout(), Duration::from_secs(3600));
		assert_eq!(config.handshake_timeout(), Duration::from_secs(30));
		assert_eq!(config.cleanup_interval(), Duration::from_secs(60));
	}

	#[rstest]
	fn test_connection_config_no_timeout() {
		// Arrange & Act
		let config = ConnectionConfig::no_timeout();

		// Assert
		assert_eq!(config.idle_timeout(), Duration::MAX);
		assert_eq!(config.handshake_timeout(), Duration::MAX);
	}

	#[rstest]
	fn test_connection_config_builder() {
		// Arrange & Act
		let config = ConnectionConfig::new()
			.with_idle_timeout(Duration::from_secs(120))
			.with_handshake_timeout(Duration::from_secs(15))
			.with_cleanup_interval(Duration::from_secs(20));

		// Assert
		assert_eq!(config.idle_timeout(), Duration::from_secs(120));
		assert_eq!(config.handshake_timeout(), Duration::from_secs(15));
		assert_eq!(config.cleanup_interval(), Duration::from_secs(20));
	}

	#[rstest]
	#[tokio::test]
	async fn test_connection_with_config() {
		// Arrange
		let config = ConnectionConfig::new().with_idle_timeout(Duration::from_secs(60));
		let (tx, _rx) = mpsc::unbounded_channel();

		// Act
		let conn = WebSocketConnection::with_config("test".to_string(), tx, config);

		// Assert
		assert_eq!(conn.config().idle_timeout(), Duration::from_secs(60));
		assert!(!conn.is_idle().await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_connection_record_activity_resets_idle() {
		// Arrange
		let config = ConnectionConfig::new().with_idle_timeout(Duration::from_millis(50));
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = WebSocketConnection::with_config("test".to_string(), tx, config);

		// Act - wait for connection to become idle
		tokio::time::sleep(Duration::from_millis(60)).await;
		assert!(conn.is_idle().await);

		// Act - record activity to reset idle timer
		conn.record_activity().await;

		// Assert - connection should no longer be idle
		assert!(!conn.is_idle().await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_connection_becomes_idle_after_timeout() {
		// Arrange
		let config = ConnectionConfig::new().with_idle_timeout(Duration::from_millis(50));
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = WebSocketConnection::with_config("test".to_string(), tx, config);

		// Act - wait for connection to exceed idle timeout
		tokio::time::sleep(Duration::from_millis(60)).await;

		// Assert
		assert!(conn.is_idle().await);
		assert!(conn.idle_duration().await >= Duration::from_millis(50));
	}

	#[rstest]
	#[tokio::test]
	async fn test_send_resets_activity() {
		// Arrange
		let config = ConnectionConfig::new().with_idle_timeout(Duration::from_millis(100));
		let (tx, mut _rx) = mpsc::unbounded_channel();
		let conn = WebSocketConnection::with_config("test".to_string(), tx, config);

		// Act - wait a bit then send
		tokio::time::sleep(Duration::from_millis(50)).await;
		conn.send_text("ping".to_string()).await.unwrap();

		// Assert - activity should be recent
		assert!(conn.idle_duration().await < Duration::from_millis(30));
		assert!(!conn.is_idle().await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_close_with_reason() {
		// Arrange
		let (tx, mut rx) = mpsc::unbounded_channel();
		let conn = WebSocketConnection::new("test".to_string(), tx);

		// Act
		conn.close_with_reason(1001, "Idle timeout".to_string())
			.await
			.unwrap();

		// Assert
		assert!(conn.is_closed().await);
		let msg = rx.recv().await.unwrap();
		match msg {
			Message::Close { code, reason } => {
				assert_eq!(code, 1001);
				assert_eq!(reason, "Idle timeout");
			}
			_ => panic!("Expected close message"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_timeout_monitor_register_and_count() {
		// Arrange
		let config = ConnectionConfig::new();
		let monitor = ConnectionTimeoutMonitor::new(config);
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("conn_1".to_string(), tx));

		// Act
		monitor.register(conn).await.unwrap();

		// Assert
		assert_eq!(monitor.connection_count().await, 1);
	}

	#[rstest]
	#[tokio::test]
	async fn test_timeout_monitor_unregister() {
		// Arrange
		let config = ConnectionConfig::new();
		let monitor = ConnectionTimeoutMonitor::new(config);
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("conn_1".to_string(), tx));
		monitor.register(conn).await.unwrap();

		// Act
		monitor.unregister("conn_1").await;

		// Assert
		assert_eq!(monitor.connection_count().await, 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_timeout_monitor_closes_idle_connections() {
		// Arrange
		let config = ConnectionConfig::new().with_idle_timeout(Duration::from_millis(50));
		let monitor = ConnectionTimeoutMonitor::new(config);

		let (tx1, mut rx1) = mpsc::unbounded_channel();
		let conn1 = Arc::new(WebSocketConnection::with_config(
			"idle_conn".to_string(),
			tx1,
			ConnectionConfig::new().with_idle_timeout(Duration::from_millis(50)),
		));

		let (tx2, _rx2) = mpsc::unbounded_channel();
		let conn2 = Arc::new(WebSocketConnection::with_config(
			"active_conn".to_string(),
			tx2,
			ConnectionConfig::new().with_idle_timeout(Duration::from_secs(300)),
		));

		monitor.register(conn1).await.unwrap();
		monitor.register(conn2.clone()).await.unwrap();

		// Act - wait for idle timeout to expire
		tokio::time::sleep(Duration::from_millis(60)).await;
		// Keep active connection alive
		conn2.record_activity().await;

		let timed_out = monitor.check_idle_connections().await;

		// Assert
		assert_eq!(timed_out.len(), 1);
		assert_eq!(timed_out[0], "idle_conn");
		assert_eq!(monitor.connection_count().await, 1);

		// Verify the idle connection received a close message
		let msg = rx1.recv().await.unwrap();
		match msg {
			Message::Close { code, reason } => {
				assert_eq!(code, 1001);
				assert!(reason.contains("Idle timeout"));
			}
			_ => panic!("Expected close message for idle connection"),
		}
	}

	#[rstest]
	#[tokio::test]
	async fn test_timeout_monitor_removes_already_closed_connections() {
		// Arrange
		let config = ConnectionConfig::new();
		let monitor = ConnectionTimeoutMonitor::new(config);
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("conn_1".to_string(), tx));
		conn.close().await.unwrap();
		monitor.register(conn).await.unwrap();

		// Act
		let timed_out = monitor.check_idle_connections().await;

		// Assert
		assert_eq!(timed_out.len(), 1);
		assert_eq!(timed_out[0], "conn_1");
		assert_eq!(monitor.connection_count().await, 0);
	}

	#[rstest]
	#[tokio::test]
	async fn test_timeout_monitor_background_task() {
		// Arrange
		let config = ConnectionConfig::new()
			.with_idle_timeout(Duration::from_millis(30))
			.with_cleanup_interval(Duration::from_millis(20));
		let monitor = Arc::new(ConnectionTimeoutMonitor::new(config));

		let (tx, mut rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::with_config(
			"bg_conn".to_string(),
			tx,
			ConnectionConfig::new().with_idle_timeout(Duration::from_millis(30)),
		));
		monitor.register(conn).await.unwrap();

		// Act - start background monitor
		let handle = monitor.start();

		// Wait for the monitor to detect and close the idle connection
		tokio::time::sleep(Duration::from_millis(120)).await;

		// Assert
		assert_eq!(monitor.connection_count().await, 0);

		// Verify close message was sent
		let msg = rx.recv().await.unwrap();
		assert!(matches!(msg, Message::Close { .. }));

		// Cleanup
		handle.abort();
	}

	#[rstest]
	fn test_ping_pong_config_default() {
		// Arrange & Act
		let config = PingPongConfig::default();

		// Assert
		assert_eq!(config.ping_interval(), Duration::from_secs(30));
		assert_eq!(config.pong_timeout(), Duration::from_secs(10));
	}

	#[rstest]
	fn test_ping_pong_config_custom() {
		// Arrange & Act
		let config = PingPongConfig::new(Duration::from_secs(15), Duration::from_secs(5));

		// Assert
		assert_eq!(config.ping_interval(), Duration::from_secs(15));
		assert_eq!(config.pong_timeout(), Duration::from_secs(5));
	}

	#[rstest]
	fn test_ping_pong_config_builder() {
		// Arrange & Act
		let config = PingPongConfig::default()
			.with_ping_interval(Duration::from_secs(60))
			.with_pong_timeout(Duration::from_secs(20));

		// Assert
		assert_eq!(config.ping_interval(), Duration::from_secs(60));
		assert_eq!(config.pong_timeout(), Duration::from_secs(20));
	}

	#[rstest]
	fn test_connection_config_has_default_ping_config() {
		// Arrange & Act
		let config = ConnectionConfig::new();

		// Assert
		assert_eq!(
			config.ping_config().ping_interval(),
			Duration::from_secs(30)
		);
		assert_eq!(config.ping_config().pong_timeout(), Duration::from_secs(10));
	}

	#[rstest]
	fn test_connection_config_with_custom_ping_config() {
		// Arrange
		let ping_config = PingPongConfig::new(Duration::from_secs(15), Duration::from_secs(5));

		// Act
		let config = ConnectionConfig::new().with_ping_config(ping_config);

		// Assert
		assert_eq!(
			config.ping_config().ping_interval(),
			Duration::from_secs(15)
		);
		assert_eq!(config.ping_config().pong_timeout(), Duration::from_secs(5));
	}

	#[rstest]
	fn test_strict_config_has_aggressive_ping() {
		// Arrange & Act
		let config = ConnectionConfig::strict();

		// Assert
		assert_eq!(
			config.ping_config().ping_interval(),
			Duration::from_secs(10)
		);
		assert_eq!(config.ping_config().pong_timeout(), Duration::from_secs(5));
	}

	#[rstest]
	fn test_permissive_config_has_relaxed_ping() {
		// Arrange & Act
		let config = ConnectionConfig::permissive();

		// Assert
		assert_eq!(
			config.ping_config().ping_interval(),
			Duration::from_secs(60)
		);
		assert_eq!(config.ping_config().pong_timeout(), Duration::from_secs(30));
	}

	#[rstest]
	#[tokio::test]
	async fn test_timeout_monitor_rejects_when_max_connections_reached() {
		// Arrange
		let config = ConnectionConfig::new().with_max_connections(Some(1));
		let monitor = ConnectionTimeoutMonitor::new(config);

		let (tx1, _rx1) = mpsc::unbounded_channel();
		let conn1 = Arc::new(WebSocketConnection::new("conn_1".to_string(), tx1));

		let (tx2, _rx2) = mpsc::unbounded_channel();
		let conn2 = Arc::new(WebSocketConnection::new("conn_2".to_string(), tx2));

		// Act
		monitor.register(conn1).await.unwrap();
		let result = monitor.register(conn2).await;

		// Assert
		assert!(result.is_err());
		assert_eq!(monitor.connection_count().await, 1);
	}

	#[rstest]
	#[tokio::test]
	async fn test_force_close_marks_connection_closed() {
		// Arrange
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = WebSocketConnection::new("test".to_string(), tx);

		// Act
		conn.force_close().await;

		// Assert
		assert!(conn.is_closed().await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_close_marks_closed_even_when_channel_dropped() {
		// Arrange
		let (tx, rx) = mpsc::unbounded_channel();
		let conn = WebSocketConnection::new("test".to_string(), tx);

		// Drop receiver to simulate broken channel
		drop(rx);

		// Act - close should still mark the connection as closed
		let result = conn.close().await;

		// Assert
		assert!(result.is_err()); // send fails because receiver is dropped
		assert!(conn.is_closed().await); // but connection is still marked closed
	}

	#[rstest]
	#[tokio::test]
	async fn test_close_with_reason_marks_closed_even_when_channel_dropped() {
		// Arrange
		let (tx, rx) = mpsc::unbounded_channel();
		let conn = WebSocketConnection::new("test".to_string(), tx);

		// Drop receiver to simulate broken channel
		drop(rx);

		// Act
		let result = conn
			.close_with_reason(1006, "Abnormal close".to_string())
			.await;

		// Assert
		assert!(result.is_err());
		assert!(conn.is_closed().await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_send_after_force_close_returns_error() {
		// Arrange
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = WebSocketConnection::new("test".to_string(), tx);
		conn.force_close().await;

		// Act
		let result = conn.send_text("should fail".to_string()).await;

		// Assert
		assert!(result.is_err());
		assert!(matches!(result.unwrap_err(), WebSocketError::Send(_)));
	}

	#[rstest]
	fn test_heartbeat_config_default() {
		// Arrange & Act
		let config = HeartbeatConfig::default();

		// Assert
		assert_eq!(config.ping_interval(), Duration::from_secs(30));
		assert_eq!(config.pong_timeout(), Duration::from_secs(10));
	}

	#[rstest]
	fn test_heartbeat_config_custom() {
		// Arrange & Act
		let config = HeartbeatConfig::new(Duration::from_secs(15), Duration::from_secs(5));

		// Assert
		assert_eq!(config.ping_interval(), Duration::from_secs(15));
		assert_eq!(config.pong_timeout(), Duration::from_secs(5));
	}

	#[rstest]
	#[tokio::test]
	async fn test_heartbeat_monitor_initial_state() {
		// Arrange
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("hb_test".to_string(), tx));
		let config = HeartbeatConfig::default();

		// Act
		let monitor = HeartbeatMonitor::new(conn, config);

		// Assert
		assert!(!monitor.is_timed_out().await);
		assert!(monitor.time_since_last_pong().await < Duration::from_secs(1));
	}

	#[rstest]
	#[tokio::test]
	async fn test_heartbeat_monitor_record_pong_resets_timer() {
		// Arrange
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("hb_pong".to_string(), tx));
		let config = HeartbeatConfig::new(Duration::from_millis(50), Duration::from_millis(30));
		let monitor = HeartbeatMonitor::new(conn, config);

		// Act - wait then record pong
		tokio::time::sleep(Duration::from_millis(20)).await;
		monitor.record_pong().await;

		// Assert
		assert!(monitor.time_since_last_pong().await < Duration::from_millis(10));
	}

	#[rstest]
	#[tokio::test]
	async fn test_heartbeat_monitor_timeout_closes_connection() {
		// Arrange
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("hb_timeout".to_string(), tx));
		let config = HeartbeatConfig::new(Duration::from_millis(50), Duration::from_millis(30));
		let monitor = HeartbeatMonitor::new(conn.clone(), config);

		// Act - wait past the pong timeout
		tokio::time::sleep(Duration::from_millis(40)).await;
		let timed_out = monitor.check_heartbeat().await;

		// Assert
		assert!(timed_out);
		assert!(monitor.is_timed_out().await);
		assert!(conn.is_closed().await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_heartbeat_monitor_no_timeout_when_pong_received() {
		// Arrange
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("hb_ok".to_string(), tx));
		let config = HeartbeatConfig::new(Duration::from_millis(100), Duration::from_millis(50));
		let monitor = HeartbeatMonitor::new(conn.clone(), config);

		// Act - record pong within timeout window
		tokio::time::sleep(Duration::from_millis(20)).await;
		monitor.record_pong().await;
		let timed_out = monitor.check_heartbeat().await;

		// Assert
		assert!(!timed_out);
		assert!(!monitor.is_timed_out().await);
		assert!(!conn.is_closed().await);
	}

	#[rstest]
	#[tokio::test]
	async fn test_heartbeat_monitor_send_ping() {
		// Arrange
		let (tx, mut rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("hb_ping".to_string(), tx));
		let config = HeartbeatConfig::default();
		let monitor = HeartbeatMonitor::new(conn, config);

		// Act
		monitor.send_ping().await.unwrap();

		// Assert
		let msg = rx.recv().await.unwrap();
		assert!(matches!(msg, Message::Ping));
	}

	#[rstest]
	#[tokio::test]
	async fn test_heartbeat_monitor_early_pong_skips_full_sleep() {
		// Arrange
		let (tx, _rx) = mpsc::unbounded_channel();
		let conn = Arc::new(WebSocketConnection::new("hb_early".to_string(), tx));
		// Use a long pong_timeout to make the test obvious
		let config = HeartbeatConfig {
			ping_interval: Duration::from_secs(60),
			pong_timeout: Duration::from_secs(10),
		};
		let monitor = Arc::new(HeartbeatMonitor::new(conn, config));

		// Act: simulate pong arriving after a short delay
		let monitor_clone = Arc::clone(&monitor);
		tokio::spawn(async move {
			tokio::time::sleep(Duration::from_millis(50)).await;
			monitor_clone.record_pong().await;
		});

		// Send ping and wait for pong or timeout via select!
		let _ = monitor.send_ping().await;
		let start = Instant::now();

		tokio::select! {
			() = tokio::time::sleep(monitor.config.pong_timeout) => {
				panic!("Should not reach full timeout");
			}
			() = monitor.pong_notify.notified() => {
				// Pong received early
			}
		}

		// Assert: should complete well before the 10s pong_timeout
		let elapsed = start.elapsed();
		assert!(
			elapsed < Duration::from_secs(2),
			"Expected early wakeup but elapsed {:?}",
			elapsed
		);
	}

	#[rstest]
	fn test_websocket_error_binary_payload_variant() {
		// Arrange & Act
		let err = WebSocketError::BinaryPayload("invalid data".to_string());

		// Assert
		assert_eq!(err.to_string(), "Invalid binary payload: invalid data");
	}

	#[rstest]
	fn test_websocket_error_heartbeat_timeout_variant() {
		// Arrange & Act
		let err = WebSocketError::HeartbeatTimeout(Duration::from_secs(10));

		// Assert
		assert_eq!(
			err.to_string(),
			"Heartbeat timeout: no pong received within 10s"
		);
	}

	#[rstest]
	fn test_websocket_error_slow_consumer_variant() {
		// Arrange & Act
		let err = WebSocketError::SlowConsumer(Duration::from_secs(5));

		// Assert
		assert_eq!(err.to_string(), "Slow consumer: send timed out after 5s");
	}
}
