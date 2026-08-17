# `page!` Macro

`page!` is the Pages view DSL. It builds a `Page` value from HTML-like Rust
tokens and keeps the same component source usable for native rendering,
component tests, and the WASM client.

## Choose a body form

Use the body form when the page is rendered immediately. Values referenced from
the surrounding function are captured and cloned into generated reactive and
event-handler closures:

```rust,ignore
use reinhardt::pages::prelude::*;

fn greeting(name: String) -> Page {
    page!({
        section {
            h1 { { format!("Hello, {name}") } }
        }
    })
}
```

Use the closure form when the result is a reusable factory. Every value used by
the body must be listed as a parameter:

```rust,ignore
let greeting = page!(|name: String| {
    h1 { { format!("Hello, {name}") } }
});

let view = greeting("Ada".to_string());
```

Do not use bare identifiers as child shorthand. Write `{value}` explicitly so
the parser can distinguish a value from a nested element:

```rust,ignore
page!({ p { {message} } });
```

## Reactive rendering

Expressions, `if`, and `for` blocks are reactive render scopes. Read a signal
inside the `page!` body so the affected subtree is rebuilt when it changes:

```rust,ignore
page!({
    if count.get() == 0 {
        p { "Nothing yet" }
    } else {
        p { { format!("Count: {}", count.get()) } }
    }
});
```

Precomputing `let has_items = items.get().is_empty();` before the macro creates
a static value and will not track later signal changes. A `for` iterator is
cloned for each reactive run, so its expression must implement `Clone`. Use
`@key(expression)` when list items have stable identities.

## Events and components

Event handlers are written once. `page!` stores them for native rendering and
component tests, and binds them to DOM events on WASM; do not duplicate the
markup with target `cfg` branches:

```rust,ignore
page!({
    button {
        @click: move |_| count.update(|current| current + 1),
        "Increment"
    }
});
```

Use `{function(args)}` for a normal component call or the brace form for a
component with named props. Route-backed components still belong in an app's
client module and are registered by that app's `urls/client_router.rs`.

## Target and formatting rules

- Keep `page!` declarations in client-owned app modules; shared data should be
  passed as serializable props or DTOs.
- `#[cfg(client)]` is for module boundaries and client-only dependencies, not
  for duplicating a page body or event handler.
- Run `cargo make fmt-check` after changing `page!`; this invokes the Pages DSL
  formatter as well as rustfmt.
- Keep the syntax in this guide aligned with the current Pages macro version;
  the `page!` parser validates the body before code generation.

