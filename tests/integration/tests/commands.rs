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

#[path = "commands/makemigrations_command_boundary.rs"]
mod makemigrations_command_boundary;

#[path = "commands/introspect_integration.rs"]
mod introspect_integration;

#[path = "commands/squashmigrations_management.rs"]
mod squashmigrations_management;

#[path = "commands/inspectdb_management.rs"]
mod inspectdb_management;

#[path = "commands/dbshell_management.rs"]
mod dbshell_management;

#[path = "commands/migration_visibility_management.rs"]
mod migration_visibility_management;

// Built-in command integration tests
#[path = "commands/builtin_integration.rs"]
mod builtin_integration;

// Cross-command workflow tests
#[path = "commands/workflow_integration.rs"]
mod workflow_integration;

// Plugin command tests
#[path = "commands/plugin_integration.rs"]
mod plugin_integration;

// Management shell end-to-end tests
#[cfg(any(
	target_os = "macos",
	all(target_os = "linux", not(target_env = "uclibc"))
))]
#[path = "commands/shell_e2e.rs"]
mod shell_e2e;

// Edge case E2E tests for makemigrations command
#[path = "commands/makemigrations_e2e_edge_cases.rs"]
mod makemigrations_e2e_edge_cases;

// Collectstatic admin asset auto-discovery integration tests
#[path = "commands/collectstatic_admin_integration.rs"]
mod collectstatic_admin_integration;

// SQLite-backed target migration integration tests.
#[path = "commands/migrate_target_sqlite_e2e.rs"]
mod migrate_target_sqlite_e2e;
