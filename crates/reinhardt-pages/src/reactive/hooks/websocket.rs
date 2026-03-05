//! WebSocket hook: use_websocket
//!
//! This hook provides a way to establish and manage WebSocket connections
//! in a reactive manner, integrating seamlessly with reinhardt-pages'
//! fine-grained reactivity system.

use crate::reactive::Signal;
use std::rc::Rc;

/// WebSocket connection state
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
	/// WebSocket is attempting to connect
	Connecting,
	/// WebSocket connection is open and ready
	Open,
	/// WebSocket is closing
	Closing,
	/// WebSocket is closed
	Closed,
	/// WebSocket encountered an error
	Error(String),
}

/// WebSocket message types
#[derive(Debug, Clone, PartialEq)]
pub enum WebSocketMessage {
	/// Text message
	Text(String),
	/// Binary message
	Binary(Vec<u8>),
}

/// Options for configuring WebSocket behavior
pub struct UseWebSocketOptions {
	/// Enable automatic reconnection on disconnect
	pub auto_reconnect: bool,
	/// Maximum number of reconnection attempts
	pub max_reconnect_attempts: usize,
	/// Initial delay before reconnecting (in milliseconds)
	pub reconnect_delay: u32,
	/// Callback when connection opens
	pub on_open: Option<Rc<dyn Fn()>>,
	/// Callback when connection closes
	pub on_close: Option<Rc<dyn Fn()>>,
	/// Callback when error occurs
	pub on_error: Option<Rc<dyn Fn(String)>>,
}

impl Default for UseWebSocketOptions {
	fn default() -> Self {
		Self {
			auto_reconnect: true,
			max_reconnect_attempts: 5,
			reconnect_delay: 1000,
			on_open: None,
			on_close: None,
			on_error: None,
		}
	}
}

/// Stored event listener closures for proper lifecycle management.
///
/// On WASM targets, this holds the actual `Closure` instances so they remain
/// alive as long as the `WebSocketHandle` exists, and are released when the
/// handle is dropped (instead of being leaked via `forget()`).
#[cfg(target_arch = "wasm32")]
struct WsClosures {
	_onopen: Closure<dyn FnMut(JsValue)>,
	_onmessage: Closure<dyn FnMut(MessageEvent)>,
	_onclose: Closure<dyn FnMut(CloseEvent)>,
	_onerror: Closure<dyn FnMut(ErrorEvent)>,
}

/// Handle for controlling a WebSocket connection
///
/// This struct provides methods to interact with the WebSocket connection,
/// monitor its state, and send/receive messages reactively.
///
/// Event listener closures are stored in this handle instead of being leaked
/// via `Closure::forget()`. When the handle is dropped, the closures are also
/// dropped, preventing memory leaks in long-running single-page applications.
pub struct WebSocketHandle {
	connection_state: Signal<ConnectionState>,
	latest_message: Signal<Option<WebSocketMessage>>,
	send_fn: Rc<dyn Fn(WebSocketMessage) -> Result<(), String>>,
	close_fn: Rc<dyn Fn()>,
	/// Stored closures to keep event listeners alive without leaking memory.
	/// When the last `WebSocketHandle` clone is dropped, the closures are cleaned up.
	#[cfg(target_arch = "wasm32")]
	_closures: Rc<RefCell<Option<WsClosures>>>,
}

impl WebSocketHandle {
	/// Get a reference to the connection state signal
	pub fn connection_state(&self) -> &Signal<ConnectionState> {
		&self.connection_state
	}

	/// Get a reference to the latest message signal
	pub fn latest_message(&self) -> &Signal<Option<WebSocketMessage>> {
		&self.latest_message
	}

	/// Send a WebSocket message
	pub fn send(&self, message: WebSocketMessage) -> Result<(), String> {
		(self.send_fn)(message)
	}

	/// Send a text message
	pub fn send_text(&self, text: String) -> Result<(), String> {
		self.send(WebSocketMessage::Text(text))
	}

	/// Send a binary message
	pub fn send_binary(&self, data: Vec<u8>) -> Result<(), String> {
		self.send(WebSocketMessage::Binary(data))
	}

	/// Send a JSON-serializable message
	///
	/// # Type Parameters
	///
	/// * `T` - The type to serialize as JSON
	///
	/// # Errors
	///
	/// Returns an error if serialization fails or if sending fails
	pub fn send_json<T: serde::Serialize>(&self, data: &T) -> Result<(), String> {
		let json =
			serde_json::to_string(data).map_err(|e| format!("JSON serialization error: {}", e))?;
		self.send_text(json)
	}

