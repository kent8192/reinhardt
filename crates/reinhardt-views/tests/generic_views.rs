//! Integration tests for generic API views (ListAPIView, CreateAPIView, etc.)

use reinhardt_db::orm::{FieldSelector, Model, QuerySet};
use reinhardt_rest::serializers::JsonSerializer;
use reinhardt_views::View;
use reinhardt_views::viewsets::{FilterConfig, PaginationConfig};
use reinhardt_views::{
	CreateAPIView, DestroyAPIView, ListAPIView, ListCreateAPIView, RetrieveAPIView,
	RetrieveDestroyAPIView, RetrieveUpdateAPIView, RetrieveUpdateDestroyAPIView, UpdateAPIView,
};
use rstest::rstest;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Test model fixtures
// ---------------------------------------------------------------------------

/// Simple test model used across multiple tests
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Article {
	id: Option<i64>,
	title: String,
	content: String,
}

#[derive(Clone)]
struct ArticleFields;

impl FieldSelector for ArticleFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

impl Model for Article {
	type PrimaryKey = i64;
	type Fields = ArticleFields;

	fn table_name() -> &'static str {
		"articles"
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}

	fn new_fields() -> Self::Fields {
		ArticleFields
	}
}

/// Secondary test model to verify generic behaviour is model-agnostic
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Comment {
	id: Option<i64>,
	body: String,
}

#[derive(Clone)]
struct CommentFields;

impl FieldSelector for CommentFields {
	fn with_alias(self, _alias: &str) -> Self {
		self
	}
}

impl Model for Comment {
	type PrimaryKey = i64;
	type Fields = CommentFields;

	fn table_name() -> &'static str {
		"comments"
	}

	fn primary_key(&self) -> Option<Self::PrimaryKey> {
		self.id
	}

	fn set_primary_key(&mut self, value: Self::PrimaryKey) {
		self.id = Some(value);
	}

	fn new_fields() -> Self::Fields {
		CommentFields
	}
}

// ---------------------------------------------------------------------------
// ListAPIView tests
// ---------------------------------------------------------------------------

#[rstest]
fn list_api_view_new_creates_instance() {
	// Act
	let view = ListAPIView::<Article, JsonSerializer<Article>>::new();

	// Assert
	let methods = view.allowed_methods();
	assert!(methods.contains(&"GET"));
}

#[rstest]
fn list_api_view_default_equals_new() {
	// Arrange
	let from_new: ListAPIView<Article, JsonSerializer<Article>> = ListAPIView::new();
	let from_default: ListAPIView<Article, JsonSerializer<Article>> = Default::default();

	// Assert
	assert_eq!(from_new.allowed_methods(), from_default.allowed_methods());
}

#[rstest]
fn list_api_view_allowed_methods_include_get_and_head() {
	// Arrange
	let view = ListAPIView::<Article, JsonSerializer<Article>>::new();

	// Act
	let methods = view.allowed_methods();

	// Assert
	assert!(methods.contains(&"GET"));
	assert!(methods.contains(&"HEAD"));
	assert!(methods.contains(&"OPTIONS"));
}

#[rstest]
fn list_api_view_with_paginate_by_compiles_and_allows_get() {
	// Arrange
	let page_size = 20_usize;

	// Act
	let view = ListAPIView::<Article, JsonSerializer<Article>>::new().with_paginate_by(page_size);

	// Assert
	let methods = view.allowed_methods();
	assert!(methods.contains(&"GET"));
}

#[rstest]
fn list_api_view_with_ordering_accepts_asc_fields() {
	// Arrange
	let ordering = vec!["title".to_string(), "id".to_string()];

	// Act
	let view = ListAPIView::<Article, JsonSerializer<Article>>::new().with_ordering(ordering);

	// Assert
	let methods = view.allowed_methods();
	assert!(methods.contains(&"GET"));
}

