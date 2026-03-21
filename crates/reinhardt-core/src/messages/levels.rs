//! Message level definitions

use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Message levels (similar to Django)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(i32)]
#[derive(Default)]
pub enum Level {
	/// Debug-level messages (value: 10).
	Debug = 10,
	/// Informational messages (value: 20). This is the default level.
	#[default]
	Info = 20,
	/// Success messages (value: 25).
	Success = 25,
	/// Warning messages (value: 30).
	Warning = 30,
	/// Error messages (value: 40).
	Error = 40,
	/// Custom level with a user-defined numeric value.
	Custom(i32), // Support for custom levels
}

impl Level {
	/// Returns the string representation of the level
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::Level;
	///
	/// assert_eq!(Level::Debug.as_str(), "debug");
	/// assert_eq!(Level::Info.as_str(), "info");
	/// assert_eq!(Level::Success.as_str(), "success");
	/// assert_eq!(Level::Warning.as_str(), "warning");
	/// assert_eq!(Level::Error.as_str(), "error");
	/// ```
	pub fn as_str(&self) -> &'static str {
		match self {
			Level::Debug => "debug",
			Level::Info => "info",
			Level::Success => "success",
			Level::Warning => "warning",
			Level::Error => "error",
			Level::Custom(_) => "custom",
		}
	}

	/// Creates a level from a numeric value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::Level;
	///
	/// assert_eq!(Level::from_value(10), Level::Debug);
	/// assert_eq!(Level::from_value(20), Level::Info);
	/// assert_eq!(Level::from_value(29), Level::Custom(29));
	/// ```
	pub fn from_value(value: i32) -> Self {
		match value {
			10 => Level::Debug,
			20 => Level::Info,
			25 => Level::Success,
			30 => Level::Warning,
			40 => Level::Error,
			v => Level::Custom(v),
		}
	}

	/// Returns the numeric value of the level
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_core::messages::Level;
	///
	/// assert_eq!(Level::Debug.value(), 10);
	/// assert_eq!(Level::Info.value(), 20);
	/// assert_eq!(Level::Custom(29).value(), 29);
	/// ```
	pub fn value(&self) -> i32 {
		match self {
			Level::Debug => 10,
			Level::Info => 20,
			Level::Success => 25,
			Level::Warning => 30,
			Level::Error => 40,
			Level::Custom(v) => *v,
		}
	}
}

impl PartialOrd for Level {
	fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
		Some(self.cmp(other))
	}
}

impl Ord for Level {
	fn cmp(&self, other: &Self) -> std::cmp::Ordering {
		self.value().cmp(&other.value())
	}
}

impl FromStr for Level {
	type Err = String;

	/// Parses a level from a string (case-insensitive)
	///
	/// # Examples
	///
	/// ```
	/// use std::str::FromStr;
	/// use reinhardt_core::messages::Level;
	///
	/// assert_eq!(Level::from_str("debug"), Ok(Level::Debug));
	/// assert_eq!(Level::from_str("INFO"), Ok(Level::Info));
	/// assert_eq!(Level::from_str("Success"), Ok(Level::Success));
	/// assert!(Level::from_str("invalid").is_err());
	/// ```
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s.to_lowercase().as_str() {
			"debug" => Ok(Level::Debug),
			"info" => Ok(Level::Info),
			"success" => Ok(Level::Success),
			"warning" => Ok(Level::Warning),
			"error" => Ok(Level::Error),
			_ => Err(format!("Unknown message level: {}", s)),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Tests from Django messages_tests/base.py - BaseTests
	#[test]
	fn test_default_level() {
		let default_level = Level::default();
		assert_eq!(default_level, Level::Info);
	}

	#[test]
	fn test_low_level() {
		// Test that Debug (10) is lower than Info (20)
		assert!(Level::Debug < Level::Info);
		assert_eq!(Level::Debug.value(), 10);
	}

	#[test]
	fn test_high_level() {
		// Test that Warning (30) and Error (40) are higher levels
		assert!(Level::Warning > Level::Info);
		assert!(Level::Error > Level::Warning);
		assert_eq!(Level::Warning.value(), 30);
		assert_eq!(Level::Error.value(), 40);
	}

	#[test]
	fn test_messages_level_from_str() {
		assert_eq!(Level::from_str("debug"), Ok(Level::Debug));
		assert_eq!(Level::from_str("info"), Ok(Level::Info));
		assert_eq!(Level::from_str("success"), Ok(Level::Success));
		assert_eq!(Level::from_str("warning"), Ok(Level::Warning));
		assert_eq!(Level::from_str("error"), Ok(Level::Error));

		// Test case insensitivity
		assert_eq!(Level::from_str("DEBUG"), Ok(Level::Debug));
		assert_eq!(Level::from_str("INFO"), Ok(Level::Info));
		assert_eq!(Level::from_str("WARNING"), Ok(Level::Warning));

		// Test invalid input
		assert!(Level::from_str("invalid").is_err());
		assert!(Level::from_str("").is_err());
	}

	#[test]
	fn test_level_values() {
		// Verify the exact numeric values match Django's conventions
		assert_eq!(Level::Debug.value(), 10);
		assert_eq!(Level::Info.value(), 20);
		assert_eq!(Level::Success.value(), 25);
		assert_eq!(Level::Warning.value(), 30);
		assert_eq!(Level::Error.value(), 40);
	}

	#[test]
	fn test_custom_level() {
		// Test custom level creation and value retrieval
		let custom_level = Level::Custom(29);
		assert_eq!(custom_level.value(), 29);
		assert_eq!(custom_level.as_str(), "custom");

		// Test from_value with custom level
		let from_value = Level::from_value(29);
		assert_eq!(from_value, Level::Custom(29));
	}

	#[test]
	fn test_level_ordering_with_custom() {
		// Test that custom levels maintain proper ordering
		let debug = Level::Debug;
		let custom_low = Level::Custom(15);
		let info = Level::Info;
		let custom_high = Level::Custom(35);
		let error = Level::Error;

		assert!(custom_low > debug);
		assert!(custom_low < info);
		assert!(custom_high > info);
		assert!(custom_high < error);
	}
}