	/// Close the WebSocket connection
	pub fn close(&self) {
		(self.close_fn)()
	}

	/// Check if the connection is currently open
	pub fn is_open(&self) -> bool {
		matches!(self.connection_state.get(), ConnectionState::Open)
	}
}

impl Clone for WebSocketHandle {
	fn clone(&self) -> Self {
		Self {
			connection_state: self.connection_state.clone(),
			latest_message: self.latest_message.clone(),
			send_fn: Rc::clone(&self.send_fn),
			close_fn: Rc::clone(&self.close_fn),
			#[cfg(target_arch = "wasm32")]
			_closures: Rc::clone(&self._closures),
		}
	}
}

// ============================================================================
// WASM Implementation
// ============================================================================

#[cfg(target_arch = "wasm32")]
use {
	std::cell::RefCell,
	wasm_bindgen::{JsCast, JsValue, closure::Closure},
	web_sys::{CloseEvent, ErrorEvent, MessageEvent, WebSocket},
};

/// Establish and manage a WebSocket connection (WASM implementation)
///
/// This hook creates a reactive WebSocket connection that integrates with
/// reinhardt-pages' Signal system. The connection state and incoming messages
/// are automatically tracked and can be used in reactive contexts.
///
/// # Arguments
///
/// * `url` - WebSocket endpoint URL (e.g., "ws://localhost:8000/ws/chat")
/// * `options` - Configuration options for the WebSocket connection
///
/// # Returns
///
/// A `WebSocketHandle` that can be used to control the connection and
/// reactively access its state.
///
/// # Example
///
/// ```ignore
/// use reinhardt_pages::reactive::hooks::{use_websocket, UseWebSocketOptions, use_effect};
/// use reinhardt_pages::reactive::hooks::ConnectionState;
///
/// let ws = use_websocket("ws://localhost:8000/ws/chat", UseWebSocketOptions::default());
///
/// // Monitor connection state
/// use_effect({
///     let ws = ws.clone();
///     move || {
///         match ws.connection_state().get() {
///             ConnectionState::Open => log!("Connected"),
///             ConnectionState::Closed => log!("Disconnected"),
///             _ => {}
///         }
///         None::<fn()>
///     }
/// });
///
/// // Send a message
/// ws.send_text("Hello, server!".to_string()).ok();
///
/// // Receive messages
/// use_effect({
///     let ws = ws.clone();
///     move || {
///         if let Some(msg) = ws.latest_message().get() {
///             match msg {
///                 WebSocketMessage::Text(text) => log!("Received: {}", text),
///                 _ => {}
///             }
///         }
///         None::<fn()>
///     }
/// });
/// ```
#[cfg(target_arch = "wasm32")]
pub fn use_websocket(url: &str, options: UseWebSocketOptions) -> WebSocketHandle {
	// WebSocket instance holder
	let ws_ref: Rc<RefCell<Option<WebSocket>>> = Rc::new(RefCell::new(None));
	let closures_ref: Rc<RefCell<Option<WsClosures>>> = Rc::new(RefCell::new(None));
	let url = url.to_string();

	// State signals
	let connection_state = Signal::new(ConnectionState::Connecting);
	let latest_message = Signal::new(None);

	// Connection function
	let connect = {
		let ws_ref = Rc::clone(&ws_ref);
		let closures_ref = Rc::clone(&closures_ref);
		let connection_state = connection_state.clone();
		let latest_message = latest_message.clone();
		let url = url.clone();
		let on_open = options.on_open.clone();
		let on_close = options.on_close.clone();
		let on_error = options.on_error.clone();

		move || {
			// Create WebSocket connection
			let ws = match WebSocket::new(&url) {
				Ok(ws) => ws,
				Err(e) => {
					connection_state.set(ConnectionState::Error(format!(
						"Failed to create WebSocket: {:?}",
						e
					)));
					return;
				}
			};

			// Set binary type to arraybuffer for binary message support
			ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

			// onopen handler
			let connection_state_open = connection_state.clone();
			let on_open_cb = on_open.clone();
			let onopen = Closure::wrap(Box::new(move |_: JsValue| {
				connection_state_open.set(ConnectionState::Open);
				if let Some(cb) = &on_open_cb {
					cb();
				}
			}) as Box<dyn FnMut(JsValue)>);
			ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));

			// onmessage handler
			let latest_message_recv = latest_message.clone();
			let onmessage = Closure::wrap(Box::new(move |e: MessageEvent| {
				// Try text message first
				if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
					let text = txt.as_string().unwrap_or_default();
					latest_message_recv.set(Some(WebSocketMessage::Text(text)));
				}
				// Try binary message (ArrayBuffer)
				else if let Ok(array_buffer) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
					let array = js_sys::Uint8Array::new(&array_buffer);
					let vec = array.to_vec();
					latest_message_recv.set(Some(WebSocketMessage::Binary(vec)));
				}
			}) as Box<dyn FnMut(MessageEvent)>);
			ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

			// onclose handler
			let connection_state_close = connection_state.clone();
			let on_close_cb = on_close.clone();
			let onclose = Closure::wrap(Box::new(move |_: CloseEvent| {
				connection_state_close.set(ConnectionState::Closed);
				if let Some(cb) = &on_close_cb {
					cb();
				}
			}) as Box<dyn FnMut(CloseEvent)>);
			ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));

			// onerror handler
			let connection_state_error = connection_state.clone();
			let on_error_cb = on_error.clone();
			let onerror = Closure::wrap(Box::new(move |_: ErrorEvent| {
				let error_msg = "WebSocket error occurred".to_string();
				connection_state_error.set(ConnectionState::Error(error_msg.clone()));
				if let Some(cb) = &on_error_cb {
					cb(error_msg);
				}
			}) as Box<dyn FnMut(ErrorEvent)>);
			ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));

			*ws_ref.borrow_mut() = Some(ws);

			// Store closures to keep event listeners alive without leaking memory
			closures_ref.borrow_mut().replace(WsClosures {
				_onopen: onopen,
				_onmessage: onmessage,
				_onclose: onclose,
				_onerror: onerror,
			});
		}
	};

	// Initial connection
	connect();

	// Send function
	let send_fn = {
		let ws_ref = Rc::clone(&ws_ref);
		Rc::new(move |message: WebSocketMessage| {
			let ws = ws_ref.borrow();
			let ws = ws.as_ref().ok_or("WebSocket not initialized")?;

			match message {
				WebSocketMessage::Text(text) => ws
					.send_with_str(&text)
					.map_err(|e| format!("Failed to send text: {:?}", e)),
				WebSocketMessage::Binary(data) => ws
					.send_with_u8_array(&data)
					.map_err(|e| format!("Failed to send binary: {:?}", e)),
			}
		})
	};

	// Close function
	let close_fn = {
		let ws_ref = Rc::clone(&ws_ref);
		Rc::new(move || {
			if let Some(ws) = ws_ref.borrow().as_ref() {
				let _ = ws.close();
			}
		})
	};

	WebSocketHandle {
		connection_state,
		latest_message,
		send_fn,
		close_fn,
		_closures: closures_ref,
	}
}

