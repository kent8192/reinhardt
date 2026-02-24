# Migration Guide: SsrOptions to head! Macro

This guide explains how to migrate from the legacy `SsrOptions` head methods to the new `head!` macro system.

## Overview

The `head!` macro provides a more declarative and type-safe way to define HTML `<head>` sections. The legacy methods on `SsrOptions` (`title()`, `meta()`, `css()`, `js()`) are now deprecated and will be removed in a future version.

## Migration Timeline

| Version | Status |
|---------|--------|
| 0.2.0   | `head!` macro introduced, legacy methods deprecated |
| 0.3.0   | Legacy methods removed (planned) |

## Quick Reference

### Before (Legacy)

```rust
use reinhardt_pages::ssr::{SsrOptions, SsrRenderer};

let options = SsrOptions::new()
    .title("My Page")
    .meta("description", "Page description")
    .css("/style.css")
    .js("/app.js");

let mut renderer = SsrRenderer::with_options(options);
let html = renderer.render_to_html(&view);
```

### After (New)

```rust
use reinhardt_pages::{head, page};
use reinhardt_pages::component::{Head, MetaTag, LinkTag, ScriptTag};
use reinhardt_pages::ssr::SsrRenderer;

let page_head = head!(|| {
    title { "My Page" }
    meta { name: "description", content: "Page description" }
    link { rel: "stylesheet", href: "/style.css" }
    script { src: "/app.js" }
});

let view = page! {
    #head: page_head,
    || {
        div { "Content" }
    }
}();

let mut renderer = SsrRenderer::new();
let html = renderer.render_page_with_view_head(view);
```

## Detailed Migration Steps

### Step 1: Replace title()

**Before:**
```rust
SsrOptions::new().title("My Page")
```

**After:**
```rust
head!(|| {
    title { "My Page" }
})
```

### Step 2: Replace meta()

**Before:**
```rust
SsrOptions::new()
    .meta("description", "Page description")
    .meta("author", "Author Name")
```

**After:**
```rust
head!(|| {
    meta { name: "description", content: "Page description" }
    meta { name: "author", content: "Author Name" }
})
```

For Open Graph and other property-based meta tags:

```rust
head!(|| {
    meta { property: "og:title", content: "Page Title" }
    meta { property: "og:description", content: "Description" }
})
```

### Step 3: Replace css()

**Before:**
```rust
SsrOptions::new()
    .css("/style.css")
    .css("/theme.css")
```

**After:**
```rust
head!(|| {
    link { rel: "stylesheet", href: "/style.css" }
    link { rel: "stylesheet", href: "/theme.css" }
})
```

### Step 4: Replace js()

**Before:**
```rust
SsrOptions::new()
    .js("/app.js")
    .js("/vendor.js")
```

**After:**
```rust
head!(|| {
    script { src: "/app.js" }
    script { src: "/vendor.js" }
})
```

For module scripts:
```rust
head!(|| {
    script { src: "/app.mjs", type: "module" }
})
```

### Step 5: Update Rendering

**Before:**
```rust
let mut renderer = SsrRenderer::with_options(options);
let html = renderer.wrap_in_html(&content);
```

**After:**
```rust
let view_with_head = view.with_head(page_head);
let mut renderer = SsrRenderer::new();
let html = renderer.render_page_with_view_head(view_with_head);
```

## Using #head in page! Macro

The `page!` macro now supports the `#head` directive for declarative head attachment:

```rust
let page_head = head!(|| {
    title { "My Page" }
    meta { name: "description", content: "Description" }
});

page! {
    #head: page_head,
    || {
        div { class: "container",
            h1 { "Welcome" }
        }
    }
}
```

## Additive Behavior

When using `render_page_with_view_head()`, the View's head is **additive** to any existing `SsrOptions` head elements:

1. **Title**: View's title takes precedence if present
2. **Meta tags**: Both sources are included
3. **CSS links**: Both sources are included
4. **Scripts**: Both sources are included

This allows gradual migration without breaking existing functionality.

## Using resolve_static()

For static file URL resolution (with collectstatic support):

```rust
use reinhardt_pages::resolve_static;

let page_head = head!(|| {
    link { rel: "stylesheet", href: resolve_static("css/style.css") }
    script { src: resolve_static("js/app.js") }
});
```

Note: `resolve_static` requires initialization via `init_static_resolver()` at application startup.

## Complete Migration Example

### Before (Legacy)

```rust
use reinhardt_pages::ssr::{SsrOptions, SsrRenderer};
use reinhardt_pages::page;

fn render_home_page() -> String {
    let options = SsrOptions::new()
        .title("Home - My App")
        .meta("description", "Welcome to my app")
        .css("/static/css/main.css")
        .js("/static/js/app.js");

    let view = page!(|| {
        div { class: "container",
            h1 { "Welcome Home" }
            p { "This is the home page." }
        }
    })();

    let mut renderer = SsrRenderer::with_options(options);
    renderer.render_to_html(&view)
}
```

### After (New)

```rust
use reinhardt_pages::{head, page, resolve_static};
use reinhardt_pages::ssr::SsrRenderer;

fn render_home_page() -> String {
    let page_head = head!(|| {
        title { "Home - My App" }
        meta { name: "description", content: "Welcome to my app" }
        link { rel: "stylesheet", href: resolve_static("css/main.css") }
        script { src: resolve_static("js/app.js") }
    });

    let view = page! {
        #head: page_head,
        || {
            div { class: "container",
                h1 { "Welcome Home" }
                p { "This is the home page." }
            }
        }
    }();

    let mut renderer = SsrRenderer::new();
    renderer.render_page_with_view_head(view)
}
```

## Benefits of the New System

1. **Type Safety**: Head elements are strongly typed
2. **JSX-like Syntax**: Familiar declarative syntax
3. **Component-Level**: Each component can define its own head
4. **Hierarchical**: Nested heads are merged with precedence rules
5. **Testable**: Head elements can be unit tested independently
6. **Future-Proof**: Designed for streaming SSR and partial hydration

## Suppressing Deprecation Warnings

During migration, you can suppress deprecation warnings with:

```rust
#[allow(deprecated)]
let options = SsrOptions::new()
    .title("My Page");
```

However, we recommend completing the migration before the 0.3.0 release.

## Need Help?

If you encounter issues during migration, please:

1. Check the [reinhardt-pages documentation](https://docs.rs/reinhardt-pages)
2. Open an issue on [GitHub](https://github.com/kent8192/reinhardt-web/issues)
