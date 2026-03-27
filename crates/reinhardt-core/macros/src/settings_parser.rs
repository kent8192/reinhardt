//! nom v8.0.0 parser for `#[settings(...)]` attribute syntax.
//!
//! Parses `key: Type | Type | !Type` composition syntax.
//! `Type` without `key:` is a type-only entry with inferred field name.
//! `key: Type { field: required, field: optional }` adds per-field policy overrides.

use nom::Parser;
use nom::branch::alt;
use nom::bytes::tag;
use nom::character::complete::{alpha1, alphanumeric1, char, multispace0};
use nom::combinator::{complete, opt, recognize, value};
use nom::multi::{many0, separated_list1};
use nom::sequence::{delimited, pair, preceded, separated_pair};

/// Policy kind parsed from override blocks.
///
/// Converted to `FieldRequirement` token references during code generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PolicyKind {
	Required,
	Optional,
}

/// A field-level policy override parsed from `{ field: required }` blocks.
#[derive(Debug, Clone)]
pub(crate) struct FieldOverride {
	/// Name of the field in the fragment struct.
	pub(crate) field_name: String,
	/// Policy to apply for this field.
	pub(crate) policy: PolicyKind,
}

/// A parsed entry from the settings composition attribute.
#[derive(Debug, Clone)]
pub(crate) enum FragmentEntry {
	/// `key: TypeName` or `key: TypeName { field: required, ... }` — include with explicit field name.
	Include {
		/// Field name in the composed struct.
		key: String,
		/// Type name of the fragment.
		type_name: String,
		/// Per-field policy overrides specified in an inline `{ ... }` block.
		overrides: Vec<FieldOverride>,
	},
	/// `TypeName` — type-only entry, field name to be inferred.
	TypeOnly(String),
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

/// Parse a policy keyword: `required` or `optional`.
fn policy(input: &str) -> nom::IResult<&str, PolicyKind> {
	alt((
		value(PolicyKind::Required, tag("required")),
		value(PolicyKind::Optional, tag("optional")),
	))
	.parse(input)
}

/// Parse a single field override: `field_name: required` or `field_name: optional`.
fn field_override(input: &str) -> nom::IResult<&str, FieldOverride> {
	let (input, _) = multispace0(input)?;
	let (input, field_name) = ident(input)?;
	let (input, _) = multispace0(input)?;
	let (input, _) = char(':')(input)?;
	let (input, _) = multispace0(input)?;
	let (input, pol) = policy(input)?;
	let (input, _) = multispace0(input)?;
	Ok((
		input,
		FieldOverride {
			field_name: field_name.to_string(),
			policy: pol,
		},
	))
}

/// Parse a `{ field: policy, ... }` override block.
///
/// Supports empty blocks `{}`, trailing commas, and arbitrary whitespace.
fn field_overrides(input: &str) -> nom::IResult<&str, Vec<FieldOverride>> {
	let (input, _) = char('{')(input)?;
	let (input, _) = multispace0(input)?;

	// Handle empty block: {}
	if let Ok((remaining, _)) = char::<&str, nom::error::Error<&str>>('}')(input) {
		return Ok((remaining, vec![]));
	}

	// Parse first override, then zero or more comma-separated overrides
	let (input, first) = field_override(input)?;
	let (input, mut rest) = many0(preceded(
		delimited(multispace0, char(','), multispace0),
		field_override,
	))
	.parse(input)?;

	let (input, _) = multispace0(input)?;
	// Allow trailing comma
	let (input, _) = opt(char(',')).parse(input)?;
	let (input, _) = multispace0(input)?;
	let (input, _) = char('}')(input)?;

	rest.insert(0, first);
	Ok((input, rest))
}

/// Parse `key: TypeName` with an optional `{ field: policy, ... }` override block.
fn include_entry(input: &str) -> nom::IResult<&str, FragmentEntry> {
	let (input, (key, type_name)) =
		separated_pair(ident, delimited(multispace0, char(':'), multispace0), ident)
			.parse(input)?;
	let (input, _) = multispace0(input)?;
	let (input, overrides) = opt(field_overrides).parse(input)?;
	Ok((
		input,
		FragmentEntry::Include {
			key: key.to_string(),
			type_name: type_name.to_string(),
			overrides: overrides.unwrap_or_default(),
		},
	))
}

/// Parse `!TypeName`.
fn exclude_entry(input: &str) -> nom::IResult<&str, FragmentEntry> {
	preceded(char('!'), ident)
		.map(|s: &str| FragmentEntry::Exclude(s.to_string()))
		.parse(input)
}

/// Parse `TypeName` (type-only, no `key:` prefix, no `!` prefix).
fn type_only_entry(input: &str) -> nom::IResult<&str, FragmentEntry> {
	ident
		.map(|s: &str| FragmentEntry::TypeOnly(s.to_string()))
		.parse(input)
}

/// Parse a single entry (include, exclude, or type-only).
fn fragment_entry(input: &str) -> nom::IResult<&str, FragmentEntry> {
	alt((exclude_entry, include_entry, type_only_entry)).parse(input)
}

/// Parse the full settings attribute: `key: Type | Type | !Type`.
pub(crate) fn parse_settings_attr(input: &str) -> nom::IResult<&str, Vec<FragmentEntry>> {
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
			FragmentEntry::Include { key, type_name, .. }
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
	fn test_parse_single_type_only() {
		// Arrange
		let input = "CacheSettings";

		// Act
		let (remaining, entries) = parse_settings_attr(input).unwrap();

		// Assert
		assert!(remaining.is_empty());
		assert_eq!(entries.len(), 1);
		assert!(matches!(
			&entries[0],
			FragmentEntry::TypeOnly(name) if name == "CacheSettings"
		));
	}

	#[rstest]
	fn test_parse_multiple_type_only() {
		// Arrange
		let input = "CoreSettings | CacheSettings";

		// Act
		let (remaining, entries) = parse_settings_attr(input).unwrap();

		// Assert
		assert!(remaining.is_empty());
		assert_eq!(entries.len(), 2);
		assert!(matches!(&entries[0], FragmentEntry::TypeOnly(name) if name == "CoreSettings"));
		assert!(matches!(&entries[1], FragmentEntry::TypeOnly(name) if name == "CacheSettings"));
	}

	#[rstest]
	fn test_parse_mixed_syntax() {
		// Arrange
		let input = "CoreSettings | custom: MyConfig";

		// Act
		let (remaining, entries) = parse_settings_attr(input).unwrap();

		// Assert
		assert!(remaining.is_empty());
		assert_eq!(entries.len(), 2);
		assert!(matches!(&entries[0], FragmentEntry::TypeOnly(name) if name == "CoreSettings"));
		assert!(matches!(
			&entries[1],
			FragmentEntry::Include { key, type_name, .. }
				if key == "custom" && type_name == "MyConfig"
		));
	}

	#[rstest]
	fn test_parse_mixed_with_exclude() {
		// Arrange
		let input = "CoreSettings | !CacheSettings | custom: MyConfig";

		// Act
		let (remaining, entries) = parse_settings_attr(input).unwrap();

		// Assert
		assert!(remaining.is_empty());
		assert_eq!(entries.len(), 3);
		assert!(matches!(&entries[0], FragmentEntry::TypeOnly(name) if name == "CoreSettings"));
		assert!(matches!(&entries[1], FragmentEntry::Exclude(name) if name == "CacheSettings"));
		assert!(matches!(&entries[2], FragmentEntry::Include { key, .. } if key == "custom"));
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
			FragmentEntry::Include { key, type_name, .. }
				if key == "static_files" && type_name == "StaticSettings"
		));
	}

	// --- New tests for override block syntax ---

	#[rstest]
	fn test_parse_include_with_overrides() {
		// Arrange
		let input = "core: CoreSettings { debug: required, allowed_hosts: required }";

		// Act
		let result = parse_settings_attr(input).unwrap();

		// Assert
		assert_eq!(result.1.len(), 1);
		match &result.1[0] {
			FragmentEntry::Include {
				key,
				type_name,
				overrides,
			} => {
				assert_eq!(key, "core");
				assert_eq!(type_name, "CoreSettings");
				assert_eq!(overrides.len(), 2);
				assert_eq!(overrides[0].field_name, "debug");
				assert_eq!(overrides[0].policy, PolicyKind::Required);
				assert_eq!(overrides[1].field_name, "allowed_hosts");
				assert_eq!(overrides[1].policy, PolicyKind::Required);
			}
			_ => panic!("expected Include"),
		}
	}

	#[rstest]
	fn test_parse_include_without_overrides() {
		// Arrange
		let input = "cors: CorsSettings";

		// Act
		let result = parse_settings_attr(input).unwrap();

		// Assert
		match &result.1[0] {
			FragmentEntry::Include {
				key,
				type_name,
				overrides,
			} => {
				assert_eq!(key, "cors");
				assert_eq!(type_name, "CorsSettings");
				assert!(overrides.is_empty());
			}
			_ => panic!("expected Include"),
		}
	}

	#[rstest]
	fn test_parse_mixed_with_and_without_overrides() {
		// Arrange
		let input = "core: CoreSettings { secret_key: required } | cors: CorsSettings";

		// Act
		let result = parse_settings_attr(input).unwrap();

		// Assert
		assert_eq!(result.1.len(), 2);
		match &result.1[0] {
			FragmentEntry::Include { overrides, .. } => assert_eq!(overrides.len(), 1),
			_ => panic!("expected Include"),
		}
		match &result.1[1] {
			FragmentEntry::Include { overrides, .. } => assert!(overrides.is_empty()),
			_ => panic!("expected Include"),
		}
	}

	#[rstest]
	fn test_parse_override_with_trailing_comma() {
		// Arrange
		let input = "core: CoreSettings { debug: required, }";

		// Act
		let result = parse_settings_attr(input).unwrap();

		// Assert
		match &result.1[0] {
			FragmentEntry::Include { overrides, .. } => assert_eq!(overrides.len(), 1),
			_ => panic!("expected Include"),
		}
	}

	#[rstest]
	fn test_parse_optional_override() {
		// Arrange
		let input = "core: CoreSettings { debug: optional }";

		// Act
		let result = parse_settings_attr(input).unwrap();

		// Assert
		match &result.1[0] {
			FragmentEntry::Include { overrides, .. } => {
				assert_eq!(overrides[0].policy, PolicyKind::Optional);
			}
			_ => panic!("expected Include"),
		}
	}

	#[rstest]
	fn test_parse_empty_override_block() {
		// Arrange
		let input = "core: CoreSettings {}";

		// Act
		let result = parse_settings_attr(input).unwrap();

		// Assert
		match &result.1[0] {
			FragmentEntry::Include { overrides, .. } => {
				assert!(overrides.is_empty());
			}
			_ => panic!("expected Include"),
		}
	}
}
