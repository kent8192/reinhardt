//! Hyper Upgrade protocol helpers.

use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio_tungstenite::WebSocketStream;
use tungstenite::handshake::server::create_response;
use tungstenite::http::{HeaderMap, Method, Request, Response, StatusCode, Uri, Version, header};
use tungstenite::protocol::{CloseFrame, Role};
use tungstenite::{Message as TungsteniteMessage, protocol::frame::coding::CloseCode};

#[cfg(feature = "di")]
use reinhardt_di::InjectionContext;

use crate::{
	ConsumerContext, Message, WebSocketConnection, WebSocketConsumer, WebSocketError,
	WebSocketResult, default_websocket_config,
};

static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(1);

/// Validates a WebSocket handshake and creates its `101` response.
pub fn create_upgrade_response(
	method: &Method,
	uri: &Uri,
	version: Version,
	headers: &HeaderMap,
) -> Result<Response<()>, StatusCode> {
	if method != Method::GET {
		return Err(StatusCode::METHOD_NOT_ALLOWED);
	}
	if !headers.contains_key(header::UPGRADE) && !headers.contains_key(header::CONNECTION) {
		return Err(StatusCode::UPGRADE_REQUIRED);
	}
	if !headers
		.get(header::SEC_WEBSOCKET_KEY)
		.is_some_and(valid_websocket_key)
	{
		return Err(StatusCode::BAD_REQUEST);
	}

	let mut request = Request::builder()
		.method(method.clone())
		.uri(uri.clone())
		.version(version)
		.body(())
		.map_err(|_| StatusCode::BAD_REQUEST)?;
	*request.headers_mut() = headers.clone();
	create_response(&request).map_err(|_| StatusCode::BAD_REQUEST)
}

fn valid_websocket_key(value: &tungstenite::http::HeaderValue) -> bool {
	let bytes = value.as_bytes();
	bytes.len() == 24
		&& bytes[..21]
			.iter()
			.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/'))
		&& matches!(bytes[21], b'A' | b'Q' | b'g' | b'w')
		&& bytes[22..] == *b"=="
}

/// Drives one consumer over an already Tokio-compatible upgraded stream.
#[cfg(feature = "di")]
pub async fn serve_upgraded_consumer<S>(
	stream: S,
	consumer: Box<dyn WebSocketConsumer>,
	headers: HeaderMap,
	metadata: HashMap<String, String>,
	di_context: Arc<InjectionContext>,
) -> WebSocketResult<()>
where
	S: AsyncRead + AsyncWrite + Unpin,
{
	let mut socket =
		WebSocketStream::from_raw_socket(stream, Role::Server, Some(default_websocket_config()))
			.await;
	let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel();
	let connection_id = NEXT_CONNECTION_ID.fetch_add(1, Ordering::Relaxed);
	let connection = Arc::new(WebSocketConnection::new(
		format!("http-upgrade-{connection_id}"),
		outbound_tx,
	));
	let mut context = ConsumerContext::with_di_context(connection, di_context);
	for (name, value) in headers {
		if let (Some(name), Ok(value)) = (name, value.to_str()) {
			context = context.with_header(name.as_str().to_string(), value.to_string());
		}
	}
	for (key, value) in metadata {
		context = context.with_metadata(key, value);
	}

	if let Err(error) = consumer.on_connect(&mut context).await {
		close_internal_error(&mut socket).await;
		let _ = consumer.on_disconnect(&mut context).await;
		return Err(error);
	}

	let result = loop {
		tokio::select! {
			incoming = socket.next() => {
				let Some(incoming) = incoming else {
					break Ok(());
				};
				let incoming = match incoming {
					Ok(message) => message,
					Err(error) => break Err(WebSocketError::Receive(error.to_string())),
				};
				let is_close = incoming.is_close();
				let Some(message) = from_tungstenite(incoming) else {
					continue;
				};
				if let Err(error) = consumer.on_message(&mut context, message).await {
					close_internal_error(&mut socket).await;
					break Err(error);
				}
				if is_close {
					while let Ok(message) = outbound_rx.try_recv() {
						if socket.send(into_tungstenite(message)).await.is_err() {
							break;
						}
					}
					let _ = socket.flush().await;
					break Ok(());
				}
			}
			outgoing = outbound_rx.recv() => {
				let Some(outgoing) = outgoing else {
					break Ok(());
				};
				let is_close = matches!(outgoing, Message::Close { .. });
				if let Err(error) = socket.send(into_tungstenite(outgoing)).await {
					break Err(WebSocketError::Send(error.to_string()));
				}
				if is_close {
					break Ok(());
				}
			}
		}
	};

	if let Err(error) = consumer.on_disconnect(&mut context).await {
		close_internal_error(&mut socket).await;
		return Err(error);
	}
	result
}

