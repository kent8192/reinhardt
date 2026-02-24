//! Materialized view DDL statement builders
//!
//! This module provides builders for materialized view-related DDL statements:
//!
//! - CREATE MATERIALIZED VIEW: [`CreateMaterializedViewStatement`]
//! - ALTER MATERIALIZED VIEW: [`AlterMaterializedViewStatement`]
//! - DROP MATERIALIZED VIEW: [`DropMaterializedViewStatement`]
//! - REFRESH MATERIALIZED VIEW: [`RefreshMaterializedViewStatement`]

mod alter_materialized_view;
mod create_materialized_view;
mod drop_materialized_view;
mod refresh_materialized_view;

pub use alter_materialized_view::*;
pub use create_materialized_view::*;
pub use drop_materialized_view::*;
pub use refresh_materialized_view::*;
