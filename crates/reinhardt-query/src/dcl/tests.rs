//! Unit tests for DCL (Data Control Language) module

#[cfg(test)]
mod privilege_tests {
	use crate::*;

	#[test]
	fn test_privilege_as_sql() {
		assert_eq!(Privilege::Select.as_sql(), "SELECT");
		assert_eq!(Privilege::Insert.as_sql(), "INSERT");
		assert_eq!(Privilege::Update.as_sql(), "UPDATE");
		assert_eq!(Privilege::Delete.as_sql(), "DELETE");
		assert_eq!(Privilege::References.as_sql(), "REFERENCES");
		assert_eq!(Privilege::Create.as_sql(), "CREATE");
		assert_eq!(Privilege::All.as_sql(), "ALL PRIVILEGES");
		assert_eq!(Privilege::Truncate.as_sql(), "TRUNCATE");
		assert_eq!(Privilege::Trigger.as_sql(), "TRIGGER");
		assert_eq!(Privilege::Maintain.as_sql(), "MAINTAIN");
		assert_eq!(Privilege::Usage.as_sql(), "USAGE");
		assert_eq!(Privilege::Connect.as_sql(), "CONNECT");
		assert_eq!(Privilege::Temporary.as_sql(), "TEMPORARY");
		assert_eq!(Privilege::Execute.as_sql(), "EXECUTE");
		assert_eq!(Privilege::Set.as_sql(), "SET");
		assert_eq!(Privilege::AlterSystem.as_sql(), "ALTER SYSTEM");
	}

	#[test]
	fn test_privilege_is_postgres_only() {
		// Common privileges
		assert!(!Privilege::Select.is_postgres_only());
		assert!(!Privilege::Insert.is_postgres_only());
		assert!(!Privilege::Update.is_postgres_only());
		assert!(!Privilege::Delete.is_postgres_only());
		assert!(!Privilege::References.is_postgres_only());
		assert!(!Privilege::Create.is_postgres_only());
		assert!(!Privilege::All.is_postgres_only());

		// PostgreSQL-specific privileges
		assert!(Privilege::Truncate.is_postgres_only());
		assert!(Privilege::Trigger.is_postgres_only());
		assert!(Privilege::Maintain.is_postgres_only());
		assert!(Privilege::Usage.is_postgres_only());
		assert!(Privilege::Connect.is_postgres_only());
		assert!(Privilege::Temporary.is_postgres_only());
		assert!(Privilege::Execute.is_postgres_only());
		assert!(Privilege::Set.is_postgres_only());
		assert!(Privilege::AlterSystem.is_postgres_only());
	}

	#[test]
	fn test_privilege_valid_for_table() {
		assert!(Privilege::Select.is_valid_for_object(ObjectType::Table));
		assert!(Privilege::Insert.is_valid_for_object(ObjectType::Table));
		assert!(Privilege::Update.is_valid_for_object(ObjectType::Table));
		assert!(Privilege::Delete.is_valid_for_object(ObjectType::Table));
		assert!(Privilege::References.is_valid_for_object(ObjectType::Table));
		assert!(Privilege::Truncate.is_valid_for_object(ObjectType::Table));
		assert!(Privilege::Trigger.is_valid_for_object(ObjectType::Table));
		assert!(Privilege::Maintain.is_valid_for_object(ObjectType::Table));
		assert!(Privilege::All.is_valid_for_object(ObjectType::Table));

		// Invalid for table
		assert!(!Privilege::Connect.is_valid_for_object(ObjectType::Table));
		assert!(!Privilege::Temporary.is_valid_for_object(ObjectType::Table));
	}

	#[test]
	fn test_privilege_valid_for_database() {
		assert!(Privilege::Create.is_valid_for_object(ObjectType::Database));
		assert!(Privilege::Connect.is_valid_for_object(ObjectType::Database));
		assert!(Privilege::Temporary.is_valid_for_object(ObjectType::Database));
		assert!(Privilege::All.is_valid_for_object(ObjectType::Database));

		// Invalid for database
		assert!(!Privilege::Select.is_valid_for_object(ObjectType::Database));
		assert!(!Privilege::Insert.is_valid_for_object(ObjectType::Database));
		assert!(!Privilege::Truncate.is_valid_for_object(ObjectType::Database));
	}

	#[test]
	fn test_privilege_valid_for_schema() {
		assert!(Privilege::Create.is_valid_for_object(ObjectType::Schema));
		assert!(Privilege::Usage.is_valid_for_object(ObjectType::Schema));
		assert!(Privilege::All.is_valid_for_object(ObjectType::Schema));

		// Invalid for schema
		assert!(!Privilege::Select.is_valid_for_object(ObjectType::Schema));
		assert!(!Privilege::Insert.is_valid_for_object(ObjectType::Schema));
		assert!(!Privilege::Connect.is_valid_for_object(ObjectType::Schema));
	}

	#[test]
	fn test_privilege_valid_for_sequence() {
		assert!(Privilege::Usage.is_valid_for_object(ObjectType::Sequence));
		assert!(Privilege::Select.is_valid_for_object(ObjectType::Sequence));
		assert!(Privilege::Update.is_valid_for_object(ObjectType::Sequence));
		assert!(Privilege::All.is_valid_for_object(ObjectType::Sequence));

		// Invalid for sequence
		assert!(!Privilege::Insert.is_valid_for_object(ObjectType::Sequence));
		assert!(!Privilege::Delete.is_valid_for_object(ObjectType::Sequence));
		assert!(!Privilege::Connect.is_valid_for_object(ObjectType::Sequence));
	}

