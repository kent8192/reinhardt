use reinhardt_auth::{DefaultUser, PermissionsMixin};
use uuid::Uuid;

#[test]
fn test_permissions_mixin_has_perm() {
	// Test intent: Verify PermissionsMixin::has_perm() correctly checks
	// if user has specific permission in user_permissions list
	// Not intent: Superuser bypass, group permissions, wildcard permissions
	let user = DefaultUser {
		id: Uuid::now_v7(),
		username: "alice".to_string(),
		email: "alice@example.com".to_string(),
		first_name: "Alice".to_string(),
		last_name: "Smith".to_string(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_staff: true,
		is_superuser: false,
		date_joined: chrono::Utc::now(),
		user_permissions: vec!["blog.add_post".to_string(), "blog.change_post".to_string()],
		groups: Vec::new(),
	};

	assert!(user.has_perm("blog.add_post"));
	assert!(user.has_perm("blog.change_post"));
	assert!(!user.has_perm("blog.delete_post"));
}

#[test]
fn test_permissions_mixin_superuser_bypass() {
	// Test intent: Verify superuser has_perm() returns true for any
	// permission string without explicit grant in user_permissions
	// Not intent: Permission validation, actual authorization enforcement
	let superuser = DefaultUser {
		id: Uuid::now_v7(),
		username: "admin".to_string(),
		email: "admin@example.com".to_string(),
		first_name: "Admin".to_string(),
		last_name: "User".to_string(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_staff: true,
		is_superuser: true,
		date_joined: chrono::Utc::now(),
		user_permissions: Vec::new(),
		groups: Vec::new(),
	};

	// Superuser has all permissions, even if not explicitly granted
	assert!(superuser.has_perm("blog.add_post"));
	assert!(superuser.has_perm("blog.delete_post"));
	assert!(superuser.has_perm("any.permission"));
}

#[test]
fn test_permissions_mixin_has_module_perms() {
	// Test intent: Verify has_module_perms() returns true if user has
	// any permission with module prefix (e.g., "blog.x")
	// Not intent: Exact permission matching, wildcard modules, case sensitivity
	let user = DefaultUser {
		id: Uuid::now_v7(),
		username: "bob".to_string(),
		email: "bob@example.com".to_string(),
		first_name: "Bob".to_string(),
		last_name: "Johnson".to_string(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_staff: false,
		is_superuser: false,
		date_joined: chrono::Utc::now(),
		user_permissions: vec!["blog.add_post".to_string(), "blog.change_post".to_string()],
		groups: Vec::new(),
	};

	assert!(user.has_module_perms("blog"));
	assert!(!user.has_module_perms("shop"));
}

#[test]
fn test_permissions_mixin_group_permissions() {
	// Test intent: Verify groups() method returns correct group list
	// and contains() can check group membership
	// Not intent: Group-based permission resolution, group hierarchy, permission inheritance
	let user = DefaultUser {
		id: Uuid::now_v7(),
		username: "charlie".to_string(),
		email: "charlie@example.com".to_string(),
		first_name: "Charlie".to_string(),
		last_name: "Brown".to_string(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_staff: false,
		is_superuser: false,
		date_joined: chrono::Utc::now(),
		user_permissions: Vec::new(),
		groups: vec!["editors".to_string(), "moderators".to_string()],
	};

	assert_eq!(user.groups().len(), 2);
	assert!(user.groups().contains(&"editors".to_string()));
	assert!(user.groups().contains(&"moderators".to_string()));
}

#[test]
fn test_permissions_mixin_get_all_permissions() {
	// Test intent: Verify get_all_permissions() returns complete list
	// of user permissions with correct count and content
	// Not intent: Group permissions aggregation, superuser all-permissions behavior, permission caching
	let user = DefaultUser {
		id: Uuid::now_v7(),
		username: "dave".to_string(),
		email: "dave@example.com".to_string(),
		first_name: "Dave".to_string(),
		last_name: "Wilson".to_string(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_staff: false,
		is_superuser: false,
		date_joined: chrono::Utc::now(),
		user_permissions: vec![
			"blog.add_post".to_string(),
			"blog.change_post".to_string(),
			"blog.delete_post".to_string(),
		],
		groups: Vec::new(),
	};

	let all_perms = user.get_all_permissions();
	assert_eq!(all_perms.len(), 3);
	assert!(all_perms.contains("blog.add_post"));
	assert!(all_perms.contains("blog.change_post"));
	assert!(all_perms.contains("blog.delete_post"));
}

#[test]
fn test_permissions_mixin_no_permissions() {
	// Test intent: Verify permission checking methods correctly handle
	// users with empty user_permissions and groups lists
	// Not intent: Default permissions, anonymous user behavior, permission denial reasons
	let user = DefaultUser {
		id: Uuid::now_v7(),
		username: "eve".to_string(),
		email: "eve@example.com".to_string(),
		first_name: "Eve".to_string(),
		last_name: "Anderson".to_string(),
		password_hash: None,
		last_login: None,
		is_active: true,
		is_staff: false,
		is_superuser: false,
		date_joined: chrono::Utc::now(),
		user_permissions: Vec::new(),
		groups: Vec::new(),
	};

	assert!(!user.has_perm("blog.add_post"));
	assert!(!user.has_module_perms("blog"));
	assert_eq!(user.get_all_permissions().len(), 0);
}
