#[cfg(feature = "websocket")]
use futures_util::{SinkExt, StreamExt};
#[cfg(feature = "websocket")]
use std::collections::HashMap;
#[cfg(feature = "websocket")]
use std::net::SocketAddr;
#[cfg(feature = "websocket")]
use std::sync::Arc;
#[cfg(feature = "websocket")]
use tokio::net::{TcpListener, TcpStream};
#[cfg(feature = "websocket")]
use tokio::sync::{Mutex, RwLock, broadcast};
#[cfg(feature = "websocket")]
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite::Message};

#[cfg(feature = "websocket")]
use crate::shutdown::ShutdownCoordinator;

/// Type alias for WebSocket stream writer
#[cfg(feature = "websocket")]
type WsWriter = futures_util::stream::SplitSink<WebSocketStream<TcpStream>, Message>;

/// Client connection information
#[cfg(feature = "websocket")]
struct Client {
	#[allow(dead_code)]
	addr: SocketAddr,
	sender: Arc<Mutex<WsWriter>>,
}

/// Broadcast manager for WebSocket connections
#[cfg(feature = "websocket")]
#[derive(Clone)]
pub struct BroadcastManager {
	clients: Arc<RwLock<HashMap<SocketAddr, Arc<Client>>>>,
	broadcast_tx: broadcast::Sender<String>,
}

#[cfg(feature = "websocket")]
impl BroadcastManager {
	/// Create a new broadcast manager with the specified capacity
	///
	/// The capacity determines how many messages can be queued before older messages are dropped.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_server::server::BroadcastManager;
	///
	/// let manager = BroadcastManager::new(100);
	/// ```
	pub fn new(capacity: usize) -> Self {
		let (broadcast_tx, _) = broadcast::channel(capacity);
		Self {
			clients: Arc::new(RwLock::new(HashMap::new())),
			broadcast_tx,
		}
	}

	/// Register a new client connection
	async fn register_client(&self, addr: SocketAddr, sender: WsWriter) {
		let client = Arc::new(Client {
			addr,
			sender: Arc::new(Mutex::new(sender)),
		});
		self.clients.write().await.insert(addr, client);
	}

	/// Unregister a client connection
	async fn unregister_client(&self, addr: &SocketAddr) {
		self.clients.write().await.remove(addr);
	}

	/// Broadcast a message to all connected clients
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_server::server::BroadcastManager;
	///
	/// # async fn example() {
	/// let manager = BroadcastManager::new(100);
	/// manager.broadcast("Hello, everyone!".to_string()).await;
	/// # }
	/// ```
	pub async fn broadcast(&self, message: String) {
		// Send through broadcast channel (ignore if no receivers)
		let _ = self.broadcast_tx.send(message.clone());
	}

	/// Get the number of connected clients
	///
	/// # Examples
	///
	/// ```no_run
	/// use reinhardt_server::server::BroadcastManager;
	///
	/// # async fn example() {
	/// let manager = BroadcastManager::new(100);
	/// let count = manager.client_count().await;
	/// println!("Connected clients: {}", count);
	/// # }
	/// ```
	pub async fn client_count(&self) -> usize {
		self.clients.read().await.len()
	}

	/// Subscribe to broadcast messages
	fn subscribe(&self) -> broadcast::Receiver<String> {
		self.broadcast_tx.subscribe()
	}
}

/// Trait for handling WebSocket messages
#[cfg(feature = "websocket")]
#[async_trait::async_trait]
pub trait WebSocketHandler: Send + Sync {
	/// Handle an incoming WebSocket message
	async fn handle_message(&self, message: String) -> Result<String, String>;

	/// Called when a WebSocket connection is established
	async fn on_connect(&self) {}

	/// Called when a WebSocket connection is closed
	async fn on_disconnect(&self) {}
}

/// WebSocket server with broadcast support
#[cfg(feature = "websocket")]
pub struct WebSocketServer {
	handler: Arc<dyn WebSocketHandler>,
	pub broadcast_manager: Option<BroadcastManager>,
}

