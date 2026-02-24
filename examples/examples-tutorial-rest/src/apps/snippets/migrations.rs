//! Database migrations for snippets app
//!
//! This module contains all database migrations for the snippets application.
//! Migrations are applied in order based on their numeric prefix.

pub mod _0001_initial;

pub use _0001_initial::Migration as Migration0001Initial;
