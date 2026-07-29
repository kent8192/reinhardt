# Reinhardt Basis Tutorial Example - Polling Application

This example demonstrates the concepts covered in the [Reinhardt Basis Tutorial](../../website/content/quickstart/tutorials/basis/). It implements a complete polling application with two cooperating apps (`polls` and `users`) — typed RPC server functions, per-app route modules, an admin panel, and a WASM single-page-application client all in a single crate.

## What This Example Covers

This example corresponds to the basis tutorial parts 1-7:

- **Part 1: Project Setup and SPA Shell** - Pages template layout, settings, WASM entry point, and `cargo make dev`
- **Part 2: Your First Feature - the Poll Index** - `polls` app, anonymous `Question`/`Choice` migration `0001`, `get_questions`, and the index component
- **Part 3: Detail Pages and Voting** - Detail/results server functions, `form!` voting, reactive errors, and vote persistence
- **Part 4: Users and Authentication** - `users` app, minimal `User`, injectable `AuthUserManager`, session middleware, and auth pages
- **Part 5: Ownership and Poll CRUD** - `Question.author` migration `0002`, ownership-checked question and choice CUD server functions, and owner-only controls
- **Part 6: The Admin and Static Files** - `QuestionAdmin`/`ChoiceAdmin`, `/admin/` route/static registration, WASM artifact registration, and static collection
- **Part 7: Testing** - Native integration tests, `createsuperuser` coverage, and WASM MSW mock tests

## Features

### Models

- **`Question`** (`src/apps/polls/models.rs`) — poll question with `question_text`, `pub_date` (`auto_now_add`), and an `author` foreign key to `User` (`#[rel(foreign_key, related_name = "questions")]`).
- **`Choice`** (`src/apps/polls/models.rs`) — answer option with a `question` foreign key (`#[rel(foreign_key, related_name = "choices")]`), `choice_text`, and a `votes` counter.
- **`User`** (`src/apps/users/models.rs`) — minimal authentication model defined with `#[user(hasher = Argon2Hasher, username_field = "username", manager = false)]` on top of `#[model(app_label = "users", table_name = "users")]`. `manager = false` opts out of the auto-generated user manager so the example can register a project-local `AuthUserManager` via a keyed `#[injectable(scope = "transient")]` provider.

### Server Functions and Pages

The example exposes its dynamic business logic through the pages stack:

- **Typed RPC server functions** in `src/apps/<app>/server_fn.rs` — `#[server_fn]` functions (`get_questions`, `get_question_detail`, `vote`, `create_question`, …, plus `login` / `logout` / `register` / `current_user` for the `users` app). The macro generates a typed client stub for WASM and a server-side handler for native; dependencies are resolved positionally with `#[inject]` (`DatabaseConnection`, `SessionData`, `CurrentUser<User>`, …).
- **Copyable ORM access** — injected `DatabaseConnection` values are lightweight `Copy` handles. Server bootstrap retains the owning lease, so server functions pass handles by value without cloning or managing connection lifetime.
- **Per-app URL modules** in `src/apps/<app>/urls.rs` — each app exposes target-specific router functions from one aggregate; server-function markers stay in `src/apps/<app>/urls/server_router.rs`, client component route tables stay in `src/apps/<app>/urls/client_router.rs`, and `src/config/urls.rs` only aggregates the app-level router functions for the active target.
- **Route-backed components** in `src/apps/<app>/client/components/` — component macros own route metadata for pages such as login/logout/signup.
- **Dynamic WASM forms** in `src/apps/polls/client/components.rs` — the poll detail route builds its `RadioSelect` voting form from the choices returned by `get_question_detail`, so each loaded choice becomes a submitted `choice_id` option.

### URL Structure

The project router mounts per-app server routers on native and merges per-app client routers on WASM through app-level `urls.rs` functions. Route names remain app-local (`polls:*`, `users:*`), and components resolve links through each app's client route reverser.