#[rstest]
fn list_api_view_with_ordering_accepts_desc_fields() {
	// Arrange
	let ordering = vec!["-created_at".to_string(), "-id".to_string()];

	// Act
	let view = ListAPIView::<Article, JsonSerializer<Article>>::new().with_ordering(ordering);

	// Assert
	assert!(view.allowed_methods().contains(&"GET"));
}

#[rstest]
fn list_api_view_with_pagination_config_page_number() {
	// Arrange
	let config = PaginationConfig::page_number(50, Some(200));

	// Act
	let view = ListAPIView::<Article, JsonSerializer<Article>>::new().with_pagination(config);

	// Assert
	assert!(view.allowed_methods().contains(&"GET"));
}

#[rstest]
fn list_api_view_with_pagination_config_limit_offset() {
	// Arrange
	let config = PaginationConfig::limit_offset(25, Some(500));

	// Act
	let view = ListAPIView::<Article, JsonSerializer<Article>>::new().with_pagination(config);

	// Assert
	assert!(view.allowed_methods().contains(&"GET"));
}

#[rstest]
fn list_api_view_with_pagination_config_none() {
	// Arrange
	let config = PaginationConfig::none();

	// Act
	let view = ListAPIView::<Article, JsonSerializer<Article>>::new().with_pagination(config);

	// Assert
	assert!(view.allowed_methods().contains(&"GET"));
}

#[rstest]
fn list_api_view_with_filter_config() {
	// Arrange
	let filter = FilterConfig::new()
		.with_filterable_fields(vec!["title", "content"])
		.with_search_fields(vec!["title"]);

	// Act
	let view = ListAPIView::<Article, JsonSerializer<Article>>::new().with_filter_config(filter);

	// Assert
	assert!(view.allowed_methods().contains(&"GET"));
}

#[rstest]
fn list_api_view_works_with_different_model() {
	// Act
	let view = ListAPIView::<Comment, JsonSerializer<Comment>>::new().with_paginate_by(5);

	// Assert
	let methods = view.allowed_methods();
	assert!(methods.contains(&"GET"));
	assert!(methods.contains(&"HEAD"));
}

// ---------------------------------------------------------------------------
// CreateAPIView tests
// ---------------------------------------------------------------------------

#[rstest]
fn create_api_view_new_creates_instance() {
	// Act
	let view = CreateAPIView::<Article, JsonSerializer<Article>>::new();

	// Assert
	let methods = view.allowed_methods();
	assert!(methods.contains(&"POST"));
}

#[rstest]
fn create_api_view_default_equals_new() {
	// Arrange
	let from_new: CreateAPIView<Article, JsonSerializer<Article>> = CreateAPIView::new();
	let from_default: CreateAPIView<Article, JsonSerializer<Article>> = Default::default();

	// Assert
	assert_eq!(from_new.allowed_methods(), from_default.allowed_methods());
}

#[rstest]
fn create_api_view_allowed_methods_include_post_and_options() {
	// Arrange
	let view = CreateAPIView::<Article, JsonSerializer<Article>>::new();

	// Act
	let methods = view.allowed_methods();

	// Assert
	assert!(methods.contains(&"POST"));
	assert!(methods.contains(&"OPTIONS"));
	assert!(!methods.contains(&"GET"));
}

#[rstest]
fn create_api_view_with_queryset_accepts_queryset() {
	// Arrange
	let qs = QuerySet::<Article>::new();

	// Act
	let view = CreateAPIView::<Article, JsonSerializer<Article>>::new().with_queryset(qs);

	// Assert
	assert!(view.allowed_methods().contains(&"POST"));
}

// ---------------------------------------------------------------------------
// RetrieveAPIView tests
// ---------------------------------------------------------------------------

#[rstest]
fn retrieve_api_view_new_creates_instance() {
	// Act
	let view = RetrieveAPIView::<Article, JsonSerializer<Article>>::new();

	// Assert
	let methods = view.allowed_methods();
	assert!(methods.contains(&"GET"));
}

