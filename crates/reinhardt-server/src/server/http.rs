use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{HeaderMap, Method, StatusCode, Uri};
use hyper_util::rt::TokioIo;
use reinhardt_di::InjectionContext;
use reinhardt_http::{Handler, Middleware, MiddlewareChain};
use reinhardt_http::{Request, Response};
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::timeout;

use crate::shutdown::ShutdownCoordinator;

use super::body::{RequestBodyPlan, collect_request_body, request_body_plan};

/// A listener-owned WebSocket Upgrade task.
#[doc(hidden)]
pub type UpgradeTask = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// One-shot Hyper Upgrade state attached to a Reinhardt request.
#[doc(hidden)]
#[derive(Clone)]
pub struct HttpUpgradeContext {
	on_upgrade: Arc<Mutex<Option<hyper::upgrade::OnUpgrade>>>,
	tasks: mpsc::UnboundedSender<UpgradeTask>,
}

impl HttpUpgradeContext {
	fn new(
		on_upgrade: hyper::upgrade::OnUpgrade,
		tasks: mpsc::UnboundedSender<UpgradeTask>,
	) -> Self {
		Self {
			on_upgrade: Arc::new(Mutex::new(Some(on_upgrade))),
			tasks,
		}
	}

	/// Takes the one-shot Hyper Upgrade future.
	pub fn take_on_upgrade(&self) -> Option<hyper::upgrade::OnUpgrade> {
		self.on_upgrade
			.lock()
			.unwrap_or_else(|poisoned| poisoned.into_inner())
			.take()
	}

	/// Queues an Upgrade task in the listener-owned task set.
	pub fn spawn(&self, task: UpgradeTask) -> Result<(), UpgradeTask> {
		self.tasks.send(task).map_err(|error| error.0)
	}
}

/// HTTP Server with middleware support
pub struct HttpServer {
	handler: Arc<dyn Handler>,
	pub(crate) middlewares: Vec<Arc<dyn Middleware>>,
	di_context: Option<Arc<InjectionContext>>,
}

