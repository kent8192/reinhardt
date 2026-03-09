//! Browsable API for Reinhardt
//!
//! This crate provides DRF-style browsable API interface for exploring APIs
//! through a web browser.

/// Middleware for enabling the browsable API on incoming requests.
pub mod middleware;
/// HTML rendering for browsable API responses.
pub mod renderer;
/// Response types used by the browsable API.
pub mod response;
/// Template engine integration for browsable API pages.
pub mod template;

/// Error type for browsable API operations
#[derive(Debug, thiserror::Error)]
pub enum Error {
	/// A template rendering error.
	#[error("Render error: {0}")]
	Render(String),
	/// A data serialization error.
	#[error("Serialization error: {0}")]
	Serialization(String),
	/// Any other browsable API error.
	#[error("{0}")]
	Other(String),
}

/// A convenience type alias for browsable API results.
pub type Result<T> = std::result::Result<T, Error>;

pub use middleware::{BrowsableApiConfig, BrowsableApiMiddleware};
pub use renderer::{ApiContext, BrowsableApiRenderer, FormContext, FormField, SelectOption};
pub use response::BrowsableResponse;
pub use template::ApiTemplate;

/// Re-export commonly used types
pub mod prelude {
	pub use crate::middleware::*;
	pub use crate::renderer::*;
	pub use crate::response::*;
	pub use crate::template::*;
}
