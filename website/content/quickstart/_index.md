+++
title = "Quickstart"
description = "Get started with Reinhardt — quickstart guide, full setup, and tutorials."
sort_by = "weight"
weight = 10
+++

# Quickstart

Get up and running with Reinhardt in 5 minutes.

## 1. Install Reinhardt Admin CLI

Install the CLI with Cargo. The command below pins this guide to the
documented release for reproducibility; omit `--version` to let Cargo choose
the latest stable release. The literal below is release-managed.

<!-- reinhardt-version-sync -->
```bash
cargo install reinhardt-admin-cli --version "0.3.3"
```

## 2. Create your project

```bash
reinhardt-admin startproject my-api
cd my-api
```

## 3. Create your first app

```bash
reinhardt-admin startapp hello --template rest
```

Edit `hello/views.rs`:

```rust
use reinhardt::prelude::*;
use reinhardt::get;

#[get("/hello", name = "hello_world")]
pub async fn hello_world() -> ViewResult<Response> {
    Response::ok().with_body("Hello, Reinhardt!")
}
```

## 4. Run

```bash
cargo make runserver
```

Visit `http://127.0.0.1:8000/hello` in your browser.

See [Getting Started](/quickstart/getting-started/) for a complete guide, or explore the [Tutorials](/quickstart/tutorials/) to learn by building.

Upgrading an existing 0.2.x application? Follow the
[0.2.x to 0.3.0 migration guide](/quickstart/migration-guides/0-2-to-0-3/).