#[rstest]
fn retrieve_api_view_default_allows_get_and_head() {
	// Act
	let view = RetrieveAPIView::<Article, JsonSerializer<Article>>::new();

	// Assert
	assert!(view.allowed_methods().contains(&"GET"));
	assert!(view.allowed_methods().contains(&"HEAD"));
}

#[rstest]
fn retrieve_api_view_with_custom_lookup_field() {
	// Act
	let view = RetrieveAPIView::<Article, JsonSerializer<Article>>::new()
		.with_lookup_field("slug".to_string());

	// Assert
	assert!(view.allowed_methods().contains(&"GET"));
}

#[rstest]
fn retrieve_api_view_allowed_methods_exclude_post() {
	// Arrange
	let view = RetrieveAPIView::<Article, JsonSerializer<Article>>::new();

	// Act
	let methods = view.allowed_methods();

	// Assert
	assert!(!methods.contains(&"POST"));
	assert!(!methods.contains(&"DELETE"));
}

// ---------------------------------------------------------------------------
// UpdateAPIView tests
// ---------------------------------------------------------------------------

#[rstest]
fn update_api_view_new_creates_instance() {
	// Act
	let view = UpdateAPIView::<Article, JsonSerializer<Article>>::new();

	// Assert
	let methods = view.allowed_methods();
	assert!(methods.contains(&"PUT"));
}

#[rstest]
fn update_api_view_allowed_methods_include_put_and_patch() {
	// Arrange
	let view = UpdateAPIView::<Article, JsonSerializer<Article>>::new();

	// Act
	let methods = view.allowed_methods();

	// Assert
	assert!(methods.contains(&"PUT"));
	assert!(methods.contains(&"PATCH"));
	assert!(methods.contains(&"OPTIONS"));
}

#[rstest]
fn update_api_view_with_lookup_field_accepts_custom_field() {
	// Act
	let view = UpdateAPIView::<Article, JsonSerializer<Article>>::new()
		.with_lookup_field("slug".to_string());

	// Assert
	assert!(view.allowed_methods().contains(&"PUT"));
}

#[rstest]
fn update_api_view_with_partial_enabled() {
	// Act
	let view = UpdateAPIView::<Article, JsonSerializer<Article>>::new().with_partial(true);

	// Assert
	assert!(view.allowed_methods().contains(&"PATCH"));
}

#[rstest]
fn update_api_view_with_partial_disabled() {
	// Act
	let view = UpdateAPIView::<Article, JsonSerializer<Article>>::new().with_partial(false);

	// Assert
	assert!(view.allowed_methods().contains(&"PUT"));
}

// ---------------------------------------------------------------------------
// DestroyAPIView tests
// ---------------------------------------------------------------------------

#[rstest]
fn destroy_api_view_new_creates_instance() {
	// Act
	let view = DestroyAPIView::<Article>::new();

	// Assert
	let methods = view.allowed_methods();
	assert!(methods.contains(&"DELETE"));
}

#[rstest]
fn destroy_api_view_default_equals_new() {
	// Arrange
	let from_new: DestroyAPIView<Article> = DestroyAPIView::new();
	let from_default: DestroyAPIView<Article> = Default::default();

	// Assert
	assert_eq!(from_new.allowed_methods(), from_default.allowed_methods());
}

#[rstest]
fn destroy_api_view_allowed_methods_exclude_get() {
	// Arrange
	let view = DestroyAPIView::<Article>::new();

	// Act
	let methods = view.allowed_methods();

	// Assert
	assert!(methods.contains(&"DELETE"));
	assert!(!methods.contains(&"GET"));
	assert!(!methods.contains(&"POST"));
}

#[rstest]
fn destroy_api_view_with_lookup_field_sets_field() {
	// Act
	let view = DestroyAPIView::<Article>::new().with_lookup_field("uuid".to_string());

	// Assert
	assert!(view.allowed_methods().contains(&"DELETE"));
}

// ---------------------------------------------------------------------------
// Composite view tests
// ---------------------------------------------------------------------------