impl HttpServer {
	/// Create a new server with the given handler
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_server::server::HttpServer;
	/// use reinhardt_http::Handler;
	/// use reinhardt_http::{Request, Response};
	///
	/// struct MyHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for MyHandler {
	///     async fn handle(&self, _req: Request) -> reinhardt_core::exception::Result<Response> {
	///         Ok(Response::ok().with_body("Hello"))
	///     }
	/// }
	///
	/// let server = HttpServer::new(MyHandler);
	/// ```
	pub fn new<H: Handler + 'static>(handler: H) -> Self {
		Self {
			handler: Arc::new(handler),
			middlewares: Vec::new(),
			di_context: None,
		}
	}

	/// Add a middleware to the server using builder pattern
	///
	/// Middlewares are executed in the order they are added.
	///
	/// # Examples
	///
	/// ```
	/// use std::sync::Arc;
	/// use reinhardt_server::server::HttpServer;
	/// use reinhardt_http::{Handler, Middleware};
	/// use reinhardt_http::{Request, Response};
	///
	/// struct MyHandler;
	/// struct MyMiddleware;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for MyHandler {
	///     async fn handle(&self, _req: Request) -> reinhardt_core::exception::Result<Response> {
	///         Ok(Response::ok())
	///     }
	/// }
	///
	/// #[async_trait::async_trait]
	/// impl Middleware for MyMiddleware {
	///     async fn process(&self, request: Request, next: Arc<dyn Handler>) -> reinhardt_core::exception::Result<Response> {
	///         next.handle(request).await
	///     }
	/// }
	///
	/// let server = HttpServer::new(MyHandler)
	///     .with_middleware(MyMiddleware);
	/// ```
	pub fn with_middleware<M: Middleware + 'static>(mut self, middleware: M) -> Self {
		self.middlewares.push(Arc::new(middleware));
		self
	}

	/// Set the dependency injection context for the server
	///
	/// When set, the DI context will be automatically injected into each request,
	/// making it available for endpoints that use `#[inject]` parameters.
	///
	/// # Examples
	///
	/// ```rust,no_run,ignore
	/// # use reinhardt_di::{InjectionContext, SingletonScope};
	/// # use std::sync::Arc;
	/// # struct Router;
	/// # struct HttpServer { di_context: Option<Arc<InjectionContext>> }
	/// # impl HttpServer {
	/// #     fn new(_router: Router) -> Self { Self { di_context: None } }
	/// #     fn with_di_context(mut self, context: Arc<InjectionContext>) -> Self {
	/// #         self.di_context = Some(context);
	/// #         self
	/// #     }
	/// # }
	/// # let router = Router;
	/// let singleton = Arc::new(SingletonScope::new());
	/// let di_context = Arc::new(InjectionContext::builder(singleton).build());
	///
	/// let server = HttpServer::new(router)
	///     .with_di_context(di_context);
	/// ```
	pub fn with_di_context(mut self, context: Arc<InjectionContext>) -> Self {
		self.di_context = Some(context);
		self
	}

	/// Get a clone of the handler
	///
	/// This is useful for test utilities that need access to the handler.
	pub fn handler(&self) -> Arc<dyn Handler> {
		self.handler.clone()
	}

	/// Build the final handler with middleware chain
	///
	/// This creates a MiddlewareChain that wraps the handler with all configured middlewares.
	fn build_handler(&self) -> Arc<dyn Handler> {
		if self.middlewares.is_empty() {
			return self.handler.clone();
		}

		let mut chain = MiddlewareChain::new(self.handler.clone());
		for middleware in &self.middlewares {
			chain.add_middleware(middleware.clone());
		}

		Arc::new(chain)
	}
	/// Start the server and listen on the given address
	///
	/// This method starts the server and begins accepting connections.
	/// It runs indefinitely until an error occurs.
	///
	/// # Examples
	///
	/// ```no_run
	/// use std::net::SocketAddr;
	/// use reinhardt_server::server::HttpServer;
	/// use reinhardt_http::Handler;
	/// use reinhardt_http::{Request, Response};
	///
	/// struct MyHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for MyHandler {
	///     async fn handle(&self, _req: Request) -> reinhardt_core::exception::Result<Response> {
	///         Ok(Response::ok())
	///     }
	/// }
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let server = HttpServer::new(MyHandler);
	/// let addr: SocketAddr = "127.0.0.1:8080".parse()?;
	/// server.listen(addr).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn listen(self, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
		let listener = TcpListener::bind(addr).await?;
		self.listen_on_with_shutdown(
			listener,
			ShutdownCoordinator::new(std::time::Duration::from_secs(30)),
		)
		.await
	}

	/// Start the server with graceful shutdown support
	///
	/// This method starts the server and listens for shutdown signals.
	/// When a shutdown signal is received, it stops accepting new connections
	/// and waits for existing connections to complete.
	///
	/// # Examples
	///
	/// ```no_run
	/// use std::net::SocketAddr;
	/// use std::time::Duration;
	/// use reinhardt_server::server::{HttpServer, ShutdownCoordinator};
	/// use reinhardt_http::Handler;
	/// use reinhardt_http::{Request, Response};
	///
	/// struct MyHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for MyHandler {
	///     async fn handle(&self, _req: Request) -> reinhardt_core::exception::Result<Response> {
	///         Ok(Response::ok())
	///     }
	/// }
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let server = HttpServer::new(MyHandler);
	/// let addr: SocketAddr = "127.0.0.1:8080".parse()?;
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
		self.listen_on_with_shutdown(listener, coordinator).await
	}

	/// Serves a pre-bound listener and owns every connection and Upgrade task.
	#[doc(hidden)]
	pub async fn listen_on_with_shutdown(
		self,
		listener: TcpListener,
		coordinator: ShutdownCoordinator,
	) -> Result<(), Box<dyn std::error::Error>> {
		// Build the handler with middleware chain
		let handler = self.build_handler();
		let di_context = self.di_context.clone();
		let mut shutdown_rx = coordinator.subscribe();
		let (upgrade_tx, mut upgrade_rx) = mpsc::unbounded_channel::<UpgradeTask>();
		let mut tasks = JoinSet::new();
		let mut accept_error = None;

		loop {
			if coordinator.is_shutdown() {
				break;
			}
			tokio::select! {
				result = listener.accept() => {
					let (stream, socket_addr) = match result {
						Ok(connection) => connection,
						Err(error) => {
							accept_error = Some(error);
							coordinator.shutdown();
							break;
						}
					};
					let handler = handler.clone();
					let di_context = di_context.clone();
					let conn_shutdown = coordinator.subscribe();
					let shutdown_started = coordinator.is_shutdown();
					let upgrade_tx = upgrade_tx.clone();

					tasks.spawn(async move {
						if let Err(err) = Self::handle_connection_tracked(
							stream,
							socket_addr,
							handler,
							di_context,
							upgrade_tx,
							conn_shutdown,
							shutdown_started,
						).await {
							eprintln!("Error handling connection: {:?}", err);
						}
					});
				}
				Some(task) = upgrade_rx.recv() => {
					tasks.spawn(task);
				}
				Some(result) = tasks.join_next(), if !tasks.is_empty() => {
					if let Err(error) = result {
						eprintln!("Server task failed: {error}");
					}
				}
				_ = shutdown_rx.recv() => break,
			}
		}

		drop(upgrade_tx);
		let graceful = async {
			loop {
				while let Ok(task) = upgrade_rx.try_recv() {
					tasks.spawn(task);
				}
				if tasks.is_empty() && upgrade_rx.is_closed() {
					break;
				}
				tokio::select! {
					Some(task) = upgrade_rx.recv() => {
						tasks.spawn(task);
					}
					Some(result) = tasks.join_next(), if !tasks.is_empty() => {
						if let Err(error) = result {
							eprintln!("Server task failed during shutdown: {error}");
						}
					}
				}
			}
		};
		if timeout(coordinator.timeout_duration(), graceful)
			.await
			.is_err()
		{
			tasks.abort_all();
			while tasks.join_next().await.is_some() {}
			upgrade_rx.close();
			while let Ok(task) = upgrade_rx.try_recv() {
				tasks.spawn(task);
			}
			tasks.abort_all();
			while tasks.join_next().await.is_some() {}
		}
		coordinator.notify_shutdown_complete();

		match accept_error {
			Some(error) => Err(error.into()),
			None => Ok(()),
		}
	}
	/// Handle a single TCP connection by processing HTTP requests
	///
	/// This is an internal method used by the server to process individual connections.
	///
	/// # Examples
	///
	/// ```no_run
	/// use std::sync::Arc;
	/// use std::net::SocketAddr;
	/// use tokio::net::TcpStream;
	/// use reinhardt_server::server::HttpServer;
	/// use reinhardt_http::Handler;
	/// use reinhardt_http::{Request, Response};
	///
	/// struct MyHandler;
	///
	/// #[async_trait::async_trait]
	/// impl Handler for MyHandler {
	///     async fn handle(&self, _req: Request) -> reinhardt_core::exception::Result<Response> {
	///         Ok(Response::ok())
	///     }
	/// }
	///
	/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let addr: SocketAddr = "127.0.0.1:8080".parse()?;
	/// let stream = TcpStream::connect(addr).await?;
	/// let socket_addr = stream.peer_addr()?;
	/// HttpServer::handle_connection(stream, socket_addr, Arc::new(MyHandler), None).await?;
	/// # Ok(())
	/// # }
	/// ```
	pub async fn handle_connection(
		stream: TcpStream,
		socket_addr: SocketAddr,
		handler: Arc<dyn Handler>,
		di_context: Option<Arc<InjectionContext>>,
	) -> Result<(), Box<dyn std::error::Error>> {
		Self::handle_connection_with(
			stream,
			socket_addr,
			move |request| {
				let handler = Arc::clone(&handler);
				async move { handler.as_ref().handle(request).await }
			},
			di_context,
		)
		.await
	}

	async fn handle_connection_tracked(
		stream: TcpStream,
		socket_addr: SocketAddr,
		handler: Arc<dyn Handler>,
		di_context: Option<Arc<InjectionContext>>,
		upgrade_tasks: mpsc::UnboundedSender<UpgradeTask>,
		mut shutdown: tokio::sync::broadcast::Receiver<()>,
		shutdown_started: bool,
	) -> Result<(), Box<dyn std::error::Error>> {
		let io = TokioIo::new(stream);
		let service = service_fn(move |req| {
			let handler = Arc::clone(&handler);
			let di_context = di_context.clone();
			let upgrade_tasks = upgrade_tasks.clone();

			handle_request_with(
				req,
				move |request| {
					let handler = Arc::clone(&handler);
					async move { handler.as_ref().handle(request).await }
				},
				socket_addr,
				di_context,
				DEFAULT_MAX_BODY_SIZE,
				Some(upgrade_tasks),
			)
		});

		let connection = http1::Builder::new()
			.serve_connection(io, service)
			.with_upgrades();
		tokio::pin!(connection);
		if shutdown_started {
			connection.as_mut().graceful_shutdown();
		} else {
			tokio::select! {
				result = connection.as_mut() => {
					result?;
					return Ok(());
				}
				_ = shutdown.recv() => {
					connection.as_mut().graceful_shutdown();
				}
			}
		}
		connection.as_mut().await?;
		Ok(())
	}

	/// Handle a single TCP connection with a concrete request handler function.
	///
	/// This lower-level adapter is useful when callers can keep their routing
	/// entry point concrete, avoiding the boxed future required by
	/// `Arc<dyn Handler>`.
	pub async fn handle_connection_with<F, Fut>(
		stream: TcpStream,
		socket_addr: SocketAddr,
		handler: F,
		di_context: Option<Arc<InjectionContext>>,
	) -> Result<(), Box<dyn std::error::Error>>
	where
		F: Fn(Request) -> Fut + Clone + Send + Sync + 'static,
		Fut: Future<Output = reinhardt_http::Result<Response>> + Send + 'static,
	{
		let io = TokioIo::new(stream);
		let (upgrade_tasks, upgrade_rx) = mpsc::unbounded_channel();
		drop(upgrade_rx);
		let service = service_fn(move |req| {
			let handler = handler.clone();
			let di_context = di_context.clone();
			let upgrade_tasks = upgrade_tasks.clone();

			handle_request_with(
				req,
				handler,
				socket_addr,
				di_context,
				DEFAULT_MAX_BODY_SIZE,
				Some(upgrade_tasks),
			)
		});

		http1::Builder::new()
			.serve_connection(io, service)
			.with_upgrades()
			.await?;

		Ok(())
	}

	/// Handle a single TCP connection with a synchronous request handler.
	///
	/// This is a lower-overhead variant for routes that complete without
	/// awaiting after the request body has been collected.
	pub async fn handle_connection_sync<F>(
		stream: TcpStream,
		socket_addr: SocketAddr,
		handler: F,
		di_context: Option<Arc<InjectionContext>>,
	) -> Result<(), Box<dyn std::error::Error>>
	where
		F: Fn(Request) -> reinhardt_http::Result<Response> + Clone + Send + Sync + 'static,
	{
		Self::handle_connection_sync_with_precheck(
			stream,
			socket_addr,
			|_, _, _| None,
			handler,
			di_context,
		)
		.await
	}

	/// Handle a single TCP connection with a synchronous request handler and a
	/// pre-request fast path.
	///
	/// The precheck runs only after the adapter has verified that the incoming
	/// request has no body. When it returns a response, the adapter skips full
	/// [`Request`] construction.
	pub async fn handle_connection_sync_with_precheck<F, P>(
		stream: TcpStream,
		socket_addr: SocketAddr,
		precheck: P,
		handler: F,
		di_context: Option<Arc<InjectionContext>>,
	) -> Result<(), Box<dyn std::error::Error>>
	where
		F: Fn(Request) -> reinhardt_http::Result<Response> + Clone + Send + Sync + 'static,
		P: Fn(&Method, &Uri, &HeaderMap) -> Option<reinhardt_http::Result<Response>>
			+ Clone
			+ Send
			+ Sync
			+ 'static,
	{
		let io = TokioIo::new(stream);
		let (upgrade_tasks, upgrade_rx) = mpsc::unbounded_channel();
		drop(upgrade_rx);
		let service = service_fn(move |req| {
			let handler = handler.clone();
			let precheck = precheck.clone();
			let di_context = di_context.clone();
			let upgrade_tasks = upgrade_tasks.clone();

			handle_request_sync_with_precheck(
				req,
				precheck,
				handler,
				socket_addr,
				di_context,
				DEFAULT_MAX_BODY_SIZE,
				Some(upgrade_tasks),
			)
		});

		http1::Builder::new()
			.serve_connection(io, service)
			.with_upgrades()
			.await?;

		Ok(())
	}
}

