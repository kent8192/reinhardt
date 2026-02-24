//! # CreateUserStatement Unit Tests
//!
//! Comprehensive unit tests for CreateUserStatement covering:
//! - Happy Path: Normal operations
//! - Error Path: Validation and error handling
//! - Edge Cases: Boundary values and special cases
//! - Attributes: All RoleAttribute and UserOption types
//!
//! ## Test Coverage
//!
//! - Statements tested: CreateUserStatement
//! - Code coverage target: 96%
//! - Total tests: ~40

use crate::backend::{MySqlQueryBuilder, PostgresQueryBuilder, QueryBuilder};
use crate::dcl::{CreateUserStatement, RoleAttribute, UserOption};
use rstest::rstest;

// ============================================================================
// Happy Path Tests
// ============================================================================

#[rstest]
#[case("basic_user")]
#[case("user_with_underscores")]
#[case("UserWithCamelCase")]
fn test_basic_user_creation(#[case] user_name: &str) {
	let stmt = CreateUserStatement::new().user(user_name);

	assert_eq!(stmt.user_name, user_name);
	assert!(!stmt.if_not_exists);
	assert!(stmt.attributes.is_empty());
	assert!(stmt.default_roles.is_empty());
	assert!(stmt.options.is_empty());
}

#[rstest]
fn test_user_with_single_attribute() {
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Login);

	assert_eq!(stmt.attributes.len(), 1);
}

#[rstest]
fn test_user_with_multiple_attributes() {
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Login)
		.attribute(RoleAttribute::CreateDb)
		.attribute(RoleAttribute::ConnectionLimit(10));

	assert_eq!(stmt.attributes.len(), 3);
}

#[rstest]
fn test_user_with_attributes_method() {
	let attrs = vec![
		RoleAttribute::Login,
		RoleAttribute::CreateDb,
		RoleAttribute::Inherit,
	];
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.attributes(attrs.clone());

	assert_eq!(stmt.attributes, attrs);
}

#[rstest]
fn test_user_if_not_exists() {
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.if_not_exists(true);

	assert!(stmt.if_not_exists);
}

#[rstest]
fn test_user_with_single_option() {
	let opt = UserOption::Comment("Test user".to_string());
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.option(opt.clone());

	assert_eq!(stmt.options.len(), 1);
	assert_eq!(stmt.options[0], opt);
}

#[rstest]
fn test_user_with_multiple_options() {
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.option(UserOption::AccountLock)
		.option(UserOption::PasswordExpireNever);

	assert_eq!(stmt.options.len(), 2);
}

#[rstest]
fn test_user_with_options_method() {
	let opts = vec![
		UserOption::Comment("Test".to_string()),
		UserOption::AccountUnlock,
	];
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.options(opts.clone());

	assert_eq!(stmt.options, opts);
}

#[rstest]
fn test_user_with_default_role() {
	let roles = vec!["role1".to_string(), "role2".to_string()];
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.default_role(roles.clone());

	assert_eq!(stmt.default_roles, roles);
}

#[rstest]
fn test_user_comprehensive() {
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Login)
		.default_role(vec!["app_role".to_string()])
		.option(UserOption::Comment("Test".to_string()));

	assert_eq!(stmt.user_name, "test_user");
	assert_eq!(stmt.attributes.len(), 1);
	assert_eq!(stmt.default_roles.len(), 1);
	assert_eq!(stmt.options.len(), 1);
}

#[rstest]
fn test_user_at_host_syntax() {
	let stmt = CreateUserStatement::new().user("app_user@localhost");

	assert_eq!(stmt.user_name, "app_user@localhost");
}

#[rstest]
fn test_user_with_password() {
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Password("secret123".to_string()));

	assert_eq!(stmt.attributes.len(), 1);
	match &stmt.attributes[0] {
		RoleAttribute::Password(pwd) => assert_eq!(pwd, "secret123"),
		_ => panic!("Expected Password attribute"),
	}
}

// ============================================================================
// Error Path Tests
// ============================================================================

