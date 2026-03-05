//! Content negotiator

use super::accept::AcceptHeader;
use super::media_type::MediaType;

/// Trait for renderers
pub trait Renderer {
	fn media_type(&self) -> &MediaType;
	fn format(&self) -> &str;
}

/// Error type for negotiation failures
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum NegotiationError {
	NoSuitableRenderer,
}

impl std::fmt::Display for NegotiationError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			NegotiationError::NoSuitableRenderer => write!(f, "No suitable renderer found"),
		}
	}
}

impl std::error::Error for NegotiationError {}

/// Base content negotiator trait
pub trait BaseContentNegotiation {
	fn select_parser(
		&self,
		_request: Option<&str>,
		_parsers: &[MediaType],
	) -> Result<MediaType, NegotiationError> {
		Err(NegotiationError::NoSuitableRenderer)
	}

	fn select_renderer(
		&self,
		_request: Option<&str>,
		_renderers: &[MediaType],
	) -> Result<(MediaType, String), NegotiationError> {
		Err(NegotiationError::NoSuitableRenderer)
	}
}

/// Content negotiator for selecting appropriate renderer
#[derive(Debug, Clone)]
pub struct ContentNegotiator {
	default_media_type: MediaType,
}

impl ContentNegotiator {
	/// Creates a new ContentNegotiator with default media type of application/json
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::ContentNegotiator;
	///
	/// let negotiator = ContentNegotiator::new();
	/// // Default media type is application/json
	/// ```
	pub fn new() -> Self {
		Self {
			default_media_type: MediaType::new("application", "json"),
		}
	}
	/// Sets a custom default media type for the negotiator
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::{ContentNegotiator, MediaType};
	///
	/// let negotiator = ContentNegotiator::new()
	///     .with_default(MediaType::new("text", "html"));
	///
	/// let available = vec![MediaType::new("text", "html")];
	/// let result = negotiator.negotiate("", &available);
	/// assert_eq!(result.subtype, "html");
	/// ```
	pub fn with_default(mut self, media_type: MediaType) -> Self {
		self.default_media_type = media_type;
		self
	}
	/// Negotiate the best media type based on Accept header
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::{ContentNegotiator, MediaType};
	///
	/// let negotiator = ContentNegotiator::new();
	/// let available = vec![
	///     MediaType::new("application", "json"),
	///     MediaType::new("text", "html"),
	/// ];
	///
	/// let result = negotiator.negotiate("text/html, application/json", &available);
	/// assert_eq!(result.subtype, "html");
	///
	/// let result2 = negotiator.negotiate("application/json", &available);
	/// assert_eq!(result2.subtype, "json");
	/// ```
	pub fn negotiate(&self, accept_header: &str, available: &[MediaType]) -> MediaType {
		let accept = AcceptHeader::parse(accept_header);

		accept
			.find_best_match(available)
			.unwrap_or_else(|| self.default_media_type.clone())
	}
	/// Select renderer based on Accept header
	/// Returns (renderer, accepted_media_type)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::{ContentNegotiator, MediaType};
	///
	/// let negotiator = ContentNegotiator::new();
	/// let renderers = vec![
	///     MediaType::new("application", "json"),
	///     MediaType::new("text", "html"),
	/// ];
	///
	/// let result = negotiator.select_renderer(
	///     Some("application/json"),
	///     &renderers
	/// );
	/// assert!(result.is_ok());
	/// let (media_type, media_type_str) = result.unwrap();
	/// assert_eq!(media_type.subtype, "json");
	///
	/// // No accept header uses first renderer
	/// let result2 = negotiator.select_renderer(None, &renderers);
	/// assert!(result2.is_ok());
	/// ```
	pub fn select_renderer(
		&self,
		accept_header: Option<&str>,
		renderers: &[MediaType],
	) -> Result<(MediaType, String), NegotiationError> {
		if renderers.is_empty() {
			return Err(NegotiationError::NoSuitableRenderer);
		}

		let accept_str = accept_header.unwrap_or("");

		// If no accept header or wildcard, use first renderer
		if accept_str.is_empty() || accept_str == "*/*" {
			let renderer = renderers[0].clone();
			let media_type_str = renderer.to_string();
			return Ok((renderer, media_type_str));
		}

		let accept = AcceptHeader::parse(accept_str);

		// Find best match considering parameters
		for accepted in &accept.media_types {
			for renderer in renderers {
				if accepted.matches(renderer) {
					// If client specifies parameters, include them in the result
					let result_str = if !accepted.parameters.is_empty() {
						accepted.full_string()
					} else {
						renderer.to_string()
					};
					return Ok((renderer.clone(), result_str));
				}
			}
		}

		Err(NegotiationError::NoSuitableRenderer)
	}
	/// Filter renderers by format
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::{ContentNegotiator, MediaType, RendererInfo};
	///
	/// let negotiator = ContentNegotiator::new();
	/// let renderers = vec![
	///     RendererInfo {
	///         media_type: MediaType::new("application", "json"),
	///         format: "json".to_string(),
	///     },
	///     RendererInfo {
	///         media_type: MediaType::new("text", "html"),
	///         format: "html".to_string(),
	///     },
	/// ];
	///
	/// let filtered = negotiator.filter_renderers(&renderers, "json");
	/// assert!(filtered.is_ok());
	/// assert_eq!(filtered.unwrap().len(), 1);
	///
	/// let not_found = negotiator.filter_renderers(&renderers, "xml");
	/// assert!(not_found.is_err());
	/// ```
	pub fn filter_renderers(
		&self,
		renderers: &[RendererInfo],
		format: &str,
	) -> Result<Vec<RendererInfo>, NegotiationError> {
		let filtered: Vec<_> = renderers
			.iter()
			.filter(|r| r.format == format)
			.cloned()
			.collect();

		if filtered.is_empty() {
			Err(NegotiationError::NoSuitableRenderer)
		} else {
			Ok(filtered)
		}
	}
	/// Select renderer based on format parameter (e.g., ?format=json)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::{ContentNegotiator, MediaType};
	///
	/// let negotiator = ContentNegotiator::new();
	/// let available = vec![
	///     MediaType::new("application", "json"),
	///     MediaType::new("text", "html"),
	/// ];
	///
	/// let result = negotiator.select_by_format("json", &available);
	/// assert!(result.is_some());
	/// assert_eq!(result.unwrap().subtype, "json");
	/// ```
	pub fn select_by_format(&self, format: &str, available: &[MediaType]) -> Option<MediaType> {
		let format_lower = format.to_lowercase();
		available
			.iter()
			.find(|mt| {
				mt.subtype.to_lowercase() == format_lower
					|| mt.to_string().to_lowercase().contains(&format_lower)
			})
			.cloned()
	}
}

/// Renderer information for testing
#[derive(Debug, Clone)]
pub struct RendererInfo {
	pub media_type: MediaType,
	pub format: String,
}

impl Default for ContentNegotiator {
	fn default() -> Self {
		Self::new()
	}
}

/// Base content negotiation implementation (abstract)
#[derive(Debug, Clone)]
pub struct BaseNegotiator;

impl BaseContentNegotiation for BaseNegotiator {}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_negotiate() {
		let negotiator = ContentNegotiator::new();
		let available = vec![
			MediaType::new("application", "json"),
			MediaType::new("text", "html"),
		];

		let result = negotiator.negotiate("text/html, application/json", &available);
		assert_eq!(result.subtype, "html");
	}

	#[test]
	fn test_select_by_format() {
		let negotiator = ContentNegotiator::new();
		let available = vec![
			MediaType::new("application", "json"),
			MediaType::new("text", "html"),
		];

		let result = negotiator.select_by_format("json", &available);
		assert!(result.is_some());
		assert_eq!(result.unwrap().subtype, "json");
	}
}
