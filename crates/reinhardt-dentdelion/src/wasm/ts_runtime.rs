//! JavaScript Runtime for SSR
//!
//! This module provides a pure Rust JavaScript runtime using boa_engine,
//! enabling React/Preact SSR without external runtime dependencies.
//!
//! # Architecture
//!
//! The JavaScript runtime runs on the **host side** (not inside WASM plugins).
//! WASM plugins request SSR through host functions, and the host executes
//! JavaScript using this runtime.
//!
//! # Thread Safety
//!
//! `boa_engine::Context` is intentionally `!Send + !Sync` because it may rely
//! on thread-local state. To provide safe multi-threaded access, `TsRuntime`
//! uses a dedicated thread that owns the `Context` and communicates via
//! `std::sync::mpsc` channels. This makes `TsRuntime` naturally `Send + Sync`
//! without requiring `unsafe impl`.
//!
//! # Features
//!
//! - Pure Rust ECMAScript engine (no V8, no native dependencies)
//! - Preact bundled locally for SSR
//! - Thread-safe via dedicated runtime thread and channel communication
//!
//! # Example
//!
//! ```ignore
//! use reinhardt_dentdelion::wasm::TsRuntime;
//!
//! let runtime = TsRuntime::new()?;
//!
//! // Evaluate simple JavaScript
//! let result: String = runtime.eval("(1 + 2).toString()")?;
//! assert_eq!(result, "3");
//!
//! // Render a Preact component
//! let component_code = r#"
//!     function Component(props) {
//!         return h('div', null, 'Hello, ' + props.name);
//!     }
//! "#;
//! let html = runtime.render_component(component_code, r#"{"name": "World"}"#)?;
//! assert!(html.contains("Hello, World"));
//! ```

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;
use std::time::Duration;

use boa_engine::{Context, JsError, JsValue, Source};

/// Default maximum JavaScript execution time per evaluation (5 seconds).
const DEFAULT_EXECUTION_TIMEOUT: Duration = Duration::from_secs(5);

/// Default maximum JavaScript source code size in bytes (1 MiB).
const DEFAULT_MAX_SOURCE_SIZE: usize = 1024 * 1024;

/// Resource limits for JavaScript execution.
///
/// Controls CPU and memory usage to prevent denial-of-service from
/// malicious or buggy JavaScript code.
#[derive(Debug, Clone)]
pub struct TsResourceLimits {
	/// Maximum execution time per evaluation.
	pub execution_timeout: Duration,
	/// Maximum source code size in bytes.
	pub max_source_size: usize,
}

impl Default for TsResourceLimits {
	fn default() -> Self {
		Self {
			execution_timeout: DEFAULT_EXECUTION_TIMEOUT,
			max_source_size: DEFAULT_MAX_SOURCE_SIZE,
		}
	}
}

/// Preact core library (minified)
const PREACT_CORE: &str = include_str!("js/preact.min.js");

/// Preact render-to-string library (minified)
const PREACT_RENDER_TO_STRING: &str = include_str!("js/preact-render-to-string.min.js");

/// Command sent to the dedicated JavaScript runtime thread.
///
/// Each variant carries a response channel for returning the result
/// back to the calling thread.
enum RuntimeCommand {
	/// Evaluate JavaScript code and return the result as a string.
	Eval {
		code: String,
		response_tx: mpsc::Sender<Result<String, TsError>>,
	},
	/// Evaluate JavaScript code without expecting a return value.
	EvalVoid {
		code: String,
		response_tx: mpsc::Sender<Result<(), TsError>>,
	},
	/// Render a Preact component to HTML string.
	RenderComponent {
		component_code: String,
		props_json: String,
		response_tx: mpsc::Sender<Result<String, TsError>>,
	},
}

