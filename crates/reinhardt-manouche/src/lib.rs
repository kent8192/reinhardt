//! Manouche DSL - Domain Specific Language for reinhardt-pages.
//!
//! This crate provides the Abstract Syntax Tree (AST) structures, parsing logic,
//! validation, and code generation infrastructure for the `page!`, `form!`, and
//! `head!` macros.
//!
//! The name "manouche" comes from [Manouche Jazz](https://en.wikipedia.org/wiki/Gypsy_jazz),
//! a genre of music created by Django Reinhardt in the 1930s.
//!
//! ## Modules
//!
//! - [`core`] - DSL types, Untyped/Typed AST, reactive traits
//! - [`parser`] - TokenStream -> Untyped AST
//! - [`validator`] - Untyped AST -> Typed AST (semantic analysis)
//! - [`ir`] - Typed AST -> Intermediate Representation
//! - [`codegen`] - IRVisitor trait definition
//!
//! ## Pipeline
//!
//! ```text
//! TokenStream -> parse -> Untyped AST -> validate -> Typed AST -> lower -> IR -> visit -> TokenStream
//! ```

pub mod codegen;
pub mod core;
pub mod ir;
pub mod parser;
pub mod validator;

// Convenience re-exports from core
pub use core::*;
