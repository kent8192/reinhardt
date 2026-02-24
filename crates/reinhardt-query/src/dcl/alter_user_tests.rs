//! # AlterUserStatement Unit Tests
//!
//! Comprehensive unit tests for AlterUserStatement covering:
//! - Happy Path: Normal operations
//! - Error Path: Validation and error handling
//! - Edge Cases: Boundary values and special cases
//! - State Transitions: Lifecycle operations
//!
//! ## Test Coverage
//!
//! - Statements tested: AlterUserStatement
//! - Code coverage target: 95%
//! - Total tests: ~30

use crate::backend::{MySqlQueryBuilder, PostgresQueryBuilder, QueryBuilder};
use crate::dcl::{AlterUserStatement, DropUserStatement, RoleAttribute, UserOption};
use rstest::rstest;

// ============================================================================
// Happy Path Tests
// ============================================================================

#[rstest]
fn test_alter_user_new() {
	let stmt = AlterUserStatement::new();

	assert_eq!(stmt.user_name, "");
	assert!(!stmt.if_exists);
	assert!(stmt.attributes.is_empty());
	assert!(stmt.default_roles.is_empty());
	assert!(stmt.options.is_empty());
}

#[rstest]
#[case("test_user")]
#[case("user_with_underscores")]
#[case("UserWithCamelCase")]
fn test_alter_user_basic(#[case] user_name: &str) {
	let stmt = AlterUserStatement::new().user(user_name);

	assert_eq!(stmt.user_name, user_name);
}

#[rstest]
fn test_alter_user_with_single_attribute() {
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Login);

	assert_eq!(stmt.attributes.len(), 1);
}

#[rstest]
fn test_alter_user_with_multiple_attributes() {
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Login)
		.attribute(RoleAttribute::CreateDb)
		.attribute(RoleAttribute::ConnectionLimit(10));

	assert_eq!(stmt.attributes.len(), 3);
}

#[rstest]
fn test_alter_user_with_attributes_method() {
	let attrs = vec![
		RoleAttribute::Login,
		RoleAttribute::CreateDb,
		RoleAttribute::Inherit,
	];
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.attributes(attrs.clone());

	assert_eq!(stmt.attributes, attrs);
}

#[rstest]
fn test_alter_user_with_options() {
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.option(UserOption::AccountLock)
		.option(UserOption::PasswordExpireNever);

	assert_eq!(stmt.options.len(), 2);
}

#[rstest]
fn test_alter_user_comprehensive() {
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Login)
		.option(UserOption::Comment("Updated".to_string()));

	assert_eq!(stmt.user_name, "test_user");
	assert_eq!(stmt.attributes.len(), 1);
	assert_eq!(stmt.options.len(), 1);
}

#[rstest]
fn test_alter_user_with_default_role() {
	let roles = vec!["role1".to_string(), "role2".to_string()];
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.default_role(roles.clone());

	assert_eq!(stmt.default_roles, roles);
}

#[rstest]
fn test_alter_user_clone() {
	let stmt1 = AlterUserStatement::new().user("test_user");
	let stmt2 = stmt1.clone();

	assert_eq!(stmt1.user_name, stmt2.user_name);
}

#[rstest]
fn test_alter_user_builder_pattern() {
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::SuperUser)
		.attribute(RoleAttribute::CreateDb)
		.option(UserOption::AccountUnlock);

	assert_eq!(stmt.attributes.len(), 2);
	assert_eq!(stmt.options.len(), 1);
}

// ============================================================================
// Error Path Tests
// ============================================================================

#[rstest]
fn test_alter_user_empty_name_validation() {
	let stmt = AlterUserStatement::new();

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_alter_user_whitespace_only_name() {
	let stmt = AlterUserStatement::new().user("   ");

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_alter_user_no_changes() {
	let stmt = AlterUserStatement::new().user("test_user");

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_alter_user_valid_with_attributes() {
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Login);

	let result = stmt.validate();
	assert!(result.is_ok());
}

#[rstest]
fn test_alter_user_valid_with_options() {
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.option(UserOption::Comment("Test".to_string()));

	let result = stmt.validate();
	assert!(result.is_ok());
}

#[rstest]
fn test_alter_user_valid_with_default_role() {
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.default_role(vec!["app_role".to_string()]);

	let result = stmt.validate();
	assert!(result.is_ok());
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[rstest]
fn test_alter_user_all_attribute_types() {
	let stmt = AlterUserStatement::new().user("test_user").attributes(vec![
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
fn test_alter_user_clear_default_roles() {
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.default_role(vec![]);

	assert_eq!(stmt.default_roles.len(), 0);
}

#[rstest]
fn test_alter_user_multiple_default_roles() {
	let roles = vec![
		"role1".to_string(),
		"role2".to_string(),
		"role3".to_string(),
	];
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.default_role(roles);

	assert_eq!(stmt.default_roles.len(), 3);
}

#[rstest]
fn test_alter_user_all_user_options() {
	let stmt = AlterUserStatement::new().user("test_user").options(vec![
		UserOption::Password("secret".to_string()),
		UserOption::AccountLock,
		UserOption::PasswordExpire,
		UserOption::Comment("Test".to_string()),
	]);

	assert_eq!(stmt.options.len(), 4);
}

#[rstest]
#[case(1, "single char")]
#[case(255, "max length")]
fn test_alter_user_name_length(#[case] length: usize, #[case] _desc: &str) {
	let user_name = "a".repeat(length);
	let stmt = AlterUserStatement::new().user(user_name.clone());

	assert_eq!(stmt.user_name, user_name);
}

#[rstest]
fn test_alter_user_at_host_syntax() {
	let stmt = AlterUserStatement::new().user("app_user@localhost");

	assert_eq!(stmt.user_name, "app_user@localhost");
}

#[rstest]
fn test_alter_user_change_password() {
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Password("new_secret".to_string()));

	assert_eq!(stmt.attributes.len(), 1);
	match &stmt.attributes[0] {
		RoleAttribute::Password(pwd) => assert_eq!(pwd, "new_secret"),
		_ => panic!("Expected Password attribute"),
	}
}

#[rstest]
fn test_alter_user_connection_limit() {
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::ConnectionLimit(50));

	assert_eq!(stmt.attributes.len(), 1);
}

// ============================================================================
// State Transition Tests
// ============================================================================

#[rstest]
fn test_lifecycle_create_alter_drop() {
	// CREATE
	let create_stmt = crate::dcl::CreateUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Login);
	assert_eq!(create_stmt.user_name, "test_user");

	// ALTER
	let alter_stmt = AlterUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::CreateDb);
	assert_eq!(alter_stmt.user_name, "test_user");
	assert_eq!(alter_stmt.attributes.len(), 1);

	// DROP
	let drop_stmt = DropUserStatement::new().user("test_user");
	assert_eq!(drop_stmt.user_names[0], "test_user");
}

#[rstest]
fn test_multiple_alters() {
	let stmt1 = AlterUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Login);

	let stmt2 = AlterUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::CreateDb);

	assert_ne!(stmt1.attributes, stmt2.attributes);
}