	#[test]
	fn test_privilege_valid_for_function() {
		assert!(Privilege::Execute.is_valid_for_object(ObjectType::Function));
		assert!(Privilege::All.is_valid_for_object(ObjectType::Function));

		// Invalid for function
		assert!(!Privilege::Select.is_valid_for_object(ObjectType::Function));
		assert!(!Privilege::Usage.is_valid_for_object(ObjectType::Function));
		assert!(!Privilege::Connect.is_valid_for_object(ObjectType::Function));
	}

	#[test]
	fn test_privilege_valid_for_procedure() {
		assert!(Privilege::Execute.is_valid_for_object(ObjectType::Procedure));
		assert!(Privilege::All.is_valid_for_object(ObjectType::Procedure));

		// Invalid for procedure
		assert!(!Privilege::Select.is_valid_for_object(ObjectType::Procedure));
		assert!(!Privilege::Usage.is_valid_for_object(ObjectType::Procedure));
		assert!(!Privilege::Connect.is_valid_for_object(ObjectType::Procedure));
	}

	#[test]
	fn test_privilege_valid_for_routine() {
		assert!(Privilege::Execute.is_valid_for_object(ObjectType::Routine));
		assert!(Privilege::All.is_valid_for_object(ObjectType::Routine));

		// Invalid for routine
		assert!(!Privilege::Select.is_valid_for_object(ObjectType::Routine));
		assert!(!Privilege::Usage.is_valid_for_object(ObjectType::Routine));
		assert!(!Privilege::Connect.is_valid_for_object(ObjectType::Routine));
	}

	#[test]
	fn test_privilege_valid_for_type() {
		assert!(Privilege::Usage.is_valid_for_object(ObjectType::Type));
		assert!(Privilege::All.is_valid_for_object(ObjectType::Type));

		// Invalid for type
		assert!(!Privilege::Select.is_valid_for_object(ObjectType::Type));
		assert!(!Privilege::Execute.is_valid_for_object(ObjectType::Type));
		assert!(!Privilege::Connect.is_valid_for_object(ObjectType::Type));
	}

	#[test]
	fn test_privilege_valid_for_domain() {
		assert!(Privilege::Usage.is_valid_for_object(ObjectType::Domain));
		assert!(Privilege::All.is_valid_for_object(ObjectType::Domain));

		// Invalid for domain
		assert!(!Privilege::Select.is_valid_for_object(ObjectType::Domain));
		assert!(!Privilege::Execute.is_valid_for_object(ObjectType::Domain));
		assert!(!Privilege::Connect.is_valid_for_object(ObjectType::Domain));
	}

	#[test]
	fn test_privilege_valid_for_foreign_data_wrapper() {
		assert!(Privilege::Usage.is_valid_for_object(ObjectType::ForeignDataWrapper));
		assert!(Privilege::All.is_valid_for_object(ObjectType::ForeignDataWrapper));

		// Invalid for foreign data wrapper
		assert!(!Privilege::Select.is_valid_for_object(ObjectType::ForeignDataWrapper));
		assert!(!Privilege::Execute.is_valid_for_object(ObjectType::ForeignDataWrapper));
		assert!(!Privilege::Connect.is_valid_for_object(ObjectType::ForeignDataWrapper));
	}

	#[test]
	fn test_privilege_valid_for_foreign_server() {
		assert!(Privilege::Usage.is_valid_for_object(ObjectType::ForeignServer));
		assert!(Privilege::All.is_valid_for_object(ObjectType::ForeignServer));

		// Invalid for foreign server
		assert!(!Privilege::Select.is_valid_for_object(ObjectType::ForeignServer));
		assert!(!Privilege::Execute.is_valid_for_object(ObjectType::ForeignServer));
		assert!(!Privilege::Connect.is_valid_for_object(ObjectType::ForeignServer));
	}

	#[test]
	fn test_privilege_valid_for_language() {
		assert!(Privilege::Usage.is_valid_for_object(ObjectType::Language));
		assert!(Privilege::All.is_valid_for_object(ObjectType::Language));

		// Invalid for language
		assert!(!Privilege::Select.is_valid_for_object(ObjectType::Language));
		assert!(!Privilege::Execute.is_valid_for_object(ObjectType::Language));
		assert!(!Privilege::Connect.is_valid_for_object(ObjectType::Language));
	}

	#[test]
	fn test_privilege_valid_for_large_object() {
		assert!(Privilege::Select.is_valid_for_object(ObjectType::LargeObject));
		assert!(Privilege::Update.is_valid_for_object(ObjectType::LargeObject));
		assert!(Privilege::All.is_valid_for_object(ObjectType::LargeObject));

		// Invalid for large object
		assert!(!Privilege::Insert.is_valid_for_object(ObjectType::LargeObject));
		assert!(!Privilege::Delete.is_valid_for_object(ObjectType::LargeObject));
		assert!(!Privilege::Execute.is_valid_for_object(ObjectType::LargeObject));
		assert!(!Privilege::Connect.is_valid_for_object(ObjectType::LargeObject));
	}

	#[test]
	fn test_privilege_valid_for_tablespace() {
		assert!(Privilege::Create.is_valid_for_object(ObjectType::Tablespace));
		assert!(Privilege::All.is_valid_for_object(ObjectType::Tablespace));

		// Invalid for tablespace
		assert!(!Privilege::Select.is_valid_for_object(ObjectType::Tablespace));
		assert!(!Privilege::Usage.is_valid_for_object(ObjectType::Tablespace));
		assert!(!Privilege::Execute.is_valid_for_object(ObjectType::Tablespace));
		assert!(!Privilege::Connect.is_valid_for_object(ObjectType::Tablespace));
	}

