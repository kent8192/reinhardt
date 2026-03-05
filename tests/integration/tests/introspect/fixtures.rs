//! Specialized fixtures for introspect integration tests
//!
//! These fixtures wrap the base testcontainers fixtures and add
//! introspect-specific schema setup for testing database introspection.
//!
//! **Features:**
//! - PostgreSQL with test schema (users, posts, comments)
//! - MySQL with test schema
//! - Empty database for edge case testing
//! - Database with all supported types

use reinhardt_test::fixtures::{ContainerAsync, GenericImage, postgres_container};
use rstest::*;
use sqlx::PgPool;
use std::sync::Arc;

/// Schema creation SQL for introspect tests
/// Split into individual statements for PostgreSQL prepared statement compatibility
const INTROSPECT_STATEMENTS: &[&str] = &[
	// Users table with various column types
	r#"CREATE TABLE users (
    id BIGSERIAL PRIMARY KEY,
    username VARCHAR(150) NOT NULL UNIQUE,
    email VARCHAR(254) NOT NULL UNIQUE,
    password_hash VARCHAR(128) NOT NULL,
    first_name VARCHAR(50),
    last_name VARCHAR(50),
    is_active BOOLEAN NOT NULL DEFAULT true,
    is_staff BOOLEAN NOT NULL DEFAULT false,
    date_joined TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_login TIMESTAMPTZ
)"#,
	// Create index on email
	r#"CREATE INDEX idx_users_email ON users(email)"#,
	// Posts table with foreign key
	r#"CREATE TABLE posts (
    id BIGSERIAL PRIMARY KEY,
    title VARCHAR(200) NOT NULL,
    slug VARCHAR(200) NOT NULL UNIQUE,
    content TEXT,
    author_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    view_count INTEGER NOT NULL DEFAULT 0
)"#,
	// Create composite index
	r#"CREATE INDEX idx_posts_author_created ON posts(author_id, created_at)"#,
	// Comments table with self-reference
	r#"CREATE TABLE comments (
    id BIGSERIAL PRIMARY KEY,
    content TEXT NOT NULL,
    post_id BIGINT NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    author_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    parent_id BIGINT REFERENCES comments(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
)"#,
	// Tags table for many-to-many
	r#"CREATE TABLE tags (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(50) NOT NULL UNIQUE,
    slug VARCHAR(50) NOT NULL UNIQUE
)"#,
	// Junction table for posts-tags many-to-many
	r#"CREATE TABLE posts_tags (
    post_id BIGINT NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    tag_id BIGINT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (post_id, tag_id)
)"#,
];

/// PostgreSQL with all PostgreSQL-specific types
const CREATE_ALL_TYPES_SCHEMA_SQL: &str = r#"
-- Table with all PostgreSQL types for type mapping tests
CREATE TABLE type_showcase (
    id BIGSERIAL PRIMARY KEY,

    -- Integer types
    int_small SMALLINT,
    int_regular INTEGER,
    int_big BIGINT,

    -- Floating point types
    float_real REAL,
    float_double DOUBLE PRECISION,
    decimal_money DECIMAL(10, 2),

    -- String types
    char_fixed CHAR(10),
    varchar_var VARCHAR(255),
    text_unlimited TEXT,

    -- Boolean
    is_active BOOLEAN,

    -- Date/time types
    date_only DATE,
    time_only TIME,
    timestamp_local TIMESTAMP,
    timestamp_tz TIMESTAMPTZ,

    -- Binary
    binary_data BYTEA,

    -- JSON types
    json_data JSON,
    jsonb_data JSONB,

    -- UUID
    uuid_col UUID,

    -- PostgreSQL arrays
    int_array INTEGER[],
    text_array TEXT[],

    -- PostgreSQL specific
    hstore_data HSTORE
);
"#;

/// Fixture: PostgreSQL container with introspect test schema
///
/// This fixture starts a PostgreSQL container and creates the test schema
/// for introspection testing.
///
/// # Returns
/// Tuple of (container, pool, database_url)
#[fixture]
pub async fn postgres_introspect_schema(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) -> (ContainerAsync<GenericImage>, Arc<PgPool>, String) {
	let (container, pool, _port, url) = postgres_container.await;

	// Create the test schema - execute each statement separately
	// for PostgreSQL prepared statement compatibility
	for statement in INTROSPECT_STATEMENTS {
		sqlx::query(statement)
			.execute(pool.as_ref())
			.await
			.expect("Failed to create introspect test schema");
	}

	(container, pool, url)
}

/// Fixture: PostgreSQL container with all types for type mapping tests
///
/// Creates a PostgreSQL database with a table containing all supported types
/// for comprehensive type mapping verification.
#[fixture]
pub async fn postgres_all_types_schema(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) -> (ContainerAsync<GenericImage>, Arc<PgPool>, String) {
	let (container, pool, _port, url) = postgres_container.await;

	// Enable hstore extension
	sqlx::query("CREATE EXTENSION IF NOT EXISTS hstore")
		.execute(pool.as_ref())
		.await
		.expect("Failed to create hstore extension");

	// Create the all-types schema
	sqlx::query(CREATE_ALL_TYPES_SCHEMA_SQL)
		.execute(pool.as_ref())
		.await
		.expect("Failed to create all-types test schema");

	(container, pool, url)
}

/// Fixture: Empty PostgreSQL database for edge case testing
///
/// Provides a fresh PostgreSQL database with no tables for testing
/// introspection behavior on empty databases.
#[fixture]
pub async fn empty_postgres_database(
	#[future] postgres_container: (ContainerAsync<GenericImage>, Arc<PgPool>, u16, String),
) -> (ContainerAsync<GenericImage>, Arc<PgPool>, String) {
	let (container, pool, _port, url) = postgres_container.await;
	// No schema creation - return empty database
	(container, pool, url)
}

/// Test helper: Verify table exists in database
pub(crate) async fn table_exists(pool: &PgPool, table_name: &str) -> bool {
	let result = sqlx::query_scalar::<_, bool>(
		r#"
        SELECT EXISTS (
            SELECT FROM information_schema.tables
            WHERE table_schema = 'public'
            AND table_name = $1
        )
        "#,
	)
	.bind(table_name)
	.fetch_one(pool)
	.await
	.expect("Failed to check table existence");

	result
}

/// Test helper: Get column count for a table
pub(crate) async fn get_column_count(pool: &PgPool, table_name: &str) -> i64 {
	let result = sqlx::query_scalar::<_, i64>(
		r#"
        SELECT COUNT(*)
        FROM information_schema.columns
        WHERE table_schema = 'public'
        AND table_name = $1
        "#,
	)
	.bind(table_name)
	.fetch_one(pool)
	.await
	.expect("Failed to count columns");

	result
}

/// Test helper: Get foreign key count for a table
pub(crate) async fn get_foreign_key_count(pool: &PgPool, table_name: &str) -> i64 {
	let result = sqlx::query_scalar::<_, i64>(
		r#"
        SELECT COUNT(*)
        FROM information_schema.table_constraints
        WHERE table_schema = 'public'
        AND table_name = $1
        AND constraint_type = 'FOREIGN KEY'
        "#,
	)
	.bind(table_name)
	.fetch_one(pool)
	.await
	.expect("Failed to count foreign keys");

	result
}
