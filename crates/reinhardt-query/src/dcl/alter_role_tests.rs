//! # AlterRoleStatement Unit Tests
//!
//! Comprehensive unit tests for AlterRoleStatement covering:
//! - Happy Path: Normal operations
//! - Error Path: Validation and error handling
//! - Edge Cases: Boundary values and special cases
//! - State Transitions: Lifecycle operations
//!
//! ## Test Coverage
//!
//! - Statements tested: AlterRoleStatement
//! - Code coverage target: 96%
//! - Total tests: ~35

use crate::backend::{MySqlQueryBuilder, PostgresQueryBuilder, QueryBuilder};
use crate::dcl::{AlterRoleStatement, DropRoleStatement, RoleAttribute, UserOption};
use rstest::rstest;

// ============================================================================
// Happy Path Tests
// ============================================================================

#[rstest]
fn test_alter_role_new() {
	let stmt = AlterRoleStatement::new();

	assert_eq!(stmt.role_name, "");
	assert!(stmt.attributes.is_empty());
	assert!(stmt.options.is_empty());
	assert!(stmt.rename_to.is_none());
}

#[rstest]
#[case("test_role")]
#[case("role_with_underscores")]
#[case("RoleWithCamelCase")]
fn test_alter_role_basic(#[case] role_name: &str) {
	let stmt = AlterRoleStatement::new().role(role_name);

	assert_eq!(stmt.role_name, role_name);
}

#[rstest]
fn test_alter_role_with_single_attribute() {
	let stmt = AlterRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Login);

	assert_eq!(stmt.attributes.len(), 1);
}

#[rstest]
fn test_alter_role_with_multiple_attributes() {
	let stmt = AlterRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Login)
		.attribute(RoleAttribute::CreateDb)
		.attribute(RoleAttribute::ConnectionLimit(10));

	assert_eq!(stmt.attributes.len(), 3);
}

#[rstest]
fn test_alter_role_with_attributes_method() {
	let attrs = vec![
		RoleAttribute::Login,
		RoleAttribute::CreateDb,
		RoleAttribute::Inherit,
	];
	let stmt = AlterRoleStatement::new()
		.role("test_role")
		.attributes(attrs.clone());

	assert_eq!(stmt.attributes, attrs);
}

#[rstest]
fn test_alter_role_rename_to() {
	let stmt = AlterRoleStatement::new()
		.role("old_role")
		.rename_to("new_role");

	assert_eq!(stmt.role_name, "old_role");
	assert_eq!(stmt.rename_to, Some("new_role".to_string()));
}

#[rstest]
fn test_alter_role_with_options() {
	let stmt = AlterRoleStatement::new()
		.role("test_role")
		.option(UserOption::AccountLock)
		.option(UserOption::PasswordExpireNever);

	assert_eq!(stmt.options.len(), 2);
}

#[rstest]
fn test_alter_role_comprehensive() {
	let stmt = AlterRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Login)
		.option(UserOption::Comment("Updated".to_string()));

	assert_eq!(stmt.role_name, "test_role");
	assert_eq!(stmt.attributes.len(), 1);
	assert_eq!(stmt.options.len(), 1);
}

#[rstest]
fn test_alter_role_clone() {
	let stmt1 = AlterRoleStatement::new().role("test_role");
	let stmt2 = stmt1.clone();

	assert_eq!(stmt1.role_name, stmt2.role_name);
}

