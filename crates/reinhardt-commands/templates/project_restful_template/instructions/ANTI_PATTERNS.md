# Anti-Patterns

- Do not add `mod.rs` or obsolete wrapper modules.
- Do not flatten app handlers, serializers, models, or routes into `src/config/`.
- Do not use raw route registration when endpoint macros provide the metadata and validation.
- Do not use unnecessary `.to_string()` or cloning when borrowing works.
- Do not leave obsolete implementations in comments or add undocumented `#[allow(...)]` attributes.
- Do not use manual cleanup paths that can be skipped by `?`, early return, or panic; use RAII guards.
- Do not add tests that only compile, always pass, or leave files, database state, or tasks behind.
- Do not save temporary or backup files in the project tree.
