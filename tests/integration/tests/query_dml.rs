//! DML Integration Tests for reinhardt-query
//!
//! This is the entry point for all DML operation tests.
//! These tests were moved from reinhardt-query crate to avoid
//! circular dev-dependency (reinhardt-query <-> reinhardt-db).

#[path = "query_dml/fixtures.rs"]
mod fixtures;

#[path = "query_dml/common_assertions.rs"]
mod common_assertions;
#[path = "query_dml/delete_combinations.rs"]
mod delete_combinations;
#[path = "query_dml/delete_edge_cases.rs"]
mod delete_edge_cases;
#[path = "query_dml/delete_error_path.rs"]
mod delete_error_path;
#[path = "query_dml/delete_happy_path.rs"]
mod delete_happy_path;
#[path = "query_dml/insert_combinations.rs"]
mod insert_combinations;
#[path = "query_dml/insert_edge_cases.rs"]
mod insert_edge_cases;
#[path = "query_dml/insert_error_path.rs"]
mod insert_error_path;
#[path = "query_dml/insert_happy_path.rs"]
mod insert_happy_path;
#[path = "query_dml/select_aggregations.rs"]
mod select_aggregations;
#[path = "query_dml/select_combinations.rs"]
mod select_combinations;
#[path = "query_dml/select_edge_cases.rs"]
mod select_edge_cases;
#[path = "query_dml/select_error_path.rs"]
mod select_error_path;
#[path = "query_dml/select_happy_path.rs"]
mod select_happy_path;
#[path = "query_dml/select_joins.rs"]
mod select_joins;
#[path = "query_dml/update_combinations.rs"]
mod update_combinations;
#[path = "query_dml/update_edge_cases.rs"]
mod update_edge_cases;
#[path = "query_dml/update_error_path.rs"]
mod update_error_path;
#[path = "query_dml/update_happy_path.rs"]
mod update_happy_path;
