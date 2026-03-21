//! Encoding negotiation based on Accept-Encoding header

/// Supported encoding types
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Encoding {
	/// Gzip compression
	Gzip,
	/// Brotli compression
	Brotli,
	/// Deflate compression
	Deflate,
	/// No compression (identity)
	Identity,
}

impl Encoding {
	/// Parses an encoding string
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::encoding::Encoding;
	///
	/// assert_eq!(Encoding::parse("gzip"), Some(Encoding::Gzip));
	/// assert_eq!(Encoding::parse("br"), Some(Encoding::Brotli));
	/// assert_eq!(Encoding::parse("deflate"), Some(Encoding::Deflate));
	/// assert_eq!(Encoding::parse("identity"), Some(Encoding::Identity));
	/// assert_eq!(Encoding::parse("unknown"), None);
	/// ```
	pub fn parse(s: &str) -> Option<Self> {
		match s.to_lowercase().as_str() {
			"gzip" | "x-gzip" => Some(Encoding::Gzip),
			"br" => Some(Encoding::Brotli),
			"deflate" => Some(Encoding::Deflate),
			"identity" => Some(Encoding::Identity),
			"*" => Some(Encoding::Identity),
			_ => None,
		}
	}

	/// Returns the encoding name as a string
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::encoding::Encoding;
	///
	/// assert_eq!(Encoding::Gzip.as_str(), "gzip");
	/// assert_eq!(Encoding::Brotli.as_str(), "br");
	/// assert_eq!(Encoding::Deflate.as_str(), "deflate");
	/// assert_eq!(Encoding::Identity.as_str(), "identity");
	/// ```
	pub fn as_str(&self) -> &str {
		match self {
			Encoding::Gzip => "gzip",
			Encoding::Brotli => "br",
			Encoding::Deflate => "deflate",
			Encoding::Identity => "identity",
		}
	}
}

impl std::fmt::Display for Encoding {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.as_str())
	}
}

/// Represents an encoding with quality factor
#[derive(Debug, Clone, PartialEq)]
pub struct EncodingQuality {
	/// The encoding type.
	pub encoding: Encoding,
	/// Quality factor between 0.0 and 1.0 (higher is preferred).
	pub quality: f32,
}

impl EncodingQuality {
	/// Creates a new EncodingQuality with quality 1.0
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::encoding::{Encoding, EncodingQuality};
	///
	/// let enc = EncodingQuality::new(Encoding::Gzip);
	/// assert_eq!(enc.encoding, Encoding::Gzip);
	/// assert_eq!(enc.quality, 1.0);
	/// ```
	pub fn new(encoding: Encoding) -> Self {
		Self {
			encoding,
			quality: 1.0,
		}
	}

	/// Creates an EncodingQuality with specified quality
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::encoding::{Encoding, EncodingQuality};
	///
	/// let enc = EncodingQuality::with_quality(Encoding::Gzip, 0.8);
	/// assert_eq!(enc.quality, 0.8);
	/// ```
	pub fn with_quality(encoding: Encoding, quality: f32) -> Self {
		Self {
			encoding,
			quality: quality.clamp(0.0, 1.0),
		}
	}

	/// Parses an encoding string with optional quality (e.g., "gzip;q=0.9")
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::encoding::{Encoding, EncodingQuality};
	///
	/// let enc = EncodingQuality::parse("gzip;q=0.9").unwrap();
	/// assert_eq!(enc.encoding, Encoding::Gzip);
	/// assert_eq!(enc.quality, 0.9);
	///
	/// let simple = EncodingQuality::parse("br").unwrap();
	/// assert_eq!(simple.encoding, Encoding::Brotli);
	/// assert_eq!(simple.quality, 1.0);
	/// ```
	pub fn parse(s: &str) -> Option<Self> {
		let parts: Vec<&str> = s.split(';').collect();
		let encoding = Encoding::parse(parts.first()?.trim())?;

		let mut quality = 1.0;
		for param in parts.iter().skip(1) {
			let param = param.trim();
			if let Some((key, value)) = param.split_once('=')
				&& key.trim() == "q"
				&& let Ok(q) = value.trim().parse::<f32>()
			{
				// Reject non-finite quality values (NaN, inf, -inf)
				if !q.is_finite() {
					return None;
				}
				quality = q.clamp(0.0, 1.0);
			}
		}

		Some(Self { encoding, quality })
	}
}

/// Encoding negotiator for Accept-Encoding header
#[derive(Debug, Clone)]
pub struct EncodingNegotiator {
	/// Default encoding preference order
	preference_order: Vec<Encoding>,
}

impl EncodingNegotiator {
	/// Creates a new EncodingNegotiator with default preference order
	///
	/// Default order: Brotli > Gzip > Deflate > Identity
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::encoding::EncodingNegotiator;
	///
	/// let negotiator = EncodingNegotiator::new();
	/// ```
	pub fn new() -> Self {
		Self {
			preference_order: vec![
				Encoding::Brotli,
				Encoding::Gzip,
				Encoding::Deflate,
				Encoding::Identity,
			],
		}
	}

	/// Creates an EncodingNegotiator with custom preference order
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::encoding::{EncodingNegotiator, Encoding};
	///
	/// let negotiator = EncodingNegotiator::with_preference(vec![
	///     Encoding::Gzip,
	///     Encoding::Identity,
	/// ]);
	/// ```
	pub fn with_preference(preference_order: Vec<Encoding>) -> Self {
		Self { preference_order }
	}

