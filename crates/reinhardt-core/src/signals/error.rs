//! Signal error types

use std::fmt;

/// Signal errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalError {
	/// Human-readable error description.
	pub message: String,
}

impl fmt::Display for SignalError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.message)
	}
}

impl std::error::Error for SignalError {}

impl SignalError {
	/// Creates a new signal error with the given message.
	pub fn new(msg: impl Into<String>) -> Self {
		Self {
			message: msg.into(),
		}
	}
}
