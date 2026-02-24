//! Generic API Views unit tests (extracted from reinhardt-views/src/generic/tests.rs)
//!
//! Tests allowed methods for each Generic API View type:
//! - ListAPIView, CreateAPIView, UpdateAPIView, DestroyAPIView
//! - Composite views: ListCreateAPIView, RetrieveUpdateAPIView,
//!   RetrieveDestroyAPIView, RetrieveUpdateDestroyAPIView

use reinhardt_rest::serializers::JsonSerializer;
use reinhardt_views::{
	CreateAPIView, DestroyAPIView, ListAPIView, ListCreateAPIView, RetrieveDestroyAPIView,
	RetrieveUpdateAPIView, RetrieveUpdateDestroyAPIView, UpdateAPIView, View,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestArticle {
	id: Option<i64>,
	title: String,
}

reinhardt_test::impl_test_model!(TestArticle, i64, "articles");

// ============================================================================
// ListAPIView Tests
// ============================================================================

#[test]
fn test_list_api_view_new() {
	// Arrange & Act
	let view = ListAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	// Assert
	let methods = view.allowed_methods();
	assert_eq!(methods, vec!["GET", "HEAD", "OPTIONS"]);
}

#[test]
fn test_list_api_view_with_ordering() {
	// Arrange & Act
	let view = ListAPIView::<TestArticle, JsonSerializer<TestArticle>>::new()
		.with_ordering(vec!["-created_at".to_string(), "title".to_string()]);

	// Assert
	let methods = view.allowed_methods();
	assert_eq!(methods, vec!["GET", "HEAD", "OPTIONS"]);
}

#[test]
fn test_list_api_view_with_pagination() {
	// Arrange & Act
	let view = ListAPIView::<TestArticle, JsonSerializer<TestArticle>>::new().with_paginate_by(20);

	// Assert
	let methods = view.allowed_methods();
	assert_eq!(methods, vec!["GET", "HEAD", "OPTIONS"]);
}

// ============================================================================
// CreateAPIView Tests
// ============================================================================

#[test]
fn test_create_api_view_new() {
	// Arrange & Act
	let view = CreateAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	// Assert
	let methods = view.allowed_methods();
	assert_eq!(methods, vec!["POST", "OPTIONS"]);
}

#[test]
fn test_create_api_view_default() {
	// Arrange & Act
	let view = CreateAPIView::<TestArticle, JsonSerializer<TestArticle>>::default();

	// Assert
	let methods = view.allowed_methods();
	assert_eq!(methods, vec!["POST", "OPTIONS"]);
}

// ============================================================================
// UpdateAPIView Tests
// ============================================================================

#[test]
fn test_update_api_view_new() {
	// Arrange & Act
	let view = UpdateAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	// Assert
	let methods = view.allowed_methods();
	assert_eq!(methods, vec!["PUT", "PATCH", "OPTIONS"]);
}

#[test]
fn test_update_api_view_with_lookup_field() {
	// Arrange & Act
	let view = UpdateAPIView::<TestArticle, JsonSerializer<TestArticle>>::new()
		.with_lookup_field("slug".to_string());

	// Assert
	let methods = view.allowed_methods();
	assert_eq!(methods, vec!["PUT", "PATCH", "OPTIONS"]);
}

#[test]
fn test_update_api_view_with_partial() {
	// Arrange & Act
	let view = UpdateAPIView::<TestArticle, JsonSerializer<TestArticle>>::new().with_partial(true);

	// Assert
	let methods = view.allowed_methods();
	assert_eq!(methods, vec!["PUT", "PATCH", "OPTIONS"]);
}

// ============================================================================
// DestroyAPIView Tests
// ============================================================================

#[test]
fn test_destroy_api_view_new() {
	// Arrange & Act
	let view = DestroyAPIView::<TestArticle>::new();

	// Assert
	let methods = view.allowed_methods();
	assert_eq!(methods, vec!["DELETE", "OPTIONS"]);
}

#[test]
fn test_destroy_api_view_with_lookup_field() {
	// Arrange & Act
	let view = DestroyAPIView::<TestArticle>::new().with_lookup_field("slug".to_string());

	// Assert
	let methods = view.allowed_methods();
	assert_eq!(methods, vec!["DELETE", "OPTIONS"]);
}

// ============================================================================
// Composite API View Tests
// ============================================================================

#[test]
fn test_list_create_api_view_new() {
	// Arrange & Act
	let view = ListCreateAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	// Assert
	let methods = view.allowed_methods();
	assert_eq!(methods, vec!["GET", "HEAD", "POST", "OPTIONS"]);
}

#[test]
fn test_retrieve_update_api_view_new() {
	// Arrange & Act
	let view = RetrieveUpdateAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	// Assert
	let methods = view.allowed_methods();
	assert_eq!(methods, vec!["GET", "HEAD", "PUT", "PATCH", "OPTIONS"]);
}

#[test]
fn test_retrieve_destroy_api_view_new() {
	// Arrange & Act
	let view = RetrieveDestroyAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	// Assert
	let methods = view.allowed_methods();
	assert_eq!(methods, vec!["GET", "HEAD", "DELETE", "OPTIONS"]);
}

#[test]
fn test_retrieve_update_destroy_api_view_new() {
	// Arrange & Act
	let view = RetrieveUpdateDestroyAPIView::<TestArticle, JsonSerializer<TestArticle>>::new();

	// Assert
	let methods = view.allowed_methods();
	assert_eq!(
		methods,
		vec!["GET", "HEAD", "PUT", "PATCH", "DELETE", "OPTIONS"]
	);
}
