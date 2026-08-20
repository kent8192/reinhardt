# Reinhardt Framework Crate Security Policy

This policy supplements the repository [Security Policy](../SECURITY.md) for
all framework crates.

- Public framework APIs may receive Internet-originated data.
- Generated code is part of the production boundary.
- Feature-gated documented production code remains in scope.
- Security checks fail closed.
- Bounded remote input causing panic, stack exhaustion, or disproportionate
  resource consumption is reportable.
- Raw SQL, raw HTML, arbitrary code, and equivalent APIs expose explicit trust
  boundaries; safe APIs must not enter them accidentally.