fn from_tungstenite(message: TungsteniteMessage) -> Option<Message> {
	match message {
		TungsteniteMessage::Text(data) => Some(Message::Text {
			data: data.to_string(),
		}),
		TungsteniteMessage::Binary(data) => Some(Message::Binary {
			data: data.to_vec(),
		}),
		TungsteniteMessage::Ping(_) => Some(Message::Ping),
		TungsteniteMessage::Pong(_) => Some(Message::Pong),
		TungsteniteMessage::Close(frame) => Some(Message::Close {
			code: frame.as_ref().map_or(1005, |frame| frame.code.into()),
			reason: frame.map_or_else(String::new, |frame| frame.reason.to_string()),
		}),
		TungsteniteMessage::Frame(_) => None,
	}
}

fn into_tungstenite(message: Message) -> TungsteniteMessage {
	match message {
		Message::Text { data } => TungsteniteMessage::Text(data.into()),
		Message::Binary { data } => TungsteniteMessage::Binary(data.into()),
		Message::Ping => TungsteniteMessage::Ping(Vec::new().into()),
		Message::Pong => TungsteniteMessage::Pong(Vec::new().into()),
		Message::Close { code, reason } => TungsteniteMessage::Close(Some(CloseFrame {
			code: code.into(),
			reason: reason.into(),
		})),
	}
}

async fn close_internal_error<S>(socket: &mut WebSocketStream<S>)
where
	S: AsyncRead + AsyncWrite + Unpin,
{
	let _ = socket
		.send(TungsteniteMessage::Close(Some(CloseFrame {
			code: CloseCode::Error,
			reason: "Internal server error".into(),
		})))
		.await;
}

#[cfg(all(test, feature = "di"))]
mod tests {
	use super::*;
	use async_trait::async_trait;
	use reinhardt_di::SingletonScope;
	use tungstenite::http::HeaderValue;

	struct EchoConsumer {
		expected_context: Arc<InjectionContext>,
	}

	#[async_trait]
	impl WebSocketConsumer for EchoConsumer {
		async fn on_connect(&self, context: &mut ConsumerContext) -> WebSocketResult<()> {
			assert!(Arc::ptr_eq(
				context.di_context().unwrap(),
				&self.expected_context
			));
			assert_eq!(context.cookie_header(), Some("session=test"));
			assert_eq!(
				context.get_metadata("path").map(String::as_str),
				Some("/ws")
			);
			Ok(())
		}

		async fn on_message(
			&self,
			context: &mut ConsumerContext,
			message: Message,
		) -> WebSocketResult<()> {
			if let Message::Text { data } = message {
				context.connection.send_text(data).await?;
			}
			Ok(())
		}

		async fn on_disconnect(&self, _context: &mut ConsumerContext) -> WebSocketResult<()> {
			Ok(())
		}
	}

	struct FailingConsumer;

	#[async_trait]
	impl WebSocketConsumer for FailingConsumer {
		async fn on_connect(&self, _context: &mut ConsumerContext) -> WebSocketResult<()> {
			Ok(())
		}

		async fn on_message(
			&self,
			_context: &mut ConsumerContext,
			_message: Message,
		) -> WebSocketResult<()> {
			Err(WebSocketError::Internal("expected failure".to_string()))
		}

