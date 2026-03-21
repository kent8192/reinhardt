//! Language negotiation based on Accept-Language header

/// Represents a language with quality factor
#[derive(Debug, Clone, PartialEq)]
pub struct Language {
	/// Language code (e.g., "en", "fr", "ja")
	pub code: String,
	/// Optional region (e.g., "US", "GB", "JP")
	pub region: Option<String>,
	/// Quality factor (0.0 to 1.0)
	pub quality: f32,
}

impl Language {
	/// Creates a new Language with quality 1.0
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::language::Language;
	///
	/// let en = Language::new("en");
	/// assert_eq!(en.code, "en");
	/// assert_eq!(en.quality, 1.0);
	/// assert_eq!(en.region, None);
	/// ```
	pub fn new(code: impl Into<String>) -> Self {
		Self {
			code: code.into(),
			region: None,
			quality: 1.0,
		}
	}

	/// Creates a Language with region
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::language::Language;
	///
	/// let en_us = Language::with_region("en", "US");
	/// assert_eq!(en_us.code, "en");
	/// assert_eq!(en_us.region, Some("US".to_string()));
	/// ```
	pub fn with_region(code: impl Into<String>, region: impl Into<String>) -> Self {
		Self {
			code: code.into(),
			region: Some(region.into()),
			quality: 1.0,
		}
	}

	/// Parses a language string (e.g., "en-US;q=0.9")
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::language::Language;
	///
	/// let lang = Language::parse("en-US;q=0.9").unwrap();
	/// assert_eq!(lang.code, "en");
	/// assert_eq!(lang.region, Some("US".to_string()));
	/// assert_eq!(lang.quality, 0.9);
	///
	/// let simple = Language::parse("fr").unwrap();
	/// assert_eq!(simple.code, "fr");
	/// assert_eq!(simple.quality, 1.0);
	/// ```
	pub fn parse(s: &str) -> Option<Self> {
		let parts: Vec<&str> = s.split(';').collect();
		let lang_part = parts.first()?.trim();

		let (code, region) = if let Some((c, r)) = lang_part.split_once('-') {
			(c.to_lowercase(), Some(r.to_uppercase()))
		} else {
			(lang_part.to_lowercase(), None)
		};

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

		Some(Self {
			code,
			region,
			quality,
		})
	}

	/// Checks if this language matches another (considering wildcards and regions)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::language::Language;
	///
	/// let en_us = Language::with_region("en", "US");
	/// let en = Language::new("en");
	/// let fr = Language::new("fr");
	///
	/// assert!(en_us.matches(&en)); // en-US matches en
	/// assert!(en.matches(&en_us)); // en matches en-US
	/// assert!(!en_us.matches(&fr)); // en-US doesn't match fr
	/// ```
	pub fn matches(&self, other: &Language) -> bool {
		if self.code == "*" || other.code == "*" {
			return true;
		}

		if self.code != other.code {
			return false;
		}

		// If either has no region, it matches
		match (&self.region, &other.region) {
			(None, _) | (_, None) => true,
			(Some(r1), Some(r2)) => r1 == r2,
		}
	}

	/// Returns the full language tag (e.g., "en-US")
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::language::Language;
	///
	/// let en_us = Language::with_region("en", "US");
	/// assert_eq!(en_us.tag(), "en-US");
	///
	/// let en = Language::new("en");
	/// assert_eq!(en.tag(), "en");
	/// ```
	pub fn tag(&self) -> String {
		match &self.region {
			Some(region) => format!("{}-{}", self.code, region),
			None => self.code.clone(),
		}
	}
}

/// Language negotiator for Accept-Language header
#[derive(Debug, Clone)]
pub struct LanguageNegotiator {
	fallback: Language,
}

impl LanguageNegotiator {
	/// Creates a new LanguageNegotiator with "en" as fallback
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::language::LanguageNegotiator;
	///
	/// let negotiator = LanguageNegotiator::new();
	/// ```
	pub fn new() -> Self {
		Self {
			fallback: Language::new("en"),
		}
	}

	/// Creates a LanguageNegotiator with custom fallback language
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::language::{LanguageNegotiator, Language};
	///
	/// let negotiator = LanguageNegotiator::with_fallback(Language::new("ja"));
	/// ```
	pub fn with_fallback(fallback: Language) -> Self {
		Self { fallback }
	}

	/// Negotiates the best language based on Accept-Language header
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::language::{LanguageNegotiator, Language};
	///
	/// let negotiator = LanguageNegotiator::new();
	/// let available = vec![
	///     Language::new("en"),
	///     Language::new("fr"),
	///     Language::new("ja"),
	/// ];
	///
	/// // Exact match
	/// let result = negotiator.negotiate("fr, en;q=0.9", &available);
	/// assert_eq!(result.code, "fr");
	///
	/// // Quality-based selection
	/// let result2 = negotiator.negotiate("ja;q=0.5, en;q=0.9", &available);
	/// assert_eq!(result2.code, "en");
	///
	/// // Fallback when no match
	/// let result3 = negotiator.negotiate("de", &available);
	/// assert_eq!(result3.code, "en"); // fallback
	/// ```
	pub fn negotiate(&self, accept_language: &str, available: &[Language]) -> Language {
		let mut requested = self.parse_accept_language(accept_language);

		// Sort by quality (highest first)
		// Non-finite values are rejected at parse time; unwrap_or is a safety net
		requested.sort_by(|a, b| {
			b.quality
				.partial_cmp(&a.quality)
				.unwrap_or(std::cmp::Ordering::Equal)
		});

		for req in &requested {
			for avail in available {
				if req.matches(avail) {
					return avail.clone();
				}
			}
		}

		self.fallback.clone()
	}

