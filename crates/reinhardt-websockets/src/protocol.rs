//! Protocol-level WebSocket configuration

use tungstenite::protocol::WebSocketConfig as TungsteniteConfig;

/// Default maximum message size: 1 MB (1,048,576 bytes)
pub const DEFAULT_MAX_MESSAGE_SIZE: usize = 1_048_576;

/// Default maximum frame size: 64 KB (65,536 bytes)
pub const DEFAULT_MAX_FRAME_SIZE: usize = 65_536;

/// Create default WebSocketConfig with secure message size limits
///
/// Returns a WebSocketConfig with default settings including:
/// - Maximum message size: 1 MB (prevents memory exhaustion)
/// - Maximum frame size: 64 KB (prevents frame-level abuse)
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::protocol::default_websocket_config;
///
/// let config = default_websocket_config();
/// assert_eq!(config.max_message_size, Some(1_048_576));
/// assert_eq!(config.max_frame_size, Some(65_536));
/// ```
pub fn default_websocket_config() -> TungsteniteConfig {
	let mut config = TungsteniteConfig::default();
	config.max_message_size = Some(DEFAULT_MAX_MESSAGE_SIZE);
	config.max_frame_size = Some(DEFAULT_MAX_FRAME_SIZE);
	config
}

/// Create a WebSocketConfig with custom message size limits
///
/// # Arguments
///
/// * `max_message_size` - Maximum message size in bytes, or `None` for no limit
/// * `max_frame_size` - Maximum frame size in bytes, or `None` for no limit
///
/// # Examples
///
/// ```
/// use reinhardt_websockets::protocol::websocket_config_with_limits;
///
/// // 2 MB messages, 128 KB frames
/// let config = websocket_config_with_limits(Some(2 * 1024 * 1024), Some(128 * 1024));
/// assert_eq!(config.max_message_size, Some(2_097_152));
/// assert_eq!(config.max_frame_size, Some(131_072));
///
/// // No limits (not recommended for production)
/// let unlimited = websocket_config_with_limits(None, None);
/// assert_eq!(unlimited.max_message_size, None);
/// ```
pub fn websocket_config_with_limits(
	max_message_size: Option<usize>,
	max_frame_size: Option<usize>,
) -> TungsteniteConfig {
	let mut config = TungsteniteConfig::default();
	config.max_message_size = max_message_size;
	config.max_frame_size = max_frame_size;
	config
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_default_websocket_config_has_message_size_limit() {
		// Arrange & Act
		let config = default_websocket_config();

		// Assert
		assert_eq!(config.max_message_size, Some(DEFAULT_MAX_MESSAGE_SIZE));
		assert_eq!(config.max_message_size, Some(1_048_576));
	}

	#[rstest]
	fn test_default_websocket_config_has_frame_size_limit() {
		// Arrange & Act
		let config = default_websocket_config();

		// Assert
		assert_eq!(config.max_frame_size, Some(DEFAULT_MAX_FRAME_SIZE));
		assert_eq!(config.max_frame_size, Some(65_536));
	}

	#[rstest]
	fn test_custom_limits() {
		// Arrange
		let max_msg = 2 * 1024 * 1024; // 2 MB
		let max_frame = 128 * 1024; // 128 KB

		// Act
		let config = websocket_config_with_limits(Some(max_msg), Some(max_frame));

		// Assert
		assert_eq!(config.max_message_size, Some(max_msg));
		assert_eq!(config.max_frame_size, Some(max_frame));
	}

	#[rstest]
	fn test_no_limits() {
		// Arrange & Act
		let config = websocket_config_with_limits(None, None);

		// Assert
		assert_eq!(config.max_message_size, None);
		assert_eq!(config.max_frame_size, None);
	}

	#[rstest]
	fn test_default_constants() {
		// Assert
		assert_eq!(DEFAULT_MAX_MESSAGE_SIZE, 1_048_576);
		assert_eq!(DEFAULT_MAX_FRAME_SIZE, 65_536);
	}
}
