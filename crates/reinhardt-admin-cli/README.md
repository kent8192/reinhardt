# reinhardt-admin-cli

Global command-line tool for Reinhardt project management.

## Overview

`reinhardt-admin-cli` is the Django's `django-admin` equivalent for Reinhardt. It provides utilities for creating new projects and applications.

## Installation

Install globally using Cargo. The command below pins this guide to the
documented release for reproducibility; omit `--version` to let Cargo choose
the latest stable release. The literal below is release-managed.

<!-- reinhardt-version-sync -->
```bash
cargo install reinhardt-admin-cli --version "0.3.3"
```

This installs the `reinhardt-admin` command.

## Usage

### Create a New Project

<!-- reinhardt-version-sync -->
```bash
# Create a RESTful API project (default in non-interactive shells)
reinhardt-admin startproject myproject

# Create a Pages (WASM + SSR) project
reinhardt-admin startproject myproject --with-pages

# Using --template flag (equivalent)
reinhardt-admin startproject myproject --template rest
reinhardt-admin startproject myproject --template pages

# Create project in a specific directory
reinhardt-admin startproject myproject --with-rest /path/to/directory

# Pin the generated Reinhardt dependency
reinhardt-admin startproject myproject --with-rest \
  --reinhardt-version 0.3.3 \
  --features standard,admin \
  --no-interactive
```

When run from an interactive terminal, `startproject` prompts for the
project type, Reinhardt version, and feature flags when those choices are
not provided as flags. Non-interactive runs use deterministic defaults.

### Configure an Existing Project

<!-- reinhardt-version-sync -->
```bash
# Interactively update ./Cargo.toml
reinhardt-admin configure

# Update a project without prompts
reinhardt-admin configure /path/to/project \
  --reinhardt-version 0.3.3 \
  --features minimal,db-sqlite \
  --no-interactive
```

### Create a New App

```bash
# Create a RESTful API app
reinhardt-admin startapp myapp --with-rest

# Create a Pages (WASM + SSR) app
reinhardt-admin startapp myapp --with-pages

# Using --template flag (equivalent)
reinhardt-admin startapp myapp --template rest
reinhardt-admin startapp myapp --template pages

# Create app in a specific directory
reinhardt-admin startapp myapp --with-rest /path/to/directory
```

### Other Commands

```bash
# Display help
reinhardt-admin --help

# Display version
reinhardt-admin --version
```

### Manage Plugins

Manage Reinhardt plugins (Dentdelion):

<!-- reinhardt-version-sync -->
```bash
# List installed plugins
reinhardt-admin plugin list
reinhardt-admin plugin list --verbose
reinhardt-admin plugin list --enabled
reinhardt-admin plugin list --disabled

# Show plugin information
reinhardt-admin plugin info auth-delion
reinhardt-admin plugin info auth-delion --remote

# Install a plugin
reinhardt-admin plugin install auth-delion
reinhardt-admin plugin install auth-delion --version 0.3.3

# Remove a plugin
reinhardt-admin plugin remove auth-delion

# Enable / disable a plugin
reinhardt-admin plugin enable auth-delion
reinhardt-admin plugin disable auth-delion

# Search for plugins on crates.io
reinhardt-admin plugin search auth

# Update plugin(s)
reinhardt-admin plugin update auth-delion
reinhardt-admin plugin update --all
```

### Format All Code

Use `reinhardt-formatter` to format all Rust files in the project. Reinhardt
DSL macros are formatted with tree-sitter grammars and Topiary queries before
`cargo fmt --all` formats the surrounding Rust code. For supported `page!`
Rust expression islands and closure parameter lists, the formatter also runs
rustfmt with the configured line-width options and reinserts the result
conservatively:

```bash
# Format all files in the project
reinhardt-formatter fmt-all

# Check formatting without modifying files
reinhardt-formatter fmt-all --check
```

The first rustfmt-island pass applies only to safe `page!` expression and
event-handler regions. Unsupported fragments remain unchanged so the formatter
does not rewrite DSL bodies it cannot classify confidently.

