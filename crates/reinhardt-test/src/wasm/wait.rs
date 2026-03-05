#![cfg(target_arch = "wasm32")]

//! Async utilities for WASM testing.
//!
//! This module provides async waiting utilities for handling asynchronous operations
//! in WASM tests, including condition-based waiting, element visibility waiting,
//! and microtask/effect flushing.
//!
//! # Example
//!
//! ```ignore
//! use reinhardt_test::wasm::wait::{wait_for, sleep, WaitOptions};
//!
//! // Wait for a condition
//! wait_for(|| document.get_element_by_id("result").is_some())
//!     .await
//!     .expect("Element should appear");
//!
//! // Wait with custom timeout
//! wait_for(|| some_condition())
//!     .with_timeout(Duration::from_secs(5))
//!     .await?;
//!
//! // Simple sleep
//! sleep(Duration::from_millis(100)).await;
//! ```

#![cfg(target_arch = "wasm32")]

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element, Window};

/// Default timeout for wait operations (5 seconds).
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Default interval for polling conditions (50ms).
pub const DEFAULT_INTERVAL: Duration = Duration::from_millis(50);

/// Error type for wait operations.
#[derive(Debug, Clone)]
pub enum WaitError {
	/// The operation timed out.
	Timeout {
		/// The timeout duration that was exceeded.
		timeout: Duration,
		/// Optional description of what was being waited for.
		description: Option<String>,
	},
	/// A JavaScript error occurred.
	JsError(String),
	/// The element was not found.
	ElementNotFound(String),
}

impl std::fmt::Display for WaitError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			WaitError::Timeout {
				timeout,
				description,
			} => {
				if let Some(desc) = description {
					write!(f, "Timed out after {:?} waiting for: {}", timeout, desc)
				} else {
					write!(f, "Timed out after {:?}", timeout)
				}
			}
			WaitError::JsError(msg) => write!(f, "JavaScript error: {}", msg),
			WaitError::ElementNotFound(selector) => {
				write!(f, "Element not found: {}", selector)
			}
		}
	}
}

impl std::error::Error for WaitError {}

/// Result type for wait operations.
pub type WaitResult<T> = Result<T, WaitError>;

/// Options for configuring wait behavior.
#[derive(Debug, Clone)]
pub struct WaitOptions {
	/// Maximum time to wait before timing out.
	pub timeout: Duration,
	/// Interval between condition checks.
	pub interval: Duration,
	/// Optional description for error messages.
	pub description: Option<String>,
}

impl Default for WaitOptions {
	fn default() -> Self {
		Self {
			timeout: DEFAULT_TIMEOUT,
			interval: DEFAULT_INTERVAL,
			description: None,
		}
	}
}

impl WaitOptions {
	/// Create new wait options with default values.
	pub fn new() -> Self {
		Self::default()
	}

	/// Set the timeout duration.
	pub fn with_timeout(mut self, timeout: Duration) -> Self {
		self.timeout = timeout;
		self
	}

	/// Set the polling interval.
	pub fn with_interval(mut self, interval: Duration) -> Self {
		self.interval = interval;
		self
	}

	/// Set a description for error messages.
	pub fn with_description(mut self, description: impl Into<String>) -> Self {
		self.description = Some(description.into());
		self
	}
}

/// Builder for wait_for operations with fluent API.
pub struct WaitForBuilder<F>
where
	F: FnMut() -> bool,
{
	condition: F,
	options: WaitOptions,
}

impl<F> WaitForBuilder<F>
where
	F: FnMut() -> bool,
{
	/// Create a new wait builder with the given condition.
	pub fn new(condition: F) -> Self {
		Self {
			condition,
			options: WaitOptions::default(),
		}
	}

	/// Set the timeout duration.
	pub fn with_timeout(mut self, timeout: Duration) -> Self {
		self.options.timeout = timeout;
		self
	}

	/// Set the polling interval.
	pub fn with_interval(mut self, interval: Duration) -> Self {
		self.options.interval = interval;
		self
	}

	/// Set a description for error messages.
	pub fn with_description(mut self, description: impl Into<String>) -> Self {
		self.options.description = Some(description.into());
		self
	}

	/// Execute the wait operation.
	pub async fn await_condition(mut self) -> WaitResult<()> {
		let start = js_sys::Date::now();
		let timeout_ms = self.options.timeout.as_millis() as f64;
		let interval_ms = self.options.interval.as_millis() as u32;

		loop {
			// Check the condition
			if (self.condition)() {
				return Ok(());
			}

			// Check timeout
			let elapsed = js_sys::Date::now() - start;
			if elapsed >= timeout_ms {
				return Err(WaitError::Timeout {
					timeout: self.options.timeout,
					description: self.options.description.clone(),
				});
			}

			// Wait for interval
			TimeoutFuture::new(interval_ms).await;
		}
	}
}

