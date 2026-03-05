//! Accept header parsing

use super::media_type::MediaType;

/// Represents an Accept header
#[derive(Debug, Clone)]
pub struct AcceptHeader {
	pub media_types: Vec<MediaType>,
}

impl AcceptHeader {
	/// Parses an Accept header string into an AcceptHeader struct
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::accept::AcceptHeader;
	///
	/// let accept = AcceptHeader::parse("application/json, text/html; q=0.9");
	/// assert_eq!(accept.media_types.len(), 2);
	/// assert_eq!(accept.media_types[0].quality, 1.0);
	/// assert_eq!(accept.media_types[1].quality, 0.9);
	///
	/// let complex = AcceptHeader::parse("text/html, application/json; q=0.8, */*; q=0.1");
	/// assert_eq!(complex.media_types.len(), 3);
	/// // Sorted by quality
	/// assert_eq!(complex.media_types[0].subtype, "html");
	/// ```
	pub fn parse(header: &str) -> Self {
		let mut media_types: Vec<MediaType> = header
			.split(',')
			.filter_map(|s| MediaType::parse(s.trim()))
			.collect();

		// Sort by quality (highest first)
		media_types.sort_by(|a, b| b.quality.partial_cmp(&a.quality).unwrap());

		Self { media_types }
	}
	/// Creates an empty AcceptHeader with no media types
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::accept::AcceptHeader;
	///
	/// let empty = AcceptHeader::empty();
	/// assert_eq!(empty.media_types.len(), 0);
	/// ```
	pub fn empty() -> Self {
		Self {
			media_types: Vec::new(),
		}
	}
	/// Finds the best matching media type from available options
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::accept::AcceptHeader;
	/// use reinhardt_core::negotiation::MediaType;
	///
	/// let accept = AcceptHeader::parse("application/json, text/html");
	/// let available = vec![
	///     MediaType::new("text", "html"),
	///     MediaType::new("application", "xml"),
	/// ];
	/// let best = accept.find_best_match(&available);
	/// assert!(best.is_some());
	/// assert_eq!(best.unwrap().subtype, "html");
	///
	/// let no_match = AcceptHeader::parse("application/json");
	/// let result = no_match.find_best_match(&available);
	/// assert!(result.is_none());
	/// ```
	pub fn find_best_match(&self, available: &[MediaType]) -> Option<MediaType> {
		for accepted in &self.media_types {
			for available_type in available {
				if accepted.matches(available_type) {
					return Some(available_type.clone());
				}
			}
		}
		None
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_parse_accept_header() {
		let accept = AcceptHeader::parse("application/json, text/html; q=0.9");
		assert_eq!(accept.media_types.len(), 2);
		assert_eq!(accept.media_types[0].quality, 1.0);
	}

	#[test]
	fn test_find_best_match() {
		let accept = AcceptHeader::parse("application/json, text/html");
		let available = vec![
			MediaType::new("text", "html"),
			MediaType::new("application", "xml"),
		];
		let best = accept.find_best_match(&available);
		assert!(best.is_some());
	}
}
