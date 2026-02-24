//! # Reinhardt DB Macros
//!
//! Procedural macros for the Reinhardt database layer.
//!
//! This crate provides attribute macros for:
//! - NoSQL documents: `#[document(...)]`
//!
//! ## NoSQL ODM Example
//!
//! ```rust,ignore
//! use reinhardt_db_macros::document;
//! use bson::oid::ObjectId;
//!
//! #[document(collection = "users", backend = "mongodb")]
//! struct User {
//!     #[field(primary_key)]
//!     id: ObjectId,
//!     #[field(required, unique)]
//!     email: String,
//!     name: String,
//! }
//! ```
//!
//! Note: The `#[field(...)]` attribute is parsed by the `#[document]` macro
//! and does not need to be imported separately.

use proc_macro::TokenStream;

mod document;
mod field;

/// Document macro for NoSQL ODM
///
/// Generates the `Document` trait implementation for structs representing
/// NoSQL database documents (e.g., MongoDB collections).
///
/// ## Attributes
///
/// - `collection` - Collection/table name (required)
/// - `backend` - Database backend: "mongodb" (required)
/// - `database` - Database name (optional, defaults to "default")
///
/// ## Field Attributes
///
/// Fields can be annotated with `#[field(...)]` to provide metadata:
/// - `primary_key` - Mark as primary key (required for one field)
/// - `required` - Field is required (non-null)
/// - `unique` - Field must be unique
/// - `index` - Create index on this field
/// - `default` - Default value expression
/// - `rename` - Rename field in database
/// - `min` / `max` - Numeric range constraints
///
/// ## Example
///
/// ```rust,ignore
/// use reinhardt_db_macros::document;
/// use bson::oid::ObjectId;
///
/// #[document(collection = "users", backend = "mongodb")]
/// struct User {
///     #[field(primary_key)]
///     id: ObjectId,
///     #[field(required, unique)]
///     email: String,
/// }
/// ```
#[proc_macro_attribute]
pub fn document(attr: TokenStream, item: TokenStream) -> TokenStream {
	document::document_impl(attr, item)
}
