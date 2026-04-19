//! E2E browser testing fixtures via Chrome DevTools Protocol (chromiumoxide).
//!
//! Each test gets a **fully isolated Chrome instance** running in a Docker
//! container managed by testcontainers. This guarantees parallel-safe execution
//! without port conflicts or shared browser state.
//!
//! # Prerequisites
//!
//! - Docker daemon running on the host
//!
//! # Environment Variables
//!
//! - `CDP_WAIT_TIMEOUT`: Element wait timeout in seconds (default: `10`)
//! - `CDP_CHROME_IMAGE`: Custom Docker image (default: `chromedp/headless-shell:latest`)
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_test::fixtures::wasm::e2e_cdp::*;
//! use rstest::*;
//!
//! #[rstest]
//! #[tokio::test]
//! async fn test_page(#[future] cdp_browser: CdpBrowser) {
//!     let browser = cdp_browser.await;
//!     let page = browser.new_page("https://example.com").await.unwrap();
//!     let title = page.title().await.unwrap();
//!     assert_eq!(title, "Example Domain");
//! }
//! ```

use std::time::Duration;

use chromiumoxide::Page;
use chromiumoxide::browser::Browser;
use chromiumoxide::error::CdpError;
use futures::StreamExt;
use rstest::*;
use testcontainers::core::{ContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage};
use tokio::task::JoinHandle;

// Re-export chromiumoxide types for downstream convenience
pub use chromiumoxide;

/// CDP debugging port exposed inside the Chrome container.
const CDP_PORT: u16 = 9222;

/// Default Docker image for headless Chrome.
const DEFAULT_CHROME_IMAGE: &str = "chromedp/headless-shell";
const DEFAULT_CHROME_TAG: &str = "latest";

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for CDP-based E2E browser testing.
#[derive(Clone, Debug)]
pub struct CdpConfig {
	/// Timeout for element wait operations.
	pub wait_timeout: Duration,
	/// Docker image name for headless Chrome.
	pub chrome_image: String,
	/// Docker image tag.
	pub chrome_tag: String,
}

impl CdpConfig {
	/// Create configuration from environment variables.
	pub fn from_env() -> Self {
		let wait_timeout = std::env::var("CDP_WAIT_TIMEOUT")
			.ok()
			.and_then(|v| v.parse::<u64>().ok())
			.map(Duration::from_secs)
			.unwrap_or_else(|| Duration::from_secs(10));

		let (chrome_image, chrome_tag) = if let Ok(full) = std::env::var("CDP_CHROME_IMAGE") {
			match full.split_once(':') {
				Some((img, tag)) => (img.to_string(), tag.to_string()),
				None => (full, DEFAULT_CHROME_TAG.to_string()),
			}
		} else {
			(
				DEFAULT_CHROME_IMAGE.to_string(),
				DEFAULT_CHROME_TAG.to_string(),
			)
		};

		Self {
			wait_timeout,
			chrome_image,
			chrome_tag,
		}
	}
}

impl Default for CdpConfig {
	fn default() -> Self {
		Self::from_env()
	}
}

// ============================================================================
// CdpBrowser Wrapper
// ============================================================================

/// Managed browser instance using Chrome DevTools Protocol over Docker.
///
/// Each instance owns an isolated Docker container running headless Chrome.
/// The container and CDP handler task are automatically cleaned up on drop.
pub struct CdpBrowser {
	browser: Browser,
	_handler_task: JoinHandle<()>,
	// Hold the container to keep it alive for the lifetime of the browser.
	_container: ContainerAsync<GenericImage>,
	config: CdpConfig,
}

