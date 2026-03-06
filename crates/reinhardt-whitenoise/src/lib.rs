//! # Reinhardt WhiteNoise
//!
//! WhiteNoise-style static file optimization for Reinhardt web framework.
//!
//! This crate provides efficient static file serving with:
//! - gzip and brotli compression
//! - Content-based hashing for cache busting
//! - Intelligent cache control headers
//! - ETag-based conditional requests
//! - Pre-compression at startup
//!
//! ## Features
//!
//! - **Compression**: Automatic gzip and brotli compression with configurable levels
//! - **Content Hashing**: MD5-based filename hashing (Django collectstatic compatible)
//! - **Cache Control**: Intelligent caching with immutable detection
//! - **ETag Support**: Conditional requests with 304 Not Modified responses
//! - **Content Negotiation**: Accept-Encoding based variant selection (br > gzip > identity)
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use reinhardt_whitenoise::{WhiteNoiseConfig, WhiteNoiseMiddleware};
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Configure WhiteNoise
//!     let config = WhiteNoiseConfig::new(
//!         PathBuf::from("static"),
//!         "/static/".to_string(),
//!     )
//!     .with_compression(true, true)
//!     .with_max_age_immutable(31536000);
//!
//!     // Initialize middleware
//!     let middleware = WhiteNoiseMiddleware::new(config).await?;
//!
//!     // Add to your app
//!     // app.layer(middleware);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Module Structure
//!
//! - [`config`] - Configuration for WhiteNoise behavior
//! - [`cache`] - In-memory file cache and metadata
//! - [`compression`] - File compression and scanning
//! - [`middleware`] - HTTP middleware implementation
//! - [`immutable`] - Immutable file detection
//! - [`error`] - Error types

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]

pub mod cache;
pub mod compression;
pub mod config;
pub mod error;
pub mod immutable;

// Re-export main types
pub use config::WhiteNoiseConfig;
pub use error::{Result, WhiteNoiseError};

#[cfg(test)]
mod tests {
	#[test]
	fn test_crate_compiles() {
		// Smoke test to ensure crate structure is valid
	}
}
