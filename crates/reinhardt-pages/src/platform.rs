//! Platform abstraction for WASM and native targets.
//!
//! This module provides unified type aliases that work across both WASM and native platforms,
//! reducing the need for conditional compilation in user code.
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_pages::platform::Event;
//!
//! // Works on both WASM and native targets
//! fn handle_click(event: Event) {
//!     // Event handling logic
//! }
//! ```

/// Platform-specific types for WASM targets.
#[cfg(wasm)]
mod inner {
	/// DOM Event type (web_sys::Event on WASM).
	pub type Event = web_sys::Event;

	/// Window object type (web_sys::Window on WASM).
	pub type Window = web_sys::Window;

	/// HTML input element type (web_sys::HtmlInputElement on WASM).
	pub type HtmlInputElement = web_sys::HtmlInputElement;

	/// HTML textarea element type (web_sys::HtmlTextAreaElement on WASM).
	pub type HtmlTextAreaElement = web_sys::HtmlTextAreaElement;

	/// HTML select element type (web_sys::HtmlSelectElement on WASM).
	pub type HtmlSelectElement = web_sys::HtmlSelectElement;

	/// HTML form element type (web_sys::HtmlFormElement on WASM).
	pub type HtmlFormElement = web_sys::HtmlFormElement;

	/// HTML button element type (web_sys::HtmlButtonElement on WASM).
	pub type HtmlButtonElement = web_sys::HtmlButtonElement;
}

/// Platform-specific types for native (non-WASM) targets.
///
/// These are stub types that maintain API compatibility with WASM code
/// without requiring web-sys dependencies on native targets.
#[cfg(native)]
mod inner {
	pub use crate::component::DummyEvent as Event;

	/// Stub Window type for SSR compatibility.
	#[derive(Debug, Clone, Default)]
	pub struct Window;

	/// Stub HtmlInputElement for SSR compatibility.
	#[derive(Debug, Clone, Default)]
	pub struct HtmlInputElement {
		value: String,
	}

	impl HtmlInputElement {
		/// Get the input value.
		pub fn value(&self) -> String {
			self.value.clone()
		}

		/// Set the input value.
		pub fn set_value(&mut self, value: &str) {
			self.value = value.to_string();
		}
	}

	/// Stub HtmlTextAreaElement for SSR compatibility.
	#[derive(Debug, Clone, Default)]
	pub struct HtmlTextAreaElement {
		value: String,
	}

	impl HtmlTextAreaElement {
		/// Get the textarea value.
		pub fn value(&self) -> String {
			self.value.clone()
		}

		/// Set the textarea value.
		pub fn set_value(&mut self, value: &str) {
			self.value = value.to_string();
		}
	}

	/// Stub HtmlSelectElement for SSR compatibility.
	#[derive(Debug, Clone, Default)]
	pub struct HtmlSelectElement {
		value: String,
	}

	impl HtmlSelectElement {
		/// Get the selected value.
		pub fn value(&self) -> String {
			self.value.clone()
		}

		/// Set the selected value.
		pub fn set_value(&mut self, value: &str) {
			self.value = value.to_string();
		}
	}

	/// Stub HtmlFormElement for SSR compatibility.
	#[derive(Debug, Clone, Default)]
	pub struct HtmlFormElement;

	/// Stub HtmlButtonElement for SSR compatibility.
	#[derive(Debug, Clone, Default)]
	pub struct HtmlButtonElement;
}

pub use inner::*;
