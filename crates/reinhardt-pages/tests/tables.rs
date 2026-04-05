#![cfg(not(target_arch = "wasm32"))]
//! Integration tests for tables module

#[path = "tables/column_test.rs"]
mod column_test;
