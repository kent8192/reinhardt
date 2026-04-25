+++
title = "Basics Tutorials"
description = "Core concepts and fundamentals of Reinhardt."
sort_by = "weight"
weight = 10

[extra]
sidebar_weight = 10
+++


# Reinhardt Basis Tutorial

Learn the fundamentals of the Reinhardt framework by building a real-world polling application on the **reinhardt-pages template** (WASM client + server functions + shared types).

## Overview

This tutorial series walks you through building a fully functional polling application from scratch. The reference implementation lives under [`examples/examples-tutorial-basis`](https://github.com/kent8192/reinhardt-web/tree/main/examples/examples-tutorial-basis); following the chapters in order will produce a project that is logically equivalent to it.

Reinhardt's basis tutorial is intentionally different from a classic server-rendered Django-style stack: the UI is a Rust-compiled WebAssembly (WASM) client, the backend exposes typed **server functions** via `#[server_fn]`, and client and server share the same DTO types. You will see each of these layers introduced explicitly below.

## Who This Tutorial Is For

- Developers new to Reinhardt who want to learn the framework from the ground up
- Django developers transitioning to Rust who want to understand Reinhardt's pages architecture
- Anyone building full-stack web applications where the browser runs Rust (WASM) and talks to a server function backend

## Prerequisites

- Basic knowledge of Rust programming
- Familiarity with Cargo and `cargo make`
- Understanding of HTTP concepts and web development
- A code editor or IDE

## What You'll Build

A polling application where end users can:

- View the latest polls on a WASM-rendered index page
- Open a poll, vote on a choice, and see the result update reactively
- See aggregated voting results on a results page

Administrators can:

- Create and manage polls via the Reinhardt admin (registered as a WASM admin component)
- Add and edit choices for each poll

## The Pages Template at a Glance

Every chapter maps onto this layout, which matches the completed example under `examples/examples-tutorial-basis/`:

```text
examples-tutorial-basis/
├── Cargo.toml                 # cdylib + rlib; reinhardt with "pages" + "client-router" features
├── Makefile.toml              # cargo make tasks: runserver, migrate, collectstatic, test, ...
├── build.rs                   # cfg_aliases: `native` vs `wasm`
├── settings/                  # TOML settings (base.toml, ci.toml, local.example.toml)
├── src/
│   ├── lib.rs                 # Entry point; #[cfg(native)] vs #[cfg(wasm)] gating
│   ├── bin/
│   │   └── manage.rs          # CLI binary (manage.py equivalent)
│   ├── config/
│   │   ├── settings.rs        # SettingsBuilder + profiles
│   │   ├── apps.rs            # installed_apps!{ polls: "polls" }
│   │   ├── urls.rs            # UnifiedRouter + server_fn registration
│   │   └── wasm.rs            # collectstatic entry for dist-wasm/
│   ├── apps/                  # Server-only Reinhardt apps
│   │   └── polls/
│   │       ├── models.rs      # #[model] structs (Question, Choice)
│   │       ├── urls.rs        # ServerRouter for HTTP endpoints
│   │       └── views.rs       # #[get]/#[post] handlers
│   ├── server_fn/             # Server functions, callable from WASM client
│   │   └── polls.rs           # #[server_fn] async fns (get_questions, submit_vote, ...)
│   ├── shared/                # Types and forms shared between WASM and server
│   │   ├── types.rs           # DTOs (QuestionInfo, ChoiceInfo, VoteRequest)
│   │   └── forms.rs           # Server-only Form definitions used by form! on the client
│   └── client/                # WASM-only UI layer
│       ├── lib.rs             # #[wasm_bindgen(start)] entry, mounts router
│       ├── router.rs          # reinhardt::pages::router::Router
│       ├── pages.rs           # Page factory functions
│       └── components/polls.rs# page! { ... } + watch blocks + form!
└── tests/integration.rs       # rstest + reinhardt-test fixtures (AAA pattern)
```

Three rules keep this structure predictable:

1. **Native vs WASM** — `#[cfg(native)]` code runs on the server (models, views, server_fn bodies). `#[cfg(wasm)]` code runs in the browser (client UI). `shared` modules compile for both.
2. **Server functions are the bridge** — anything the WASM client needs from the database goes through a `#[server_fn]` in `src/server_fn/`. Client and server exchange the DTOs in `src/shared/types.rs`.
3. **Reactivity via `page!` + `watch` + `use_action`** — UI updates happen declaratively in WASM components; there is no HTML templating engine.

## Tutorial Structure

### [Part 1: Project Setup](1-project-setup/)

- Generate a project from the reinhardt-pages template
- Understand the `src/{lib,client,server_fn,shared,apps,config,bin}` layout
- Configure `settings/base.toml` and run the dev server with `cargo make runserver`

### [Part 2: Models and Database](2-models-and-database/)

- Define `Question` and `Choice` with `#[model(app_label = "polls", table_name = "...")]`
- Place models under `src/apps/polls/models.rs`
- Run `cargo make makemigrations` and `cargo make migrate` from the project root

### [Part 3: Views and URLs](3-views-and-urls/)

- Write **server functions** (`#[server_fn]`) in `src/server_fn/polls.rs` — this is the "views" layer of the pages architecture
- Register them in `src/config/urls.rs` with `UnifiedRouter::new().server(|s| s.server_fn(...))`
- Mount server-rendered `ServerRouter` endpoints from `src/apps/polls/urls.rs` at `/polls/`
- Add client routes in `src/client/router.rs` that render `page!` components

### [Part 4: Forms and Generic Views](4-forms-and-generic-views/)

- Define `VotingForm` in `src/shared/forms.rs` using `Form::new().add_field(...)`
- Build the voting UI with the **`form!` macro** + `watch { ... }` blocks inside a `page!` component
- Call `submit_vote` (a `#[server_fn]`) on submit; show server validation errors reactively

### [Part 5: Testing](5-testing/)

- Use `rstest` fixtures + `reinhardt-test` helpers to spin up an in-memory SQLite
- Follow the Arrange-Act-Assert pattern with `// Arrange`, `// Act`, `// Assert` labels
- Exercise models, server functions, and shared DTO conversions

### [Part 6: Static Files](6-static-files/)

- Understand the two static-asset tiers used by the pages template:
  - `dist-wasm/` — output of `wasm-pack` / WASM build, registered via `src/config/wasm.rs`
  - `staticfiles/` — final output of `cargo make collectstatic`, served at `/static/`
- Wire it all up through `Makefile.toml` tasks

### [Part 7: Admin Customization](7-admin-customization/)

- Register `ModelAdmin` implementations server-side
- Mount the admin UI as a **WASM component** in `src/client/` so the admin renders inside the same pages runtime
- Customize list columns, search, and change forms

## Recommended Learning Path

Work through the chapters in order. Each chapter assumes the directory layout produced by the previous one. If you get stuck, compare your tree against [`examples/examples-tutorial-basis/`](https://github.com/kent8192/reinhardt-web/tree/main/examples/examples-tutorial-basis) — the reference source is the authoritative answer key.

## Getting Help

- [Reinhardt repository](https://github.com/kent8192/reinhardt-web)
- [Getting Started Guide](/quickstart/getting-started/)
- [Feature Flags Guide](/docs/feature-flags/)
- [GitHub Discussions](https://github.com/kent8192/reinhardt-web/discussions)

## Comparison with REST Tutorial

If you're also interested in building pure JSON APIs, see the [REST Tutorial](../rest/quickstart/).

- **Basis Tutorial** (this one): full-stack pages template — WASM client + `#[server_fn]` + shared DTOs.
- **REST Tutorial**: `#[get]` / `#[post]` views, `Serializer`s, and `ViewSet` + `Router` for classic REST endpoints.

The underlying model and database layers are identical, so lessons transfer in both directions.

## Let's Get Started!

Head over to [Part 1: Project Setup](1-project-setup/) to generate your first pages project.
