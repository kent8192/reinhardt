# Anti-Patterns

- Do not add `mod.rs`, old `pages.rs`, or obsolete wrapper modules.
- Do not mix native-only dependencies into browser modules.
- Do not flatten app handlers or routes into `src/config/`.
- Do not use unnecessary `.to_string()` or cloning when borrowing works.
- Do not leave obsolete implementations in comments or add undocumented `#[allow(...)]` attributes.
- Do not use manual cleanup paths that can be skipped by `?`, early return, or panic; use RAII guards.
- Do not hand-edit `dist/` or `dist-wasm/`.
- Do not add tests that only compile, always pass, or leave files, database state, or tasks behind.
- Do not save temporary or backup files in the project tree.
