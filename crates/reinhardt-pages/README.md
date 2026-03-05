# reinhardt-pages

WASM-based reactive frontend framework for Reinhardt with Django-like API.

## Features

- **Fine-grained Reactivity**: Leptos/Solid.js-style Signal system with automatic dependency tracking
- **Hybrid Rendering**: SSR + Client-side Hydration for optimal performance and SEO
- **Django-like API**: Familiar patterns for Reinhardt developers
- **Low-level Only**: Built on wasm-bindgen, web-sys, and js-sys (no high-level framework dependencies)
- **Security First**: Built-in CSRF protection, XSS prevention, and session management
- **Simplified Conditional Compilation**: `cfg_aliases` integration and automatic event handler handling

## Quick Start

### Using the Prelude (Recommended)

The prelude provides all commonly used types with a single import:

```rust
// Instead of multiple scattered imports:
// use reinhardt_pages::{Signal, View, use_state, ...};
// use reinhardt_pages::component::{ElementView, IntoView};
// use reinhardt_pages::reactive::{Effect, Memo};

// Use the unified prelude:
use reinhardt_pages::prelude::*;
// or via reinhardt crate:
use reinhardt::pages::prelude::*;
```

### Platform-Agnostic Event Type

The `platform` module provides unified types that work across both WASM and native:

```rust
use reinhardt_pages::platform::Event;

// Works on both WASM and native targets
fn handle_click(_event: Event) {
    // Event handling logic
}
```

### Simplified cfg Attributes with cfg_aliases

Configure `cfg_aliases` in your project's `build.rs`:

```rust
// build.rs
use cfg_aliases::cfg_aliases;

fn main() {
    // Rust 2024 edition requires explicit check-cfg declarations
    println!("cargo::rustc-check-cfg=cfg(wasm)");
    println!("cargo::rustc-check-cfg=cfg(native)");

    cfg_aliases! {
        wasm: { target_arch = "wasm32" },
        native: { not(target_arch = "wasm32") },
    }
}
```

Add to `Cargo.toml`:

```toml
[build-dependencies]
cfg_aliases = "0.2"
```

Now you can use shorter cfg attributes:

```rust
// Before:
#[cfg(target_arch = "wasm32")]
// After:
#[cfg(wasm)]

// Before:
#[cfg(not(target_arch = "wasm32"))]
// After:
#[cfg(native)]
```

### Automatic Event Handler Handling

The `page!` macro automatically handles event handlers for server-side rendering. You no longer need to write duplicate conditional blocks:

```rust
use reinhardt_pages::prelude::*;

// This works on both WASM and native targets!
// On WASM: Event handlers are bound to DOM events
// On native: Event handlers are automatically ignored
fn my_button(on_click: Signal<bool>) -> View {
    page!(|| {
        button {
            @click: move |_| { on_click.set(true); },
            "Click me"
        }
    })
}
```

**Before** (manual conditional compilation):
```rust
#[cfg(target_arch = "wasm32")]
{
    page!(|| {
        button {
            @click: move |_| { on_click.set(true); },
            "Click me"
        }
    })
}
#[cfg(not(target_arch = "wasm32"))]
{
    let _ = on_click; // suppress warning
    page!(|| {
        button { "Click me" }
    })
}
```

**After** (automatic handling):
```rust
// Just write once - the macro handles everything!
page!(|| {
    button {
        @click: move |_| { on_click.set(true); },
        "Click me"
    }
})
```

### Reactive Conditional Rendering with `watch`

The `watch { expr }` syntax enables reactive re-rendering when Signal dependencies change. Unlike static `if` conditions that are evaluated only at render time, `watch` blocks automatically re-evaluate and update the DOM when their Signal dependencies change.

#### Why `watch` is Needed

When you extract Signal values before the `page!` macro, they become static:

```rust
// Problem: Static values don't update when Signal changes
let has_error = error.get().is_some();  // Static bool captured at render time
page!(|has_error: bool| {
    if has_error {  // This never re-evaluates!
        div { "Error occurred" }
    }
})(has_error)
```

The `watch` syntax solves this by creating a reactive context:

```rust
// Solution: Pass Signal directly and use watch
page!(|error: Signal<Option<String>>| {
    watch {
        if error.get().is_some() {  // Re-evaluates when error changes!
            div { { error.get().unwrap_or_default() } }
        }
    }
})(error.clone())
```

#### Signal-first Pattern

For reactive UIs, pass Signals directly to the `page!` macro instead of extracting values:

