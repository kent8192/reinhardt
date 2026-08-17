# Testing Standards

- Every test must assert meaningful behavior and use at least one Reinhardt component.
- Keep Arrange, Act, and Assert phases clear for non-trivial tests.
- Use strict assertions such as `assert_eq!` when the exact value is the contract.
- Put unit tests near the component and cross-module integration tests in `tests/`.
- Test endpoint contracts, status codes, serialization, and route registration rather than only compilation.
- Clean up files, database state, global state, and spawned tasks created by tests.
- Isolate tests that change process environment or other global state.

Focused checks should run before broader checks:

```bash
cargo test <test_name>
cargo make test-unit
cargo make test-integration
```
