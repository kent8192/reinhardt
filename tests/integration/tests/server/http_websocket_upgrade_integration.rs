use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use hyper::{Method, StatusCode, header};
use hyper_util::rt::TokioIo;
use reinhardt_di::{DiError, InjectionContext, SingletonScope};
use reinhardt_http::{Handler, Request, Response};
use reinhardt_server::server::http::HttpUpgradeContext;
use reinhardt_server::{HttpServer, ShutdownCoordinator};
use reinhardt_urls::routers::ServerRouter;
use reinhardt_websockets::{
	ConsumerBuildError, ConsumerContext, Message, WebSocketConsumer, WebSocketResult,
	create_upgrade_response, serve_upgraded_consumer,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::Notify;
use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;

struct EchoConsumer {
	disconnect_entered: Arc<Notify>,
	release_disconnect: Arc<Notify>,
	block_disconnect: AtomicBool,
	expected_di_context: Arc<InjectionContext>,
}

#[async_trait]
impl WebSocketConsumer for EchoConsumer {
	async fn on_connect(&self, context: &mut ConsumerContext) -> WebSocketResult<()> {
		assert!(Arc::ptr_eq(
			context.di_context().unwrap(),
			&self.expected_di_context
		));
		assert_eq!(
			context.get_metadata("path").map(String::as_str),
			Some("/ws/room/42")
		);
		assert_eq!(
			context.get_metadata("room_id").map(String::as_str),
			Some("42")
		);
		assert!(context.get_header("host").is_some());
		Ok(())
	}

	async fn on_message(
		&self,
		context: &mut ConsumerContext,
		message: Message,
	) -> WebSocketResult<()> {
		if let Message::Text { data } = message {
			if data == "fail" {
				self.block_disconnect.store(false, Ordering::Release);
				return Err(reinhardt_websockets::WebSocketError::Internal(
					"expected test failure".to_string(),
				));
			}
			context.connection.send_text(data).await?;
		}
		Ok(())
	}

	async fn on_disconnect(&self, _context: &mut ConsumerContext) -> WebSocketResult<()> {
		if !self.block_disconnect.load(Ordering::Acquire) {
			return Ok(());
		}
		self.disconnect_entered.notify_one();
		self.release_disconnect.notified().await;
		Ok(())
	}
}

struct UpgradeHandler {
	di_context: Arc<InjectionContext>,
	disconnect_entered: Arc<Notify>,
	release_disconnect: Arc<Notify>,
}

impl UpgradeHandler {
	async fn build_consumer(
		&self,
		request: &Request,
	) -> Result<Box<dyn WebSocketConsumer>, ConsumerBuildError> {
		if request.headers.contains_key("x-fail-consumer") {
			return Err(ConsumerBuildError::new(
				concat!(module_path!(), "::EchoConsumer"),
				"MissingDependency",
				DiError::DependencyNotRegistered {
					type_name: "MissingDependency".to_string(),
				},
			));
		}

		Ok(Box::new(EchoConsumer {
			disconnect_entered: Arc::clone(&self.disconnect_entered),
			release_disconnect: Arc::clone(&self.release_disconnect),
			block_disconnect: AtomicBool::new(true),
			expected_di_context: Arc::clone(&self.di_context),
		}))
	}
}

#[async_trait]
impl Handler for UpgradeHandler {
	async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
		let upgrade_response = match create_upgrade_response(
			&request.method,
			&request.uri,
			request.version,
			&request.headers,
		) {
			Ok(response) => response,
			Err(status) => return Ok(Response::new(status)),
		};
		let consumer = match self.build_consumer(&request).await {
			Ok(consumer) => consumer,
			Err(_) => return Ok(Response::internal_server_error()),
		};
		let Some(upgrade) = request.extensions.get::<HttpUpgradeContext>() else {
			return Ok(Response::internal_server_error());
		};
		let Some(on_upgrade) = upgrade.take_on_upgrade() else {
			return Ok(Response::internal_server_error());
		};
		let headers = request.headers.clone();
		let mut metadata = HashMap::from([("path".to_string(), request.uri.path().to_string())]);
		metadata.extend(
			request
				.path_params
				.iter()
				.map(|(key, value)| (key.clone(), value.to_string())),
		);
		let di_context = Arc::clone(&self.di_context);
		let task = Box::pin(async move {
			let Ok(stream) = on_upgrade.await else {
				return;
			};
			let _ = serve_upgraded_consumer(
				TokioIo::new(stream),
				consumer,
				headers,
				metadata,
				di_context,
			)
			.await;
		});
		if upgrade.spawn(task).is_err() {
			return Ok(Response::internal_server_error());
		}

		let mut response = Response::new(*upgrade_response.status());
		response.headers = upgrade_response.headers().clone();
		Ok(response)
	}
}