| Path | Layer | Where it is defined |
|------|-------|---------------------|
| `/` | SPA home (`polls:index`) backed by `get_questions` | `apps/polls/urls.rs` + `apps/polls/server_fn.rs` |
| `/polls/{question_id}/` | SPA detail route (`polls:detail`) backed by `get_question_detail` | `apps/polls/urls.rs` + `server_fn.rs` |
| `/polls/{question_id}/results/` | SPA results route (`polls:results`) backed by `get_question_results` | `apps/polls/urls.rs` + `server_fn.rs` |
| `/polls/new/`, `/polls/{question_id}/edit/`, `/polls/{question_id}/delete/` | Author-only CUD client routes backed by `#[server_fn]`s | `apps/polls/urls.rs` + `apps/polls/server_fn.rs` |
| `/polls/{question_id}/choices/new/`, `…/edit/`, `…/delete/` | Choice CUD client routes backed by `#[server_fn]`s | `apps/polls/urls.rs` + `server_fn.rs` |
| `/login/`, `/logout/`, `/signup/` | Auth client routes; server functions registered in `apps/users/urls/server_router.rs` | `apps/users/urls.rs` + `apps/users/server_fn.rs` |
| `/admin/` | Admin shell and admin route/static wiring. The embedded shell falls back to a placeholder unless the admin WASM SPA is built. | `src/config/admin.rs` mounted in `src/config/urls.rs` |

## Setup

### Prerequisites

- Rust 1.96.0 or later (2024 edition, matches the workspace MSRV)
- `cargo-make` (`cargo install cargo-make`)
- `wasm-pack` for the WASM client build
- Docker (optional, for TestContainers in integration tests)

### Installation

```bash
# From the project root
cd examples/examples-tutorial-basis

# Build the project
cargo build
```

## Usage

### Run the Development Server

The recommended workflow is driven entirely through `cargo make` (defined in `Makefile.toml`):

```bash
# Build the WASM bundle, collect static files, then start the dev server.
# Use this when iterating on either the server or the WASM client.
cargo make dev

# Server only (no WASM rebuild). Runs migrations then `manage runserver --with-pages`.
cargo make runserver

# Optimised path: release-mode WASM build + collectstatic + dev server.
cargo make wasm-build-release
cargo make dev-release
```

The server listens at `http://127.0.0.1:8000/`.

### Explore Models in the Rust Shell

The tutorial exposes the shell through an opt-in `commands-shell` feature that
is absent from its defaults:

```bash
cargo run --bin manage --features commands-shell -- shell
cargo run --bin manage --features commands-shell -- shell -c \
  'println!("{:?}", db.backend())'
```

The feature-gated `config::shell::get_shell_config()` and outer process hook
load the concrete settings, ORM `db` handle, application `di` context,
`framework` alias, and uniquely named installed models such as `Question` and
`Choice`. Ambiguous short model names are skipped with a deterministic warning
that lists their concrete registered crate paths; the evaluator's
`project_crate` alias can reference those same types. A project can extend the
last prelude layer with `ShellConfig::with_prelude(...)`.

Interactive Rust preserves successful definitions, supports top-level
`.await`, and uses `>>> ` / `... ` prompts for complete / multiline input.
Ctrl+C during evaluation, a panic, or evaluator process exit clears user state
and reloads settings, database/DI bindings, model imports, and the project
prelude. `shell -c` evaluates exactly once, exits zero only on success, returns
non-zero on failure, and Reinhardt's own diagnostics do not repeat the raw
source. Arbitrary Rust, compiler output, panics, and user code can still print
literals; the shell is not a sandbox.

History is loaded and saved best-effort at
`<platform local data directory>/reinhardt/shell/examples-tutorial-basis.history`;
a missing file is a silent first run, while directory-resolution, read, or
write failures warn without blocking the prompt. `shell-rhai` was removed, so
`shell` now means the Rust evaluator and does not accept old Rhai syntax.

### Inspect Registered Routes

