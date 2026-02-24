//! Admin framework unit tests (extracted from reinhardt-views/src/admin.rs)
//!
//! Tests ModelAdmin configuration, search, filtering, rendering, and permissions.

use reinhardt_views::admin::{AdminView, ModelAdmin};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct TestModel {
	id: Option<i64>,
	name: String,
	active: bool,
}

reinhardt_test::impl_test_model!(TestModel, i64, "test_models");

#[test]
fn test_model_admin_creation() {
	// Arrange & Act
	let admin = ModelAdmin::<TestModel>::new();

	// Assert
	assert_eq!(admin.list_per_page(), 100);
	assert!(admin.list_display().is_empty());
	assert!(admin.list_filter().is_empty());
	assert!(admin.search_fields().is_empty());
}

#[test]
fn test_model_admin_with_list_display() {
	// Arrange & Act
	let admin = ModelAdmin::<TestModel>::new()
		.with_list_display(vec!["id".to_string(), "name".to_string()]);

	// Assert
	assert_eq!(admin.list_display().len(), 2);
	assert_eq!(admin.list_display()[0], "id");
	assert_eq!(admin.list_display()[1], "name");
}

#[test]
fn test_model_admin_with_filters() {
	// Arrange & Act
	let admin = ModelAdmin::<TestModel>::new()
		.with_list_filter(vec!["active".to_string()])
		.with_search_fields(vec!["name".to_string()]);

	// Assert
	assert_eq!(admin.list_filter().len(), 1);
	assert_eq!(admin.search_fields().len(), 1);
}

#[test]
fn test_model_admin_search() {
	// Arrange
	let admin = ModelAdmin::<TestModel>::new().with_search_fields(vec!["name".to_string()]);

	let objects = vec![
		TestModel {
			id: Some(1),
			name: "Alice".to_string(),
			active: true,
		},
		TestModel {
			id: Some(2),
			name: "Bob".to_string(),
			active: false,
		},
		TestModel {
			id: Some(3),
			name: "Charlie".to_string(),
			active: true,
		},
	];

	// Act
	let results = admin.search("ali", objects);

	// Assert
	assert_eq!(results.len(), 1);
	assert_eq!(results[0].name, "Alice");
}

#[test]
fn test_model_admin_filter() {
	// Arrange
	let admin = ModelAdmin::<TestModel>::new().with_list_filter(vec!["active".to_string()]);

	let objects = vec![
		TestModel {
			id: Some(1),
			name: "Alice".to_string(),
			active: true,
		},
		TestModel {
			id: Some(2),
			name: "Bob".to_string(),
			active: false,
		},
		TestModel {
			id: Some(3),
			name: "Charlie".to_string(),
			active: true,
		},
	];

	let mut filters = HashMap::new();
	filters.insert("active".to_string(), "true".to_string());

	// Act
	let results = admin.filter(&filters, objects);

	// Assert
	assert_eq!(results.len(), 2);
	assert!(results.iter().all(|r| r.active));
}

#[tokio::test]
async fn test_model_admin_get_queryset() {
	// Arrange
	let objects = vec![TestModel {
		id: Some(1),
		name: "Test".to_string(),
		active: true,
	}];

	let admin = ModelAdmin::<TestModel>::new().with_queryset(objects.clone());

	// Act
	let queryset = admin.get_queryset().await.unwrap();

	// Assert
	assert_eq!(queryset.len(), 1);
	assert_eq!(queryset[0].name, "Test");
}

#[tokio::test]
async fn test_model_admin_render_list() {
	// Arrange
	let objects = vec![
		TestModel {
			id: Some(1),
			name: "Alice".to_string(),
			active: true,
		},
		TestModel {
			id: Some(2),
			name: "Bob".to_string(),
			active: false,
		},
	];

	let admin = ModelAdmin::<TestModel>::new()
		.with_queryset(objects)
		.with_list_display(vec!["id".to_string(), "name".to_string()]);

	// Act
	let html = admin.render_list().await.unwrap();

	// Assert
	assert!(html.contains("<div class=\"admin-list\">"));
	assert!(html.contains("test_models List"));
	assert!(html.contains("Total: 2 items"));
	assert!(html.contains("<th>id</th>"));
	assert!(html.contains("<th>name</th>"));
}

#[tokio::test]
async fn test_model_admin_render_via_trait() {
	// Arrange
	let objects = vec![TestModel {
		id: Some(1),
		name: "Test".to_string(),
		active: true,
	}];

	let admin = ModelAdmin::<TestModel>::new().with_queryset(objects);

	// Act
	let html = admin.render().await.unwrap();

	// Assert
	assert!(html.contains("<div class=\"admin-list\">"));
}

#[test]
fn test_model_admin_permissions() {
	// Arrange & Act
	let admin = ModelAdmin::<TestModel>::new();

	// Assert
	assert!(admin.has_view_permission());
	assert!(admin.has_add_permission());
	assert!(admin.has_change_permission());
	assert!(admin.has_delete_permission());
}
