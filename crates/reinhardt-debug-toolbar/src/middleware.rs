//! Middleware components
//!
//! This module provides Tower/Axum middleware integration for the debug toolbar.

pub mod config;
pub mod layer;
pub mod service;

pub use config::ToolbarConfig;
pub use layer::DebugToolbarLayer;
pub use service::DebugToolbarService;