impl CdpBrowser {
	/// Start a new containerized Chrome instance and connect via CDP.
	///
	/// 1. Launches a `chromedp/headless-shell` Docker container
	/// 2. Waits for the CDP port (9222) to become ready
	/// 3. Connects chromiumoxide via `Browser::connect`
	pub async fn start(config: CdpConfig) -> Result<Self, CdpError> {
		let image = GenericImage::new(&config.chrome_image, &config.chrome_tag)
			.with_exposed_port(ContainerPort::Tcp(CDP_PORT))
			.with_wait_for(WaitFor::message_on_stderr("DevTools listening on"));

		let container = image.start().await.map_err(|e| {
			CdpError::Io(std::io::Error::new(
				std::io::ErrorKind::ConnectionRefused,
				format!("Failed to start Chrome container: {}", e),
			))
		})?;

		let host_port = container.get_host_port_ipv4(CDP_PORT).await.map_err(|e| {
			CdpError::Io(std::io::Error::new(
				std::io::ErrorKind::AddrNotAvailable,
				format!("Failed to get mapped port: {}", e),
			))
		})?;

		let cdp_url = format!("http://127.0.0.1:{}", host_port);

		// Connect to the containerized Chrome via CDP.
		// chromiumoxide resolves the WebSocket URL from /json/version automatically.
		let (browser, mut handler) = Browser::connect(&cdp_url).await?;

		let handler_task = tokio::spawn(async move {
			while let Some(result) = handler.next().await {
				if result.is_err() {
					break;
				}
			}
		});

		Ok(Self {
			browser,
			_handler_task: handler_task,
			_container: container,
			config,
		})
	}

	/// Start with retry logic for transient Docker/CDP failures.
	pub async fn start_with_retries(config: CdpConfig, retries: u32) -> Result<Self, CdpError> {
		let total = retries + 1;
		let mut last_err = None;

		for attempt in 0..total {
			match Self::start(config.clone()).await {
				Ok(b) => return Ok(b),
				Err(e) => {
					if attempt + 1 < total {
						let delay = Duration::from_millis(
							500u64
								.saturating_mul(2u64.saturating_pow(attempt))
								.min(8_000),
						);
						eprintln!(
							"[e2e-cdp] Chrome container attempt {}/{} failed: {}. Retrying in {:?}...",
							attempt + 1,
							total,
							e,
							delay,
						);
						tokio::time::sleep(delay).await;
					}
					last_err = Some(e);
				}
			}
		}

		Err(last_err.expect("Should have at least one error"))
	}

	/// Create a new page and navigate to the specified URL.
	pub async fn new_page(&self, url: &str) -> Result<CdpPage, CdpError> {
		let page = self.browser.new_page(url).await?;
		Ok(CdpPage {
			page,
			wait_timeout: self.config.wait_timeout,
		})
	}

	/// Get a reference to the underlying `chromiumoxide::Browser`.
	pub fn inner(&self) -> &Browser {
		&self.browser
	}

	/// Get a reference to the current configuration.
	pub fn config(&self) -> &CdpConfig {
		&self.config
	}
}

impl Drop for CdpBrowser {
	fn drop(&mut self) {
		self._handler_task.abort();
	}
}

// ============================================================================
// CdpPage Wrapper
// ============================================================================

/// Ergonomic wrapper around [`chromiumoxide::Page`] for E2E testing.
pub struct CdpPage {
	page: Page,
	wait_timeout: Duration,
}

impl CdpPage {
	/// Navigate to the specified URL.
	pub async fn navigate(&self, url: &str) -> Result<(), CdpError> {
		self.page.goto(url).await?;
		Ok(())
	}

	/// Get the current page URL.
	pub async fn url(&self) -> Result<Option<String>> {
		self.page.url().await
	}

	/// Get the current page title.
	pub async fn title(&self) -> Result<Option<String>> {
		let val = self
			.page
			.evaluate("document.title")
			.await?
			.into_value::<String>();
		match val {
			Ok(s) => Ok(Some(s)),
			Err(_) => Ok(None),
		}
	}

	/// Get the full HTML source of the current page.
	pub async fn content(&self) -> Result<String> {
		self.page.content().await
	}

	/// Find a single element by CSS selector.
	pub async fn find(&self, css: &str) -> Result<chromiumoxide::element::Element> {
		self.page.find_element(css).await
	}

	/// Find all elements matching a CSS selector.
	pub async fn find_all(&self, css: &str) -> Result<Vec<chromiumoxide::element::Element>> {
		self.page.find_elements(css).await
	}

	/// Click an element identified by CSS selector.
	pub async fn click(&self, css: &str) -> Result<()> {
		let elem = self.page.find_element(css).await?;
		elem.click().await?;
		Ok(())
	}

	/// Type text into an element identified by CSS selector.
	///
	/// Clicks the element first, clears it via JS, then types character by character.
	pub async fn type_into(&self, css: &str, text: &str) -> Result<()> {
		let elem = self.page.find_element(css).await?;
		elem.click().await?;
		self.page
			.evaluate(format!(
				"document.querySelector({}).value = ''",
				serde_json::to_string(css).expect("CSS selector should serialize")
			))
			.await?;
		elem.type_str(text).await?;
		Ok(())
	}

