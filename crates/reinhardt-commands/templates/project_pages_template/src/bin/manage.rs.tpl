//! Reinhardt Project Management CLI for {{ project_name }}
//!
//! This is the project-specific management command interface (equivalent to Django's manage.py).
//!
//! This binary is intentionally native-only. The whole module body is gated
//! behind `not(target_arch = "wasm32")` so that
//! `cargo check --target wasm32-unknown-unknown` on the workspace does not
//! try to compile a tokio-based CLI for the browser target. The wasm side
//! still requires a `main` symbol for `bin` crate-types, so we keep an
//! empty stub.
//!
//! ## Router Registration
//!
//! URL patterns are automatically registered by the framework.
//! No manual registration is required - see `src/config/urls.rs` for the
//! `#[routes]` attribute macro that enables this.

#[cfg(not(target_arch = "wasm32"))]
mod native {
    // Force-link the parent library so its `#[routes]` / `#[model]`
    // `inventory::submit!` registrations survive dead-code elimination.
    // Referencing `get_settings` alone does not guarantee the whole crate
    // (and thus every inventory entry) is linked.
    use {{ crate_name }} as _;
    #[cfg(feature = "commands-shell")]
    use {{ crate_name }}::config::shell::get_shell_config;
    use {{ crate_name }}::config::settings::get_settings;
    #[cfg(feature = "commands-shell")]
    use reinhardt::commands::execute_from_command_line_with_resolved_settings_and_shell;
    #[cfg(not(feature = "commands-shell"))]
    use reinhardt::commands::execute_from_command_line_with_resolved_settings;
    use std::process;

    #[tokio::main]
    pub(super) async fn main() {
        // Set settings module environment variable
        // SAFETY: Called at program start before any spawned tasks.
        unsafe {
            std::env::set_var("REINHARDT_SETTINGS_MODULE", "{{ project_name }}.config.settings");
        }

        // Hand the project's composed settings to the runtime so that
        // database-requiring commands (migrate, makemigrations, runserver,
        // createsuperuser) resolve the connection from settings/*.toml
        // (`[core.databases.default]`) without requiring DATABASE_URL.
        // Router registration still happens automatically inside the runtime
        // via the #[routes] attribute macro in src/config/urls.rs.
        #[cfg(feature = "commands-shell")]
        let result =
            execute_from_command_line_with_resolved_settings_and_shell(
                get_settings(),
                get_shell_config(),
            )
                .await;
        #[cfg(not(feature = "commands-shell"))]
        let result = execute_from_command_line_with_resolved_settings(get_settings()).await;

        if let Err(e) = result {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    reinhardt::commands::shell_runtime_hook();
    native::main();
}

#[cfg(target_arch = "wasm32")]
fn main() {}
