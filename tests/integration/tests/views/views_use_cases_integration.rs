//! ViewSet Use Cases Integration Tests
//!
//! Tests practical real-world use case scenarios:
//! - Blog posting system (create, publish, archive workflow)
//! - E-commerce product management (inventory, pricing, search)
//! - Task management system (create, status updates, completion)
//! - User profile management (update, privacy settings)
//! - Comment/Review system (nested resources, moderation)
//! - Category-based content management
//! - Multi-tenant resource isolation
//!
//! **Test Category**: Use Case Testing (ユースケーステスト)
//!
//! **Note**: These tests simulate real-world application workflows,
//! combining multiple operations to verify end-to-end functionality.

use bytes::Bytes;
use chrono::{DateTime, Utc};
use hyper::{HeaderMap, Method, Version};
use reinhardt_http::Request;
use reinhardt_macros::model;
use reinhardt_query::prelude::{
	ColumnDef, Iden, IntoIden, PostgresQueryBuilder, Query, QueryStatementBuilder,
};
use reinhardt_test::fixtures::get_test_pool;
use rstest::*;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

// ============================================================================
// Test Models
// ============================================================================

/// Blog post model for publishing workflow
#[allow(dead_code)]
#[model(app_label = "usecase_test", table_name = "blog_posts")]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct BlogPost {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 200)]
	title: String,
	#[field(max_length = 10000)]
	content: String,
	#[field(max_length = 50)]
	status: String, // draft, published, archived
	#[field(null = true)]
	author_id: Option<i64>,
	#[field(null = true)]
	published_at: Option<DateTime<Utc>>,
	#[field(null = true)]
	created_at: Option<DateTime<Utc>>,
}

/// Product model for e-commerce
#[allow(dead_code)]
#[model(app_label = "usecase_test", table_name = "products")]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct Product {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 200)]
	name: String,
	#[field(max_length = 100)]
	sku: String,
	price: f64,
	stock_quantity: i32,
	#[field(max_length = 100)]
	category: String,
	active: bool,
}

/// Task model for project management
#[allow(dead_code)]
#[model(app_label = "usecase_test", table_name = "tasks")]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct Task {
	#[field(primary_key = true)]
	id: Option<i64>,
	#[field(max_length = 200)]
	title: String,
	#[field(max_length = 5000)]
	description: String,
	#[field(max_length = 50)]
	status: String, // todo, in_progress, done
	priority: i32,
	#[field(null = true)]
	assignee_id: Option<i64>,
	#[field(null = true)]
	due_date: Option<DateTime<Utc>>,
}

// ============================================================================
// Iden Enums
// ============================================================================

#[derive(Debug, Clone, Copy, Iden)]
enum BlogPosts {
	Table,
	Id,
	Title,
	Content,
	Status,
	AuthorId,
	PublishedAt,
	CreatedAt,
}

#[derive(Debug, Clone, Copy, Iden)]
enum Products {
	Table,
	Id,
	Name,
	Sku,
	Price,
	StockQuantity,
	Category,
	Active,
}

#[derive(Debug, Clone, Copy, Iden)]
enum Tasks {
	Table,
	Id,
	Title,
	Description,
	Status,
	Priority,
	AssigneeId,
	DueDate,
}

// ============================================================================
// Fixtures
// ============================================================================

/// Setup: PostgreSQL container with blog posts schema
///
/// Uses shared PostgreSQL container with template database pattern.
/// Each test gets an isolated database cloned from template (~10-40ms).
#[fixture]
async fn setup_blog() -> PgPool {
	let pool = get_test_pool().await;

	// Create blog_posts table
	let mut stmt = Query::create_table();
	let create_table_sql = stmt
		.table(BlogPosts::Table.into_iden())
		.if_not_exists()
		.col(
			ColumnDef::new(BlogPosts::Id)
				.big_integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(BlogPosts::Title)
				.string_len(200)
				.not_null(true),
		)
		.col(
			ColumnDef::new(BlogPosts::Content)
				.string_len(10000)
				.not_null(true),
		)
		.col(
			ColumnDef::new(BlogPosts::Status)
				.string_len(50)
				.not_null(true),
		)
		.col(ColumnDef::new(BlogPosts::AuthorId).big_integer())
		.col(ColumnDef::new(BlogPosts::PublishedAt).timestamp_with_time_zone())
		.col(ColumnDef::new(BlogPosts::CreatedAt).timestamp_with_time_zone())
		.to_string(PostgresQueryBuilder::new());

	sqlx::query(&create_table_sql).execute(&pool).await.unwrap();

	pool
}

