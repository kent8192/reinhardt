//! Unit tests for Table trait and SimpleTable implementation

use reinhardt_tables::column::BaseColumn;
use reinhardt_tables::table::{SimpleTable, SortOrder};
use reinhardt_tables::{Column, Table};
use rstest::*;

mod fixtures;

use fixtures::{TestUser, empty_table, sample_users, table_with_columns};

// ============================================================================
// Table Creation Tests
// ============================================================================

/// Tests that SimpleTable::new() creates an empty table with no rows or columns
#[rstest]
fn test_create_empty_table(empty_table: SimpleTable<TestUser>) {
	assert_eq!(empty_table.rows().len(), 0);
	assert_eq!(empty_table.total_rows(), 0);
	assert!(empty_table.columns().is_empty());
}

/// Tests that SimpleTable::with_rows() creates a table with the specified data
#[rstest]
fn test_create_table_with_rows(sample_users: Vec<TestUser>) {
	let table = SimpleTable::with_rows(sample_users);
	assert_eq!(table.rows().len(), 3);
	assert_eq!(table.total_rows(), 3);
}

/// Tests that SimpleTable::default() creates an empty table
#[rstest]
fn test_table_default_impl() {
	let table: SimpleTable<TestUser> = SimpleTable::default();
	assert_eq!(table.rows().len(), 0);
	assert_eq!(table.total_rows(), 0);
}

// ============================================================================
// Column Management Tests
// ============================================================================

/// Tests adding a single column to a table
#[rstest]
fn test_add_single_column() {
	let mut table: SimpleTable<TestUser> = SimpleTable::new();

	let column: Box<dyn Column<Row = TestUser>> =
		Box::new(BaseColumn::new("name", "Name", |user: &TestUser| {
			user.name.clone()
		}));

	table.add_column(column);

	assert_eq!(table.columns().len(), 1);
	assert_eq!(table.columns()[0].name(), "name");
	assert_eq!(table.columns()[0].header(), "Name");
}

/// Tests adding multiple columns to a table
#[rstest]
fn test_add_multiple_columns(sample_users: Vec<TestUser>) {
	let mut table = SimpleTable::with_rows(sample_users);

	table.add_column(Box::new(BaseColumn::new("id", "ID", |user: &TestUser| {
		user.id.to_string()
	})));

	table.add_column(Box::new(BaseColumn::new(
		"name",
		"Name",
		|user: &TestUser| user.name.clone(),
	)));

	table.add_column(Box::new(BaseColumn::new(
		"email",
		"Email",
		|user: &TestUser| user.email.clone(),
	)));

	assert_eq!(table.columns().len(), 3);
	assert_eq!(table.columns()[0].name(), "id");
	assert_eq!(table.columns()[1].name(), "name");
	assert_eq!(table.columns()[2].name(), "email");
}

// ============================================================================
// Sort Configuration Tests
// ============================================================================

/// Tests that sort_config() returns None initially
#[rstest]
fn test_sort_config_initially_none(table_with_columns: SimpleTable<TestUser>) {
	assert!(table_with_columns.sort_config().is_none());
}

/// Tests that sort_by() with Ascending order stores the correct config
#[rstest]
fn test_sort_by_ascending(table_with_columns: SimpleTable<TestUser>) {
	let mut table = table_with_columns;

	let result = table.sort_by("name", SortOrder::Ascending);

	assert!(result.is_ok());

	let sort_config = table.sort_config().unwrap();
	assert_eq!(sort_config.field, "name");
	assert_eq!(sort_config.order, SortOrder::Ascending);
}

/// Tests that sort_by() with Descending order stores the correct config
#[rstest]
fn test_sort_by_descending(table_with_columns: SimpleTable<TestUser>) {
	let mut table = table_with_columns;

	let result = table.sort_by("name", SortOrder::Descending);

	assert!(result.is_ok());

	let sort_config = table.sort_config().unwrap();
	assert_eq!(sort_config.field, "name");
	assert_eq!(sort_config.order, SortOrder::Descending);
}

/// Tests that sorting by a non-existent column returns TableError::ColumnNotFound
#[rstest]
fn test_sort_by_nonexistent_column(table_with_columns: SimpleTable<TestUser>) {
	let mut table = table_with_columns;

	let result = table.sort_by("nonexistent", SortOrder::Ascending);

	assert!(result.is_err());

	use reinhardt_tables::TableError;
	assert!(matches!(result.unwrap_err(), TableError::ColumnNotFound(_)));
}