	/// Negotiates the best encoding based on Accept-Encoding header
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::encoding::{EncodingNegotiator, Encoding};
	///
	/// let negotiator = EncodingNegotiator::new();
	/// let available = vec![Encoding::Gzip, Encoding::Identity];
	///
	/// // Client accepts gzip
	/// let result = negotiator.negotiate("gzip, deflate, br", &available);
	/// assert_eq!(result, Encoding::Gzip);
	///
	/// // Client accepts only identity
	/// let result2 = negotiator.negotiate("identity", &available);
	/// assert_eq!(result2, Encoding::Identity);
	///
	/// // Quality-based selection
	/// let result3 = negotiator.negotiate("gzip;q=0.5, identity;q=1.0", &available);
	/// assert_eq!(result3, Encoding::Identity);
	/// ```
	pub fn negotiate(&self, accept_encoding: &str, available: &[Encoding]) -> Encoding {
		let mut requested = self.parse_accept_encoding(accept_encoding);

		// Sort by quality (highest first)
		// Non-finite values are rejected at parse time; unwrap_or is a safety net
		requested.sort_by(|a, b| {
			b.quality
				.partial_cmp(&a.quality)
				.unwrap_or(std::cmp::Ordering::Equal)
		});

		// Try to find best match based on client preference
		for req in &requested {
			if available.contains(&req.encoding) {
				return req.encoding.clone();
			}
		}

		// Fallback to server preference order
		for pref in &self.preference_order {
			if available.contains(pref) {
				return pref.clone();
			}
		}

		// Ultimate fallback
		Encoding::Identity
	}

	/// Parses Accept-Encoding header into a list of encodings with quality
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::encoding::EncodingNegotiator;
	///
	/// let negotiator = EncodingNegotiator::new();
	/// let encodings = negotiator.parse_accept_encoding("gzip, deflate;q=0.8, br;q=0.9");
	/// assert_eq!(encodings.len(), 3);
	/// assert_eq!(encodings[0].quality, 1.0); // gzip
	/// assert_eq!(encodings[1].quality, 0.8); // deflate
	/// ```
	pub fn parse_accept_encoding(&self, header: &str) -> Vec<EncodingQuality> {
		header
			.split(',')
			.filter_map(|s| EncodingQuality::parse(s.trim()))
			.collect()
	}

	/// Selects the best encoding considering both client and server preferences
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::encoding::{EncodingNegotiator, Encoding};
	///
	/// let negotiator = EncodingNegotiator::new();
	/// let available = vec![Encoding::Gzip, Encoding::Brotli, Encoding::Identity];
	///
	/// // Brotli has highest preference for both client and server
	/// let result = negotiator.select_best("br;q=1.0, gzip;q=0.9", &available);
	/// assert_eq!(result, Encoding::Brotli);
	///
	/// // When client prefers gzip, but server prefers brotli
	/// let result2 = negotiator.select_best("gzip;q=1.0, br;q=0.9", &available);
	/// assert_eq!(result2, Encoding::Gzip); // Client preference wins
	/// ```
	pub fn select_best(&self, accept_encoding: &str, available: &[Encoding]) -> Encoding {
		self.negotiate(accept_encoding, available)
	}
}

impl Default for EncodingNegotiator {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[test]
	fn test_encoding_parse() {
		assert_eq!(Encoding::parse("gzip"), Some(Encoding::Gzip));
		assert_eq!(Encoding::parse("br"), Some(Encoding::Brotli));
		assert_eq!(Encoding::parse("deflate"), Some(Encoding::Deflate));
		assert_eq!(Encoding::parse("identity"), Some(Encoding::Identity));
	}

	#[test]
	fn test_encoding_quality_parse() {
		let enc = EncodingQuality::parse("gzip;q=0.9").unwrap();
		assert_eq!(enc.encoding, Encoding::Gzip);
		assert_eq!(enc.quality, 0.9);
	}

	#[test]
	fn test_negotiate() {
		let negotiator = EncodingNegotiator::new();
		let available = vec![Encoding::Gzip, Encoding::Identity];

		let result = negotiator.negotiate("gzip, deflate", &available);
		assert_eq!(result, Encoding::Gzip);
	}

	#[test]
	fn test_negotiate_quality() {
		let negotiator = EncodingNegotiator::new();
		let available = vec![Encoding::Gzip, Encoding::Identity];

		let result = negotiator.negotiate("gzip;q=0.5, identity;q=1.0", &available);
		assert_eq!(result, Encoding::Identity);
	}

	#[test]
	fn test_negotiate_fallback() {
		let negotiator = EncodingNegotiator::new();
		let available = vec![Encoding::Identity];

		let result = negotiator.negotiate("br, gzip", &available);
		assert_eq!(result, Encoding::Identity);
	}

	#[rstest]
	#[case("gzip;q=NaN")]
	#[case("gzip;q=inf")]
	#[case("gzip;q=-inf")]
	fn test_parse_rejects_non_finite_quality(#[case] input: &str) {
		// Arrange
		// (input provided by rstest case)

		// Act
		let result = EncodingQuality::parse(input);

		// Assert
		assert_eq!(result, None, "Non-finite quality value should be rejected");
	}

	#[rstest]
	fn test_negotiate_does_not_panic_on_nan_quality() {
		// Arrange
		let negotiator = EncodingNegotiator::new();
		let available = vec![Encoding::Gzip, Encoding::Identity];

		// Act
		let result = negotiator.negotiate("gzip;q=NaN, identity;q=0.9", &available);

		// Assert
		assert_eq!(result, Encoding::Identity);
	}
}
