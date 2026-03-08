//! Media type representation

use std::fmt;

/// Represents a media type (MIME type)
#[derive(Debug, Clone, PartialEq)]
pub struct MediaType {
	/// Primary type (e.g., "application", "text", "*").
	pub type_: String,
	/// Subtype (e.g., "json", "html", "*").
	pub subtype: String,
	/// Additional parameters (e.g., charset=utf-8), excluding quality.
	pub parameters: Vec<(String, String)>,
	/// Quality factor between 0.0 and 1.0 (higher is preferred).
	pub quality: f32,
}

impl MediaType {
	/// Creates a new MediaType with the specified type and subtype
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::MediaType;
	///
	/// let json = MediaType::new("application", "json");
	/// assert_eq!(json.type_, "application");
	/// assert_eq!(json.subtype, "json");
	/// assert_eq!(json.quality, 1.0);
	/// ```
	pub fn new(type_: impl Into<String>, subtype: impl Into<String>) -> Self {
		Self {
			type_: type_.into(),
			subtype: subtype.into(),
			parameters: Vec::new(),
			quality: 1.0,
		}
	}
	/// Parses a media type string into a MediaType struct
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::MediaType;
	///
	/// let mt = MediaType::parse("application/json; q=0.8").unwrap();
	/// assert_eq!(mt.type_, "application");
	/// assert_eq!(mt.subtype, "json");
	/// assert_eq!(mt.quality, 0.8);
	///
	/// let mt_with_params = MediaType::parse("text/html; charset=utf-8").unwrap();
	/// assert_eq!(mt_with_params.parameters.len(), 1);
	/// assert_eq!(mt_with_params.parameters[0].0, "charset");
	/// assert_eq!(mt_with_params.parameters[0].1, "utf-8");
	/// ```
	pub fn parse(s: &str) -> Option<Self> {
		let parts: Vec<&str> = s.split(';').collect();
		let mime_type = parts.first()?.trim();

		let type_parts: Vec<&str> = mime_type.split('/').collect();
		if type_parts.len() != 2 {
			return None;
		}

		let mut media_type = MediaType::new(type_parts[0], type_parts[1]);

		// Parse parameters
		for param in parts.iter().skip(1) {
			let param = param.trim();
			if let Some((key, value)) = param.split_once('=') {
				let key = key.trim();
				let value = value.trim();

				if key == "q" {
					if let Ok(q) = value.parse::<f32>() {
						media_type.quality = q.clamp(0.0, 1.0);
					}
				} else {
					media_type
						.parameters
						.push((key.to_string(), value.to_string()));
				}
			}
		}

		Some(media_type)
	}
	/// Checks if this media type matches another, supporting wildcards
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::MediaType;
	///
	/// let json = MediaType::new("application", "json");
	/// let wildcard = MediaType::new("*", "*");
	/// let app_wildcard = MediaType::new("application", "*");
	///
	/// assert!(json.matches(&wildcard));
	/// assert!(json.matches(&app_wildcard));
	/// assert!(json.matches(&MediaType::new("application", "json")));
	/// assert!(!json.matches(&MediaType::new("text", "html")));
	/// ```
	pub fn matches(&self, other: &MediaType) -> bool {
		let main_type_match = self.type_ == "*" || other.type_ == "*" || self.type_ == other.type_;
		let subtype_match =
			self.subtype == "*" || other.subtype == "*" || self.subtype == other.subtype;

		if !main_type_match || !subtype_match {
			return false;
		}

		// Check parameter matching (excluding quality)
		for (key, value) in &self.parameters {
			if let Some(other_value) = other.parameters.iter().find(|(k, _)| k == key)
				&& other_value.1 != *value
			{
				return false;
			}
		}

		true
	}
	/// Calculate precedence for content negotiation
	/// Higher precedence = more specific
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::MediaType;
	///
	/// let wildcard = MediaType::new("*", "*");
	/// let app_wildcard = MediaType::new("application", "*");
	/// let json = MediaType::new("application", "json");
	///
	/// assert_eq!(wildcard.precedence(), 0);
	/// assert_eq!(app_wildcard.precedence(), 2);
	/// assert_eq!(json.precedence(), 3);
	/// assert!(json.precedence() > app_wildcard.precedence());
	/// ```
	pub fn precedence(&self) -> i32 {
		let mut prec = 0;

		// Wildcard main type = lowest precedence
		if self.type_ == "*" {
			prec += 0;
		} else {
			prec += 2;
		}

		// Wildcard subtype
		if self.subtype == "*" {
			prec += 0;
		} else {
			prec += 1;
		}

		prec
	}

	/// Full string representation including parameters
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::negotiation::MediaType;
	///
	/// let mut json = MediaType::new("application", "json");
	/// json.parameters.push(("charset".to_string(), "utf-8".to_string()));
	/// assert_eq!(json.full_string(), "application/json; charset=utf-8");
	///
	/// let simple = MediaType::new("text", "html");
	/// assert_eq!(simple.full_string(), "text/html");
	/// ```
	pub fn full_string(&self) -> String {
		let mut s = format!("{}/{}", self.type_, self.subtype);
		for (key, value) in &self.parameters {
			s.push_str(&format!("; {}={}", key, value));
		}
		s
	}
}

impl fmt::Display for MediaType {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "{}/{}", self.type_, self.subtype)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_parse_media_type() {
		let mt = MediaType::parse("application/json; q=0.8").unwrap();
		assert_eq!(mt.type_, "application");
		assert_eq!(mt.subtype, "json");
		assert_eq!(mt.quality, 0.8);
	}

	#[test]
	fn test_matches() {
		let json = MediaType::new("application", "json");
		let wildcard = MediaType::new("*", "*");
		assert!(json.matches(&wildcard));
	}
}
