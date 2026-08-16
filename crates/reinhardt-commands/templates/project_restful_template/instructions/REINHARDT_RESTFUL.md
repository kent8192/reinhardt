# Reinhardt RESTful Guidance

## Registration

- `src/apps.rs` declares application modules.
- `src/config/apps.rs` owns the `installed_apps!` registry.
- `src/config/urls.rs` composes app route aggregates and framework routes.
- Each app owns its handlers, serializers, models, services, and route table.
- Project configuration should mount app aggregates instead of importing individual endpoint functions.

## API Boundaries

- Keep request and response DTOs serializable and versionable.
- Use endpoint macros for method metadata and validation.
- Keep database and filesystem work in native code paths.
- Keep API errors explicit and preserve the framework's response contract.

## Settings

- Compose typed settings from `settings/base.toml`, the selected profile, and `REINHARDT_` environment overrides.
- Update example TOML files when the settings shape changes.
- Do not commit secrets or personal local settings.

## Verification

Run formatting, quality, application checks, route inspection, and tests:

```bash
cargo make fmt-check
cargo make quality
cargo run --bin manage check
cargo run --bin manage showurls
cargo make test
```
