//! E2E browser testing fixtures via WebDriver (fantoccini).
//!
//! This module provides rstest fixtures for end-to-end browser testing
//! using fantoccini as a WebDriver client. Unlike the in-browser WASM
//! fixtures, these run natively and control the browser externally.
//!
//! # Prerequisites
//!
//! A WebDriver-compatible server must be running (e.g., chromedriver, geckodriver).
//!
//! # Environment Variables
//!
//! - `WEBDRIVER_URL`: WebDriver server URL (default: `http://localhost:4444`)
//! - `BROWSER_HEADLESS`: Set to `"false"`, `"0"`, `"no"`, or `"off"` (case-insensitive) to disable headless mode (default: enabled)
//! - `BROWSER_TYPE`: `"chrome"` or `"firefox"` (default: `"chrome"`)
//! - `BROWSER_WAIT_TIMEOUT`: Element wait timeout in seconds (default: `10`)
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_test::fixtures::wasm::e2e::*;
//! use rstest::*;
//!
//! #[rstest]
//! #[tokio::test]
//! async fn test_page_navigation(#[future] browser_client: BrowserClient) {
//!     let client = browser_client.await;
//!     client.navigate("https://example.com").await.unwrap();
//!     assert_eq!(client.title().await.unwrap(), "Example Domain");
//!     client.close().await.unwrap();
//! }
//! ```

use std::time::Duration;

use fantoccini::ClientBuilder;
use rstest::*;
use serde_json::json;

// Re-export fantoccini types for downstream convenience
pub use fantoccini::{Client as FantocciniClient, Locator};

/// Default number of connection retry attempts for WebDriver
const DEFAULT_CONNECT_RETRIES: u32 = 3;

/// Base delay between retry attempts (doubles each attempt)
const BASE_RETRY_DELAY_MS: u64 = 500;

// ============================================================================
// Configuration Types
// ============================================================================

/// Supported browser types for E2E testing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserType {
	/// Google Chrome (via chromedriver)
	Chrome,
	/// Mozilla Firefox (via geckodriver)
	Firefox,
}

impl BrowserType {
	/// Parse browser type from string.
	///
	/// Returns `Chrome` for unrecognized values.
	pub fn from_str_lossy(s: &str) -> Self {
		match s.to_lowercase().as_str() {
			"firefox" | "gecko" | "geckodriver" => Self::Firefox,
			_ => Self::Chrome,
		}
	}
}

/// Configuration for E2E browser testing.
///
/// Reads settings from environment variables with sensible defaults.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_test::fixtures::wasm::e2e::BrowserConfig;
///
/// let config = BrowserConfig::from_env();
/// assert_eq!(config.webdriver_url, "http://localhost:4444");
/// assert!(config.headless);
/// ```
#[derive(Clone, Debug)]
pub struct BrowserConfig {
	/// WebDriver server URL.
	pub webdriver_url: String,
	/// Whether to run the browser in headless mode.
	pub headless: bool,
	/// Browser type to use.
	pub browser_type: BrowserType,
	/// Timeout for element wait operations.
	pub wait_timeout: Duration,
}

impl BrowserConfig {
	/// Create configuration from environment variables.
	///
	/// Falls back to sensible defaults when variables are not set.
	pub fn from_env() -> Self {
		let webdriver_url =
			std::env::var("WEBDRIVER_URL").unwrap_or_else(|_| "http://localhost:4444".to_string());

		let headless = std::env::var("BROWSER_HEADLESS")
			.map(|v| !matches!(v.to_lowercase().trim(), "false" | "0" | "no" | "off"))
			.unwrap_or(true);

		let browser_type = std::env::var("BROWSER_TYPE")
			.map(|v| BrowserType::from_str_lossy(&v))
			.unwrap_or(BrowserType::Chrome);

		let wait_timeout = std::env::var("BROWSER_WAIT_TIMEOUT")
			.ok()
			.and_then(|v| v.parse::<u64>().ok())
			.map(Duration::from_secs)
			.unwrap_or_else(|| Duration::from_secs(10));

		Self {
			webdriver_url,
			headless,
			browser_type,
			wait_timeout,
		}
	}
}

