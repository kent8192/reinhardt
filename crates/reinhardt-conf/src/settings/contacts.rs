//! Contact settings fragment
//!
//! Administrator and manager contact information for notifications.

use super::Contact;
use reinhardt_core::macros::settings;
use serde::{Deserialize, Serialize};

/// Administrator and manager contact information.
///
/// Used for error notifications and broken link notifications.
#[settings(fragment = true, section = "contacts")]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ContactSettings {
	/// List of administrator contacts.
	#[serde(default)]
	pub admins: Vec<Contact>,
	/// List of manager contacts.
	#[serde(default)]
	pub managers: Vec<Contact>,
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::settings::Contact;
	use crate::settings::fragment::SettingsFragment;
	use rstest::rstest;

	#[rstest]
	fn test_contact_settings_section() {
		// Arrange / Act / Assert
		assert_eq!(ContactSettings::section(), "contacts");
	}

	#[rstest]
	fn test_contact_settings_default() {
		// Arrange / Act
		let settings = ContactSettings::default();

		// Assert
		assert!(settings.admins.is_empty());
		assert!(settings.managers.is_empty());
	}

	#[rstest]
	fn test_contact_settings_with_contacts() {
		// Arrange
		let settings = ContactSettings {
			admins: vec![Contact::new("Admin", "admin@example.com")],
			managers: vec![],
		};

		// Act / Assert
		assert_eq!(settings.admins.len(), 1);
		assert_eq!(settings.admins[0].name, "Admin");
	}
}