/// Default maximum request body size (10 MB)
const DEFAULT_MAX_BODY_SIZE: u64 = 10 * 1024 * 1024;
type BoxError = Box<dyn std::error::Error + Send + Sync>;

async fn handle_request_with<F, Fut>(
	mut req: hyper::Request<Incoming>,
	handler: F,
	remote_addr: SocketAddr,
	di_context: Option<Arc<InjectionContext>>,
	max_body_size: u64,
	upgrade_tasks: Option<mpsc::UnboundedSender<UpgradeTask>>,
) -> Result<hyper::Response<Full<Bytes>>, BoxError>
where
	F: Fn(Request) -> Fut + Clone + Send + Sync + 'static,
	Fut: Future<Output = reinhardt_http::Result<Response>> + Send + 'static,
{
	let upgrade_context =
		upgrade_tasks.map(|tasks| HttpUpgradeContext::new(hyper::upgrade::on(&mut req), tasks));
	let (parts, body) = req.into_parts();

	let body_bytes = match request_body_plan(&parts.method, &parts.headers, max_body_size) {
		RequestBodyPlan::Empty => Bytes::new(),
		RequestBodyPlan::Collect => match collect_request_body(body, max_body_size).await {
			Ok(body) => body,
			Err(error) if error.is_too_large() => return Ok(request_body_too_large_response()),
			Err(error) => return Err(error.into_box_error()),
		},
		RequestBodyPlan::RejectTooLarge => return Ok(request_body_too_large_response()),
	};

	// Create reinhardt Request
	let mut request = Request::from_hyper_parts(
		parts.method,
		parts.uri,
		parts.version,
		parts.headers,
		body_bytes,
		false,
		Some(remote_addr),
	);

	// Set DI context if available
	if let Some(ctx) = di_context {
		request.set_di_context(ctx);
	}
	if let Some(upgrade_context) = upgrade_context {
		request.extensions.insert(upgrade_context);
	}

	// Handle request.
	// The middleware chain converts handler errors to responses internally
	// (in ConditionalComposedHandler) so that middleware post-processing
	// always runs. This unwrap_or_else is a safety net for errors that
	// escape the chain (e.g., middleware-internal failures without a chain).
	#[cfg(debug_assertions)]
	let request_path_for_warning = {
		let path = request.uri.path();
		if path.contains('.') && !path.ends_with(".json") {
			Some(path.to_string())
		} else {
			None
		}
	};
	let response = handler(request).await.unwrap_or_else(|e| {
		#[cfg(debug_assertions)]
		if let Some(request_path) = request_path_for_warning.as_deref() {
			eprintln!(
				"[reinhardt WARN] Non-API request hit error-to-JSON conversion: path={}, error={}",
				request_path, e
			);
		}
		Response::from(e)
	});

	Ok(into_hyper_response(response))
}