/// JavaScript runtime for SSR.
///
/// This runtime provides a sandboxed JavaScript execution environment
/// using boa_engine (pure Rust ECMAScript implementation).
///
/// # Thread Safety
///
/// `boa_engine::Context` is `!Send + !Sync` by design. Instead of using
/// `unsafe impl Send/Sync`, this struct uses a dedicated thread that owns
/// the `Context` and communicates via `std::sync::mpsc::SyncSender`.
/// `SyncSender<T>` is `Send + Sync` when `T: Send`, making `TsRuntime`
/// safely shareable across threads without `unsafe`.
pub struct TsRuntime {
	/// Channel to send commands to the dedicated runtime thread.
	///
	/// `SyncSender` is both `Send` and `Sync` (when `T: Send`),
	/// which makes `TsRuntime` naturally `Send + Sync`.
	command_tx: SyncSender<RuntimeCommand>,
	/// Whether Preact has been initialized (set once during construction)
	preact_initialized: bool,
	/// Resource limits for JavaScript execution
	limits: TsResourceLimits,
}

impl TsRuntime {
	/// Create a new JavaScript runtime with Preact pre-loaded and default resource limits.
	///
	/// This spawns a dedicated thread that owns the `boa_engine::Context`
	/// and initializes Preact libraries. The `Context` never leaves this
	/// thread, ensuring safe single-threaded access.
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - Runtime thread creation fails
	/// - Preact initialization fails
	pub fn new() -> Result<Self, TsError> {
		Self::with_limits(TsResourceLimits::default())
	}

	/// Create a new JavaScript runtime with custom resource limits.
	///
	/// # Arguments
	///
	/// * `limits` - Resource limits controlling execution timeout and source size
	///
	/// # Errors
	///
	/// Returns an error if:
	/// - Runtime thread creation fails
	/// - Preact initialization fails
	pub fn with_limits(limits: TsResourceLimits) -> Result<Self, TsError> {
		// Bounded channel for command passing (backpressure at 16 pending commands)
		let (command_tx, command_rx) = mpsc::sync_channel::<RuntimeCommand>(16);

		// Oneshot channel for initialization result
		let (init_tx, init_rx) = mpsc::channel::<Result<(), TsError>>();

		// Spawn dedicated thread for boa_engine::Context.
		// Context is !Send + !Sync, so it must stay on this single thread.
		thread::spawn(move || {
			runtime_thread_main(command_rx, init_tx);
		});

		// Wait for initialization result from the dedicated thread
		init_rx.recv().map_err(|_| {
			TsError::InitFailed("Runtime thread terminated during initialization".to_string())
		})??;

		Ok(Self {
			command_tx,
			preact_initialized: true,
			limits,
		})
	}

	/// Evaluate JavaScript code and return the result as a string.
	///
	/// # Arguments
	///
	/// * `code` - JavaScript code to evaluate
	///
	/// # Example
	///
	/// ```ignore
	/// let result = runtime.eval("(1 + 2).toString()")?;
	/// assert_eq!(result, "3");
	/// ```
	pub fn eval(&self, code: &str) -> Result<String, TsError> {
		self.check_source_size(code)?;

		let (response_tx, response_rx) = mpsc::channel();
		self.command_tx
			.send(RuntimeCommand::Eval {
				code: code.to_string(),
				response_tx,
			})
			.map_err(|_| TsError::EvalFailed("Runtime thread is not available".to_string()))?;

		response_rx
			.recv_timeout(self.limits.execution_timeout)
			.map_err(|e| match e {
				mpsc::RecvTimeoutError::Timeout => TsError::ExecutionTimeout {
					timeout: self.limits.execution_timeout,
				},
				mpsc::RecvTimeoutError::Disconnected => {
					TsError::EvalFailed("Runtime thread terminated during evaluation".to_string())
				}
			})?
	}

	/// Evaluate JavaScript code without expecting a return value.
	///
	/// Useful for executing side-effect code like defining functions.
	pub fn eval_void(&self, code: &str) -> Result<(), TsError> {
		self.check_source_size(code)?;

		let (response_tx, response_rx) = mpsc::channel();
		self.command_tx
			.send(RuntimeCommand::EvalVoid {
				code: code.to_string(),
				response_tx,
			})
			.map_err(|_| TsError::EvalFailed("Runtime thread is not available".to_string()))?;

		response_rx
			.recv_timeout(self.limits.execution_timeout)
			.map_err(|e| match e {
				mpsc::RecvTimeoutError::Timeout => TsError::ExecutionTimeout {
					timeout: self.limits.execution_timeout,
				},
				mpsc::RecvTimeoutError::Disconnected => {
					TsError::EvalFailed("Runtime thread terminated during evaluation".to_string())
				}
			})?
	}

