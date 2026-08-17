# Reactive Hooks

Reactive hooks create state, subscriptions, derived values, and lifecycle
behavior inside a Pages component. Import common hooks from
`reinhardt::pages::prelude::*`; specialized hooks and handle types are also
available under `reinhardt::pages::reactive` and
`reinhardt::pages::reactive::hooks`. Call hooks while the component's reactive
scope is active.

## Dependency rules

Dependency-aware hooks take a typed dependency list:

```rust,ignore
use reinhardt::pages::prelude::*;

let (count, _set_count) = use_state(0);
let doubled = use_memo(move || count.get() * 2, deps![count]);
let _effect = use_effect(
    move || {
        log!("count={}", count.get());
    },
    deps![count],
);
```

- Use `deps![value, ...]` for explicit dependencies and `deps![]` for
  mount-only behavior.
- `use_effect`, `use_layout_effect`, and `use_memo` also accept
  `deps_auto!()`, which tracks signals read by the closure.
- Retained effects, callbacks, resources, queries, and actions use explicit
  dependencies or their own descriptor/options; do not pass a tuple or `()`.
- Capture reactive handles (`Signal`, `Memo`, `Resource`, and `Action`) rather
  than a one-time snapshot when a closure must observe later changes.

## State hooks

- `use_state(initial)` returns `(Signal<T>, SetState<T>)`. Read with
  `signal.get()`, replace with `set(value)`, or derive the next value with
  `set.update(|current| ...)`. It is the normal choice for client UI state.
- `use_shared_state(initial)` uses a thread-safe shared signal for state that
  crosses native/server event boundaries.
- `use_reducer(reducer, initial)` keeps complex transitions in a pure reducer
  and returns `(Signal<State>, Dispatch<Action>)`.

Keep state local to the app that owns the feature. Use an explicit serializable
DTO or context contract when another app needs to consume the result.

## Effects and derived values

- `use_effect` performs ordinary side effects such as logging, subscriptions,
  and non-blocking data work. Its closure may return `()` or `Some(cleanup)`;
  cleanup runs before a rerun and when the scope is disposed.
- `use_layout_effect` is for DOM measurement or synchronous visual work before
  paint. Prefer `use_effect` unless layout timing is required.
- `use_retained_effect` and `use_retained_layout_effect` register the effect in
  the mounted view scope. Use these when a local guard should not be dropped
  immediately after registration.
- `use_memo` caches an expensive derived value and returns `Memo<T>`.
- `use_callback` and `use_callback_with` provide stable callback handles for
  event props or child components. They take explicit `deps![...]`.

Effects and retained effects are RAII-managed. Keep a returned `Effect` alive,
or use a retained variant whose owner is the mounted view scope; never invent a
manual cleanup path.

## Async data and mutations

- `use_resource(fetcher, deps![...])` runs an async read and returns a
  `Resource<T, E>`. Render its `ResourceState` in `page!` and use
  `use_resource_with_key` when a conditional hook needs a stable SSR hydration
  key.
- `use_latest_resource_value(resource)` composes a resource with successful
  action results so the UI can keep showing the latest value during a refresh.
- `use_query(descriptor, options)` subscribes to an app-wide keyed query cache.
  Use a `QueryFamily`/`QueryDescriptor` for stable keys, then read the returned
  `QueryHandle` snapshot and call `refetch()` when an explicit refresh is
  needed.
- `use_action(async_fn)` models an async mutation and exposes pending,
  success, and error state. `use_action_state` adds a builder for lifecycle
  callbacks and UI state.
- `use_optimistic(initial)` displays a predicted value while a mutation is in
  flight; call `confirm(value)` on success and `revert()` on failure.

Use `Resource`/`Query` for reads and `Action` for writes. Keep server function
calls and their serializable request/response types in the owning app.

## Context, refs, and external stores

- `use_context(&context)` reads the nearest value provided by
  `provide_context`; it returns `Option<T>` when no provider exists.
- `use_ref(initial)` stores mutable, non-reactive data. Updating a `Ref` does
  not rerender the page; use a `Signal` when the UI must react.
- `use_sync_external_store(subscribe, get_snapshot)` bridges browser APIs or
  another state store into a `SignalWithSubscription`. The returned handle
  automatically unsubscribes when dropped.
- `use_id()` creates stable SSR/client-safe IDs. Import
  `use_id_with_prefix` from `reinhardt::pages::reactive::hooks::id` when a
  stable prefix is required.
- `use_debug_value()` labels a value for diagnostics without changing UI state.

## Navigation, head, and scheduling

- `use_router()` returns a `RouterHandle` for `push`, `replace`, or
  `navigate`; handle the returned `NavigateError` instead of silently falling
  back to a hard navigation.
- `use_head` and `use_page_title` register reactive document-head values and
  remove them with the mounted scope. Pass explicit dependencies.
- `use_transition()` marks non-urgent updates and exposes `is_pending`.
- `use_deferred_value(signal)` keeps input responsive while derived content
  catches up.
- `use_websocket()` manages a browser WebSocket connection and its connection,
  message, and error state; keep browser-only WebSocket work in client modules.

`use_form` and `use_form_action` are the typed form hooks. Use them with
`form!` for validation and submission rather than rebuilding form state from
separate signals.

## Hooks with `page!`

Call hooks before constructing the view, and read their handles inside
`page!` so reactive nodes subscribe to the right scope:

```rust,ignore
fn counter() -> Page {
    let (count, set_count) = use_state(0);
    let doubled = use_memo(move || count.get() * 2, deps![count]);
    let _effect = use_effect(
        move || {
            log!("count={}", count.get());
        },
        deps![count],
    );

    page!({
        p { { format!("Count: {} (x2={})", count.get(), doubled.get()) } }
        button {
            @click: move |_| set_count.update(|value| value + 1),
            "Increment"
        }
    })
}
```

Do not put hooks behind `if`, `match`, or target `cfg` branches that change the
call order between renders. Keep client-only modules behind `#[cfg(client)]`,
but let target-neutral state, effects, and `page!` event handlers share one
implementation across native rendering and WASM.