impl<F> std::future::IntoFuture for WaitForBuilder<F>
where
	F: FnMut() -> bool,
{
	type Output = WaitResult<()>;
	type IntoFuture = Pin<Box<dyn Future<Output = Self::Output>>>;

	fn into_future(self) -> Self::IntoFuture {
		Box::pin(self.await_condition())
	}
}

/// Wait for a condition to become true.
///
/// Returns a builder that can be configured with timeout and interval options.
///
/// # Example
///
/// ```ignore
/// use reinhardt_test::wasm::wait::wait_for;
/// use std::time::Duration;
///
/// // Simple usage
/// wait_for(|| some_condition()).await?;
///
/// // With custom timeout
/// wait_for(|| some_condition())
///     .with_timeout(Duration::from_secs(10))
///     .await?;
/// ```
pub fn wait_for<F>(condition: F) -> WaitForBuilder<F>
where
	F: FnMut() -> bool,
{
	WaitForBuilder::new(condition)
}

/// Wait for an element to become visible in the DOM.
///
/// # Arguments
///
/// * `selector` - CSS selector for the element
/// * `options` - Optional wait configuration
///
/// # Example
///
/// ```ignore
/// use reinhardt_test::wasm::wait::wait_for_element_visible;
///
/// let element = wait_for_element_visible("#loading-spinner", None).await?;
/// ```
pub async fn wait_for_element_visible(
	selector: &str,
	options: Option<WaitOptions>,
) -> WaitResult<Element> {
	let opts = options.unwrap_or_else(|| {
		WaitOptions::default().with_description(format!("element '{}' to be visible", selector))
	});

	let selector_owned = selector.to_string();
	let document = get_document()?;

	let start = js_sys::Date::now();
	let timeout_ms = opts.timeout.as_millis() as f64;
	let interval_ms = opts.interval.as_millis() as u32;

	loop {
		if let Some(element) = document.query_selector(&selector_owned).ok().flatten() {
			if is_element_visible(&element) {
				return Ok(element);
			}
		}

		let elapsed = js_sys::Date::now() - start;
		if elapsed >= timeout_ms {
			return Err(WaitError::Timeout {
				timeout: opts.timeout,
				description: opts.description.clone(),
			});
		}

		TimeoutFuture::new(interval_ms).await;
	}
}

/// Wait for an element to become hidden or removed from the DOM.
///
/// # Arguments
///
/// * `selector` - CSS selector for the element
/// * `options` - Optional wait configuration
///
/// # Example
///
/// ```ignore
/// use reinhardt_test::wasm::wait::wait_for_element_hidden;
///
/// wait_for_element_hidden("#loading-spinner", None).await?;
/// ```
pub async fn wait_for_element_hidden(
	selector: &str,
	options: Option<WaitOptions>,
) -> WaitResult<()> {
	let opts = options.unwrap_or_else(|| {
		WaitOptions::default().with_description(format!("element '{}' to be hidden", selector))
	});

	let selector_owned = selector.to_string();
	let document = get_document()?;

	let start = js_sys::Date::now();
	let timeout_ms = opts.timeout.as_millis() as f64;
	let interval_ms = opts.interval.as_millis() as u32;

	loop {
		let element = document.query_selector(&selector_owned).ok().flatten();

		// Element is hidden if it doesn't exist or is not visible
		if element.is_none() || !is_element_visible(element.as_ref().unwrap()) {
			return Ok(());
		}

		let elapsed = js_sys::Date::now() - start;
		if elapsed >= timeout_ms {
			return Err(WaitError::Timeout {
				timeout: opts.timeout,
				description: opts.description.clone(),
			});
		}

		TimeoutFuture::new(interval_ms).await;
	}
}

