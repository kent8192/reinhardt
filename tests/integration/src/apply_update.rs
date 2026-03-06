//! Integration tests for ApplyUpdate derive macro

#[cfg(test)]
mod tests {
	use reinhardt::ApplyUpdate;
	use reinhardt::apply_update;
	use rstest::rstest;

	struct User {
		pub name: String,
		pub email: String,
		pub age: i32,
		pub display_name: String,
	}

	impl User {
		fn new(name: &str, email: &str, age: i32, display_name: &str) -> Self {
			Self {
				name: name.to_string(),
				email: email.to_string(),
				age,
				display_name: display_name.to_string(),
			}
		}
	}

	#[apply_update(target(User))]
	struct UpdateUserBasic {
		pub name: Option<String>,
		pub email: Option<String>,
		pub age: Option<i32>,
		#[apply_update(skip)]
		pub display_name: Option<String>,
	}

	#[rstest]
	fn test_apply_update_option_some() {
		// Arrange
		let mut user = User::new("Alice", "alice@example.com", 30, "Alice A.");
		let update = UpdateUserBasic {
			name: Some("Bob".to_string()),
			email: None,
			age: Some(25),
			display_name: Some("Should be skipped".to_string()),
		};

		// Act
		update.apply_to(&mut user);

		// Assert
		assert_eq!(user.name, "Bob");
		assert_eq!(user.email, "alice@example.com");
		assert_eq!(user.age, 25);
		assert_eq!(user.display_name, "Alice A.");
	}

	#[rstest]
	fn test_apply_update_all_none() {
		// Arrange
		let mut user = User::new("Alice", "alice@example.com", 30, "Alice A.");
		let update = UpdateUserBasic {
			name: None,
			email: None,
			age: None,
			display_name: None,
		};

		// Act
		update.apply_to(&mut user);

		// Assert
		assert_eq!(user.name, "Alice");
		assert_eq!(user.email, "alice@example.com");
		assert_eq!(user.age, 30);
	}

	#[apply_update(target(User))]
	struct UpdateWithRename {
		#[apply_update(rename = "display_name")]
		pub new_name: Option<String>,
	}

	#[rstest]
	fn test_apply_update_rename() {
		// Arrange
		let mut user = User::new("Alice", "alice@example.com", 30, "Alice A.");
		let update = UpdateWithRename {
			new_name: Some("New Display".to_string()),
		};

		// Act
		update.apply_to(&mut user);

		// Assert
		assert_eq!(user.display_name, "New Display");
	}

	#[apply_update(target(User))]
	struct UpdateWithNonOption {
		pub name: String,
		pub age: i32,
	}

	#[rstest]
	fn test_apply_update_non_option_always_applies() {
		// Arrange
		let mut user = User::new("Alice", "alice@example.com", 30, "Alice A.");
		let update = UpdateWithNonOption {
			name: "Charlie".to_string(),
			age: 40,
		};

		// Act
		update.apply_to(&mut user);

		// Assert
		assert_eq!(user.name, "Charlie");
		assert_eq!(user.age, 40);
	}

	// Test multiple targets
	struct AdminUser {
		pub name: String,
		pub email: String,
	}

	impl AdminUser {
		fn new(name: &str, email: &str) -> Self {
			Self {
				name: name.to_string(),
				email: email.to_string(),
			}
		}
	}

	#[apply_update(target(User, AdminUser))]
	struct UpdateShared {
		pub name: Option<String>,
		pub email: Option<String>,
	}

	#[rstest]
	fn test_apply_update_multiple_targets() {
		// Arrange
		let mut user = User::new("Alice", "alice@example.com", 30, "Alice A.");
		let mut admin = AdminUser::new("Admin", "admin@example.com");

		let update_user = UpdateShared {
			name: Some("Bob".to_string()),
			email: None,
		};
		let update_admin = UpdateShared {
			name: None,
			email: Some("new@example.com".to_string()),
		};

		// Act
		update_user.apply_to(&mut user);
		update_admin.apply_to(&mut admin);

		// Assert
		assert_eq!(user.name, "Bob");
		assert_eq!(user.email, "alice@example.com");
		assert_eq!(admin.name, "Admin");
		assert_eq!(admin.email, "new@example.com");
	}
}
