//! Table utilities for reinhardt-pages (django-tables2 equivalent)
//!
//! This module provides table rendering with various column types,
//! sorting, pagination, filtering, and export functionality.
//!
//! # Features
//!
//! - Multiple column types: `Column<T>`, `LinkColumn`, `BooleanColumn`,
//!   `CheckBoxColumn`, `DateTimeColumn`, `EmailColumn`, `ChoiceColumn`,
//!   `TemplateColumn`, `JSONColumn`, `URLColumn`
//! - Sorting with `SortDirection` and `Sortable` trait
//! - Pagination with `Pagination` struct
//! - Filtering with `Filterable` trait
//! - Export to CSV and JSON
//!
//! # Example
//!
//! ```rust,ignore
//! use reinhardt_pages::tables::columns::{Column, LinkColumn, BooleanColumn};
//!
//! let name_col = Column::<String>::new("name", "Name");
//! let link_col = LinkColumn::new("id", "Profile", "/users/{id}");
//! let active_col = BooleanColumn::new("is_active", "Active");
//! ```

pub mod column;
pub mod columns;
pub mod export;
pub mod filtering;
pub mod pagination;
pub mod sorting;
pub mod table;

// Re-exports for convenience
pub use column::Column as ColumnTrait;
pub use columns::*;
pub use export::{ExportFormat, Exportable};
pub use filtering::Filterable;
pub use pagination::Pagination;
pub use sorting::{SortDirection, Sortable};
pub use table::Table;