```bash
# Server URL patterns: server functions, admin routes, and static mounts
cargo make showurls
```

### Verify Migration State

```bash
# Auto-starts the disposable PostgreSQL container, resolves DATABASE_URL from settings/local.toml,
# fills a local DOCKER_HOST fallback when needed, and fails if the committed
# migrations do not match the current models.
cargo make makemigrations-check
```

## Project Structure

```text
examples-tutorial-basis/
├── .gitignore
├── Cargo.toml
├── Makefile.toml
├── README.md
├── build.rs
├── favicon.png
├── index.html
├── migrations/
│   ├── auth/
│   │   └── 0001_initial.rs
│   ├── default/
│   │   └── 0001_initial.rs
│   ├── polls/
│   │   ├── 0001_initial.rs
│   │   └── 0002_question_author.rs
│   └── users/
│       └── 0001_initial.rs
├── scripts/
│   ├── clean-cache.sh
│   ├── db_url.sh
│   ├── docker_host.sh
│   ├── infra_down.sh
│   ├── infra_up.sh
│   ├── parse_local_toml.py
│   ├── run-dev-release-server.sh
│   ├── run-dev-server.sh
│   ├── wasm-build-dev.sh
│   ├── wasm-build-release.sh
│   └── wasm-finalize-release.sh
├── settings/
│   ├── base.toml
│   ├── ci.toml
│   └── local.toml
├── src/
│   ├── apps/
│   │   ├── polls/
│   │   │   ├── client/
│   │   │   │   ├── components.rs
│   │   │   │   └── components/
│   │   │   │       ├── choice_delete.rs
│   │   │   │       ├── choice_edit.rs
│   │   │   │       ├── choice_new.rs
│   │   │   │       ├── polls_detail.rs
│   │   │   │       ├── polls_index.rs
│   │   │   │       ├── polls_results.rs
│   │   │   │       ├── question_delete.rs
│   │   │   │       ├── question_edit.rs
│   │   │   │       └── question_new.rs
│   │   │   ├── client.rs
│   │   │   ├── models.rs
│   │   │   ├── server/
│   │   │   │   ├── admin.rs
│   │   │   │   └── serializers.rs
│   │   │   ├── server.rs
│   │   │   ├── server_fn.rs
│   │   │   ├── services.rs
│   │   │   ├── services/
│   │   │   │   └── server.rs
│   │   │   ├── urls.rs
│   │   │   └── urls/
│   │   │       ├── client_router.rs
│   │   │       └── server_router.rs
│   │   ├── polls.rs
│   │   └── users/
│   │       ├── client/
│   │       │   ├── components.rs
│   │       │   └── components/
│   │       │       ├── login_page.rs
│   │       │       ├── logout_page.rs
│   │       │       └── signup_page.rs
│   │       ├── client.rs
│   │       ├── models.rs
│   │       ├── server.rs
│   │       ├── server_fn.rs
│   │       ├── urls.rs
│   │       └── urls/
│   │           ├── client_router.rs
│   │           └── server_router.rs
│   ├── apps.rs
│   ├── bin/
│   │   └── manage.rs
│   ├── client/
│   │   ├── components/
│   │   │   └── nav.rs
│   │   ├── components.rs
│   │   └── lib.rs
│   ├── client.rs
│   ├── config/
│   │   ├── admin.rs
│   │   ├── apps.rs
│   │   ├── shell.rs
│   │   ├── settings.rs
│   │   ├── urls.rs
│   │   └── wasm.rs
│   ├── config.rs
│   ├── lib.rs
│   ├── shared/
│   │   ├── forms.rs
│   │   └── types.rs
│   └── shared.rs
├── static/
│   ├── css/
│   │   └── style.css
│   └── images/
│       ├── README.md
│       └── poll-icon.svg
└── tests/
    ├── createsuperuser.rs
    ├── integration.rs
    └── wasm/
        ├── polls_mock_test.rs
        └── users_mock_test.rs
```

