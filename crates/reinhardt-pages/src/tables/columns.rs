//! Column type implementations
//!
//! This module provides various column types for different data rendering:
//! - `Column<T>`: Basic column for any type
//! - `LinkColumn`: Column with hyperlink
//! - `BooleanColumn`: Column for boolean values (checkmark/X)
//! - `DateTimeColumn`: Column for date/time formatting
//! - `EmailColumn`: Column for email addresses with mailto links
//! - `ChoiceColumn`: Column for choice fields
//! - `TemplateColumn`: Column with custom template
//! - `JSONColumn`: Column for JSON data
//! - `CheckBoxColumn`: Column with checkbox
//! - `URLColumn`: Column for URLs

pub mod basic;
pub mod boolean;
pub mod checkbox;
pub mod choice;
pub mod datetime;
pub mod email;
pub mod json;
pub mod link;
pub mod template;
pub mod url;

// Re-exports
pub use basic::Column;
pub use boolean::BooleanColumn;
pub use checkbox::CheckBoxColumn;
pub use choice::ChoiceColumn;
pub use datetime::DateTimeColumn;
pub use email::EmailColumn;
pub use json::JSONColumn;
pub use link::LinkColumn;
pub use template::TemplateColumn;
pub use url::URLColumn;
