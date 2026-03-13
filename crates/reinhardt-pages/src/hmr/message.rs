//! HMR protocol messages.

use serde::{Deserialize, Serialize};

/// Messages sent from the HMR server to connected browser clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HmrMessage {
	/// A CSS file was updated - the client should replace the stylesheet.
	CssUpdate {
		/// The path of the changed CSS file, relative to the project root.
		path: String,
	},
	/// A non-CSS file was changed - the client should perform a full page reload.
	FullReload {
		/// Human-readable reason for the reload.
		reason: String,
	},
	/// Initial connection acknowledgment.
	Connected,
}

impl HmrMessage {
	/// Serializes the message to a JSON string.
	pub fn to_json(&self) -> Result<String, serde_json::Error> {
		serde_json::to_string(self)
	}

	/// Deserializes a message from a JSON string.
	pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
		serde_json::from_str(json)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_css_update_serialization() {
		// Arrange
		let msg = HmrMessage::CssUpdate {
			path: "styles/main.css".to_string(),
		};

		// Act
		let json = msg.to_json().unwrap();
		let deserialized = HmrMessage::from_json(&json).unwrap();

		// Assert
		assert_eq!(msg, deserialized);
		assert!(json.contains("\"type\":\"css_update\""));
		assert!(json.contains("\"path\":\"styles/main.css\""));
	}

	#[rstest]
	fn test_full_reload_serialization() {
		// Arrange
		let msg = HmrMessage::FullReload {
			reason: "Rust source changed".to_string(),
		};

		// Act
		let json = msg.to_json().unwrap();
		let deserialized = HmrMessage::from_json(&json).unwrap();

		// Assert
		assert_eq!(msg, deserialized);
		assert!(json.contains("\"type\":\"full_reload\""));
		assert!(json.contains("\"reason\":\"Rust source changed\""));
	}

	#[rstest]
	fn test_connected_serialization() {
		// Arrange
		let msg = HmrMessage::Connected;

		// Act
		let json = msg.to_json().unwrap();
		let deserialized = HmrMessage::from_json(&json).unwrap();

		// Assert
		assert_eq!(msg, deserialized);
		assert!(json.contains("\"type\":\"connected\""));
	}

	#[rstest]
	fn test_deserialization_from_raw_json() {
		// Arrange
		let json = r#"{"type":"css_update","path":"app.css"}"#;

		// Act
		let msg = HmrMessage::from_json(json).unwrap();

		// Assert
		assert_eq!(
			msg,
			HmrMessage::CssUpdate {
				path: "app.css".to_string()
			}
		);
	}
}