### Format Reinhardt Macro DSLs

Format `page!`, `form!`, and `head!` macro DSLs in your source files:

```bash
# Format all Rust files in the current directory
reinhardt-formatter fmt .

# Format a specific file
reinhardt-formatter fmt src/main.rs

# Check formatting without modifying files
reinhardt-formatter fmt --check .

# Show all files (including unchanged)
reinhardt-formatter fmt -v .
```

#### Ignore Markers

You can control which Reinhardt DSL macros should be skipped during formatting by using special comment markers:

##### File-Wide Ignore

Skip formatting for the entire file by adding `// reinhardt-fmt: ignore-all` at the beginning (within the first 50 lines, before any code):

```rust
// reinhardt-fmt: ignore-all

use reinhardt_pages::prelude::*;

page!(|| { div{bad_format} })  // Will not be formatted
```

##### Range Ignore

Skip formatting for multiple DSL macros within a range using `// reinhardt-fmt: off` and `// reinhardt-fmt: on`:

```rust
// reinhardt-fmt: off
page!(|| { div{bad1} })
page!(|| { span{bad2} })
// reinhardt-fmt: on

page!(|| { p { good } })  // This will be formatted
```

##### Individual Macro Ignore

Skip formatting for a specific DSL macro by adding `// reinhardt-fmt: ignore` on the line immediately before it:

```rust
// reinhardt-fmt: ignore
page!(|| { div{bad} })

page!(|| { span { good } })  // This will be formatted
```

##### Priority Order

When multiple markers are present, they are applied in this priority order:

1. **Individual marker** (highest) - `// reinhardt-fmt: ignore`
2. **Range marker** (medium) - `// reinhardt-fmt: off/on`
3. **File-wide marker** (lowest) - `// reinhardt-fmt: ignore-all`

##### Notes

- Markers are case-sensitive and spaces are optional (e.g., `//reinhardt-fmt:ignore` also works)
- Individual markers must be on the line **immediately before** the macro (no blank lines)
- Range markers can span multiple DSL macros
- Nested `off` markers will generate a warning but use the first `off` position
- Unclosed ranges (missing `on`) will extend to the end of the file

#### Verbosity Levels

- **Default**: Show formatted files, errors, and summary (with color output)
- **`-v`**: Also show unchanged files
- **`-vv`**: Show all file processing status (deprecated, same as `-v`)

#### Output Format

- **Progress display**: Shows current processing position in `[1/50]` format
- **Color output**:
  - Success (Formatted): Green
  - Error: Red
  - Unchanged: Gray (dimmed)
  - Progress counter: Blue

#### Example Output

```bash
$ reinhardt-formatter fmt .
[1/47] Formatted: src/main.rs
[2/47] Formatted: src/config/settings.rs
[3/47] Error src/broken.rs: Parse error

Summary: 2 formatted, 45 unchanged, 1 errors
```

## Django Equivalents

| Django                                | Reinhardt                                |
|---------------------------------------|------------------------------------------|
| `django-admin startproject myproject` | `reinhardt-admin startproject myproject` |
| `django-admin startapp myapp`         | `reinhardt-admin startapp myapp`         |

## Project Templates

`reinhardt-admin-cli` includes two project templates:

- **rest**: RESTful API project (use `--with-rest` or `--template rest`)
- **pages**: WASM + SSR project with reinhardt-pages (use `--with-pages` or `--template pages`)

## App Templates

Apps can be created in two forms:

- **Module** (default): Created in `apps/` directory
- **Workspace**: Separate crate in workspace

## Features

- **Embedded Templates**: Templates are compiled into the binary using `rust-embed`
- **No External Dependencies**: Works without internet connection
- **Django-Compatible**: Familiar interface for Django developers

## Architecture

`reinhardt-admin-cli` depends on `reinhardt-commands` for its core functionality:

```
reinhardt-admin-cli (CLI binary)
    ↓
reinhardt-commands (Library)
    ↓
StartProjectCommand / StartAppCommand
```

## License

Licensed under the BSD 3-Clause License.