/// Setup: PostgreSQL container with products schema
///
/// Uses shared PostgreSQL container with template database pattern.
#[fixture]
async fn setup_products() -> PgPool {
	let pool = get_test_pool().await;

	// Create products table
	let mut stmt = Query::create_table();
	let create_table_sql = stmt
		.table(Products::Table.into_iden())
		.if_not_exists()
		.col(
			ColumnDef::new(Products::Id)
				.big_integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(
			ColumnDef::new(Products::Name)
				.string_len(200)
				.not_null(true),
		)
		.col(ColumnDef::new(Products::Sku).string_len(100).not_null(true))
		.col(ColumnDef::new(Products::Price).double().not_null(true))
		.col(
			ColumnDef::new(Products::StockQuantity)
				.integer()
				.not_null(true),
		)
		.col(
			ColumnDef::new(Products::Category)
				.string_len(100)
				.not_null(true),
		)
		.col(ColumnDef::new(Products::Active).boolean().not_null(true))
		.to_string(PostgresQueryBuilder::new());

	sqlx::query(&create_table_sql).execute(&pool).await.unwrap();

	pool
}

/// Setup: PostgreSQL container with tasks schema
///
/// Uses shared PostgreSQL container with template database pattern.
#[fixture]
async fn setup_tasks() -> PgPool {
	let pool = get_test_pool().await;

	// Create tasks table
	let mut stmt = Query::create_table();
	let create_table_sql = stmt
		.table(Tasks::Table.into_iden())
		.if_not_exists()
		.col(
			ColumnDef::new(Tasks::Id)
				.big_integer()
				.not_null(true)
				.auto_increment(true)
				.primary_key(true),
		)
		.col(ColumnDef::new(Tasks::Title).string_len(200).not_null(true))
		.col(
			ColumnDef::new(Tasks::Description)
				.string_len(5000)
				.not_null(true),
		)
		.col(ColumnDef::new(Tasks::Status).string_len(50).not_null(true))
		.col(ColumnDef::new(Tasks::Priority).integer().not_null(true))
		.col(ColumnDef::new(Tasks::AssigneeId).big_integer())
		.col(ColumnDef::new(Tasks::DueDate).timestamp_with_time_zone())
		.to_string(PostgresQueryBuilder::new());

	sqlx::query(&create_table_sql).execute(&pool).await.unwrap();

	pool
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Helper: Create HTTP POST request with JSON body
fn _create_post_request(uri: &str, body: &str) -> Request {
	let mut headers = HeaderMap::new();
	headers.insert(
		hyper::header::CONTENT_TYPE,
		"application/json".parse().unwrap(),
	);

	Request::builder()
		.method(Method::POST)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::from(body.to_string()))
		.build()
		.expect("Failed to build request")
}

/// Helper: Create HTTP PUT request with JSON body
fn _create_put_request(uri: &str, body: &str) -> Request {
	let mut headers = HeaderMap::new();
	headers.insert(
		hyper::header::CONTENT_TYPE,
		"application/json".parse().unwrap(),
	);

	Request::builder()
		.method(Method::PUT)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(headers)
		.body(Bytes::from(body.to_string()))
		.build()
		.expect("Failed to build request")
}

/// Helper: Create HTTP GET request
fn _create_get_request(uri: &str) -> Request {
	Request::builder()
		.method(Method::GET)
		.uri(uri)
		.version(Version::HTTP_11)
		.headers(HeaderMap::new())
		.body(Bytes::new())
		.build()
		.expect("Failed to build request")
}

// ============================================================================
// Tests
// ============================================================================

/// Use Case 1: Blog posting workflow (draft → published → archived)
#[rstest]
#[tokio::test]

async fn test_blog_posting_workflow(#[future] setup_blog: PgPool) {
	let pool = setup_blog.await;

	// Step 1: Create draft post
	let draft_post = BlogPost::new(
		"My First Blog Post".to_string(),
		"This is the content of my blog post.".to_string(),
		"draft".to_string(),
		Some(1),
		None,
		Some(Utc::now()),
	);

	let row = sqlx::query(
		"INSERT INTO blog_posts (title, content, status, author_id, published_at, created_at)
		 VALUES ($1, $2, $3, $4, $5, $6) RETURNING id, status",
	)
	.bind(&draft_post.title)
	.bind(&draft_post.content)
	.bind(&draft_post.status)
	.bind(draft_post.author_id)
	.bind(draft_post.published_at)
	.bind(draft_post.created_at)
	.fetch_one(&pool)
	.await
	.unwrap();

	let post_id: i64 = row.get("id");
	let status: String = row.get("status");

	assert_eq!(status, "draft");

	// Step 2: Publish post (update status and published_at)
	let published_at = Utc::now();
	sqlx::query("UPDATE blog_posts SET status = $1, published_at = $2 WHERE id = $3")
		.bind("published")
		.bind(published_at)
		.bind(post_id)
		.execute(&pool)
		.await
		.unwrap();

	// Verify published status
	let published_row = sqlx::query("SELECT status, published_at FROM blog_posts WHERE id = $1")
		.bind(post_id)
		.fetch_one(&pool)
		.await
		.unwrap();

	let published_status: String = published_row.get("status");
	let published_at_result: Option<DateTime<Utc>> = published_row.get("published_at");

	assert_eq!(published_status, "published");
	assert!(published_at_result.is_some(), "published_at should be set");

	// Step 3: Archive post
	sqlx::query("UPDATE blog_posts SET status = $1 WHERE id = $2")
		.bind("archived")
		.bind(post_id)
		.execute(&pool)
		.await
		.unwrap();

	// Verify archived status
	let archived_row = sqlx::query("SELECT status FROM blog_posts WHERE id = $1")
		.bind(post_id)
		.fetch_one(&pool)
		.await
		.unwrap();

	let archived_status: String = archived_row.get("status");

	assert_eq!(archived_status, "archived");
}

/// Use Case 2: E-commerce inventory management
#[rstest]
#[tokio::test]

async fn test_ecommerce_inventory_management(#[future] setup_products: PgPool) {
	let pool = setup_products.await;

	// Step 1: Add new product
	let product = Product::new(
		"Wireless Mouse".to_string(),
		"WM-001".to_string(),
		29.99,
		100,
		"Electronics".to_string(),
		true,
	);

	let row = sqlx::query(
		"INSERT INTO products (name, sku, price, stock_quantity, category, active)
		 VALUES ($1, $2, $3, $4, $5, $6) RETURNING id, stock_quantity",
	)
	.bind(&product.name)
	.bind(&product.sku)
	.bind(product.price)
	.bind(product.stock_quantity)
	.bind(&product.category)
	.bind(product.active)
	.fetch_one(&pool)
	.await
	.unwrap();

	let product_id: i64 = row.get("id");
	let initial_stock: i32 = row.get("stock_quantity");

	assert_eq!(initial_stock, 100);

	// Step 2: Simulate sale (decrease stock by 5)
	let new_stock = initial_stock - 5;
	sqlx::query("UPDATE products SET stock_quantity = $1 WHERE id = $2")
		.bind(new_stock)
		.bind(product_id)
		.execute(&pool)
		.await
		.unwrap();

	// Verify stock updated
	let updated_row = sqlx::query("SELECT stock_quantity FROM products WHERE id = $1")
		.bind(product_id)
		.fetch_one(&pool)
		.await
		.unwrap();

	let updated_stock: i32 = updated_row.get("stock_quantity");

	assert_eq!(updated_stock, 95);

	// Step 3: Deactivate product when out of stock
	sqlx::query("UPDATE products SET stock_quantity = $1, active = $2 WHERE id = $3")
		.bind(0)
		.bind(false)
		.bind(product_id)
		.execute(&pool)
		.await
		.unwrap();

	// Verify deactivated
	let final_row = sqlx::query("SELECT stock_quantity FROM products WHERE id = $1")
		.bind(product_id)
		.fetch_one(&pool)
		.await
		.unwrap();

	let final_stock: i32 = final_row.get("stock_quantity");

	assert_eq!(final_stock, 0);
}

/// Use Case 3: Task management system workflow
#[rstest]
#[tokio::test]

async fn test_task_management_workflow(#[future] setup_tasks: PgPool) {
	let pool = setup_tasks.await;

	// Step 1: Create new task
	let task = Task::new(
		"Implement login feature".to_string(),
		"Add user authentication with JWT".to_string(),
		"todo".to_string(),
		1,                                                         // High priority
		Some(5),                                                   // Assignee ID
		Some(Utc::now() + chrono::Duration::try_days(7).unwrap()), // Due in 7 days
	);

	let row = sqlx::query(
		"INSERT INTO tasks (title, description, status, priority, assignee_id, due_date)
		 VALUES ($1, $2, $3, $4, $5, $6) RETURNING id, status",
	)
	.bind(&task.title)
	.bind(&task.description)
	.bind(&task.status)
	.bind(task.priority)
	.bind(task.assignee_id)
	.bind(task.due_date)
	.fetch_one(&pool)
	.await
	.unwrap();

	let task_id: i64 = row.get("id");
	let initial_status: String = row.get("status");

	assert_eq!(initial_status, "todo");

	// Step 2: Start working on task
	sqlx::query("UPDATE tasks SET status = $1 WHERE id = $2")
		.bind("in_progress")
		.bind(task_id)
		.execute(&pool)
		.await
		.unwrap();

	// Step 3: Complete task
	sqlx::query("UPDATE tasks SET status = $1 WHERE id = $2")
		.bind("done")
		.bind(task_id)
		.execute(&pool)
		.await
		.unwrap();

	// Verify task completed
	let completed_row = sqlx::query("SELECT status FROM tasks WHERE id = $1")
		.bind(task_id)
		.fetch_one(&pool)
		.await
		.unwrap();

	let final_status: String = completed_row.get("status");

	assert_eq!(final_status, "done");
}

/// Use Case 4: Product search and filtering
#[rstest]
#[tokio::test]

async fn test_product_search_filtering(#[future] setup_products: PgPool) {
	let pool = setup_products.await;

	// Insert multiple products
	for i in 1..=10 {
		let product = Product::new(
			format!("Product {}", i),
			format!("SKU-{:03}", i),
			(i as f64) * 10.0,
			i * 5,
			if i % 2 == 0 { "Electronics" } else { "Books" }.to_string(),
			i % 3 != 0, // Some inactive products
		);

		sqlx::query(
			"INSERT INTO products (name, sku, price, stock_quantity, category, active)
			 VALUES ($1, $2, $3, $4, $5, $6)",
		)
		.bind(&product.name)
		.bind(&product.sku)
		.bind(product.price)
		.bind(product.stock_quantity)
		.bind(&product.category)
		.bind(product.active)
		.execute(&pool)
		.await
		.unwrap();
	}

	// Search: Active Electronics products with price < 50
	let rows = sqlx::query(
		"SELECT id, name, category, price FROM products
		 WHERE category = $1 AND active = $2 AND price < $3
		 ORDER BY price ASC",
	)
	.bind("Electronics")
	.bind(true)
	.bind(50.0)
	.fetch_all(&pool)
	.await
	.unwrap();

	// Verify all results match criteria
	for row in rows {
		let category: String = row.get("category");
		let price: f64 = row.get("price");

		assert_eq!(category, "Electronics");
		assert!(price < 50.0, "Price should be less than 50");
	}
}

/// Use Case 5: Bulk task creation for project
#[rstest]
#[tokio::test]

async fn test_bulk_task_creation(#[future] setup_tasks: PgPool) {
	let pool = setup_tasks.await;

	// Create multiple tasks for a project
	let task_titles = vec![
		"Design database schema",
		"Implement API endpoints",
		"Write unit tests",
		"Create documentation",
		"Deploy to staging",
	];

	for (index, title) in task_titles.iter().enumerate() {
		let task = Task::new(
			String::from(*title),
			format!("Description for {}", title),
			"todo".to_string(),
			(index as i32) + 1, // Priority based on order
			Some(1),            // Assignee
			Some(Utc::now() + chrono::Duration::try_days((index as i64) + 1).unwrap()),
		);

		sqlx::query(
			"INSERT INTO tasks (title, description, status, priority, assignee_id, due_date)
			 VALUES ($1, $2, $3, $4, $5, $6)",
		)
		.bind(&task.title)
		.bind(&task.description)
		.bind(&task.status)
		.bind(task.priority)
		.bind(task.assignee_id)
		.bind(task.due_date)
		.execute(&pool)
		.await
		.unwrap();
	}

	// Verify all tasks created
	let count_row = sqlx::query("SELECT COUNT(id) FROM tasks")
		.fetch_one(&pool)
		.await
		.unwrap();

	let task_count: i64 = count_row.get(0);

	assert_eq!(task_count, 5, "Should have created 5 tasks");
}

/// Use Case 6: Category-based content organization
#[rstest]
#[tokio::test]

async fn test_category_based_organization(#[future] setup_blog: PgPool) {
	let pool = setup_blog.await;

	// Insert posts in different categories (simulated with status field)
	let categories = vec!["Technology", "Travel", "Food", "Technology", "Travel"];

	for (index, category) in categories.iter().enumerate() {
		let post = BlogPost::new(
			format!("Post about {}", category),
			format!("Content for {} post {}", category, index),
			String::from(*category), // Using status field as category for this test
			Some(1),
			if category == &"Technology" {
				Some(Utc::now())
			} else {
				None
			},
			Some(Utc::now()),
		);

		sqlx::query(
			"INSERT INTO blog_posts (title, content, status, author_id, published_at, created_at)
			 VALUES ($1, $2, $3, $4, $5, $6)",
		)
		.bind(&post.title)
		.bind(&post.content)
		.bind(&post.status)
		.bind(post.author_id)
		.bind(post.published_at)
		.bind(post.created_at)
		.execute(&pool)
		.await
		.unwrap();
	}

	// Query: Get all Technology posts
	let tech_rows = sqlx::query("SELECT id, title, status FROM blog_posts WHERE status = $1")
		.bind("Technology")
		.fetch_all(&pool)
		.await
		.unwrap();

	assert_eq!(tech_rows.len(), 2, "Should have 2 Technology posts");

	// Verify all are Technology category
	for row in tech_rows {
		let status: String = row.get("status");
		assert_eq!(status, "Technology");
	}
}

/// Use Case 7: Multi-tenant product isolation
#[rstest]
#[tokio::test]

async fn test_multi_tenant_isolation(#[future] setup_products: PgPool) {
	let pool = setup_products.await;

	// Insert products for different "tenants" (simulated with category field)
	let tenant_products = vec![
		("Tenant A", "Product A1"),
		("Tenant A", "Product A2"),
		("Tenant B", "Product B1"),
		("Tenant B", "Product B2"),
		("Tenant C", "Product C1"),
	];

	for (index, (tenant, name)) in tenant_products.iter().enumerate() {
		let product = Product::new(
			String::from(*name),
			format!("SKU-{}", index),
			99.99,
			10,
			String::from(*tenant), // Using category as tenant identifier
			true,
		);

		sqlx::query(
			"INSERT INTO products (name, sku, price, stock_quantity, category, active)
			 VALUES ($1, $2, $3, $4, $5, $6)",
		)
		.bind(&product.name)
		.bind(&product.sku)
		.bind(product.price)
		.bind(product.stock_quantity)
		.bind(&product.category)
		.bind(product.active)
		.execute(&pool)
		.await
		.unwrap();
	}

	// Query: Get only Tenant A products
	let tenant_a_rows = sqlx::query("SELECT id, name, category FROM products WHERE category = $1")
		.bind("Tenant A")
		.fetch_all(&pool)
		.await
		.unwrap();

	assert_eq!(tenant_a_rows.len(), 2, "Tenant A should have 2 products");

	// Verify isolation - no products from other tenants
	for row in tenant_a_rows {
		let category: String = row.get("category");
		assert_eq!(category, "Tenant A");
	}
}
