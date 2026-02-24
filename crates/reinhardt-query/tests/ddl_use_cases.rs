//! Real-world use case integration tests
//!
//! Tests that simulate realistic database schema scenarios:
//! - E-commerce schema (users, orders, products)
//! - Multi-tenant setup (schema per tenant)
//! - Blog with cascading deletes
//! - Inventory with check constraints
//! - User authentication schema

use std::sync::Arc;

use rstest::rstest;

use reinhardt_query::prelude::*;
use reinhardt_query::types::{
	ColumnDef, ColumnType, ForeignKeyAction, IntoIden, IntoTableRef, TableConstraint,
};

mod common;
use common::{PgContainer, postgres_ddl, unique_schema_name, unique_table_name};

// =============================================================================
// UC-01: E-commerce Schema
// =============================================================================

/// UC-01: Test complete e-commerce schema creation
/// Creates users, products, orders, order_items tables with proper relationships
#[rstest]
#[tokio::test]
async fn test_ecommerce_schema(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let builder = PostgresQueryBuilder::new();

	// Generate unique table names
	let users_table = unique_table_name("users");
	let products_table = unique_table_name("products");
	let orders_table = unique_table_name("orders");
	let order_items_table = unique_table_name("order_items");

	// Create users table
	let mut users_stmt = Query::create_table();
	users_stmt
		.table(users_table.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true)
				.auto_increment(true),
		)
		.col(
			ColumnDef::new("email")
				.column_type(ColumnType::String(Some(255)))
				.not_null(true),
		)
		.col(
			ColumnDef::new("name")
				.column_type(ColumnType::String(Some(100)))
				.not_null(true),
		)
		.col(
			ColumnDef::new("created_at")
				.column_type(ColumnType::TimestampWithTimeZone)
				.not_null(true),
		);

	let (sql, _values) = builder.build_create_table(&users_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create users table");

	// Create products table
	let mut products_stmt = Query::create_table();
	products_stmt
		.table(products_table.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true)
				.auto_increment(true),
		)
		.col(
			ColumnDef::new("name")
				.column_type(ColumnType::String(Some(255)))
				.not_null(true),
		)
		.col(
			ColumnDef::new("price")
				.column_type(ColumnType::Decimal(Some((10, 2))))
				.not_null(true)
				.check(Expr::col("price").gte(0)),
		)
		.col(
			ColumnDef::new("stock")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.check(Expr::col("stock").gte(0)),
		);

	let (sql, _values) = builder.build_create_table(&products_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create products table");

	// Create orders table
	let mut orders_stmt = Query::create_table();
	orders_stmt
		.table(orders_table.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true)
				.auto_increment(true),
		)
		.col(
			ColumnDef::new("user_id")
				.column_type(ColumnType::Integer)
				.not_null(true),
		)
		.col(
			ColumnDef::new("total")
				.column_type(ColumnType::Decimal(Some((12, 2))))
				.not_null(true),
		)
		.col(
			ColumnDef::new("status")
				.column_type(ColumnType::String(Some(20)))
				.not_null(true),
		)
		.col(
			ColumnDef::new("created_at")
				.column_type(ColumnType::TimestampWithTimeZone)
				.not_null(true),
		)
		.constraint(TableConstraint::ForeignKey {
			name: None,
			columns: vec!["user_id".to_string().into_iden()],
			ref_table: Box::new(users_table.clone().into_table_ref()),
			ref_columns: vec!["id".to_string().into_iden()],
			on_delete: Some(ForeignKeyAction::Cascade),
			on_update: None,
		});

	let (sql, _values) = builder.build_create_table(&orders_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create orders table");

	// Create order_items table
	let mut order_items_stmt = Query::create_table();
	order_items_stmt
		.table(order_items_table.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true)
				.auto_increment(true),
		)
		.col(
			ColumnDef::new("order_id")
				.column_type(ColumnType::Integer)
				.not_null(true),
		)
		.col(
			ColumnDef::new("product_id")
				.column_type(ColumnType::Integer)
				.not_null(true),
		)
		.col(
			ColumnDef::new("quantity")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.check(Expr::col("quantity").gt(0)),
		)
		.col(
			ColumnDef::new("unit_price")
				.column_type(ColumnType::Decimal(Some((10, 2))))
				.not_null(true),
		)
		.constraint(TableConstraint::ForeignKey {
			name: None,
			columns: vec!["order_id".to_string().into_iden()],
			ref_table: Box::new(orders_table.clone().into_table_ref()),
			ref_columns: vec!["id".to_string().into_iden()],
			on_delete: Some(ForeignKeyAction::Cascade),
			on_update: None,
		})
		.constraint(TableConstraint::ForeignKey {
			name: None,
			columns: vec!["product_id".to_string().into_iden()],
			ref_table: Box::new(products_table.clone().into_table_ref()),
			ref_columns: vec!["id".to_string().into_iden()],
			on_delete: Some(ForeignKeyAction::Restrict),
			on_update: None,
		});

	let (sql, _values) = builder.build_create_table(&order_items_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create order_items table");

	// Verify all tables exist
	for table in [
		&users_table,
		&products_table,
		&orders_table,
		&order_items_table,
	] {
		let exists: bool = sqlx::query_scalar(
			"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
		)
		.bind(table)
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
		assert!(exists, "Table {} should exist", table);
	}

	// Cleanup (reverse order due to FKs)
	for table in [
		&order_items_table,
		&orders_table,
		&products_table,
		&users_table,
	] {
		sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table))
			.execute(pool.as_ref())
			.await
			.unwrap();
	}
}