impl Default for BrowserConfig {
	fn default() -> Self {
		Self::from_env()
	}
}

// ============================================================================
// Browser Client Wrapper
// ============================================================================

/// Ergonomic wrapper around `fantoccini::Client` for E2E browser testing.
///
/// Provides simplified methods for common browser operations while still
/// allowing access to the underlying `fantoccini::Client` for advanced use.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_test::fixtures::wasm::e2e::BrowserClient;
///
/// let client = BrowserClient::connect(BrowserConfig::from_env()).await?;
/// client.navigate("https://example.com").await?;
/// let title = client.title().await?;
/// client.close().await?;
/// ```
pub struct BrowserClient {
	client: fantoccini::Client,
	config: BrowserConfig,
}

impl BrowserClient {
	/// Build WebDriver capabilities from the current configuration.
	fn build_capabilities(config: &BrowserConfig) -> serde_json::Map<String, serde_json::Value> {
		let mut caps = serde_json::Map::new();

		match config.browser_type {
			BrowserType::Chrome => {
				caps.insert("browserName".into(), json!("chrome"));
				let mut chrome_opts = serde_json::Map::new();
				let mut args = vec![
					"--no-sandbox".to_string(),
					"--disable-dev-shm-usage".to_string(),
				];
				if config.headless {
					args.push("--headless=new".to_string());
				}
				chrome_opts.insert("args".into(), json!(args));
				caps.insert("goog:chromeOptions".into(), json!(chrome_opts));
			}
			BrowserType::Firefox => {
				caps.insert("browserName".into(), json!("firefox"));
				let mut firefox_opts = serde_json::Map::new();
				if config.headless {
					firefox_opts.insert("args".into(), json!(["-headless"]));
				}
				caps.insert("moz:firefoxOptions".into(), json!(firefox_opts));
			}
		}

		caps
	}

	/// Connect to a WebDriver server using the provided configuration.
	///
	/// Builds browser capabilities based on the config (headless mode,
	/// browser type) and establishes a WebDriver session.
	pub async fn connect(
		config: BrowserConfig,
	) -> Result<Self, fantoccini::error::NewSessionError> {
		let caps = Self::build_capabilities(&config);

		let client = ClientBuilder::native()
			.capabilities(caps)
			.connect(&config.webdriver_url)
			.await?;

		Ok(Self { client, config })
	}

