//! Integration tests for `#[user]` macro with `#[field(skip)]` and
//! `PermissionsMixin` including group permission resolution.
//!
//! Note: Full `#[user]` + `#[model]` compilation tests are in the trybuild
//! pass tests (`tests/ui/user/pass/`). Runtime integration with the `Model`
//! trait requires the full registration infrastructure (linkme, reinhardt-apps)
//! which is available in the `reinhardt-integration-tests` crate.

#[cfg(feature = "argon2-hasher")]
mod tests {
	use chrono::{DateTime, Utc};
	use reinhardt_auth::Argon2Hasher;
	use reinhardt_auth::{AuthIdentity, BaseUser, FullUser, PermissionsMixin};
	use reinhardt_macros::user;
	use rstest::rstest;
	use serde::{Deserialize, Serialize};
	use uuid::Uuid;

	// A full user struct with all convention fields including permissions.
	// Tests that #[user] correctly generates all trait impls and that
	// Vec<String> fields work with PermissionsMixin.
	#[user(hasher = Argon2Hasher, username_field = "username", full = true)]
	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub struct FullTestUser {
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

	fn make_full_test_user() -> FullTestUser {
		FullTestUser {
			id: Uuid::now_v7(),
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

	// --- Compilation verification ---

	#[rstest]
	fn test_user_model_struct_compiles() {
		// Arrange / Act — the struct definition itself is the test
		let user = make_full_test_user();

		// Assert — basic field access works
		assert_eq!(user.get_username(), "testuser");
		assert!(user.is_active());
	}

	// --- BaseUser trait ---

	#[rstest]
	fn test_model_user_base_user_password() {
		// Arrange
		let mut user = make_full_test_user();

		// Act
		user.set_password("secret123").unwrap();

		// Assert
		assert!(user.password_hash().is_some());
		assert!(user.check_password("secret123").unwrap());
		assert!(!user.check_password("wrong").unwrap());
	}

	#[rstest]
	fn test_model_user_base_user_username_field() {
		// Act / Assert
		assert_eq!(FullTestUser::get_username_field(), "username");
	}

	#[rstest]
	fn test_model_user_base_user_last_login() {
		// Arrange
		let mut user = make_full_test_user();
		let now = Utc::now();

		// Act
		user.set_last_login(now);

		// Assert
		assert_eq!(user.last_login(), Some(now));
	}

	// --- FullUser trait ---

	#[rstest]
	fn test_model_user_full_user_names() {
		// Arrange
		let user = make_full_test_user();

		// Act / Assert
		assert_eq!(user.get_full_name(), "Test User");
		assert_eq!(user.get_short_name(), "Test");
		assert_eq!(user.email(), "test@example.com");
	}

	#[rstest]
	fn test_model_user_full_user_staff() {
		// Arrange
		let user = make_full_test_user();

		// Act / Assert
		assert!(!user.is_staff());
		assert!(!FullUser::is_superuser(&user));
	}

	// --- PermissionsMixin trait ---

	#[rstest]
	fn test_model_user_permissions_direct() {
		// Arrange
		let mut user = make_full_test_user();
		user.user_permissions = vec!["blog.add_post".to_string(), "blog.edit_post".to_string()];

		// Act / Assert
		assert!(user.has_perm("blog.add_post"));
		assert!(user.has_perm("blog.edit_post"));
		assert!(!user.has_perm("blog.delete_post"));
	}

	#[rstest]
	fn test_model_user_permissions_superuser() {
		// Arrange
		let mut user = make_full_test_user();
		user.is_superuser = true;

		// Act / Assert
		assert!(user.has_perm("any.permission"));
		assert!(user.has_module_perms("any"));
	}

	#[rstest]
	fn test_model_user_has_perms_multiple() {
		// Arrange
		let mut user = make_full_test_user();
		user.user_permissions = vec!["a.one".to_string(), "a.two".to_string()];

		// Act / Assert
		assert!(user.has_perms(&["a.one", "a.two"]));
		assert!(!user.has_perms(&["a.one", "a.three"]));
	}

	#[rstest]
	fn test_model_user_has_module_perms() {
		// Arrange
		let mut user = make_full_test_user();
		user.user_permissions = vec!["blog.add_post".to_string()];

		// Act / Assert
		assert!(user.has_module_perms("blog"));
		assert!(!user.has_module_perms("admin"));
	}

	#[rstest]
	fn test_model_user_empty_permissions() {
		// Arrange
		let user = make_full_test_user();

		// Act / Assert
		assert!(!user.has_perm("any.perm"));
		assert!(user.get_all_permissions().is_empty());
	}

	#[rstest]
	fn test_model_user_groups_field() {
		// Arrange
		let mut user = make_full_test_user();
		user.groups = vec!["editors".to_string(), "moderators".to_string()];

		// Act / Assert
		assert_eq!(user.groups().len(), 2);
		assert_eq!(user.groups()[0], "editors");
	}

	// --- AuthIdentity trait ---

	#[rstest]
	fn test_model_user_auth_identity() {
		// Arrange
		let user = make_full_test_user();

		// Act / Assert
		assert!(!user.id().is_empty());
		assert!(user.is_authenticated());
		assert!(!user.is_admin());
	}

	#[rstest]
	fn test_model_user_admin_reflects_superuser() {
		// Arrange
		let mut user = make_full_test_user();

		// Act
		user.is_superuser = true;

		// Assert
		assert!(user.is_admin());
	}

	// --- Vec<String> field access ---

	#[rstest]
	fn test_vec_string_permissions_field_access() {
		// Arrange
		let mut user = make_full_test_user();
		user.user_permissions = vec![
			"blog.add_post".to_string(),
			"blog.edit_post".to_string(),
			"blog.delete_post".to_string(),
		];

		// Act
		let perms = user.user_permissions();
		let all = user.get_all_permissions();

		// Assert
		assert_eq!(perms.len(), 3);
		assert_eq!(all.len(), 3);
		assert!(all.contains("blog.add_post"));
		assert!(all.contains("blog.edit_post"));
		assert!(all.contains("blog.delete_post"));
	}

	#[rstest]
	fn test_vec_string_groups_field_access() {
		// Arrange
		let mut user = make_full_test_user();
		user.groups = vec!["editors".to_string(), "moderators".to_string()];

		// Act
		let groups = user.groups();

		// Assert
		assert_eq!(groups.len(), 2);
		assert_eq!(groups[0], "editors");
		assert_eq!(groups[1], "moderators");
	}
}
