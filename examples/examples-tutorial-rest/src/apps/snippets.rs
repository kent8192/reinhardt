//! Snippets application - Tutorial REST example
//!
//! This app demonstrates:
//! - RESTful API design
//! - Serialization and validation
//! - CRUD operations
//! - ViewSets

use reinhardt::app_config;

pub mod models;
pub mod serializers;
pub mod urls;
pub mod views;

#[app_config(name = "snippets", label = "snippets")]
pub struct SnippetsConfig;
