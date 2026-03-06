//! Common test fixtures for reinhardt-tables tests

use reinhardt_tables::column::BaseColumn;
use reinhardt_tables::table::SimpleTable;
use rstest::*;

/// Test user data structure for table tests
#[derive(Debug, Clone, PartialEq)]
pub struct TestUser {
	pub id: i32,
	pub name: String,
	pub email: String,
	pub active: bool,
	pub created_at: String,
}

/// Fixture providing sample users for testing
#[fixture]
pub fn sample_users() -> Vec<TestUser> {
	vec![
		TestUser {
			id: 1,
			name: "Alice".to_string(),
			email: "alice@example.com".to_string(),
			active: true,
			created_at: "2024-01-15".to_string(),
		},
		TestUser {
			id: 2,
			name: "Bob".to_string(),
			email: "bob@example.com".to_string(),
			active: false,
			created_at: "2024-02-20".to_string(),
		},
		TestUser {
			id: 3,
			name: "Charlie".to_string(),
			email: "charlie@example.com".to_string(),
			active: true,
			created_at: "2024-03-10".to_string(),
		},
	]
}

/// Fixture providing a table with columns configured
#[fixture]
pub fn table_with_columns(sample_users: Vec<TestUser>) -> SimpleTable<TestUser> {
	let mut table = SimpleTable::with_rows(sample_users);

	// Add ID column
	table.add_column(Box::new(BaseColumn::new("id", "ID", |user: &TestUser| {
		user.id.to_string()
	})));

	// Add name column (sortable, filterable)
	table.add_column(Box::new(
		BaseColumn::new("name", "Name", |user: &TestUser| user.name.clone())
			.sortable(true)
			.filterable(true),
	));

	// Add email column
	table.add_column(Box::new(BaseColumn::new(
		"email",
		"Email",
		|user: &TestUser| user.email.clone(),
	)));

	// Add active column (boolean)
	table.add_column(Box::new(BaseColumn::new(
		"active",
		"Active",
		|user: &TestUser| user.active.to_string(),
	)));

	// Add created_at column
	table.add_column(Box::new(BaseColumn::new(
		"created_at",
		"Created At",
		|user: &TestUser| user.created_at.clone(),
	)));

	table
}

/// Fixture providing empty table
#[fixture]
pub fn empty_table() -> SimpleTable<TestUser> {
	SimpleTable::new()
}
