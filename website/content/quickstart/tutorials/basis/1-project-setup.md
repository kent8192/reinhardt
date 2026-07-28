+++
title = "Part 1: Project Setup and SPA Shell"
description = "Generate a Reinhardt pages project, inspect the native/WASM layout, and run the empty SPA shell."
weight = 10

[extra]
sidebar_weight = 10
+++

# Part 1: Project Setup and SPA Shell

In this part you will create a Reinhardt pages project and run the browser shell. The polling features arrive in later parts; here the goal is to understand the project shape that lets one crate build both a native server and a WASM client.

The finished reference for this tutorial is `examples/examples-tutorial-basis`. Use its app source and tests as the comparison point when your local project differs from the snippets below. Its Cargo workspace and PostgreSQL/Redis helper tasks are repository-specific; the standalone project generated here starts with SQLite and does not need those files.

## Install the Tools

Use Rust 1.96.0 or newer. The generated Rust 2024 project requires that
toolchain level.

<!-- reinhardt-version-sync -->
This tutorial uses the `0.3.3` Reinhardt generator.

Install the Reinhardt project generator:

<!-- reinhardt-version-sync -->
```bash
cargo install reinhardt-admin-cli --version "0.3.3"
```

The installed binary is `reinhardt-admin`.

Install `cargo-make`; the generated project can install the WASM target,
`wasm-pack`, and the file watcher through its own `install-tools` task:

```bash
cargo install cargo-make
```

## Create a Pages Project

Create a new project from the pages template:

```bash
reinhardt-admin startproject tutorial --template pages
cd tutorial
cargo make install-tools
```

The completed tutorial will eventually add `polls` and `users` apps. Do not create them yet. The first milestone is a project that can compile the browser entry point and serve the SPA shell.

The generated tree starts with these landmarks:

```text
tutorial/
+-- Cargo.toml
+-- Makefile.toml
+-- build.rs
+-- index.html
+-- settings/
|   +-- base.toml
|   +-- local.toml
+-- src/
    +-- lib.rs
    +-- apps.rs
    +-- config.rs
    +-- client.rs
    +-- bin/
    |   +-- manage.rs
    +-- config/
    |   +-- apps.rs
    |   +-- settings.rs
    |   +-- urls.rs
    |   +-- wasm.rs
    +-- client/
    |   +-- components.rs
    |   +-- lib.rs
    |   +-- components/
    |       +-- nav.rs
```

The reference example has more files because it is the completed project. You will add those files as each slice needs them.

## Read the Crate Targets

Open `Cargo.toml`. The pages example builds an `rlib` for the native server and a `cdylib` for the WASM client:

```toml
[lib]
crate-type = ["cdylib", "rlib"]  # cdylib for WASM, rlib for server
```

The management command is native-only, so the binary is gated behind the
`with-reinhardt` feature. The generated project enables that feature by default
for native development, while `wasm-pack test -- --no-default-features` can skip
the native-only binary:

```toml
[[bin]]
name = "manage"
path = "src/bin/manage.rs"
required-features = ["with-reinhardt"]
```

The generated feature section keeps those local gates explicit:

```toml
[features]
default = ["with-reinhardt", "client-router"]
client-router = []
with-reinhardt = []
msw = ["reinhardt/msw"]
```

The dependency split is the important design. WASM gets pages and client
routing; the server gets the framework, database backend, commands, admin, and
configuration features selected by `startproject`. The native feature list also
includes the client router, form/session, middleware, and password-hasher
features used by later parts of this tutorial:

<!-- reinhardt-version-sync -->
```toml
[target.'cfg(target_arch = "wasm32")'.dependencies]
reinhardt = { version = "0.3.3", package = "reinhardt-web", default-features = false, features = ["pages", "client-router"] }
wasm-bindgen = "=0.2.122"
```

<!-- reinhardt-version-sync -->
```toml
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
reinhardt = { version = "0.3.3", package = "reinhardt-web", default-features = false, features = [
    "minimal",
    "pages",
    "client-router",
    "admin",
    "conf",
    "commands",
    "commands-server",
    "commands-autoreload",
    "server",
    "db-sqlite",
    "forms",
    "auth-session",
    "middleware",
    "argon2-hasher",
] }
tokio = { version = "1", features = ["full"] }
```

The generated native target uses SQLite by default so the first run does not
require a PostgreSQL container. If you pass an explicit SQLite feature list,
keep the Pages runtime and command facades in place: `commands-server` enables
`manage runserver`, `commands-autoreload` powers the generated dev watcher, and
`server` provides the HTTP server facade. Current `startproject` adds the
required `minimal`, routing, form/session, middleware, password-hasher, and
server-side Pages features automatically when a custom Pages list lacks them.

