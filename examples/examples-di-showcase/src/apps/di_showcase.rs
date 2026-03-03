//! DI Showcase application module
//!
//! Demonstrates Reinhardt's dependency injection patterns.

use reinhardt::app_config;

#[path = "di_showcase/services.rs"]
pub mod services;

#[path = "di_showcase/urls.rs"]
pub mod urls;

#[path = "di_showcase/views.rs"]
pub mod views;

#[app_config(name = "di_showcase", label = "DI Showcase")]
pub struct DiShowcaseConfig;
