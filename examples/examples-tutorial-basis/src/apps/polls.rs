//! Polls application - Tutorial basis example
//!
//! This app demonstrates:
//! - Database models (Question, Choice)
//! - Views and URL routing
//! - Forms and generic views
//! - Admin panel integration

use reinhardt::app_config;

pub mod models;
pub mod serializers;
pub mod urls;
pub mod views;

#[app_config(name = "polls", label = "polls")]
pub struct PollsConfig;
