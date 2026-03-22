//! nom v8.0.0 parser for `#[settings(...)]` attribute syntax.
//!
//! Parses `key: Type | key: Type | !Type` composition syntax.

use nom::Parser;
use nom::branch::alt;
use nom::bytes::tag;
use nom::character::complete::{alpha1, alphanumeric1, char, multispace0};
use nom::combinator::{complete, recognize};
use nom::multi::{many0, separated_list1};
use nom::sequence::{delimited, pair, preceded, separated_pair};

/// A parsed entry from the settings composition attribute.
#[derive(Debug, Clone)]
pub enum FragmentEntry {
	/// `key: TypeName` — include with explicit field name.
	Include {
		/// Field name in the composed struct.
		key: String,
		/// Type name of the fragment.
		type_name: String,
	},
	/// `!TypeName` — exclude an implicit fragment.
	Exclude(String),
}

/// Parse an identifier (field name or type name).
fn ident(input: &str) -> nom::IResult<&str, &str> {
	recognize(pair(
		alt((alpha1, tag("_"))),
		complete(many0(alt((alphanumeric1, tag("_"))))),
	))
	.parse(input)
}

/// Parse `key: TypeName`.
fn include_entry(input: &str) -> nom::IResult<&str, FragmentEntry> {
	separated_pair(
		ident,
		delimited(multispace0, char(':'), multispace0),
		ident,
	)
	.map(|(key, type_name)| FragmentEntry::Include {
		key: key.to_string(),
		type_name: type_name.to_string(),
	})
	.parse(input)
}

/// Parse `!TypeName`.
fn exclude_entry(input: &str) -> nom::IResult<&str, FragmentEntry> {
	preceded(char('!'), ident)
		.map(|s: &str| FragmentEntry::Exclude(s.to_string()))
		.parse(input)
}

/// Parse a single entry (include or exclude).
fn fragment_entry(input: &str) -> nom::IResult<&str, FragmentEntry> {
	alt((exclude_entry, include_entry)).parse(input)
}

/// Parse the full settings attribute: `key: Type | key: Type | !Type`.
pub fn parse_settings_attr(input: &str) -> nom::IResult<&str, Vec<FragmentEntry>> {
	complete(separated_list1(
		delimited(multispace0, char('|'), multispace0),
		fragment_entry,
	))
	.parse(input)
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_parse_single_include() {
		// Arrange
		let input = "cache: CacheSettings";

		// Act
		let (remaining, entries) = parse_settings_attr(input).unwrap();

		// Assert
		assert!(remaining.is_empty());
		assert_eq!(entries.len(), 1);
		assert!(matches!(
			&entries[0],
			FragmentEntry::Include { key, type_name }
				if key == "cache" && type_name == "CacheSettings"
		));
	}

	#[rstest]
	fn test_parse_single_exclude() {
		// Arrange
		let input = "!CoreSettings";

		// Act
		let (remaining, entries) = parse_settings_attr(input).unwrap();

		// Assert
		assert!(remaining.is_empty());
		assert_eq!(entries.len(), 1);
		assert!(matches!(
			&entries[0],
			FragmentEntry::Exclude(name) if name == "CoreSettings"
		));
	}

	#[rstest]
	fn test_parse_multiple_entries() {
		// Arrange
		let input = "cache: CacheSettings | session: SessionSettings | !CorsSettings";

		// Act
		let (remaining, entries) = parse_settings_attr(input).unwrap();

		// Assert
		assert!(remaining.is_empty());
		assert_eq!(entries.len(), 3);
		assert!(matches!(&entries[0], FragmentEntry::Include { key, .. } if key == "cache"));
		assert!(matches!(&entries[1], FragmentEntry::Include { key, .. } if key == "session"));
		assert!(matches!(&entries[2], FragmentEntry::Exclude(name) if name == "CorsSettings"));
	}

	#[rstest]
	fn test_parse_empty_input() {
		// Arrange
		let input = "";

		// Act
		let result = parse_settings_attr(input);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_parse_with_extra_whitespace() {
		// Arrange
		let input = "cache :  CacheSettings  |  !CoreSettings";

		// Act
		let (_remaining, entries) = parse_settings_attr(input).unwrap();

		// Assert
		assert_eq!(entries.len(), 2);
	}

	#[rstest]
	fn test_parse_underscore_identifiers() {
		// Arrange
		let input = "static_files: StaticSettings";

		// Act
		let (_remaining, entries) = parse_settings_attr(input).unwrap();

		// Assert
		assert!(matches!(
			&entries[0],
			FragmentEntry::Include { key, type_name }
				if key == "static_files" && type_name == "StaticSettings"
		));
	}
}
