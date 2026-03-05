//! Data table rendering utilities for Reinhardt
//!
//! This crate provides Django-tables2 equivalent functionality for Reinhardt,
//! enabling table definitions with sorting, pagination, and filtering.
//!
//! # Features
//!
//! - **Table Definition**: Table definition using the `Table` trait and `SimpleTable`
//! - **Column Types**: `BaseColumn` with customizable render functions
//! - **Sorting**: Programmatic sorting by column with ascending/descending order
//! - **Pagination**: Configurable page size and page navigation
//! - **Filtering**: Column-level filtering with string matching
//! - **Integration**: Integration with `reinhardt-pages` (requires `pages-integration` feature)
//!
//! # Architecture
//!
//! ```mermaid
//! graph TD
//!     A[Table] --> B[Columns]
//!     A --> C[Rows]
//!     A --> D[Sort Config]
//!     A --> E[Pagination]
//!     A --> F[Filters]
//!     B --> G[BaseColumn]
//! ```
//!
//! # Example
//!
//! ```rust
//! use reinhardt_tables::{Table, Column};
//!
//! struct User {
//!     id: i32,
//!     name: String,
//!     active: bool,
//! }
//!
//! // Basic table usage will be demonstrated once core types are implemented
//! ```

#![warn(missing_docs)]
#![warn(rustdoc::broken_intra_doc_links)]

pub mod column;
pub mod error;
pub mod table;

// Re-exports for convenience
pub use column::Column;
pub use error::{Result, TableError};
pub use table::Table;