#[rstest]
fn test_alter_role_builder_pattern() {
	let stmt = AlterRoleStatement::new()
		.role("test_role")
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
fn test_alter_role_empty_name_validation() {
	let stmt = AlterRoleStatement::new();

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_alter_role_whitespace_only_name() {
	let stmt = AlterRoleStatement::new().role("   ");

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_alter_role_no_changes() {
	let stmt = AlterRoleStatement::new().role("test_role");

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_alter_role_valid_with_attributes() {
	let stmt = AlterRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Login);

	let result = stmt.validate();
	assert!(result.is_ok());
}

#[rstest]
fn test_alter_role_valid_with_options() {
	let stmt = AlterRoleStatement::new()
		.role("test_role")
		.option(UserOption::Comment("Test".to_string()));

	let result = stmt.validate();
	assert!(result.is_ok());
}

#[rstest]
fn test_alter_role_valid_with_rename() {
	let stmt = AlterRoleStatement::new()
		.role("old_role")
		.rename_to("new_role");

	let result = stmt.validate();
	assert!(result.is_ok());
}

#[rstest]
fn test_alter_role_valid_comprehensive() {
	let stmt = AlterRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Login)
		.option(UserOption::AccountUnlock);

	let result = stmt.validate();
	assert!(result.is_ok());
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[rstest]
fn test_alter_role_all_attribute_types() {
	let stmt = AlterRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::SuperUser)
		.attribute(RoleAttribute::CreateDb)
		.attribute(RoleAttribute::CreateRole)
		.attribute(RoleAttribute::Inherit)
		.attribute(RoleAttribute::Login)
		.attribute(RoleAttribute::Replication)
		.attribute(RoleAttribute::BypassRls);

	assert_eq!(stmt.attributes.len(), 7);
}

#[rstest]
fn test_alter_role_connection_limits() {
	let limits = vec![-1, 0, 1, 100, i32::MAX];

	for limit in limits {
		let stmt = AlterRoleStatement::new()
			.role("test_role")
			.attribute(RoleAttribute::ConnectionLimit(limit));

		assert_eq!(stmt.attributes.len(), 1);
	}
}

#[rstest]
fn test_alter_role_password_variants() {
	let stmt1 = AlterRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Password("secret".to_string()));
	let stmt2 = AlterRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::EncryptedPassword("md5xxx".to_string()));
	let stmt3 = AlterRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::UnencryptedPassword("plain".to_string()));

	assert_eq!(stmt1.attributes.len(), 1);
	assert_eq!(stmt2.attributes.len(), 1);
	assert_eq!(stmt3.attributes.len(), 1);
}

#[rstest]
fn test_alter_role_all_user_options() {
	let stmt = AlterRoleStatement::new()
		.role("test_role")
		.option(UserOption::Password("secret".to_string()))
		.option(UserOption::AccountLock)
		.option(UserOption::PasswordExpire)
		.option(UserOption::Comment("Test".to_string()));

	assert_eq!(stmt.options.len(), 4);
}

#[rstest]
fn test_alter_role_rename_to_same_name() {
	let stmt = AlterRoleStatement::new()
		.role("test_role")
		.rename_to("test_role");

	assert_eq!(stmt.rename_to, Some("test_role".to_string()));
}

#[rstest]
fn test_alter_role_empty_rename_to() {
	let stmt = AlterRoleStatement::new().role("test_role").rename_to("");

	assert_eq!(stmt.rename_to, Some("".to_string()));
}

#[rstest]
#[case(1, "single char")]
#[case(255, "max length")]
fn test_alter_role_rename_length(#[case] length: usize, #[case] _desc: &str) {
	let new_name = "a".repeat(length);
	let stmt = AlterRoleStatement::new()
		.role("old_role")
		.rename_to(new_name.clone());

	assert_eq!(stmt.rename_to, Some(new_name));
}

// ============================================================================
// State Transition Tests
// ============================================================================

#[rstest]
fn test_lifecycle_create_alter_drop() {
	// CREATE
	let create_stmt = crate::dcl::CreateRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Login);
	assert_eq!(create_stmt.role_name, "test_role");

	// ALTER
	let alter_stmt = AlterRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::CreateDb);
	assert_eq!(alter_stmt.role_name, "test_role");
	assert_eq!(alter_stmt.attributes.len(), 1);

	// DROP
	let drop_stmt = DropRoleStatement::new().role("test_role");
	assert_eq!(drop_stmt.role_names[0], "test_role");
}

#[rstest]
fn test_multiple_alters() {
	let stmt1 = AlterRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Login);

	let stmt2 = AlterRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::CreateDb);

	assert_ne!(stmt1.attributes, stmt2.attributes);
}