#[cfg(feature = "websocket")]
impl WebSocketServer {
	/// Create a new WebSocket server with the given handler
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_server::server::WebSocketServer;
	/// use reinhardt_server::server::WebSocketHandler;
	///
	/// struct EchoHandler;
	///
	/// #[async_trait::async_trait]
	/// impl WebSocketHandler for EchoHandler {
	///     async fn handle_message(&self, message: String) -> Result<String, String> {
	///         Ok(format!("Echo: {}", message))
	///     }
	/// }
	///
	/// let server = WebSocketServer::new(EchoHandler);
	/// ```
	pub fn new<H: WebSocketHandler + 'static>(handler: H) -> Self {
		Self {
			handler: Arc::new(handler),
			broadcast_manager: None,
		}
	}

	/// Enable broadcast support with the specified capacity
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_server::server::WebSocketServer;
	/// use reinhardt_server::server::WebSocketHandler;
	///
	/// struct EchoHandler;
	///
	/// #[async_trait::async_trait]
	/// impl WebSocketHandler for EchoHandler {
	///     async fn handle_message(&self, message: String) -> Result<String, String> {
	///         Ok(format!("Echo: {}", message))
	///     }
	/// }
	///
	/// let server = WebSocketServer::new(EchoHandler)
	///     .with_broadcast(100);
	/// ```
	pub fn with_broadcast(mut self, capacity: usize) -> Self {
		self.broadcast_manager = Some(BroadcastManager::new(capacity));
		self
	}

	/// Create a new WebSocketServer from an existing `Arc<dyn WebSocketHandler>`
	///
	/// This is useful when you already have a handler wrapped in Arc,
	/// such as in test fixtures or dependency injection scenarios.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_server::server::WebSocketServer;
	/// use reinhardt_server::server::WebSocketHandler;
	///
	/// struct EchoHandler;
	///
	/// #[async_trait::async_trait]
	/// impl WebSocketHandler for EchoHandler {
	///     async fn handle_message(&self, message: String) -> Result<String, String> {
	///         Ok(format!("Echo: {}", message))
	///     }
	/// }
	///
	/// let handler: Arc<dyn WebSocketHandler> = Arc::new(EchoHandler);
	/// let server = WebSocketServer::from_arc(handler);
	/// ```
	pub fn from_arc(handler: Arc<dyn WebSocketHandler>) -> Self {
		Self {
			handler,
			broadcast_manager: None,
		}
	}

	/// Get a reference to the broadcast manager if enabled
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_server::server::WebSocketServer;
	/// use reinhardt_server::server::WebSocketHandler;
	///
	/// struct EchoHandler;
	///
	/// #[async_trait::async_trait]
	/// impl WebSocketHandler for EchoHandler {
	///     async fn handle_message(&self, message: String) -> Result<String, String> {
	///         Ok(format!("Echo: {}", message))
	///     }
	/// }
	///
	/// # async fn example() {
	/// let server = WebSocketServer::new(EchoHandler)
	///     .with_broadcast(100);
	///
	/// if let Some(manager) = server.broadcast_manager() {
	///     manager.broadcast("Hello!".to_string()).await;
	/// }
	/// # }
	/// ```
	pub fn broadcast_manager(&self) -> Option<&BroadcastManager> {
		self.broadcast_manager.as_ref()
	}
	/// Start the WebSocket server and listen on the given address
	///
	/// This method starts the server and begins accepting WebSocket connections.
	/// It runs indefinitely until an error occurs.
	///
	/// # Examples
	///
	/// ```no_run
	/// use std::net::SocketAddr;
	/// use reinhardt_server::server::WebSocketServer;
	/// use reinhardt_server::server::WebSocketHandler;
	///
	/// struct EchoHandler;
	///
	/// #[async_trait::async_trait]
	/// impl WebSocketHandler for EchoHandler {
	///     async fn handle_message(&self, message: String) -> Result<String, String> {
	///         Ok(message)
	///     }
	/// }
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let server = WebSocketServer::new(EchoHandler);
	/// let addr: SocketAddr = "127.0.0.1:9001".parse()?;
	/// server.listen(addr).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn listen(self, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
		let listener = TcpListener::bind(addr).await?;
		println!("WebSocket server listening on ws://{}", addr);

		let broadcast_manager = self.broadcast_manager.clone();

		loop {
			let (stream, peer_addr) = listener.accept().await?;
			let handler = self.handler.clone();
			let manager = broadcast_manager.clone();

			tokio::spawn(async move {
				if let Err(e) = Self::handle_connection(stream, handler, peer_addr, manager).await {
					eprintln!("Error handling WebSocket connection: {:?}", e);
				}
			});
		}
	}

	/// Start the WebSocket server with graceful shutdown support
	///
	/// This method starts the server and listens for connections until a shutdown
	/// signal is received via the coordinator. It supports graceful shutdown.
	///
	/// # Examples
	///
	/// ```no_run
	/// use std::net::SocketAddr;
	/// use std::time::Duration;
	/// use reinhardt_server::server::{WebSocketServer, ShutdownCoordinator};
	/// use reinhardt_server::server::WebSocketHandler;
	///
	/// struct EchoHandler;
	///
	/// #[async_trait::async_trait]
	/// impl WebSocketHandler for EchoHandler {
	///     async fn handle_message(&self, message: String) -> Result<String, String> {
	///         Ok(message)
	///     }
	/// }
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let server = WebSocketServer::new(EchoHandler);
	/// let addr: SocketAddr = "127.0.0.1:9001".parse()?;
	/// let coordinator = ShutdownCoordinator::new(Duration::from_secs(30));
	/// server.listen_with_shutdown(addr, coordinator).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn listen_with_shutdown(
		self,
		addr: SocketAddr,
		coordinator: ShutdownCoordinator,
	) -> Result<(), Box<dyn std::error::Error>> {
		let listener = TcpListener::bind(addr).await?;
		println!("WebSocket server listening on ws://{}", addr);

		let broadcast_manager = self.broadcast_manager.clone();
		let mut shutdown_rx = coordinator.subscribe();

		loop {
			tokio::select! {
				result = listener.accept() => {
					let (stream, peer_addr) = result?;
					let handler = self.handler.clone();
					let manager = broadcast_manager.clone();
					let mut conn_shutdown = coordinator.subscribe();

					tokio::spawn(async move {
						tokio::select! {
							result = Self::handle_connection(stream, handler, peer_addr, manager) => {
								if let Err(e) = result {
									eprintln!("Error handling WebSocket connection: {:?}", e);
								}
							}
							_ = conn_shutdown.recv() => {
								// Connection interrupted by shutdown
							}
						}
					});
				}
				_ = shutdown_rx.recv() => {
					println!("Shutdown signal received, stopping WebSocket server...");
					break;
				}
			}
		}

		coordinator.notify_shutdown_complete();
		Ok(())
	}

	/// Handle a single WebSocket connection
	///
	/// This is an internal method used by the server to process individual WebSocket connections.
	/// It manages the WebSocket handshake, message handling, and connection lifecycle.
	///
	/// # Examples
	///
	/// ```no_run
	/// use std::sync::Arc;
	/// use std::net::SocketAddr;
	/// use tokio::net::TcpStream;
	/// use reinhardt_server::server::WebSocketServer;
	/// use reinhardt_server::server::WebSocketHandler;
	///
	/// struct EchoHandler;
	///
	/// #[async_trait::async_trait]
	/// impl WebSocketHandler for EchoHandler {
	///     async fn handle_message(&self, message: String) -> Result<String, String> {
	///         Ok(message)
	///     }
	/// }
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let addr: SocketAddr = "127.0.0.1:9001".parse()?;
	/// let stream = TcpStream::connect(addr).await?;
	/// WebSocketServer::handle_connection(stream, Arc::new(EchoHandler), addr, None).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn handle_connection(
		stream: TcpStream,
		handler: Arc<dyn WebSocketHandler>,
		peer_addr: SocketAddr,
		broadcast_manager: Option<BroadcastManager>,
	) -> Result<(), Box<dyn std::error::Error>> {
		#[cfg(debug_assertions)]
		eprintln!("[ws:debug] new connection from: {}", peer_addr);

		let ws_stream = accept_async(stream).await?;
		let (write, mut read) = ws_stream.split();

		// Register client if broadcast is enabled, or keep write for direct use
		let mut direct_write = if let Some(ref manager) = broadcast_manager {
			manager.register_client(peer_addr, write).await;
			None
		} else {
			Some(write)
		};

		let use_broadcast = broadcast_manager.is_some();

		// Notify handler of connection
		handler.on_connect().await;

		// Subscribe to broadcast messages if enabled
		let mut broadcast_rx = broadcast_manager.as_ref().map(|m| m.subscribe());

		// Handle messages
		loop {
			tokio::select! {
				// Handle incoming messages from client
				message = read.next() => {
					match message {
						Some(Ok(msg)) => {
							if msg.is_text() {
								let text = msg.to_text()?;
								#[cfg(debug_assertions)]
								eprintln!("[ws:trace] text message from peer ({} bytes)", text.len());

								// Process message through handler
								match handler.handle_message(text.to_string()).await {
									Ok(response) => {
										if use_broadcast {
											// Broadcast mode: send through broadcast manager
											if let Some(ref manager) = broadcast_manager
												&& let Some(clients) = manager.clients.read().await.get(&peer_addr) {
													let mut sender = clients.sender.lock().await;
													sender.send(Message::Text(response.into())).await?;
												}
										} else if let Some(ref mut w) = direct_write {
											// Normal mode: send directly
											w.send(Message::Text(response.into())).await?;
										}
									}
									Err(error) => {
										if use_broadcast {
											if let Some(ref manager) = broadcast_manager
												&& let Some(clients) = manager.clients.read().await.get(&peer_addr) {
													let mut sender = clients.sender.lock().await;
													sender.send(Message::Text(error.into())).await?;
												}
										} else if let Some(ref mut w) = direct_write {
											w.send(Message::Text(error.into())).await?;
										}
									}
								}
							} else if msg.is_binary() {
								let data = msg.into_data();
								#[cfg(debug_assertions)]
								eprintln!("[ws:trace] binary message from peer ({} bytes)", data.len());

								// Echo binary messages directly back to client
								if use_broadcast {
									// Broadcast mode: send through broadcast manager
									if let Some(ref manager) = broadcast_manager
										&& let Some(clients) = manager.clients.read().await.get(&peer_addr) {
											let mut sender = clients.sender.lock().await;
											sender.send(Message::Binary(data)).await?;
										}
								} else if let Some(ref mut w) = direct_write {
									// Normal mode: send directly
									w.send(Message::Binary(data)).await?;
								}
							} else if msg.is_close() {
								#[cfg(debug_assertions)]
								eprintln!("[ws:debug] connection closing for peer");
								// Send close frame response before closing
								if use_broadcast {
									if let Some(ref manager) = broadcast_manager
										&& let Some(clients) = manager.clients.read().await.get(&peer_addr) {
											let mut sender = clients.sender.lock().await;
											let _ = sender.send(Message::Close(None)).await;
											// Flush to ensure close frame is sent before disconnecting
											let _ = sender.flush().await;
										}
								} else if let Some(ref mut w) = direct_write {
									let _ = w.send(Message::Close(None)).await;
									// Flush to ensure close frame is sent before disconnecting
									let _ = w.flush().await;
								}
								// Brief delay to ensure client receives close frame (RFC 6455 compliance)
								tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
								break;
							}
						}
						Some(Err(e)) => {
							eprintln!("WebSocket error: {}", e);
							break;
						}
						None => break,
					}
				}
				// Handle broadcast messages
				broadcast_msg = async {
					match &mut broadcast_rx {
						Some(rx) => rx.recv().await.ok(),
						None => std::future::pending().await,
					}
				} => {
					if let Some(msg) = broadcast_msg
						&& let Some(ref manager) = broadcast_manager
							&& let Some(client) = manager.clients.read().await.get(&peer_addr) {
								let mut sender = client.sender.lock().await;
								if let Err(e) = sender.send(Message::Text(msg.into())).await {
									eprintln!("Failed to send broadcast message: {}", e);
									break;
								}
							}
				}
			}
		}

		// Unregister client if broadcast is enabled
		if let Some(ref manager) = broadcast_manager {
			manager.unregister_client(&peer_addr).await;
		}

		// Notify handler of disconnection
		handler.on_disconnect().await;

		#[cfg(debug_assertions)]
		eprintln!("[ws:debug] connection closed for peer");
		Ok(())
	}
}