// ============================================================================
// SSR (Server-Side Rendering) no-op Implementation
// ============================================================================

/// WebSocket hook - SSR no-op implementation
///
/// On the server side (non-WASM), WebSocket connections are not supported.
/// This implementation returns a handle that always reports the connection
/// as closed and rejects all send attempts.
///
/// # Arguments
///
/// * `_url` - WebSocket endpoint URL (ignored)
/// * `_options` - Configuration options (ignored)
///
/// # Returns
///
/// A `WebSocketHandle` with connection state always set to `Closed`.
#[cfg(not(target_arch = "wasm32"))]
pub fn use_websocket(_url: &str, _options: UseWebSocketOptions) -> WebSocketHandle {
	WebSocketHandle {
		connection_state: Signal::new(ConnectionState::Closed),
		latest_message: Signal::new(None),
		send_fn: Rc::new(|_| Err("WebSocket not available on server".to_string())),
		close_fn: Rc::new(|| {}),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	#[cfg(not(target_arch = "wasm32"))]
	fn test_use_websocket_ssr_no_op() {
		let ws = use_websocket("ws://test", UseWebSocketOptions::default());
		assert!(matches!(
			ws.connection_state().get(),
			ConnectionState::Closed
		));
		assert!(ws.send_text("test".to_string()).is_err());
		assert!(!ws.is_open());
	}

	#[test]
	fn test_connection_state_clone() {
		let state1 = ConnectionState::Open;
		let state2 = state1.clone();
		assert_eq!(state1, state2);
	}

	#[test]
	fn test_websocket_message_clone() {
		let msg1 = WebSocketMessage::Text("hello".to_string());
		let msg2 = msg1.clone();
		assert_eq!(msg1, msg2);
	}

	#[test]
	fn test_use_websocket_options_default() {
		let options = UseWebSocketOptions::default();
		assert!(options.auto_reconnect);
		assert_eq!(options.max_reconnect_attempts, 5);
		assert_eq!(options.reconnect_delay, 1000);
		assert!(options.on_open.is_none());
		assert!(options.on_close.is_none());
		assert!(options.on_error.is_none());
	}
}