// =============================================================================
// UC-03: Multi-tenant Setup
// =============================================================================

/// UC-03: Test multi-tenant setup with schema per tenant
#[rstest]
#[tokio::test]
async fn test_multitenant_schema(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let builder = PostgresQueryBuilder::new();

	// Create schemas for multiple tenants
	let tenant_schemas = vec![
		unique_schema_name("tenant_a"),
		unique_schema_name("tenant_b"),
	];

	for schema_name in &tenant_schemas {
		// Create schema
		let mut create_schema = Query::create_schema();
		create_schema.name(schema_name.clone()).if_not_exists();

		let (sql, _values) = builder.build_create_schema(&create_schema);
		sqlx::query(&sql)
			.execute(pool.as_ref())
			.await
			.expect("Failed to create schema");

		// Create tenant-specific users table in each schema
		let table_sql = format!(
			r#"CREATE TABLE "{}"."users" (
                id SERIAL PRIMARY KEY,
                name VARCHAR(100) NOT NULL,
                email VARCHAR(255) NOT NULL
            )"#,
			schema_name
		);
		sqlx::query(&table_sql)
			.execute(pool.as_ref())
			.await
			.expect("Failed to create users table in schema");

		// Verify table exists in schema
		let exists: bool = sqlx::query_scalar(
			"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_schema = $1 AND table_name = 'users')",
		)
		.bind(schema_name)
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
		assert!(exists, "Users table should exist in schema {}", schema_name);
	}

	// Cleanup
	for schema_name in &tenant_schemas {
		sqlx::query(&format!(
			r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
			schema_name
		))
		.execute(pool.as_ref())
		.await
		.unwrap();
	}
}

// =============================================================================
// UC-05: User Auth Schema
// =============================================================================

