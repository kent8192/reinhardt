/// Escapes control characters and non-ASCII bytes in a string to `\xNN` notation.
///
/// This is useful for sanitizing log output to prevent control character injection.
pub fn escape_control_chars(s: &str) -> String {
	let mut result = String::with_capacity(s.len());

	for ch in s.chars() {
		if ch.is_control() || !ch.is_ascii() {
			// Escape control characters and non-ASCII characters as \xNN
			for byte in ch.to_string().as_bytes() {
				result.push_str(&format!("\\x{:02x}", byte));
			}
		} else {
			result.push(ch);
		}
	}

	result
}
/// A trait for formatting log records into string representations.
pub trait Formatter {}
/// A formatter tailored for server-side log output.
pub struct ServerFormatter;
/// A standard log formatter that produces a default textual representation.
pub struct StandardFormatter;