/// Helper function to create and run a WebSocket server
///
/// This is a convenience function that creates a `WebSocketServer` and starts listening.
///
/// # Examples
///
/// ```no_run
/// use std::net::SocketAddr;
/// use reinhardt_server::server::serve_websocket;
/// use reinhardt_server::server::WebSocketHandler;
///
/// struct ChatHandler;
///
/// #[async_trait::async_trait]
/// impl WebSocketHandler for ChatHandler {
///     async fn handle_message(&self, message: String) -> Result<String, String> {
///         Ok(format!("Received: {}", message))
///     }
///
///     async fn on_connect(&self) {
///         println!("Client connected");
///     }
///
///     async fn on_disconnect(&self) {
///         println!("Client disconnected");
///     }
/// }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let addr: SocketAddr = "127.0.0.1:9001".parse()?;
/// serve_websocket(addr, ChatHandler).await?;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "websocket")]
pub async fn serve_websocket<H: WebSocketHandler + 'static>(
	addr: SocketAddr,
	handler: H,
) -> Result<(), Box<dyn std::error::Error>> {
	let server = WebSocketServer::new(handler);
	server.listen(addr).await
}

#[cfg(all(test, feature = "websocket"))]
mod tests {
	use super::*;

	struct EchoHandler;

	#[async_trait::async_trait]
	impl WebSocketHandler for EchoHandler {
		async fn handle_message(&self, message: String) -> Result<String, String> {
			Ok(format!("Echo: {}", message))
		}
	}

