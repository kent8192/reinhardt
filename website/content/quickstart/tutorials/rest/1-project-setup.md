+++
title = "Part 1: Project Setup"
description = "Generate a Reinhardt REST project, inspect the settings layout, and run the development server."
weight = 10

[extra]
sidebar_weight = 10
+++

# Part 1: Project Setup

Let's create the project that will become the snippets API. In this chapter you will generate a REST project, add the `snippets` app, look at the settings and management-command entry points, and start the development server.

The reference implementation for this tutorial is `examples/examples-tutorial-rest`. You do not need to copy that whole crate now; use it as the answer key when you want to compare your files with a finished project.

## Install the Project Generator

Install `reinhardt-admin-cli` once:

<!-- reinhardt-version-sync -->
```bash
cargo install reinhardt-admin-cli --version "0.3.3"
```

The installed command is named `reinhardt-admin`.

## Create the Project

Choose a workspace directory and generate a REST project:

```bash
reinhardt-admin startproject tutorial --template rest
cd tutorial
```

Now create the app that will own the snippets model and API routes:

```bash
reinhardt-admin startapp snippets --template rest
```

`startproject` creates the project shell. `startapp` creates `src/apps/snippets/`, adds the module entry under `src/apps.rs`, and registers the app label in `src/config/apps.rs`. That registration matters later: migrations, route discovery, admin metadata, and generated names all use the app label.

Your project should now have this shape:

```text
tutorial/
+-- Cargo.toml
+-- Makefile.toml
+-- settings/
|   +-- base.toml
|   +-- local.toml
+-- src/
    +-- apps.rs
    +-- config.rs
    +-- lib.rs
    +-- apps/
    |   +-- snippets.rs
    |   +-- snippets/
    |       +-- models.rs
    |       +-- serializers.rs
    |       +-- urls.rs
    |       +-- views.rs
    +-- config/
    |   +-- apps.rs
    |   +-- settings.rs
    |   +-- urls.rs
    +-- bin/
        +-- manage.rs
```

The finished example has a few more files: migrations, Bruno API collections, Docker helpers, and tests. We will add the tutorial-facing pieces as the chapters need them.

## Read the Generated Crate

Open `Cargo.toml`. The reference crate uses the facade `reinhardt` dependency with the features this API needs:

```toml
[dependencies]
reinhardt = { workspace = true, features = [
    "minimal",
    "core",
    "conf",
    "database",
    "db-postgres",
    "db-sqlite",
    "commands",
    "di",
    "server",
    "api",
    "client-router",
] }
tokio = { version = "1.48.0", features = ["full"] }
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.145"
chrono = { version = "0.4", features = ["serde"] }
syntect = "5.2"
```

The important point is not the exact dependency versions. It is that example projects import Reinhardt APIs through the `reinhardt` facade crate, not through internal sub-crates.

## Inspect Settings

Open `settings/base.toml`. The reference example keeps only settings that the current runtime consumes:

```toml
[core]
debug = false
secret_key = "CHANGE_THIS_IN_PRODUCTION"
allowed_hosts = []
installed_apps = []
middleware = []
root_urlconf = ""

[core.security]
secure_ssl_redirect = false
secure_hsts_include_subdomains = false
secure_hsts_preload = false
session_cookie_secure = false
csrf_cookie_secure = false
append_slash = true

[core.databases.default]
engine = "postgresql"
host = "localhost"
port = 5432
name = "examples_tutorial_rest"
user = "reinhardt"
password = "reinhardt"

[contacts]
admins = []
managers = []
```

This file tells the management commands and server where the project database lives. The tutorial uses PostgreSQL for the running app and SQLite in a few tests. If you use the generated `Makefile.toml` tasks, the local PostgreSQL container is started for you.

Now open `src/config/settings.rs`. The reference project composes the `core` and `contacts` fragments into one project settings type:

```rust
use reinhardt::conf::settings::builder::SettingsBuilder;
use reinhardt::conf::settings::profile::Profile;
use reinhardt::conf::settings::sources::{DefaultSource, LowPriorityEnvSource, TomlFileSource};
use reinhardt::core::serde::json;
use reinhardt::settings;
use std::env;
use std::path::PathBuf;

#[settings(core: CoreSettings | contacts: ContactSettings)]
pub struct ProjectSettings;
```

The rest of that file chooses the profile (`local` by default, `ci` in CI), reads `settings/base.toml`, then overlays `settings/<profile>.toml`. That gives you a predictable place for local secrets and CI-only database settings.

## Check App Registration

Open `src/apps.rs`. After `startapp snippets --template rest`, it should expose the app module:

```rust
pub mod snippets;
```

Then open `src/config/apps.rs`. `startapp` should have registered the generated app label automatically:

```rust
use reinhardt::installed_apps;

installed_apps! {
	snippets: "snippets",
}

pub fn get_installed_apps() -> Vec<String> {
	InstalledApp::all_apps()
}
```

This is why you used `startapp` instead of creating the directory by hand. Reinhardt can only discover migrations and app-level routes for apps that are registered here, but the scaffold command should do the registration for you.

## Check URL Mounting

The project-level router lives in `src/config/urls.rs`. The reference example mounts the snippets app under `/api/`:

```rust
use reinhardt::prelude::*;
use reinhardt::routes;

#[routes]
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new().mount("/api/", crate::apps::snippets::urls::url_patterns())
}
```

This does not create any endpoint by itself. It says, "take whatever the snippets app registers, and serve it below `/api/`." In Part 2, `src/apps/snippets/urls.rs` will provide the app router.

## Understand the Manage Binary

The command entry point is `src/bin/manage.rs`:

```rust
use examples_tutorial_rest::config;
use reinhardt::commands::execute_from_command_line_with_settings;
use reinhardt::core::tokio;
use std::process;

#[tokio::main]
async fn main() {
	unsafe {
		std::env::set_var(
			"REINHARDT_SETTINGS_MODULE",
			"examples_tutorial_rest.config.settings",
		);
	}

	let _ = &config::urls::routes;

	if let Err(e) = execute_from_command_line_with_settings(config::settings::get_settings()).await
	{
		eprintln!("Error: {}", e);
		process::exit(1);
	}
}
```

For your generated crate, the module path will use your package name instead of `examples_tutorial_rest`. This binary is the local equivalent of Django's `manage.py`: it loads project settings, makes sure the route macro inventory is linked, and then dispatches commands such as `migrate`, `showurls`, and `runserver`.

## Run the Empty Server

Start the server:

```bash
cargo run --bin manage -- runserver
```

In the reference example, prefer the make task for normal local development:

```bash
cargo make runserver
```

That task starts disposable PostgreSQL and Redis containers, applies migrations, and then runs the same `cargo run --bin manage -- runserver` command. The direct command is useful for understanding the management CLI; the make task is easier when the database-backed chapters are in place.

You should see the server listening on `127.0.0.1:8000`. Leave it running and open another terminal:

```bash
curl http://127.0.0.1:8000/api/snippets/
```

At this point the route may still return 404, because you have not written the snippets app endpoints yet. That is fine. Part 2 adds the first temporary JSON endpoints so the URL starts responding.

## What You Built

You now have:

- A REST project generated from the `rest` template
- A `snippets` app created by `startapp`
- A settings stack based on `ProjectSettings`, `settings/base.toml`, and profile-specific TOML files
- A project router ready to mount app-level routes under `/api/`
- A `manage` binary that loads settings and runs Reinhardt management commands

When you are comfortable with that shell, continue to [Part 2: Your First Endpoints](../2-first-endpoints/).
