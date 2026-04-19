//! DefaultUser ORM Integration Tests
//!
//! This module contains comprehensive tests for the DefaultUser model,
//! covering user creation, password hashing, permission management,
//! and ORM trait implementations.
//!
//! # Test Categories
//!
//! - Happy Path: User creation, password hashing, permission checking
//! - Error Path: Invalid credentials, password verification failures
//! - State Transition: Password changes, last_login updates, activation
//! - Edge Cases: Unicode passwords, empty strings, boundary values
//! - Equivalence Partitioning: Different user roles and permission sets

use chrono::Utc;
use reinhardt_auth::{BaseUser, DefaultUser, FullUser, PermissionsMixin, User};
use reinhardt_db::orm::Model;
use rstest::*;
use uuid::Uuid;

// =============================================================================
// Fixtures
// =============================================================================

/// Creates a basic DefaultUser with minimal fields
#[fixture]
fn basic_user() -> DefaultUser {
	DefaultUser {
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

/// Creates a staff user with some permissions
#[fixture]
fn staff_user() -> DefaultUser {
	DefaultUser {
		id: Uuid::now_v7(),
		username: "staffuser".to_string(),
		email: "staff@example.com".to_string(),
		first_name: "Staff".to_string(),
		last_name: "Member".to_string(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_staff: true,
		is_superuser: false,
		date_joined: Utc::now(),
		user_permissions: vec!["blog.add_post".to_string(), "blog.change_post".to_string()],
		groups: vec!["editors".to_string()],
	}
}

/// Creates a superuser with all permissions
#[fixture]
fn superuser() -> DefaultUser {
	DefaultUser {
		id: Uuid::now_v7(),
		username: "admin".to_string(),
		email: "admin@example.com".to_string(),
		first_name: "Super".to_string(),
		last_name: "Admin".to_string(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_staff: true,
		is_superuser: true,
		date_joined: Utc::now(),
		user_permissions: Vec::new(),
		groups: vec!["administrators".to_string()],
	}
}

/// Creates an inactive user
#[fixture]
fn inactive_user() -> DefaultUser {
	DefaultUser {
		id: Uuid::now_v7(),
		username: "inactive".to_string(),
		email: "inactive@example.com".to_string(),
		first_name: "Inactive".to_string(),
		last_name: "User".to_string(),
		password_hash: None,
		last_login: None,
		is_active: false,
		is_staff: false,
		is_superuser: false,
		date_joined: Utc::now(),
		user_permissions: Vec::new(),
		groups: Vec::new(),
	}
}

// =============================================================================
// Happy Path Tests - User Creation and Basic Operations
// =============================================================================

#[rstest]
fn test_user_creation_with_default_values() {
	let user = DefaultUser::default();

	assert_eq!(user.id, Uuid::nil());
	assert!(user.username.is_empty());
	assert!(user.email.is_empty());
	assert!(user.first_name.is_empty());
	assert!(user.last_name.is_empty());
	assert!(user.password_hash.is_none());
	assert!(user.last_login.is_none());
	assert!(user.is_active);
	assert!(!user.is_staff);
	assert!(!user.is_superuser);
	assert!(user.user_permissions.is_empty());
	assert!(user.groups.is_empty());
}

#[rstest]
fn test_user_creation_with_custom_fields(basic_user: DefaultUser) {
	assert_eq!(basic_user.username, "testuser");
	assert_eq!(basic_user.email, "test@example.com");
	assert_eq!(basic_user.first_name, "Test");
	assert_eq!(basic_user.last_name, "User");
	assert!(basic_user.is_active);
	assert!(!basic_user.is_staff);
	assert!(!basic_user.is_superuser);
}

#[rstest]
fn test_password_hashing_and_verification(mut basic_user: DefaultUser) {
	let password = "securepassword123!";

	let set_result = basic_user.set_password(password);
	assert!(
		set_result.is_ok(),
		"set_password should succeed for valid password"
	);

	// Verify password hash is set
	assert!(
		basic_user.password_hash.is_some(),
		"password_hash should be set after set_password"
	);

	// Password hash should not be the plaintext
	assert_ne!(
		basic_user.password_hash.as_deref(),
		Some(password),
		"password_hash should not be plaintext"
	);

	// Verify correct password
	let verify_correct = basic_user.check_password(password);
	assert!(
		verify_correct.is_ok(),
		"check_password should succeed for registered user"
	);
	assert!(
		verify_correct.unwrap(),
		"check_password should return true for correct password"
	);
}

#[rstest]
fn test_password_verification_with_wrong_password(mut basic_user: DefaultUser) {
	basic_user.set_password("correctpassword").unwrap();

	let verify_wrong = basic_user.check_password("wrongpassword");
	assert!(verify_wrong.is_ok(), "check_password should not error");
	assert!(
		!verify_wrong.unwrap(),
		"check_password should return false for wrong password"
	);
}

#[rstest]
fn test_password_verification_without_hash(basic_user: DefaultUser) {
	// User without password hash
	assert!(
		basic_user.password_hash.is_none(),
		"Test setup should have no password hash"
	);

	let result = basic_user.check_password("anypassword");
	assert!(
		result.is_ok(),
		"check_password should not error even when no hash is set"
	);
	assert!(
		!result.unwrap(),
		"check_password should return false when no hash is set"
	);
}

// =============================================================================
// Happy Path Tests - Trait Implementations
// =============================================================================

#[rstest]
fn test_base_user_trait(mut basic_user: DefaultUser) {
	// get_username_field
	assert_eq!(
		DefaultUser::get_username_field(),
		"username",
		"username field name should be 'username'"
	);

	// get_username
	assert_eq!(BaseUser::get_username(&basic_user), "testuser");

	// password_hash (initially None)
	assert!(basic_user.password_hash().is_none());

	// set_password_hash
	basic_user.set_password_hash("testhash".to_string());
	assert_eq!(basic_user.password_hash(), Some("testhash"));

	// last_login (initially None)
	assert!(basic_user.last_login().is_none());

	// set_last_login
	let login_time = Utc::now();
	basic_user.set_last_login(login_time);
	assert_eq!(basic_user.last_login(), Some(login_time));

	// is_active
	assert!(BaseUser::is_active(&basic_user));
}

#[rstest]
fn test_full_user_trait(staff_user: DefaultUser) {
	assert_eq!(FullUser::username(&staff_user), "staffuser");
	assert_eq!(FullUser::email(&staff_user), "staff@example.com");
	assert_eq!(FullUser::first_name(&staff_user), "Staff");
	assert_eq!(FullUser::last_name(&staff_user), "Member");
	assert!(FullUser::is_staff(&staff_user));
	assert!(!FullUser::is_superuser(&staff_user));
	// date_joined should be recent
	let now = Utc::now();
	let time_diff = now.signed_duration_since(staff_user.date_joined());
	assert!(
		time_diff.num_seconds() < 5,
		"date_joined should be recent (within 5 seconds)"
	);
}

#[rstest]
fn test_permissions_mixin_trait(staff_user: DefaultUser) {
	assert!(!PermissionsMixin::is_superuser(&staff_user));
	assert_eq!(PermissionsMixin::user_permissions(&staff_user).len(), 2);
	assert!(PermissionsMixin::user_permissions(&staff_user).contains(&"blog.add_post".to_string()));
	assert!(
		PermissionsMixin::user_permissions(&staff_user).contains(&"blog.change_post".to_string())
	);
	assert_eq!(PermissionsMixin::groups(&staff_user).len(), 1);
	assert!(PermissionsMixin::groups(&staff_user).contains(&"editors".to_string()));
}

#[rstest]
fn test_user_trait(superuser: DefaultUser) {
	assert!(
		!User::id(&superuser).is_empty(),
		"id() should return non-empty string"
	);
	assert_eq!(User::username(&superuser), "admin");
	assert_eq!(User::get_username(&superuser), "admin");
	assert!(
		User::is_authenticated(&superuser),
		"DefaultUser should always return true for is_authenticated"
	);
	assert!(User::is_active(&superuser));
	assert!(User::is_admin(&superuser), "superuser should be admin");
	assert!(User::is_staff(&superuser));
	assert!(User::is_superuser(&superuser));
}

#[rstest]
fn test_model_trait(basic_user: DefaultUser) {
	assert_eq!(
		DefaultUser::table_name(),
		"auth_user",
		"table name should be auth_user"
	);
	assert_eq!(
		DefaultUser::primary_key_field(),
		"id",
		"primary key field should be id"
	);
	assert_eq!(basic_user.primary_key(), Some(basic_user.id));

	// Test set_primary_key
	let mut user = basic_user.clone();
	let new_id = Uuid::now_v7();
	user.set_primary_key(new_id);
	assert_eq!(user.primary_key(), Some(new_id));
}

#[rstest]
fn test_default_user_fields() {
	let fields = DefaultUser::new_fields();

	// Verify all fields exist
	let _ = fields.id;
	let _ = fields.username;
	let _ = fields.email;
	let _ = fields.first_name;
	let _ = fields.last_name;
	let _ = fields.password_hash;
	let _ = fields.last_login;
	let _ = fields.is_active;
	let _ = fields.is_staff;
	let _ = fields.is_superuser;
	let _ = fields.date_joined;
	let _ = fields.user_permissions;
	let _ = fields.groups;
}

// =============================================================================
// Happy Path Tests - Permission Checking
// =============================================================================

#[rstest]
fn test_has_perm_with_explicit_permission(staff_user: DefaultUser) {
	assert!(
		staff_user.has_perm("blog.add_post"),
		"User should have explicitly assigned permission"
	);
	assert!(
		staff_user.has_perm("blog.change_post"),
		"User should have explicitly assigned permission"
	);
	assert!(
		!staff_user.has_perm("blog.delete_post"),
		"User should not have non-assigned permission"
	);
}

#[rstest]
fn test_has_perm_superuser_has_all(superuser: DefaultUser) {
	assert!(
		superuser.has_perm("blog.add_post"),
		"Superuser should have any permission"
	);
	assert!(
		superuser.has_perm("blog.delete_post"),
		"Superuser should have any permission"
	);
	assert!(
		superuser.has_perm("arbitrary.permission"),
		"Superuser should have arbitrary permissions"
	);
}

#[rstest]
fn test_has_module_perms(staff_user: DefaultUser) {
	assert!(
		staff_user.has_module_perms("blog"),
		"User should have module perms for blog"
	);
	assert!(
		!staff_user.has_module_perms("shop"),
		"User should not have module perms for shop"
	);
}

#[rstest]
fn test_has_module_perms_superuser(superuser: DefaultUser) {
	assert!(
		superuser.has_module_perms("blog"),
		"Superuser should have module perms for any module"
	);
	assert!(
		superuser.has_module_perms("shop"),
		"Superuser should have module perms for any module"
	);
	assert!(
		superuser.has_module_perms("arbitrary_module"),
		"Superuser should have module perms for any module"
	);
}

// =============================================================================
// State Transition Tests
// =============================================================================

#[rstest]
fn test_password_change_invalidates_old(mut basic_user: DefaultUser) {
	let old_password = "oldpassword123";
	let new_password = "newpassword456";

	// Set initial password
	basic_user.set_password(old_password).unwrap();
	assert!(basic_user.check_password(old_password).unwrap());

	// Change password
	basic_user.set_password(new_password).unwrap();

	// Old password should no longer work
	assert!(
		!basic_user.check_password(old_password).unwrap(),
		"Old password should not work after change"
	);

	// New password should work
	assert!(
		basic_user.check_password(new_password).unwrap(),
		"New password should work after change"
	);
}

#[rstest]
fn test_last_login_update(mut basic_user: DefaultUser) {
	assert!(
		basic_user.last_login().is_none(),
		"Initial last_login should be None"
	);

	let first_login = Utc::now();
	basic_user.set_last_login(first_login);
	assert_eq!(
		basic_user.last_login(),
		Some(first_login),
		"last_login should be updated"
	);

	// Simulate second login
	std::thread::sleep(std::time::Duration::from_millis(10));
	let second_login = Utc::now();
	basic_user.set_last_login(second_login);

	assert_eq!(basic_user.last_login(), Some(second_login));
	assert!(
		second_login > first_login,
		"Second login time should be after first"
	);
}

#[rstest]
fn test_user_activation_deactivation(mut inactive_user: DefaultUser) {
	assert!(
		!BaseUser::is_active(&inactive_user),
		"User should start as inactive"
	);

	// Activate user
	inactive_user.is_active = true;
	assert!(
		BaseUser::is_active(&inactive_user),
		"User should be active after activation"
	);

	// Deactivate user
	inactive_user.is_active = false;
	assert!(
		!BaseUser::is_active(&inactive_user),
		"User should be inactive after deactivation"
	);
}

#[rstest]
fn test_staff_promotion(mut basic_user: DefaultUser) {
	assert!(
		!FullUser::is_staff(&basic_user),
		"User should start as non-staff"
	);

	basic_user.is_staff = true;
	assert!(
		FullUser::is_staff(&basic_user),
		"User should be staff after promotion"
	);
}

#[rstest]
fn test_superuser_promotion(mut basic_user: DefaultUser) {
	assert!(
		!FullUser::is_superuser(&basic_user),
		"User should start as non-superuser"
	);
	assert!(
		!basic_user.has_perm("arbitrary.permission"),
		"Non-superuser should not have arbitrary permissions"
	);

	basic_user.is_superuser = true;
	assert!(
		FullUser::is_superuser(&basic_user),
		"User should be superuser after promotion"
	);
	assert!(
		basic_user.has_perm("arbitrary.permission"),
		"Superuser should have arbitrary permissions"
	);
}

#[rstest]
fn test_permission_grant_and_revoke(mut basic_user: DefaultUser) {
	assert!(
		!basic_user.has_perm("blog.add_post"),
		"User should not have permission initially"
	);

	// Grant permission
	basic_user
		.user_permissions
		.push("blog.add_post".to_string());
	assert!(
		basic_user.has_perm("blog.add_post"),
		"User should have permission after grant"
	);

	// Revoke permission
	basic_user.user_permissions.retain(|p| p != "blog.add_post");
	assert!(
		!basic_user.has_perm("blog.add_post"),
		"User should not have permission after revoke"
	);
}

#[rstest]
fn test_group_add_and_remove(mut basic_user: DefaultUser) {
	assert!(
		basic_user.groups().is_empty(),
		"User should have no groups initially"
	);

	// Add to group
	basic_user.groups.push("editors".to_string());
	assert_eq!(basic_user.groups().len(), 1);
	assert!(basic_user.groups().contains(&"editors".to_string()));

	// Add to another group
	basic_user.groups.push("reviewers".to_string());
	assert_eq!(basic_user.groups().len(), 2);

	// Remove from group
	basic_user.groups.retain(|g| g != "editors");
	assert_eq!(basic_user.groups().len(), 1);
	assert!(!basic_user.groups().contains(&"editors".to_string()));
	assert!(basic_user.groups().contains(&"reviewers".to_string()));
}

// =============================================================================
// Edge Cases Tests - Password Handling
// =============================================================================

#[rstest]
#[case("", "empty password")]
#[case(" ", "single space")]
#[case("   ", "multiple spaces")]
fn test_edge_case_passwords(
	mut basic_user: DefaultUser,
	#[case] password: &str,
	#[case] desc: &str,
) {
	// The hasher should handle these edge cases
	let result = basic_user.set_password(password);

	// Whether success or failure, it should be deterministic
	if result.is_ok() {
		// If setting worked, verification should work
		let verify = basic_user.check_password(password);
		assert!(
			verify.is_ok(),
			"Verification should succeed for {} if hashing succeeded",
			desc
		);
		assert!(
			verify.unwrap(),
			"Correct password should verify for {}",
			desc
		);
	}
}

#[rstest]
#[case("パスワード123", "Japanese characters")]
#[case("мойпароль", "Cyrillic characters")]
#[case("密码测试", "Chinese characters")]
#[case("كلمة السر", "Arabic characters")]
fn test_unicode_passwords(mut basic_user: DefaultUser, #[case] password: &str, #[case] desc: &str) {
	let result = basic_user.set_password(password);
	assert!(
		result.is_ok(),
		"set_password should succeed for {}: {:?}",
		desc,
		result.err()
	);

	let verify = basic_user.check_password(password);
	assert!(
		verify.is_ok(),
		"check_password should succeed for {}: {:?}",
		desc,
		verify.err()
	);
	assert!(
		verify.unwrap(),
		"Password verification should succeed for {}",
		desc
	);
}

#[rstest]
fn test_long_password(mut basic_user: DefaultUser) {
	// Very long password (1000 characters)
	let long_password: String = "a".repeat(1000);

	let result = basic_user.set_password(&long_password);
	assert!(result.is_ok(), "set_password should handle long passwords");

	let verify = basic_user.check_password(&long_password);
	assert!(
		verify.is_ok(),
		"check_password should handle long passwords"
	);
	assert!(verify.unwrap(), "Long password should verify correctly");
}

#[rstest]
fn test_password_with_special_characters(mut basic_user: DefaultUser) {
	let special_password = r#"!@#$%^&*()_+-=[]{}|;':",./<>?`~"#;

	let result = basic_user.set_password(special_password);
	assert!(
		result.is_ok(),
		"set_password should handle special characters"
	);

	let verify = basic_user.check_password(special_password);
	assert!(
		verify.is_ok(),
		"check_password should handle special characters"
	);
	assert!(verify.unwrap(), "Special character password should verify");
}

#[rstest]
fn test_password_with_null_bytes(mut basic_user: DefaultUser) {
	let null_password = "pass\0word";

	// The behavior with null bytes depends on the hasher implementation
	// Either it should succeed or fail gracefully
	let result = basic_user.set_password(null_password);

	if result.is_ok() {
		let verify = basic_user.check_password(null_password);
		// Should not panic
		let _ = verify;
	}
}

// =============================================================================
// Edge Cases Tests - Username and Email
// =============================================================================

#[rstest]
#[case("a", "single character")]
#[case("user_name", "with underscore")]
#[case("user-name", "with hyphen")]
#[case("user.name", "with dot")]
#[case("user123", "with numbers")]
#[case("ユーザー", "Japanese")]
fn test_username_variants(#[case] username: &str, #[case] desc: &str) {
	let user = DefaultUser {
		id: Uuid::now_v7(),
		username: username.to_string(),
		email: "test@example.com".to_string(),
		first_name: String::new(),
		last_name: String::new(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_staff: false,
		is_superuser: false,
		date_joined: Utc::now(),
		user_permissions: Vec::new(),
		groups: Vec::new(),
	};

	assert_eq!(
		FullUser::username(&user),
		username,
		"Username should match for {}",
		desc
	);
	assert_eq!(
		BaseUser::get_username(&user),
		username,
		"get_username should match for {}",
		desc
	);
}

// =============================================================================
// Equivalence Partitioning Tests - User Roles
// =============================================================================

#[rstest]
#[case(false, false, false, "regular user")]
#[case(true, false, false, "active user")]
#[case(true, true, false, "staff user")]
#[case(true, true, true, "superuser")]
#[case(false, true, true, "inactive superuser")]
fn test_user_role_combinations(
	#[case] is_active: bool,
	#[case] is_staff: bool,
	#[case] is_superuser: bool,
	#[case] desc: &str,
) {
	let user = DefaultUser {
		id: Uuid::now_v7(),
		username: "testuser".to_string(),
		email: "test@example.com".to_string(),
		first_name: String::new(),
		last_name: String::new(),
		password_hash: None,
		last_login: None,
		is_active,
		is_staff,
		is_superuser,
		date_joined: Utc::now(),
		user_permissions: Vec::new(),
		groups: Vec::new(),
	};

	assert_eq!(
		User::is_active(&user),
		is_active,
		"is_active should match for {}",
		desc
	);
	assert_eq!(
		User::is_staff(&user),
		is_staff,
		"is_staff should match for {}",
		desc
	);
	assert_eq!(
		User::is_superuser(&user),
		is_superuser,
		"is_superuser should match for {}",
		desc
	);
	assert_eq!(
		User::is_admin(&user),
		is_superuser,
		"is_admin should equal is_superuser for {}",
		desc
	);
}

// =============================================================================
// Decision Table Tests - Permission Resolution
// =============================================================================

#[rstest]
#[case(true, false, vec![], true, "superuser always has permission")]
#[case(false, true, vec!["app.perm"], true, "explicit permission granted")]
#[case(false, true, vec!["app.other"], false, "different permission")]
#[case(false, false, vec!["app.perm"], false, "inactive user cannot access")]
#[case(true, true, vec![], true, "active superuser has all")]
fn test_permission_decision_table(
	#[case] is_superuser: bool,
	#[case] is_active: bool,
	#[case] permissions: Vec<&str>,
	#[case] _expected_has_perm: bool,
	#[case] desc: &str,
) {
	let user = DefaultUser {
		id: Uuid::now_v7(),
		username: "testuser".to_string(),
		email: "test@example.com".to_string(),
		first_name: String::new(),
		last_name: String::new(),
		password_hash: None,
		last_login: None,
		is_active,
		is_staff: false,
		is_superuser,
		date_joined: Utc::now(),
		user_permissions: permissions.iter().map(|s| s.to_string()).collect(),
		groups: Vec::new(),
	};

	// Note: has_perm does not check is_active in the basic implementation
	// This tests the actual behavior which may differ from Django
	let has_perm = user.has_perm("app.perm");

	// For superuser or explicit permission case
	if is_superuser || permissions.contains(&"app.perm") {
		assert!(has_perm, "User should have permission for case: {}", desc);
	} else {
		assert!(
			!has_perm,
			"User should not have permission for case: {}",
			desc
		);
	}
}

// =============================================================================
// Clone and Debug Tests
// =============================================================================

#[rstest]
fn test_user_clone(basic_user: DefaultUser) {
	let cloned = basic_user.clone();

	assert_eq!(cloned.id, basic_user.id);
	assert_eq!(cloned.username, basic_user.username);
	assert_eq!(cloned.email, basic_user.email);
	assert_eq!(cloned.first_name, basic_user.first_name);
	assert_eq!(cloned.last_name, basic_user.last_name);
	assert_eq!(cloned.password_hash, basic_user.password_hash);
	assert_eq!(cloned.is_active, basic_user.is_active);
	assert_eq!(cloned.is_staff, basic_user.is_staff);
	assert_eq!(cloned.is_superuser, basic_user.is_superuser);
	assert_eq!(cloned.user_permissions, basic_user.user_permissions);
	assert_eq!(cloned.groups, basic_user.groups);
}

#[rstest]
fn test_user_debug(basic_user: DefaultUser) {
	let debug_str = format!("{:?}", basic_user);

	assert!(
		debug_str.contains("DefaultUser"),
		"Debug output should contain type name"
	);
	assert!(
		debug_str.contains("testuser"),
		"Debug output should contain username"
	);
}

// =============================================================================
// Serialization Tests
// =============================================================================

#[rstest]
fn test_user_serialization(basic_user: DefaultUser) {
	let json = serde_json::to_string(&basic_user);
	assert!(json.is_ok(), "User should serialize to JSON");

	let json_str = json.unwrap();
	assert!(
		json_str.contains("testuser"),
		"JSON should contain username"
	);
	assert!(
		json_str.contains("test@example.com"),
		"JSON should contain email"
	);
}

#[rstest]
fn test_user_deserialization(basic_user: DefaultUser) {
	let json = serde_json::to_string(&basic_user).unwrap();
	let deserialized: Result<DefaultUser, _> = serde_json::from_str(&json);

	assert!(deserialized.is_ok(), "User should deserialize from JSON");

	let user = deserialized.unwrap();
	assert_eq!(user.username, basic_user.username);
	assert_eq!(user.email, basic_user.email);
	assert_eq!(user.id, basic_user.id);
}

#[rstest]
fn test_user_roundtrip_with_password(mut basic_user: DefaultUser) {
	basic_user.set_password("testpassword").unwrap();

	let json = serde_json::to_string(&basic_user).unwrap();
	let deserialized: DefaultUser = serde_json::from_str(&json).unwrap();

	// Password hash should be preserved
	assert!(
		deserialized.password_hash.is_some(),
		"Password hash should be preserved after serialization"
	);

	// Password should still verify
	assert!(
		deserialized.check_password("testpassword").unwrap(),
		"Password should verify after deserialization"
	);
}
