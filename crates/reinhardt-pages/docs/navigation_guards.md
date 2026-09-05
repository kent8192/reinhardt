# Asynchronous navigation guards

Navigation guards decide whether a matched Pages route may be prepared and
rendered. They are navigation and user-experience controls, not endpoint
authorization. Every server endpoint must still authenticate the request and
enforce its authorization and object-level permissions independently.

## Naming and existing guard APIs

Reinhardt has several intentionally different APIs whose names contain
"guard":

| API | Lifecycle | Behavior |
| --- | --- | --- |
| `reinhardt_auth::Guard<P>` | HTTP dependency injection | Evaluates a `Permission` while resolving an endpoint parameter and rejects an unauthorized request with HTTP 403. |
| `reinhardt_auth::guard!(...)` | HTTP type generation | Generates a `Guard<...>` permission type expression; it does not execute a route check. |
| `ClientRoute::with_guard(...)` | Synchronous route matching | Runs a boolean predicate while matching. `false` makes the route unmatched. |
| `reinhardt_pages::router::guard()` / `guard_or()` | Synchronous rendering | Includes or replaces rendered content according to a boolean condition. |
| `#[navigation_guard]` | Async navigation | Decides whether a route tree may load, commit, hydrate, or prefetch. |

The async API is named **navigation guard** because its decision belongs to a
navigation attempt. It does not adapt HTTP `Guard<P>` or `guard!(...)`, and it
does not replace any of the existing APIs.

## Declare and attach a guard

The `#[navigation_guard]` attribute preserves the async function and registers
it for route execution. Its function shape is fixed: one
`NavigationContext` argument and a
`Result<NavigationDecision, NavigationGuardError>` result.

```rust,ignore
use reinhardt_pages::{
	NavigationContext, NavigationDecision, NavigationGuardError, QueryOptions, Outlet, Page,
	layout, navigation_guard,
};

#[navigation_guard]
async fn require_authenticated(
	context: NavigationContext,
) -> Result<NavigationDecision, NavigationGuardError> {
	let session = context
		.query(current_session::query(), QueryOptions::new())
		.await?;
	if session.is_authenticated {
		Ok(NavigationDecision::Allow)
	} else {
		let login_path = users_routes::reverse("login", &[]);
		Ok(NavigationDecision::Redirect {
			location: format!(
				"{login_path}?next={}",
				encode_return_location(context.destination()),
			),
			replace: true,
		})
	}
}

#[layout(
	"/dashboard/",
	name = "dashboard",
	navigation_guard = require_authenticated,
)]
fn dashboard_layout(outlet: Outlet) -> Page {
	render_dashboard(outlet)
}
```

`current_session::query`, `users_routes::reverse`, `encode_return_location`,
and `render_dashboard` in this example are application functions. Reverse the
named login route instead of hardcoding its path so the redirect tracks the
registered URL. A guard may be attached to a leaf `#[component]` or to a
`#[layout]`; a layout guard applies to every matched descendant. Each
route-tree node accepts at most one navigation guard. Compose additional
checks in ordinary Rust and return the first non-`Allow` result immediately:

```rust,ignore
#[navigation_guard]
async fn require_project_access(
	context: NavigationContext,
) -> Result<NavigationDecision, NavigationGuardError> {
	let session = context
		.query(current_session::query(), QueryOptions::new())
		.await?;
	if !session.is_authenticated {
		return Ok(NavigationDecision::Redirect {
			location: users_routes::reverse("login", &[]),
			replace: true,
		});
	}
	if !session.can_read_projects {
		return Ok(NavigationDecision::Forbidden);
	}
	Ok(NavigationDecision::Allow)
}
```

## Context and decisions

`NavigationContext` is a cloneable, read-only view of one attempt. It provides:

- `destination()`: the complete attempted path, including its query string;
- `route_context()`: merged path parameters and the raw query for the match;
- `navigation_type()`: `Initial`, `Push`, `Replace`, `Pop`, or `Prefetch`;
- `cancellation_token()`: cancellation for superseded navigation work; and
- `query(...)`: an awaitable read through this attempt's existing
  `QueryClient`.

The result expresses expected control flow:

- `Allow` continues preparation;
- `Redirect { location, replace }` starts another navigation without
  committing the denied destination;
- `NotFound` selects the normal unmatched-route surface; and
- `Forbidden` selects the navigation-forbidden surface.

Unexpected failures use `NavigationGuardError`.
Its public message and optional status are safe to expose; a retained
diagnostic cause is not serialized to browser state or SSR output.

Guards can run more than once for one logical navigation: before loaders and
again immediately before commit. They can also run during hydration,
prefetch, and active-branch revalidation. Guard functions must therefore be
idempotent and read-only. Do not mutate application state, create one-time
side effects, or assume that one invocation means one user action.

## Execution lifecycle

For a matched destination, Pages uses the following order:

1. Structural matching runs, including the existing synchronous
   `ClientRoute::with_guard` predicates.
2. Async navigation guards run sequentially from the root layout through the
   deepest layout and then the leaf.
3. The first redirect, `NotFound`, `Forbidden`, or error stops the chain.
4. Only after every guard returns `Allow` do all matched route loaders start;
   loaders retain their existing concurrent preparation behavior.
5. The same guard chain runs again immediately before commit.
6. The destination commits only if the navigation is still current and every
   guard still returns `Allow`.