async fn handle_request_sync_with_precheck<F, P>(
	mut req: hyper::Request<Incoming>,
	precheck: P,
	handler: F,
	remote_addr: SocketAddr,
	di_context: Option<Arc<InjectionContext>>,
	max_body_size: u64,
	upgrade_tasks: Option<mpsc::UnboundedSender<UpgradeTask>>,
) -> Result<hyper::Response<Full<Bytes>>, BoxError>
where
	F: Fn(Request) -> reinhardt_http::Result<Response> + Clone + Send + Sync + 'static,
	P: Fn(&Method, &Uri, &HeaderMap) -> Option<reinhardt_http::Result<Response>>
		+ Clone
		+ Send
		+ Sync
		+ 'static,
{
	let upgrade_context =
		upgrade_tasks.map(|tasks| HttpUpgradeContext::new(hyper::upgrade::on(&mut req), tasks));
	let (parts, body) = req.into_parts();

	let body_plan = request_body_plan(&parts.method, &parts.headers, max_body_size);
	if body_plan == RequestBodyPlan::Empty
		&& di_context.is_none()
		&& !is_upgrade_candidate(&parts.headers)
		&& let Some(response) = precheck(&parts.method, &parts.uri, &parts.headers)
	{
		return Ok(into_hyper_response(
			response.unwrap_or_else(reinhardt_http::Response::from),
		));
	}

	let body_bytes = match body_plan {
		RequestBodyPlan::Empty => Bytes::new(),
		RequestBodyPlan::Collect => match collect_request_body(body, max_body_size).await {
			Ok(body) => body,
			Err(error) if error.is_too_large() => return Ok(request_body_too_large_response()),
			Err(error) => return Err(error.into_box_error()),
		},
		RequestBodyPlan::RejectTooLarge => return Ok(request_body_too_large_response()),
	};

	let mut request = Request::from_hyper_parts(
		parts.method,
		parts.uri,
		parts.version,
		parts.headers,
		body_bytes,
		false,
		Some(remote_addr),
	);

	if let Some(ctx) = di_context {
		request.set_di_context(ctx);
	}
	if let Some(upgrade_context) = upgrade_context {
		request.extensions.insert(upgrade_context);
	}

	#[cfg(debug_assertions)]
	let request_path_for_warning = {
		let path = request.uri.path();
		if path.contains('.') && !path.ends_with(".json") {
			Some(path.to_string())
		} else {
			None
		}
	};
	let response = handler(request).unwrap_or_else(|e| {
		#[cfg(debug_assertions)]
		if let Some(request_path) = request_path_for_warning.as_deref() {
			eprintln!(
				"[reinhardt WARN] Non-API request hit error-to-JSON conversion: path={}, error={}",
				request_path, e
			);
		}
		Response::from(e)
	});

	Ok(into_hyper_response(response))
}

