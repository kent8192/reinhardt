use thiserror::Error;

/// Errors that can occur in the streaming layer.
#[derive(Debug, Error)]
pub enum StreamingError {
	#[error("connection failed: {0}")]
	Connection(String),
	#[error("serialization error: {0}")]
	Serialization(String),
	#[error("topic not found: {0}")]
	TopicNotFound(String),
	#[error("retryable error: {0}")]
	Retryable(String),
	#[error("fatal error: {0}")]
	Fatal(String),
	#[error("backend error: {0}")]
	Backend(String),
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn retryable_formats_message() {
		let err = StreamingError::Retryable("timeout".to_owned());
		assert_eq!(err.to_string(), "retryable error: timeout");
	}

	#[rstest]
	fn fatal_formats_message() {
		let err = StreamingError::Fatal("disk full".to_owned());
		assert_eq!(err.to_string(), "fatal error: disk full");
	}

	#[rstest]
	fn connection_formats_message() {
		let err = StreamingError::Connection("refused".to_owned());
		assert_eq!(err.to_string(), "connection failed: refused");
	}
}
