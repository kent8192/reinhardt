//! Generic API Views
//!
//! This module provides production-ready generic API views for common CRUD operations.
//! These views are similar to Django REST Framework's generic views but designed for Rust.
//!
//! # Available Views
//!
//! ## Single Operation Views
//! - [`ListAPIView`] - List objects (GET)
//! - [`CreateAPIView`] - Create objects (POST)
//! - [`UpdateAPIView`] - Update objects (PUT/PATCH)
//! - [`DestroyAPIView`] - Delete objects (DELETE)
//!
//! ## Composite Views
//! - [`ListCreateAPIView`] - List and create (GET, POST)
//! - [`RetrieveUpdateAPIView`] - Retrieve and update (GET, PUT, PATCH)
//! - [`RetrieveDestroyAPIView`] - Retrieve and delete (GET, DELETE)
//! - [`RetrieveUpdateDestroyAPIView`] - Full CRUD except list (GET, PUT, PATCH, DELETE)
//!
//! # Examples
//!
//! ```rust,no_run
//! use reinhardt_views::ListAPIView;
//! use reinhardt_db::orm::Model;
//! use reinhardt_rest::serializers::JsonSerializer;
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Debug, Clone, Serialize, Deserialize)]
//! struct Article {
//!     id: Option<i64>,
//!     title: String,
//!     content: String,
//! }
//!
//! #[derive(Clone)]
//! struct ArticleFields;
//!
//! impl reinhardt_db::orm::FieldSelector for ArticleFields {
//!     fn with_alias(self, _alias: &str) -> Self {
//!         self
//!     }
//! }
//!
//! impl Model for Article {
//!     type PrimaryKey = i64;
//!     type Fields = ArticleFields;
//!     fn table_name() -> &'static str { "articles" }
//!     fn primary_key(&self) -> Option<Self::PrimaryKey> { self.id }
//!     fn set_primary_key(&mut self, value: Self::PrimaryKey) { self.id = Some(value); }
//!     fn new_fields() -> Self::Fields { ArticleFields }
//! }
//!
//! let view = ListAPIView::<Article, JsonSerializer<Article>>::new()
//!     .with_paginate_by(10)
//!     .with_ordering(vec!["-created_at".into()]);
//! ```

// Submodules (Rust 2024 Edition: use module.rs + module/ directory)
mod composite;
mod create_api;
mod destroy_api;
mod list_api;
mod retrieve_api;
mod update_api;

// Re-export all API views
pub use composite::{
	ListCreateAPIView, RetrieveDestroyAPIView, RetrieveUpdateAPIView, RetrieveUpdateDestroyAPIView,
};
pub use create_api::CreateAPIView;
pub use destroy_api::DestroyAPIView;
pub use list_api::ListAPIView;
pub use retrieve_api::RetrieveAPIView;
pub use update_api::UpdateAPIView;
