# Reinhardt Framework Crate Security Policy

This policy supplements the repository [Security Policy](../SECURITY.md) for
all framework crates. Security policies compose from the repository root to
each nested component; the closest policy wins on conflict.

- Public framework APIs may receive Internet-originated data.
- Generated code is part of the production boundary.
- Feature-gated documented production code remains in scope.
- Security checks fail closed.
- Bounded remote input causing panic, stack exhaustion, or disproportionate
  resource consumption is reportable.
- Raw SQL, raw HTML, arbitrary code, and equivalent APIs expose explicit trust
  boundaries; safe APIs must not enter them accidentally.
- URL helpers that concatenate caller-supplied static asset names do not
  validate dot segments or encode path components automatically. Applications
  must validate asset names before passing them to a static URL helper.