## Learning Path

This example is designed to be studied alongside the basis tutorial:

1. **Start with the tutorial**: Read [Part 1](../../website/content/quickstart/tutorials/basis/1-project-setup.md)
2. **Examine the code**: Look at how concepts are implemented in this example
3. **Run the tests**: `cargo make test` to see the functionality in action
4. **Experiment**: Modify the code and see what happens

## Key Concepts Demonstrated

- `src/apps/polls/models.rs` defines the `Question` and `Choice` models with `#[model]`, `#[field]`, and `#[rel(foreign_key)]`.
- `src/apps/users/models.rs` defines the tutorial `User` model with `#[user]` and the injectable `AuthUserManager`.
- `src/apps/polls/server_fn.rs` and `src/apps/users/server_fn.rs` expose typed `#[server_fn]` RPC handlers for the WASM client.
- `src/apps/polls/urls.rs` and `src/apps/users/urls.rs` expose the app-level server and client router functions that `src/config/urls.rs` aggregates.
- `src/apps/polls/urls/server_router.rs` and `src/apps/users/urls/server_router.rs` provide native `ServerRouter` registrations.
- `src/client/lib.rs` starts the browser app with `ClientLauncher::new("#root").register_routes_from_inventory().launch()`.
- `src/apps/polls/client/components.rs`, `src/apps/polls/client/components/*.rs`, `src/apps/users/client/components.rs`, `src/apps/users/client/components/*.rs`, and `src/client/components/nav.rs` define the page bodies and route-backed components used by the SPA.
- `src/apps/users/client/components/{login_page,logout_page,signup_page}.rs` define route-backed auth components with the component macro.
- `src/config/settings.rs`, `src/config/apps.rs`, `src/config/urls.rs`, `src/config/admin.rs`, and `src/config/wasm.rs` wire settings, app labels, routing, admin, and WASM static files.
- `tests/integration.rs`, `tests/createsuperuser.rs`, `tests/wasm/polls_mock_test.rs`, and `tests/wasm/users_mock_test.rs` cover native and WASM behavior.

## Testing

Tests are split between native (`tests/integration.rs`, `tests/createsuperuser.rs`) and WASM (`tests/wasm/polls_mock_test.rs`, `tests/wasm/users_mock_test.rs`). The native integration target is gated by `required-features = ["with-reinhardt"]` and the WASM mock targets by `required-features = ["msw"]` — Cargo cannot auto-discover test files under `tests/<subdir>/`, hence the explicit `[[test]]` declarations in `Cargo.toml`. Run them through `cargo make`:

```bash
# All tests (native lib + integration; forwards --all-features)
cargo make test

# Integration tests only
cargo make test-integration

# Unit tests only (library)
cargo make test-unit

# WASM tests in headless Chrome (msw-mocked server functions)
cargo make wasm-test
```

The `msw` feature is forwarded to the `reinhardt` facade so `#[server_fn]` generates the `MockableServerFn` markers that `tests/wasm/polls_mock_test.rs` and `tests/wasm/users_mock_test.rs` consume.

## Next Steps

After understanding this example:

1. **Add richer poll features**: comments, tags, scheduled publication, or poll closing times
2. **Strengthen authorization tests**: extend the native fixture with users plus `author_id` rows and assert author success vs non-author 403 cases
3. **Improve production deployment**: add environment-specific settings, TLS/static hosting strategy, and persistent database configuration
4. **Customize the admin**: add project-specific filters, read-only computed columns, stricter permissions, or a project-specific admin SPA build
5. **Expand WASM coverage**: add browser tests for the create/edit/delete flows beyond the current MSW smoke tests

## Related Documentation

- [Basis Tutorial](../../website/content/quickstart/tutorials/basis/) - Step-by-step guide
- [API Documentation](https://docs.rs/reinhardt-web) - Complete API reference

## License

This example is part of the Reinhardt project and is licensed under the BSD 3-Clause License.