#[rstest]
fn test_empty_user_name_validation() {
	let stmt = CreateUserStatement::new();

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_whitespace_only_user_name() {
	let stmt = CreateUserStatement::new().user("   ");

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_empty_string_user_name() {
	let stmt = CreateUserStatement::new().user("");

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_validate_with_valid_user() {
	let stmt = CreateUserStatement::new().user("valid_user");

	let result = stmt.validate();
	assert!(result.is_ok());
}

#[rstest]
fn test_validate_with_valid_user_and_attributes() {
	let stmt = CreateUserStatement::new()
		.user("valid_user")
		.attribute(RoleAttribute::Login);

	let result = stmt.validate();
	assert!(result.is_ok());
}

#[rstest]
fn test_validate_with_valid_user_and_options() {
	let stmt = CreateUserStatement::new()
		.user("valid_user")
		.option(UserOption::Comment("Test".to_string()));

	let result = stmt.validate();
	assert!(result.is_ok());
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[rstest]
#[case(1, "single character")]
#[case(63, "boundary before limit")]
#[case(64, "boundary at limit")]
#[case(255, "long name")]
fn test_user_name_length(#[case] length: usize, #[case] _desc: &str) {
	let user_name = "a".repeat(length);
	let stmt = CreateUserStatement::new().user(user_name.clone());

	assert_eq!(stmt.user_name, user_name);
	assert_eq!(stmt.user_name.len(), length);
}

#[rstest]
fn test_very_long_user_name() {
	let user_name = "a".repeat(256);
	let stmt = CreateUserStatement::new().user(user_name);

	assert_eq!(stmt.user_name.len(), 256);
}

#[rstest]
fn test_empty_password() {
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Password("".to_string()));

	assert_eq!(stmt.attributes.len(), 1);
	match &stmt.attributes[0] {
		RoleAttribute::Password(pwd) => assert_eq!(pwd, ""),
		_ => panic!("Expected Password attribute"),
	}
}

#[rstest]
fn test_password_with_special_characters() {
	let password = "P@ssw0rd!#$%^&*()";
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Password(password.to_string()));

	assert_eq!(stmt.attributes.len(), 1);
	match &stmt.attributes[0] {
		RoleAttribute::Password(pwd) => assert_eq!(pwd, password),
		_ => panic!("Expected Password attribute"),
	}
}

#[rstest]
fn test_multiple_default_roles() {
	let roles = vec![
		"role1".to_string(),
		"role2".to_string(),
		"role3".to_string(),
	];
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.default_role(roles.clone());

	assert_eq!(stmt.default_roles.len(), 3);
}

#[rstest]
fn test_user_at_host_variations() {
	let cases = vec![
		"user@localhost",
		"user@%",
		"user@'192.168.1.1'",
		"user@'example.com'",
	];

	for user_host in cases {
		let stmt = CreateUserStatement::new().user(user_host);
		assert_eq!(stmt.user_name, user_host);
	}
}

#[rstest]
fn test_encrypted_password() {
	let encrypted = "md5a1b2c3d4e5f6";
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::EncryptedPassword(encrypted.to_string()));

	assert_eq!(stmt.attributes.len(), 1);
	match &stmt.attributes[0] {
		RoleAttribute::EncryptedPassword(pwd) => assert_eq!(pwd, encrypted),
		_ => panic!("Expected EncryptedPassword attribute"),
	}
}

#[rstest]
fn test_valid_until() {
	let timestamp = "2025-12-31T23:59:59Z";
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::ValidUntil(timestamp.to_string()));

	assert_eq!(stmt.attributes.len(), 1);
}

#[rstest]
fn test_connection_limit_values() {
	let limits = vec![-1, 0, 1, 100, i32::MAX];

	for limit in limits {
		let stmt = CreateUserStatement::new()
			.user("test_user")
			.attribute(RoleAttribute::ConnectionLimit(limit));

		assert_eq!(stmt.attributes.len(), 1);
	}
}

// ============================================================================
// Attributes Tests
// ============================================================================

#[rstest]
fn test_all_boolean_attributes() {
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.attributes(vec![
			RoleAttribute::SuperUser,
			RoleAttribute::CreateDb,
			RoleAttribute::CreateRole,
			RoleAttribute::Inherit,
			RoleAttribute::Login,
			RoleAttribute::Replication,
			RoleAttribute::BypassRls,
		]);

	assert_eq!(stmt.attributes.len(), 7);
}

