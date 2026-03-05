// Auto-generated module file for di integration tests
// Each test file in di/ subdirectory is explicitly included with #[path] attribute

// Test helpers
#[path = "di/test_helpers.rs"]
mod test_helpers;

#[path = "di/core_integration.rs"]
mod core_integration;

#[path = "di/macros_integration.rs"]
mod macros_integration;

#[path = "di/core_error_handling.rs"]
mod core_error_handling;

#[path = "di/macros_advanced.rs"]
mod macros_advanced;

#[path = "di/auto_injection_basic.rs"]
mod auto_injection_basic;

#[path = "di/circular_dependency_detection.rs"]
mod circular_dependency_detection;

#[path = "di/performance_benchmarks.rs"]
mod performance_benchmarks;

// Phase 3: Cross-crate integration tests
#[path = "di/cross_crate_injection.rs"]
mod cross_crate_injection;

#[path = "di/database_integration.rs"]
mod database_integration;

#[path = "di/server_integration.rs"]
mod server_integration;

// Unit tests migrated from reinhardt-di to break circular publish dependency
#[path = "di/context_tests.rs"]
mod context_tests;

#[path = "di/depends_tests.rs"]
mod depends_tests;

#[path = "di/function_handle_tests.rs"]
mod function_handle_tests;

#[path = "di/injectable_tests.rs"]
mod injectable_tests;

#[path = "di/injected_tests.rs"]
mod injected_tests;

#[path = "di/provider_tests.rs"]
mod provider_tests;

#[path = "di/registry_tests.rs"]
mod registry_tests;