fn is_upgrade_candidate(headers: &HeaderMap) -> bool {
	headers.contains_key(hyper::header::UPGRADE)
		|| headers
			.get_all(hyper::header::CONNECTION)
			.iter()
			.filter_map(|value| value.to_str().ok())
			.flat_map(|value| value.split(','))
			.any(|token| token.trim().eq_ignore_ascii_case("upgrade"))
}

fn into_hyper_response(response: Response) -> hyper::Response<Full<Bytes>> {
	let status = response.status;
	let headers = response.headers;
	let mut hyper_response = hyper::Response::new(Full::new(response.body));
	if status != StatusCode::OK {
		*hyper_response.status_mut() = status;
	}
	if !headers.is_empty() {
		*hyper_response.headers_mut() = headers;
	}
	hyper_response
}

fn request_body_too_large_response() -> hyper::Response<Full<Bytes>> {
	hyper::Response::builder()
		.status(StatusCode::PAYLOAD_TOO_LARGE)
		.body(Full::new(Bytes::from_static(b"Request body too large")))
		.expect("Failed to build 413 response")
}

/// Helper function to create and run a server
///
/// This is a convenience function that creates an `HttpServer` and starts listening.
///
/// # Examples
///
/// ```no_run
/// use std::net::SocketAddr;
/// use reinhardt_server::server::serve;
/// use reinhardt_http::Handler;
/// use reinhardt_http::{Request, Response};
///
/// struct MyHandler;
///
/// #[async_trait::async_trait]
/// impl Handler for MyHandler {
///     async fn handle(&self, _req: Request) -> reinhardt_core::exception::Result<Response> {
///         Ok(Response::ok().with_body("Hello, World!"))
///     }
/// }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let addr: SocketAddr = "127.0.0.1:3000".parse()?;
/// serve(addr, MyHandler).await?;
/// # Ok(())
/// # }
/// ```
pub async fn serve<H: Handler + 'static>(
	addr: SocketAddr,
	handler: H,
) -> Result<(), Box<dyn std::error::Error>> {
	let server = HttpServer::new(handler);
	server.listen(addr).await
}

