// Auto-generated module file for commands integration tests
// Each test file in commands/ subdirectory is explicitly included with #[path] attribute

#[path = "commands/custom_command_integration.rs"]
mod custom_command_integration;

#[path = "commands/system_check_integration.rs"]
mod system_check_integration;

#[path = "commands/template_integration.rs"]
mod template_integration;

// Specialized fixtures for command integration tests
#[path = "commands/fixtures.rs"]
mod fixtures;

// Database command integration tests
#[path = "commands/migrate_integration.rs"]
mod migrate_integration;

#[path = "commands/makemigrations_integration.rs"]
mod makemigrations_integration;

#[path = "commands/introspect_integration.rs"]
mod introspect_integration;

// Built-in command integration tests
#[path = "commands/builtin_integration.rs"]
mod builtin_integration;

// Cross-command workflow tests
#[path = "commands/workflow_integration.rs"]
mod workflow_integration;

// Plugin command tests
#[path = "commands/plugin_integration.rs"]
mod plugin_integration;

// Edge case E2E tests for makemigrations command
#[path = "commands/makemigrations_e2e_edge_cases.rs"]
mod makemigrations_e2e_edge_cases;