#[rstest]
fn list_create_api_view_new_creates_instance() {
	// Act
	let view = ListCreateAPIView::<Article, JsonSerializer<Article>>::new();

	// Assert
	let methods = view.allowed_methods();
	assert!(methods.contains(&"GET"));
	assert!(methods.contains(&"POST"));
}

#[rstest]
fn list_create_api_view_allowed_methods_include_get_post_head() {
	// Arrange
	let view = ListCreateAPIView::<Article, JsonSerializer<Article>>::new();

	// Act
	let methods = view.allowed_methods();

	// Assert
	assert!(methods.contains(&"GET"));
	assert!(methods.contains(&"HEAD"));
	assert!(methods.contains(&"POST"));
	assert!(methods.contains(&"OPTIONS"));
}

#[rstest]
fn list_create_api_view_with_paginate_by() {
	// Act
	let view = ListCreateAPIView::<Article, JsonSerializer<Article>>::new().with_paginate_by(10);

	// Assert
	let methods = view.allowed_methods();
	assert!(methods.contains(&"GET"));
	assert!(methods.contains(&"POST"));
}

#[rstest]
fn list_create_api_view_with_ordering() {
	// Act
	let view = ListCreateAPIView::<Article, JsonSerializer<Article>>::new()
		.with_ordering(vec!["-id".to_string()]);

	// Assert
	assert!(view.allowed_methods().contains(&"GET"));
}

#[rstest]
fn retrieve_update_api_view_new_creates_instance() {
	// Act
	let view = RetrieveUpdateAPIView::<Article, JsonSerializer<Article>>::new();

	// Assert
	let methods = view.allowed_methods();
	assert!(methods.contains(&"GET"));
	assert!(methods.contains(&"PUT"));
	assert!(methods.contains(&"PATCH"));
}

#[rstest]
fn retrieve_update_api_view_with_lookup_field() {
	// Act
	let view = RetrieveUpdateAPIView::<Article, JsonSerializer<Article>>::new()
		.with_lookup_field("slug".to_string());

	// Assert
	let methods = view.allowed_methods();
	assert!(methods.contains(&"GET"));
	assert!(methods.contains(&"PUT"));
}

#[rstest]
fn retrieve_destroy_api_view_new_creates_instance() {
	// Act
	let view = RetrieveDestroyAPIView::<Article, JsonSerializer<Article>>::new();

	// Assert
	let methods = view.allowed_methods();
	assert!(methods.contains(&"GET"));
	assert!(methods.contains(&"DELETE"));
}

#[rstest]
fn retrieve_destroy_api_view_allowed_methods_exclude_post() {
	// Arrange
	let view = RetrieveDestroyAPIView::<Article, JsonSerializer<Article>>::new();

	// Act
	let methods = view.allowed_methods();

	// Assert
	assert!(methods.contains(&"GET"));
	assert!(methods.contains(&"DELETE"));
	assert!(!methods.contains(&"POST"));
	assert!(!methods.contains(&"PUT"));
}

#[rstest]
fn retrieve_update_destroy_api_view_new_creates_instance() {
	// Act
	let view = RetrieveUpdateDestroyAPIView::<Article, JsonSerializer<Article>>::new();

	// Assert
	let methods = view.allowed_methods();
	assert!(methods.contains(&"GET"));
	assert!(methods.contains(&"PUT"));
	assert!(methods.contains(&"PATCH"));
	assert!(methods.contains(&"DELETE"));
}

#[rstest]
fn retrieve_update_destroy_api_view_allowed_methods_include_all_crud() {
	// Arrange
	let view = RetrieveUpdateDestroyAPIView::<Article, JsonSerializer<Article>>::new();

	// Act
	let methods = view.allowed_methods();

	// Assert
	assert!(methods.contains(&"GET"));
	assert!(methods.contains(&"HEAD"));
	assert!(methods.contains(&"PUT"));
	assert!(methods.contains(&"PATCH"));
	assert!(methods.contains(&"DELETE"));
	assert!(methods.contains(&"OPTIONS"));
	// POST is not included (this is not a create+list view)
	assert!(!methods.contains(&"POST"));
}

