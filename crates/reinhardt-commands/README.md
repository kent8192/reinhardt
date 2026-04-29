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
reinhardt = { version = "0.1.0-rc.24", features = ["commands"] }

# Or use a preset:
# reinhardt = { version = "0.1.0-rc.24", features = ["standard"] }  # Recommended
# reinhardt = { version = "0.1.0-rc.24", features = ["full"] }      # All features
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
# During the RC phase, `cargo install` requires an explicit `--version`.
# The version below is auto-bumped by release-plz on each release.
cargo install reinhardt-admin-cli --version "0.1.0-rc.24"
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
- **migrate** - Apply database migrations
- **runserver** - Start the development server
- **shell** - Run an interactive REPL
- **check** - Check the project for common issues
- **collectstatic** - Collect static files into `STATIC_ROOT`
- **showurls** - Display all registered URL patterns (requires `routers`
  feature)

### Feature Flags

- `migrations` - Enable migration-related commands (requires
  `reinhardt-db`)
- `routers` - Enable URL-related commands (requires `reinhardt-urls`)

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
context.insert("version", "0.1.0-rc.24");
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

### `collect_migrations!` Macro and `linkme` Dependency

The `collect_migrations!` macro registers migration modules for runtime
discovery. It relies on the [`linkme`](https://crates.io/crates/linkme) crate
for compile-time distributed slice registration.

Projects using `collect_migrations!` must add `linkme` as a dependency:

<!-- reinhardt-version-sync -->
```toml
[dependencies]
reinhardt = { version = "0.1.0-rc.24", features = ["standard"] }
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
```

### App Templates

```bash
reinhardt-admin startapp myapp --template rest
```

Templates are embedded in the binary using `rust-embed` for fast,
dependency-free project generation.

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
- `app_pages_workspace_template`
- `app_restful_workspace_template`

Any file present in your override directory wins; any file absent falls back to
the embedded copy. This lets you customise a single file without vendoring the
entire template tree.

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