		async fn on_disconnect(&self, _context: &mut ConsumerContext) -> WebSocketResult<()> {
			Ok(())
		}
	}

	fn valid_headers() -> HeaderMap {
		HeaderMap::from_iter([
			(header::CONNECTION, HeaderValue::from_static("Upgrade")),
			(header::UPGRADE, HeaderValue::from_static("websocket")),
			(
				header::SEC_WEBSOCKET_VERSION,
				HeaderValue::from_static("13"),
			),
			(
				header::SEC_WEBSOCKET_KEY,
				HeaderValue::from_static("dGhlIHNhbXBsZSBub25jZQ=="),
			),
		])
	}

	#[test]
	fn handshake_statuses_are_distinct() {
		let uri = Uri::from_static("/ws");
		let valid = valid_headers();
		assert_eq!(
			create_upgrade_response(&Method::POST, &uri, Version::HTTP_11, &valid).unwrap_err(),
			StatusCode::METHOD_NOT_ALLOWED
		);
		assert_eq!(
			create_upgrade_response(&Method::GET, &uri, Version::HTTP_11, &HeaderMap::new())
				.unwrap_err(),
			StatusCode::UPGRADE_REQUIRED
		);
		let mut malformed = valid.clone();
		malformed.insert(
			header::SEC_WEBSOCKET_KEY,
			HeaderValue::from_static("not-a-valid-key"),
		);
		assert_eq!(
			create_upgrade_response(&Method::GET, &uri, Version::HTTP_11, &malformed).unwrap_err(),
			StatusCode::BAD_REQUEST
		);
		assert_eq!(
			create_upgrade_response(&Method::GET, &uri, Version::HTTP_11, &valid)
				.unwrap()
				.status(),
			StatusCode::SWITCHING_PROTOCOLS
		);
	}

	#[tokio::test]
	async fn upgraded_consumer_echoes_and_receives_context() {
		let (client_io, server_io) = tokio::io::duplex(4096);
		let di_context =
			Arc::new(InjectionContext::builder(Arc::new(SingletonScope::new())).build());
		let consumer = Box::new(EchoConsumer {
			expected_context: Arc::clone(&di_context),
		});
		let headers =
			HeaderMap::from_iter([(header::COOKIE, HeaderValue::from_static("session=test"))]);
		let metadata = HashMap::from([("path".to_string(), "/ws".to_string())]);

		let server = serve_upgraded_consumer(server_io, consumer, headers, metadata, di_context);
		let client = async move {
			let mut socket = WebSocketStream::from_raw_socket(client_io, Role::Client, None).await;
			socket
				.send(TungsteniteMessage::Text("echo".into()))
				.await
				.unwrap();
			assert_eq!(
				socket.next().await.unwrap().unwrap(),
				TungsteniteMessage::Text("echo".into())
			);
			socket.close(None).await.unwrap();
			assert!(matches!(
				socket.next().await.unwrap().unwrap(),
				TungsteniteMessage::Close(_)
			));
		};

		let (server_result, ()) = tokio::join!(server, client);
		server_result.unwrap();
	}

	#[tokio::test]
	async fn consumer_failure_closes_with_1011() {
		let (client_io, server_io) = tokio::io::duplex(4096);
		let di_context =
			Arc::new(InjectionContext::builder(Arc::new(SingletonScope::new())).build());
		let server = serve_upgraded_consumer(
			server_io,
			Box::new(FailingConsumer),
			HeaderMap::new(),
			HashMap::new(),
			di_context,
		);
		let client = async move {
			let mut socket = WebSocketStream::from_raw_socket(client_io, Role::Client, None).await;
			socket
				.send(TungsteniteMessage::Text("fail".into()))
				.await
				.unwrap();
			assert!(matches!(
				socket.next().await.unwrap().unwrap(),
				TungsteniteMessage::Close(Some(frame)) if frame.code == CloseCode::Error
			));
		};

		let (server_result, ()) = tokio::join!(server, client);
		assert!(matches!(server_result, Err(WebSocketError::Internal(_))));
	}
}
