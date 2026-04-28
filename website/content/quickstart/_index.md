+++
title = "Quickstart"
description = "Get started with Reinhardt — quickstart guide, full setup, and tutorials."
sort_by = "weight"
weight = 10
+++

# Quickstart

Get up and running with Reinhardt in 5 minutes.

## 1. Install Reinhardt Admin CLI

During the RC phase, only release-candidate versions are published to
crates.io, so `cargo install` requires an explicit `--version`. The version
below is auto-bumped by release-plz on each release. Once a stable release
ships, the bare `cargo install reinhardt-admin-cli` will also work.

<!-- reinhardt-version-sync -->
```bash
cargo install reinhardt-admin-cli --version "0.1.0-rc.23"
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