	/// Render a Preact/React component to HTML string.
	///
	/// # Arguments
	///
	/// * `component_code` - JavaScript code defining the component.
	///   Must define a function named `Component`.
	/// * `props_json` - JSON string representing component props
	///
	/// # Component Code Format
	///
	/// The component code should define a `Component` function:
	///
	/// ```javascript
	/// function Component(props) {
	///     return h('div', { class: 'container' },
	///         h('h1', null, props.title),
	///         h('p', null, props.content)
	///     );
	/// }
	/// ```
	///
	/// # Example
	///
	/// ```ignore
	/// let component = r#"
	///     function Component(props) {
	///         return h('div', null, 'Hello, ' + props.name);
	///     }
	/// "#;
	/// let html = runtime.render_component(component, r#"{"name": "World"}"#)?;
	/// assert!(html.contains("Hello, World"));
	/// ```
	pub fn render_component(
		&self,
		component_code: &str,
		props_json: &str,
	) -> Result<String, TsError> {
		if !self.preact_initialized {
			return Err(TsError::InitFailed("Preact not initialized".to_string()));
		}

		// Check combined size of component code and props
		let total_size = component_code.len().saturating_add(props_json.len());
		if total_size > self.limits.max_source_size {
			return Err(TsError::SourceTooLarge {
				size: total_size,
				max: self.limits.max_source_size,
			});
		}

		let (response_tx, response_rx) = mpsc::channel();
		self.command_tx
			.send(RuntimeCommand::RenderComponent {
				component_code: component_code.to_string(),
				props_json: props_json.to_string(),
				response_tx,
			})
			.map_err(|_| TsError::RenderFailed("Runtime thread is not available".to_string()))?;

		response_rx
			.recv_timeout(self.limits.execution_timeout)
			.map_err(|e| match e {
				mpsc::RecvTimeoutError::Timeout => TsError::ExecutionTimeout {
					timeout: self.limits.execution_timeout,
				},
				mpsc::RecvTimeoutError::Disconnected => {
					TsError::RenderFailed("Runtime thread terminated during rendering".to_string())
				}
			})?
	}

	/// Check if Preact is initialized and ready for SSR.
	pub fn is_preact_ready(&self) -> bool {
		self.preact_initialized
	}

	/// Get the current resource limits.
	pub fn limits(&self) -> &TsResourceLimits {
		&self.limits
	}

	/// Validate source code size against the configured limit.
	fn check_source_size(&self, code: &str) -> Result<(), TsError> {
		if code.len() > self.limits.max_source_size {
			return Err(TsError::SourceTooLarge {
				size: code.len(),
				max: self.limits.max_source_size,
			});
		}
		Ok(())
	}
}

/// Main function for the dedicated JavaScript runtime thread.
///
/// This thread owns the `boa_engine::Context` (which is `!Send + !Sync`)
/// and processes commands received via channel. The context never leaves
/// this thread, ensuring safe single-threaded access without `unsafe`.
fn runtime_thread_main(
	command_rx: Receiver<RuntimeCommand>,
	init_tx: mpsc::Sender<Result<(), TsError>>,
) {
	let mut context = Context::default();

	// Initialize Preact libraries
	let init_result = init_preact(&mut context);
	let init_ok = init_result.is_ok();

	// Send initialization result back to the caller
	if init_tx.send(init_result).is_err() {
		// Caller dropped the receiver; exit the thread
		return;
	}

	if !init_ok {
		// Initialization failed; exit the thread
		return;
	}

	// Process commands until the channel is closed (all senders dropped)
	while let Ok(command) = command_rx.recv() {
		match command {
			RuntimeCommand::Eval { code, response_tx } => {
				let result = eval_code(&code, &mut context);
				let _ = response_tx.send(result);
			}
			RuntimeCommand::EvalVoid { code, response_tx } => {
				let result = eval_void_code(&code, &mut context);
				let _ = response_tx.send(result);
			}
			RuntimeCommand::RenderComponent {
				component_code,
				props_json,
				response_tx,
			} => {
				let result = render_component_code(&component_code, &props_json, &mut context);
				let _ = response_tx.send(result);
			}
		}
	}
}

