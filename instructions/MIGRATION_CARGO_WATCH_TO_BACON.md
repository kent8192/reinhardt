# Migration Guide: cargo-watch to bacon

This guide explains how to migrate from cargo-watch to bacon for automatic code
reloading in Reinhardt projects.

## Overview

As of Reinhardt version 0.1.0-alpha.2 (upcoming), cargo-watch support has been
removed in favor of bacon. Bacon provides a better development experience with:

- **Real-time feedback**: Displays build output and errors immediately
- **Keyboard shortcuts**: Switch between check, clippy, test, and other jobs
  without restarting
- **Better performance**: More efficient file watching and rebuilding
- **Configurable**: Customize jobs via `bacon.toml`

## Breaking Changes

The following cargo-watch-related features have been removed:

1. **Feature flags** (in `crates/reinhardt-commands/Cargo.toml`):
   - `cargo-watch-reload` - removed
   - `watch` - removed

2. **CLI options** (no longer supported):
   - `--clear` - configure via `bacon.toml` instead
   - `--watch-delay` - configure via `bacon.toml` instead

3. **Makefile.toml tasks**:
   - `install-cargo-watch` - replaced with `install-bacon`
   - All `cargo watch` commands replaced with `bacon` commands

## Migration Steps

### 1. Install bacon

```bash
cargo install --locked bacon
```

### 2. Update Makefile.toml

**Before (cargo-watch):**

```toml
[tasks.runserver-watch]
description = "Start the development server with auto-reload (requires cargo-watch)"
command = "cargo"
args = ["watch", "-x", "run --bin manage runserver"]
dependencies = ["install-cargo-watch"]

[tasks.test-watch]
description = "Run tests with auto-reload (requires cargo-watch)"
command = "cargo"
args = ["watch", "-x", "nextest run --all-features"]
dependencies = ["install-cargo-watch", "install-nextest"]
```

**After (bacon):**

```toml
[tasks.runserver-watch]
description = "Start the development server with auto-reload (requires bacon)"
command = "bacon"
args = ["runserver"]
dependencies = ["install-bacon"]

[tasks.test-watch]
description = "Run tests with auto-reload (requires bacon)"
command = "bacon"
args = ["test"]
dependencies = ["install-bacon", "install-nextest"]
```

### 3. Remove cargo-watch Feature from Cargo.toml

**Before:**

```toml
[dependencies]
reinhardt-commands = { version = "0.1.0-alpha.1", features = ["cargo-watch-reload"] }
```

**After:**

```toml
[dependencies]
reinhardt-commands = { version = "0.1.0-alpha.2" }
# No feature flag needed - bacon is used externally
```

### 4. Configure bacon (Optional)

Create `bacon.toml` in your project root:

```toml
# Default job when running `bacon`
default_job = "check"

[jobs.check]
command = ["cargo", "check", "--all-features"]
need_stdout = false

[jobs.clippy]
command = ["cargo", "clippy", "--all-features", "--", "-D", "warnings"]
need_stdout = false

[jobs.test]
command = ["cargo", "nextest", "run", "--all-features"]
need_stdout = true

[jobs.runserver]
command = ["cargo", "run", "--bin", "manage", "runserver"]
need_stdout = true
watch = ["src/**", "Cargo.toml"]
```

See the [bacon documentation](https://dystroy.org/bacon/) for more configuration
options.

## Usage Examples

### Running Development Server

**Before (cargo-watch):**

```bash
cargo watch -x 'run --bin manage runserver'
cargo watch -c -x 'run --bin manage runserver'  # with clear screen
```

**After (bacon):**

```bash
bacon runserver
# Or via cargo make
cargo make watch
```

### Running Tests

**Before (cargo-watch):**

```bash
cargo watch -x 'nextest run --all-features'
```

**After (bacon):**

```bash
bacon test
# Or via cargo make
cargo make watch-test
```

### Running Clippy

**Before (cargo-watch):**

```bash
cargo watch -x clippy
```

**After (bacon):**

```bash
bacon clippy
# Or via cargo make
cargo make watch-clippy
```

## Keyboard Shortcuts

Bacon provides interactive keyboard shortcuts while running:

- `t` - Switch to test mode
- `c` - Switch to clippy mode
- `b` - Switch to build mode
- `r` - Switch to run mode
- `Esc` - Return to previous job
- `Ctrl+j` - Show all available jobs
- `h` - Show help
- `q` - Quit

## VSCode Integration (Optional)

You can integrate bacon with rust-analyzer to avoid duplicate checks:

Create or update `.vscode/settings.json`:

```json
{
   "rust-analyzer.check.overrideCommand": [
      "bacon",
      "clippy",
      "--message-format=json"
   ],
   "rust-analyzer.check.workspace": true
}
```

This makes rust-analyzer use bacon for checking instead of running its own cargo
check.

## Troubleshooting

### Command not found: bacon

Make sure bacon is installed and in your PATH:

```bash
cargo install --locked bacon
which bacon  # Should show the installation path
```

### Jobs not running

Check your `bacon.toml` configuration. If the file doesn't exist, bacon uses
default jobs.

To see default configuration:

```bash
bacon --prefs
```

### File changes not detected

Bacon watches specific paths defined in the `watch` field of each job. Make sure
your source files are included:

```toml
[jobs.runserver]
command = ["cargo", "run", "--bin", "manage", "runserver"]
watch = ["src/**", "Cargo.toml", "settings/**"]  # Add paths as needed
```

## Additional Resources

- [Bacon Official Documentation](https://dystroy.org/bacon/)
- [Bacon GitHub Repository](https://github.com/Canop/bacon)
- [Reinhardt Documentation](../README.md)

## Questions?

If you encounter any issues during migration, please:

1. Check the [bacon documentation](https://dystroy.org/bacon/)
2. Open an issue on
   [Reinhardt GitHub](https://github.com/kent8192/reinhardt-web/issues)
3. Ask in
   [Reinhardt Discussions](https://github.com/kent8192/reinhardt-web/discussions)
