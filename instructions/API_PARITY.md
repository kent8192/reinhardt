# WASM/Native API Parity Contract

Public APIs that can be named by normal application code in both native and
`wasm32-unknown-unknown` builds must declare one parity level.

## Parity Levels

| Level | Name | Contract | Call-site result |
|---|---|---|---|
| P2 | Behavioral parity | The symbol exists and performs meaningful target-specific behavior on both targets. | cfg-free use on both targets |
| P1 | Symbol parity | The symbol exists on both targets, and one target is inert by design. | cfg-free naming and type-checking |
| P0 | Target-only behavior | The behavior is intentionally absent from the other target. | misuse fails at compile time |

The level applies per symbol. A type can be P1 while selected methods are P0.

## P2 API Surface

| API | Native behavior | WASM behavior |
|---|---|---|
| `page!` controlled `bind:` directive and `control_binding` support types | Renders signal state during SSR and synchronizes values in native component tests. | Adopts the live control property during hydration, then synchronizes user and signal writes. |

Both targets apply browser text-fallback sanitization for unknown input types
and ignore reactive type or select-cardinality changes that would invalidate an
existing binding.

## P1 API Surface

| API | Native behavior | WASM behavior |
|---|---|---|
| `UnifiedRouter::server` | Invokes the closure and stores native server routes. | Type-checks and drops the closure without invoking it. |
| `UnifiedRouter::client` | Type-checks and drops the closure without invoking it. | Invokes the closure and stores client routes. |

Inactive `UnifiedRouter` closures must not contain required side effects. Each
active target must have an executable test proving its route configuration.

## Stub Taxonomy

- Mirror: the same user-facing operation is meaningful on both targets.
- No-op passthrough: the inert side accepts the same call shape and returns the builder unchanged.
- Marker stub: a zero-sized symbol exists for type checking and registration syntax.
- Inert data: data and metadata compile on both targets while runtime behavior stays native-only.
- Native page stub: a client page function returns `Page::empty()` on native for route-table construction.

## Accepted Application Paths

The strict cfg-free acceptance scope is:

- `examples/**/src/apps/**`
- `examples/**/src/config/**`
- `examples/**/src/shared/**`

The scan excludes `#[cfg(test)]`, `build.rs`, binary entrypoints, and framework internals.

## Review Checklist

When adding a public API that can cross the client/server boundary:

1. Declare P0, P1, or P2 in rustdoc.
2. For P1, document the inert side and prove it has no network, database, filesystem, or registration side effect.
3. For P0, keep the absent target unnameable so misuse fails at compile time.
4. For WASM-visible symbols, verify that native-only dependencies do not enter the WASM graph.
5. Add at least one compile test or example check for the parity surface.
