# Reinhardt Pages Guidance

## Project shape

Keep the generated entry points stable unless the routing model intentionally
changes:

- `src/bin/manage.rs` is the native management CLI.
- `src/lib.rs` contains shared exports and macro support.
- `src/client/lib.rs` launches the WASM application.
- `src/config/urls.rs` aggregates project routes.
- `src/config/apps.rs` owns the installed-app registry.
- `src/config/settings.rs` composes typed settings.

## Adding apps

Use the Pages scaffold so every registration point is updated together:

```bash
cargo run --bin manage startapp notes --with-pages
```

Verify that the command updates `src/apps.rs`, `src/config/apps.rs`,
`src/config/urls.rs`, and the app's `urls.rs`, `server_fn.rs`, and client
modules. App-level routes belong to the app; project configuration should only
mount the app aggregate.

## Route aggregation

The project router should call app-level route functions rather than importing
individual server functions:

```rust,ignore
#[routes]
pub fn routes() -> UnifiedRouter {
    let router = UnifiedRouter::new();

    #[cfg(server)]
    let router = router.server(|server| {
        server.mount("/", crate::apps::notes::urls::server_url_patterns())
    });

    #[cfg(client)]
    let router = router.mount_unified(
        "/",
        UnifiedRouter::new().client(|_| crate::apps::notes::urls::client_url_patterns()),
    );

    router
}
```

## WASM launcher

Keep the launcher inventory-driven unless the project deliberately adopts a
different client router:

```rust,ignore
ClientLauncher::new("#root")
    .register_routes_from_inventory()
    .launch()
```

## Target boundaries

- Use the generated `client`/`wasm` aliases for browser code and
  `server`/`native` aliases for native code.
- Keep serializable DTOs and target-neutral declarations outside target-only
  modules.
- Keep Tokio, database migrations, filesystem access, and management commands
  out of WASM builds.
- Event handlers inside `page!` normally do not need duplicate native branches.
- Verify both native and browser targets after changing a shared boundary.

## Settings and artifacts

Settings load from `settings/base.toml`, the selected profile, and
`REINHARDT_` environment overrides. Update the relevant `settings/*.example.toml`
files when the settings shape changes, and never commit secrets or personal
local values. Treat `dist/` and `dist-wasm/` as generated artifacts; do not
hand-edit their contents.

## Verification

Use the Pages formatter for DSL changes, then run native and browser checks:

```bash
cargo make fmt-check
cargo make quality
cargo make wasm-build-dev
cargo make wasm-test
cargo run --bin manage check
cargo run --bin manage showurls
```