struct NormalHttpHandler;

#[async_trait]
impl Handler for NormalHttpHandler {
	async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
		Ok(Response::ok().with_body("normal-http"))
	}
}

#[tokio::test]
async fn http_websocket_upgrade_uses_same_prebound_listener_and_drains_upgrade_tasks() {
	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
	let address = listener.local_addr().unwrap();
	let coordinator = ShutdownCoordinator::new(Duration::from_secs(2));
	let disconnect_entered = Arc::new(Notify::new());
	let release_disconnect = Arc::new(Notify::new());
	let di_context = Arc::new(InjectionContext::builder(Arc::new(SingletonScope::new())).build());
	let upgrade_handler = UpgradeHandler {
		di_context: Arc::clone(&di_context),
		disconnect_entered: Arc::clone(&disconnect_entered),
		release_disconnect: Arc::clone(&release_disconnect),
	};
	let router = ServerRouter::new()
		.handler("/", NormalHttpHandler)
		.handler("/ws/room/{room_id}", upgrade_handler);
	let server_coordinator = coordinator.clone();
	let server_task = tokio::spawn(async move {
		HttpServer::new(router)
			.with_di_context(di_context)
			.listen_on_with_shutdown(listener, server_coordinator)
			.await
			.unwrap();
	});

	let client = reqwest::Client::new();
	let http_url = format!("http://{address}/");
	let response = client.get(&http_url).send().await.unwrap();
	assert_eq!(response.status(), StatusCode::OK);
	assert_eq!(response.text().await.unwrap(), "normal-http");

	let ws_url = format!("ws://{address}/ws/room/42");
	let (mut socket, response) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();
	assert_eq!(response.status(), StatusCode::SWITCHING_PROTOCOLS);
	socket
		.send(TungsteniteMessage::Text("same-port-echo".into()))
		.await
		.unwrap();
	assert_eq!(
		socket.next().await.unwrap().unwrap(),
		TungsteniteMessage::Text("same-port-echo".into())
	);
	let (mut failing_socket, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();
	failing_socket
		.send(TungsteniteMessage::Text("fail".into()))
		.await
		.unwrap();
	let close = failing_socket.next().await.unwrap().unwrap();
	assert!(matches!(
		close,
		TungsteniteMessage::Close(Some(frame))
			if frame.code == tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::Error
	));

	let valid_headers = [
		(header::CONNECTION.as_str(), "Upgrade"),
		(header::UPGRADE.as_str(), "websocket"),
		(header::SEC_WEBSOCKET_VERSION.as_str(), "13"),
		(
			header::SEC_WEBSOCKET_KEY.as_str(),
			"dGhlIHNhbXBsZSBub25jZQ==",
		),
	];
	let status_cases = [
		(
			Method::POST,
			valid_headers.as_slice(),
			false,
			StatusCode::METHOD_NOT_ALLOWED,
		),
		(Method::GET, &[][..], false, StatusCode::UPGRADE_REQUIRED),
		(
			Method::GET,
			&valid_headers[..3],
			false,
			StatusCode::BAD_REQUEST,
		),
		(
			Method::GET,
			valid_headers.as_slice(),
			true,
			StatusCode::INTERNAL_SERVER_ERROR,
		),
	];
	for (method, headers, fail_consumer, expected) in status_cases {
		let mut request = client.request(method, format!("http://{address}/ws/room/42"));
		for (name, value) in headers {
			request = request.header(*name, *value);
		}
		if fail_consumer {
			request = request.header("x-fail-consumer", "1");
		}
		assert_eq!(request.send().await.unwrap().status(), expected);
	}

	socket.close(None).await.unwrap();
	disconnect_entered.notified().await;
	coordinator.shutdown();
	tokio::time::sleep(Duration::from_millis(50)).await;
	assert!(!server_task.is_finished());
	release_disconnect.notify_one();
	tokio::time::timeout(Duration::from_secs(1), server_task)
		.await
		.unwrap()
		.unwrap();
}

#[tokio::test]
async fn shutdown_is_atomic_and_broadcast_once() {
	let coordinator = ShutdownCoordinator::new(Duration::from_secs(1));
	let mut receiver = coordinator.subscribe();

	assert!(!coordinator.is_shutdown());
	coordinator.shutdown();
	coordinator.shutdown();
	assert!(coordinator.is_shutdown());
	assert_eq!(receiver.recv().await, Ok(()));
	assert_eq!(
		receiver.try_recv(),
		Err(tokio::sync::broadcast::error::TryRecvError::Empty)
	);
}