/// Helper function to create and run a server with graceful shutdown
///
/// This function sets up a server with shutdown signal handling and graceful shutdown support.
///
/// # Examples
///
/// ```no_run
/// use std::net::SocketAddr;
/// use std::time::Duration;
/// use reinhardt_server::server::{serve_with_shutdown, shutdown_signal, ShutdownCoordinator};
/// use reinhardt_http::Handler;
/// use reinhardt_http::{Request, Response};
///
/// struct MyHandler;
///
/// #[async_trait::async_trait]
/// impl Handler for MyHandler {
///     async fn handle(&self, _req: Request) -> reinhardt_core::exception::Result<Response> {
///         Ok(Response::ok().with_body("Hello, World!"))
///     }
/// }
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let addr: SocketAddr = "127.0.0.1:3000".parse()?;
/// let coordinator = ShutdownCoordinator::new(Duration::from_secs(30));
///
/// tokio::select! {
///     result = serve_with_shutdown(addr, MyHandler, coordinator.clone()) => {
///         result?;
///     }
///     _ = shutdown_signal() => {
///         coordinator.shutdown();
///         coordinator.wait_for_shutdown().await;
///     }
/// }
/// # Ok(())
/// # }
/// ```
pub async fn serve_with_shutdown<H: Handler + 'static>(
	addr: SocketAddr,
	handler: H,
	coordinator: ShutdownCoordinator,
) -> Result<(), Box<dyn std::error::Error>> {
	let server = HttpServer::new(handler);
	server.listen_with_shutdown(addr, coordinator).await
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	struct TestHandler;

	#[async_trait::async_trait]
	impl Handler for TestHandler {
		async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
			Ok(Response::ok().with_body("Hello, World!"))
		}
	}

	#[tokio::test]
	async fn test_http_server_creation() {
		let _server = HttpServer::new(TestHandler);
		// Just verify server can be created without panicking
	}

	#[tokio::test]
	async fn test_http_server_with_middleware() {
		use reinhardt_http::Middleware;

		struct TestMiddleware {
			prefix: String,
		}

		#[async_trait::async_trait]
		impl Middleware for TestMiddleware {
			async fn process(
				&self,
				request: Request,
				next: Arc<dyn Handler>,
			) -> reinhardt_core::exception::Result<Response> {
				let response = next.handle(request).await?;
				let current_body = String::from_utf8(response.body.to_vec()).unwrap_or_default();
				let new_body = format!("{}{}", self.prefix, current_body);
				Ok(Response::ok().with_body(new_body))
			}
		}

		let server = HttpServer::new(TestHandler).with_middleware(TestMiddleware {
			prefix: "Middleware: ".to_string(),
		});

		// Verify middleware is added
		assert_eq!(server.middlewares.len(), 1);
	}

	#[tokio::test]
	async fn test_http_server_multiple_middlewares() {
		use reinhardt_http::Middleware;

		struct PrefixMiddleware {
			prefix: String,
		}

		#[async_trait::async_trait]
		impl Middleware for PrefixMiddleware {
			async fn process(
				&self,
				request: Request,
				next: Arc<dyn Handler>,
			) -> reinhardt_core::exception::Result<Response> {
				let response = next.handle(request).await?;
				let current_body = String::from_utf8(response.body.to_vec()).unwrap_or_default();
				let new_body = format!("{}{}", self.prefix, current_body);
				Ok(Response::ok().with_body(new_body))
			}
		}

		let server = HttpServer::new(TestHandler)
			.with_middleware(PrefixMiddleware {
				prefix: "MW1:".to_string(),
			})
			.with_middleware(PrefixMiddleware {
				prefix: "MW2:".to_string(),
			});

		assert_eq!(server.middlewares.len(), 2);
	}

	#[tokio::test]
	async fn test_middleware_chain_execution() {
		use bytes::Bytes;
		use hyper::{HeaderMap, Method, Version};
		use reinhardt_http::Middleware;

		struct PrefixMiddleware {
			prefix: String,
		}

		#[async_trait::async_trait]
		impl Middleware for PrefixMiddleware {
			async fn process(
				&self,
				request: Request,
				next: Arc<dyn Handler>,
			) -> reinhardt_core::exception::Result<Response> {
				let response = next.handle(request).await?;
				let current_body = String::from_utf8(response.body.to_vec()).unwrap_or_default();
				let new_body = format!("{}{}", self.prefix, current_body);
				Ok(Response::ok().with_body(new_body))
			}
		}

		let server = HttpServer::new(TestHandler)
			.with_middleware(PrefixMiddleware {
				prefix: "First:".to_string(),
			})
			.with_middleware(PrefixMiddleware {
				prefix: "Second:".to_string(),
			});

		// Build the handler with middleware chain
		let handler = server.build_handler();

		// Create a test request
		let request = Request::builder()
			.method(Method::GET)
			.uri("/")
			.version(Version::HTTP_11)
			.headers(HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Execute the handler
		let response = handler.handle(request).await.unwrap();
		let body = String::from_utf8(response.body.to_vec()).unwrap();

		// Middlewares should be applied in order: First -> Second -> Handler
		assert_eq!(body, "First:Second:Hello, World!");
	}

	/// Handler that returns a database error containing sensitive internal details
	struct ErrorHandler {
		error_message: String,
		error_kind: reinhardt_core::exception::DatabaseErrorKind,
	}

	#[async_trait::async_trait]
	impl Handler for ErrorHandler {
		async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
			Err(reinhardt_core::exception::DatabaseError::new(
				self.error_kind,
				self.error_message.clone(),
			)
			.into())
		}
	}

	#[rstest]
	#[case::database_connection_string(
		"postgres://admin:s3cret@10.0.0.5/prod_db: connection refused",
		"postgres",
		reinhardt_core::exception::DatabaseErrorKind::Connection,
		StatusCode::SERVICE_UNAVAILABLE
	)]
	#[case::internal_file_path(
		"/opt/app/config/secrets.yml: file not found",
		"/opt/app",
		reinhardt_core::exception::DatabaseErrorKind::Configuration,
		StatusCode::INTERNAL_SERVER_ERROR
	)]
	#[case::sql_query_details(
		"SELECT * FROM users WHERE password = 'hash123': syntax error",
		"SELECT",
		reinhardt_core::exception::DatabaseErrorKind::Query,
		StatusCode::INTERNAL_SERVER_ERROR
	)]
	#[case::serialization_details(
		"failed to serialize field `password_hash`",
		"password_hash",
		reinhardt_core::exception::DatabaseErrorKind::Serialization,
		StatusCode::INTERNAL_SERVER_ERROR
	)]
	#[tokio::test]
	async fn test_error_handler_does_not_leak_internal_details(
		#[case] sensitive_message: &str,
		#[case] leaked_fragment: &str,
		#[case] error_kind: reinhardt_core::exception::DatabaseErrorKind,
		#[case] expected_status: StatusCode,
	) {
		// Arrange
		let server = HttpServer::new(ErrorHandler {
			error_message: sensitive_message.to_string(),
			error_kind,
		});
		let handler = server.build_handler();
		let request = Request::builder()
			.method(hyper::Method::GET)
			.uri("/")
			.version(hyper::Version::HTTP_11)
			.headers(hyper::HeaderMap::new())
			.body(Bytes::new())
			.build()
			.unwrap();

		// Act
		let response = handler.handle(request).await.unwrap_or_else(Response::from);
		let body = String::from_utf8(response.body.to_vec()).unwrap();

		// Assert
		assert_eq!(response.status, expected_status);
		assert!(
			!body.contains(leaked_fragment),
			"Response body must not contain internal details '{leaked_fragment}', but got: {body}"
		);
	}

	struct TrackedUpgradeTaskHandler {
		started: Arc<tokio::sync::Notify>,
		release: Arc<tokio::sync::Notify>,
	}

	#[async_trait::async_trait]
	impl Handler for TrackedUpgradeTaskHandler {
		async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
			let upgrade = request.extensions.get::<HttpUpgradeContext>().unwrap();
			let started = Arc::clone(&self.started);
			let release = Arc::clone(&self.release);
			let queued = upgrade.spawn(Box::pin(async move {
				started.notify_one();
				release.notified().await;
			}));
			assert!(queued.is_ok());
			Ok(Response::ok().with_body("tracked"))
		}
	}

	#[tokio::test]
	async fn listen_on_waits_for_queued_upgrade_tasks() {
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let address = listener.local_addr().unwrap();
		let coordinator = ShutdownCoordinator::new(std::time::Duration::from_secs(2));
		let started = Arc::new(tokio::sync::Notify::new());
		let release = Arc::new(tokio::sync::Notify::new());
		let handler = TrackedUpgradeTaskHandler {
			started: Arc::clone(&started),
			release: Arc::clone(&release),
		};
		let server_coordinator = coordinator.clone();
		let server_task = tokio::spawn(async move {
			HttpServer::new(handler)
				.listen_on_with_shutdown(listener, server_coordinator)
				.await
				.unwrap();
		});

		let response = reqwest::get(format!("http://{address}/")).await.unwrap();
		assert_eq!(response.status(), StatusCode::OK);
		assert_eq!(response.text().await.unwrap(), "tracked");
		started.notified().await;
		coordinator.shutdown();
		tokio::time::sleep(std::time::Duration::from_millis(25)).await;
		assert!(!server_task.is_finished());
		release.notify_one();
		tokio::time::timeout(std::time::Duration::from_secs(1), server_task)
			.await
			.unwrap()
			.unwrap();
	}

	struct InFlightHandler {
		entered: Arc<tokio::sync::Notify>,
		release: Arc<tokio::sync::Notify>,
	}

	#[async_trait::async_trait]
	impl Handler for InFlightHandler {
		async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
			self.entered.notify_one();
			self.release.notified().await;
			Ok(Response::ok().with_body("finished"))
		}
	}

	#[tokio::test]
	async fn shutdown_gracefully_finishes_in_flight_http_response() {
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let address = listener.local_addr().unwrap();
		let coordinator = ShutdownCoordinator::new(std::time::Duration::from_secs(2));
		let entered = Arc::new(tokio::sync::Notify::new());
		let release = Arc::new(tokio::sync::Notify::new());
		let handler = InFlightHandler {
			entered: Arc::clone(&entered),
			release: Arc::clone(&release),
		};
		let server_coordinator = coordinator.clone();
		let server_task = tokio::spawn(async move {
			HttpServer::new(handler)
				.listen_on_with_shutdown(listener, server_coordinator)
				.await
				.unwrap();
		});
		let client_task =
			tokio::spawn(async move { reqwest::get(format!("http://{address}/")).await.unwrap() });

		entered.notified().await;
		coordinator.shutdown();
		tokio::time::sleep(std::time::Duration::from_millis(25)).await;
		assert!(!server_task.is_finished());
		release.notify_one();
		let response = tokio::time::timeout(std::time::Duration::from_secs(1), client_task)
			.await
			.unwrap()
			.unwrap();
		assert_eq!(response.status(), StatusCode::OK);
		assert_eq!(response.text().await.unwrap(), "finished");
		tokio::time::timeout(std::time::Duration::from_secs(1), server_task)
			.await
			.unwrap()
			.unwrap();
	}

	struct SignalOnDrop {
		started: Arc<tokio::sync::Notify>,
		release: Arc<std::sync::Barrier>,
	}

	impl Drop for SignalOnDrop {
		fn drop(&mut self) {
			self.started.notify_one();
			self.release.wait();
		}
	}

	struct EnqueueOnDrop {
		tasks: mpsc::UnboundedSender<UpgradeTask>,
		late_drop_started: Arc<tokio::sync::Notify>,
		late_drop_release: Arc<std::sync::Barrier>,
	}

	impl Drop for EnqueueOnDrop {
		fn drop(&mut self) {
			let drop_guard = SignalOnDrop {
				started: Arc::clone(&self.late_drop_started),
				release: Arc::clone(&self.late_drop_release),
			};
			let task = Box::pin(async move {
				let _drop_guard = drop_guard;
				std::future::pending::<()>().await;
			});
			let _ = self.tasks.send(task);
		}
	}

	struct TimeoutRaceHandler {
		context: Arc<Mutex<Option<HttpUpgradeContext>>>,
		queued: Arc<tokio::sync::Notify>,
		late_drop_started: Arc<tokio::sync::Notify>,
		late_drop_release: Arc<std::sync::Barrier>,
	}

	#[async_trait::async_trait]
	impl Handler for TimeoutRaceHandler {
		async fn handle(&self, request: Request) -> reinhardt_core::exception::Result<Response> {
			let upgrade = request
				.extensions
				.get::<HttpUpgradeContext>()
				.unwrap()
				.clone();
			*self.context.lock().unwrap() = Some(upgrade.clone());
			let enqueue_on_drop = EnqueueOnDrop {
				tasks: upgrade.tasks.clone(),
				late_drop_started: Arc::clone(&self.late_drop_started),
				late_drop_release: Arc::clone(&self.late_drop_release),
			};
			let queued = upgrade.spawn(Box::pin(async move {
				let _enqueue_on_drop = enqueue_on_drop;
				std::future::pending::<()>().await;
			}));
			assert!(queued.is_ok());
			self.queued.notify_one();
			Ok(Response::ok().with_body("queued"))
		}
	}

	#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
	async fn timeout_closes_upgrade_queue_before_final_join() {
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let address = listener.local_addr().unwrap();
		let coordinator = ShutdownCoordinator::new(std::time::Duration::from_millis(25));
		let context = Arc::new(Mutex::new(None));
		let queued = Arc::new(tokio::sync::Notify::new());
		let late_drop_started = Arc::new(tokio::sync::Notify::new());
		let late_drop_release = Arc::new(std::sync::Barrier::new(2));
		let handler = TimeoutRaceHandler {
			context: Arc::clone(&context),
			queued: Arc::clone(&queued),
			late_drop_started: Arc::clone(&late_drop_started),
			late_drop_release: Arc::clone(&late_drop_release),
		};
		let server_coordinator = coordinator.clone();
		let server_task = tokio::spawn(async move {
			HttpServer::new(handler)
				.listen_on_with_shutdown(listener, server_coordinator)
				.await
				.unwrap();
		});

		let response = reqwest::get(format!("http://{address}/")).await.unwrap();
		assert_eq!(response.status(), StatusCode::OK);
		queued.notified().await;
		coordinator.shutdown();
		tokio::time::timeout(
			std::time::Duration::from_secs(1),
			late_drop_started.notified(),
		)
		.await
		.unwrap();
		let upgrade = context.lock().unwrap().clone().unwrap();
		let late_enqueue = upgrade.spawn(Box::pin(async {}));
		late_drop_release.wait();
		assert!(late_enqueue.is_err());
		tokio::time::timeout(std::time::Duration::from_secs(1), server_task)
			.await
			.unwrap()
			.unwrap();
	}
}