	/// Connect to a WebDriver server with retry logic.
	///
	/// Retries connection with exponential backoff when the WebDriver
	/// server is not immediately available (e.g., during CI startup).
	/// The first attempt is always made, followed by up to `retries`
	/// additional attempts (for a total of `retries + 1` attempts).
	///
	/// # Arguments
	///
	/// * `config` - Browser configuration
	/// * `retries` - Number of additional retry attempts after the first failure
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let client = BrowserClient::connect_with_retries(
	///     BrowserConfig::from_env(),
	///     3,
	/// ).await?;
	/// ```
	pub async fn connect_with_retries(
		config: BrowserConfig,
		retries: u32,
	) -> Result<Self, fantoccini::error::NewSessionError> {
		let base_caps = Self::build_capabilities(&config);
		let mut last_error = None;
		let total_attempts = retries + 1;

		for attempt in 0..total_attempts {
			let caps = base_caps.clone();
			match ClientBuilder::native()
				.capabilities(caps)
				.connect(&config.webdriver_url)
				.await
			{
				Ok(client) => return Ok(Self { client, config }),
				Err(e) => {
					if attempt + 1 < total_attempts {
						// Cap backoff at 8 seconds to avoid excessive waits
						let delay = Duration::from_millis(
							BASE_RETRY_DELAY_MS
								.saturating_mul(2u64.saturating_pow(attempt))
								.min(8_000),
						);
						eprintln!(
							"[e2e] WebDriver connection attempt {}/{} failed: {}. Retrying in {:?}...",
							attempt + 1,
							total_attempts,
							e,
							delay,
						);
						tokio::time::sleep(delay).await;
					}
					last_error = Some(e);
				}
			}
		}

		Err(last_error.expect("Should have at least one error after retries"))
	}

	// ========================================================================
	// Navigation
	// ========================================================================

	/// Navigate to the specified URL.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// client.navigate("https://example.com").await?;
	/// ```
	pub async fn navigate(&self, url: &str) -> Result<(), fantoccini::error::CmdError> {
		self.client.goto(url).await
	}

	/// Get the current page URL.
	pub async fn current_url(&self) -> Result<url::Url, fantoccini::error::CmdError> {
		self.client.current_url().await
	}

	/// Get the current page title.
	pub async fn title(&self) -> Result<String, fantoccini::error::CmdError> {
		self.client.title().await
	}

	/// Get the current page HTML source.
	pub async fn source(&self) -> Result<String, fantoccini::error::CmdError> {
		self.client.source().await
	}

	// ========================================================================
	// Element Interaction
	// ========================================================================

	/// Find a single element by CSS selector.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let element = client.find("button.submit").await?;
	/// ```
	pub async fn find(
		&self,
		css: &str,
	) -> Result<fantoccini::elements::Element, fantoccini::error::CmdError> {
		self.client.find(Locator::Css(css)).await
	}

	/// Find all elements matching a CSS selector.
	pub async fn find_all(
		&self,
		css: &str,
	) -> Result<Vec<fantoccini::elements::Element>, fantoccini::error::CmdError> {
		self.client.find_all(Locator::Css(css)).await
	}

	/// Click an element identified by CSS selector.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// client.click("button.submit").await?;
	/// ```
	pub async fn click(&self, css: &str) -> Result<(), fantoccini::error::CmdError> {
		let elem = self.find(css).await?;
		elem.click().await
	}

	/// Type text into an element identified by CSS selector.
	///
	/// Finds the element, clears it, then sends the specified keys.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// client.type_into("input[name='email']", "user@example.com").await?;
	/// ```
	pub async fn type_into(
		&self,
		css: &str,
		text: &str,
	) -> Result<(), fantoccini::error::CmdError> {
		let elem = self.find(css).await?;
		elem.clear().await?;
		elem.send_keys(text).await
	}

	/// Wait for an element matching the CSS selector to appear.
	///
	/// Uses the configured `wait_timeout` from [`BrowserConfig`].
	pub async fn wait_for(
		&self,
		css: &str,
	) -> Result<fantoccini::elements::Element, fantoccini::error::CmdError> {
		self.client
			.wait()
			.at_most(self.config.wait_timeout)
			.for_element(Locator::Css(css))
			.await
	}

	/// Get the visible text content of an element identified by CSS selector.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let text = client.get_text("h1.title").await?;
	/// assert_eq!(text, "Welcome");
	/// ```
	pub async fn get_text(&self, css: &str) -> Result<String, fantoccini::error::CmdError> {
		let elem = self.find(css).await?;
		elem.text().await
	}

	/// Get an attribute value of an element identified by CSS selector.
	///
	/// Returns `None` if the attribute is not present on the element.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let href = client.get_attribute("a.link", "href").await?;
	/// assert!(href.is_some());
	/// ```
	pub async fn get_attribute(
		&self,
		css: &str,
		attribute: &str,
	) -> Result<Option<String>, fantoccini::error::CmdError> {
		let elem = self.find(css).await?;
		elem.attr(attribute).await
	}

	/// Select an option in a `<select>` element by its visible text.
	///
	/// Uses JavaScript to find the matching `<option>` and set the value,
	/// which is reliable across browsers.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// client.select_by_text("select#country", "Japan").await?;
	/// ```
	pub async fn select_by_text(
		&self,
		select_css: &str,
		text: &str,
	) -> Result<(), fantoccini::error::CmdError> {
		// Use JS with querySelector to select option by text for reliability
		let script = format!(
			r#"
			const select = document.querySelector({css});
			if (!select) return false;
			for (const opt of select.options) {{
				if (opt.text === {text}) {{
					select.value = opt.value;
					select.dispatchEvent(new Event('change', {{ bubbles: true }}));
					return true;
				}}
			}}
			return false;
			"#,
			css = serde_json::to_string(select_css).expect("failed to JSON-encode CSS selector"),
			text = serde_json::to_string(text).expect("failed to JSON-encode option text"),
		);
		let result = self.client.execute(&script, vec![]).await?;
		// The script returns false when the select element or option text is not found
		let success = result.as_bool().unwrap_or(false);
		if !success {
			return Err(fantoccini::error::CmdError::NotW3C(serde_json::json!({
				"error": "select_by_text failed",
				"message": format!(
					"Could not find option with text {:?} in select {:?}",
					text, select_css
				),
			})));
		}
		Ok(())
	}

	/// Wait until the current URL matches the given predicate.
	///
	/// Polls the current URL at short intervals until the predicate returns
	/// `true` or the configured `wait_timeout` is exceeded.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// client.click("a.next-page").await?;
	/// client.wait_for_url(|url| url.path().contains("/page/2")).await?;
	/// ```
	pub async fn wait_for_url<F>(
		&self,
		predicate: F,
	) -> Result<url::Url, fantoccini::error::CmdError>
	where
		F: Fn(&url::Url) -> bool,
	{
		let start = std::time::Instant::now();
		let poll_interval = Duration::from_millis(100);

		loop {
			let current = self.current_url().await?;
			if predicate(&current) {
				return Ok(current);
			}
			if start.elapsed() > self.config.wait_timeout {
				return Err(fantoccini::error::CmdError::WaitTimeout);
			}
			tokio::time::sleep(poll_interval).await;
		}
	}

	// ========================================================================
	// Page Utilities
	// ========================================================================

	/// Take a PNG screenshot of the current page.
	///
	/// Returns the raw PNG bytes.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let png_bytes = client.screenshot().await?;
	/// std::fs::write("/tmp/screenshot.png", &png_bytes)?;
	/// ```
	pub async fn screenshot(&self) -> Result<Vec<u8>, fantoccini::error::CmdError> {
		self.client.screenshot().await
	}

	/// Execute JavaScript in the browser context.
	///
	/// # Example
	///
	/// ```rust,ignore
	/// let result = client.execute_js(
	///     "return document.title;",
	///     vec![],
	/// ).await?;
	/// ```
	pub async fn execute_js(
		&self,
		script: &str,
		args: Vec<serde_json::Value>,
	) -> Result<serde_json::Value, fantoccini::error::CmdError> {
		self.client.execute(script, args).await
	}

	// ========================================================================
	// Session Management
	// ========================================================================

	/// Close the browser session.
	///
	/// This consumes the client. Must be called to clean up the WebDriver session.
	pub async fn close(self) -> Result<(), fantoccini::error::CmdError> {
		self.client.close().await
	}

	/// Access the underlying `fantoccini::Client` for advanced operations.
	///
	/// Use this when the wrapper methods don't cover your needs.
	pub fn inner(&self) -> &fantoccini::Client {
		&self.client
	}

	/// Consume the wrapper and return the underlying `fantoccini::Client`.
	pub fn into_inner(self) -> fantoccini::Client {
		self.client
	}

	/// Get a reference to the current browser configuration.
	pub fn config(&self) -> &BrowserConfig {
		&self.config
	}
}

// ============================================================================
// rstest Fixtures
// ============================================================================

/// Fixture providing a [`BrowserConfig`] from environment variables.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_test::fixtures::wasm::e2e::*;
/// use rstest::*;
///
/// #[rstest]
/// fn test_config(browser_config: BrowserConfig) {
///     assert!(!browser_config.webdriver_url.is_empty());
/// }
/// ```
#[fixture]
pub fn browser_config() -> BrowserConfig {
	BrowserConfig::from_env()
}

/// Fixture providing a connected [`BrowserClient`].
///
/// This is an async fixture that connects to the WebDriver server
/// specified by environment variables (see [`BrowserConfig`]).
///
/// # Panics
///
/// Panics if the WebDriver connection fails.
///
/// # Example
///
/// ```rust,ignore
/// use reinhardt_test::fixtures::wasm::e2e::*;
/// use rstest::*;
///
/// #[rstest]
/// #[tokio::test]
/// async fn test_navigation(#[future] browser_client: BrowserClient) {
///     let client = browser_client.await;
///     client.navigate("https://example.com").await.unwrap();
///     client.close().await.unwrap();
/// }
/// ```
#[fixture]
pub async fn browser_client(browser_config: BrowserConfig) -> BrowserClient {
	BrowserClient::connect_with_retries(browser_config.clone(), DEFAULT_CONNECT_RETRIES)
		.await
		.unwrap_or_else(|e| {
			panic!(
				"Failed to connect to WebDriver server at {} (browser: {:?}, headless: {}) after {} retries: {e}",
				browser_config.webdriver_url, browser_config.browser_type, browser_config.headless,
				DEFAULT_CONNECT_RETRIES,
			)
		})
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;
	use serial_test::serial;

	#[rstest]
	#[case("chrome", BrowserType::Chrome)]
	#[case("Chrome", BrowserType::Chrome)]
	#[case("CHROME", BrowserType::Chrome)]
	#[case("firefox", BrowserType::Firefox)]
	#[case("Firefox", BrowserType::Firefox)]
	#[case("FIREFOX", BrowserType::Firefox)]
	#[case("gecko", BrowserType::Firefox)]
	#[case("geckodriver", BrowserType::Firefox)]
	#[case("unknown", BrowserType::Chrome)]
	#[case("", BrowserType::Chrome)]
	fn test_browser_type_from_str_lossy(#[case] input: &str, #[case] expected: BrowserType) {
		// Act
		let result = BrowserType::from_str_lossy(input);

		// Assert
		assert_eq!(result, expected);
	}

	#[rstest]
	#[serial(browser_env)]
	fn test_browser_config_defaults() {
		// Arrange
		// SAFETY: Modifying environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			std::env::remove_var("WEBDRIVER_URL");
			std::env::remove_var("BROWSER_HEADLESS");
			std::env::remove_var("BROWSER_TYPE");
			std::env::remove_var("BROWSER_WAIT_TIMEOUT");
		}

		// Act
		let config = BrowserConfig::from_env();

		// Assert
		assert_eq!(config.webdriver_url, "http://localhost:4444");
		assert!(config.headless);
		assert_eq!(config.browser_type, BrowserType::Chrome);
		assert_eq!(config.wait_timeout, Duration::from_secs(10));
	}