/// Initialize Preact libraries in the JavaScript context.
fn init_preact(context: &mut Context) -> Result<(), TsError> {
	// Load Preact core
	context.eval(Source::from_bytes(PREACT_CORE)).map_err(|e| {
		TsError::InitFailed(format!(
			"Failed to load Preact core: {}",
			js_error_to_string(&e, context)
		))
	})?;

	// Load Preact render-to-string
	context
		.eval(Source::from_bytes(PREACT_RENDER_TO_STRING))
		.map_err(|e| {
			TsError::InitFailed(format!(
				"Failed to load Preact render-to-string: {}",
				js_error_to_string(&e, context)
			))
		})?;

	// Create global bindings for convenience
	// This makes `h`, `Fragment`, and `renderToString` available directly in the global scope
	const GLOBAL_BINDINGS: &str = r#"
		var h = preact.h;
		var Fragment = preact.Fragment;
		var createElement = preact.createElement;
		var renderToString = preactRenderToString.renderToString;
		var renderToStaticMarkup = preactRenderToString.renderToStaticMarkup;
	"#;
	context
		.eval(Source::from_bytes(GLOBAL_BINDINGS))
		.map_err(|e| {
			TsError::InitFailed(format!(
				"Failed to create global bindings: {}",
				js_error_to_string(&e, context)
			))
		})?;

	Ok(())
}

/// Evaluate JavaScript code and return the result as a string.
fn eval_code(code: &str, context: &mut Context) -> Result<String, TsError> {
	let result = context
		.eval(Source::from_bytes(code))
		.map_err(|e| TsError::EvalFailed(js_error_to_string(&e, context)))?;

	js_value_to_string(&result, context)
}

/// Evaluate JavaScript code without expecting a return value.
fn eval_void_code(code: &str, context: &mut Context) -> Result<(), TsError> {
	context
		.eval(Source::from_bytes(code))
		.map_err(|e| TsError::EvalFailed(js_error_to_string(&e, context)))?;
	Ok(())
}

/// Render a Preact component to HTML using the JavaScript context.
fn render_component_code(
	component_code: &str,
	props_json: &str,
	context: &mut Context,
) -> Result<String, TsError> {
	let render_script = format!(
		r#"
		(function() {{
			{component_code}
			var props = {props_json};
			return renderToString(h(Component, props));
		}})()
		"#
	);

	let result = context
		.eval(Source::from_bytes(&render_script))
		.map_err(|e| TsError::RenderFailed(js_error_to_string(&e, context)))?;

	js_value_to_string(&result, context)
}

/// Convert `JsValue` to String
fn js_value_to_string(value: &JsValue, context: &mut Context) -> Result<String, TsError> {
	value
		.to_string(context)
		.map(|s| s.to_std_string_escaped())
		.map_err(|e| TsError::EvalFailed(js_error_to_string(&e, context)))
}

/// Convert `JsError` to String
fn js_error_to_string(error: &JsError, context: &mut Context) -> String {
	error
		.to_opaque(context)
		.to_string(context)
		.map(|s| s.to_std_string_escaped())
		.unwrap_or_else(|_| "Unknown JavaScript error".to_string())
}

/// Shared JavaScript runtime instance type.
pub type SharedTsRuntime = Arc<TsRuntime>;