#[rstest]
fn test_alter_sequence() {
	let mut role = "test_role".to_string();

	// First alter
	let stmt1 = AlterRoleStatement::new()
		.role(&role)
		.attribute(RoleAttribute::Login);

	// Change role name
	role = "new_role".to_string();

	// Second alter
	let stmt2 = AlterRoleStatement::new()
		.role(&role)
		.attribute(RoleAttribute::CreateDb);

	assert_eq!(stmt1.role_name, "test_role");
	assert_eq!(stmt2.role_name, "new_role");
}

#[rstest]
fn test_alter_with_conflicting_attributes() {
	let stmt = AlterRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::SuperUser)
		.attribute(RoleAttribute::NoSuperUser);

	// Both attributes are added (order matters)
	assert_eq!(stmt.attributes.len(), 2);
}

#[rstest]
fn test_alter_remove_attribute() {
	// Simulate removing an attribute by replacing the list
	let stmt1 = AlterRoleStatement::new()
		.role("test_role")
		.attributes(vec![RoleAttribute::Login, RoleAttribute::CreateDb]);

	let stmt2 = AlterRoleStatement::new()
		.role("test_role")
		.attributes(vec![RoleAttribute::Login]);

	assert_eq!(stmt1.attributes.len(), 2);
	assert_eq!(stmt2.attributes.len(), 1);
}

#[rstest]
fn test_alter_add_then_remove() {
	let stmt1 = AlterRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Login)
		.attribute(RoleAttribute::CreateDb);

	// "Remove" CreateDb by only specifying Login
	let stmt2 = AlterRoleStatement::new()
		.role("test_role")
		.attributes(vec![RoleAttribute::Login]);

	assert!(stmt1.attributes.len() > stmt2.attributes.len());
}

#[rstest]
fn test_alter_evolution() {
	let initial = AlterRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Login);

	let with_create_db = AlterRoleStatement::new()
		.role("test_role")
		.attributes(vec![RoleAttribute::Login, RoleAttribute::CreateDb]);

	let with_connection_limit = AlterRoleStatement::new().role("test_role").attributes(vec![
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
fn test_postgres_alter_role_with_attribute() {
	let builder = PostgresQueryBuilder::new();
	let stmt = AlterRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Login);

	let (sql, values) = builder.build_alter_role(&stmt);

	assert!(sql.contains(r#"ALTER ROLE "test_role" WITH"#));
	assert!(sql.contains("LOGIN"));
	assert!(values.is_empty());
}

#[rstest]
fn test_postgres_alter_role_with_password() {
	let builder = PostgresQueryBuilder::new();
	let stmt = AlterRoleStatement::new()
		.role("test_role")
		.attribute(RoleAttribute::Password("secret".to_string()));

	let (sql, values) = builder.build_alter_role(&stmt);

	assert!(sql.contains(r#"ALTER ROLE "test_role""#));
	assert!(sql.contains("PASSWORD"));
	assert!(!sql.contains("secret"));
	assert_eq!(values.len(), 1);
}

#[rstest]
fn test_postgres_alter_role_rename() {
	let builder = PostgresQueryBuilder::new();
	let stmt = AlterRoleStatement::new()
		.role("old_role")
		.rename_to("new_role");

	let (sql, values) = builder.build_alter_role(&stmt);

	assert!(sql.contains(r#"ALTER ROLE "old_role" RENAME TO "new_role""#));
	assert!(values.is_empty());
}

// ============================================================================
// SQL Generation Tests (MySQL)
// ============================================================================

#[rstest]
fn test_mysql_alter_role_with_option() {
	let builder = MySqlQueryBuilder::new();
	let stmt = AlterRoleStatement::new()
		.role("test_role")
		.option(UserOption::AccountLock);

	let (sql, values) = builder.build_alter_role(&stmt);

	assert!(sql.contains(r#"ALTER ROLE `test_role`"#));
	assert!(sql.contains("ACCOUNT LOCK"));
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_alter_role_with_comment() {
	let builder = MySqlQueryBuilder::new();
	let stmt = AlterRoleStatement::new()
		.role("test_role")
		.option(UserOption::Comment("Test role".to_string()));

	let (sql, values) = builder.build_alter_role(&stmt);

	assert!(sql.contains(r#"ALTER ROLE `test_role`"#));
	assert!(sql.contains("COMMENT"));
	assert!(values.is_empty());
}