In an example project, import Reinhardt APIs through the `reinhardt` facade. Do not depend on internal `reinhardt-*` crates directly.

## Understand the Target Aliases

Open `build.rs`. The example declares readable cfg aliases that the rest of the project uses:

```rust
cfg_aliases! {
    client: { target_arch = "wasm32" },
    server: { not(target_arch = "wasm32") },
    wasm: { target_arch = "wasm32" },
    native: { not(target_arch = "wasm32") },
}
```

That gives you `#[cfg(client)]` for browser code and `#[cfg(server)]` for server-only code. The top-level modules follow that split:

```rust
pub mod apps;
pub mod config;

#[cfg(client)]
pub mod client;
```

`apps` compiles on both targets. Each generated app gates `client` with `#[cfg(client)]` and gates server-only implementation modules at the declaration site. Route-backed components live under `src/apps/<app>/client/components/`, `#[model]` generates shared info DTOs from model modules, request DTOs live in `src/shared/types.rs`, and server-only admin/services/forms stay under server-gated modules.

## Inspect Settings

Open `src/config/settings.rs`. The reference example composes the core settings with the contacts fragment:

```rust
#[settings(core: CoreSettings | contacts: ContactSettings)]
pub struct ProjectSettings;
```

`get_settings()` loads defaults, low-priority environment variables, `settings/base.toml`, and the active profile file:

```rust
SettingsBuilder::new()
    .profile(Profile::parse(&profile_str))
    .add_source(DefaultSource::new().with_value(
        "core.base_dir",
        json::Value::String(base_dir.to_string_lossy().to_string()),
    ))
    .add_source(LowPriorityEnvSource::new().with_prefix("REINHARDT_"))
    .add_source(TomlFileSource::new(settings_dir.join("base.toml")))
    .add_source(TomlFileSource::new(
        settings_dir.join(format!("{}.toml", profile_str)),
    ))
    .build_composed()
    .expect("Failed to build settings")
```

The matching `settings/base.toml` must include `[contacts]` because `ProjectSettings` includes `ContactSettings`:

```toml
[core]
secret_key = "insecure-..."

[contacts]
admins = []
managers = []
```

The tutorial database is a local SQLite file:

```toml
[core.databases.default]
engine = "sqlite"
name = "db.sqlite3"
```

`cargo make migrate` and `cargo make dev` run the settings-aware `manage`
binary, so database commands resolve `[core.databases.default]` and create the
SQLite file on demand. No PostgreSQL or Redis container is required for this
tutorial path.

## See the Browser Mount Point

Open `index.html`. The pages template gives the WASM client a stable DOM mount point:

```html
<div id="root">
    <div class="flex items-center justify-center min-h-screen">
        <div class="text-center">
            <div class="spinner w-12 h-12 mx-auto mb-4"></div>
            <p class="text-muted">Loading...</p>
        </div>
    </div>
</div>
```

The WASM entry point in `src/client/lib.rs` mounts the client router there:

```rust
#[cfg_attr(not(feature = "msw"), wasm_bindgen(start))]
pub fn main() -> Result<(), JsValue> {
    ClientLauncher::new("#root")
        .register_routes_from_inventory()
        .launch()
}
```

Later parts will register routes from the `polls` and `users` apps. Their route-backed components will live under `src/apps/<app>/client/components/`; `src/client/` remains the cross-app browser shell. For now, confirm that the browser can load the client bundle and that the server is serving the pages application.

## Run the Development Workflow

Start the dev workflow:

```bash
cargo make dev
```

In the generated project, `dev` runs the quality checks, builds the WASM bundle, and starts the pages server. Run `cargo make migrate` separately when you add migrations. The underlying `runserver` command passes `--with-pages` and reuses the bundle that `wasm-build-dev` just produced:

```bash
cargo run --bin manage -- runserver --with-pages --no-override-wasm
```

Open `http://127.0.0.1:8000/`. At this point the application is only the shell. If the page loads without a missing-WASM error and the server logs show the pages runtime starting, the setup slice is complete.

## Checkpoint

Before continuing:

- `cargo make install-tools` has installed the WASM target and `wasm-pack`.
- `cargo make dev` starts the server.
- The browser reaches `http://127.0.0.1:8000/`.
- `settings/base.toml` contains `[core]`, `[core.databases.default]`, and `[contacts]`.
- Your project imports framework APIs from `reinhardt`, not internal sub-crates.

Next, you will add the first real feature: the poll index.