/// Tests that sorting a non-sortable column returns an error
#[rstest]
fn test_table_sort_non_sortable_column(sample_users: Vec<TestUser>) {
	let mut table = SimpleTable::with_rows(sample_users);

	table.add_column(Box::new(
		BaseColumn::new("name", "Name", |user: &TestUser| user.name.clone()).sortable(false),
	));

	let result = table.sort_by("name", SortOrder::Ascending);

	assert!(result.is_err());

	use reinhardt_tables::TableError;
	assert!(matches!(
		result.unwrap_err(),
		TableError::InvalidSortOrder(_)
	));
}

// ============================================================================
// Pagination Configuration Tests
// ============================================================================

/// Tests that pagination_config() returns None initially
#[rstest]
fn test_pagination_config_initially_none(table_with_columns: SimpleTable<TestUser>) {
	assert!(table_with_columns.pagination_config().is_none());
}

/// Tests that paginate() with valid parameters stores the correct config
#[rstest]
fn test_paginate_valid_page(table_with_columns: SimpleTable<TestUser>) {
	let mut table = table_with_columns;

	let result = table.paginate(1, 25);

	assert!(result.is_ok());

	let pagination = table.pagination_config().unwrap();
	assert_eq!(pagination.page, 1);
	assert_eq!(pagination.per_page, 25);
}

/// Tests that paginate() with page 0 returns TableError::InvalidPageNumber
#[rstest]
fn test_paginate_page_zero_error(table_with_columns: SimpleTable<TestUser>) {
	let mut table = table_with_columns;

	let result = table.paginate(0, 25);

	assert!(result.is_err());

	use reinhardt_tables::TableError;
	assert!(matches!(
		result.unwrap_err(),
		TableError::InvalidPageNumber(0)
	));
}

/// Tests that total_pages() calculates the correct number of pages
#[rstest]
fn test_total_pages_calculation(table_with_columns: SimpleTable<TestUser>) {
	let mut table = table_with_columns;

	// 3 rows, 2 per page = 2 pages
	table.paginate(1, 2).unwrap();
	assert_eq!(table.total_pages(), 2);

	// 3 rows, 5 per page = 1 page
	table.paginate(1, 5).unwrap();
	assert_eq!(table.total_pages(), 1);

	// 3 rows, 1 per page = 3 pages
	table.paginate(1, 1).unwrap();
	assert_eq!(table.total_pages(), 3);
}

// ============================================================================
// Visible Rows Tests
// ============================================================================

/// Tests that visible_rows() returns all rows when no filters or pagination
#[rstest]
fn test_visible_rows_no_filters_or_pagination(table_with_columns: SimpleTable<TestUser>) {
	let table = table_with_columns;

	let visible = table.visible_rows();
	assert_eq!(visible.len(), 3);
}

/// Tests that visible_rows() respects pagination
#[rstest]
fn test_visible_rows_with_pagination(table_with_columns: SimpleTable<TestUser>) {
	let mut table = table_with_columns;

	table.paginate(1, 2).unwrap();

	let visible = table.visible_rows();
	assert_eq!(visible.len(), 2);
}

/// Tests that visible_rows() returns correctly sorted data (ascending)
#[rstest]
fn test_visible_rows_with_sorting_ascending(table_with_columns: SimpleTable<TestUser>) {
	let mut table = table_with_columns;

	table.sort_by("name", SortOrder::Ascending).unwrap();

	let visible = table.visible_rows();
	assert_eq!(visible.len(), 3);
	assert_eq!(visible[0].name, "Alice");
	assert_eq!(visible[1].name, "Bob");
	assert_eq!(visible[2].name, "Charlie");
}

/// Tests that visible_rows() returns correctly sorted data (descending)
#[rstest]
fn test_visible_rows_with_sorting_descending(table_with_columns: SimpleTable<TestUser>) {
	let mut table = table_with_columns;

	table.sort_by("name", SortOrder::Descending).unwrap();

	let visible = table.visible_rows();
	assert_eq!(visible.len(), 3);
	assert_eq!(visible[0].name, "Charlie");
	assert_eq!(visible[1].name, "Bob");
	assert_eq!(visible[2].name, "Alice");
}

// ============================================================================
// Filter Tests
// ============================================================================

/// Tests that filters() returns an empty HashMap initially
#[rstest]
fn test_filters_initially_empty(table_with_columns: SimpleTable<TestUser>) {
	let table = table_with_columns;
	assert!(table.filters().is_empty());
}

/// Tests that filter() applies a single filter correctly
#[rstest]
fn test_apply_single_filter(table_with_columns: SimpleTable<TestUser>) {
	let mut table = table_with_columns;

	let mut filters = std::collections::HashMap::new();
	filters.insert("name".to_string(), "Alice".to_string());

	let result = table.filter(filters);

	assert!(result.is_ok());
	assert_eq!(table.filters().len(), 1);
	assert_eq!(table.filtered_rows_count(), 1);
}

