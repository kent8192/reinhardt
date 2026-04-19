// Auto-generated module file for server integration tests
// Each test file in server/ subdirectory is explicitly included with #[path] attribute

#[path = "server/e2e_graphql_integration.rs"]
mod e2e_graphql_integration;

#[path = "server/graceful_shutdown_integration.rs"]
mod graceful_shutdown_integration;

#[path = "server/http2_integration.rs"]
mod http2_integration;

#[path = "server/server_advanced_integration.rs"]
mod server_advanced_integration;

#[path = "server/server_middleware_integration_tests.rs"]
mod server_middleware_integration_tests;

#[path = "server/server_test_helpers.rs"]
mod server_test_helpers;

#[path = "server/server_tests.rs"]
mod server_tests;

#[path = "server/http_advanced_integration.rs"]
mod http_advanced_integration;

#[path = "server/rate_limit_strategies_integration.rs"]
mod rate_limit_strategies_integration;

#[path = "server/websocket_advanced_integration.rs"]
mod websocket_advanced_integration;

#[path = "server/http2_advanced_integration.rs"]
mod http2_advanced_integration;

#[path = "server/middleware_error_handling_integration.rs"]
mod middleware_error_handling_integration;

#[path = "server/server_error_scenarios_integration.rs"]
mod server_error_scenarios_integration;

#[path = "server/combined_features_integration.rs"]
mod combined_features_integration;

#[path = "server/edge_cases_integration.rs"]
mod edge_cases_integration;

#[path = "server/use_case_integration.rs"]
mod use_case_integration;

#[path = "server/http_response_pipeline_integration.rs"]
mod http_response_pipeline_integration;

#[cfg(feature = "graphql")]
#[path = "server/graphql_advanced_integration.rs"]
mod graphql_advanced_integration;
