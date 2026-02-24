use std::fmt;

/// A type-safe URL for redirects.
///
/// This newtype provides basic validation and type safety for URLs
/// used in redirect operations.
///
/// # Examples
///
/// ```
/// use reinhardt_shortcuts::Url;
///
/// // Valid URLs
/// let url = Url::new("/home").unwrap();
/// let url = Url::new("https://example.com/page").unwrap();
///
/// // Invalid URL (empty)
/// assert!(Url::new("").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Url(String);

/// Error type for URL validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlError {
	/// The URL is empty
	Empty,
	/// The URL contains only whitespace
	Whitespace,
}

impl fmt::Display for UrlError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			UrlError::Empty => write!(f, "URL cannot be empty"),
			UrlError::Whitespace => write!(f, "URL cannot contain only whitespace"),
		}
	}
}

impl std::error::Error for UrlError {}

impl Url {
	/// Creates a new URL with validation.
	///
	/// # Errors
	///
	/// Returns `UrlError::Empty` if the URL is empty.
	/// Returns `UrlError::Whitespace` if the URL contains only whitespace.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_shortcuts::Url;
	///
	/// let url = Url::new("/home")?;
	/// assert_eq!(url.as_str(), "/home");
	///
	/// assert!(Url::new("").is_err());
	/// assert!(Url::new("   ").is_err());
	/// # Ok::<(), reinhardt_shortcuts::UrlError>(())
	/// ```
	pub fn new(url: impl Into<String>) -> Result<Self, UrlError> {
		let url = url.into();

		if url.is_empty() {
			return Err(UrlError::Empty);
		}

		if url.trim().is_empty() {
			return Err(UrlError::Whitespace);
		}

		Ok(Self(url))
	}

	/// Returns the URL as a string slice.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_shortcuts::Url;
	///
	/// let url = Url::new("/page")?;
	/// assert_eq!(url.as_str(), "/page");
	/// # Ok::<(), reinhardt_shortcuts::UrlError>(())
	/// ```
	pub fn as_str(&self) -> &str {
		&self.0
	}

	/// Converts the URL into a String.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_shortcuts::Url;
	///
	/// let url = Url::new("/about")?;
	/// let string: String = url.into_string();
	/// assert_eq!(string, "/about");
	/// # Ok::<(), reinhardt_shortcuts::UrlError>(())
	/// ```
	pub fn into_string(self) -> String {
		self.0
	}
}

impl fmt::Display for Url {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl AsRef<str> for Url {
	fn as_ref(&self) -> &str {
		&self.0
	}
}

impl TryFrom<String> for Url {
	type Error = UrlError;

	fn try_from(s: String) -> Result<Self, Self::Error> {
		Self::new(s)
	}
}

impl TryFrom<&str> for Url {
	type Error = UrlError;

	fn try_from(s: &str) -> Result<Self, Self::Error> {
		Self::new(s)
	}
}
