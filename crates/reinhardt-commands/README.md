# reinhardt-commands

Django-style management command framework for Reinhardt.

## Overview

`reinhardt-commands` provides a Django-inspired command-line interface for
managing Reinhardt projects. It includes built-in commands for database
migrations, static file collection, development server, and more.

## Installation

Add `reinhardt` to your `Cargo.toml`:

<!-- reinhardt-version-sync:3 -->
```toml
[dependencies]
reinhardt = { version = "0.4.0-alpha.9", features = ["commands"] }

# Or use a preset:
# reinhardt = { version = "0.4.0-alpha.9", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.4.0-alpha.9", features = ["full"] }      # All features
```

Then import command features:

```rust
use reinhardt::commands::{BaseCommand, CommandRegistry};
```

**Note:** Command features are included in the `standard` and `full` feature presets.

### As a global CLI tool

For creating new projects and apps, use the separate `reinhardt-admin-cli`
package:

<!-- reinhardt-version-sync -->
```bash
# Pin the documented Reinhardt release for reproducibility.
# Omit --version to let Cargo choose the latest stable release.
cargo install reinhardt-admin-cli --version "0.4.0-alpha.9"
```

This installs the `reinhardt-admin` command:

```bash
reinhardt-admin startproject myproject
reinhardt-admin startapp myapp
```

See [reinhardt-admin documentation](../reinhardt-admin-cli/README.md) for more
details.

## Features

### Built-in Commands

- **makemigrations** - Create new database migrations based on model changes
- **squashmigrations** - Combine a safe migration range into one replacement
  migration without connecting to a database
- **migrate** - Apply database migrations
- **inspectdb** - Generate deterministic Reinhardt models from an existing
  PostgreSQL, MySQL, or SQLite schema
- **dbshell** - Launch the native client for a configured PostgreSQL, MySQL, or
  SQLite database
- **showmigrations** - Display applied state or the selected dependency plan
  without creating migration history
- **sqlmigrate** - Render backend-specific forward or rollback SQL without
  executing schema statements
- **dumpdata** - Export model rows as Django-compatible JSON fixtures
- **loaddata** - Load Django-compatible JSON fixtures into the database
- **seed** - Run idempotent per-application development seed hooks
- **runserver** - Start the development server
- **infra** - Start, stop, inspect, and use local development infrastructure
- **shell** - Run an interactive REPL
- **check** - Check the project for common issues
- **collectstatic** - Collect static files into `STATIC_ROOT`
- **showurls** - Display all registered server URL patterns (requires `routers`
  feature)
- **contract export** - Export deterministic application metadata as JSON

### Native protocol launch

`runserver` consumes the generated native URL aggregate. HTTP and application
WebSocket routes share the HTTP listener, while gRPC services use the optional
`--grpc-address` listener (default `127.0.0.1:50051`):

```bash
cargo run --bin manage -- runserver
cargo run --bin manage -- runserver --grpc-address 127.0.0.1:50061
```

The generated project enables these protocol capabilities in its native
dependencies; no extra Cargo feature flag is required at launch time.

### Feature Flags

- `migrations` - Enable migration-related commands (requires
  `reinhardt-db`)
- `reinhardt-db` - Enable database-backed management commands such as
  `dumpdata`, `loaddata`, and `seed`
- `routers` - Enable URL-related commands (requires `reinhardt-urls`)
- `contract` - Enable application contract export (requires `migrations` and
  `routers`). The `reinhardt` facade exposes this as `commands-contract`.
- `shell` - Enable the stateful Rust management shell. The facade exposes this
  as the project-facing `commands-shell` feature.

### Exporting the application contract

Export the resolved models, migrations, routes, and settings metadata as the
version 0 application contract:

```bash
cargo run --bin manage -- contract export --format json
```

