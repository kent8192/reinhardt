//! # CreateRoleStatement Unit Tests
//!
//! Comprehensive unit tests for CreateRoleStatement covering:
//! - Happy Path: Normal operations
//! - Error Path: Validation and error handling
//! - Edge Cases: Boundary values and special cases
//! - Combinations: Attribute/option combinations
//!
//! ## Test Coverage
//!
//! - Statements tested: CreateRoleStatement
//! - Code coverage target: 97%
//! - Total tests: ~40

use crate::backend::{MySqlQueryBuilder, PostgresQueryBuilder, QueryBuilder};
use crate::dcl::{CreateRoleStatement, RoleAttribute, UserOption};
use rstest::rstest;

// ============================================================================
// Happy Path Tests
// ============================================================================

#[rstest]
#[case("basic_role")]
#[case("role_with_underscores")]
#[case("RoleWithCamelCase")]
#[case("role123")]
#[case("UPPERCASE_ROLE")]
fn test_basic_creation(#[case] role_name: &str) {
	let stmt = CreateRoleStatement::new().role(role_name);

	assert_eq!(stmt.role_name, role_name);
	assert!(!stmt.if_not_exists);
	assert!(stmt.attributes.is_empty());
	assert!(stmt.options.is_empty());
}

#[rstest]
#[case(RoleAttribute::Login)]
#[case(RoleAttribute::CreateDb)]
#[case(RoleAttribute::CreateRole)]
#[case(RoleAttribute::Inherit)]
#[case(RoleAttribute::Replication)]
#[case(RoleAttribute::BypassRls)]
fn test_single_attribute(#[case] attr: RoleAttribute) {
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.attribute(attr.clone());

	assert_eq!(stmt.attributes.len(), 1);
	assert_eq!(stmt.attributes[0], attr);
}

#[rstest]
fn test_multiple_attributes() {
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Login)
		.attribute(RoleAttribute::CreateDb)
		.attribute(RoleAttribute::ConnectionLimit(10));

	assert_eq!(stmt.attributes.len(), 3);
}

#[rstest]
fn test_attributes_method() {
	let attrs = vec![
		RoleAttribute::Login,
		RoleAttribute::CreateDb,
		RoleAttribute::Inherit,
	];

	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.attributes(attrs.clone());

	assert_eq!(stmt.attributes, attrs);
}

#[rstest]
fn test_if_not_exists() {
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.if_not_exists(true);

	assert!(stmt.if_not_exists);
}

#[rstest]
fn test_single_option() {
	let opt = UserOption::Comment("Test role".to_string());
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.option(opt.clone());

	assert_eq!(stmt.options.len(), 1);
	assert_eq!(stmt.options[0], opt);
}

#[rstest]
fn test_multiple_options() {
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.option(UserOption::AccountLock)
		.option(UserOption::PasswordExpireNever);

	assert_eq!(stmt.options.len(), 2);
}

#[rstest]
fn test_options_method() {
	let opts = vec![
		UserOption::Comment("Test".to_string()),
		UserOption::AccountUnlock,
	];

	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.options(opts.clone());

	assert_eq!(stmt.options, opts);
}

#[rstest]
fn test_attributes_and_options() {
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Login)
		.option(UserOption::Comment("Test".to_string()));

	assert_eq!(stmt.attributes.len(), 1);
	assert_eq!(stmt.options.len(), 1);
}

#[rstest]
fn test_connection_limit_attribute() {
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::ConnectionLimit(100));

	assert_eq!(stmt.attributes.len(), 1);
	match &stmt.attributes[0] {
		RoleAttribute::ConnectionLimit(limit) => assert_eq!(*limit, 100),
		_ => panic!("Expected ConnectionLimit attribute"),
	}
}

