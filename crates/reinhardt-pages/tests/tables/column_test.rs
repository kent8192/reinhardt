//! Tests for table column types

use reinhardt_pages::tables::column::Column as ColumnTrait;
use reinhardt_pages::tables::columns::*;
use rstest::rstest;

#[rstest]
fn test_basic_column_creation() {
	let column = Column::<String>::new("name", "Name");
	assert_eq!(column.name(), "name");
	assert_eq!(column.label(), "Name");
	assert!(column.is_orderable());
	assert!(column.is_visible());
}

#[rstest]
fn test_basic_column_orderable() {
	let column = Column::<String>::new("name", "Name").orderable(false);
	assert!(!column.is_orderable());
}

#[rstest]
fn test_basic_column_visible() {
	let column = Column::<String>::new("name", "Name").visible(false);
	assert!(!column.is_visible());
}

#[rstest]
fn test_link_column_creation() {
	let column = LinkColumn::new("email", "Email", "/users/{id}");
	assert_eq!(column.name(), "email");
	assert_eq!(column.label(), "Email");
	assert!(column.is_orderable());
}

#[rstest]
fn test_link_column_with_text() {
	let column = LinkColumn::with_text("email", "Email", "/users/{id}", "View Profile");
	assert_eq!(column.name(), "email");
	assert_eq!(column.label(), "Email");
}

#[rstest]
fn test_boolean_column_creation() {
	let column = BooleanColumn::new("is_active", "Active");
	assert_eq!(column.name(), "is_active");
	assert_eq!(column.label(), "Active");
	assert!(column.is_orderable());
}

#[rstest]
fn test_boolean_column_with_icons() {
	let column = BooleanColumn::with_icons("is_active", "Active", "✓", "✗");
	assert_eq!(column.name(), "is_active");
	assert_eq!(column.label(), "Active");
}

#[rstest]
fn test_datetime_column_creation() {
	let column = DateTimeColumn::new("created_at", "Created");
	assert_eq!(column.name(), "created_at");
	assert_eq!(column.label(), "Created");
	assert!(column.is_orderable());
}

#[rstest]
fn test_email_column_creation() {
	let column = EmailColumn::new("email", "Email");
	assert_eq!(column.name(), "email");
	assert_eq!(column.label(), "Email");
	assert!(column.is_orderable());
}

#[rstest]
fn test_choice_column_creation() {
	let column = ChoiceColumn::new("status", "Status");
	assert_eq!(column.name(), "status");
	assert_eq!(column.label(), "Status");
	assert!(column.is_orderable());
}

#[rstest]
fn test_template_column_creation() {
	let column = TemplateColumn::new("custom", "Custom");
	assert_eq!(column.name(), "custom");
	assert_eq!(column.label(), "Custom");
	assert!(!column.is_orderable()); // Template columns are not orderable by default
}

#[rstest]
fn test_json_column_creation() {
	let column = JSONColumn::new("data", "Data");
	assert_eq!(column.name(), "data");
	assert_eq!(column.label(), "Data");
	assert!(!column.is_orderable()); // JSON columns are not orderable by default
}

#[rstest]
fn test_checkbox_column_creation() {
	let column = CheckBoxColumn::new("selected", "Select");
	assert_eq!(column.name(), "selected");
	assert_eq!(column.label(), "Select");
	assert!(!column.is_orderable()); // Checkbox columns are not orderable by default
}

#[rstest]
fn test_url_column_creation() {
	let column = URLColumn::new("website", "Website");
	assert_eq!(column.name(), "website");
	assert_eq!(column.label(), "Website");
	assert!(column.is_orderable());
}
