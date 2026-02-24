+++
title = "Quickstart"
description = "Get started with Reinhardt — quickstart guide, full setup, and tutorials."
sort_by = "weight"
weight = 10
+++

# Quickstart

Get up and running with Reinhardt in 5 minutes.

## 1. Install Reinhardt Admin CLI

```bash
cargo install reinhardt-admin-cli
```

## 2. Create your project

```bash
reinhardt-admin startproject my-api
cd my-api
```

## 3. Create your first app

```bash
reinhardt-admin startapp hello --template-type restful
```

Edit `hello/views.rs`:

```rust
use reinhardt::prelude::*;
use reinhardt::get;

#[get("/hello", name = "hello_world")]
pub async fn hello_world() -> Result<Response> {
    Response::ok().with_body("Hello, Reinhardt!")
}
```

## 4. Run

```bash
cargo make runserver
```

Visit `http://127.0.0.1:8000/hello` in your browser.

See [Getting Started](/quickstart/getting-started/) for a complete guide, or explore the [Tutorials](/quickstart/tutorials/) to learn by building.
