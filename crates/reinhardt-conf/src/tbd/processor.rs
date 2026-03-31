//! File-level scanner for TBD DSL `![...]` markers.
//!
//! Scans template strings for `![expression]` markers, evaluates
//! each expression through the parse-typecheck-generate pipeline,
//! and replaces the markers with serialized TOML values.

use crate::tbd::error::{Span, TbdError};
use crate::tbd::evaluator::generate;
use crate::tbd::parser::parse_expression;
use crate::tbd::typechecker::typecheck;

/// Process a template string, replacing all `![...]` expressions with generated TOML values.
///
/// Scans through the input looking for `![expression]` markers that are not
/// inside TOML comments (`# ...`) or TOML string values (`"..."`).
/// Each expression is parsed, type-checked, and evaluated, then the marker
/// is replaced with the serialized TOML representation of the result.
///
/// # Errors
///
/// Returns a [`TbdError`] if any expression fails to parse, type-check, or evaluate,
/// or if a `![` marker has no matching closing bracket.
pub fn process_template(input: &str) -> Result<String, TbdError> {
	let mut output = String::with_capacity(input.len());
	let mut pos = 0;
	let bytes = input.as_bytes();

	while pos < bytes.len() {
		if let Some(marker_pos) = find_marker_start(input, pos) {
			// Append everything before the marker
			output.push_str(&input[pos..marker_pos]);

			// Find the matching closing bracket
			let expr_start = marker_pos + 2; // skip "!["
			let close_pos = find_matching_bracket(input, expr_start).ok_or_else(|| {
				TbdError::ParseError {
					message: "unclosed `![` marker: no matching `]` found".to_string(),
					span: Span {
						start: marker_pos,
						end: input.len(),
					},
				}
			})?;

			let expr_text = &input[expr_start..close_pos];

			// Run the pipeline: parse -> typecheck -> generate
			let ast = parse_expression(expr_text)?;
			let _ty = typecheck(&ast)?;
			let value = generate(&ast)?;

			// Serialize and append
			output.push_str(&serialize_toml_value(&value));

			pos = close_pos + 1; // skip past ']'
		} else {
			// No more markers; append the rest
			output.push_str(&input[pos..]);
			break;
		}
	}

	Ok(output)
}

/// Find the byte position of the next `![` marker that is NOT inside a
/// TOML comment (`# ...` until end of line) or a TOML string (`"..."`).
///
/// Returns `None` if no valid marker is found from `start` onward.
fn find_marker_start(text: &str, start: usize) -> Option<usize> {
	let bytes = text.as_bytes();
	let mut i = start;
	let mut in_string = false;
	let mut in_comment = false;

	while i < bytes.len() {
		let ch = bytes[i];

		if in_comment {
			if ch == b'\n' {
				in_comment = false;
			}
			i += 1;
			continue;
		}

		if in_string {
			if ch == b'\\' {
				// Skip escaped character
				i += 2;
				continue;
			}
			if ch == b'"' {
				in_string = false;
			}
			i += 1;
			continue;
		}

		// Not in comment or string
		if ch == b'#' {
			in_comment = true;
			i += 1;
			continue;
		}

		if ch == b'"' {
			in_string = true;
			i += 1;
			continue;
		}

		if ch == b'!' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
			return Some(i);
		}

		i += 1;
	}

	None
}

/// Find the byte position of the `]` that closes the `![...]` marker,
/// handling nested brackets `[...]` and parentheses `(...)`, and skipping
/// content inside double-quoted strings.
///
/// `start` should point to the first character after `![`.
/// Returns the byte position of the matching `]`, or `None` if unmatched.
fn find_matching_bracket(text: &str, start: usize) -> Option<usize> {
	let bytes = text.as_bytes();
	let mut depth: usize = 1; // We're already inside one `[`
	let mut i = start;
	let mut in_string = false;

	while i < bytes.len() {
		let ch = bytes[i];

		if in_string {
			if ch == b'\\' {
				// Skip escaped character
				i += 2;
				continue;
			}
			if ch == b'"' {
				in_string = false;
			}
			i += 1;
			continue;
		}

		match ch {
			b'"' => {
				in_string = true;
			}
			b'[' => {
				depth += 1;
			}
			b']' => {
				depth -= 1;
				if depth == 0 {
					return Some(i);
				}
			}
			b'(' => {
				depth += 1;
			}
			b')' => {
				depth -= 1;
			}
			_ => {}
		}

		i += 1;
	}

	None
}

