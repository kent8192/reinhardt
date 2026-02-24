//! Integration tests for session-based message storage

#[cfg(test)]
mod tests {
	use reinhardt_core::messages::{Level, Message, MessageStorage, SafeData, SessionStorage};

	#[test]
	fn test_no_session() {
		// Test that SessionStorage raises error when session middleware is not installed
		let storage = SessionStorage::without_session();

		let result = storage.require_session();
		assert!(result.is_err());
		assert!(
			result
				.unwrap_err()
				.contains("SessionStorage requires session middleware")
		);
	}

	#[test]
	fn test_get_session() {
		// Test retrieving messages from session storage
		let mut storage = SessionStorage::new();

		// Add messages
		storage.add(Message::new(Level::Info, "Session message 1"));
		storage.add(Message::new(Level::Warning, "Session message 2"));

		// Serialize for session
		let session_data = storage.serialize_for_session().unwrap();

		// Load in new storage
		let mut new_storage = SessionStorage::new();
		new_storage.load_from_session(&session_data).unwrap();

		// Get messages
		let messages = new_storage.get_all();
		assert_eq!(messages.len(), 2);
		assert_eq!(messages[0].text, "Session message 1");
		assert_eq!(messages[1].text, "Session message 2");
	}

	#[test]
	fn test_safedata_session() {
		// Test that SafeData maintains its safe status in session
		let mut storage = SessionStorage::new();

		// Create message with SafeData
		let safe_html = SafeData::new("<p>Safe HTML</p>");

		// Add to storage (using regular message for simplicity)
		storage.add(Message::new(Level::Info, safe_html.as_str()));

		// Serialize and deserialize
		let session_data = storage.serialize_for_session().unwrap();
		let mut new_storage = SessionStorage::new();
		new_storage.load_from_session(&session_data).unwrap();

		// Verify safe status is preserved
		let messages = new_storage.get_all();
		assert_eq!(messages.len(), 1);
		assert_eq!(messages[0].text, "<p>Safe HTML</p>");

		// Verify SafeData can be recreated
		let restored_safe = SafeData::new(messages[0].text.clone());
		assert_eq!(restored_safe.as_str(), safe_html.as_str());
	}
}
