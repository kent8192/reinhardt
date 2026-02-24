# reinhardt-pages cfg Simplification Migration Guide

This guide explains how to introduce the reinhardt-pages cfg simplification features to existing projects.

## Overview

### What's Changed

1. **prelude module**: Unified imports for commonly used types
2. **platform module**: Common type aliases for WASM/native
3. **cfg_aliases**: `#[cfg(wasm)]` and `#[cfg(native)]` shortcuts
4. **page! macro improvements**: Event handlers are automatically ignored on the server side

### Benefits

- Significantly reduces `#[cfg(target_arch = "wasm32")]` boilerplate
- Eliminates import duplication
- Improves component code readability

---

## Migration Steps

### Step 1: Update Cargo.toml

Add `cfg_aliases` to the `[build-dependencies]` section:

```toml
[build-dependencies]
cfg_aliases = "0.2"
```

### Step 2: Create/Update build.rs

Create `build.rs` in your project root if it doesn't exist, or update it:

```rust
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

### Step 3: Update Imports

#### Before

```rust
use reinhardt_pages::{Signal, View, use_state};
use reinhardt_pages::component::{ElementView, IntoView};
use reinhardt_pages::reactive::{Effect, Memo};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;
```

#### After

```rust
use reinhardt_pages::prelude::*;
// spawn_local is also included in prelude (WASM only)
```

Or via the reinhardt crate:

```rust
use reinhardt::pages::prelude::*;
```

### Step 4: Shorten cfg Attributes

#### Before

```rust
#[cfg(target_arch = "wasm32")]
mod client;

#[cfg(not(target_arch = "wasm32"))]
mod server;
```

#### After

```rust
#[cfg(wasm)]
mod client;

#[cfg(native)]
mod server;
```

### Step 5: Simplify Event Handlers

Event handlers within the `page!` macro are now automatically handled.

#### Before (manual conditional branching)

```rust
pub fn button(on_click: Signal<bool>) -> View {
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
        let _ = on_click; // Suppress unused warning
        page!(|| {
            button { "Click me" }
        })
    }
}
```

#### After (automatic handling)

```rust
pub fn button(on_click: Signal<bool>) -> View {
    // The macro automatically ignores event handlers on the server side
    page!(|| {
        button {
            @click: move |_| { on_click.set(true); },
            "Click me"
        }
    })
}
```

### Step 6: Use the platform Module (Optional)

If you want to abstract platform-specific types:

```rust
use reinhardt_pages::platform::Event;

// WASM: web_sys::Event
// Native: DummyEvent
fn handle_event(_event: Event) {
    // Processing
}
```

---

## Checklist

- [ ] Add `cfg_aliases = "0.2"` to `Cargo.toml`
- [ ] Set up `cfg_aliases!` in `build.rs`
- [ ] Unify imports to `prelude::*`
- [ ] Change `#[cfg(target_arch = "wasm32")]` to `#[cfg(wasm)]`
- [ ] Change `#[cfg(not(target_arch = "wasm32"))]` to `#[cfg(native)]`
- [ ] Remove duplicate event handler blocks in components
- [ ] Verify with `cargo check`

---

## Troubleshooting

### Getting `unknown cfg: wasm` Warning

Set `cargo::rustc-check-cfg` in `build.rs`:

```rust
println!("cargo::rustc-check-cfg=cfg(wasm)");
println!("cargo::rustc-check-cfg=cfg(native)");
```

### Import Conflicts with prelude

If you want to import only specific types, import them explicitly:

```rust
use reinhardt_pages::prelude::{Signal, View, use_state};
// Don't use other types from prelude
```

### Warnings for Captured Variables in Event Handlers

The `page!` macro automatically suppresses warnings for captured variables, but you may need to handle variables defined outside the macro manually:

```rust
#[cfg(wasm)]
let handler = move |_| { /* ... */ };

#[cfg(native)]
let handler = |_: reinhardt_pages::platform::Event| {};
```

---

## Related Documentation

- [reinhardt-pages README](../crates/reinhardt-pages/README.md)
- [Feature Flags Guide](FEATURE_FLAGS.md)
- [Getting Started Guide](GETTING_STARTED.md)