/// Convert a [`toml::Value`] to its inline TOML text representation.
fn serialize_toml_value(value: &toml::Value) -> String {
	match value {
		toml::Value::String(s) => {
			// Escape special characters for TOML string
			let escaped = s
				.replace('\\', "\\\\")
				.replace('"', "\\\"")
				.replace('\n', "\\n")
				.replace('\r', "\\r")
				.replace('\t', "\\t");
			format!("\"{escaped}\"")
		}
		toml::Value::Integer(n) => n.to_string(),
		toml::Value::Float(f) => {
			let s = f.to_string();
			// Ensure a decimal point is present
			if s.contains('.') {
				s
			} else {
				format!("{s}.0")
			}
		}
		toml::Value::Boolean(b) => b.to_string(),
		toml::Value::Array(arr) => {
			let items: Vec<String> = arr.iter().map(serialize_toml_value).collect();
			format!("[{}]", items.join(", "))
		}
		toml::Value::Datetime(dt) => dt.to_string(),
		toml::Value::Table(table) => {
			// Inline table representation
			let pairs: Vec<String> = table
				.iter()
				.map(|(k, v)| format!("{k} = {}", serialize_toml_value(v)))
				.collect();
			format!("{{{}}}", pairs.join(", "))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// Happy path tests

	#[rstest]
	fn test_process_fixed_boolean() {
		// Arrange
		let input = "debug = ![false | fixed | TBD]\n";

		// Act
		let output = process_template(input).unwrap();

		// Assert
		assert_eq!(output, "debug = false\n");
	}

	#[rstest]
	fn test_process_fixed_integer() {
		// Arrange
		let input = "port = ![8080 | fixed | TBD]\n";

		// Act
		let output = process_template(input).unwrap();

		// Assert
		assert_eq!(output, "port = 8080\n");
	}

	#[rstest]
	fn test_process_fixed_string() {
		// Arrange
		let input = "name = ![\"myapp\" | fixed | TBD]\n";

		// Act
		let output = process_template(input).unwrap();

		// Assert
		assert_eq!(output, "name = \"myapp\"\n");
	}

	#[rstest]
	fn test_process_multiple() {
		// Arrange
		let input =
			"[settings]\ndebug = ![false | fixed | TBD]\nport = ![8080 | fixed | TBD]\n";

		// Act
		let output = process_template(input).unwrap();

		// Assert
		let parsed: toml::Value = toml::from_str(&output).unwrap();
		assert_eq!(parsed["settings"]["debug"], toml::Value::Boolean(false));
		assert_eq!(parsed["settings"]["port"], toml::Value::Integer(8080));
	}

	// Edge cases

	#[rstest]
	fn test_process_no_expressions() {
		// Arrange
		let input = "key = 42\n";

		// Act
		let output = process_template(input).unwrap();

		// Assert
		assert_eq!(output, input);
	}

	#[rstest]
	fn test_process_comment_ignored() {
		// Arrange
		let input = "# comment with ![fake]\nkey = 1\n";

		// Act
		let output = process_template(input).unwrap();

		// Assert
		assert_eq!(output, input);
	}

	#[rstest]
	fn test_process_string_value_ignored() {
		// Arrange
		let input = "key = \"![not a dsl]\"\n";

		// Act
		let output = process_template(input).unwrap();

		// Assert
		assert_eq!(output, input);
	}

	// Error tests

	#[rstest]
	fn test_process_unclosed_marker() {
		// Arrange / Act / Assert
		assert!(process_template("key = ![").is_err());
	}

	// Realistic use case

	#[rstest]
	fn test_process_realistic_settings() {
		// Arrange
		let input = "[general]\ndebug = ![false | fixed | TBD]\n\n[database]\nport = ![5432 | fixed | TBD]\nname = ![\"myapp_db\" | fixed | TBD]\n";

		// Act
		let output = process_template(input).unwrap();

		// Assert
		let parsed: toml::Value = toml::from_str(&output).unwrap();
		assert_eq!(parsed["general"]["debug"], toml::Value::Boolean(false));
		assert_eq!(parsed["database"]["port"], toml::Value::Integer(5432));
		assert_eq!(
			parsed["database"]["name"],
			toml::Value::String("myapp_db".into())
		);
	}

	// Sanity

	#[rstest]
	fn sanity_output_is_valid_toml() {
		// Arrange
		let input = "key = ![true | fixed | TBD]\n";

		// Act
		let output = process_template(input).unwrap();

		// Assert
		assert!(toml::from_str::<toml::Value>(&output).is_ok());
	}
}
