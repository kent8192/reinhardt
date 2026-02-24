//! todos application module
//!
//! A simple TODO list application demonstrating database integration

use reinhardt::app_config;

pub mod admin;
pub mod models;
pub mod urls;
pub mod views;

#[app_config(name = "todos", label = "todos")]
pub struct TodosConfig;