This ordering keeps parent access checks ahead of child checks, outlets, route
queries, and loaders. A superseding navigation cancels the old attempt and
late results cannot commit it. Prepared loader leases are released when the
attempt is rejected or cancelled.

## Query reuse, hydration, and prefetch

`NavigationContext::query` acquires the descriptor through the existing
`QueryClient`. It follows normal freshness, garbage-collection, and in-flight
deduplication rules. Navigation and prefetch consumers do not automatically
retry failed fetches, even when a retry policy is configured; a failed guard
query is returned as a `NavigationGuardError`. Because a guard must reach a
decision, `QueryOptions::enabled(false)` also returns an immediate safe error
instead of waiting for a fetch that cannot start. The pre-loader and pre-commit
checks therefore reuse one fresh session query, and sibling navigation can
reuse the same settled entry. There is no separate navigation-guard result
cache, timestamp, or dependency tracker.

On SSR, guard queries are serialized through the normal query state. Browser
hydration reads that existing state and reruns the pure guard decision; it
does not hydrate a guard outcome or create a guard-specific payload. A guarded
initial branch mounts only after its guard allows, so denied routes do not
flash protected layouts, outlets, or leaf content. An initial client render
without SSR state follows the same guard-then-loader-then-mount sequence.

Prefetch uses the same root-to-leaf chain before dispatching route loaders. An
`Allow` may warm the query and loader caches. Any other decision or an error
silently stops prefetch: it does not change browser history, the visible
route, route signals, or navigation-pending state. A later click performs a
new guard evaluation.

## Browser navigation and redirects

Push, replace, links, and `popstate` all use the same decision contract. The
current committed route remains mounted while an async destination guard and
its loaders prepare. A denied destination never owns the outlet or its
prepared loader store. A denied history traversal restores the committed
entry when the existing failed-navigation path requires restoration.

For a `Redirect`, the guard owns the destination and any return-location
encoding. `destination()` already includes the query; Pages does not choose a
`next` parameter name, encode the value, or store a global return location.
`replace: true` replaces the denied history entry, which is usually correct
for login and session invalidation. `replace: false` requests ordinary push
semantics.

Redirects derived from one guard chain carry a normalized visited-destination
set. A redirect to the same normalized destination, or to a destination
already visited in that chain, returns a safe status-500
`NavigationGuardError` instead of mutating history again. A fresh user
navigation starts a new chain.

## SSR and response status

SSR runs the same registry and ordered guard executor before route loaders or
protected HTML are generated:

| Decision | Result |
| --- | --- |
| `Allow` | Load and render the route with status 200. |
| `Redirect` | Render no protected route HTML, return status 302, and expose `Location` through `SsrRenderer::route_redirect_location()`. |
| `NotFound` | Render the configured unmatched surface with status 404. |
| `Forbidden` | Render the safe navigation-forbidden surface with status 403. |
| `Err` | Render the safe public error message with its status or status 500. |

`SsrRouteOutput` remains `{ html, status }` for compatibility. An HTTP
adapter can copy redirect metadata to its response headers:

```rust,ignore
let output = renderer
	.render_route_to_string(&router, "/dashboard/")
	.await;
if output.status == 302 {
	let location = renderer
		.route_redirect_location()
		.expect("a redirect decision supplies Location");
	response.headers_mut().insert("Location", location);
}
```

The redirect location is reset at the beginning of each route render and is
`Some` only for the most recent redirect decision. Its value is the normalized
same-origin path and query used by browser navigation; relative destinations
are rooted at `/`, and URL fragments are omitted.

## Authentication boundaries

Call `auth::invalidate_authentication()`
after logout, account switching, or another local session boundary. It
clears the current query and normalized-entity state, including in-flight and
hydrated protected data, cancels current navigation preparation, and asks an
installed launcher coordinator to replace-revalidate the active branch. The
launcher unmounts the active route before revalidation so a rejected guard or
loader failure cannot leave content from the previous authentication state in
the DOM.
If a logout or account-switch handler starts another navigation after
invalidation and before the deferred replacement runs, that newer attempt is
preserved and the replacement of the previous branch is skipped.
Repeated invalidations for the same authentication identity are coalesced.
`AuthState::login`, `AuthState::login_full`, `AuthState::update`, and a
state-changing `AuthState::logout` advance that generation, so a newer account
or session boundary always clears caches again and supersedes an in-flight
replacement from the previous generation. Replacing or removing a JWT advances
the JWT identity generation and provides the same guarantee for token-only
sessions. The revalidation is deferred until
the triggering guard or request settles, so a guard-originated 401 cannot
recursively start nested navigation. Coalescing remains active until the
replacement route attempt settles, so another managed 401 from that same
generation neither cancels it nor schedules a replacement loop.

Managed server-function clients invoke the same invalidation path after HTTP
401 only when the request started with an established hydrated auth state or a
JWT bearer token. An expected 401 from an anonymous endpoint such as login is
returned to its form without clearing or remounting the active route. A stale
401 from an older authentication generation or JWT identity cannot log out a
newer session or clear its replacement token.
HTTP 403 does not invalidate authentication: it normally means the user is
authenticated but lacks permission. No session-expiry timer or polling loop is
installed; silent expiry is observed on the next guarded navigation, explicit
invalidation, or authenticated managed request that returns 401.

For route-loader details and the shared prepare/commit cache model, see
[Route-level data loaders](route_loaders.md).
