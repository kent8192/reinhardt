//! Logging abstraction layer for reinhardt-pages
//!
//! This module provides logging macros that work seamlessly across WASM and native targets.
//! All macros are no-ops in release builds for zero production overhead.
//!
//! ## Macro Overview
//!
//! | Macro | Debug Assertions | Feature Required | WASM | Non-WASM |
//! |-------|------------------|------------------|------|----------|
//! | `debug_log!` | Required | `debug-hooks` | `console.debug` | `eprintln!` |
//! | `info_log!` | Required | None | `console.info` | `eprintln!` |
//! | `warn_log!` | Required | None | `console.warn` | `eprintln!` |
//! | `error_log!` | Required | None | `console.error` | `eprintln!` |
//!
//! ## Example
//!
//! ```ignore
//! use reinhardt_pages::{debug_log, info_log, warn_log, error_log};
//!
//! // Only logged when both `debug-hooks` feature and `debug_assertions` are enabled
//! debug_log!("Hook state: {:?}", hook_state);
//!
//! // Logged when `debug_assertions` are enabled
//! info_log!("Component mounted");
//! warn_log!("Performance warning: render took {}ms", time);
//! error_log!("Failed to validate: {}", error);
//! ```

/// Logs a debug message (requires `debug-hooks` feature + `debug_assertions`)
///
/// This macro is specifically for debug hooks and internal debugging.
/// It compiles to a no-op when conditions are not met.
///
/// # Arguments
///
/// Takes format arguments similar to `format!` or `println!`.
///
/// # Example
///
/// ```ignore
/// debug_log!("Debug value: {:?}", value);
/// ```
#[macro_export]
#[cfg(all(debug_assertions, feature = "debug-hooks", wasm))]
macro_rules! debug_log {
	($($arg:tt)*) => {{
		web_sys::console::debug_1(&format!($($arg)*).into());
	}};
}

/// Logs a debug message (requires `debug-hooks` feature + `debug_assertions`)
#[macro_export]
#[cfg(all(debug_assertions, feature = "debug-hooks", native))]
macro_rules! debug_log {
	($($arg:tt)*) => {{
		eprintln!("[DEBUG] {}", format!($($arg)*));
	}};
}

/// No-op debug_log when conditions are not met
#[macro_export]
#[cfg(not(all(debug_assertions, feature = "debug-hooks")))]
macro_rules! debug_log {
	($($arg:tt)*) => {{}};
}

/// Logs an info message (requires `debug_assertions`)
///
/// This macro is for general informational logging during development.
/// It compiles to a no-op in release builds.
///
/// # Arguments
///
/// Takes format arguments similar to `format!` or `println!`.
///
/// # Example
///
/// ```no_run
/// info_log!("Form submitted successfully");
/// ```
#[macro_export]
#[cfg(all(debug_assertions, wasm))]
macro_rules! info_log {
	($($arg:tt)*) => {{
		web_sys::console::info_1(&format!($($arg)*).into());
	}};
}

/// Logs an info message (requires `debug_assertions`)
#[macro_export]
#[cfg(all(debug_assertions, native))]
macro_rules! info_log {
	($($arg:tt)*) => {{
		eprintln!("[INFO] {}", format!($($arg)*));
	}};
}

/// No-op info_log in release builds
#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! info_log {
	($($arg:tt)*) => {{}};
}

/// Logs a warning message (requires `debug_assertions`)
///
/// This macro is for warning messages during development.
/// It compiles to a no-op in release builds.
///
/// # Arguments
///
/// Takes format arguments similar to `format!` or `println!`.
///
/// # Example
///
/// ```no_run
/// warn_log!("Performance warning: slow render");
/// ```
#[macro_export]
#[cfg(all(debug_assertions, wasm))]
macro_rules! warn_log {
	($($arg:tt)*) => {{
		web_sys::console::warn_1(&format!($($arg)*).into());
	}};
}

/// Logs a warning message (requires `debug_assertions`)
#[macro_export]
#[cfg(all(debug_assertions, native))]
macro_rules! warn_log {
	($($arg:tt)*) => {{
		eprintln!("[WARN] {}", format!($($arg)*));
	}};
}

/// No-op warn_log in release builds
#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! warn_log {
	($($arg:tt)*) => {{}};
}

/// Logs an error message (requires `debug_assertions`)
///
/// This macro is for error messages during development.
/// It compiles to a no-op in release builds.
///
/// # Arguments
///
/// Takes format arguments similar to `format!` or `println!`.
///
/// # Example
///
/// ```ignore
/// error_log!("Submit failed: {:?}", error);
/// ```
#[macro_export]
#[cfg(all(debug_assertions, wasm))]
macro_rules! error_log {
	($($arg:tt)*) => {{
		web_sys::console::error_1(&format!($($arg)*).into());
	}};
}

/// Logs an error message (requires `debug_assertions`)
#[macro_export]
#[cfg(all(debug_assertions, native))]
macro_rules! error_log {
	($($arg:tt)*) => {{
		eprintln!("[ERROR] {}", format!($($arg)*));
	}};
}

/// No-op error_log in release builds
#[macro_export]
#[cfg(not(debug_assertions))]
macro_rules! error_log {
	($($arg:tt)*) => {{}};
}

#[cfg(test)]
mod tests {
	// Import macros from crate root
	#[allow(unused_imports)]
	use crate::{debug_log, error_log, info_log, warn_log};

	#[test]
	fn test_logging_macros_compile() {
		// These should compile without errors
		debug_log!("Debug message: {}", 42);
		info_log!("Info message: {}", "test");
		warn_log!("Warning message: {:?}", vec![1, 2, 3]);
		error_log!("Error message: {}", "error");
	}

	#[test]
	fn test_logging_macros_no_args() {
		// Macros should work without format arguments
		debug_log!("Simple debug");
		info_log!("Simple info");
		warn_log!("Simple warning");
		error_log!("Simple error");
	}
}