#[rstest]
fn test_alter_sequence() {
	let mut user = "test_user".to_string();

	// First alter
	let stmt1 = AlterUserStatement::new()
		.user(&user)
		.attribute(RoleAttribute::Login);

	// Change user name
	user = "new_user".to_string();

	// Second alter
	let stmt2 = AlterUserStatement::new()
		.user(&user)
		.attribute(RoleAttribute::CreateDb);

	assert_eq!(stmt1.user_name, "test_user");
	assert_eq!(stmt2.user_name, "new_user");
}

#[rstest]
fn test_alter_evolution() {
	let initial = AlterUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Login);

	let with_create_db = AlterUserStatement::new()
		.user("test_user")
		.attributes(vec![RoleAttribute::Login, RoleAttribute::CreateDb]);

	let with_connection_limit = AlterUserStatement::new().user("test_user").attributes(vec![
		RoleAttribute::Login,
		RoleAttribute::CreateDb,
		RoleAttribute::ConnectionLimit(10),
	]);

	assert_eq!(initial.attributes.len(), 1);
	assert_eq!(with_create_db.attributes.len(), 2);
	assert_eq!(with_connection_limit.attributes.len(), 3);
}

// ============================================================================
// SQL Generation Tests (PostgreSQL)
// ============================================================================

#[rstest]
fn test_postgres_alter_user_with_attribute() {
	let builder = PostgresQueryBuilder::new();
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Login);

	let (sql, values) = builder.build_alter_user(&stmt);

	// PostgreSQL ALTER USER is an alias for ALTER ROLE
	assert!(sql.contains(r#"ALTER ROLE "test_user" WITH"#));
	assert!(sql.contains("LOGIN"));
	assert!(values.is_empty());
}

#[rstest]
fn test_postgres_alter_user_with_password() {
	let builder = PostgresQueryBuilder::new();
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.attribute(RoleAttribute::Password("new_secret".to_string()));

	let (sql, values) = builder.build_alter_user(&stmt);

	// PostgreSQL ALTER USER is an alias for ALTER ROLE
	assert!(sql.contains(r#"ALTER ROLE "test_user""#));
	assert!(sql.contains("PASSWORD"));
	assert!(!sql.contains("new_secret"));
	assert_eq!(values.len(), 1);
}

// ============================================================================
// SQL Generation Tests (MySQL)
// ============================================================================

#[rstest]
fn test_mysql_alter_user_with_option() {
	let builder = MySqlQueryBuilder::new();
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.option(UserOption::AccountLock);

	let (sql, values) = builder.build_alter_user(&stmt);

	assert!(sql.contains("ALTER USER 'test_user'@'%'"));
	assert!(sql.contains("ACCOUNT LOCK"));
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_alter_user_with_default_role() {
	let builder = MySqlQueryBuilder::new();
	let stmt = AlterUserStatement::new()
		.user("test_user")
		.default_role(vec!["app_role".to_string()]);

	let (sql, values) = builder.build_alter_user(&stmt);

	assert!(sql.contains("ALTER USER 'test_user'@'%'"));
	assert!(sql.contains(r#"DEFAULT ROLE `app_role`"#));
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_alter_user_at_host() {
	let builder = MySqlQueryBuilder::new();
	let stmt = AlterUserStatement::new()
		.user("app_user@localhost")
		.option(UserOption::AccountUnlock);

	let (sql, values) = builder.build_alter_user(&stmt);

	assert!(sql.contains(r#"ALTER USER 'app_user'@'localhost'"#));
	assert!(sql.contains("ACCOUNT UNLOCK"));
	assert!(values.is_empty());
}