#[rstest]
fn test_password_attribute() {
	let stmt = CreateRoleStatement::new()
		.role("test_role")
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
fn test_empty_role_name_validation() {
	let stmt = CreateRoleStatement::new();

	let result = stmt.validate();
	assert!(result.is_err());
	assert_eq!(
		result.unwrap_err(),
		"Role name cannot be empty or whitespace only"
	);
}

#[rstest]
fn test_whitespace_only_role_name() {
	let stmt = CreateRoleStatement::new().role("   ");

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_empty_string_role_name() {
	let stmt = CreateRoleStatement::new().role("");

	let result = stmt.validate();
	assert!(result.is_err());
	assert_eq!(
		result.unwrap_err(),
		"Role name cannot be empty or whitespace only"
	);
}

#[rstest]
fn test_validate_with_valid_role() {
	let stmt = CreateRoleStatement::new().role("valid_role");

	let result = stmt.validate();
	assert!(result.is_ok());
}

#[rstest]
fn test_validate_with_valid_role_and_attributes() {
	let stmt = CreateRoleStatement::new()
		.role("valid_role")
		.attribute(RoleAttribute::Login)
		.attribute(RoleAttribute::CreateDb);

	let result = stmt.validate();
	assert!(result.is_ok());
}

#[rstest]
fn test_validate_with_valid_role_and_options() {
	let stmt = CreateRoleStatement::new()
		.role("valid_role")
		.option(UserOption::Comment("Test".to_string()));

	let result = stmt.validate();
	assert!(result.is_ok());
}

#[rstest]
fn test_validate_with_complete_statement() {
	let stmt = CreateRoleStatement::new()
		.role("valid_role")
		.attribute(RoleAttribute::Login)
		.option(UserOption::AccountUnlock);

	let result = stmt.validate();
	assert!(result.is_ok());
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[rstest]
#[case(0, "empty string")]
#[case(1, "single character")]
#[case(63, "boundary before limit")]
#[case(64, "boundary at limit")]
#[case(255, "long name")]
fn test_role_name_length(#[case] length: usize, #[case] _desc: &str) {
	let role_name = "a".repeat(length);
	let stmt = CreateRoleStatement::new().role(role_name.clone());

	assert_eq!(stmt.role_name, role_name);
	assert_eq!(stmt.role_name.len(), length);
}

#[rstest]
fn test_very_long_role_name() {
	// 256 characters - likely exceeds database limits
	let role_name = "a".repeat(256);
	let stmt = CreateRoleStatement::new().role(role_name);

	assert_eq!(stmt.role_name.len(), 256);
}

#[rstest]
#[case(-1, "unlimited")]
#[case(0, "no connections")]
#[case(1, "single connection")]
#[case(100, "moderate limit")]
#[case(i32::MAX, "maximum limit")]
fn test_connection_limit_values(#[case] limit: i32, #[case] _desc: &str) {
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::ConnectionLimit(limit));

	assert_eq!(stmt.attributes.len(), 1);
}

#[rstest]
fn test_empty_password() {
	let stmt = CreateRoleStatement::new()
		.role("test_role")
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
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Password(password.to_string()));

	assert_eq!(stmt.attributes.len(), 1);
	match &stmt.attributes[0] {
		RoleAttribute::Password(pwd) => assert_eq!(pwd, password),
		_ => panic!("Expected Password attribute"),
	}
}

#[rstest]
fn test_valid_until_attribute() {
	let timestamp = "2025-12-31T23:59:59Z";
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::ValidUntil(timestamp.to_string()));

	assert_eq!(stmt.attributes.len(), 1);
	match &stmt.attributes[0] {
		RoleAttribute::ValidUntil(ts) => assert_eq!(ts, timestamp),
		_ => panic!("Expected ValidUntil attribute"),
	}
}

#[rstest]
fn test_in_role_attribute() {
	let roles = vec!["role1".to_string(), "role2".to_string()];
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::InRole(roles.clone()));

	assert_eq!(stmt.attributes.len(), 1);
	match &stmt.attributes[0] {
		RoleAttribute::InRole(r) => assert_eq!(r, &roles),
		_ => panic!("Expected InRole attribute"),
	}
}

#[rstest]
fn test_encrypted_password_attribute() {
	let encrypted = "md5a1b2c3d4e5f6";
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::EncryptedPassword(encrypted.to_string()));

	assert_eq!(stmt.attributes.len(), 1);
	match &stmt.attributes[0] {
		RoleAttribute::EncryptedPassword(pwd) => assert_eq!(pwd, encrypted),
		_ => panic!("Expected EncryptedPassword attribute"),
	}
}

// ============================================================================
// Combination Tests
// ============================================================================

#[rstest]
fn test_boolean_attribute_pairs() {
	// SuperUser vs NoSuperUser
	let stmt1 = CreateRoleStatement::new()
		.role("role1")
		.attribute(RoleAttribute::SuperUser);
	let stmt2 = CreateRoleStatement::new()
		.role("role2")
		.attribute(RoleAttribute::NoSuperUser);

	assert_ne!(stmt1.attributes, stmt2.attributes);
}

#[rstest]
fn test_all_boolean_attributes() {
	let attrs = vec![
		RoleAttribute::SuperUser,
		RoleAttribute::CreateDb,
		RoleAttribute::CreateRole,
		RoleAttribute::Inherit,
		RoleAttribute::Login,
		RoleAttribute::Replication,
		RoleAttribute::BypassRls,
	];

	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.attributes(attrs.clone());

	assert_eq!(stmt.attributes.len(), 7);
}