#[rstest]
fn test_all_user_option_types() {
	let stmt = CreateUserStatement::new().user("test_user").options(vec![
		UserOption::Password("secret".to_string()),
		UserOption::AccountLock,
		UserOption::PasswordExpireNever,
		UserOption::Comment("Test".to_string()),
	]);

	assert_eq!(stmt.options.len(), 4);
}

#[rstest]
fn test_mixed_attributes_and_options() {
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Login)
		.attribute(RoleAttribute::ConnectionLimit(10))
		.option(UserOption::AccountUnlock)
		.option(UserOption::PasswordExpireInterval(90));

	assert_eq!(stmt.attributes.len(), 2);
	assert_eq!(stmt.options.len(), 2);
}

// ============================================================================
// SQL Generation Tests (PostgreSQL)
// ============================================================================

#[rstest]
fn test_postgres_create_user_basic() {
	let builder = PostgresQueryBuilder::new();
	let stmt = CreateUserStatement::new().user("test_user");

	let (sql, values) = builder.build_create_user(&stmt);

	// PostgreSQL CREATE USER is CREATE ROLE WITH LOGIN
	assert_eq!(sql, r#"CREATE ROLE "test_user" WITH LOGIN"#);
	assert!(values.is_empty());
}

#[rstest]
fn test_postgres_create_user_with_login() {
	let builder = PostgresQueryBuilder::new();
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Login);

	let (sql, values) = builder.build_create_user(&stmt);

	// PostgreSQL CREATE USER is CREATE ROLE WITH LOGIN
	assert!(sql.contains(r#"CREATE ROLE "test_user""#));
	assert!(sql.contains("WITH"));
	assert!(sql.contains("LOGIN"));
	assert!(values.is_empty());
}

#[rstest]
fn test_postgres_create_user_with_password() {
	let builder = PostgresQueryBuilder::new();
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Password("secret".to_string()));

	let (sql, values) = builder.build_create_user(&stmt);

	// PostgreSQL CREATE USER is CREATE ROLE WITH LOGIN
	assert!(sql.contains(r#"CREATE ROLE "test_user""#));
	assert!(sql.contains("PASSWORD"));
	assert!(!sql.contains("secret"));
	assert_eq!(values.len(), 1);
}

// ============================================================================
// SQL Generation Tests (MySQL)
// ============================================================================

#[rstest]
fn test_mysql_create_user_basic() {
	let builder = MySqlQueryBuilder::new();
	let stmt = CreateUserStatement::new().user("test_user");

	let (sql, values) = builder.build_create_user(&stmt);

	assert_eq!(sql, "CREATE USER 'test_user'@'%'");
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_create_user_at_host() {
	let builder = MySqlQueryBuilder::new();
	let stmt = CreateUserStatement::new().user("app_user@localhost");

	let (sql, values) = builder.build_create_user(&stmt);

	assert!(sql.contains(r#"CREATE USER 'app_user'@'localhost'"#));
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_create_user_if_not_exists() {
	let builder = MySqlQueryBuilder::new();
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.if_not_exists(true);

	let (sql, values) = builder.build_create_user(&stmt);

	assert!(sql.contains(r#"CREATE USER IF NOT EXISTS"#));
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_create_user_with_default_role() {
	let builder = MySqlQueryBuilder::new();
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.default_role(vec!["app_role".to_string()]);

	let (sql, values) = builder.build_create_user(&stmt);

	assert!(sql.contains(r#"DEFAULT ROLE `app_role`"#));
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_create_user_with_multiple_default_roles() {
	let builder = MySqlQueryBuilder::new();
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.default_role(vec!["role1".to_string(), "role2".to_string()]);

	let (sql, values) = builder.build_create_user(&stmt);

	assert!(sql.contains(r#"DEFAULT ROLE"#));
	assert!(sql.contains("`role1`"));
	assert!(sql.contains("`role2`"));
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_create_user_with_option() {
	let builder = MySqlQueryBuilder::new();
	let stmt = CreateUserStatement::new()
		.user("test_user")
		.option(UserOption::AccountLock);

	let (sql, values) = builder.build_create_user(&stmt);

	assert!(sql.contains(r#"CREATE USER"#));
	assert!(sql.contains("ACCOUNT LOCK"));
	assert!(values.is_empty());
}