	/// Get the visible text content of an element.
	pub async fn get_text(&self, css: &str) -> Result<Option<String>> {
		let elem = self.page.find_element(css).await?;
		elem.inner_text().await
	}

	/// Get an attribute value of an element.
	pub async fn get_attribute(&self, css: &str, attribute: &str) -> Result<Option<String>> {
		let elem = self.page.find_element(css).await?;
		elem.attribute(attribute).await
	}

	/// Wait for an element matching the CSS selector to appear in the DOM.
	///
	/// Polls at 100ms intervals up to the configured `wait_timeout`.
	pub async fn wait_for(&self, css: &str) -> Result<chromiumoxide::element::Element> {
		let start = std::time::Instant::now();
		let poll_interval = Duration::from_millis(100);

		loop {
			match self.page.find_element(css).await {
				Ok(elem) => return Ok(elem),
				Err(_) if start.elapsed() < self.wait_timeout => {
					tokio::time::sleep(poll_interval).await;
				}
				Err(e) => return Err(e),
			}
		}
	}

	/// Wait until the current URL matches the given predicate.
	pub async fn wait_for_url<F>(&self, predicate: F) -> Result<String>
	where
		F: Fn(&str) -> bool,
	{
		let start = std::time::Instant::now();
		let poll_interval = Duration::from_millis(100);

		loop {
			if let Some(url) = self.page.url().await?
				&& predicate(&url)
			{
				return Ok(url);
			}
			if start.elapsed() > self.wait_timeout {
				return Err(CdpError::Timeout);
			}
			tokio::time::sleep(poll_interval).await;
		}
	}

	/// Execute JavaScript in the page context.
	pub async fn execute_js(&self, script: &str) -> Result<serde_json::Value> {
		let val = self.page.evaluate(script).await?;
		Ok(val.into_value()?)
	}

	/// Take a PNG screenshot of the current page.
	pub async fn screenshot(&self) -> Result<Vec<u8>> {
		self.page
			.screenshot(
				chromiumoxide::page::ScreenshotParams::builder()
					.full_page(true)
					.build(),
			)
			.await
	}

	/// Access the underlying `chromiumoxide::Page`.
	pub fn inner(&self) -> &Page {
		&self.page
	}
}

// Type alias for ergonomics
type Result<T, E = CdpError> = std::result::Result<T, E>;

// ============================================================================
// rstest Fixtures
// ============================================================================

/// Fixture providing a [`CdpConfig`] from environment variables.
#[fixture]
pub fn cdp_config() -> CdpConfig {
	CdpConfig::from_env()
}

/// Default retry count for container startup.
const DEFAULT_START_RETRIES: u32 = 2;

/// Fixture providing a [`CdpBrowser`] backed by an isolated Docker container.
///
/// Each invocation starts a fresh `chromedp/headless-shell` container with its
/// own Chrome process, ensuring full isolation between parallel tests.
/// The container is destroyed when the `CdpBrowser` is dropped.
#[fixture]
pub async fn cdp_browser(cdp_config: CdpConfig) -> CdpBrowser {
	CdpBrowser::start_with_retries(cdp_config, DEFAULT_START_RETRIES)
		.await
		.expect(
			"Failed to start Chrome container. Ensure Docker is running \
			 and the image is available (docker pull chromedp/headless-shell:latest).",
		)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_cdp_config_defaults() {
		// SAFETY: remove env vars for testing defaults.
		// This test is not run in parallel with other env-var tests.
		unsafe {
			std::env::remove_var("CDP_CHROME_IMAGE");
			std::env::remove_var("CDP_WAIT_TIMEOUT");
		}

		let config = CdpConfig::from_env();
		assert_eq!(config.wait_timeout, Duration::from_secs(10));
		assert_eq!(config.chrome_image, DEFAULT_CHROME_IMAGE);
		assert_eq!(config.chrome_tag, DEFAULT_CHROME_TAG);
	}

	#[test]
	fn test_cdp_config_custom_image() {
		unsafe {
			std::env::set_var("CDP_CHROME_IMAGE", "zenika/alpine-chrome:100");
		}

		let config = CdpConfig::from_env();
		assert_eq!(config.chrome_image, "zenika/alpine-chrome");
		assert_eq!(config.chrome_tag, "100");

		unsafe {
			std::env::remove_var("CDP_CHROME_IMAGE");
		}
	}
}