/// Tests that filter() returns error for non-existent column
#[rstest]
fn test_table_filter_nonexistent_column(table_with_columns: SimpleTable<TestUser>) {
	let mut table = table_with_columns;

	let mut filters = std::collections::HashMap::new();
	filters.insert("nonexistent".to_string(), "value".to_string());

	let result = table.filter(filters);

	assert!(result.is_err());

	use reinhardt_tables::TableError;
	assert!(matches!(result.unwrap_err(), TableError::ColumnNotFound(_)));
}

/// Tests that filter() returns error for non-filterable column
#[rstest]
fn test_table_filter_non_filterable_column(sample_users: Vec<TestUser>) {
	let mut table = SimpleTable::with_rows(sample_users);

	table.add_column(Box::new(
		BaseColumn::new("name", "Name", |user: &TestUser| user.name.clone()).filterable(false),
	));

	let mut filters = std::collections::HashMap::new();
	filters.insert("name".to_string(), "Alice".to_string());

	let result = table.filter(filters);

	assert!(result.is_err());

	use reinhardt_tables::TableError;
	assert!(matches!(
		result.unwrap_err(),
		TableError::ColumnNotFilterable(_)
	));
}

// ============================================================================
// Combined Operations Tests
// ============================================================================

/// Tests that sort and filter operations work correctly together
#[rstest]
fn test_table_sort_and_filter(sample_users: Vec<TestUser>) {
	let mut table = SimpleTable::with_rows(sample_users);

	table.add_column(Box::new(BaseColumn::new(
		"name",
		"Name",
		|user: &TestUser| user.name.clone(),
	)));

	table.add_column(Box::new(BaseColumn::new(
		"active",
		"Active",
		|user: &TestUser| user.active.to_string(),
	)));

	// Filter by active = true
	let mut filters = std::collections::HashMap::new();
	filters.insert("active".to_string(), "true".to_string());
	table.filter(filters).unwrap();

	// Sort by name descending
	table.sort_by("name", SortOrder::Descending).unwrap();

	let visible = table.visible_rows();
	assert_eq!(visible.len(), 2);
	assert_eq!(visible[0].name, "Charlie");
	assert_eq!(visible[1].name, "Alice");
}

/// Tests that pagination works with filtered results
#[rstest]
fn test_table_pagination_with_filter(sample_users: Vec<TestUser>) {
	let mut table = SimpleTable::with_rows(sample_users);

	table.add_column(Box::new(BaseColumn::new(
		"name",
		"Name",
		|user: &TestUser| user.name.clone(),
	)));

	// Filter by name containing "li" (matches Alice and Charlie)
	let mut filters = std::collections::HashMap::new();
	filters.insert("name".to_string(), "li".to_string());
	table.filter(filters).unwrap();

	// Paginate: 1 per page
	table.paginate(1, 1).unwrap();

	let visible = table.visible_rows();
	assert_eq!(visible.len(), 1);
	assert_eq!(visible[0].name, "Alice");
	assert_eq!(table.filtered_rows_count(), 2);
	assert_eq!(table.total_pages(), 2);

	// Page 2
	table.paginate(2, 1).unwrap();
	let visible = table.visible_rows();
	assert_eq!(visible.len(), 1);
	assert_eq!(visible[0].name, "Charlie");
}

/// Tests that sort, filter, and pagination work correctly together
#[rstest]
fn test_table_sort_filter_paginate(sample_users: Vec<TestUser>) {
	let mut table = SimpleTable::with_rows(sample_users);

	table.add_column(Box::new(BaseColumn::new(
		"name",
		"Name",
		|user: &TestUser| user.name.clone(),
	)));

	table.add_column(Box::new(BaseColumn::new(
		"active",
		"Active",
		|user: &TestUser| user.active.to_string(),
	)));

	// Filter by active = true (Alice, Charlie)
	let mut filters = std::collections::HashMap::new();
	filters.insert("active".to_string(), "true".to_string());
	table.filter(filters).unwrap();

	// Sort by name descending
	table.sort_by("name", SortOrder::Descending).unwrap();

	// Paginate: 1 per page
	table.paginate(1, 1).unwrap();

	let visible = table.visible_rows();
	assert_eq!(visible.len(), 1);
	assert_eq!(visible[0].name, "Charlie");

	// Page 2
	table.paginate(2, 1).unwrap();
	let visible = table.visible_rows();
	assert_eq!(visible.len(), 1);
	assert_eq!(visible[0].name, "Alice");
}