	#[test]
	fn test_privilege_valid_for_parameter() {
		assert!(Privilege::Set.is_valid_for_object(ObjectType::Parameter));
		assert!(Privilege::AlterSystem.is_valid_for_object(ObjectType::Parameter));
		assert!(Privilege::All.is_valid_for_object(ObjectType::Parameter));

		// Invalid for parameter
		assert!(!Privilege::Select.is_valid_for_object(ObjectType::Parameter));
		assert!(!Privilege::Usage.is_valid_for_object(ObjectType::Parameter));
		assert!(!Privilege::Execute.is_valid_for_object(ObjectType::Parameter));
		assert!(!Privilege::Connect.is_valid_for_object(ObjectType::Parameter));
	}
}

#[cfg(test)]
mod object_type_tests {
	use crate::*;

	#[test]
	fn test_object_type_as_sql() {
		assert_eq!(ObjectType::Table.as_sql(), "TABLE");
		assert_eq!(ObjectType::Database.as_sql(), "DATABASE");
		assert_eq!(ObjectType::Schema.as_sql(), "SCHEMA");
		assert_eq!(ObjectType::Sequence.as_sql(), "SEQUENCE");
		assert_eq!(ObjectType::Function.as_sql(), "FUNCTION");
		assert_eq!(ObjectType::Procedure.as_sql(), "PROCEDURE");
		assert_eq!(ObjectType::Routine.as_sql(), "ROUTINE");
		assert_eq!(ObjectType::Type.as_sql(), "TYPE");
		assert_eq!(ObjectType::Domain.as_sql(), "DOMAIN");
		assert_eq!(
			ObjectType::ForeignDataWrapper.as_sql(),
			"FOREIGN DATA WRAPPER"
		);
		assert_eq!(ObjectType::ForeignServer.as_sql(), "FOREIGN SERVER");
		assert_eq!(ObjectType::Language.as_sql(), "LANGUAGE");
		assert_eq!(ObjectType::LargeObject.as_sql(), "LARGE OBJECT");
		assert_eq!(ObjectType::Tablespace.as_sql(), "TABLESPACE");
		assert_eq!(ObjectType::Parameter.as_sql(), "PARAMETER");
	}

	#[test]
	fn test_object_type_is_postgres_only() {
		// Common object types
		assert!(!ObjectType::Table.is_postgres_only());
		assert!(!ObjectType::Database.is_postgres_only());

		// PostgreSQL-specific object types
		assert!(ObjectType::Schema.is_postgres_only());
		assert!(ObjectType::Sequence.is_postgres_only());
		assert!(ObjectType::Function.is_postgres_only());
		assert!(ObjectType::Procedure.is_postgres_only());
		assert!(ObjectType::Routine.is_postgres_only());
		assert!(ObjectType::Type.is_postgres_only());
		assert!(ObjectType::Domain.is_postgres_only());
		assert!(ObjectType::ForeignDataWrapper.is_postgres_only());
		assert!(ObjectType::ForeignServer.is_postgres_only());
		assert!(ObjectType::Language.is_postgres_only());
		assert!(ObjectType::LargeObject.is_postgres_only());
		assert!(ObjectType::Tablespace.is_postgres_only());
		assert!(ObjectType::Parameter.is_postgres_only());
	}
}

#[cfg(test)]
mod grantee_tests {
	use crate::*;

	#[test]
	fn test_grantee_role_constructor() {
		let grantee = Grantee::role("app_user");
		match grantee {
			Grantee::Role(name) => assert_eq!(name, "app_user"),
			_ => panic!("Expected Role variant"),
		}
	}

	#[test]
	fn test_grantee_user_constructor() {
		let grantee = Grantee::user("app_user", "localhost");
		match grantee {
			Grantee::User(user, host) => {
				assert_eq!(user, "app_user");
				assert_eq!(host, "localhost");
			}
			_ => panic!("Expected User variant"),
		}
	}

	#[test]
	fn test_grantee_is_postgres_only() {
		// Common grantees
		assert!(!Grantee::role("app_user").is_postgres_only());
		assert!(!Grantee::CurrentUser.is_postgres_only()); // Also supported in MySQL

		// PostgreSQL-specific grantees
		assert!(Grantee::Public.is_postgres_only());
		assert!(Grantee::CurrentRole.is_postgres_only());
		assert!(Grantee::SessionUser.is_postgres_only());
	}

	#[test]
	fn test_grantee_is_mysql_specific() {
		// MySQL-specific grantees
		assert!(Grantee::user("app_user", "localhost").is_mysql_specific());

		// Not MySQL-specific
		assert!(!Grantee::role("app_user").is_mysql_specific());
		assert!(!Grantee::Public.is_mysql_specific());
		assert!(!Grantee::CurrentUser.is_mysql_specific());
		assert!(!Grantee::CurrentRole.is_mysql_specific());
		assert!(!Grantee::SessionUser.is_mysql_specific());
	}
}

#[cfg(test)]
mod grant_statement_tests {
	use crate::*;

	#[test]
	fn test_grant_statement_new() {
		let stmt = GrantStatement::new();
		assert!(stmt.privileges.is_empty());
		assert!(stmt.objects.is_empty());
		assert!(stmt.grantees.is_empty());
		assert!(!stmt.with_grant_option);
		assert!(stmt.granted_by.is_none());
	}

	#[test]
	fn test_grant_statement_builder() {
		let stmt = GrantStatement::new()
			.privilege(Privilege::Select)
			.privilege(Privilege::Insert)
			.on_table("users")
			.to("app_user")
			.with_grant_option(true);

		assert_eq!(stmt.privileges.len(), 2);
		assert_eq!(stmt.object_type, ObjectType::Table);
		assert_eq!(stmt.objects.len(), 1);
		assert_eq!(stmt.grantees.len(), 1);
		assert!(stmt.with_grant_option);
	}