/// Wait for an element matching the selector to appear in the DOM.
///
/// This doesn't check visibility, only presence in the DOM.
///
/// # Arguments
///
/// * `selector` - CSS selector for the element
/// * `options` - Optional wait configuration
pub async fn wait_for_element(selector: &str, options: Option<WaitOptions>) -> WaitResult<Element> {
	let opts = options.unwrap_or_else(|| {
		WaitOptions::default().with_description(format!("element '{}' to appear in DOM", selector))
	});

	let selector_owned = selector.to_string();
	let document = get_document()?;

	let start = js_sys::Date::now();
	let timeout_ms = opts.timeout.as_millis() as f64;
	let interval_ms = opts.interval.as_millis() as u32;

	loop {
		if let Some(element) = document.query_selector(&selector_owned).ok().flatten() {
			return Ok(element);
		}

		let elapsed = js_sys::Date::now() - start;
		if elapsed >= timeout_ms {
			return Err(WaitError::Timeout {
				timeout: opts.timeout,
				description: opts.description.clone(),
			});
		}

		TimeoutFuture::new(interval_ms).await;
	}
}

/// Wait for an element to be removed from the DOM.
///
/// # Arguments
///
/// * `selector` - CSS selector for the element
/// * `options` - Optional wait configuration
pub async fn wait_for_element_removed(
	selector: &str,
	options: Option<WaitOptions>,
) -> WaitResult<()> {
	let opts = options.unwrap_or_else(|| {
		WaitOptions::default()
			.with_description(format!("element '{}' to be removed from DOM", selector))
	});

	let selector_owned = selector.to_string();
	let document = get_document()?;

	let start = js_sys::Date::now();
	let timeout_ms = opts.timeout.as_millis() as f64;
	let interval_ms = opts.interval.as_millis() as u32;

	loop {
		if document
			.query_selector(&selector_owned)
			.ok()
			.flatten()
			.is_none()
		{
			return Ok(());
		}

		let elapsed = js_sys::Date::now() - start;
		if elapsed >= timeout_ms {
			return Err(WaitError::Timeout {
				timeout: opts.timeout,
				description: opts.description.clone(),
			});
		}

		TimeoutFuture::new(interval_ms).await;
	}
}

/// Sleep for the specified duration.
///
/// # Example
///
/// ```ignore
/// use reinhardt_test::wasm::wait::sleep;
/// use std::time::Duration;
///
/// sleep(Duration::from_millis(100)).await;
/// ```
pub async fn sleep(duration: Duration) {
	TimeoutFuture::new(duration.as_millis() as u32).await;
}

/// Flush all pending microtasks.
///
/// This uses `queueMicrotask` to schedule a callback that resolves when
/// the microtask queue is empty.
///
/// # Example
///
/// ```ignore
/// use reinhardt_test::wasm::wait::flush_microtasks;
///
/// // After triggering some async operation
/// flush_microtasks().await;
/// ```
pub async fn flush_microtasks() {
	// Use Promise.resolve() to queue a microtask
	let promise = js_sys::Promise::resolve(&JsValue::UNDEFINED);
	let _ = JsFuture::from(promise).await;
}

/// Flush pending effects and reactive updates.
///
/// This is useful when testing reactive components that use Signal/Effect/Memo.
/// It waits for a short duration to allow effects to propagate.
///
/// # Example
///
/// ```ignore
/// use reinhardt_test::wasm::wait::flush_effects;
///
/// signal.set(new_value);
/// flush_effects().await;
/// // Now check the DOM for updates
/// ```
pub async fn flush_effects() {
	// First flush microtasks
	flush_microtasks().await;

	// Then wait one animation frame to allow effects to run
	request_animation_frame().await;

	// Flush any microtasks queued by effects
	flush_microtasks().await;
}

/// Wait for the next animation frame.
///
/// This is useful for waiting for visual updates to be applied.
pub async fn request_animation_frame() {
	let window = get_window().expect("window should be available in WASM environment");

	let promise = js_sys::Promise::new(&mut |resolve, _reject| {
		let closure = Closure::once_into_js(move || {
			resolve
				.call0(&JsValue::UNDEFINED)
				.expect("Promise resolve callback should not fail");
		});
		window
			.request_animation_frame(closure.unchecked_ref())
			.expect("requestAnimationFrame should be available in browser environment");
	});

	let _ = JsFuture::from(promise).await;
}

