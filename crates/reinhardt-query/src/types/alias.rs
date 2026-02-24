//! Alias type for dynamic SQL identifiers.
//!
//! This module provides the [`Alias`] type for creating identifiers at runtime.

use super::iden::Iden;
use std::fmt::Write;

/// A dynamic identifier that can be created at runtime.
///
/// Unlike enum-based identifiers which are fixed at compile time,
/// `Alias` allows creating identifiers from strings at runtime.
///
/// # Example
///
/// ```rust
/// use reinhardt_query::{Alias, Iden};
///
/// let alias = Alias::new("my_alias");
/// assert_eq!(alias.to_string(), "my_alias");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Alias(String);

impl Alias {
	/// Create a new alias from a string.
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_query::Alias;
	///
	/// let alias = Alias::new("my_table");
	/// ```
	pub fn new<S: Into<String>>(name: S) -> Self {
		Self(name.into())
	}
}

impl Iden for Alias {
	fn unquoted(&self, s: &mut dyn Write) {
		write!(s, "{}", self.0).unwrap();
	}
}

impl From<&str> for Alias {
	fn from(s: &str) -> Self {
		Self::new(s)
	}
}

impl From<String> for Alias {
	fn from(s: String) -> Self {
		Self::new(s)
	}
}

impl From<Alias> for String {
	fn from(alias: Alias) -> Self {
		alias.0
	}
}

impl AsRef<str> for Alias {
	fn as_ref(&self) -> &str {
		&self.0
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_alias_new() {
		let alias = Alias::new("test_alias");
		assert_eq!(alias.to_string(), "test_alias");
	}

	#[rstest]
	fn test_alias_from_str() {
		let alias: Alias = "from_str".into();
		assert_eq!(alias.to_string(), "from_str");
	}

	#[rstest]
	fn test_alias_from_string() {
		let alias: Alias = String::from("from_string").into();
		assert_eq!(alias.to_string(), "from_string");
	}

	#[rstest]
	fn test_alias_quoted() {
		let alias = Alias::new("my_alias");
		let mut s = String::new();
		alias.quoted('"', &mut s);
		assert_eq!(s, "\"my_alias\"");
	}

	#[rstest]
	fn test_alias_as_ref() {
		let alias = Alias::new("test");
		let s: &str = alias.as_ref();
		assert_eq!(s, "test");
	}
}
