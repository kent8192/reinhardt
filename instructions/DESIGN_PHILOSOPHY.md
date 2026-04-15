# Reinhardt Design Philosophy

These principles guide ALL design decisions in the Reinhardt framework.

---

1. **Magic must be understandable** — Every convention must be a trick the user can see through; no opaque magic
2. **CoC must be predictable without domain knowledge** — Convention over Configuration must be guessable regardless of the user's background knowledge
   - Anti-pattern: `Person` → `People` (requires English pluralization knowledge)
   - Good pattern: `Person` → `PersonSettings` (trivially predictable)
3. **CoC is a right, not an obligation** — Users must always have the choice to opt out of conventions
4. **Fail early** — Earlier error detection is always better:
   - Type consistency errors > compile-time errors > runtime initialization errors > runtime errors
5. **API ergonomics is paramount** — Delegate explicit boilerplate to macros that provide compile-time verification
6. **Async over sync** — Prefer async APIs by default
7. **Confusable APIs are bad APIs** — If two APIs can be confused, the design is wrong
8. **Boilerplate is evil** — Minimize repetitive code at every opportunity
9. **Every framework eventually becomes outdated** — Design with this inevitability in mind
10. **Own the implementation for framework optimization** — Build custom implementations over adopting external libraries when it serves the framework's goals (e.g., SeaQuery → reinhardt-query)
