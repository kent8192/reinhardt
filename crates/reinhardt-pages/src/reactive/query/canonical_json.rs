//! Canonical JSON encoding for server-function query keys.
//!
//! This encoder serializes values directly to JSON before sorting object keys.
//! Keeping numbers in their serialized representation preserves integers wider
//! than `u64` without enabling `serde_json`'s workspace-wide
//! `arbitrary_precision` feature.

use std::collections::BTreeMap;
use std::fmt;

use serde::Serialize;

pub(crate) fn encode<T>(value: &T) -> Result<String, CanonicalJsonError>
where
	T: Serialize,
{
	let json = serde_json::to_string(value)?;
	CanonicalJsonParser::new(&json).parse()
}

#[derive(Debug)]
pub(crate) struct CanonicalJsonError {
	message: String,
}

impl CanonicalJsonError {
	fn new(message: impl Into<String>) -> Self {
		Self {
			message: message.into(),
		}
	}
}

impl From<serde_json::Error> for CanonicalJsonError {
	fn from(error: serde_json::Error) -> Self {
		Self::new(format!("JSON serialization failed: {error}"))
	}
}

impl fmt::Display for CanonicalJsonError {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter.write_str(&self.message)
	}
}

impl std::error::Error for CanonicalJsonError {}

struct CanonicalJsonParser<'input> {
	input: &'input str,
	position: usize,
}

impl<'input> CanonicalJsonParser<'input> {
	fn new(input: &'input str) -> Self {
		Self { input, position: 0 }
	}

	fn parse(mut self) -> Result<String, CanonicalJsonError> {
		self.skip_whitespace();
		let value = self.parse_value()?;
		self.skip_whitespace();
		if self.position != self.input.len() {
			return Err(self.error("unexpected trailing JSON data"));
		}
		Ok(value)
	}

	fn parse_value(&mut self) -> Result<String, CanonicalJsonError> {
		self.skip_whitespace();
		match self.peek_byte() {
			Some(b'{') => self.parse_object(),
			Some(b'[') => self.parse_array(),
			Some(b'"') => self.parse_string().map(str::to_owned),
			Some(b't') => self.parse_literal("true"),
			Some(b'f') => self.parse_literal("false"),
			Some(b'n') => self.parse_literal("null"),
			Some(b'-' | b'0'..=b'9') => self.parse_number(),
			Some(_) => Err(self.error("invalid JSON value")),
			None => Err(self.error("unexpected end of JSON input")),
		}
	}

	fn parse_array(&mut self) -> Result<String, CanonicalJsonError> {
		self.expect_byte(b'[')?;
		self.skip_whitespace();
		if self.consume_byte(b']') {
			return Ok("[]".to_string());
		}

		let mut values = Vec::new();
		loop {
			values.push(self.parse_value()?);
			self.skip_whitespace();
			if self.consume_byte(b']') {
				break;
			}
			self.expect_byte(b',')?;
			self.skip_whitespace();
		}

		Ok(format!("[{}]", values.join(",")))
	}

	fn parse_object(&mut self) -> Result<String, CanonicalJsonError> {
		self.expect_byte(b'{')?;
		self.skip_whitespace();
		if self.consume_byte(b'}') {
			return Ok("{}".to_string());
		}

		let mut entries = BTreeMap::new();
		loop {
			self.skip_whitespace();
			let key: String = serde_json::from_str(self.parse_string()?)
				.map_err(|_| CanonicalJsonError::new("invalid JSON object key"))?;
			self.skip_whitespace();
			self.expect_byte(b':')?;
			let value = self.parse_value()?;
			entries.insert(key, value);
			self.skip_whitespace();
			if self.consume_byte(b'}') {
				break;
			}
			self.expect_byte(b',')?;
			self.skip_whitespace();
		}

		let mut json = String::from("{");
		for (index, (key, value)) in entries.into_iter().enumerate() {
			if index > 0 {
				json.push(',');
			}
			json.push_str(&serde_json::to_string(&key)?);
			json.push(':');
			json.push_str(&value);
		}
		json.push('}');
		Ok(json)
	}

	fn parse_string(&mut self) -> Result<&'input str, CanonicalJsonError> {
		let start = self.position;
		self.expect_byte(b'"')?;
		while let Some(byte) = self.peek_byte() {
			match byte {
				b'"' => {
					self.position += 1;
					return Ok(&self.input[start..self.position]);
				}
				b'\\' => self.skip_escape_sequence()?,
				0..=31 => return Err(self.error("control character in JSON string")),
				_ => self.position += 1,
			}
		}
		Err(self.error("unterminated JSON string"))
	}

	fn skip_escape_sequence(&mut self) -> Result<(), CanonicalJsonError> {
		self.position += 1;
		let Some(escaped) = self.peek_byte() else {
			return Err(self.error("unterminated JSON escape sequence"));
		};
		match escaped {
			b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => self.position += 1,
			b'u' => {
				let start = self.position + 1;
				let end = start + 4;
				let Some(code_point) = self.input.as_bytes().get(start..end) else {
					return Err(self.error("incomplete JSON unicode escape"));
				};
				if !code_point.iter().all(u8::is_ascii_hexdigit) {
					return Err(self.error("invalid JSON unicode escape"));
				}
				self.position = end;
			}
			_ => return Err(self.error("invalid JSON escape sequence")),
		}
		Ok(())
	}

	fn parse_literal(&mut self, literal: &str) -> Result<String, CanonicalJsonError> {
		let end = self.position + literal.len();
		if self.input.get(self.position..end) != Some(literal) {
			return Err(self.error("invalid JSON literal"));
		}
		self.position = end;
		Ok(literal.to_string())
	}

	fn parse_number(&mut self) -> Result<String, CanonicalJsonError> {
		let start = self.position;
		while matches!(self.peek_byte(), Some(byte) if !matches!(byte, b' ' | b'\n' | b'\r' | b'\t' | b',' | b']' | b'}'))
		{
			self.position += 1;
		}
		if start == self.position {
			return Err(self.error("invalid JSON number"));
		}
		Ok(self.input[start..self.position].to_string())
	}

	fn skip_whitespace(&mut self) {
		while matches!(self.peek_byte(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
			self.position += 1;
		}
	}

	fn expect_byte(&mut self, expected: u8) -> Result<(), CanonicalJsonError> {
		if self.consume_byte(expected) {
			Ok(())
		} else {
			Err(self.error("unexpected JSON token"))
		}
	}

	fn consume_byte(&mut self, expected: u8) -> bool {
		if self.peek_byte() == Some(expected) {
			self.position += 1;
			true
		} else {
			false
		}
	}

	fn peek_byte(&self) -> Option<u8> {
		self.input.as_bytes().get(self.position).copied()
	}

	fn error(&self, message: &str) -> CanonicalJsonError {
		CanonicalJsonError::new(format!("{message} at byte {}", self.position))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn preserves_large_integer_tokens() {
		assert_eq!(encode(&u128::MAX).unwrap(), u128::MAX.to_string());
	}

	#[test]
	fn canonicalizes_nested_objects_and_escaped_keys() {
		let json =
			CanonicalJsonParser::new(r#"{"z":{"\u0062":2,"a":1},"a":"quote: \" and slash: \\"}"#)
				.parse()
				.unwrap();

		assert_eq!(json, r#"{"a":"quote: \" and slash: \\","z":{"a":1,"b":2}}"#);
	}
}