	/// Parses Accept-Language header into a list of languages
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::language::LanguageNegotiator;
	///
	/// let negotiator = LanguageNegotiator::new();
	/// let languages = negotiator.parse_accept_language("en-US, fr;q=0.9, ja;q=0.8");
	/// assert_eq!(languages.len(), 3);
	/// assert_eq!(languages[0].code, "en");
	/// assert_eq!(languages[0].quality, 1.0);
	/// assert_eq!(languages[1].quality, 0.9);
	/// ```
	pub fn parse_accept_language(&self, header: &str) -> Vec<Language> {
		header
			.split(',')
			.filter_map(|s| Language::parse(s.trim()))
			.collect()
	}

	/// Finds all matching languages in priority order
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::language::{LanguageNegotiator, Language};
	///
	/// let negotiator = LanguageNegotiator::new();
	/// let available = vec![
	///     Language::new("en"),
	///     Language::new("fr"),
	///     Language::new("ja"),
	/// ];
	///
	/// let matches = negotiator.find_all_matches("en, fr, de", &available);
	/// assert_eq!(matches.len(), 2); // en and fr match
	/// assert_eq!(matches[0].code, "en");
	/// assert_eq!(matches[1].code, "fr");
	/// ```
	pub fn find_all_matches(&self, accept_language: &str, available: &[Language]) -> Vec<Language> {
		let mut requested = self.parse_accept_language(accept_language);
		// Use unwrap_or as a safety net in case NaN slips through
		requested.sort_by(|a, b| {
			b.quality
				.partial_cmp(&a.quality)
				.unwrap_or(std::cmp::Ordering::Equal)
		});

		let mut matches = Vec::new();
		for req in &requested {
			for avail in available {
				if req.matches(avail) && !matches.iter().any(|m: &Language| m.code == avail.code) {
					matches.push(avail.clone());
				}
			}
		}

		matches
	}
}

impl Default for LanguageNegotiator {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[test]
	fn test_language_parse() {
		let lang = Language::parse("en-US;q=0.9").unwrap();
		assert_eq!(lang.code, "en");
		assert_eq!(lang.region, Some("US".to_string()));
		assert_eq!(lang.quality, 0.9);
	}

	#[test]
	fn test_language_matches() {
		let en_us = Language::with_region("en", "US");
		let en = Language::new("en");
		assert!(en_us.matches(&en));
		assert!(en.matches(&en_us));
	}

	#[test]
	fn test_negotiate() {
		let negotiator = LanguageNegotiator::new();
		let available = vec![Language::new("en"), Language::new("fr")];

		let result = negotiator.negotiate("fr, en;q=0.9", &available);
		assert_eq!(result.code, "fr");
	}

	#[test]
	fn test_negotiate_fallback() {
		let negotiator = LanguageNegotiator::new();
		let available = vec![Language::new("en"), Language::new("fr")];

		let result = negotiator.negotiate("de", &available);
		assert_eq!(result.code, "en");
	}

	#[test]
	fn test_find_all_matches() {
		let negotiator = LanguageNegotiator::new();
		let available = vec![
			Language::new("en"),
			Language::new("fr"),
			Language::new("ja"),
		];

		let matches = negotiator.find_all_matches("en, fr, de", &available);
		assert_eq!(matches.len(), 2);
	}

	#[rstest]
	#[case("en-US;q=NaN")]
	#[case("en-US;q=inf")]
	#[case("en-US;q=-inf")]
	fn test_parse_rejects_non_finite_quality(#[case] input: &str) {
		// Arrange
		// (input provided by rstest case)

		// Act
		let result = Language::parse(input);

		// Assert
		assert_eq!(result, None, "Non-finite quality value should be rejected");
	}

	#[rstest]
	fn test_negotiate_does_not_panic_on_nan_quality() {
		// Arrange
		let negotiator = LanguageNegotiator::new();
		let available = vec![Language::new("en"), Language::new("fr")];

		// Act
		let result = negotiator.negotiate("en;q=NaN, fr;q=0.9", &available);

		// Assert
		assert_eq!(result.code, "fr");
	}

	#[rstest]
	fn test_find_all_matches_does_not_panic_on_nan_quality() {
		// Arrange
		let negotiator = LanguageNegotiator::new();
		let available = vec![Language::new("en"), Language::new("fr")];

		// Act
		let matches = negotiator.find_all_matches("en;q=NaN, fr;q=0.9", &available);

		// Assert
		assert_eq!(matches.len(), 1);
		assert_eq!(matches[0].code, "fr");
	}
}