	#[tokio::test]
	async fn test_websocket_server_creation() {
		let _server = WebSocketServer::new(EchoHandler);
	}

	#[tokio::test]
	async fn test_websocket_server_with_broadcast() {
		let _server = WebSocketServer::new(EchoHandler).with_broadcast(100);
	}

	#[tokio::test]
	async fn test_broadcast_manager_creation() {
		let manager = BroadcastManager::new(50);
		assert_eq!(manager.client_count().await, 0);
	}

	#[tokio::test]
	async fn test_broadcast_manager_broadcast() {
		let manager = BroadcastManager::new(50);

		// Subscribe before broadcasting
		let mut rx = manager.subscribe();

		// Broadcast a message
		manager.broadcast("Hello!".to_string()).await;

		// Receive the message
		let received = rx.recv().await.unwrap();
		assert_eq!(received, "Hello!");
	}

	#[tokio::test]
	async fn test_broadcast_manager_multiple_subscribers() {
		let manager = BroadcastManager::new(50);

		// Subscribe multiple receivers
		let mut rx1 = manager.subscribe();
		let mut rx2 = manager.subscribe();
		let mut rx3 = manager.subscribe();

		// Broadcast a message
		manager.broadcast("Broadcast message".to_string()).await;

		// All receivers should get the message
		assert_eq!(rx1.recv().await.unwrap(), "Broadcast message");
		assert_eq!(rx2.recv().await.unwrap(), "Broadcast message");
		assert_eq!(rx3.recv().await.unwrap(), "Broadcast message");
	}
}
