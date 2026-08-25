# Route-level data loaders

Route loaders make entry data an explicit part of a routed page. A loader is
prepared after URL matching and before the destination route is committed, so
the old route remains mounted while the next route is loading.

## Declare and bind a loader

`#[loader]` keeps the original async function callable for ordinary Rust unit
tests and generates a stable marker for routing:

```rust,ignore
use reinhardt_pages::{Loader, Path, Page, component, loader, page};

#[loader]
async fn project_loader(Path(project_id): Path<i64>) -> Result<Project, String> {
    fetch_project(project_id).await
}

#[component(
    "/projects/{project_id}/",
    name = "project-detail",
    loader = project_loader
)]
fn project_detail(Loader(project): Loader<Project>) -> Page {
    page!(|project: Project| { h1 { { project.name } } })(project)
}
```

The `Loader<T>` type must match the loader's `Result<T, E>` data type. A loader
may also accept `Query<T>` and one `CancellationToken` extractor. Cache keys
preserve the raw route-matcher value for `Path<T>` inputs, while `Query<T>`
inputs use their decoded query value. This keeps cache identity aligned with
the extractor contract, including percent-encoded path segments. Layouts use
the same `loader = ...` option and receive their value before the leaf route is
rendered. A persistent layout is remounted when one of its declared loader
inputs changes, even if its route path parameters are unchanged.

An optional query extractor uses an explicit outer `Option<T>`:

```rust,ignore
#[loader]
async fn deployment_loader(
    Query(logs): Query<Option<i64>>,
) -> Result<DeploymentData, String> {
    load_deployments(logs).await
}
```

For optional loader queries, missing, empty, and ordinary present values have
different cache keys:

```json
{"path":[],"query":[["logs",null]]}
{"path":[],"query":[["logs",""]]}
{"path":[],"query":[["logs","42"]]}
```

These shapes identify distinct loader cache entries across SSR, hydration,
prefetch, and navigation.

## Navigation and cancellation

`ClientLauncher` installs a pages-owned `NavigationCoordinator`. It performs
matching and guards synchronously, then prepares all matched layout and leaf
loaders concurrently. Only the latest navigation generation can commit. A
superseded attempt drops its loader futures and releases its query leases;
Reinhardt-managed browser requests also receive an abort signal. Work started
by application code outside the loader future is intentionally not cancelled.
The matched leaf and layout guards are evaluated again immediately before an
asynchronously prepared route commits, so a session or authorization change
during loading cannot commit a route that is no longer allowed.

`use_transition().is_pending` includes coordinator navigation pending state in
addition to local transition work and becomes `true` synchronously when a
transition starts. A failed loader leaves the current route and reactive scopes
mounted. Configure a sibling fallback on `RouterOutlet` when a route-specific
error view is needed:

```rust,ignore
RouterOutlet::new(router).navigation_error_fallback(|error| {
    page!(|message: String| { p { { message } } })(error.public_message().to_owned())
})
```

When that fallback reacts after a failed later navigation, `RouterOutlet`
re-enters the currently mounted loader store before rebuilding the retained
route, so loader bindings remain available while the fallback is displayed.

Loader errors expose only their safe public message and optional status. The
internal diagnostic cause never enters the browser state payload. Failed pop
navigations restore the committed history entry after preparation fails.
On first launch, legacy or host-created history state without a framework entry
index is upgraded in place to index `0`, retaining compatible custom state and
scroll metadata while taking the current URL, route parameters, and query from
the browser; an existing framework index is retained across reloads so
back/forward restoration remains monotonic. A loader route opened directly
without an SSR state script prepares its initial values on the client before
the first route mount. If that initial preparation fails, the root renders a
safe loader-error surface instead of remaining empty. Browser history
preparation preserves the search query for both initial loads and back/forward
navigations.
DOM-dependent lifecycle callbacks (`after_launch`, `on_path`, and
`on_path_pattern`) wait for that prepared route to commit and mount. Path
subscriptions match the pathname, while their `PathCtx::path()` value retains
the full route location including its query string.

## Prefetch and query sharing

Links can prepare the same cache entry before a click:

```rust,ignore
use reinhardt_pages::{Link, PrefetchMode};

Link::new("/projects/42/", "Project 42")
    .prefetch(PrefetchMode::Hover);
```

`Hover` covers pointer intent and keyboard focus. `Viewport` uses an
`IntersectionObserver`, which is created only after a mounted viewport link
needs it; browsers without that API can still launch and use other prefetch
modes. Prefetch does not change history, route signals, or navigation pending
state, and a settled prefetch error is silent. Navigation, prefetch, mounted
routes, and `use_query` all use the single keyed query cache; an in-flight
request is shared while at least one RAII lease remains. When a refetch is
queued behind an in-flight request, existing leases can still observe the
settled generation they acquired; dropping the final lease cancels both the
active request and its queued follow-up.

## SSR and hydration

`SsrRenderer::render_route_to_string` matches the route, prepares every
matched loader concurrently, installs a request-local `LoaderStore`, and then
renders the loaded layout/leaf tree:

```rust,ignore
let mut renderer = SsrRenderer::new();
let output = renderer
    .render_route_to_string(&router, "/projects/42/")
    .await;
assert_eq!(output.status, 200);
```

The first loader error cancels the shared preparation source and returns its
safe error response without waiting for slower sibling loaders.
An unmatched request uses the router's configured `not_found` page while still
returning status `404`, matching client-side navigation behavior.

Successful values are registered in `SsrState` before the page is serialized,
so the emitted HTML contains both a stable `route-loader:<id>` entry and the
opaque query-cache key. Hydration reconstructs the initial typed loader store
from the stable entry and restores the query success state by key, so the
first client render does not issue a duplicate request. Cancellations, partial
results, and internal error causes are never serialized.

## Testing loaders directly

The original function remains an ordinary async function, so direct tests do
not need a router or browser runtime:

```rust,ignore
let project = project_loader(Path(42)).await?;
```

Use a matched `ClientRouteTreeMatch` and `loader_cache_id` when testing input
canonicalization or cache deduplication. Native SSR tests can assert both the
rendered output and `SsrState::get_route_loader_state`.