/// JavaScript runtime errors.
///
/// This enum is marked `#[non_exhaustive]` to allow adding new error variants
/// in future minor versions without breaking downstream code. Match arms
/// should include a wildcard pattern.
#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum TsError {
	/// JavaScript runtime initialization failed
	#[error("JavaScript runtime initialization failed: {0}")]
	InitFailed(String),

	/// JavaScript evaluation failed
	#[error("JavaScript evaluation failed: {0}")]
	EvalFailed(String),

	/// Props serialization failed
	#[error("Props serialization failed: {0}")]
	PropsSerialization(String),

	/// Component rendering failed
	#[error("Component rendering failed: {0}")]
	RenderFailed(String),

	/// JavaScript execution exceeded the configured timeout
	#[error("JavaScript execution timed out after {timeout:?}")]
	ExecutionTimeout {
		/// The timeout duration that was exceeded
		timeout: Duration,
	},

	/// JavaScript source code exceeded the configured size limit
	#[error("JavaScript source size ({size} bytes) exceeds limit ({max} bytes)")]
	SourceTooLarge {
		/// Actual source size in bytes
		size: usize,
		/// Maximum allowed size in bytes
		max: usize,
	},
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_ts_runtime_creation() {
		// Arrange & Act
		let runtime = TsRuntime::new();

		// Assert
		assert!(runtime.is_ok());
		assert!(runtime.unwrap().is_preact_ready());
	}

	#[rstest]
	fn test_eval_simple_expression() {
		// Arrange
		let runtime = TsRuntime::new().unwrap();

		// Act
		let result = runtime.eval("(1 + 2).toString()").unwrap();

		// Assert
		assert_eq!(result, "3");
	}

	#[rstest]
	fn test_eval_string_expression() {
		// Arrange
		let runtime = TsRuntime::new().unwrap();

		// Act
		let result = runtime.eval("'Hello, ' + 'World'").unwrap();

		// Assert
		assert_eq!(result, "Hello, World");
	}

	#[rstest]
	fn test_eval_void() {
		// Arrange
		let runtime = TsRuntime::new().unwrap();

		// Act & Assert
		runtime.eval_void("var x = 42;").unwrap();
	}

	#[rstest]
	fn test_render_simple_component() {
		// Arrange
		let runtime = TsRuntime::new().unwrap();
		let component_code = r#"
			function Component(props) {
				return h('div', null, 'Hello, ' + props.name);
			}
		"#;
		let props = r#"{"name": "World"}"#;

		// Act
		let result = runtime.render_component(component_code, props);

		// Assert
		assert!(result.is_ok());
		let html = result.unwrap();
		assert!(html.contains("Hello, World"));
		assert!(html.contains("<div>"));
	}

	#[rstest]
	fn test_render_nested_component() {
		// Arrange
		let runtime = TsRuntime::new().unwrap();
		let component_code = r#"
			function Child(props) {
				return h('span', null, props.text);
			}
			function Component(props) {
				return h('div', { class: 'container' },
					h(Child, { text: props.message })
				);
			}
		"#;
		let props = r#"{"message": "Nested!"}"#;

		// Act
		let result = runtime.render_component(component_code, props);

		// Assert
		assert!(result.is_ok());
		let html = result.unwrap();
		assert!(html.contains("container"));
		assert!(html.contains("Nested!"));
	}

	#[rstest]
	fn test_render_with_attributes() {
		// Arrange
		let runtime = TsRuntime::new().unwrap();
		let component_code = r#"
			function Component(props) {
				return h('a', { href: props.url, class: 'link' }, props.label);
			}
		"#;
		let props = r#"{"url": "https://example.com", "label": "Click me"}"#;

		// Act
		let result = runtime.render_component(component_code, props);

		// Assert
		assert!(result.is_ok());
		let html = result.unwrap();
		assert!(html.contains("href="));
		assert!(html.contains("Click me"));
	}

	/// Verify `TsRuntime` is `Send + Sync` at compile time.
	///
	/// This is the core safety property: because the `boa_engine::Context`
	/// is confined to a dedicated thread and `TsRuntime` only holds a
	/// `SyncSender` (which is `Send + Sync`), the struct is naturally
	/// thread-safe without `unsafe impl`.
	#[rstest]
	fn test_ts_runtime_is_send_and_sync() {
		fn assert_send<T: Send>() {}
		fn assert_sync<T: Sync>() {}
		assert_send::<TsRuntime>();
		assert_sync::<TsRuntime>();
	}

	/// Verify `SharedTsRuntime` (`Arc<TsRuntime>`) can be shared across threads.
	#[rstest]
	fn test_shared_ts_runtime_across_threads() {
		// Arrange
		let runtime = Arc::new(TsRuntime::new().unwrap());
		let runtime_clone = Arc::clone(&runtime);

		// Act: use from another thread
		let handle = std::thread::spawn(move || runtime_clone.eval("(2 + 3).toString()").unwrap());

		// Assert
		let result = handle.join().unwrap();
		assert_eq!(result, "5");
	}

	// ==========================================================================
	// Resource Limits Tests (#690)
	// ==========================================================================

	#[rstest]
	fn test_source_size_limit_rejects_oversized_code() {
		// Arrange
		let limits = TsResourceLimits {
			max_source_size: 64,
			..TsResourceLimits::default()
		};
		let runtime = TsRuntime::with_limits(limits).unwrap();
		let oversized_code = "a".repeat(128);

		// Act
		let result = runtime.eval(&oversized_code);

		// Assert
		assert!(matches!(
			result,
			Err(TsError::SourceTooLarge { size: 128, max: 64 })
		));
	}

	#[rstest]
	fn test_source_size_limit_allows_within_limit() {
		// Arrange
		let limits = TsResourceLimits {
			max_source_size: 256,
			..TsResourceLimits::default()
		};
		let runtime = TsRuntime::with_limits(limits).unwrap();

		// Act
		let result = runtime.eval("(1 + 1).toString()");

		// Assert
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), "2");
	}

	#[rstest]
	fn test_eval_void_source_size_limit() {
		// Arrange
		let limits = TsResourceLimits {
			max_source_size: 32,
			..TsResourceLimits::default()
		};
		let runtime = TsRuntime::with_limits(limits).unwrap();
		let oversized_code = "var x = ".to_string() + &"0".repeat(64);

		// Act
		let result = runtime.eval_void(&oversized_code);

		// Assert
		assert!(matches!(result, Err(TsError::SourceTooLarge { .. })));
	}

	#[rstest]
	fn test_render_component_source_size_limit() {
		// Arrange
		let limits = TsResourceLimits {
			max_source_size: 64,
			..TsResourceLimits::default()
		};
		let runtime = TsRuntime::with_limits(limits).unwrap();
		let oversized_component = "a".repeat(128);

		// Act
		let result = runtime.render_component(&oversized_component, "{}");

		// Assert
		assert!(matches!(result, Err(TsError::SourceTooLarge { .. })));
	}

	#[rstest]
	fn test_execution_timeout_rejects_long_running_code() {
		// Arrange: use a very short timeout
		let limits = TsResourceLimits {
			execution_timeout: Duration::from_millis(100),
			max_source_size: DEFAULT_MAX_SOURCE_SIZE,
		};
		let runtime = TsRuntime::with_limits(limits).unwrap();
		// Intentionally long-running JavaScript (busy loop)
		let infinite_loop = "var i = 0; while(true) { i++; }";

		// Act
		let result = runtime.eval(infinite_loop);

		// Assert
		assert!(matches!(result, Err(TsError::ExecutionTimeout { .. })));
	}

	#[rstest]
	fn test_custom_limits_are_preserved() {
		// Arrange
		let limits = TsResourceLimits {
			execution_timeout: Duration::from_secs(10),
			max_source_size: 512,
		};

		// Act
		let runtime = TsRuntime::with_limits(limits).unwrap();

		// Assert
		assert_eq!(runtime.limits().execution_timeout, Duration::from_secs(10));
		assert_eq!(runtime.limits().max_source_size, 512);
	}

	#[rstest]
	fn test_default_limits() {
		// Arrange & Act
		let limits = TsResourceLimits::default();

		// Assert
		assert_eq!(limits.execution_timeout, DEFAULT_EXECUTION_TIMEOUT);
		assert_eq!(limits.max_source_size, DEFAULT_MAX_SOURCE_SIZE);
	}
}
