# {{ project_name }}

A Reinhardt Pages project with WASM frontend and server-side rendering.

## Prerequisites

- Rust 1.94.1 or later (2024 Edition)
- wasm-bindgen-cli: `cargo install wasm-bindgen-cli`
- PostgreSQL (optional, for database features)

## Getting Started

### Install Tools

```bash
# Install all development tools (includes WASM build tools)
cargo make install-tools
```

### Development Server

```bash
# Build WASM and start development server
cargo make dev

# Or step by step:
cargo make wasm-build-dev    # Build WASM
cargo make runserver         # Start server
```

Visit `http://127.0.0.1:8000/` in your browser.

### Build for Production

```bash
# Build WASM (release + optimized)
cargo make wasm-build-release

# Build server
cargo build --release
```

## Project Structure

```
{{ project_name }}/
├── src/
│   ├── client/       # WASM UI (runs in browser)
│   │   ├── lib.rs     # WASM entry point
│   │   ├── router.rs  # Client-side routing
│   │   └── state.rs   # Global state management
│   ├── server/       # Server functions (runs on server)
│   │   └── server_fn.rs
│   ├── shared/       # Shared types (used by both)
│   │   ├── types.rs
│   │   └── errors.rs
│   └── config/       # Server configuration
├── dist/             # WASM build output
├── index.html        # WASM entry HTML
└── Cargo.toml
```

## Management Commands

```bash
# Create a new app
cargo run --bin {{ project_name }} startapp myapp --with-pages

# Database migrations (when using database features)
cargo run --bin {{ project_name }} makemigrations
cargo run --bin {{ project_name }} migrate

# Check project for issues
cargo run --bin {{ project_name }} check
```

## WASM Build Commands

```bash
cargo make wasm-build-dev      # Build WASM (debug)
cargo make wasm-build-release  # Build WASM (release + optimize)
cargo make wasm-watch          # Watch and rebuild on changes
cargo make wasm-clean          # Clean WASM build artifacts
```

## Learn More

- [Reinhardt Documentation](https://github.com/kent8192/reinhardt-rs)
- [Reinhardt Pages Guide](https://github.com/kent8192/reinhardt-rs/tree/main/docs)
- [wasm-bindgen Documentation](https://rustwasm.github.io/wasm-bindgen/)