#[rstest]
fn test_mixed_attributes() {
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Login)
		.attribute(RoleAttribute::ConnectionLimit(10))
		.attribute(RoleAttribute::Password("secret".to_string()))
		.attribute(RoleAttribute::ValidUntil("2025-12-31".to_string()));

	assert_eq!(stmt.attributes.len(), 4);
}

#[rstest]
fn test_all_user_option_types() {
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.option(UserOption::Password("secret".to_string()))
		.option(UserOption::AccountLock)
		.option(UserOption::PasswordExpireNever)
		.option(UserOption::Comment("Test".to_string()));

	assert_eq!(stmt.options.len(), 4);
}

#[rstest]
fn test_comprehensive_role_statement() {
	let stmt = CreateRoleStatement::new()
		.role("app_user")
		.if_not_exists(true)
		.attributes(vec![
			RoleAttribute::Login,
			RoleAttribute::CreateDb,
			RoleAttribute::ConnectionLimit(10),
			RoleAttribute::Password("secret123".to_string()),
		])
		.options(vec![
			UserOption::Comment("Application user".to_string()),
			UserOption::AccountUnlock,
		]);

	assert_eq!(stmt.role_name, "app_user");
	assert!(stmt.if_not_exists);
	assert_eq!(stmt.attributes.len(), 4);
	assert_eq!(stmt.options.len(), 2);
}

#[rstest]
fn test_clone_statement() {
	let stmt1 = CreateRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Login);
	let stmt2 = stmt1.clone();

	assert_eq!(stmt1.role_name, stmt2.role_name);
	assert_eq!(stmt1.attributes, stmt2.attributes);
}

#[rstest]
fn test_default_statement() {
	let stmt = CreateRoleStatement::new();

	assert_eq!(stmt.role_name, "");
	assert!(!stmt.if_not_exists);
	assert!(stmt.attributes.is_empty());
	assert!(stmt.options.is_empty());
}

#[rstest]
fn test_builder_pattern_chaining() {
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.if_not_exists(true)
		.attribute(RoleAttribute::Login)
		.attribute(RoleAttribute::CreateDb)
		.option(UserOption::Comment("Test".to_string()))
		.option(UserOption::AccountUnlock);

	assert_eq!(stmt.role_name, "test_role");
	assert!(stmt.if_not_exists);
	assert_eq!(stmt.attributes.len(), 2);
	assert_eq!(stmt.options.len(), 2);
}

// ============================================================================
// SQL Generation Tests (PostgreSQL)
// ============================================================================

#[rstest]
fn test_postgres_basic_create_role() {
	let builder = PostgresQueryBuilder::new();
	let stmt = CreateRoleStatement::new().role("test_role");

	let (sql, values) = builder.build_create_role(&stmt);

	assert_eq!(sql, r#"CREATE ROLE "test_role""#);
	assert!(values.is_empty());
}

#[rstest]
fn test_postgres_create_role_with_login() {
	let builder = PostgresQueryBuilder::new();
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Login);

	let (sql, values) = builder.build_create_role(&stmt);

	assert!(sql.contains(r#"CREATE ROLE "test_role""#));
	assert!(sql.contains("WITH"));
	assert!(sql.contains("LOGIN"));
	assert!(values.is_empty());
}

#[rstest]
fn test_postgres_create_role_with_password() {
	let builder = PostgresQueryBuilder::new();
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Password("secret".to_string()));

	let (sql, values) = builder.build_create_role(&stmt);

	assert!(sql.contains(r#"CREATE ROLE "test_role""#));
	assert!(sql.contains("PASSWORD"));
	// Password should be parameterized
	assert!(!sql.contains("secret"));
	assert_eq!(values.len(), 1);
}

// ============================================================================
// SQL Generation Tests (MySQL)
// ============================================================================

#[rstest]
fn test_mysql_basic_create_role() {
	let builder = MySqlQueryBuilder::new();
	let stmt = CreateRoleStatement::new().role("test_role");

	let (sql, values) = builder.build_create_role(&stmt);

	assert_eq!(sql, r#"CREATE ROLE `test_role`"#);
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_create_role_if_not_exists() {
	let builder = MySqlQueryBuilder::new();
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.if_not_exists(true);

	let (sql, values) = builder.build_create_role(&stmt);

	assert!(sql.contains(r#"CREATE ROLE IF NOT EXISTS `test_role`"#));
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_create_role_with_comment() {
	let builder = MySqlQueryBuilder::new();
	let stmt = CreateRoleStatement::new()
		.role("test_role")
		.option(UserOption::Comment("Test role".to_string()));

	let (sql, values) = builder.build_create_role(&stmt);

	assert!(sql.contains(r#"CREATE ROLE `test_role`"#));
	assert!(sql.contains("COMMENT"));
	assert!(values.is_empty());
}