/// Wait for multiple animation frames.
///
/// # Arguments
///
/// * `count` - Number of frames to wait
pub async fn wait_frames(count: u32) {
	for _ in 0..count {
		request_animation_frame().await;
	}
}

/// Wait for the DOM to stabilize (no mutations for the specified duration).
///
/// This is useful for waiting for complex animations or transitions to complete.
///
/// # Arguments
///
/// * `stability_duration` - Duration with no DOM mutations to consider stable
/// * `timeout` - Maximum time to wait
pub async fn wait_for_dom_stable(
	stability_duration: Duration,
	timeout: Duration,
) -> WaitResult<()> {
	let start = js_sys::Date::now();
	let timeout_ms = timeout.as_millis() as f64;
	let stability_ms = stability_duration.as_millis() as u32;

	// Use a simple polling approach - check if content hash changes
	let document = get_document()?;
	let mut last_content = get_body_content(&document);
	let mut stable_since = js_sys::Date::now();

	loop {
		let current_content = get_body_content(&document);

		if current_content != last_content {
			last_content = current_content;
			stable_since = js_sys::Date::now();
		} else {
			let stable_duration = js_sys::Date::now() - stable_since;
			if stable_duration >= stability_ms as f64 {
				return Ok(());
			}
		}

		let elapsed = js_sys::Date::now() - start;
		if elapsed >= timeout_ms {
			return Err(WaitError::Timeout {
				timeout,
				description: Some("DOM to stabilize".to_string()),
			});
		}

		TimeoutFuture::new(16).await; // ~60fps
	}
}

// Helper functions

fn get_window() -> WaitResult<Window> {
	web_sys::window().ok_or_else(|| WaitError::JsError("Window not available".to_string()))
}

fn get_document() -> WaitResult<Document> {
	get_window()?
		.document()
		.ok_or_else(|| WaitError::JsError("Document not available".to_string()))
}

fn is_element_visible(element: &Element) -> bool {
	// Check if element has display: none or visibility: hidden
	if let Some(html_element) = element.dyn_ref::<web_sys::HtmlElement>() {
		// Check offsetParent - if null, element is hidden
		if html_element.offset_parent().is_none() {
			// Exception: fixed/sticky positioned elements may have no offsetParent
			if let Ok(style) = get_window()
				.ok()
				.and_then(|w| w.get_computed_style(element).ok())
				.flatten()
				.ok_or(())
			{
				let position = style.get_property_value("position").unwrap_or_default();
				if position != "fixed" && position != "sticky" {
					return false;
				}
			} else {
				return false;
			}
		}

		// Check computed visibility
		if let Ok(Some(style)) = get_window()
			.ok()
			.and_then(|w| Some(w.get_computed_style(element)))
			.flatten()
		{
			let visibility = style.get_property_value("visibility").unwrap_or_default();
			let display = style.get_property_value("display").unwrap_or_default();

			if visibility == "hidden" || display == "none" {
				return false;
			}
		}

		return true;
	}

	// For non-HTML elements (SVG, etc.), assume visible if in DOM
	true
}

fn get_body_content(document: &Document) -> String {
	document.body().map(|b| b.inner_html()).unwrap_or_default()
}

/// Extension trait for adding wait functionality to elements.
pub trait ElementWaitExt {
	/// Wait for this element to become visible.
	fn wait_until_visible(&self, options: Option<WaitOptions>) -> WaitForVisibleFuture;

	/// Wait for this element to become hidden.
	fn wait_until_hidden(&self, options: Option<WaitOptions>) -> WaitForHiddenFuture;
}

impl ElementWaitExt for Element {
	fn wait_until_visible(&self, options: Option<WaitOptions>) -> WaitForVisibleFuture {
		WaitForVisibleFuture {
			element: self.clone(),
			options: options.unwrap_or_default(),
			started: false,
			start_time: 0.0,
			pending_closure: None,
		}
	}