	#[test]
	fn test_grant_statement_privileges_method() {
		let stmt = GrantStatement::new()
			.privileges(vec![
				Privilege::Select,
				Privilege::Insert,
				Privilege::Update,
			])
			.on_table("users")
			.to("app_user");

		assert_eq!(stmt.privileges.len(), 3);
	}

	#[test]
	fn test_grant_statement_on_database() {
		let stmt = GrantStatement::new()
			.privilege(Privilege::Create)
			.on_database("mydb")
			.to("app_user");

		assert_eq!(stmt.object_type, ObjectType::Database);
		assert_eq!(stmt.objects.len(), 1);
	}

	#[test]
	fn test_grant_statement_on_schema() {
		let stmt = GrantStatement::new()
			.privilege(Privilege::Usage)
			.on_schema("public")
			.to("app_user");

		assert_eq!(stmt.object_type, ObjectType::Schema);
		assert_eq!(stmt.objects.len(), 1);
	}

	#[test]
	fn test_grant_statement_multiple_grantees() {
		let stmt = GrantStatement::new()
			.privilege(Privilege::Select)
			.on_table("users")
			.grantee(Grantee::role("app_user"))
			.grantee(Grantee::role("readonly_user"));

		assert_eq!(stmt.grantees.len(), 2);
	}

	#[test]
	fn test_grant_statement_granted_by() {
		let stmt = GrantStatement::new()
			.privilege(Privilege::Select)
			.on_table("users")
			.to("app_user")
			.granted_by(Grantee::role("admin"));

		assert!(stmt.granted_by.is_some());
	}

	#[test]
	fn test_grant_statement_validate_missing_privileges() {
		let stmt = GrantStatement::new().on_table("users").to("app_user");

		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_grant_statement_validate_missing_objects() {
		let stmt = GrantStatement::new()
			.privilege(Privilege::Select)
			.to("app_user");

		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_grant_statement_validate_missing_grantees() {
		let stmt = GrantStatement::new()
			.privilege(Privilege::Select)
			.on_table("users");

		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_grant_statement_validate_invalid_privilege_object() {
		let stmt = GrantStatement::new()
			.privilege(Privilege::Connect)  // Invalid for TABLE
			.on_table("users")
			.to("app_user");

		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_grant_statement_validate_valid() {
		let stmt = GrantStatement::new()
			.privilege(Privilege::Select)
			.on_table("users")
			.to("app_user");

		assert!(stmt.validate().is_ok());
	}
}

#[cfg(test)]
mod revoke_statement_tests {
	use crate::dcl::DropBehavior;
	use crate::*;

	#[test]
	fn test_revoke_statement_new() {
		let stmt = RevokeStatement::new();
		assert!(stmt.privileges.is_empty());
		assert!(stmt.objects.is_empty());
		assert!(stmt.grantees.is_empty());
		assert!(!stmt.cascade);
		assert!(!stmt.grant_option_for);
	}

	#[test]
	fn test_revoke_statement_builder() {
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Insert)
			.from_table("users")
			.from("app_user");

		assert_eq!(stmt.privileges.len(), 1);
		assert_eq!(stmt.object_type, ObjectType::Table);
		assert_eq!(stmt.objects.len(), 1);
		assert_eq!(stmt.grantees.len(), 1);
	}

	#[test]
	fn test_revoke_statement_cascade() {
		let stmt = RevokeStatement::new()
			.privilege(Privilege::All)
			.from_table("users")
			.from("app_user")
			.cascade(true);

		assert!(stmt.cascade);
	}

	#[test]
	fn test_revoke_statement_grant_option_for() {
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Select)
			.from_table("users")
			.from("app_user")
			.grant_option_for(true);

		assert!(stmt.grant_option_for);
	}

	#[test]
	fn test_revoke_statement_from_database() {
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Create)
			.from_database("mydb")
			.from("app_user");

		assert_eq!(stmt.object_type, ObjectType::Database);
		assert_eq!(stmt.objects.len(), 1);
	}

	#[test]
	fn test_revoke_statement_from_schema() {
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Usage)
			.from_schema("public")
			.from("app_user");

		assert_eq!(stmt.object_type, ObjectType::Schema);
		assert_eq!(stmt.objects.len(), 1);
	}

	#[test]
	fn test_revoke_statement_validate_missing_privileges() {
		let stmt = RevokeStatement::new().from_table("users").from("app_user");

		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_revoke_statement_validate_missing_objects() {
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Insert)
			.from("app_user");

		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_revoke_statement_validate_missing_grantees() {
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Insert)
			.from_table("users");

		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_revoke_statement_validate_invalid_privilege_object() {
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Connect)  // Invalid for TABLE
			.from_table("users")
			.from("app_user");

		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_revoke_statement_validate_valid() {
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Insert)
			.from_table("users")
			.from("app_user");

		assert!(stmt.validate().is_ok());
	}

	// ========================================
	// GrantRoleStatement Tests
	// ========================================

	#[test]
	fn test_grant_role_new() {
		let stmt = GrantRoleStatement::new();
		assert_eq!(stmt.roles.len(), 0);
		assert_eq!(stmt.grantees.len(), 0);
		assert_eq!(stmt.with_admin_option, false);
		assert!(stmt.granted_by.is_none());
	}

	#[test]
	fn test_grant_role_builder() {
		let stmt = GrantRoleStatement::new()
			.role("developer")
			.to(RoleSpecification::new("alice"));

		assert_eq!(stmt.roles, vec!["developer"]);
		assert_eq!(stmt.grantees.len(), 1);
	}

