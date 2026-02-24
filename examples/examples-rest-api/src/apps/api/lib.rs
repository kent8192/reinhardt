//! api application module
//!
//! A RESTful API application demonstrating REST features

use reinhardt::app_config;

#[cfg(feature = "with-admin")]
pub mod admin;
pub mod models;
pub mod serializers;
pub mod storage;
pub mod urls;
pub mod views;

#[app_config(name = "api", label = "api")]
pub struct ApiConfig;