/// UC-05: Test user authentication schema with roles and permissions
#[rstest]
#[tokio::test]
async fn test_user_auth_schema(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let builder = PostgresQueryBuilder::new();

	let users_table = unique_table_name("auth_users");
	let roles_table = unique_table_name("auth_roles");
	let permissions_table = unique_table_name("auth_permissions");
	let user_roles_table = unique_table_name("auth_user_roles");
	let role_permissions_table = unique_table_name("auth_role_permissions");

	// Create users table
	let mut users_stmt = Query::create_table();
	users_stmt
		.table(users_table.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true)
				.auto_increment(true),
		)
		.col(
			ColumnDef::new("username")
				.column_type(ColumnType::String(Some(50)))
				.not_null(true),
		)
		.col(
			ColumnDef::new("password_hash")
				.column_type(ColumnType::String(Some(255)))
				.not_null(true),
		)
		.col(
			ColumnDef::new("is_active")
				.column_type(ColumnType::Boolean)
				.not_null(true),
		)
		.constraint(TableConstraint::Unique {
			name: None,
			columns: vec!["username".to_string().into_iden()],
		});

	let (sql, _values) = builder.build_create_table(&users_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create users table");

	// Create roles table
	let mut roles_stmt = Query::create_table();
	roles_stmt
		.table(roles_table.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true)
				.auto_increment(true),
		)
		.col(
			ColumnDef::new("name")
				.column_type(ColumnType::String(Some(50)))
				.not_null(true),
		)
		.constraint(TableConstraint::Unique {
			name: None,
			columns: vec!["name".to_string().into_iden()],
		});

	let (sql, _values) = builder.build_create_table(&roles_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create roles table");

	// Create permissions table
	let mut permissions_stmt = Query::create_table();
	permissions_stmt
		.table(permissions_table.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true)
				.auto_increment(true),
		)
		.col(
			ColumnDef::new("name")
				.column_type(ColumnType::String(Some(100)))
				.not_null(true),
		)
		.constraint(TableConstraint::Unique {
			name: None,
			columns: vec!["name".to_string().into_iden()],
		});

	let (sql, _values) = builder.build_create_table(&permissions_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create permissions table");

	// Create user_roles junction table (M2M)
	let mut user_roles_stmt = Query::create_table();
	user_roles_stmt
		.table(user_roles_table.clone())
		.col(
			ColumnDef::new("user_id")
				.column_type(ColumnType::Integer)
				.not_null(true),
		)
		.col(
			ColumnDef::new("role_id")
				.column_type(ColumnType::Integer)
				.not_null(true),
		)
		.constraint(TableConstraint::PrimaryKey {
			name: None,
			columns: vec![
				"user_id".to_string().into_iden(),
				"role_id".to_string().into_iden(),
			],
		})
		.constraint(TableConstraint::ForeignKey {
			name: None,
			columns: vec!["user_id".to_string().into_iden()],
			ref_table: Box::new(users_table.clone().into_table_ref()),
			ref_columns: vec!["id".to_string().into_iden()],
			on_delete: Some(ForeignKeyAction::Cascade),
			on_update: None,
		})
		.constraint(TableConstraint::ForeignKey {
			name: None,
			columns: vec!["role_id".to_string().into_iden()],
			ref_table: Box::new(roles_table.clone().into_table_ref()),
			ref_columns: vec!["id".to_string().into_iden()],
			on_delete: Some(ForeignKeyAction::Cascade),
			on_update: None,
		});

	let (sql, _values) = builder.build_create_table(&user_roles_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create user_roles table");

	// Create role_permissions junction table (M2M)
	let mut role_permissions_stmt = Query::create_table();
	role_permissions_stmt
		.table(role_permissions_table.clone())
		.col(
			ColumnDef::new("role_id")
				.column_type(ColumnType::Integer)
				.not_null(true),
		)
		.col(
			ColumnDef::new("permission_id")
				.column_type(ColumnType::Integer)
				.not_null(true),
		)
		.constraint(TableConstraint::PrimaryKey {
			name: None,
			columns: vec![
				"role_id".to_string().into_iden(),
				"permission_id".to_string().into_iden(),
			],
		})
		.constraint(TableConstraint::ForeignKey {
			name: None,
			columns: vec!["role_id".to_string().into_iden()],
			ref_table: Box::new(roles_table.clone().into_table_ref()),
			ref_columns: vec!["id".to_string().into_iden()],
			on_delete: Some(ForeignKeyAction::Cascade),
			on_update: None,
		})
		.constraint(TableConstraint::ForeignKey {
			name: None,
			columns: vec!["permission_id".to_string().into_iden()],
			ref_table: Box::new(permissions_table.clone().into_table_ref()),
			ref_columns: vec!["id".to_string().into_iden()],
			on_delete: Some(ForeignKeyAction::Cascade),
			on_update: None,
		});

	let (sql, _values) = builder.build_create_table(&role_permissions_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create role_permissions table");

	// Verify all tables exist
	for table in [
		&users_table,
		&roles_table,
		&permissions_table,
		&user_roles_table,
		&role_permissions_table,
	] {
		let exists: bool = sqlx::query_scalar(
			"SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_name = $1)",
		)
		.bind(table)
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
		assert!(exists, "Table {} should exist", table);
	}

	// Cleanup (reverse order due to FKs)
	for table in [
		&role_permissions_table,
		&user_roles_table,
		&permissions_table,
		&roles_table,
		&users_table,
	] {
		sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table))
			.execute(pool.as_ref())
			.await
			.unwrap();
	}
}