	fn wait_until_hidden(&self, options: Option<WaitOptions>) -> WaitForHiddenFuture {
		WaitForHiddenFuture {
			element: self.clone(),
			options: options.unwrap_or_default(),
			started: false,
			start_time: 0.0,
			pending_closure: None,
		}
	}
}

/// Future for waiting until an element becomes visible.
// Fixes #877
pub struct WaitForVisibleFuture {
	element: Element,
	options: WaitOptions,
	started: bool,
	start_time: f64,
	/// Stored closure to prevent memory leak from `Closure::forget()`.
	/// The closure is kept alive until the timeout fires or the future completes.
	pending_closure: Option<Closure<dyn FnMut()>>,
}

impl Future for WaitForVisibleFuture {
	type Output = WaitResult<()>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		if !self.started {
			self.started = true;
			self.start_time = js_sys::Date::now();
		}

		if is_element_visible(&self.element) {
			return Poll::Ready(Ok(()));
		}

		let elapsed = js_sys::Date::now() - self.start_time;
		if elapsed >= self.options.timeout.as_millis() as f64 {
			return Poll::Ready(Err(WaitError::Timeout {
				timeout: self.options.timeout,
				description: self.options.description.clone(),
			}));
		}

		// Schedule a wake-up
		let waker = cx.waker().clone();
		let interval_ms = self.options.interval.as_millis() as i32;

		let closure = Closure::once(move || {
			waker.wake();
		});

		if let Ok(window) = get_window() {
			let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
				closure.as_ref().unchecked_ref(),
				interval_ms,
			);
		}

		// Store closure to keep it alive until the timeout fires,
		// instead of leaking it with `forget()`.
		self.pending_closure = Some(closure);

		Poll::Pending
	}
}

/// Future for waiting until an element becomes hidden.
// Fixes #877
pub struct WaitForHiddenFuture {
	element: Element,
	options: WaitOptions,
	started: bool,
	start_time: f64,
	/// Stored closure to prevent memory leak from `Closure::forget()`.
	/// The closure is kept alive until the timeout fires or the future completes.
	pending_closure: Option<Closure<dyn FnMut()>>,
}

impl Future for WaitForHiddenFuture {
	type Output = WaitResult<()>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		if !self.started {
			self.started = true;
			self.start_time = js_sys::Date::now();
		}

		if !is_element_visible(&self.element) {
			return Poll::Ready(Ok(()));
		}

		let elapsed = js_sys::Date::now() - self.start_time;
		if elapsed >= self.options.timeout.as_millis() as f64 {
			return Poll::Ready(Err(WaitError::Timeout {
				timeout: self.options.timeout,
				description: self.options.description.clone(),
			}));
		}

		// Schedule a wake-up
		let waker = cx.waker().clone();
		let interval_ms = self.options.interval.as_millis() as i32;

		let closure = Closure::once(move || {
			waker.wake();
		});

		if let Ok(window) = get_window() {
			let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
				closure.as_ref().unchecked_ref(),
				interval_ms,
			);
		}

		// Store closure to keep it alive until the timeout fires,
		// instead of leaking it with `forget()`.
		self.pending_closure = Some(closure);

		Poll::Pending
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Note: WASM tests should use wasm_bindgen_test
	// These tests are for compile-time verification only

	#[test]
	fn test_wait_options_builder() {
		let opts = WaitOptions::new()
			.with_timeout(Duration::from_secs(10))
			.with_interval(Duration::from_millis(100))
			.with_description("test wait");

		assert_eq!(opts.timeout, Duration::from_secs(10));
		assert_eq!(opts.interval, Duration::from_millis(100));
		assert_eq!(opts.description, Some("test wait".to_string()));
	}

	#[test]
	fn test_wait_error_display() {
		let timeout_error = WaitError::Timeout {
			timeout: Duration::from_secs(5),
			description: Some("element to appear".to_string()),
		};
		assert!(timeout_error.to_string().contains("5s"));
		assert!(timeout_error.to_string().contains("element to appear"));

		let js_error = WaitError::JsError("test error".to_string());
		assert!(js_error.to_string().contains("test error"));

		let not_found = WaitError::ElementNotFound("#missing".to_string());
		assert!(not_found.to_string().contains("#missing"));
	}
}
