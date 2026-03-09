//! # Reinhardt Views
//!
//! Generic views for Reinhardt framework, inspired by Django's class-based views.
//!
//! ## Features
//!
//! - **ListView**: Display a list of objects with pagination support
//! - **DetailView**: Display a single object
//! - **CreateView**: Handle object creation
//! - **UpdateView**: Handle object updates
//! - **DeleteView**: Handle object deletion
//! - **Browsable API**: HTML rendering for interactive API exploration
//! - **Interactive Docs**: Swagger UI-like documentation interface
//! - **Form Generation**: Automatic form generation for POST/PUT/PATCH methods
//! - **Syntax Highlighting**: JSON response highlighting with customizable color schemes
//!
//! ## Example
//!
//! ```rust,ignore
//! use reinhardt_views::{ListView, DetailView};
//! # use reinhardt_http::{Request, Response};
//! # use reinhardt_core::exception::Error;
//!
//! # #[derive(Debug, Clone)]
//! # struct User {
//! #     id: Option<i64>,
//! #     username: String,
//! #     email: String,
//! # }
//!
//! // Create a ListView to display paginated users
//! let list_view = ListView::<User>::new()
//!     .with_paginate_by(10)
//!     .with_ordering(vec!["-id".to_string()]);
//!
//! // Create a DetailView to display a single user
//! let detail_view = DetailView::<User>::new()
//!     .with_context_object_name("user");
//!
//! // Use the views in request handlers
//! # async fn handle_list(request: Request) -> Result<Response, Error> {
//! #     list_view.dispatch(request).await
//! # }
//! #
//! # async fn handle_detail(request: Request) -> Result<Response, Error> {
//! #     detail_view.dispatch(request).await
//! # }
//! ```
#![warn(missing_docs)]

pub mod viewsets;

// Module declarations from merged views-core
pub mod admin;
pub mod browsable_api;
pub mod generic;
pub mod openapi;
pub mod openapi_inspector;

// Module declarations
mod core;
mod detail;
mod list;
mod mixins;

// Re-export public API
pub use core::{Context, View};
pub use detail::DetailView;
pub use list::ListView;
pub use mixins::{MultipleObjectMixin, SingleObjectMixin};

// Re-export viewsets commonly used types at crate root
pub use viewsets::{Action, ViewSet};

// Re-export Generic API Views
pub use generic::{
	CreateAPIView, DestroyAPIView, ListAPIView, ListCreateAPIView, RetrieveAPIView,
	RetrieveDestroyAPIView, RetrieveUpdateAPIView, RetrieveUpdateDestroyAPIView, UpdateAPIView,
};
