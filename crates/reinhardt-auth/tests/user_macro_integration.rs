//! Integration tests for the #[user] macro generated trait implementations.

#[cfg(feature = "argon2-hasher")]
mod tests {
	use chrono::{DateTime, Utc};
	use reinhardt_auth::Argon2Hasher;
	use reinhardt_auth::{AuthIdentity, BaseUser, FullUser, PermissionsMixin};
	use reinhardt_macros::user;
	use rstest::rstest;
	use serde::{Deserialize, Serialize};
	use uuid::Uuid;

	#[user(hasher = Argon2Hasher, username_field = "username", full = true)]
	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub(crate) struct TestUser {
		pub id: Uuid,
		pub username: String,
		pub email: String,
		pub first_name: String,
		pub last_name: String,
		pub password_hash: Option<String>,
		pub last_login: Option<DateTime<Utc>>,
		pub is_active: bool,
		pub is_staff: bool,
		pub is_superuser: bool,
		pub date_joined: DateTime<Utc>,
		pub user_permissions: Vec<String>,
		pub groups: Vec<String>,
	}

	fn make_test_user() -> TestUser {
		TestUser {
			id: Uuid::nil(),
			username: "testuser".to_string(),
			email: "test@example.com".to_string(),
			first_name: "Test".to_string(),
			last_name: "User".to_string(),
			password_hash: None,
			last_login: None,
			is_active: true,
			is_staff: false,
			is_superuser: false,
			date_joined: Utc::now(),
			user_permissions: Vec::new(),
			groups: Vec::new(),
		}
	}

	// BaseUser tests

	#[rstest]
	fn test_base_user_set_and_check_password() {
		// Arrange
		let mut user = make_test_user();

		// Act
		user.set_password("secure_password").unwrap();

		// Assert
		assert!(user.check_password("secure_password").unwrap());
		assert!(!user.check_password("wrong_password").unwrap());
	}

	#[rstest]
	fn test_base_user_unusable_password() {
		// Arrange
		let mut user = make_test_user();

		// Act
		user.set_unusable_password();

		// Assert
		assert!(!user.has_usable_password());
		assert!(!user.check_password("anything").unwrap());
	}

	#[rstest]
	fn test_base_user_username_field() {
		// Arrange / Act / Assert
		assert_eq!(TestUser::get_username_field(), "username");
	}

	#[rstest]
	fn test_base_user_get_username() {
		// Arrange
		let user = make_test_user();

		// Act / Assert
		assert_eq!(user.get_username(), "testuser");
	}

	#[rstest]
	fn test_base_user_last_login() {
		// Arrange
		let mut user = make_test_user();
		let now = Utc::now();

		// Act
		user.set_last_login(now);

		// Assert
		assert_eq!(user.last_login(), Some(now));
	}

	#[rstest]
	fn test_base_user_is_active() {
		// Arrange
		let user = make_test_user();

		// Act / Assert
		assert!(user.is_active());
	}

	// FullUser tests

	#[rstest]
	fn test_full_user_get_full_name() {
		// Arrange
		let user = make_test_user();

		// Act / Assert
		assert_eq!(user.get_full_name(), "Test User");
	}

	#[rstest]
	fn test_full_user_get_short_name() {
		// Arrange
		let user = make_test_user();

		// Act / Assert
		assert_eq!(user.get_short_name(), "Test");
	}

	#[rstest]
	fn test_full_user_accessors() {
		// Arrange
		let user = make_test_user();

		// Act / Assert
		assert_eq!(user.username(), "testuser");
		assert_eq!(user.email(), "test@example.com");
		assert!(!user.is_staff());
		assert!(!FullUser::is_superuser(&user));
	}

	// PermissionsMixin tests

	#[rstest]
	fn test_permissions_has_perm() {
		// Arrange
		let mut user = make_test_user();
		user.user_permissions = vec!["blog.add_post".to_string()];

		// Act / Assert
		assert!(user.has_perm("blog.add_post"));
		assert!(!user.has_perm("blog.delete_post"));
	}

	#[rstest]
	fn test_permissions_superuser_has_all_perms() {
		// Arrange
		let mut user = make_test_user();
		user.is_superuser = true;

		// Act / Assert
		assert!(user.has_perm("any.permission"));
		assert!(user.has_module_perms("any"));
	}

	#[rstest]
	fn test_permissions_has_module_perms() {
		// Arrange
		let mut user = make_test_user();
		user.user_permissions = vec!["blog.add_post".to_string()];

		// Act / Assert
		assert!(user.has_module_perms("blog"));
		assert!(!user.has_module_perms("admin"));
	}

	// AuthIdentity tests

	#[rstest]
	fn test_auth_identity_id() {
		// Arrange
		let user = make_test_user();

		// Act / Assert
		assert_eq!(user.id(), Uuid::nil().to_string());
	}

	#[rstest]
	fn test_auth_identity_is_authenticated() {
		// Arrange
		let user = make_test_user();

		// Act / Assert
		assert!(user.is_authenticated());
	}

	#[rstest]
	fn test_auth_identity_is_admin() {
		// Arrange
		let mut user = make_test_user();

		// Act / Assert
		assert!(!user.is_admin());

		user.is_superuser = true;
		assert!(user.is_admin());
	}
}
