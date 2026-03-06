//! # Reinhardt Debug Toolbar
//!
//! A debug toolbar for the Reinhardt web framework, inspired by Django Debug Toolbar.
//!
//! This crate provides comprehensive development-time debugging capabilities including:
//! - SQL query inspection with duplicate detection and N+1 query warnings
//! - Request/Response information display
//! - Template rendering insights (with reinhardt-pages integration)
//! - Cache statistics and hit/miss rates
//! - Performance profiling and timeline visualization
//!
//! ## Features
//!
//! - `sql-panel` - SQL query debugging panel
//! - `template-panel` - Template rendering panel
//! - `cache-panel` - Cache statistics panel
//! - `performance-panel` - Performance profiling panel
//! - `full` - All panels enabled
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use reinhardt_debug_toolbar::{DebugToolbarLayer, ToolbarConfig};
//! use axum::Router;
//! use std::net::IpAddr;
//!
//! let config = ToolbarConfig {
//!     enabled: true,
//!     internal_ips: vec!["127.0.0.1".parse().unwrap()],
//!     ..Default::default()
//! };
//!
//! let app = Router::new()
//!     .route("/", get(handler))
//!     .layer(DebugToolbarLayer::new(config));
//! ```
//!
//! ## Architecture
//!
//! The toolbar follows a layered architecture:
//!
//! 1. **Middleware Layer**: Request/response interception using Tower middleware
//! 2. **Collection Layer**: Data collection from framework components
//! 3. **Panel Layer**: Statistics generation and UI rendering
//! 4. **UI Layer**: HTML/CSS/JS rendering and injection
//!
//! ## Zero-Cost Abstraction
//!
//! The toolbar uses `#[cfg(debug_assertions)]` for conditional compilation,
//! ensuring zero runtime overhead in release builds.

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]

// Module declarations following Rust 2024 module system (no mod.rs)
pub mod collectors;
pub mod context;
pub mod error;
pub mod middleware;
pub mod panels;
pub mod ui;
pub mod utils;

// Re-export main types
pub use context::{TOOLBAR_CONTEXT, ToolbarContext};
pub use error::{ToolbarError, ToolbarResult};
pub use middleware::{DebugToolbarLayer, DebugToolbarService, ToolbarConfig};
pub use panels::{Panel, PanelRegistry, PanelStats};

// Re-export panel implementations (feature-gated)
#[cfg(feature = "sql-panel")]
pub use panels::sql::SqlPanel;

pub use panels::request::RequestPanel;

// Note: These panels will be re-exported once implemented
// #[cfg(feature = "template-panel")]
// pub use panels::templates::TemplatesPanel;

// #[cfg(feature = "cache-panel")]
// pub use panels::cache::CachePanel;

// #[cfg(feature = "performance-panel")]
// pub use panels::performance::PerformancePanel;