// =============================================================================
// UC-06: Blog with Cascading Deletes
// =============================================================================

/// UC-06: Test blog schema with cascading deletes
#[rstest]
#[tokio::test]
async fn test_blog_cascade_deletes(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let builder = PostgresQueryBuilder::new();

	let authors_table = unique_table_name("blog_authors");
	let posts_table = unique_table_name("blog_posts");
	let comments_table = unique_table_name("blog_comments");

	// Create authors table
	let mut authors_stmt = Query::create_table();
	authors_stmt
		.table(authors_table.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true)
				.auto_increment(true),
		)
		.col(
			ColumnDef::new("name")
				.column_type(ColumnType::String(Some(100)))
				.not_null(true),
		);

	let (sql, _values) = builder.build_create_table(&authors_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create authors table");

	// Create posts table
	let mut posts_stmt = Query::create_table();
	posts_stmt
		.table(posts_table.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true)
				.auto_increment(true),
		)
		.col(
			ColumnDef::new("author_id")
				.column_type(ColumnType::Integer)
				.not_null(true),
		)
		.col(
			ColumnDef::new("title")
				.column_type(ColumnType::String(Some(255)))
				.not_null(true),
		)
		.col(
			ColumnDef::new("content")
				.column_type(ColumnType::Text)
				.not_null(true),
		)
		.constraint(TableConstraint::ForeignKey {
			name: None,
			columns: vec!["author_id".to_string().into_iden()],
			ref_table: Box::new(authors_table.clone().into_table_ref()),
			ref_columns: vec!["id".to_string().into_iden()],
			on_delete: Some(ForeignKeyAction::Cascade),
			on_update: None,
		});

	let (sql, _values) = builder.build_create_table(&posts_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create posts table");

	// Create comments table
	let mut comments_stmt = Query::create_table();
	comments_stmt
		.table(comments_table.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true)
				.auto_increment(true),
		)
		.col(
			ColumnDef::new("post_id")
				.column_type(ColumnType::Integer)
				.not_null(true),
		)
		.col(
			ColumnDef::new("body")
				.column_type(ColumnType::Text)
				.not_null(true),
		)
		.constraint(TableConstraint::ForeignKey {
			name: None,
			columns: vec!["post_id".to_string().into_iden()],
			ref_table: Box::new(posts_table.clone().into_table_ref()),
			ref_columns: vec!["id".to_string().into_iden()],
			on_delete: Some(ForeignKeyAction::Cascade),
			on_update: None,
		});

	let (sql, _values) = builder.build_create_table(&comments_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create comments table");

	// Insert test data
	sqlx::query(&format!(
		r#"INSERT INTO "{}" (name) VALUES ('Author1')"#,
		authors_table
	))
	.execute(pool.as_ref())
	.await
	.unwrap();

	sqlx::query(&format!(
		r#"INSERT INTO "{}" (author_id, title, content) VALUES (1, 'Post1', 'Content1')"#,
		posts_table
	))
	.execute(pool.as_ref())
	.await
	.unwrap();

	sqlx::query(&format!(
		r#"INSERT INTO "{}" (post_id, body) VALUES (1, 'Comment1')"#,
		comments_table
	))
	.execute(pool.as_ref())
	.await
	.unwrap();

	// Verify data exists
	let comment_count: i64 =
		sqlx::query_scalar(&format!(r#"SELECT COUNT(*) FROM "{}""#, comments_table))
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(comment_count, 1, "Comment should exist");

	// Delete author (should cascade to posts and comments)
	sqlx::query(&format!(r#"DELETE FROM "{}" WHERE id = 1"#, authors_table))
		.execute(pool.as_ref())
		.await
		.unwrap();

	// Verify cascade delete worked
	let comment_count: i64 =
		sqlx::query_scalar(&format!(r#"SELECT COUNT(*) FROM "{}""#, comments_table))
			.fetch_one(pool.as_ref())
			.await
			.unwrap();
	assert_eq!(comment_count, 0, "Comment should be deleted via cascade");

	let post_count: i64 = sqlx::query_scalar(&format!(r#"SELECT COUNT(*) FROM "{}""#, posts_table))
		.fetch_one(pool.as_ref())
		.await
		.unwrap();
	assert_eq!(post_count, 0, "Post should be deleted via cascade");

	// Cleanup
	for table in [&comments_table, &posts_table, &authors_table] {
		sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table))
			.execute(pool.as_ref())
			.await
			.unwrap();
	}
}

// =============================================================================
// UC-07: Inventory with Check Constraints
// =============================================================================

/// UC-07: Test inventory schema with check constraints
#[rstest]
#[tokio::test]
async fn test_inventory_check_constraints(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let builder = PostgresQueryBuilder::new();

	let inventory_table = unique_table_name("inventory");

	// Create inventory table with check constraints
	let mut stmt = Query::create_table();
	stmt.table(inventory_table.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true)
				.auto_increment(true),
		)
		.col(
			ColumnDef::new("product_name")
				.column_type(ColumnType::String(Some(255)))
				.not_null(true),
		)
		.col(
			ColumnDef::new("quantity")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.check(Expr::col("quantity").gte(0)),
		)
		.col(
			ColumnDef::new("min_quantity")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.check(Expr::col("min_quantity").gte(0)),
		)
		.col(
			ColumnDef::new("max_quantity")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.check(Expr::col("max_quantity").gt(0)),
		)
		.col(
			ColumnDef::new("unit_price")
				.column_type(ColumnType::Decimal(Some((10, 2))))
				.not_null(true)
				.check(Expr::col("unit_price").gte(0)),
		);

	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create inventory table");

	// Test valid insert
	let valid_insert = sqlx::query(&format!(
		r#"INSERT INTO "{}" (product_name, quantity, min_quantity, max_quantity, unit_price) VALUES ('Widget', 100, 10, 1000, 9.99)"#,
		inventory_table
	))
	.execute(pool.as_ref())
	.await;
	assert!(valid_insert.is_ok(), "Valid insert should succeed");

	// Test invalid insert (negative quantity)
	let invalid_insert = sqlx::query(&format!(
		r#"INSERT INTO "{}" (product_name, quantity, min_quantity, max_quantity, unit_price) VALUES ('Widget', -1, 10, 1000, 9.99)"#,
		inventory_table
	))
	.execute(pool.as_ref())
	.await;
	assert!(
		invalid_insert.is_err(),
		"Insert with negative quantity should fail"
	);

	// Test invalid insert (negative price)
	let invalid_insert = sqlx::query(&format!(
		r#"INSERT INTO "{}" (product_name, quantity, min_quantity, max_quantity, unit_price) VALUES ('Widget', 100, 10, 1000, -1.00)"#,
		inventory_table
	))
	.execute(pool.as_ref())
	.await;
	assert!(
		invalid_insert.is_err(),
		"Insert with negative price should fail"
	);

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, inventory_table))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// UC-09: Payment with Precise Decimals
// =============================================================================

/// UC-09: Test payment schema with precise decimal handling
#[rstest]
#[tokio::test]
async fn test_payment_precise_decimals(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let builder = PostgresQueryBuilder::new();

	let payments_table = unique_table_name("payments");

	// Create payments table with DECIMAL(19,4) for money precision
	let mut stmt = Query::create_table();
	stmt.table(payments_table.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true)
				.auto_increment(true),
		)
		.col(
			ColumnDef::new("amount")
				.column_type(ColumnType::Decimal(Some((19, 4))))
				.not_null(true),
		)
		.col(
			ColumnDef::new("currency")
				.column_type(ColumnType::Char(Some(3)))
				.not_null(true),
		)
		.col(
			ColumnDef::new("exchange_rate")
				.column_type(ColumnType::Decimal(Some((12, 6))))
				.not_null(false),
		)
		.col(
			ColumnDef::new("created_at")
				.column_type(ColumnType::TimestampWithTimeZone)
				.not_null(true),
		);

	let (sql, _values) = builder.build_create_table(&stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create payments table");

	// Insert payment with precise decimal
	sqlx::query(&format!(
		r#"INSERT INTO "{}" (amount, currency, exchange_rate, created_at) VALUES (12345.6789, 'USD', 1.123456, NOW())"#,
		payments_table
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to insert payment");

	// Verify precision is maintained using text cast
	let (amount, exchange_rate): (String, String) = sqlx::query_as(&format!(
		r#"SELECT amount::text, exchange_rate::text FROM "{}" WHERE id = 1"#,
		payments_table
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();

	// Verify decimal precision
	assert_eq!(
		amount, "12345.6789",
		"Amount precision should be maintained"
	);
	assert_eq!(
		exchange_rate, "1.123456",
		"Exchange rate precision should be maintained"
	);

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, payments_table))
		.execute(pool.as_ref())
		.await
		.unwrap();
}

// =============================================================================
// UC-04: Migration Pattern - Add Nullable Column, Backfill, Set NOT NULL
// =============================================================================

/// UC-04: Test safe column addition migration pattern
#[rstest]
#[tokio::test]
async fn test_migration_add_column_safely(
	#[future] postgres_ddl: (PgContainer, Arc<sqlx::PgPool>, u16, String),
) {
	let (_container, pool, _port, _url) = postgres_ddl.await;
	let builder = PostgresQueryBuilder::new();

	let table_name = unique_table_name("migrating_table");

	// Step 1: Create initial table
	let mut create_stmt = Query::create_table();
	create_stmt
		.table(table_name.clone())
		.col(
			ColumnDef::new("id")
				.column_type(ColumnType::Integer)
				.not_null(true)
				.primary_key(true)
				.auto_increment(true),
		)
		.col(
			ColumnDef::new("name")
				.column_type(ColumnType::String(Some(100)))
				.not_null(true),
		);

	let (sql, _values) = builder.build_create_table(&create_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create table");

	// Insert existing data
	sqlx::query(&format!(
		r#"INSERT INTO "{}" (name) VALUES ('Item1'), ('Item2'), ('Item3')"#,
		table_name
	))
	.execute(pool.as_ref())
	.await
	.unwrap();

	// Step 2: Add nullable column
	let mut alter_stmt = Query::alter_table();
	alter_stmt
		.table(table_name.clone())
		.add_column(ColumnDef::new("new_field").column_type(ColumnType::String(Some(50))));

	let (sql, _values) = builder.build_alter_table(&alter_stmt);
	sqlx::query(&sql)
		.execute(pool.as_ref())
		.await
		.expect("Failed to add column");

	// Step 3: Backfill data
	sqlx::query(&format!(
		r#"UPDATE "{}" SET new_field = 'default_value' WHERE new_field IS NULL"#,
		table_name
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to backfill");

	// Step 4: Set NOT NULL
	sqlx::query(&format!(
		r#"ALTER TABLE "{}" ALTER COLUMN new_field SET NOT NULL"#,
		table_name
	))
	.execute(pool.as_ref())
	.await
	.expect("Failed to set NOT NULL");

	// Verify column is NOT NULL
	let is_nullable: String = sqlx::query_scalar(
		"SELECT is_nullable FROM information_schema.columns WHERE table_name = $1 AND column_name = 'new_field'",
	)
	.bind(&table_name)
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(is_nullable, "NO", "Column should be NOT NULL");

	// Verify all rows have the new field populated
	let null_count: i64 = sqlx::query_scalar(&format!(
		r#"SELECT COUNT(*) FROM "{}" WHERE new_field IS NULL"#,
		table_name
	))
	.fetch_one(pool.as_ref())
	.await
	.unwrap();
	assert_eq!(null_count, 0, "No rows should have NULL new_field");

	// Cleanup
	sqlx::query(&format!(r#"DROP TABLE IF EXISTS "{}""#, table_name))
		.execute(pool.as_ref())
		.await
		.unwrap();
}
