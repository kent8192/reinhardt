+++
title = "Part 1: Project Setup"
weight = 10

[extra]
sidebar_weight = 10
+++

# Part 1: Project Setup

In this tutorial, we'll create a new Reinhardt project and write our first view.

## Verifying Your Installation

Before we begin, let's verify that Rust and Cargo are installed correctly:

```bash
rustc --version
cargo --version
```

You should see version information for both commands. If not, visit
[rust-lang.org](https://www.rust-lang.org/tools/install) to install Rust.

## Installing Reinhardt Admin CLI

First, install the global tool for project generation. During the RC phase,
only release-candidate versions are published to crates.io, so
`cargo install` requires an explicit `--version`. The version below is
auto-bumped by release-plz on each release. Once a stable release ships, the
bare `cargo install reinhardt-admin-cli` will also work.

<!-- reinhardt-version-sync -->
```bash
cargo install reinhardt-admin-cli --version "0.1.0-rc.21"
```

## Creating a Project

This tutorial uses the **reinhardt-pages template** — a WASM client + server functions + shared types layout. Generate the project from that template:

```bash
# Create a pages-backed project named polls_project
reinhardt-admin startproject polls_project --template pages
cd polls_project
```

The generated tree matches the reference implementation in [`examples/examples-tutorial-basis/`](https://github.com/kent8192/reinhardt-web/tree/main/examples/examples-tutorial-basis):

```text
polls_project/
├── Cargo.toml                 # cdylib + rlib; reinhardt with "pages" + "client-router" features
├── Makefile.toml              # cargo make runserver / migrate / test / collectstatic / ...
├── build.rs                   # cfg_aliases: `native` vs `wasm`
├── README.md
├── settings/
│   ├── base.toml
│   ├── ci.toml
│   └── local.example.toml
└── src/
    ├── lib.rs                 # Entry: #[cfg(native)] server side, #[cfg(wasm)] client side
    ├── bin/
    │   └── manage.rs          # CLI binary (Django's manage.py equivalent)
    ├── config/
    │   ├── settings.rs        # SettingsBuilder with profiles
    │   ├── apps.rs            # installed_apps!{ ... }
    │   ├── urls.rs            # UnifiedRouter + server_fn registration
    │   └── wasm.rs            # dist-wasm/ registered for collectstatic
    ├── apps/                  # (filled in Part 2 onward) — server-only Reinhardt apps
    ├── server_fn/             # (filled in Part 3) — #[server_fn] async fns
    ├── shared/                # DTOs + forms shared between client and server
    │   ├── types.rs
    │   └── forms.rs
    └── client/                # (filled in Part 3) — WASM UI layer
        ├── lib.rs             # #[wasm_bindgen(start)]
        ├── router.rs
        ├── pages.rs
        └── components/
```

**Why this layout?** Three rules keep it predictable:

1. **`#[cfg(native)]` vs `#[cfg(wasm)]`** — server-only code (`apps/`, `server_fn/` bodies) is gated on `native`; browser-only code (`client/`) on `wasm`. `shared/` compiles for both so DTOs stay in sync.
2. **Server functions are the bridge** — the WASM client never touches the database directly. Every request goes through a `#[server_fn]` defined in `src/server_fn/` and returns a DTO from `src/shared/types.rs`.
3. **Reactivity via `page!` + `watch` + `use_action`** — UI updates are declarative WASM components. There is no HTML templating engine.

**Available `cargo make` tasks (defined in `Makefile.toml`):**

| Task | Purpose |
|------|---------|
| `cargo make runserver` | Start the development server |
| `cargo make makemigrations` | Generate migrations from model changes |
| `cargo make migrate` | Apply migrations to the configured database |
| `cargo make collectstatic` | Collect static assets (including `dist-wasm/`) into `staticfiles/` |
| `cargo make test` | `cargo nextest run --all-features` |
| `cargo make showurls` | Print every registered URL pattern |
| `cargo make check` | Project self-check (Django-style `check`) |

> **Note**: This tutorial targets the **reinhardt-pages** architecture end-to-end. If you are instead building a pure JSON backend consumed by an external SPA or mobile client, start with the [REST Tutorial](../rest/0-http-macros/).

## Understanding the Project Structure

Each generated file has a specific role. Walking top-down:

- `Cargo.toml` — declares `crate-type = ["cdylib", "rlib"]` (cdylib for WASM, rlib for the server binary) and pulls in `reinhardt` with the `pages` + `client-router` features for WASM, plus `full`, `conf`, `commands`, `db-sqlite`, `forms` for the server target.
- `build.rs` — uses `cfg_aliases` to create `native` and `wasm` cfgs you will see throughout the source.
- `settings/` — TOML settings files loaded by `SettingsBuilder`. `base.toml` is always loaded; `local.toml` / `ci.toml` / `production.toml` layer on top depending on `REINHARDT_ENV`.
- `src/lib.rs` — the crate root. It `#[cfg(native)]`-gates server-only modules (`apps`, server pieces of `config`, `shared::forms`) and `#[cfg(wasm)]`-gates `client`. `server_fn` and `shared::types` compile for both targets.
- `src/bin/manage.rs` — the server-side binary. It sets `REINHARDT_SETTINGS_MODULE` and calls `reinhardt::commands::execute_from_command_line()`. This is the Django `manage.py` equivalent.
- `src/config/`
  - `settings.rs` — `#[settings(core: CoreSettings)]` + `SettingsBuilder` that resolves the active profile and loads TOML files.
  - `apps.rs` — `installed_apps!{ polls: "polls" }` macro declaring which apps are active.
  - `urls.rs` — defines `routes() -> UnifiedRouter`, registers `#[server_fn]` entries, and mounts server-side `ServerRouter`s.
  - `wasm.rs` — an `inventory::submit!` entry that registers `dist-wasm/` with `collectstatic` so WASM build artifacts end up in `staticfiles/`.
- `src/apps/` — server-only Reinhardt apps (filled in starting from Part 2). Each app owns its models, views, serializers, and a `ServerRouter`.
- `src/server_fn/` — `#[server_fn]` async functions. A single function compiles to both a server implementation and a client stub, giving you type-safe RPC.
- `src/shared/` — code used by both targets. `types.rs` holds DTOs (`QuestionInfo`, `ChoiceInfo`, ...) with `Serialize + Deserialize`; `forms.rs` defines `Form`s used by the `form!` macro (server-only because it imports server-side field types).
- `src/client/` — WASM-only UI. `lib.rs` is the `#[wasm_bindgen(start)]` entry, `router.rs` holds the `reinhardt::pages::router::Router`, `pages.rs` exposes page factories, and `components/` contains the `page!` components that render the UI.

### Architecture: WASM + SSR (reinhardt-pages)

This tutorial uses the **WASM + SSR** architecture with **reinhardt-pages**, which is ideal for:
- Full-stack web applications with integrated frontend and backend
- Single Page Applications (SPAs) with server-side rendering
- Type-safe client-server communication
- Modern reactive user interfaces

**Key characteristics of WASM + SSR projects:**
- Unified codebase for frontend and backend
- Type-safe RPC-style communication (`#[server_fn]` macro)
- Client-side reactivity with server-side rendering
- Single deployment artifact
- WASM compilation for the client-side UI

**Project structure:**

```
my-app/
├── src/
│   ├── client/      # WASM UI (runs in browser)
│   ├── server_fn/   # Server functions (runs on server)
│   └── shared/      # Shared types (used by both)
```

**Alternative: RESTful API Architecture**

If you're building backend APIs for separate frontends (React, Vue, mobile apps), use the **RESTful API** approach instead:

**Key characteristics of RESTful API projects:**
- Server-side only (no WASM compilation)
- HTTP method decorators (`#[get]`, `#[post]`, etc.)
- JSON/XML serialization for data exchange
- Traditional request-response patterns
- Consumed by external clients

**Which should you choose?**
- **WASM + SSR** (this tutorial): When building full-stack applications with integrated UI
- **RESTful API**: When building APIs for multiple clients

For RESTful API development, see the [REST API Tutorial](../rest/0-http-macros/).

See [examples/examples-twitter](https://github.com/kent8192/reinhardt-web/tree/main/examples/examples-twitter) for a complete WASM + SSR implementation.

## Understanding reinhardt-pages Architecture

This tutorial uses **reinhardt-pages**, a modern WASM-based framework for building full-stack applications with:

- **Type-safe RPC** - Server functions (`#[server_fn]`) provide type-safe communication between client and server
- **Reactive UI** - Signal-based reactivity for dynamic user interfaces
- **Single codebase** - Share types and logic between frontend and backend
- **Zero JavaScript** - Write everything in Rust, compile to WASM

### Three-Layer Architecture

```
my-app/
├── src/
│   ├── client/      # WASM UI (runs in browser)
│   ├── server_fn/   # Server functions (runs on server)
│   └── shared/      # Shared types (used by both)
```

**Why this architecture?**

- **Type safety** - The compiler catches mismatches between client and server
- **Developer experience** - No need to write API endpoints manually
- **Performance** - WASM runs at near-native speed in the browser
- **Simplicity** - Single language for frontend and backend

## Understanding Server Functions

Server functions are the backbone of reinhardt-pages applications. They provide a type-safe RPC (Remote Procedure Call) mechanism between the client and server.

### What is a Server Function?

A server function is a Rust async function annotated with `#[server_fn]` that:

1. **Runs on the server** - Executes server-side code with full access to databases, file systems, etc.
2. **Called from the client** - WASM code can call it as if it were a local function
3. **Type-safe** - The compiler ensures type correctness across the network boundary
4. **Automatic serialization** - Arguments and return values are automatically serialized/deserialized

### Basic Example

```rust
use reinhardt::pages::server_fn::{ServerFnError, server_fn};
use crate::shared::types::QuestionInfo;

#[server_fn]
pub async fn get_questions(
    #[inject] _db: reinhardt::DatabaseConnection,
) -> Result<Vec<QuestionInfo>, ServerFnError> {
    // This code runs on the server only
    let questions = Question::objects()
        .all()
        .await
        .map_err(|e| ServerFnError::application(e.to_string()))?;

    Ok(questions.into_iter()
        .map(QuestionInfo::from)
        .collect())
}
```

**Key features:**

- `#[server_fn]` - Defines a server function (auto-detects `#[inject]` parameters)
- `#[inject]` - Automatically injects the database connection
- `Result<T, ServerFnError>` - Required return type for all server functions
- Server-only code - Database queries, file operations, etc.

### Calling from the Client

On the client side (WASM), use `use_action` to call server functions reactively:

```rust
#[cfg(client)]
use crate::server_fn::polls::get_questions;

// In your component
let load_questions = use_action(|_: ()| async move {
    get_questions().await.map_err(|e| e.to_string())
});
load_questions.dispatch(());

// Access results reactively
// load_questions.is_pending()  -- true while loading
// load_questions.result()      -- Option<Vec<QuestionInfo>>
// load_questions.error()       -- Option<String>
```

**What happens under the hood:**

1. Client calls `get_questions()`
2. Arguments are serialized to JSON
3. HTTP POST request sent to server
4. Server deserializes arguments
5. Server executes the function with injected dependencies
6. Result is serialized to JSON
7. Response sent back to client
8. Client deserializes the result

### Dependency Injection in Server Functions

The `#[inject]` attribute enables FastAPI-style dependency injection:

```rust
#[server_fn]
pub async fn create_question(
    question_text: String,
    #[inject] db: reinhardt::DatabaseConnection,
    #[inject] user: CurrentUser,
) -> Result<QuestionInfo, ServerFnError> {
    // db and user are automatically injected by the framework
    let question = Question::new(&question_text);
    question.save(&db).await?;
    Ok(QuestionInfo::from(question))
}
```

**Benefits:**

- **No boilerplate** - No need to manually thread connections through your application
- **Type-safe** - Compiler ensures the right dependencies are available
- **Testable** - Easy to mock dependencies for testing
- **Flexible** - Can inject any registered service

## Creating Your First Component

In reinhardt-pages, UI is built using **components** - Rust functions that return `Page` objects. Components use the `page!` macro for JSX-like syntax and reactive `Signal`s for state management.

### Component Structure

A basic component follows this pattern:

```rust
use reinhardt::pages::component::Page;
use reinhardt::pages::page;
use reinhardt::pages::reactive::hooks::use_state;

pub fn my_component() -> Page {
    // 1. Local state with use_state (for simple values)
    let (message, _set_message) = use_state("Hello, world!".to_string());

    // 2. Clone signal for passing to page! macro
    let message_signal = message.clone();

    // 3. Render UI with page! macro
    page!(|message_signal: Signal<String>| {
        div {
            class: "container",
            h1 { { message_signal.get() } }
        }
    })(message_signal)
}
```

### Creating Your First Component

Let's create a simple polls index component. Create `src/client/components/polls.rs`:

```rust
use reinhardt::pages::component::Page;
use reinhardt::pages::page;
use reinhardt::pages::reactive::hooks::{Action, use_action};
use crate::shared::types::QuestionInfo;

#[cfg(client)]
use crate::server_fn::polls::get_questions;

pub fn polls_index() -> Page {
    // Load questions with use_action (combines loading, error, and data states)
    let load_questions = use_action(|_: ()| async move {
        get_questions().await.map_err(|e| e.to_string())
    });
    load_questions.dispatch(());

    // Clone action for passing to page! macro
    let load_questions_signal = load_questions.clone();

    // Render UI
    page!(|load_questions_signal: Action<Vec<QuestionInfo>, String>| {
        div {
            class: "max-w-4xl mx-auto px-4 mt-12",
            h1 {
                class: "text-3xl font-bold mb-6",
                "Latest Polls"
            }
            watch {
                if load_questions_signal.error().is_some() {
                    div {
                        class: "text-center text-red-500",
                        { load_questions_signal.error().unwrap_or_default() }
                    }
                } else if load_questions_signal.is_pending() {
                    div {
                        class: "text-center",
                        "Loading..."
                    }
                } else if load_questions_signal.result().unwrap_or_default().is_empty() {
                    p {
                        class: "text-gray-500",
                        "No polls are available."
                    }
                } else {
                    div {
                        class: "space-y-2",
                        // Iterate over questions
                        // (simplified - see examples-tutorial-basis for full implementation)
                    }
                }
            }
        }
    })(load_questions_signal)
}
```

### Key Concepts

**1. Async Data Loading with `use_action`**

```rust
let load_data = use_action(|_: ()| async move {
    get_data().await.map_err(|e| e.to_string())
});
load_data.dispatch(());
```

- `use_action` is the primary hook for async data loading in reinhardt-pages
- It combines loading state, error state, and result data into a single `Action<T, E>` value
- Call `.dispatch(())` to trigger the async operation
- No need for `#[cfg(target_arch = "wasm32")]` guards or `spawn_local` -- `use_action` handles platform differences internally

**2. Action State Methods**

```rust
load_data.is_pending()                  // true while loading
load_data.result()                      // Option<T> - the success value
load_data.error()                       // Option<String> - the error message
load_data.result().unwrap_or_default()  // T with fallback
```

The `Action<T, E>` type provides reactive accessors for all async operation states.

**3. Local State with `use_state`**

```rust
let (selected, set_selected) = use_state(None::<i64>);
```

Use `use_state` for simple local UI state (selected items, form inputs, toggle flags). For async data loading, prefer `use_action` instead.

**4. Conditional Rendering with `watch`**

```rust
watch {
    if load_data.is_pending() {
        // Show loading UI
    } else {
        // Show content
    }
}
```

The `watch` block re-evaluates whenever its dependencies (signals or actions) change.

## Setting Up Client Routing

In reinhardt-pages, routing happens on the **client side** (in WASM). The router matches URL paths to component functions and handles navigation without full page reloads.

### Router Configuration

Create `src/client/router.rs`:

```rust
use reinhardt::pages::router::{Router, Route};
use crate::client::pages::{index_page, polls_page, poll_detail_page};

pub fn create_router() -> Router {
    Router::new()
        .route("/", Route::new(index_page))
        .route("/polls/", Route::new(polls_page))
        .route("/polls/{id}/", Route::new(poll_detail_page))
}
```

### Page Functions

Page functions connect routes to components. Create `src/client/pages.rs`:

```rust
use reinhardt::pages::component::Page;
use crate::client::components::polls::{polls_index, polls_detail};

/// Home page
pub fn index_page() -> Page {
    polls_index()
}

/// Polls list page
pub fn polls_page() -> Page {
    polls_index()
}

/// Poll detail page (with dynamic :id parameter)
pub fn poll_detail_page() -> Page {
    // Route parameters are accessed via Router::current_params()
    // In practice, use Router::route_params() for type-safe parameter extraction
    let id = Router::current_params().get::<i64>("id").unwrap_or_default();
    polls_detail(id)
}
```

### Client Entry Point

The client entry point initializes the router. Create `src/client/lib.rs`:

```rust
use wasm_bindgen::prelude::*;
use reinhardt::pages::router::Router;
use crate::client::router::create_router;

#[wasm_bindgen(start)]
pub fn start() {
    // Initialize panic hook for better error messages
    console_error_panic_hook::set_once();

    // Create and mount router
    let router = create_router();
    router.mount("#root");
}
```

### How Client Routing Works

**1. URL Match**

```
/polls/5/ → Match route "/polls/{id}/" → Extract id=5 → Call poll_detail_page()
```

**2. Component Rendering**

```
poll_detail_page() → polls_detail(5) → Render UI with question #5
```

**3. Navigation**

```rust
// Programmatic navigation
// Use Router::push("/polls/5/") to change URL and render new component
```

**4. Link Elements**

```rust
a {
    href: "/polls/5/",
    "View Poll #5"
}
```

Clicking the link triggers client-side navigation (no page reload).

### Key Differences from Server Routing

| Aspect | Server Routing (REST) | Client Routing (Pages) |
|--------|----------------------|------------------------|
| **Where** | Server (HTTP handlers) | Client (WASM) |
| **Route Match** | Per HTTP request | Per URL change in browser |
| **Page Load** | Full page reload | Single Page App (SPA) |
| **URL Parameters** | `Request.path_params` | `Router::current_params()` |
| **Handler** | `async fn(Request) -> Response` | `fn() -> Page` |

**Note**: Reinhardt projects generated with `--template pages` already include client routing configuration. You don't need to manually create routing files for development.

## Running the Development Server

For reinhardt-pages projects, use `cargo make dev` to build WASM and start the development server:

```bash
# Build WASM and start development server
cargo make dev
```

This command:
1. Compiles your Rust code to WASM
2. Generates JavaScript glue code
3. Starts the development server on port 8000
4. Watches for file changes (auto-reload)

**Alternative commands:**

```bash
# Build WASM only (debug mode)
cargo make wasm-build-dev

# Build WASM only (release mode with optimizations)
cargo make wasm-build-release

# Development with watch mode
cargo make dev-watch

# Production build
cargo make build-release
```

You should see output similar to:

```
    Compiling polls_project v0.1.0 (/path/to/polls_project)
     Finished dev [unoptimized + debuginfo] target(s) in 2.34s
      Running `target/debug/runserver`

Reinhardt Development Server
──────────────────────────────────────────────────

  ✓ http://127.0.0.1:8000
  Environment: Debug

Quit the server with CTRL+C
```

Open your web browser and visit `http://127.0.0.1:8000/`. You should see a
welcome message.

Congratulations! Your Reinhardt project is now running!

## Understanding What Happened

Let's review the reinhardt-pages architecture:

1. **Created a server function** (`get_questions`) that runs on the server and returns data
2. **Created a component** (`polls_index`) that renders UI in the browser (WASM)
3. **Set up client routing** to map URLs to components
4. **Started a development server** that serves both server functions and WASM

### The Request Flow

**Traditional Server-Rendered:**
```
Browser → HTTP Request → Server → View Function → HTML → Browser
```

**reinhardt-pages (WASM + Server Functions):**
```
Browser loads WASM → Router matches URL → Component renders
                                        ↓
Component needs data → Server Function call → Server executes → JSON response
                                        ↓
Component updates → UI re-renders with new data
```

**Key Differences:**

| Aspect | Traditional | reinhardt-pages |
|--------|------------|-----------------|
| **Initial Load** | Full HTML page | WASM app + index.html |
| **Navigation** | Page reload | Client-side (SPA) |
| **Data Fetching** | Server renders HTML | Server Functions return JSON |
| **UI Updates** | New page load | Reactive signal updates |
| **Routing** | Server-side | Client-side (WASM) |

**Benefits of this approach:**

- **Type safety** - Compiler checks client ↔ server communication
- **Performance** - WASM runs near-native speed, no page reloads
- **Developer experience** - Single language for everything
- **Rich interactivity** - Signal-based reactivity for dynamic UIs

## Creating the Polls App

In Reinhardt, we organize features into apps (similar to Django). Let's create a
`polls` app:

```bash
reinhardt-admin startapp polls
```

This creates a `polls` directory with the following structure:

```
polls/
├── lib.rs
├── models.rs
├── models/
├── views.rs
├── views/
├── admin.rs
├── urls.rs
└── tests.rs
```

### Creating Server Functions

In reinhardt-pages apps, we define server functions instead of HTTP handlers. Create `src/server_fn/polls.rs`:

```rust
use reinhardt::pages::server_fn::{ServerFnError, server_fn};
use crate::shared::types::QuestionInfo;

/// Get all questions
#[server_fn]
pub async fn get_questions(
    #[inject] _db: reinhardt::DatabaseConnection,
) -> Result<Vec<QuestionInfo>, ServerFnError> {
    use crate::apps::polls::models::Question;
    use reinhardt::Model;

    let questions = Question::objects()
        .all()
        .await
        .map_err(|e| ServerFnError::application(e.to_string()))?;

    Ok(questions.into_iter()
        .take(5)
        .map(QuestionInfo::from)
        .collect())
}
```

### Creating Components

Create `src/client/components/polls.rs`:

```rust
use reinhardt::pages::component::Page;
use reinhardt::pages::page;
use reinhardt::pages::reactive::hooks::{Action, use_action};
use crate::shared::types::QuestionInfo;

#[cfg(client)]
use crate::server_fn::polls::get_questions;

pub fn polls_index() -> Page {
    let load_questions = use_action(|_: ()| async move {
        get_questions().await.map_err(|e| e.to_string())
    });
    load_questions.dispatch(());

    let load_questions_signal = load_questions.clone();

    page!(|load_questions_signal: Action<Vec<QuestionInfo>, String>| {
        div {
            class: "max-w-4xl mx-auto px-4 mt-12",
            h1 {
                class: "text-3xl font-bold mb-6",
                "Polls"
            }
            watch {
                if load_questions_signal.is_pending() {
                    div { "Loading..." }
                } else {
                    div {
                        // Render questions list
                        "Hello, world. You're at the polls index."
                    }
                }
            }
        }
    })(load_questions_signal)
}
```

### Setting Up Client Routing

Create `src/client/router.rs`:

```rust
use reinhardt::pages::router::{Router, Route};
use crate::client::pages::polls_index_page;

pub fn create_router() -> Router {
    Router::new()
        .route("/", Route::new(polls_index_page))
        .route("/polls/", Route::new(polls_index_page))
}
```

Create `src/client/pages.rs`:

```rust
use reinhardt::pages::component::Page;
use crate::client::components::polls::polls_index;

pub fn polls_index_page() -> Page {
    polls_index()
}
```

### Integrating with the Project

In `src/client/lib.rs`:

```rust
mod router;
mod pages;
mod components;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();

    let router = router::create_router();
    router.mount("#root");
}
```

### Registering the App

Edit `src/config/apps.rs`:

```rust
use reinhardt::installed_apps;

installed_apps! {
    polls: "polls",
}

pub fn get_installed_apps() -> Vec<String> {
    InstalledApp::all_apps()
}
```

**Understanding `installed_apps!`**

The `installed_apps!` macro registers your application modules with Reinhardt's
app registry. This enables several framework features:

**What it does:**

1. **Auto-discovery** - Reinhardt automatically discovers:
   - Models for migrations
   - Admin panel registrations
   - Static files and templates
   - Management commands

2. **Type-safe references** - Creates type-safe app identifiers:
   ```rust
   // You can reference apps by name in code
   InstalledApp::Polls  // Type-safe reference to "polls" app
   ```

3. **Configuration registry** - Centralized app management
   - Apps are initialized in declaration order
   - Dependencies between apps can be managed
   - Apps can be conditionally included based on features

**Basic usage (single app):**

```rust
installed_apps! {
    polls: "polls",  // Your app
}
```

**Equivalent to Django's INSTALLED_APPS:**

Unlike Django, Reinhardt separates concerns:
- **User apps**: Registered via `installed_apps!` macro
- **Built-in features**: Enabled via Cargo feature flags

Python (Django):
```python
INSTALLED_APPS = [
    'django.contrib.auth',      # ← Framework feature
    'django.contrib.admin',     # ← Framework feature
    'polls',                    # ← User app
]
```

Rust (Reinhardt):
{% versioned_code(lang="toml") %}
# Cargo.toml - Enable framework features
[dependencies]
reinhardt = { version = "LATEST_VERSION", package = "reinhardt-web", features = ["auth", "admin"] }
{% end %}

```rust
// src/config/apps.rs - Register user apps only
use reinhardt::installed_apps;

installed_apps! {
    polls: "polls",
}
```

**Why the two-part syntax?**

- `polls:` - The identifier used in Rust code (`InstalledApp::Polls`)
- `"polls"` - The module path where the app is located

This allows flexibility in naming while keeping code references clean.

### Built-in Framework Features

Reinhardt's built-in features (auth, admin, sessions, etc.) are **NOT** registered
via `installed_apps!`. Instead, they are enabled through Cargo feature flags.

**Available Built-in Features**:

| Feature | Cargo.toml | Import |
|---------|------------|--------|
| Authentication | `features = ["auth"]` | `use reinhardt::{IsAuthenticated, AllowAny, JwtAuth};` |
| Admin Panel | `features = ["admin"]` | `use reinhardt::admin::*;` |
| Sessions | `features = ["sessions"]` | `use reinhardt::sessions::*;` |
| REST API | `features = ["rest"]` | `use reinhardt::rest::*;` |
| Database | `features = ["database"]` | `use reinhardt::{QuerySet, DatabaseConnection};` |

**Example Configuration**:

{% versioned_code(lang="toml") %}
# Cargo.toml
[dependencies]
reinhardt = {
    version = "LATEST_VERSION",
    package = "reinhardt-web",
    default-features = false,
    features = ["standard"]  # Includes auth, database, REST API
}
{% end %}

For a complete list of available features, see the [Feature Flags Guide](/docs/feature-flags/).

**Why This Design?**

Unlike Django's runtime registration, Reinhardt uses compile-time feature flags:
- ✅ **Zero overhead**: Unused features are not compiled
- ✅ **Faster builds**: Only compile what you need
- ✅ **Type safety**: Features are validated at compile time
- ✅ **Smaller binaries**: Exclude unnecessary code

Restart your server (press Ctrl-C and run `cargo make runserver` again) and
visit `http://127.0.0.1:8000/polls/`. You should see the message.

## Adding More Components and Server Functions

Let's add components for viewing poll details and results. This demonstrates how to handle dynamic routing parameters and multiple server functions.

### Additional Server Functions

Add to `src/server_fn/polls.rs`:

```rust
/// Get question detail with choices
#[server_fn]
pub async fn get_question_detail(
    question_id: i64,
    #[inject] _db: reinhardt::DatabaseConnection,
) -> Result<(QuestionInfo, Vec<ChoiceInfo>), ServerFnError> {
    use crate::apps::polls::models::{Question, Choice};
    use reinhardt::Model;

    // Get question
    let question = Question::objects()
        .get(question_id)
        .await
        .map_err(|e| ServerFnError::application(e.to_string()))?;

    // Get choices for this question
    let choices = Choice::objects()
        .filter(Choice::field_question().eq(question_id))
        .all()
        .await
        .map_err(|e| ServerFnError::application(e.to_string()))?;

    Ok((
        QuestionInfo::from(question),
        choices.into_iter().map(ChoiceInfo::from).collect()
    ))
}

/// Submit a vote
#[server_fn]
pub async fn vote(
    question_id: i64,
    choice_id: i64,
    #[inject] _db: reinhardt::DatabaseConnection,
) -> Result<ChoiceInfo, ServerFnError> {
    use crate::apps::polls::models::Choice;
    use reinhardt::Model;

    let mut choice = Choice::objects()
        .get(choice_id)
        .await
        .map_err(|e| ServerFnError::application(e.to_string()))?;

    choice.votes += 1;
    choice.save(&_db).await
        .map_err(|e| ServerFnError::application(e.to_string()))?;

    Ok(ChoiceInfo::from(choice))
}
```

### Detail Component

Add to `src/client/components/polls.rs`:

```rust
/// Poll detail page with voting form
pub fn polls_detail(question_id: i64) -> Page {
    let load_detail = use_action(move |_: ()| async move {
        get_question_detail(question_id).await.map_err(|e| e.to_string())
    });
    load_detail.dispatch(());

    let load_detail_signal = load_detail.clone();

    page!(|load_detail_signal: Action<(QuestionInfo, Vec<ChoiceInfo>), String>| {
        div {
            class: "max-w-4xl mx-auto px-4 mt-12",
            watch {
                if load_detail_signal.error().is_some() {
                    div {
                        class: "text-red-500",
                        { load_detail_signal.error().unwrap_or_default() }
                    }
                }
            }
            watch {
                if load_detail_signal.is_pending() {
                    div { "Loading..." }
                } else if let Some((q, _choices)) = load_detail_signal.result() {
                    div {
                        h1 {
                            class: "text-3xl font-bold mb-6",
                            { q.question_text }
                        }
                        // Voting form (simplified)
                        // See examples-tutorial-basis for full implementation
                    }
                }
            }
        }
    })(load_detail_signal)
}
```

### Update Router

Update `src/client/router.rs`:

```rust
use reinhardt::pages::router::{Router, Route};
use crate::client::pages::{
    polls_index_page,
    poll_detail_page,
    poll_results_page,
};

pub fn create_router() -> Router {
    Router::new()
        .route("/", Route::new(polls_index_page))
        .route("/polls/", Route::new(polls_index_page))
        .route("/polls/{id}/", Route::new(poll_detail_page))
        .route("/polls/{id}/results/", Route::new(poll_results_page))
}
```

Update `src/client/pages.rs`:

```rust
use reinhardt::pages::component::Page;
use crate::client::components::polls::{polls_index, polls_detail, polls_results};

pub fn polls_index_page() -> Page {
    polls_index()
}

pub fn poll_detail_page() -> Page {
    // Route parameters are accessed via Router::current_params()
    // In practice, use Router::route_params() for type-safe parameter extraction
    let id = Router::current_params().get::<i64>("id").unwrap_or_default();
    polls_detail(id)
}

pub fn poll_results_page() -> Page {
    // Route parameters are accessed via Router::current_params()
    // In practice, use Router::route_params() for type-safe parameter extraction
    let id = Router::current_params().get::<i64>("id").unwrap_or_default();
    polls_results(id)
}
```

### Try the Application

Run the development server:

```bash
cargo make dev
```

Visit these URLs:

- `http://127.0.0.1:8000/` - Shows the polls index
- `http://127.0.0.1:8000/polls/1/` - Shows detail for question 1
- `http://127.0.0.1:8000/polls/1/results/` - Shows results for question 1

**Note**: Navigation happens client-side (no page reload) thanks to the WASM router

## What's Next?

We've created a basic Reinhardt project with URL routing and simple views. In
the next tutorial, we'll set up a database and create models to store poll
questions and choices.

When you're ready, move on to
[Part 2: Models and Database](../2-models-and-database/).

## Summary

In this tutorial, you learned:

- How to create a new reinhardt-pages project with WASM support
- How to use **server functions** (`#[server_fn]`) for type-safe RPC communication
- How to create **components** with the `page!` macro
- How to load data asynchronously with `use_action()` and manage loading/error/result states
- How to set up **client-side routing** with dynamic parameters
- How to manage simple local state with `use_state()`
- The **three-layer architecture** (client, server_fn, shared)

### Key Takeaways

**Server Functions:**
- Run on the server with full access to databases and services
- Called from WASM as if they were local functions
- Automatically handle serialization and network communication
- Support dependency injection with `#[inject]`

**Components:**
- Pure Rust functions that return `Page` objects
- Use `page!` macro for JSX-like UI syntax
- Load async data with `use_action` (combines loading, error, and result states)
- Use `use_state` for simple local state (selections, toggles, form inputs)
- Re-render automatically when state changes

**Architecture:**
- **Client** (`src/client/`) - WASM UI code
- **Server Functions** (`src/server_fn/`) - Server-side business logic
- **Shared** (`src/shared/`) - Types and logic used by both

**Next Steps:**
- Learn about **models and databases** in Part 2
- Explore **forms** with the `form!` macro in Part 4
- See **complete examples** in `examples/examples-tutorial-basis/`

You now have a solid foundation for building full-stack Rust applications with reinhardt-pages!
