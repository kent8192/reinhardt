use async_trait::async_trait;
use chrono::Local;
use colored::Colorize;
use reinhardt_http::{Handler, Middleware, Request, Response, Result};
use std::sync::Arc;
use std::time::Instant;

/// Configuration for logging middleware
///
/// Controls how request/response information is logged.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct LoggingConfig {
	/// Whether to include raw values (request body, etc.) in error logs.
	/// Disable in production to avoid logging sensitive data.
	pub include_raw_values: bool,

	/// Whether to output errors in multi-line format for better readability.
	/// When false, errors are logged on a single line.
	pub multiline_errors: bool,
}

impl Default for LoggingConfig {
	fn default() -> Self {
		Self {
			include_raw_values: true, // Default is to include (development-friendly)
			multiline_errors: true,   // Multi-line is more readable
		}
	}
}

impl LoggingConfig {
	/// Create a production-safe configuration
	///
	/// - `include_raw_values`: false (don't log potentially sensitive request data)
	/// - `multiline_errors`: true (keep readable format)
	pub fn production() -> Self {
		Self {
			include_raw_values: false,
			multiline_errors: true,
		}
	}
}

/// Django-style request logging middleware with colored output
///
/// Outputs request logs in Django's runserver format with latency:
/// `[DD/Mon/YYYY HH:MM:SS] "METHOD /path HTTP/1.1" STATUS SIZE LATENCYms`
///
/// Status codes are color-coded:
/// - 2xx: Green (success)
/// - 3xx: Cyan (redirect)
/// - 4xx: Yellow (client error)
/// - 5xx: Red (server error)
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use reinhardt_middleware::LoggingMiddleware;
/// use reinhardt_http::{Handler, Middleware, Request, Response};
/// use hyper::{Method, Version, HeaderMap, StatusCode};
/// use bytes::Bytes;
///
/// struct TestHandler;
///
/// #[async_trait::async_trait]
/// impl Handler for TestHandler {
///     async fn handle(&self, _request: Request) -> reinhardt_core::exception::Result<Response> {
///         Ok(Response::new(StatusCode::OK).with_body(Bytes::from("OK")))
///     }
/// }
///
/// # tokio_test::block_on(async {
/// let middleware = LoggingMiddleware::new();
/// let handler = Arc::new(TestHandler);
/// let request = Request::builder()
///     .method(Method::GET)
///     .uri("/api/users")
///     .version(Version::HTTP_11)
///     .headers(HeaderMap::new())
///     .body(Bytes::new())
///     .build()
///     .unwrap();
///
/// let response = middleware.process(request, handler).await.unwrap();
/// assert_eq!(response.status, StatusCode::OK);
/// // Logs: [15/Dec/2024 10:30:45] "GET /api/users HTTP/1.1" 200 2 0ms
/// # });
/// ```
pub struct LoggingMiddleware {
	config: LoggingConfig,
}

impl LoggingMiddleware {
	/// Create a new logging middleware with default configuration
	pub fn new() -> Self {
		Self {
			config: LoggingConfig::default(),
		}
	}

	/// Create a logging middleware with custom configuration
	pub fn with_config(config: LoggingConfig) -> Self {
		Self { config }
	}

	/// Create a production-ready logging middleware
	///
	/// Uses `LoggingConfig::production()` which disables raw value logging.
	pub fn production() -> Self {
		Self {
			config: LoggingConfig::production(),
		}
	}
}

impl Default for LoggingMiddleware {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl Middleware for LoggingMiddleware {
	async fn process(&self, request: Request, next: Arc<dyn Handler>) -> Result<Response> {
		let start = Instant::now();
		let method = request.method.to_string();
		let path = request.path().to_string();
		let version = format_http_version(request.version);

		let result = next.handle(request).await;
		let duration = start.elapsed();

		match &result {
			Ok(response) => {
				let status_code = response.status.as_u16();
				let status_colored = colorize_status(status_code);
				let timestamp = Local::now().format("%d/%b/%Y %H:%M:%S");
				let request_line = format!("\"{} {} {}\"", method, path, version);

				println!(
					"{} {} {} {} {}",
					format!("[{timestamp}]").dimmed(),
					request_line.white(),
					status_colored,
					response.body.len().to_string().cyan(),
					format!("{}ms", duration.as_millis()).dimmed(),
				);
			}
			Err(err) => {
				let status_code = err.status_code();
				let status_colored = colorize_status(status_code);
				let timestamp = Local::now().format("%d/%b/%Y %H:%M:%S");
				let request_line = format!("\"{} {} {}\"", method, path, version);

				// Output main request line
				eprintln!(
					"{} {} {} {}",
					format!("[{timestamp}]").dimmed(),
					request_line.white(),
					status_colored,
					format!("{}ms", duration.as_millis()).dimmed(),
				);

				// Output error details based on configuration
				if self.config.multiline_errors {
					// Multi-line format for better readability
					let error_details = format_error_multiline(err, self.config.include_raw_values);
					for line in error_details.lines() {
						eprintln!("{}", line.red());
					}
				} else {
					// Single-line format (legacy)
					eprintln!("  {}", err.to_string().red());
				}
			}
		}

		result
	}
}

fn format_http_version(version: hyper::Version) -> &'static str {
	match version {
		hyper::Version::HTTP_09 => "HTTP/0.9",
		hyper::Version::HTTP_10 => "HTTP/1.0",
		hyper::Version::HTTP_11 => "HTTP/1.1",
		hyper::Version::HTTP_2 => "HTTP/2.0",
		hyper::Version::HTTP_3 => "HTTP/3.0",
		_ => "HTTP/1.1",
	}
}

/// Colorize HTTP status code based on its class
fn colorize_status(status: u16) -> colored::ColoredString {
	let status_str = status.to_string();
	match status {
		200..=299 => status_str.green().bold(),
		300..=399 => status_str.cyan().bold(),
		400..=499 => status_str.yellow().bold(),
		500..=599 => status_str.red().bold(),
		_ => status_str.white(),
	}
}

/// Format error details in multi-line format for better readability
///
/// This function uses structured error data when available (e.g., ParamValidation),
/// otherwise falls back to simple string formatting.
fn format_error_multiline(
	err: &reinhardt_core::exception::Error,
	include_raw_values: bool,
) -> String {
	use reinhardt_core::exception::Error;

	match err {
		// ParamValidation: Use structured context for detailed formatting
		Error::ParamValidation(ctx) => ctx.format_multiline(include_raw_values),

		// All other errors: Simple indented format
		_ => format!("  {}", err),
	}
}
