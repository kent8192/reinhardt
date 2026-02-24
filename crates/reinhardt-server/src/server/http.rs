use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::StatusCode;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::Service;
use hyper_util::rt::TokioIo;
use reinhardt_di::InjectionContext;
use reinhardt_http::{Handler, Middleware, MiddlewareChain};
use reinhardt_http::{Request, Response};
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};

use crate::shutdown::ShutdownCoordinator;

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

		// Build the handler with middleware chain
		let handler = self.build_handler();
		let di_context = self.di_context.clone();

		loop {
			let (stream, socket_addr) = listener.accept().await?;
			let handler = handler.clone();
			let di_context = di_context.clone();

			tokio::task::spawn(async move {
				if let Err(err) =
					Self::handle_connection(stream, socket_addr, handler, di_context).await
				{
					eprintln!("Error handling connection: {:?}", err);
				}
			});
		}
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

		// Build the handler with middleware chain
		let handler = self.build_handler();
		let di_context = self.di_context.clone();

		let mut shutdown_rx = coordinator.subscribe();

		loop {
			tokio::select! {
				// Accept new connection
				result = listener.accept() => {
					let (stream, socket_addr) = result?;
					let handler = handler.clone();
					let di_context = di_context.clone();
					let mut conn_shutdown = coordinator.subscribe();

					tokio::task::spawn(async move {
						// Handle connection with shutdown support
						tokio::select! {
							result = Self::handle_connection(stream, socket_addr, handler, di_context) => {
								if let Err(err) = result {
									eprintln!("Error handling connection: {:?}", err);
								}
							}
							_ = conn_shutdown.recv() => {
								// Connection interrupted by shutdown
							}
						}
					});
				}
				// Shutdown signal received
				_ = shutdown_rx.recv() => {
					println!("Shutdown signal received, stopping server...");
					break;
				}
			}
		}

		// Notify that server has stopped accepting connections
		coordinator.notify_shutdown_complete();

		Ok(())
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
		let io = TokioIo::new(stream);
		let service = RequestService {
			handler,
			remote_addr: socket_addr,
			di_context,
			max_body_size: DEFAULT_MAX_BODY_SIZE,
		};

		http1::Builder::new().serve_connection(io, service).await?;

		Ok(())
	}
}

/// Default maximum request body size (10 MB)
const DEFAULT_MAX_BODY_SIZE: u64 = 10 * 1024 * 1024;

/// Service implementation for hyper
struct RequestService {
	handler: Arc<dyn Handler>,
	remote_addr: SocketAddr,
	di_context: Option<Arc<InjectionContext>>,
	max_body_size: u64,
}

impl Service<hyper::Request<Incoming>> for RequestService {
	type Response = hyper::Response<Full<Bytes>>;
	type Error = Box<dyn std::error::Error + Send + Sync>;
	type Future =
		Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

	fn call(&self, req: hyper::Request<Incoming>) -> Self::Future {
		let handler = self.handler.clone();
		let remote_addr = self.remote_addr;
		let di_context = self.di_context.clone();
		let max_body_size = self.max_body_size;

		Box::pin(async move {
			// Check Content-Length before reading body
			if let Some(content_length) = req.headers().get(hyper::header::CONTENT_LENGTH)
				&& let Ok(len_str) = content_length.to_str()
				&& let Ok(len) = len_str.parse::<u64>()
				&& len > max_body_size
			{
				return Ok(hyper::Response::builder()
					.status(StatusCode::PAYLOAD_TOO_LARGE)
					.body(Full::new(Bytes::from("Request body too large")))
					.expect("Failed to build 413 response"));
			}

			// Extract request parts
			let (parts, body) = req.into_parts();

			// Read body with size limit
			let body_bytes = http_body_util::Limited::new(body, max_body_size as usize)
				.collect()
				.await
				.map_err(|_| {
					Box::new(std::io::Error::new(
						std::io::ErrorKind::InvalidData,
						"Request body exceeds size limit",
					)) as Box<dyn std::error::Error + Send + Sync>
				})?
				.to_bytes();

			// Create reinhardt Request
			let mut request = Request::builder()
				.method(parts.method)
				.uri(parts.uri)
				.version(parts.version)
				.headers(parts.headers)
				.body(body_bytes)
				.remote_addr(remote_addr)
				.build()
				.expect("Failed to build request");

			// Set DI context if available
			if let Some(ctx) = di_context {
				request.set_di_context(ctx);
			}

			// Handle request
			let response = handler.handle(request).await.unwrap_or_else(|err| {
				// Convert error to appropriate HTTP response based on status code
				let status_code = StatusCode::from_u16(err.status_code())
					.unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
				Response::new(status_code).with_body(err.to_string())
			});

			// Convert to hyper response
			let mut hyper_response = hyper::Response::builder().status(response.status);

			// Add headers
			for (key, value) in response.headers.iter() {
				hyper_response = hyper_response.header(key, value);
			}

			Ok(hyper_response.body(Full::new(response.body))?)
		})
	}
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
}
