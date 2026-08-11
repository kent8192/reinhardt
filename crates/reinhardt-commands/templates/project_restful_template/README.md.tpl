# {{ project_name }}

A Reinhardt project.

## Getting Started

### Using cargo-make (Recommended)

Install cargo-make:
```bash
cargo install cargo-make
```

Run the development server:
```bash
cargo make runserver
```

### Using manage command

```bash
# Run the development server
cargo run --bin manage runserver

# Run migrations
cargo run --bin manage migrate

# Create a new app
cargo run --bin manage startapp myapp

# Export the deterministic application contract
cargo run --bin manage contract export --format json
```

### Rust management shell (opt-in)

The generated `commands-shell` feature is intentionally not a default feature.
Enable it when starting the stateful Rust shell:

```bash
cargo run --bin manage --features commands-shell -- shell
cargo run --bin manage --features commands-shell -- shell -c \
  'println!("{}", settings.core.debug)'
```

`src/config/shell.rs` supplies `get_shell_config()`. The generated native
entry calls `shell_runtime_hook()` before Tokio starts, then selects
`execute_from_command_line_with_resolved_settings_and_shell` when the feature is
enabled; without it, the resolved-settings dispatcher remains active for
non-shell commands.

The shell binds concrete project `settings`, the copyable ORM `db` handle,
the application `di` context, and the stable `framework` alias. Unique
installed model names are imported automatically. A collision emits a
deterministically ordered warning with concrete registered crate paths instead
of choosing an ambiguous short name; the evaluator's `project_crate` alias can
reference those same types. Add project Rust with
`ShellConfig::with_prelude(...)`.

Interactive input supports top-level `.await`, preserves successful state, and
uses `>>> ` / `... ` for primary / multiline input. A panic, evaluator exit,
or Ctrl+C during evaluation clears user state and reloads every prelude layer.
`shell -c` evaluates one snippet, returns zero only on success, returns non-zero
on failure, and Reinhardt's own diagnostics do not repeat the raw source.
Arbitrary Rust, compiler output, panics, and user code can still print literals;
the shell is not a sandbox. History is best-effort at
`<platform local data directory>/reinhardt/shell/<package-name>.history`.
A missing file is a silent first run; directory-resolution, read, or write
failures warn without preventing startup.

`shell-rhai` has been removed. `shell` now means the Rust evaluator; old Rhai
syntax is not supported.

## Common Tasks

### Development

```bash
cargo make dev              # Run checks + build + start server
cargo make runserver-watch  # Start server with auto-reload
```

### Database

```bash
cargo make makemigrations   # Create new migrations
cargo make migrate          # Apply migrations
```

### Testing

```bash
cargo make test             # Run all tests
cargo make test-watch       # Run tests with auto-reload
```

### Code Quality

```bash
cargo make quality          # Run all checks (format + lint)
cargo make quality-fix      # Fix all issues automatically
```

### Help

```bash
cargo make help             # Show all available tasks
```

## Generated with

This project was created using `reinhardt-admin startproject`.