#[rstest]
fn retrieve_update_destroy_api_view_with_lookup_field() {
	// Act
	let view = RetrieveUpdateDestroyAPIView::<Article, JsonSerializer<Article>>::new()
		.with_lookup_field("uuid".to_string());

	// Assert
	assert!(view.allowed_methods().contains(&"GET"));
	assert!(view.allowed_methods().contains(&"DELETE"));
}

// ---------------------------------------------------------------------------
// PaginationConfig builder tests
// ---------------------------------------------------------------------------

#[rstest]
fn pagination_config_page_number_stores_page_size() {
	// Act
	let config = PaginationConfig::page_number(30, Some(300));

	// Assert
	match config {
		PaginationConfig::PageNumber {
			page_size,
			max_page_size,
		} => {
			assert_eq!(page_size, 30);
			assert_eq!(max_page_size, Some(300));
		}
		_ => panic!("Expected PageNumber variant"),
	}
}

#[rstest]
fn pagination_config_limit_offset_stores_limits() {
	// Act
	let config = PaginationConfig::limit_offset(15, Some(150));

	// Assert
	match config {
		PaginationConfig::LimitOffset {
			default_limit,
			max_limit,
		} => {
			assert_eq!(default_limit, 15);
			assert_eq!(max_limit, Some(150));
		}
		_ => panic!("Expected LimitOffset variant"),
	}
}

#[rstest]
fn pagination_config_cursor_stores_page_size_and_field() {
	// Act
	let config = PaginationConfig::cursor(20, "created_at");

	// Assert
	match config {
		PaginationConfig::Cursor {
			page_size,
			ordering_field,
		} => {
			assert_eq!(page_size, 20);
			assert_eq!(ordering_field, "created_at");
		}
		_ => panic!("Expected Cursor variant"),
	}
}

#[rstest]
fn pagination_config_none_is_none_variant() {
	// Act
	let config = PaginationConfig::none();

	// Assert
	assert!(matches!(config, PaginationConfig::None));
}

#[rstest]
fn pagination_config_default_is_page_number() {
	// Act
	let config = PaginationConfig::default();

	// Assert
	match config {
		PaginationConfig::PageNumber { page_size, .. } => {
			assert_eq!(page_size, 10);
		}
		_ => panic!("Expected default to be PageNumber"),
	}
}

// ---------------------------------------------------------------------------
// FilterConfig builder tests
// ---------------------------------------------------------------------------

#[rstest]
fn filter_config_new_creates_empty_config() {
	// Act
	let config = FilterConfig::new();

	// Assert
	assert!(config.filterable_fields.is_empty());
	assert!(config.search_fields.is_empty());
	assert!(config.case_insensitive_search);
}

#[rstest]
fn filter_config_with_filterable_fields_stores_fields() {
	// Act
	let config = FilterConfig::new().with_filterable_fields(vec!["status", "category"]);

	// Assert
	assert_eq!(config.filterable_fields, vec!["status", "category"]);
}

#[rstest]
fn filter_config_with_search_fields_stores_fields() {
	// Act
	let config = FilterConfig::new().with_search_fields(vec!["title", "body"]);

	// Assert
	assert_eq!(config.search_fields, vec!["title", "body"]);
}

#[rstest]
fn filter_config_case_insensitive_can_be_disabled() {
	// Act
	let config = FilterConfig::new().case_insensitive(false);

	// Assert
	assert!(!config.case_insensitive_search);
}

#[rstest]
fn filter_config_builder_chain() {
	// Act
	let config = FilterConfig::new()
		.with_filterable_fields(vec!["status"])
		.with_search_fields(vec!["title"])
		.case_insensitive(false);

	// Assert
	assert_eq!(config.filterable_fields, vec!["status"]);
	assert_eq!(config.search_fields, vec!["title"]);
	assert!(!config.case_insensitive_search);
}
