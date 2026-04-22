#![warn(missing_docs)]
//! Manouche DSL - front-end for the reinhardt-pages macro family.
//!
//! This crate provides the Abstract Syntax Tree (AST) structures,
//! parsing logic, and semantic validation for the `page!`, `form!`,
//! and `head!` macros. Final code generation is performed by
//! downstream consumers (e.g. `reinhardt-pages/macros`) directly from
//! the Typed AST.
//!
//! The name "manouche" comes from [Manouche Jazz](https://en.wikipedia.org/wiki/Gypsy_jazz),
//! a genre of music created by Django Reinhardt in the 1930s.
//!
//! ## Modules
//!
//! - [`core`] - DSL types, Untyped/Typed AST, reactive traits
//! - [`parser`] - TokenStream -> Untyped AST
//! - [`validator`] - Untyped AST -> Typed AST (semantic analysis)
//!
//! ## Pipeline
//!
//! ```text
//! TokenStream -> parse -> Untyped AST -> validate -> Typed AST
//! ```

pub mod core;
pub mod parser;
pub mod validator;

// Convenience re-exports from core
pub use core::*;