	#[test]
	fn test_grant_role_with_admin_option() {
		let stmt = GrantRoleStatement::new()
			.role("developer")
			.to(RoleSpecification::new("alice"))
			.with_admin_option();

		assert!(stmt.with_admin_option);
	}

	#[test]
	fn test_grant_role_granted_by() {
		let stmt = GrantRoleStatement::new()
			.role("developer")
			.to(RoleSpecification::new("alice"))
			.granted_by(RoleSpecification::current_user());

		assert!(stmt.granted_by.is_some());
	}

	#[test]
	fn test_grant_role_validate_missing_roles() {
		let stmt = GrantRoleStatement::new().to(RoleSpecification::new("alice"));

		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_grant_role_validate_empty_role() {
		let stmt = GrantRoleStatement::new()
			.role("")
			.to(RoleSpecification::new("alice"));

		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_grant_role_validate_missing_grantees() {
		let stmt = GrantRoleStatement::new().role("developer");

		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_grant_role_validate_valid() {
		let stmt = GrantRoleStatement::new()
			.role("developer")
			.to(RoleSpecification::new("alice"));

		assert!(stmt.validate().is_ok());
	}

	// ========================================
	// RevokeRoleStatement Tests
	// ========================================

	#[test]
	fn test_revoke_role_new() {
		let stmt = RevokeRoleStatement::new();
		assert_eq!(stmt.roles.len(), 0);
		assert_eq!(stmt.grantees.len(), 0);
		assert_eq!(stmt.admin_option_for, false);
		assert!(stmt.granted_by.is_none());
		assert!(stmt.drop_behavior.is_none());
	}

	#[test]
	fn test_revoke_role_builder() {
		let stmt = RevokeRoleStatement::new()
			.role("developer")
			.from(RoleSpecification::new("alice"));

		assert_eq!(stmt.roles, vec!["developer"]);
		assert_eq!(stmt.grantees.len(), 1);
	}

	#[test]
	fn test_revoke_role_admin_option_for() {
		let stmt = RevokeRoleStatement::new()
			.role("developer")
			.from(RoleSpecification::new("alice"))
			.admin_option_for();

		assert!(stmt.admin_option_for);
	}

	#[test]
	fn test_revoke_role_cascade() {
		let stmt = RevokeRoleStatement::new()
			.role("developer")
			.from(RoleSpecification::new("alice"))
			.cascade();

		assert_eq!(stmt.drop_behavior, Some(DropBehavior::Cascade));
	}

	#[test]
	fn test_revoke_role_restrict() {
		let stmt = RevokeRoleStatement::new()
			.role("developer")
			.from(RoleSpecification::new("alice"))
			.restrict();

		assert_eq!(stmt.drop_behavior, Some(DropBehavior::Restrict));
	}

	#[test]
	fn test_revoke_role_validate_missing_roles() {
		let stmt = RevokeRoleStatement::new().from(RoleSpecification::new("alice"));

		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_revoke_role_validate_missing_grantees() {
		let stmt = RevokeRoleStatement::new().role("developer");

		assert!(stmt.validate().is_err());
	}

	#[test]
	fn test_revoke_role_validate_valid() {
		let stmt = RevokeRoleStatement::new()
			.role("developer")
			.from(RoleSpecification::new("alice"));

		assert!(stmt.validate().is_ok());
	}

	// ========================================
	// PostgreSQL SQL Generation Tests
	// ========================================

	#[test]
	fn test_postgres_grant_role_basic() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantRoleStatement::new()
			.role("developer")
			.to(RoleSpecification::new("alice"));

		let (sql, values) = builder.build_grant_role(&stmt);
		assert_eq!(sql, r#"GRANT "developer" TO alice"#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_grant_role_multiple() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantRoleStatement::new()
			.roles(vec!["developer", "analyst"])
			.to(RoleSpecification::new("alice"))
			.to(RoleSpecification::new("bob"));

		let (sql, values) = builder.build_grant_role(&stmt);
		assert!(sql.contains(r#"GRANT "developer", "analyst""#));
		assert!(sql.contains("TO alice, bob"));
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_grant_role_with_admin() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantRoleStatement::new()
			.role("developer")
			.to(RoleSpecification::new("alice"))
			.with_admin_option();

		let (sql, values) = builder.build_grant_role(&stmt);
		assert!(sql.contains(r#"GRANT "developer" TO alice"#));
		assert!(sql.contains("WITH ADMIN OPTION"));
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_grant_role_granted_by() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantRoleStatement::new()
			.role("developer")
			.to(RoleSpecification::new("alice"))
			.granted_by(RoleSpecification::current_user());

		let (sql, values) = builder.build_grant_role(&stmt);
		assert!(sql.contains(r#"GRANT "developer" TO alice"#));
		assert!(sql.contains("GRANTED BY CURRENT_USER"));
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_grant_role_current_role() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantRoleStatement::new()
			.role("developer")
			.to(RoleSpecification::current_role());

		let (sql, values) = builder.build_grant_role(&stmt);
		assert!(sql.contains(r#"GRANT "developer" TO CURRENT_ROLE"#));
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_revoke_role_basic() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeRoleStatement::new()
			.role("developer")
			.from(RoleSpecification::new("alice"));

		let (sql, values) = builder.build_revoke_role(&stmt);
		assert_eq!(sql, r#"REVOKE "developer" FROM alice"#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_revoke_role_admin_option_for() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeRoleStatement::new()
			.role("developer")
			.from(RoleSpecification::new("alice"))
			.admin_option_for();

		let (sql, values) = builder.build_revoke_role(&stmt);
		assert!(sql.contains(r#"REVOKE ADMIN OPTION FOR "developer" FROM alice"#));
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_revoke_role_cascade() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeRoleStatement::new()
			.role("developer")
			.from(RoleSpecification::new("alice"))
			.cascade();

		let (sql, values) = builder.build_revoke_role(&stmt);
		assert!(sql.contains(r#"REVOKE "developer" FROM alice"#));
		assert!(sql.contains("CASCADE"));
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_revoke_role_restrict() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeRoleStatement::new()
			.role("developer")
			.from(RoleSpecification::new("alice"))
			.restrict();

		let (sql, values) = builder.build_revoke_role(&stmt);
		assert!(sql.contains(r#"REVOKE "developer" FROM alice"#));
		assert!(sql.contains("RESTRICT"));
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_revoke_role_granted_by() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeRoleStatement::new()
			.role("developer")
			.from(RoleSpecification::new("alice"))
			.granted_by(RoleSpecification::session_user());

		let (sql, values) = builder.build_revoke_role(&stmt);
		assert!(sql.contains(r#"REVOKE "developer" FROM alice"#));
		assert!(sql.contains("GRANTED BY SESSION_USER"));
		assert!(values.is_empty());
	}

	// Extended object types tests

	#[test]
	fn test_postgres_grant_on_function() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Execute)
			.on_function("calculate_total")
			.to("app_user");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(
			sql,
			r#"GRANT EXECUTE ON FUNCTION "calculate_total" TO "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_grant_on_procedure() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Execute)
			.on_procedure("process_order")
			.to("app_user");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(
			sql,
			r#"GRANT EXECUTE ON PROCEDURE "process_order" TO "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_grant_on_routine() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Execute)
			.on_routine("my_routine")
			.to("app_user");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(
			sql,
			r#"GRANT EXECUTE ON ROUTINE "my_routine" TO "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_grant_on_type() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Usage)
			.on_type("custom_type")
			.to("app_user");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(sql, r#"GRANT USAGE ON TYPE "custom_type" TO "app_user""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_grant_on_domain() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Usage)
			.on_domain("email_domain")
			.to("app_user");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(sql, r#"GRANT USAGE ON DOMAIN "email_domain" TO "app_user""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_grant_on_foreign_data_wrapper() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Usage)
			.on_foreign_data_wrapper("my_fdw")
			.to("app_user");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(
			sql,
			r#"GRANT USAGE ON FOREIGN DATA WRAPPER "my_fdw" TO "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_grant_on_foreign_server() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Usage)
			.on_foreign_server("my_server")
			.to("app_user");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(
			sql,
			r#"GRANT USAGE ON FOREIGN SERVER "my_server" TO "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_grant_on_language() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Usage)
			.on_language("plpgsql")
			.to("app_user");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(sql, r#"GRANT USAGE ON LANGUAGE "plpgsql" TO "app_user""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_grant_on_large_object() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Select)
			.on_large_object("12345")
			.to("app_user");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(sql, r#"GRANT SELECT ON LARGE OBJECT "12345" TO "app_user""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_grant_on_tablespace() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Create)
			.on_tablespace("fast_storage")
			.to("app_user");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(
			sql,
			r#"GRANT CREATE ON TABLESPACE "fast_storage" TO "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_grant_on_parameter() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = GrantStatement::new()
			.privilege(Privilege::Set)
			.on_parameter("work_mem")
			.to("app_user");

		let (sql, values) = builder.build_grant(&stmt);
		assert_eq!(sql, r#"GRANT SET ON PARAMETER "work_mem" TO "app_user""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_revoke_from_function() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Execute)
			.from_function("calculate_total")
			.from("app_user");

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(
			sql,
			r#"REVOKE EXECUTE ON FUNCTION "calculate_total" FROM "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_revoke_from_procedure() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Execute)
			.from_procedure("process_order")
			.from("app_user");

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(
			sql,
			r#"REVOKE EXECUTE ON PROCEDURE "process_order" FROM "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_revoke_from_routine() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Execute)
			.from_routine("my_routine")
			.from("app_user");

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(
			sql,
			r#"REVOKE EXECUTE ON ROUTINE "my_routine" FROM "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_revoke_from_type() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Usage)
			.from_type("custom_type")
			.from("app_user");

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(sql, r#"REVOKE USAGE ON TYPE "custom_type" FROM "app_user""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_revoke_from_domain() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Usage)
			.from_domain("email_domain")
			.from("app_user");

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(
			sql,
			r#"REVOKE USAGE ON DOMAIN "email_domain" FROM "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_revoke_from_foreign_data_wrapper() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Usage)
			.from_foreign_data_wrapper("my_fdw")
			.from("app_user");

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(
			sql,
			r#"REVOKE USAGE ON FOREIGN DATA WRAPPER "my_fdw" FROM "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_revoke_from_foreign_server() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Usage)
			.from_foreign_server("my_server")
			.from("app_user");

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(
			sql,
			r#"REVOKE USAGE ON FOREIGN SERVER "my_server" FROM "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_revoke_from_language() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Usage)
			.from_language("plpgsql")
			.from("app_user");

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(sql, r#"REVOKE USAGE ON LANGUAGE "plpgsql" FROM "app_user""#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_revoke_from_large_object() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Select)
			.from_large_object("12345")
			.from("app_user");

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(
			sql,
			r#"REVOKE SELECT ON LARGE OBJECT "12345" FROM "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_revoke_from_tablespace() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Create)
			.from_tablespace("fast_storage")
			.from("app_user");

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(
			sql,
			r#"REVOKE CREATE ON TABLESPACE "fast_storage" FROM "app_user""#
		);
		assert!(values.is_empty());
	}

	#[test]
	fn test_postgres_revoke_from_parameter() {
		use crate::backend::{PostgresQueryBuilder, QueryBuilder};

		let builder = PostgresQueryBuilder::new();
		let stmt = RevokeStatement::new()
			.privilege(Privilege::Set)
			.from_parameter("work_mem")
			.from("app_user");

		let (sql, values) = builder.build_revoke(&stmt);
		assert_eq!(sql, r#"REVOKE SET ON PARAMETER "work_mem" FROM "app_user""#);
		assert!(values.is_empty());
	}

	// ========================================
	// MySQL SQL Generation Tests
	// ========================================

	#[test]
	fn test_mysql_grant_role_basic() {
		use crate::backend::{MySqlQueryBuilder, QueryBuilder};

		let builder = MySqlQueryBuilder::new();
		let stmt = GrantRoleStatement::new()
			.role("developer")
			.to(RoleSpecification::new("alice"));

		let (sql, values) = builder.build_grant_role(&stmt);
		assert_eq!(sql, r#"GRANT `developer` TO alice"#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_mysql_grant_role_multiple() {
		use crate::backend::{MySqlQueryBuilder, QueryBuilder};

		let builder = MySqlQueryBuilder::new();
		let stmt = GrantRoleStatement::new()
			.roles(vec!["developer", "analyst"])
			.to(RoleSpecification::new("alice"))
			.to(RoleSpecification::new("bob"));

		let (sql, values) = builder.build_grant_role(&stmt);
		assert!(sql.contains(r#"GRANT `developer`, `analyst`"#));
		assert!(sql.contains("TO alice, bob"));
		assert!(values.is_empty());
	}

	#[test]
	fn test_mysql_grant_role_with_admin() {
		use crate::backend::{MySqlQueryBuilder, QueryBuilder};

		let builder = MySqlQueryBuilder::new();
		let stmt = GrantRoleStatement::new()
			.role("developer")
			.to(RoleSpecification::new("alice"))
			.with_admin_option();

		let (sql, values) = builder.build_grant_role(&stmt);
		assert!(sql.contains(r#"GRANT `developer` TO alice"#));
		assert!(sql.contains("WITH ADMIN OPTION"));
		assert!(values.is_empty());
	}

	#[test]
	fn test_mysql_revoke_role_basic() {
		use crate::backend::{MySqlQueryBuilder, QueryBuilder};

		let builder = MySqlQueryBuilder::new();
		let stmt = RevokeRoleStatement::new()
			.role("developer")
			.from(RoleSpecification::new("alice"));

		let (sql, values) = builder.build_revoke_role(&stmt);
		assert_eq!(sql, r#"REVOKE `developer` FROM alice"#);
		assert!(values.is_empty());
	}

	#[test]
	fn test_mysql_revoke_role_admin_option_for() {
		use crate::backend::{MySqlQueryBuilder, QueryBuilder};

		let builder = MySqlQueryBuilder::new();
		let stmt = RevokeRoleStatement::new()
			.role("developer")
			.from(RoleSpecification::new("alice"))
			.admin_option_for();

		let (sql, values) = builder.build_revoke_role(&stmt);
		assert!(sql.contains(r#"REVOKE ADMIN OPTION FOR `developer` FROM alice"#));
		assert!(values.is_empty());
	}

	#[test]
	fn test_mysql_grant_role_user_host() {
		use crate::backend::{MySqlQueryBuilder, QueryBuilder};

		let builder = MySqlQueryBuilder::new();
		let stmt = GrantRoleStatement::new()
			.role("developer")
			.to(RoleSpecification::new("'alice'@'localhost'"));

		let (sql, values) = builder.build_grant_role(&stmt);
		assert!(sql.contains(r#"GRANT `developer` TO 'alice'@'localhost'"#));
		assert!(values.is_empty());
	}

	#[test]
	fn test_mysql_revoke_role_user_host() {
		use crate::backend::{MySqlQueryBuilder, QueryBuilder};

		let builder = MySqlQueryBuilder::new();
		let stmt = RevokeRoleStatement::new()
			.role("developer")
			.from(RoleSpecification::new("'alice'@'localhost'"));

		let (sql, values) = builder.build_revoke_role(&stmt);
		assert!(sql.contains(r#"REVOKE `developer` FROM 'alice'@'localhost'"#));
		assert!(values.is_empty());
	}

	// ========================================
	// SQLite Error Tests
	// ========================================

	#[test]
	#[should_panic(expected = "SQLite does not support DCL (GRANT role)")]
	fn test_sqlite_grant_role_panics() {
		use crate::backend::{QueryBuilder, SqliteQueryBuilder};

		let builder = SqliteQueryBuilder::new();
		let stmt = GrantRoleStatement::new()
			.role("developer")
			.to(RoleSpecification::new("alice"));

		builder.build_grant_role(&stmt);
	}

	#[test]
	#[should_panic(expected = "SQLite does not support DCL (REVOKE role)")]
	fn test_sqlite_revoke_role_panics() {
		use crate::backend::{QueryBuilder, SqliteQueryBuilder};

		let builder = SqliteQueryBuilder::new();
		let stmt = RevokeRoleStatement::new()
			.role("developer")
			.from(RoleSpecification::new("alice"));

		builder.build_revoke_role(&stmt);
	}
}

#[cfg(test)]
mod role_attribute_tests {
	use crate::*;

	#[test]
	fn test_role_attribute_variants() {
		// Test basic privilege attributes
		let superuser = RoleAttribute::SuperUser;
		let no_superuser = RoleAttribute::NoSuperUser;
		let createdb = RoleAttribute::CreateDb;
		let no_createdb = RoleAttribute::NoCreateDb;

		// Verify they are different
		assert_ne!(superuser, no_superuser);
		assert_ne!(createdb, no_createdb);
	}

	#[test]
	fn test_role_attribute_connection_limit() {
		let limit = RoleAttribute::ConnectionLimit(10);
		assert_eq!(limit, RoleAttribute::ConnectionLimit(10));
		assert_ne!(limit, RoleAttribute::ConnectionLimit(5));
	}

	#[test]
	fn test_role_attribute_password() {
		let password = RoleAttribute::Password("secret".to_string());
		assert_eq!(password, RoleAttribute::Password("secret".to_string()));
	}

	#[test]
	fn test_role_attribute_in_role() {
		let in_role = RoleAttribute::InRole(vec!["role1".to_string(), "role2".to_string()]);
		assert_eq!(
			in_role,
			RoleAttribute::InRole(vec!["role1".to_string(), "role2".to_string()])
		);
	}

	#[test]
	fn test_role_attribute_clone() {
		let attr = RoleAttribute::SuperUser;
		let cloned = attr.clone();
		assert_eq!(attr, cloned);
	}

	#[test]
	fn test_role_attribute_debug() {
		let attr = RoleAttribute::SuperUser;
		let debug_str = format!("{:?}", attr);
		assert!(debug_str.contains("SuperUser"));
	}
}

#[cfg(test)]
mod user_option_tests {
	use crate::*;

	#[test]
	fn test_user_option_password() {
		let opt = UserOption::Password("secret".to_string());
		assert_eq!(opt, UserOption::Password("secret".to_string()));
	}

	#[test]
	fn test_user_option_auth_plugin() {
		let opt = UserOption::AuthPlugin {
			plugin: "mysql_native_password".to_string(),
			by: Some("password".to_string()),
			as_: None,
		};

		match opt {
			UserOption::AuthPlugin { plugin, by, as_ } => {
				assert_eq!(plugin, "mysql_native_password");
				assert_eq!(by, Some("password".to_string()));
				assert_eq!(as_, None);
			}
			_ => panic!("Expected AuthPlugin variant"),
		}
	}

	#[test]
	fn test_user_option_account_lock() {
		let lock = UserOption::AccountLock;
		let unlock = UserOption::AccountUnlock;
		assert_ne!(lock, unlock);
	}

	#[test]
	fn test_user_option_password_expire() {
		let expire = UserOption::PasswordExpire;
		let never = UserOption::PasswordExpireNever;
		let interval = UserOption::PasswordExpireInterval(90);

		assert_ne!(expire, never);
		assert_ne!(never, interval);
		assert_eq!(interval, UserOption::PasswordExpireInterval(90));
	}

	#[test]
	fn test_user_option_failed_login_attempts() {
		let opt = UserOption::FailedLoginAttempts(3);
		assert_eq!(opt, UserOption::FailedLoginAttempts(3));
		assert_ne!(opt, UserOption::FailedLoginAttempts(5));
	}

	#[test]
	fn test_user_option_comment() {
		let opt = UserOption::Comment("Application user".to_string());
		assert_eq!(opt, UserOption::Comment("Application user".to_string()));
	}

	#[test]
	fn test_user_option_clone() {
		let opt = UserOption::AccountLock;
		let cloned = opt.clone();
		assert_eq!(opt, cloned);
	}

	#[test]
	fn test_user_option_debug() {
		let opt = UserOption::AccountLock;
		let debug_str = format!("{:?}", opt);
		assert!(debug_str.contains("AccountLock"));
	}
}

#[cfg(test)]
mod create_role_statement_tests {
	use crate::*;

	#[test]
	fn test_create_role_new() {
		let stmt = CreateRoleStatement::new();
		assert!(stmt.role_name.is_empty());
		assert!(!stmt.if_not_exists);
		assert!(stmt.attributes.is_empty());
		assert!(stmt.options.is_empty());
	}

	#[test]
	fn test_create_role_builder() {
		let stmt = CreateRoleStatement::new()
			.role("developer")
			.attribute(RoleAttribute::Login)
			.attribute(RoleAttribute::CreateDb);

		assert_eq!(stmt.role_name, "developer");
		assert_eq!(stmt.attributes.len(), 2);
	}

	#[test]
	fn test_create_role_if_not_exists() {
		let stmt = CreateRoleStatement::new()
			.role("app_role")
			.if_not_exists(true);

		assert!(stmt.if_not_exists);
	}

	#[test]
	fn test_create_role_validate_success() {
		let stmt = CreateRoleStatement::new().role("developer");
		assert!(stmt.validate().is_ok());
	}

	#[test]
	fn test_create_role_validate_empty_name() {
		let stmt = CreateRoleStatement::new();
		assert!(stmt.validate().is_err());
		assert_eq!(
			stmt.validate().unwrap_err(),
			"Role name cannot be empty or whitespace only"
		);
	}

	#[test]
	fn test_create_role_with_attributes() {
		let stmt = CreateRoleStatement::new().role("app_user").attributes(vec![
			RoleAttribute::Login,
			RoleAttribute::CreateDb,
			RoleAttribute::ConnectionLimit(10),
		]);

		assert_eq!(stmt.attributes.len(), 3);
	}

	#[test]
	fn test_create_role_with_options() {
		let stmt = CreateRoleStatement::new().role("app_role").options(vec![
			UserOption::Comment("App role".to_string()),
			UserOption::AccountLock,
		]);

		assert_eq!(stmt.options.len(), 2);
	}
}
