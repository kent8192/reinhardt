# Reinhardt Pages Guidance

## Registration

- `src/apps.rs` declares application modules.
- `src/config/apps.rs` owns the `installed_apps!` registry.
- `src/config/urls.rs` composes app route aggregates and framework routes.
- Each app owns its server and client route tables in its `urls` module.
- The WASM launcher should keep inventory-based route registration unless the routing model intentionally changes.

## Target Boundaries

- Use the generated `client`/`wasm` aliases for browser code and `server`/`native` aliases for native code.
- Keep serializable DTOs and target-neutral declarations outside target-only modules.
- Keep Tokio, database migrations, filesystem access, and management commands out of WASM builds.
- Keep `dist/` and `dist-wasm/` as generated artifacts.

## Settings

- Compose typed settings from `settings/base.toml`, the selected profile, and `REINHARDT_` environment overrides.
- Update example TOML files when the settings shape changes.
- Do not commit secrets or personal local settings.

## Verification

Use the Pages formatter for DSL changes, then run native and browser checks:

```bash
cargo make fmt-check
cargo make wasm-build-dev
cargo make wasm-test
```
