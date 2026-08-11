# reinhardt-testkit-macros

<!-- reinhardt-version-sync: reinhardt-testkit-macros -->

Procedural macros for [`reinhardt-testkit`](https://docs.rs/reinhardt-testkit/). The macros here are re-exported from `reinhardt-testkit` — depend on `reinhardt-testkit` rather than this crate directly.

## `with_di_overrides!`

Expands into a call to `reinhardt-testkit`'s `injection_context_with_di_overrides`, which returns the context plus a guard token that reverts every override on drop.

```rust,ignore
use reinhardt_testkit::with_di_overrides;
use rstest::*;

#[rstest]
#[tokio::test]
async fn test_login_flow() {
    let (ctx, _di) = with_di_overrides! {
        singleton MockDatabase { url: "test://db".into() },
        singleton MockConfig { api_key: "test_key".into() },
        transient MockHttpClient => |_ctx| async {
            Ok(MockHttpClient::new())
        },
    };

    // ... assertions ...
}
```

See `reinhardt-testkit::fixtures::di_overrides` for the underlying API and the design rationale.

## Testing

Inline unit tests exercise the parser and compare the complete generated token
structure for `with_di_overrides!`. Run them with:

```bash
cargo nextest run -p reinhardt-testkit-macros --lib
```

The trybuild suite compiles consumer fixtures and pins both successful usage
and exact compiler diagnostics:

```bash
cargo nextest run -p reinhardt-testkit-macros --test trybuild
```

## Versioning

This crate's version is managed by release-plz independently of its sibling
crates: a new patch is published only when a conventional-commit change
(`feat`, `fix`, `refactor`, etc.) actually touches files inside this crate
directory. As a result, the patch number may lag behind the rest of the
Reinhardt workspace whenever no behavior-affecting changes have landed
here since the last tag. This is by design and not a release accident.

For users, the practical guidance is to depend on
[`reinhardt-testkit`](https://docs.rs/reinhardt-testkit/) (which re-exports
these macros) rather than pinning this crate directly — Cargo then picks a
compatible `reinhardt-testkit-macros` automatically.
