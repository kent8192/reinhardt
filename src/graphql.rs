//! GraphQL API module.
//!
//! This module provides GraphQL support including schema definition,
//! resolvers, subscriptions, and context management.
//!
//! # Examples
//!
//! ```rust,no_run
//! use reinhardt::graphql::{AppSchema, Query, Mutation, create_schema};
//! ```

#[cfg(feature = "graphql")]
pub use reinhardt_graphql::*;
