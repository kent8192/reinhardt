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

	#[user(hasher = Argon2Hasher, username_field = "email", full = true)]
	#[derive(Debug, Clone, Serialize, Deserialize)]
	pub(crate) struct CustomFieldUser {
		pub id: Uuid,
		pub email: String,
		pub first_name: String,
		pub last_name: String,
		pub is_active: bool,
		pub is_staff: bool,
		pub is_superuser: bool,

		#[user_field(password_hash)]
		pub pwd: Option<String>,

		#[user_field(last_login)]
		pub signed_in: Option<DateTime<Utc>>,

		#[user_field(date_joined)]
		pub created: DateTime<Utc>,
	}

	fn make_custom_field_user() -> CustomFieldUser {
		CustomFieldUser {
			id: Uuid::nil(),
			email: "custom@example.com".to_string(),
			first_name: "Custom".to_string(),
			last_name: "User".to_string(),
			is_active: true,
			is_staff: false,
			is_superuser: false,
			pwd: None,
			signed_in: None,
			created: Utc::now(),
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

	// --- BaseUser edge case tests ---

	#[rstest]
	fn test_base_user_normalize_username() {
		// Arrange / Act
		let normalized = TestUser::normalize_username("Ås\u{0041}\u{030A}@example.com");

		// Assert
		assert!(!normalized.is_empty());
		assert!(normalized.contains("@example.com"));
	}

	#[rstest]
	fn test_base_user_session_auth_hash_changes_with_password() {
		// Arrange
		let mut user = make_test_user();
		user.set_password("password1").unwrap();
		let hash1 = user.get_session_auth_hash("secret-key");

		// Act
		user.set_password("password2").unwrap();
		let hash2 = user.get_session_auth_hash("secret-key");

		// Assert
		assert_ne!(hash1, hash2);
	}

	#[rstest]
	fn test_base_user_session_auth_hash_changes_with_secret() {
		// Arrange
		let mut user = make_test_user();
		user.set_password("same-password").unwrap();

		// Act
		let hash1 = user.get_session_auth_hash("secret-a");
		let hash2 = user.get_session_auth_hash("secret-b");

		// Assert
		assert_ne!(hash1, hash2);
	}

	#[rstest]
	fn test_base_user_password_hash_none_by_default() {
		// Arrange
		let user = make_test_user();

		// Act / Assert
		assert!(user.password_hash().is_none());
		assert!(!user.has_usable_password());
	}

	#[rstest]
	fn test_base_user_check_password_returns_false_without_password() {
		// Arrange
		let user = make_test_user();

		// Act / Assert
		assert!(!user.check_password("anything").unwrap());
	}

	#[rstest]
	fn test_base_user_last_login_none_by_default() {
		// Arrange
		let user = make_test_user();

		// Act / Assert
		assert!(user.last_login().is_none());
	}

	// --- FullUser edge case tests ---

	#[rstest]
	fn test_full_user_empty_names() {
		// Arrange
		let mut user = make_test_user();
		user.first_name = String::new();
		user.last_name = String::new();

		// Act / Assert
		assert_eq!(user.get_full_name(), "");
		assert_eq!(user.get_short_name(), "");
	}

	#[rstest]
	fn test_full_user_first_name_only() {
		// Arrange
		let mut user = make_test_user();
		user.last_name = String::new();

		// Act / Assert
		assert_eq!(user.get_full_name(), "Test");
	}

	#[rstest]
	fn test_full_user_last_name_only() {
		// Arrange
		let mut user = make_test_user();
		user.first_name = String::new();

		// Act / Assert
		assert_eq!(user.get_full_name(), "User");
	}

	#[rstest]
	fn test_full_user_date_joined() {
		// Arrange
		let user = make_test_user();

		// Act / Assert
		assert!(user.date_joined() <= Utc::now());
	}

	// --- PermissionsMixin edge case tests ---

	#[rstest]
	fn test_permissions_has_perms_multiple() {
		// Arrange
		let mut user = make_test_user();
		user.user_permissions = vec!["blog.add_post".to_string(), "blog.edit_post".to_string()];

		// Act / Assert
		assert!(user.has_perms(&["blog.add_post", "blog.edit_post"]));
		assert!(!user.has_perms(&["blog.add_post", "blog.delete_post"]));
	}

	#[rstest]
	fn test_permissions_get_all_permissions() {
		// Arrange
		let mut user = make_test_user();
		user.user_permissions = vec!["blog.add_post".to_string(), "blog.edit_post".to_string()];

		// Act
		let all_perms = user.get_all_permissions();

		// Assert
		assert_eq!(all_perms.len(), 2);
		assert!(all_perms.contains("blog.add_post"));
		assert!(all_perms.contains("blog.edit_post"));
	}

	#[rstest]
	fn test_permissions_empty_permissions() {
		// Arrange
		let user = make_test_user();

		// Act / Assert
		assert!(!user.has_perm("any.permission"));
		assert!(!user.has_module_perms("any"));
		assert!(user.get_all_permissions().is_empty());
	}

	#[rstest]
	fn test_permissions_get_user_permissions() {
		// Arrange
		let mut user = make_test_user();
		user.user_permissions = vec!["app.perm1".to_string()];

		// Act
		let user_perms = user.get_user_permissions();

		// Assert
		assert_eq!(user_perms.len(), 1);
		assert!(user_perms.contains("app.perm1"));
	}

	// --- AuthIdentity edge case tests ---

	#[rstest]
	fn test_auth_identity_admin_reflects_superuser_change() {
		// Arrange
		let mut user = make_test_user();
		assert!(!user.is_admin());

		// Act
		user.is_superuser = true;

		// Assert
		assert!(user.is_admin());
	}

	#[rstest]
	fn test_auth_identity_always_authenticated() {
		// Arrange
		let mut user = make_test_user();
		user.is_active = false;

		// Act / Assert
		// AuthIdentity::is_authenticated() always returns true for concrete types
		assert!(user.is_authenticated());
	}

	// --- Custom field mapping tests ---

	#[rstest]
	fn test_custom_field_password_maps_to_pwd() {
		// Arrange
		let mut user = make_custom_field_user();

		// Act
		user.set_password("test123").unwrap();

		// Assert
		assert!(user.pwd.is_some()); // The actual field is `pwd`
		assert!(user.check_password("test123").unwrap());
	}

	#[rstest]
	fn test_custom_field_last_login_maps_to_signed_in() {
		// Arrange
		let mut user = make_custom_field_user();
		let now = Utc::now();

		// Act
		user.set_last_login(now);

		// Assert
		assert_eq!(user.signed_in, Some(now)); // The actual field is `signed_in`
		assert_eq!(user.last_login(), Some(now));
	}

	#[rstest]
	fn test_custom_field_date_joined_maps_to_created() {
		// Arrange
		let user = make_custom_field_user();

		// Act / Assert
		assert!(FullUser::date_joined(&user) <= Utc::now());
		assert_eq!(FullUser::date_joined(&user), user.created);
	}

	#[rstest]
	fn test_custom_field_username_is_email() {
		// Arrange
		let user = make_custom_field_user();

		// Act / Assert
		assert_eq!(CustomFieldUser::get_username_field(), "email");
		assert_eq!(user.get_username(), "custom@example.com");
		assert_eq!(user.username(), "custom@example.com");
	}

	// --- GroupManager integration tests ---
	// Note: These tests share a single OnceLock-based global GroupManager.
	// The manager is registered once and persists for the process lifetime.

	use std::sync::Once;

	static INIT_GROUP_MANAGER: Once = Once::new();

	fn ensure_group_manager() {
		INIT_GROUP_MANAGER.call_once(|| {
			let rt = tokio::runtime::Runtime::new().unwrap();
			rt.block_on(async {
				use reinhardt_auth::group_management::{CreateGroupData, GroupManager};
				use std::sync::Arc;

				let mut manager = GroupManager::new();
				let group = manager
					.create_group(CreateGroupData {
						name: "editors".to_string(),
						description: None,
					})
					.await
					.unwrap();
				manager
					.add_group_permission(&group.id.to_string(), "blog.add_post")
					.await
					.unwrap();
				manager
					.add_group_permission(&group.id.to_string(), "blog.edit_post")
					.await
					.unwrap();

				reinhardt_auth::register_group_manager(Arc::new(manager));
			});
		});
	}

	#[rstest]
	#[serial_test::serial(global_group_manager)]
	fn test_group_permissions_resolved_via_manager() {
		// Arrange
		ensure_group_manager();
		let mut user = make_test_user();
		user.groups = vec!["editors".to_string()];

		// Act
		let group_perms = user.get_group_permissions();

		// Assert
		assert_eq!(group_perms.len(), 2);
		assert!(group_perms.contains("blog.add_post"));
		assert!(group_perms.contains("blog.edit_post"));

		// has_perm includes group permissions
		assert!(user.has_perm("blog.add_post"));
		assert!(user.has_perm("blog.edit_post"));
		assert!(!user.has_perm("blog.delete_post"));
	}

	#[rstest]
	#[serial_test::serial(global_group_manager)]
	fn test_get_all_permissions_merges_user_and_group() {
		// Arrange
		ensure_group_manager();
		let mut user = make_test_user();
		user.user_permissions = vec!["blog.delete_post".to_string()];
		user.groups = vec!["editors".to_string()];

		// Act
		let all_perms = user.get_all_permissions();

		// Assert — user perm + group perms
		assert!(all_perms.contains("blog.delete_post")); // direct
		assert!(all_perms.contains("blog.add_post")); // from group
		assert!(all_perms.contains("blog.edit_post")); // from group
		assert_eq!(all_perms.len(), 3);
	}

	#[rstest]
	#[serial_test::serial(global_group_manager)]
	fn test_superuser_bypasses_group_check() {
		// Arrange
		ensure_group_manager();
		let mut user = make_test_user();
		user.is_superuser = true;
		user.groups = vec![];

		// Act / Assert — superuser has all permissions regardless
		assert!(user.has_perm("any.permission"));
		assert!(user.has_module_perms("any"));
	}

	#[rstest]
	#[serial_test::serial(global_group_manager)]
	fn test_non_member_group_returns_no_permissions() {
		// Arrange
		ensure_group_manager();
		let mut user = make_test_user();
		user.groups = vec!["nonexistent_group".to_string()];

		// Act
		let group_perms = user.get_group_permissions();

		// Assert — group not in GroupManager, no permissions
		assert!(group_perms.is_empty());
	}
}