```rust
use reinhardt_pages::prelude::*;

fn error_display() -> View {
    let (error, set_error) = use_state(None::<String>);

    // Pass the Signal directly (not the extracted value)
    let error_signal = error.clone();

    page!(|error_signal: Signal<Option<String>>| {
        watch {
            if error_signal.get().is_some() {
                div {
                    class: "alert-danger",
                    { error_signal.get().unwrap_or_default() }
                }
            }
        }
    })(error_signal)
}
```

#### `watch` vs Static `if`

| Syntax | Use Case | Behavior |
|--------|----------|----------|
| `if condition { ... }` | Static conditions, Copy types | Evaluated once at render time |
| `watch { if signal.get() { ... } }` | Signal-dependent conditions | Re-evaluates when Signal changes |
| `watch { match signal.get() { ... } }` | Multiple reactive branches | Re-evaluates when Signal changes |

#### Using `watch` with `match`

The `watch` block also supports `match` expressions:

```rust
page!(|state: Signal<AppState>| {
    watch {
        match state.get() {
            AppState::Loading => div { "Loading..." },
            AppState::Ready(data) => div { { data } },
            AppState::Error(msg) => div { class: "error", { msg } },
        }
    }
})(state.clone())
```

#### Best Practices

1. **Pass Signals directly**: Use `Signal<T>` parameters instead of extracting values
2. **Clone Signals**: `Signal::clone()` is cheap (Rc-based), so clone freely
3. **Single expression**: `watch` blocks must contain exactly one expression
4. **Avoid nesting**: Don't nest `watch` blocks (performance concern)

## Architecture

This framework consists of several key modules:

- **`reactive`**: Fine-grained reactivity system (Signal, Effect, Memo)
- **`dom`**: DOM abstraction layer
- **`builder`**: HTML element builder API
- **`component`**: Component system with IntoView trait
- **`form`**: Django Form integration (native only)
- **`csrf`**: CSRF protection
- **`auth`**: Authentication integration
- **`api`**: API client with Django QuerySet-like interface
- **`server_fn`**: Server Functions (RPC)
- **`ssr`**: Server-side rendering
- **`hydration`**: Client-side hydration
- **`router`**: Client-side routing (reinhardt-urls compatible)
- **`platform`**: Platform abstraction types
- **`prelude`**: Unified imports

## Prelude Contents

The prelude includes:

### Reactive System
- `Signal`, `Effect`, `Memo`, `Resource`, `ResourceState`
- Context: `Context`, `ContextGuard`, `create_context`, `get_context`, `provide_context`, `remove_context`

### Hooks
- `use_state`, `use_effect`, `use_memo`, `use_callback`, `use_context`
- `use_ref`, `use_reducer`, `use_transition`, `use_deferred_value`
- `use_id`, `use_layout_effect`, `use_effect_event`, `use_debug_value`
- `use_optimistic`, `use_action_state`, `use_shared_state`, `use_sync_external_store`

### Component System
- `Component`, `ElementView`, `IntoView`, `View`, `Props`, `ViewEventHandler`

### Events and Callbacks
- `Callback`, `IntoEventHandler`, `into_event_handler`
- `Event` (platform-agnostic via `platform` module)

### DOM
- `Document`, `Element`, `EventHandle`, `EventType`, `document`

### Routing
- `Link`, `Router`, `Route`, `RouterOutlet`, `PathPattern`

### API and Server Functions
- `ApiModel`, `ApiQuerySet`, `Filter`, `FilterOp`
- `ServerFn`, `ServerFnError`
- See [Server Function Macro Guide](docs/server_fn_macro.md) for detailed usage and migration information

### Authentication and Security
- `AuthData`, `AuthError`, `AuthState`, `auth_state`
- `CsrfManager`, `get_csrf_token`

### SSR and Hydration
- `HydrationContext`, `HydrationError`, `hydrate`
- `SsrOptions`, `SsrRenderer`, `SsrState`

### Forms (native only)
- `FormBinding`, `FormComponent`
- `Widget`, `FieldMetadata`, `FormMetadata`

### Macros
- `page!`

### WASM-specific
- `spawn_local` (re-exported from wasm_bindgen_futures)
- `create_resource`, `create_resource_with_deps`

## Example

```rust
use reinhardt_pages::prelude::*;

fn counter() -> View {
    let (count, set_count) = use_state(|| 0);

    page!(|| {
        div {
            p { format!("Count: {}", count.get()) }
            button {
                @click: move |_| set_count.update(|n| *n + 1),
                "Increment"
            }
        }
    })
}
```

## Feature Flags

| Feature | Description |
|---------|-------------|
| `msgpack` | MessagePack serialization support |
| `pages-full` | All features enabled |
| `static` | Static file serving |
| `urls` | URL routing integration |

## License

Licensed under the BSD 3-Clause License.
