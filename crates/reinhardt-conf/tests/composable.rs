#[path = "composable/fixtures.rs"]
mod fixtures;

#[path = "composable/happy_path.rs"]
mod happy_path;

#[path = "composable/error_path.rs"]
mod error_path;

#[path = "composable/edge_cases.rs"]
mod edge_cases;

#[path = "composable/state_transitions.rs"]
mod state_transitions;

#[path = "composable/fuzz.rs"]
mod fuzz;

#[path = "composable/property_tests.rs"]
mod property_tests;

#[path = "composable/combination.rs"]
mod combination;

#[path = "composable/sanity.rs"]
mod sanity;

#[path = "composable/equivalence_partition.rs"]
mod equivalence_partition;

#[path = "composable/boundary_values.rs"]
mod boundary_values;

#[path = "composable/decision_table.rs"]
mod decision_table;