The management binary must dispatch through
`execute_from_command_line_with_resolved_settings`. Applied migration state is
best effort when no database option is supplied; pass `--database ALIAS` or
`--database-url URL` to require that overlay. See the
[application contract documentation](https://reinhardt-web.dev/docs/application-contract/)
for the canonical schema and field rules.

### Verifying the application contract

With the `contract` feature enabled, use the human-readable default for people
or the version 1 JSON report for automation:

```bash
cargo run --bin manage -- verify
cargo run --bin manage -- verify --format json
```

The clean JSON report is:

```json
{
  "schema_version": 1,
  "status": "passed",
  "violations": []
}
```

| Result | Exit status |
| --- | ---: |
| `passed` | 0 |
| `failed` | 1 |
| `error` | 2 |

In JSON mode, stdout contains only one report document. Cargo output and
operational diagnostics use stderr. Every current violation has severity
`error`. Settings values and concrete dynamic keys are absent, and `location`
is currently `null` because the verifier does not retain source positions.
Human-readable output remains the default.

Each violation has the structured fields `code`, `class`, `severity`,
`target`, `location`, `evidence`, and `suggested_fix`. The seven stable finding
codes are `schema.missing_migration`, `schema.unapplied_migration`,
`authorization.missing_declaration`, `settings.missing_required`,
`settings.type_mismatch`, `settings.map_key_type_mismatch`, and
`settings.duplicate_input`. Reports inherit canonical ordering from
`VerificationRun`.

Targets use one of these shapes:

```text
model_change: app_label, name_fragment
migration: app_label, migration_name
endpoint: method, path, module_path, function_name
setting: canonical wildcarded path
```

An agent can consume the report with this repair loop:

```bash
cargo run --bin manage -- verify --format json > /tmp/reinhardt-verify.json
status=$?
case "$status" in
  0) echo "contract verified" ;;
  1) jq -r '.violations[] | [.code, .target.kind, .suggested_fix] | @tsv' /tmp/reinhardt-verify.json ;;
  2) echo "verification could not complete" >&2 ;;
esac
rm -f /tmp/reinhardt-verify.json
```

Repair source only for exit 1, rerun the command, and stop when it reports
`passed`. Exit 2 requires repairing the execution environment or configuration
before findings can be trusted.

### Squashing migrations

`squashmigrations` accepts Django-compatible two- and three-positional forms:

```bash
cargo run --bin manage -- squashmigrations APP_LABEL MIGRATION_NAME
cargo run --bin manage -- squashmigrations APP_LABEL START_MIGRATION MIGRATION_NAME
```

Migration names may be exact names or unique prefixes. The command rejects an
ambiguous prefix, a branched ancestry, or a range that is not a continuous
same-application ancestor chain. Dependencies entering the selected range from
other applications remain dependencies of the generated migration.

Conditional migration dependencies are resolved from the active project's
`MigrationSettings` fragment.
`migration_swappable_settings` maps each swappable dependency key to its
`"app.Model"` target, `migration_settings` supplies values for optional
dependencies using `SettingEnabled`, and `migration_features` enables optional
dependencies whose feature condition matches an entry in the list.
`installed_apps` enables optional dependencies gated on application presence.
Inactive optional dependencies remain in the generated replacement without
resolving their target application, so the migration graph is correct if the
condition is enabled in another environment.

```toml
[core]
installed_apps = ["accounts"]

[migrations]
migration_features = ["gis"]

[migrations.migration_swappable_settings]
AUTH_USER_MODEL = "accounts.User"

[migrations.migration_settings]
ENABLE_AUDIT = "true"
```

When `--migrations-dir` is omitted, the command reads migrations from the
active project's `core.base_dir/migrations` directory. The explicit option
always takes precedence. Entry points without project settings retain the
project-root/current-directory fallback.

By default, Reinhardt prompts before writing. Use `--no-input` (or the
Django-compatible `--noinput` alias) in non-interactive environments. Use
`--no-optimize` to preserve the exact source operation order, and `--no-header`
to omit the generated-file header. `--squashed-name descriptive_name` supplies
a descriptive suffix; Reinhardt keeps the range's starting number, for example
`0001_descriptive_name`.

The optimizer performs only proven schema reductions. Data operations,
renames, constraints, indexes, bulk operations, custom operations, and other
non-reducible operations are barriers: optimization never crosses them.
Reinhardt validates and renders the complete result before prompting. It then
creates a new migration file without overwriting an existing destination.
Invalid source is rejected before file creation. If a write fails, Reinhardt
attempts to remove the incomplete file through its anchored directory handle.
A cleanup failure reports both the original write error and the cleanup error.
The command only reads migration source files and does not require a database
connection.

### Inspecting migration state and SQL

`showmigrations` lists migration state by application by default. Use `--plan`
to show the complete selected dependency order; selecting an application keeps
its transitive cross-application dependencies. `sqlmigrate APP MIGRATION`
accepts an exact name or unique prefix and renders through the same SQL planner
used by migration execution. Pass `--backwards` for rollback SQL.

```bash
# List every migration, or only polls plus its transitive dependencies.
cargo run --bin manage -- showmigrations
cargo run --bin manage -- showmigrations polls --list

# Display execution order and render forward or rollback SQL.
cargo run --bin manage -- showmigrations polls --plan
cargo run --bin manage -- sqlmigrate polls 0002
cargo run --bin manage -- sqlmigrate polls 0002 --backwards
```

`--list` (`-l`) and `--plan` (`-p`) are mutually exclusive; list mode is the
default. At verbosity level two, applied entries include their recorded
timestamps. Prefix matching is scoped to the selected application and must
identify exactly one migration. An ambiguous or unknown prefix is rejected
before output.

Both commands accept `--database ALIAS` and a one-off `--database-url URL`.
Without a URL override, the alias selects a configured database. With
`--database-url`, the command connects directly to that URL without looking up
the alias; the alias remains a safe diagnostic label and settings are not
modified. Diagnostics redact URL credentials and sensitive-looking aliases.
Both commands load and validate the complete migration catalog before output.
When `--migrations-dir` is omitted, both commands read from the active
project's `core.base_dir/migrations` directory. Conditional and swappable
dependencies use the composed `MigrationSettings` fragment passed by the
generated `manage.rs` entry point, together with installed applications and
legacy migration settings from the core fragment.
`showmigrations` reads an existing recorder table without creating it, while
`sqlmigrate` performs no schema or migration-history writes. SQL output is
fully buffered before its single stdout write, so an irreversible rollback or
late planning error emits no partial script.

Rendered SQL follows the selected backend. PostgreSQL uses double-quoted
identifiers, MySQL uses backticks, and SQLite uses its table-recreation sequence
when an alteration cannot be expressed directly. Transaction wrappers are
emitted only when the migration plan is atomic and the backend supports
transactional DDL; MySQL DDL is therefore never wrapped. Informational
data-operation comments remain comments and are never executed.

Statement splitting also follows the selected backend: PostgreSQL nested block
comments, MySQL's whitespace-sensitive `--` comments, and SQLite trigger bodies
are kept intact. This ensures that previewed SQL has the same statement
boundaries as execution.

When SQL planning reconstructs state from historical migration source, it
fails closed if that source cannot represent required schema metadata exactly,
including table comments, declared column order, or specialized constraints.
This prevents a plausible but incomplete preview from being emitted.

### Pages template hot reload

The `pages` feature enables the Pages HMR transport used by
`runserver --with-pages`. For WASM-owned `page!` source, literal text and
literal attribute edits can use transactional template patches that preserve
reactive state and DOM-bound handlers. Changes to dynamic expressions,
handlers, bindings, control flow, components, callsite structure, or shared
server-visible code conservatively fall back to the normal rebuild path.

The served dist directory is available both at its legacy root URLs and under
the configured `STATIC_URL`. This keeps SPA and WASM development URLs working
while ensuring manifest-resolved `collectstatic` assets use the same URLs that
`static_url()` emits. Missing assets under `STATIC_URL` return through the
application router instead of receiving the SPA index fallback.
Unhashed source paths under `STATIC_URL` are resolved through
`manifest.json`, allowing source HTML to reference stable asset names while
the collected files retain fingerprinted names. This also applies when
`STATIC_URL` is `/`; stable alias responses use a revalidating cache policy.
Running collection with hashing disabled removes an older manifest so stale
fingerprinted aliases cannot override the newly collected files.

Patch application is gated by each mounted template's key and dynamic ABI.
Patches for unloaded routes or branches are retained until their descriptor
first mounts, then validated before use. A rejected patch or failed build keeps
the last successful client active while the HMR channel reports normalized
diagnostics; a successful fallback uses the existing readiness-gated reload
behavior.

## Template System

`reinhardt-commands` uses the
[Tera template engine](https://keats.github.io/tera/) for rendering project and
app templates during code generation (e.g., `startproject` and `startapp`
commands).

### Template Syntax

Templates use Tera's syntax, which is compatible with Jinja2/Django templates:

```rust
// Variable substitution
{{ variable_name }}
{{ camel_case_app_name }}

// Conditional logic (available in Tera, not in old string replacement)
{% if is_mtv %}
pub mod templates;
{% endif %}

// Loops (available in Tera, not in old string replacement)
{% for item in items %}
    {{ item }}
{% endfor %}
```

### Template Context

When rendering templates, the following variables are available:

**For `startproject`:**

- `project_name` - The project name (e.g., "my_project")
- `camel_case_project_name` - CamelCase version (e.g., "MyProject")
- `secret_key` - Generated Django-compatible secret key
- `reinhardt_version` - Current Reinhardt framework version
- `is_mtv` - "true" or "false" flag
- `is_restful` - "true" or "false" flag

**For `startapp`:**

- `app_name` - The app name (e.g., "users")
- `camel_case_app_name` - CamelCase version (e.g., "Users")
- `is_mtv` - "true" or "false" flag
- `is_restful` - "true" or "false" flag

### Custom Template Variables

You can pass custom variables to templates programmatically:

<!-- reinhardt-version-sync -->
```rust
use reinhardt::commands::TemplateContext;

let mut context = TemplateContext::new();
context.insert("project_name", "my_project");
context.insert("version", "0.4.0-alpha.9");
context.insert("features", vec!["auth", "admin"]);  // Any Serialize type
```

## Usage

### In Project (`manage.rs`)

Create a `manage.rs` in your project's `src/bin/` directory:

```rust
use clap::{Parser, Subcommand};
use reinhardt::commands::{
    CheckCommand, CommandContext, MakeMigrationsCommand,
    MigrateCommand, RunServerCommand,
};

#[derive(Parser)]
#[command(name = "manage")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Makemigrations {
        #[arg(value_name = "APP_LABEL")]
        app_labels: Vec<String>,

        #[arg(long)]
        dry_run: bool,
    },
    Migrate {
        #[arg(value_name = "APP_LABEL")]
        app_label: Option<String>,
    },
    // ... other commands
}

#[tokio::main]
async fn main() {
    // Parse CLI and execute commands
    // See templates/project/src/bin/manage.rs for complete example
}
```

Then run commands with:

```bash
cargo run --bin manage makemigrations
cargo run --bin manage migrate
cargo run --bin manage runserver
```

### Database schema inspection

`inspectdb` follows Django's positional-table form:

```bash
cargo run --bin manage -- inspectdb [TABLE ...]
```

Table arguments are exact names rather than patterns. Without table arguments,
the command inspects every table; pass `--include-views` to include views or
`--include-partitions` for PostgreSQL partitions. Any selected schema object
without primary-key metadata is rejected because it cannot produce a lossless
ORM model.

`--database` selects a configured database alias and defaults to `default`. It
never accepts a connection URL. Use `--database-url` for an explicit one-off
URL that takes precedence over the selected alias:

```bash
cargo run --bin manage -- inspectdb accounts --database reporting
cargo run --bin manage -- inspectdb accounts \
  --database-url 'sqlite:///var/lib/example.sqlite3'
```

Generated Rust is the only stdout content by default, so it can be redirected
directly:

```bash
cargo run --bin manage -- inspectdb > src/models.rs
```

Use `--output DIRECTORY` for a generated Rust 2024 multi-file module. It writes
`DIRECTORY/models.rs` and one child module per table beneath
`DIRECTORY/models/`; it never generates `mod.rs`. The command preflights the
complete file set and refuses to overwrite any existing file. Add `--force`
only with `--output` to replace existing generated files. File publication is
rollback-safe and all-or-nothing when the command reports a failure: replaced
files are restored and newly created partial output is removed.

`inspectdb` preserves supported relationship targets, referential actions,
identity modes, scalar defaults, and explicit JSON versus JSONB field metadata.
It rejects schema features that cannot be represented by generated model
attributes (including composite unique constraints or foreign keys,
shared-primary-key relationships, partial indexes, table-level CHECK constraints,
and storage-width-specific integer, text, binary, or enum types) instead of
silently generating a lossy migration model.

### Native database shell

`dbshell` launches the database vendor's native interactive client. PostgreSQL
requires `psql`, MySQL requires `mysql`, and SQLite requires `sqlite3`; the
selected executable must be available on `PATH`.

The command uses the `default` configured database alias unless `--database`
selects another alias. An explicit `--database-url` takes precedence over the
selected alias:

```bash
cargo run --bin manage -- dbshell
cargo run --bin manage -- dbshell --database reporting
cargo run --bin manage -- dbshell \
  --database-url 'postgresql://operator@example.internal/reporting'
```

Arguments after `--` are passed to the native client without reinterpretation:

```bash
cargo run --bin manage -- dbshell -- --expanded
```

The client inherits the terminal's standard input, output, and error streams,
so prompts and interactive features continue to work. Database passwords are
not placed in the native client's arguments. Reinhardt adds `PGPASSWORD` or
`MYSQL_PWD` only to the child process environment; it does not add those
variables to the parent process or expose their values in its diagnostics. A
MySQL URL with a host uses TCP explicitly so a `localhost` port is honored;
add the supported `socket` query parameter when Unix-socket transport is
required.

### Rust Management Shell

The shell is an opt-in Rust evaluator. In a generated project, enable the local
feature explicitly:

```bash
cargo run --bin manage --features commands-shell -- shell
cargo run --bin manage --features commands-shell -- shell -c \
  'println!("{}", settings.core.debug)'
```

`commands-shell` is intentionally absent from generated default features.
REST projects forward the facade feature to `reinhardt/commands-shell`. Pages
projects enable the shell implementation through a native-only
`reinhardt-commands` dependency so server-only evaluator dependencies do not
enter WASM builds. `config::shell` defines the aliases used by the evaluator,
and `get_shell_config()` identifies the package, crate, settings factory,
installed apps, and optional project prelude.
When the management binary enables additional project features, pass the same
selection to `ShellConfig::with_dependency_features`; also call
`without_default_features` when the binary was built without defaults. This
keeps the evaluator's path dependency aligned with the management binary.
The complete native startup shape is:

```rust,ignore
#[cfg(not(target_arch = "wasm32"))]
mod native {
    // Force-link the parent library so its `#[routes]` / `#[model]`
    // `inventory::submit!` registrations survive dead-code elimination.
    // Referencing `get_settings` alone does not guarantee the whole crate
    // (and thus every inventory entry) is linked.
    use my_project as _;
    #[cfg(feature = "commands-shell")]
    use my_project::config::shell::get_shell_config;
    use my_project::config::settings::get_settings;
    #[cfg(not(feature = "commands-shell"))]
    use reinhardt::commands::execute_from_command_line_with_migration_settings;
    #[cfg(feature = "commands-shell")]
    use reinhardt::commands::execute_from_command_line_with_migration_settings_and_shell;

    #[tokio::main]
    pub(super) async fn main() {
        // Preserve the project's existing settings-module initialization.
        // SAFETY: Called at program start before any spawned tasks.
        unsafe {
            std::env::set_var(
                "REINHARDT_SETTINGS_MODULE",
                "my_project.config.settings",
            );
        }

        #[cfg(feature = "commands-shell")]
        let result =
            execute_from_command_line_with_migration_settings_and_shell(
                get_settings(),
                get_shell_config(),
            )
            .await;
        #[cfg(not(feature = "commands-shell"))]
        let result = execute_from_command_line_with_migration_settings(get_settings()).await;

        if let Err(error) = result {
            eprintln!("Error: {error}");
            std::process::exit(1);
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
```

The `use my_project as _;` import is intentional. Merely calling the settings
factory does not guarantee that the linker retains every project module, while
the shell discovers models and DI providers through their inventory
registrations. Force-linking the project crate preserves those registrations.
Preserve the existing `REINHARDT_SETTINGS_MODULE` initialization and substitute
the project's actual module string for `my_project.config.settings`; the shell
migration does not replace or remove that startup setting.
The outer `main` call is required before `#[tokio::main]` constructs the Tokio
runtime. With `commands-shell` disabled, the hook is a no-op and the existing
settings-only entry point remains compatible for non-shell commands.

After bootstrap, normal Rust expressions can use the settings, database, DI,
and model bindings directly. In this example, `User` has a unique short name;
the colliding `Record` model is named through its project-qualified path:

```rust,ignore
println!("{}", settings.core.debug);
println!("{:?}", db.backend());
println!(
    "{}",
    di.get_singleton::<framework::db::orm::DatabaseConnection>()
        .is_some()
);
println!("{}", std::any::type_name::<User>());
println!(
    "{}",
    std::any::type_name::<project_crate::apps::billing::models::Record>()
);
```

- `settings` is the concrete project settings value.
- `db` is the copyable ORM `DatabaseConnection` backed by the shell-owned
  database lease.
- `di` is the application's `Arc<InjectionContext>` and contains the database
  lease and connection in addition to registered providers.
- `framework` is the generated stable alias for `reinhardt`.

Installed models with unique Rust type names are imported automatically.
Colliding names are not imported; one deterministic startup warning lists
their concrete registered crate paths, such as
`my_project::apps::billing::models::Record`. Inside the evaluator, the stable
`project_crate` alias can reference the same type as
`project_crate::apps::billing::models::Record`.
`ShellConfig::with_prelude(...)` adds project-defined Rust after the standard
bindings.

Interactive input is stateful and supports top-level `.await`. The primary
prompt is `>>> `; unmatched brackets continue at `... ` and the complete block
is evaluated as one snippet. Ctrl+C while editing discards the pending input.
Ctrl+C during evaluation, a user-code panic, or evaluator process exit replaces
the evaluator, clears user-defined state, and reloads settings, ORM/DI bindings,
model imports, and the project prelude.

`shell -c SOURCE` uses the same bootstrap and evaluates exactly one snippet.
Successful evaluation exits zero; compilation, bootstrap, evaluation, panic,
or process-exit failures return a command error and the generated management
binary exits non-zero. Reinhardt does not echo the raw `SOURCE` in its own
framework diagnostics. Arbitrary Rust, compiler diagnostics, panics, and user
code can still print source literals or other sensitive values; the shell is
not a secrecy boundary or sandbox.

History is stored best-effort at
`<platform local data directory>/reinhardt/shell/<package-name>.history`
(for example, under `~/Library/Application Support` on macOS). A missing file
is the silent, normal first-run case. An unreadable file, unwritable directory
or file, or unavailable platform data directory produces a warning, disables
only the affected history operation, and does not prevent shell startup.

The first cold start may compile the project crate and evaluator support.
Subsequent warm starts reuse normal Cargo artifacts when inputs are unchanged.

#### Migration from the Rhai shell

- `shell-rhai` was removed; there is no compatibility alias.
- `shell` now means the Rust `evcxr` evaluator, and old Rhai snippets are not
  supported syntax.
- Existing settings-only command entry points continue to work for non-shell
  commands.
- Projects enabling `commands-shell` must add `config::shell::get_shell_config`,
  the pre-Tokio runtime hook, and the settings-and-shell dispatcher.
- Generated projects keep `commands-shell` outside their default feature set.

### Model Fixture Commands

`dumpdata` and `loaddata` use Django-compatible JSON records with
`model`, `pk`, and `fields` keys. Model labels use the
`app_label.ModelName` form registered by `#[model(...)]`.
`dumpdata` keeps stdout as machine-readable JSON. `loaddata` loads fixtures
inside one transaction, upserts rows by explicit primary key, preserves
explicit `null` values, accepts Django-style foreign key field names, and
validates foreign-key values as scalar identifiers (or `null` for nullable
relationships). It also includes many-to-many fixture arrays, preserves binary
fields as Django-compatible base64 strings, and resets PostgreSQL sequences
after explicit integer primary keys.

```bash
# Export all registered fixture models.
cargo run --bin manage dumpdata > fixtures/dev.json

# Export selected models and exclude another app or model.
cargo run --bin manage dumpdata writing_sources.WritingProject \
  --exclude sessions.Session > fixtures/writing_projects.json

# Load fixture files inside a database transaction.
cargo run --bin manage loaddata fixtures/dev.json
```

The `seed` command runs registered idempotent seed hooks. Omit app labels to
run every registered hook, or pass app labels to seed a subset. Every requested
app label must have a registered hook.

```bash
cargo run --bin manage seed
cargo run --bin manage seed writing_sources
```

### `infra` Command

The `infra` command manages project-local Docker containers derived from the
resolved Reinhardt settings. Use `infra run` for short-lived management
commands that need local infrastructure values. Keep long-running server
processes on the normal `manage runserver` entrypoint.

When a required service image is missing from the active Docker daemon,
`infra up` pulls it before creating the container.

```bash
# Start containers for services inferred from settings
cargo run --bin manage infra up

# Print the resolved state as JSON
cargo run --bin manage infra up --json

# Run a short-lived management command with local infrastructure settings applied
cargo run --bin manage infra run -- migrate

# Select the state created by `infra up --profile staging`
cargo run --bin manage infra run --profile staging -- migrate

# Run the development server separately after exporting local infrastructure env
eval "$(cargo run --bin manage infra up --print-env)"
cargo run --bin manage runserver

# Inspect or stop the persisted local infrastructure state
cargo run --bin manage infra status
cargo run --bin manage infra down
```

State is stored under `.reinhardt/local-infra.json` in the project directory.
Before it is used, the command verifies that it belongs to the current workspace,
contains only the expected loopback services and project-scoped container names,
and still matches each container's Docker port binding. Public callers of
`InfraCommand::up_with_config` may provide any project identifier because the
command binds the persisted state to `project_root` before provisioning. Delete
the state file and run `infra up` again if validation fails.
When neither `infra run --profile` nor `REINHARDT_ENV` selects a profile,
`infra run` uses the validated profile persisted by `infra up`.
The child process receives `DATABASE_URL`, `REDIS_URL`, and compatible
`REINHARDT_` environment variables for discovered local services.
`infra run -- runserver` is intentionally unsupported; start infrastructure
first, then run `manage runserver` as its own process.

### `makemigrations` Command Options

The `makemigrations` command supports the following flags and options:

| Flag / Option | Description |
|---------------|-------------|
| `--dry-run` | Show what would be created without writing files |
| `--empty` | Create an empty migration |
| `--from-db` | Use database history instead of TestContainers for state building |
| `--force-empty-state` | Force using empty state when database/TestContainers is unavailable (**dangerous**) |
| `-v`, `--verbose` | Show detailed operation list |
| `-n`, `--name <NAME>` | Name for the migration |
| `--migrations-dir <DIR>` | Directory for migration files (default: `migrations`) |

#### The `--force-empty-state` Flag

By default, `makemigrations` builds the current project state by replaying
existing migrations using TestContainers (or a real database with `--from-db`).
If neither is available, the command fails.

The `--force-empty-state` flag overrides this behavior by assuming an empty
starting state, which is useful in the following scenarios:

- **Initial migration generation**: When a project has no existing migrations
  and no database is available
- **Starting fresh**: When you want to regenerate migrations from scratch

**Warning:** Using `--force-empty-state` on a project with existing migrations
may create duplicate migrations because the command cannot detect previously
applied changes. Use with caution.

```bash
# Generate initial migrations without a running database
cargo run --bin manage makemigrations --force-empty-state

# Combine with --dry-run to preview without writing files
cargo run --bin manage makemigrations --force-empty-state --dry-run
```

### `migrate` Command Options

The `migrate` command applies and unapplies migrations using Django-style
*migrate-with-target* semantics: a single command expresses both directions and
the direction is resolved from the currently applied state.

| Flag / Option | Description |
|---------------|-------------|
| `<APP_LABEL>` | App to migrate (positional, optional) |
| `<MIGRATION_NAME>` | Target migration, or the special token `zero` (positional, optional) |
| `--fake` | Update the recorder (mark applied for forward, unapplied for rollback) without executing migration SQL |
| `--fake-initial` | Skip the initial migration if the tables already exist |
| `--plan` | Preview the migration plan without applying or rolling back |
| `--migrations-dir <DIR>` | Root directory containing migration files (default: `./migrations`) |
| `-d`, `--database <URL>` | Database connection string (falls back to `DATABASE_URL`) |

**Note:** Although `<APP_LABEL>` and `<MIGRATION_NAME>` are each individually
optional, supplying a `<MIGRATION_NAME>` requires `<APP_LABEL>` to be given as
well; otherwise the command fails with `<migration> requires <app>`.

#### Migrate to a Target

When a `<MIGRATION_NAME>` is given, the direction is auto-detected by comparing
the target against the migrations currently applied for `<APP_LABEL>`:

```bash
# Apply every unapplied migration for the app (no target)
cargo run --bin manage migrate myapp

# Forward: apply up to and including 0003_third (and its dependencies)
cargo run --bin manage migrate myapp 0003_third

# Backward: roll back every migration applied after 0001_initial
cargo run --bin manage migrate myapp 0001_initial

# Unapply ALL migrations for the app (Django's special `zero` token)
cargo run --bin manage migrate myapp zero

# Preview any of the above without touching the database
cargo run --bin manage migrate myapp 0001_initial --plan
```

The resolution rules are:

- `<target> == "zero"` — unapply **all** migrations for the app.
- `<target>` is currently applied — roll back every migration applied **after**
  it (backward). When `<target>` is already the latest applied migration this is
  a no-op.
- `<target>` is **not** applied — apply `<target>` and its intra-app dependency
  closure (forward), skipping anything already applied.

`--plan` never mutates the database, including the migration bookkeeping table:
on a fresh database a dry-run leaves it uncreated. Apply plans are displayed in
the same dependency-resolved order used by real migration execution, including
cross-app dependencies.

### `collect_migrations!` Macro and `linkme` Dependency

The `collect_migrations!` macro registers migration modules for runtime
discovery. It relies on the [`linkme`](https://crates.io/crates/linkme) crate
for compile-time distributed slice registration.

Projects using `collect_migrations!` must add `linkme` as a dependency:

<!-- reinhardt-version-sync -->
```toml
[dependencies]
reinhardt = { version = "0.4.0-alpha.9", features = ["standard"] }
linkme = "0.3"
```

Usage in your app's `migrations.rs`:

```rust
pub mod _0001_initial;
pub mod _0002_add_fields;

reinhardt::collect_migrations!(
    app_label = "myapp",
    _0001_initial,
    _0002_add_fields,
);
```

The `linkme` crate is re-exported by `reinhardt` under `reinhardt::linkme`, but
the direct dependency is required for the macro to resolve the crate at compile
time.

### Django Equivalents

| Django                            | Reinhardt                               |
| --------------------------------- | --------------------------------------- |
| `python manage.py makemigrations` | `cargo run --bin manage makemigrations` |
| `python manage.py migrate`        | `cargo run --bin manage migrate`        |
| `python manage.py migrate app 0001` | `cargo run --bin manage migrate app 0001` |
| `python manage.py migrate app zero` | `cargo run --bin manage migrate app zero` |
| `python manage.py runserver`      | `cargo run --bin manage runserver`      |
| `python manage.py shell`          | `cargo run --bin manage shell`          |
| `python manage.py check`          | `cargo run --bin manage check`          |
| `python manage.py collectstatic`  | `cargo run --bin manage collectstatic`  |
| `django-admin startproject`       | `reinhardt-admin startproject`          |
| `django-admin startapp`           | `reinhardt-admin startapp`              |

## Custom Commands

Create custom commands by implementing the `BaseCommand` trait:

```rust
use reinhardt::commands::{BaseCommand, CommandContext, CommandResult};
use async_trait::async_trait;

struct MyCommand;

#[async_trait]
impl BaseCommand for MyCommand {
    fn name(&self) -> &str {
        "mycommand"
    }

    fn description(&self) -> &str {
        "My custom command"
    }

    async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
        ctx.info("Executing my command!");
        Ok(())
    }
}
```

Register your command in `manage.rs`:

```rust
use reinhardt::commands::CommandRegistry;

let mut registry = CommandRegistry::new();
registry.register(Box::new(MyCommand));
```

## Plugin Command System

The plugin command system integrates with `reinhardt-dentdelion` to provide CLI commands for managing plugins:

### Available Commands

| Command | Description |
|---------|-------------|
| `plugin list` | List all installed plugins |
| `plugin info <name>` | Show detailed information about a plugin |
| `plugin install <name>` | Install a plugin from crates.io |
| `plugin remove <name>` | Remove an installed plugin |
| `plugin enable <name>` | Enable a disabled plugin |
| `plugin disable <name>` | Disable an active plugin |
| `plugin search <query>` | Search for plugins on crates.io |
| `plugin update <name>` | Update a plugin to the latest version |

### Usage Examples

```bash
# List all plugins
reinhardt plugin list

# Install a plugin from crates.io
reinhardt plugin install my-awesome-plugin

# Show plugin details
reinhardt plugin info my-awesome-plugin

# Enable/disable plugins
reinhardt plugin enable my-awesome-plugin
reinhardt plugin disable my-awesome-plugin

# Search for plugins
reinhardt plugin search authentication

# Update a plugin
reinhardt plugin update my-awesome-plugin
```

### Integration with dentdelion.toml

Plugin commands automatically update your project's `dentdelion.toml` manifest:

```toml
[plugins]
my-awesome-plugin = { version = "1.0.0", enabled = true }
auth-plugin = { version = "2.1.0", enabled = false }
```

### Implementation

Plugin commands are implemented in `src/plugin_commands.rs` and use the `reinhardt-dentdelion` crate for plugin management:

```rust
use reinhardt::commands::BaseCommand;
use reinhardt::dentdelion::{PluginInstaller, CratesIoClient};

#[async_trait]
impl BaseCommand for PluginInstallCommand {
    fn name(&self) -> &str {
        "plugin install"
    }

    async fn execute(&self, ctx: &CommandContext) -> CommandResult<()> {
        let installer = PluginInstaller::new()?;
        installer.install(&plugin_name, None).await?;
        Ok(())
    }
}
```

## Project Templates

`reinhardt-commands` includes project and app templates:

### Project Templates

- **MTV** (Model-Template-View) - Traditional server-rendered web applications
- **RESTful** - API-first applications

```bash
reinhardt-admin startproject myproject --template rest
reinhardt-admin startproject myproject --template pages
reinhardt-admin startproject myproject --features standard,admin --no-interactive
```

Pass the project type and dependency selection explicitly. Use
`--reinhardt-version`, `--feature`, `--features`, `--default-features`, and
`--no-interactive` for reproducible scripts instead of relying on a
`startproject` wizard.

#### Generated Agent Guidance

The built-in RESTful and Pages project templates generate `AGENTS.md`,
`CLAUDE.md`, and a small `instructions/` directory at the project root. The
root files contain project conventions and point to focused technical guidance;
the Pages pair also documents native/WASM source boundaries. Apps generated
with `startapp` inherit this root guidance instead of receiving nested copies.

The files are project-owned snapshots. When project conventions change, update
both files in the same change and keep them identical except for the filename
used as the top-level title. Framework upgrades and `configure` do not rewrite
them automatically.

Existing projects can update their `reinhardt` dependency through the same
selection flow:

```bash
reinhardt-admin configure
reinhardt-admin configure /path/to/project --features minimal,db-sqlite --no-interactive
```

### App Templates

```bash
reinhardt-admin startapp myapp --template rest
```

Templates are embedded in the binary using `rust-embed` for fast,
dependency-free project generation.

Generated model examples always declare `app_label` and rely on the model
macro's singular snake_case table-name convention. Add `table_name = "..."`
when a generated app must map a model to an existing or custom table.

## Architecture

`reinhardt-commands` is designed to be:

- **Independent** - Can be installed and used standalone
- **Composable** - Commands can be combined and extended
- **Feature-gated** - Optional dependencies reduce compile times
- **Django-compatible** - Familiar interface for Django developers

## Customizing Templates

`reinhardt-commands` ships its scaffolding templates embedded in the binary via
`rust-embed`, so `cargo install reinhardt-commands` produces a self-contained
executable without any external template files.

Two override mechanisms are available:

### Full replacement: `--template <PATH>`

Pass `--template <PATH>` to `startproject` or `startapp` to use `<PATH>` as the
complete template tree. No fallback to embedded assets is performed. Use this
when you maintain a fully custom project template from scratch.

A complete custom template owns every output file. Reinhardt does not inject
the built-in `AGENTS.md` or `CLAUDE.md` when those files are absent from a
`--template` tree.

```bash
reinhardt-admin startproject myproject --template /path/to/my-template
```

### Partial override: `--template-dir <ROOT>`

Pass `--template-dir <ROOT>` (or set the `REINHARDT_TEMPLATE_DIR` environment
variable) to point at a directory that contains one or more template
subdirectories matching the built-in names:

- `project_pages_template`
- `project_restful_template`
- `app_pages_template`
- `app_restful_template`

Any file present in your override directory wins; any file absent falls back to
the embedded copy. This lets you customise a single file without vendoring the
entire template tree.

Built-in guidance files use the relative paths `AGENTS.md.tpl` and
`CLAUDE.md.tpl` inside each project template directory. Missing files fall back
to the embedded copies. If an overlay customizes the guidance, override both
templates and keep them synchronized; a one-sided override is accepted but is
owned entirely by the custom-template author.

```bash
# Only override the Cargo.toml template; everything else stays embedded
mkdir -p ~/my-templates/project_restful_template
cp ... ~/my-templates/project_restful_template/Cargo.toml.tpl
reinhardt-admin startproject myproject --template-dir ~/my-templates

# Or use the environment variable
export REINHARDT_TEMPLATE_DIR=~/my-templates
reinhardt-admin startproject myproject
```

**Precedence:** `--template` > `--template-dir` CLI flag > `REINHARDT_TEMPLATE_DIR` env > embedded defaults.

## License

Licensed under the BSD 3-Clause License.
- Component styles are compiled from canonical `#[style_def] static ... = style! { ... }`
  definitions. In workspaces, select the owning crate with `collectstatic --package NAME`
  or `runserver --package NAME`. For feature-gated Pages styles, pass matching
  `--features feature_a,feature_b` or `--all-features` options so extraction
  uses the same Cargo feature set as the WASM build.
- Pages WASM artifacts use the selected Cargo library target name, while package
  selection continues to use the Cargo package name. This supports packages
  whose `[lib] name` differs from `[package] name`.
- The framework reserves `__reinhardt__/` for generated assets. Production
  collection hashes `__reinhardt__/components.css` through `manifest.json`;
  development serves the same logical URL from an RAII-owned temporary root.
- Applications must include the stylesheet link explicitly. Neither production
  collection nor development serving injects it into arbitrary documents.