	#[rstest]
	#[case("false", false)]
	#[case("False", false)]
	#[case("FALSE", false)]
	#[case("0", false)]
	#[case("no", false)]
	#[case("off", false)]
	#[case("true", true)]
	#[case("1", true)]
	#[case("yes", true)]
	#[serial(browser_env)]
	fn test_browser_headless_parsing(#[case] value: &str, #[case] expected: bool) {
		// Arrange
		// SAFETY: Modifying environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			std::env::set_var("BROWSER_HEADLESS", value);
		}

		// Act
		let config = BrowserConfig::from_env();

		// Assert
		assert_eq!(config.headless, expected);

		// Cleanup
		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			std::env::remove_var("BROWSER_HEADLESS");
		}
	}

	#[rstest]
	#[serial(browser_env)]
	fn test_browser_config_custom_values() {
		// Arrange
		// SAFETY: Modifying environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			std::env::set_var("WEBDRIVER_URL", "http://selenium:4444");
			std::env::set_var("BROWSER_TYPE", "firefox");
			std::env::set_var("BROWSER_WAIT_TIMEOUT", "30");
		}

		// Act
		let config = BrowserConfig::from_env();

		// Assert
		assert_eq!(config.webdriver_url, "http://selenium:4444");
		assert_eq!(config.browser_type, BrowserType::Firefox);
		assert_eq!(config.wait_timeout, Duration::from_secs(30));

		// Cleanup
		// SAFETY: Removing environment variables is unsafe in multi-threaded programs.
		// This test uses #[serial] to ensure exclusive access to environment variables.
		unsafe {
			std::env::remove_var("WEBDRIVER_URL");
			std::env::remove_var("BROWSER_TYPE");
			std::env::remove_var("BROWSER_WAIT_TIMEOUT");
		}
	}
}
