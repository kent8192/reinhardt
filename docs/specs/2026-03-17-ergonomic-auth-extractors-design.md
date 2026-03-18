# Ergonomic Auth Extractors Design Spec

## Overview

Make authentication info extraction ergonomic by integrating auth-related types
into the existing `#[inject]` dependency injection system. Introduce `AuthUser<U>`
and `AuthInfo` as new `Injectable` types with destructuring support, deprecate
the existing `CurrentUser<U>`, and simplify DI configuration validation.

## Problem Statement

Using `CurrentUser` requires multi-step manual setup (DI context construction,
`DatabaseConnection` registration, `.with_di_context()`, auth middleware
configuration) with no compile-time or startup-time validation. Missing any step
causes silent fallback to `anonymous()`, which is hard to debug.

Additionally, the current `CurrentUser::inject()` implementation falls back to
`Uuid::nil()` on `user_id` parse failure, creating a potential user
impersonation risk (see #2430).

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Scope | Setup simplification + extractor pattern | Both reduce boilerplate and improve safety |
| Extractor strategy | Hierarchical: `AuthInfo`, `AuthUser<U>`, `Option<...>` | Match information needs to DB access cost |
| Error handling | Type-based semantics via `Injectable` + `Option<T>` | Compile-time safety, Rust idiom |
| Setup simplification | Middleware Composition: `.with_middleware(AuthExtractors)` | Explicit, doesn't break existing DI system |
| Backward compatibility | `AuthUser<U>` is new; `CurrentUser<U>` deprecated | Clean migration to 0.2.0 |
| API consistency | Destructuring pattern: `Type(val): Type<Inner>` | Matches existing `Path`, `Json`, `Query` |

## Migration Strategy

### RC Phase (current, 0.1.0-rc.N)

- Add `AuthUser<U>(pub U)` as a new tuple struct with `Injectable` impl
- Add `AuthInfo(pub AuthState)` as a new tuple struct with `Injectable` impl
- Add `Option<T>` blanket `Injectable` impl
- Add `#[inject]` auto-detection in HTTP method macros
- Add `AuthExtractors` middleware for startup validation
- Deprecate `CurrentUser<U>` with `#[deprecated]` pointing to `AuthUser<U>`
- Fix `Uuid::nil()` fallback in existing `CurrentUser::inject()` (#2430)

### 0.2.0 (`develop/0.2.0`)

- Remove the old `CurrentUser<U>` named struct
- Add `pub type CurrentUser<U> = AuthUser<U>` as a type alias
- This makes `CurrentUser` a familiar name that uses the new extractor pattern

## New Types

### `AuthUser<U>` (DB query, tuple struct)

New primary extractor for authenticated user access with full model loading.

```rust
// crates/reinhardt-auth/src/auth_user.rs

/// Authenticated user extractor that loads the full user model from database.
///
/// Wraps the user model `U` as a tuple struct for destructuring, consistent
/// with `Path<T>`, `Json<T>`, and other Reinhardt extractors.
///
/// Requires `feature = "params"` to access request data from `InjectionContext`.
///
/// # Usage
///
/// ```rust
/// #[get("/profile/")]
/// pub async fn profile(
///     #[inject] AuthUser(user): AuthUser<DefaultUser>,
/// ) -> ViewResult<Response> {
///     let username = user.username();
///     // ...
/// }
/// ```
///
/// # Failure
///
/// Returns an injection error when:
/// - No `AuthState` in request extensions (HTTP 401)
/// - `user_id` parse failure (HTTP 401, not nil UUID fallback)
/// - `DatabaseConnection` not registered in DI (HTTP 503)
/// - Database query failure (HTTP 500)
#[derive(Debug, Clone)]
pub struct AuthUser<U: BaseUser>(pub U);
```

### `AuthInfo` (lightweight, no DB)

Wraps `AuthState` as a tuple struct for destructuring.

```rust
// crates/reinhardt-auth/src/auth_info.rs

/// Lightweight authentication extractor that reads from request extensions.
///
/// Does NOT perform a database query. Use `AuthUser<U>` when the full
/// user model object is needed.
///
/// Requires `feature = "params"` to access request data from `InjectionContext`.
///
/// # Usage
///
/// ```rust
/// #[get("/admin/")]
/// pub async fn admin(
///     #[inject] AuthInfo(state): AuthInfo,
/// ) -> ViewResult<Response> {
///     if !state.is_admin() {
///         return Err(Error::Forbidden("Admin access required"));
///     }
///     // ...
/// }
/// ```
///
/// # Failure
///
/// Returns an injection error (maps to HTTP 401) when:
/// - No `AuthState` is present in request extensions
/// - `AuthState` indicates the user is not authenticated
#[derive(Debug, Clone)]
pub struct AuthInfo(pub AuthState);
```

### `Option<T>` blanket implementation

```rust
// crates/reinhardt-di/src/injectable.rs

#[async_trait]
impl<T: Injectable + Clone + Send + Sync + 'static> Injectable for Option<T> {
    async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
        match T::inject(ctx).await {
            Ok(value) => Ok(Some(value)),
            Err(_) => Ok(None),
        }
    }
}
```

**Security note**: `Option<AuthUser<U>>` swallows ALL injection errors
(including DB failures and malformed `user_id`) into `None`. This is intentional
for endpoints that serve both authenticated and anonymous users, but means
system errors are indistinguishable from "not authenticated". For
security-critical endpoints, use `AuthUser<U>` (not `Option<AuthUser<U>>`) to
ensure errors are surfaced as HTTP 401/500.

## `Injectable` Implementations

### `AuthInfo`

```rust
#[cfg(feature = "params")]
#[async_trait]
impl Injectable for AuthInfo {
    async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
        // Get Request from InjectionContext (requires "params" feature)
        let request = ctx.get_http_request()
            .ok_or_else(|| DiError::resolution_failed(
                "AuthInfo",
                "No HTTP request available in InjectionContext. \
                 Ensure the router is configured with .with_di_context()"
            ))?;

        // Get AuthState from request extensions
        let auth_state: AuthState = request.extensions.get::<AuthState>()
            .ok_or_else(|| DiError::resolution_failed(
                "AuthInfo",
                "No AuthState found in request extensions. \
                 Ensure authentication middleware is configured."
            ))?;

        if !auth_state.is_authenticated() {
            return Err(DiError::resolution_failed(
                "AuthInfo",
                "User is not authenticated"
            ));
        }

        Ok(AuthInfo(auth_state))
    }
}

#[cfg(not(feature = "params"))]
#[async_trait]
impl Injectable for AuthInfo {
    async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
        Err(DiError::resolution_failed(
            "AuthInfo",
            "AuthInfo requires the 'params' feature to be enabled"
        ))
    }
}
```

### `AuthUser<U>`

```rust
#[cfg(feature = "params")]
#[async_trait]
impl<U> Injectable for AuthUser<U>
where
    U: BaseUser + Model + Clone + Send + Sync + 'static,
    <U as BaseUser>::PrimaryKey: std::str::FromStr + ToString + Send + Sync,
    <<U as BaseUser>::PrimaryKey as std::str::FromStr>::Err: std::fmt::Debug,
    <U as Model>::PrimaryKey: From<<U as BaseUser>::PrimaryKey>,
{
    async fn inject(ctx: &InjectionContext) -> DiResult<Self> {
        // Get Request from InjectionContext
        let request = ctx.get_http_request()
            .ok_or_else(|| DiError::resolution_failed(
                "AuthUser",
                "No HTTP request available in InjectionContext"
            ))?;

        // Get AuthState from request extensions
        let auth_state: AuthState = request.extensions.get::<AuthState>()
            .ok_or_else(|| DiError::resolution_failed(
                "AuthUser",
                "No AuthState found in request extensions"
            ))?;

        if !auth_state.is_authenticated() {
            return Err(DiError::resolution_failed(
                "AuthUser",
                "User is not authenticated"
            ));
        }

        // Parse user_id — NO fallback to nil UUID (#2430)
        let user_pk = auth_state.user_id()
            .parse::<<U as BaseUser>::PrimaryKey>()
            .map_err(|e| {
                ::tracing::warn!(
                    user_id = %auth_state.user_id(),
                    error = ?e,
                    "failed to parse user_id from AuthState"
                );
                DiError::resolution_failed(
                    "AuthUser",
                    "Invalid user_id format in AuthState"
                )
            })?;

        let model_pk = <U as Model>::PrimaryKey::from(user_pk);

        // Resolve DatabaseConnection from DI using ctx.resolve()
        let db: Arc<DatabaseConnection> = ctx.resolve::<DatabaseConnection>()
            .await
            .map_err(|e| {
                ::tracing::warn!(
                    error = ?e,
                    "DatabaseConnection not available for AuthUser resolution"
                );
                DiError::resolution_failed(
                    "AuthUser",
                    "DatabaseConnection not registered in DI context"
                )
            })?;

        // Query user from database
        let user = U::objects()
            .get(model_pk)
            .first_with_db(&db)
            .await
            .map_err(|e| {
                ::tracing::warn!(error = ?e, "Failed to load user from database");
                DiError::resolution_failed("AuthUser", "Database query failed")
            })?
            .ok_or_else(|| {
                ::tracing::warn!(
                    user_id = %auth_state.user_id(),
                    "User not found in database"
                );
                DiError::resolution_failed("AuthUser", "User not found")
            })?;

        Ok(AuthUser(user))
    }
}
```

## Macro Changes

### Auto-detect `#[inject]` without `use_inject = true`

Currently, `#[get]`, `#[post]` etc. require `use_inject = true` to enable DI.
The presence of any `#[inject]` attribute on parameters will auto-enable
injection mode.

```rust
// Before: use_inject = true required
#[get("/profile/", use_inject = true)]
pub async fn profile(#[inject] user: AuthUser<DefaultUser>) -> ViewResult<Response> { }

// After: auto-detected from #[inject] presence
#[get("/profile/")]
pub async fn profile(#[inject] AuthUser(user): AuthUser<DefaultUser>) -> ViewResult<Response> { }

// Backward compatible: use_inject = true still works
#[get("/profile/", use_inject = true)]
pub async fn profile(#[inject] AuthUser(user): AuthUser<DefaultUser>) -> ViewResult<Response> { }
```

**Implementation details:**

In `crates/reinhardt-core/macros/src/routes.rs`, the `route_impl` function
currently rejects `#[inject]` when `use_inject = false` (line ~656). Change
this to auto-set `options.use_inject = true` when `#[inject]` attributes are
detected:

```rust
// Before (routes.rs ~656-664):
if !options.use_inject && !all_inject_params.is_empty() {
    return Err(Error::new_spanned(
        &first_inject.pat,
        "#[inject] attribute requires use_inject = true option. ...",
    ));
}

// After:
if !options.use_inject && !all_inject_params.is_empty() {
    // Auto-enable injection when #[inject] attributes are present
    options.use_inject = true;
}
```

The trybuild test `inject_without_use_inject.rs` must be updated to expect
successful compilation instead of an error, or converted to a passing test
that verifies auto-detection works.

## Security

### AuthRejection (error response type)

```rust
/// Rejection type for auth-related injection failures.
///
/// Converts `DiError` to HTTP responses without leaking authentication details.
/// `NotAuthenticated` and `InvalidCredentials` produce identical 401
/// responses to prevent user enumeration.
pub enum AuthRejection {
    NotAuthenticated,
    InvalidCredentials,
    ServiceUnavailable,
    InternalError,
}
```

`AuthRejection` implements `From<DiError>` to convert DI resolution failures
into appropriate HTTP responses. The `#[use_inject]` macro's error handling
path already converts `DiError` into `Error::Internal`, which returns HTTP 500.
`AuthRejection` refines this by mapping auth-specific failures to 401 instead.

Integration with existing `ViewResult<Response>`: `AuthRejection` implements
`Into<reinhardt_core::exception::Error>` so it can be used with the `?`
operator in handlers returning `ViewResult<Response>`.

### Critical fix: No `Uuid::nil()` fallback (#2430)

The `AuthUser::inject()` and the existing `CurrentUser::inject()` MUST return
an error on `user_id` parse failure instead of falling back to `Uuid::nil()`.
This prevents potential user impersonation if a nil-UUID user exists in the
database.

### `Option<T>` security considerations

`Option<AuthUser<U>>` converts ALL injection errors to `None`. This means:
- Not authenticated → `None` (intended)
- Malformed `user_id` → `None` (masks #2430-class issues)
- DB connection failure → `None` (masks 503 condition)

**Recommendation**: Use `AuthUser<U>` (not `Option<AuthUser<U>>`) for
security-critical endpoints where authentication failures must be surfaced.
Reserve `Option<AuthUser<U>>` for endpoints that legitimately serve both
authenticated and anonymous users (e.g., personalized homepages).

## AuthExtractors Middleware

Optional middleware for startup-time validation of DI configuration.

```rust
/// Validates DI context configuration for auth extractors at startup.
///
/// This middleware is optional. Auth extractors work without it, but
/// adding it provides early detection of configuration problems.
///
/// Implements the `Middleware` trait. On each request, it passes through
/// without modification. Its primary value is the `on_startup` validation.
///
/// # Usage
///
/// ```rust
/// let server = HttpServer::new(router)
///     .with_di_context(di_context)
///     .with_middleware(auth_middleware)
///     .with_middleware(AuthExtractors);
/// ```
pub struct AuthExtractors;

#[async_trait]
impl Middleware for AuthExtractors {
    async fn process(&self, request: Request, next: Next<'_>) -> ViewResult<Response> {
        // Pass-through — no request-time processing needed
        next.run(request).await
    }

    fn on_startup(&self, ctx: Option<&InjectionContext>) {
        if let Some(ctx) = ctx {
            if ctx.get_singleton::<DatabaseConnection>().is_some() {
                tracing::info!("AuthExtractors: DatabaseConnection registered");
            } else {
                tracing::warn!(
                    "AuthExtractors: DatabaseConnection not registered. \
                     AuthUser<U> injection will fail at request time. \
                     AuthInfo will still work."
                );
            }
        } else {
            tracing::warn!(
                "AuthExtractors: No DI context configured. \
                 AuthUser<U> and AuthInfo injection will fail."
            );
        }
    }
}
```

## Handler Examples

```rust
use reinhardt::{get, Response, ViewResult, Path};
use reinhardt::auth::{AuthInfo, AuthUser};
use reinhardt::DefaultUser;

// ── Authenticated endpoint (401 if not authenticated) ──────────────
#[get("/profile/")]
pub async fn profile(
    #[inject] AuthUser(user): AuthUser<DefaultUser>,
) -> ViewResult<Response> {
    let username = user.username();
    Ok(Response::ok().with_json(&json!({"username": username}))?)
}

// ── Lightweight auth check (no DB access) ──────────────────────────
#[get("/admin/")]
pub async fn admin(
    #[inject] AuthInfo(state): AuthInfo,
) -> ViewResult<Response> {
    if !state.is_admin() {
        return Err(Error::Forbidden("Admin access required"));
    }
    Ok(Response::ok().with_body("Welcome, admin"))
}

// ── Optional authentication (None if not authenticated) ────────────
#[get("/home/")]
pub async fn home(
    #[inject] auth: Option<AuthInfo>,
) -> ViewResult<Response> {
    match auth {
        Some(AuthInfo(state)) => {
            Ok(Response::ok().with_body(format!("Hello, {}", state.user_id())))
        }
        None => Ok(Response::ok().with_body("Hello, guest")),
    }
}

// ── Mixed: auth extractor + DI service ─────────────────────────────
#[get("/dashboard/")]
pub async fn dashboard(
    #[inject] AuthInfo(state): AuthInfo,
    #[inject] analytics: AnalyticsService,
) -> ViewResult<Response> {
    let stats = analytics.get_user_stats(state.user_id()).await?;
    Ok(Response::ok().with_json(&stats)?)
}

// ── Path params + auth ─────────────────────────────────────────────
#[get("/users/{id}/")]
pub async fn get_user(
    Path(id): Path<i64>,
    #[inject] AuthUser(current): AuthUser<DefaultUser>,
) -> ViewResult<Response> {
    // current user can only view their own profile or is admin
    // ...
}

// ── Optional user with DB access ───────────────────────────────────
#[get("/articles/{id}/")]
pub async fn article(
    Path(id): Path<i64>,
    #[inject] user: Option<AuthUser<DefaultUser>>,
) -> ViewResult<Response> {
    let article = load_article(id).await?;
    match user {
        Some(AuthUser(u)) if u.is_staff() => {
            // Show edit controls
        }
        _ => {
            // Read-only view
        }
    }
    // ...
}

// ── Deprecated CurrentUser still works (during RC) ─────────────────
#[get("/legacy/", use_inject = true)]
pub async fn legacy(
    #[inject] user: CurrentUser<DefaultUser>,  // deprecated, use AuthUser
) -> ViewResult<Response> {
    if user.is_authenticated() {
        // ...
    }
    // ...
}
```

## Type Semantics Summary

| Parameter Type | Auth Required | DB Query | On Failure |
|---------------|---------------|----------|------------|
| `AuthInfo` | Yes | No | Injection error (→ 401) |
| `Option<AuthInfo>` | No | No | `None` |
| `AuthUser<U>` | Yes | Yes | Injection error (→ 401/500) |
| `Option<AuthUser<U>>` | No | Yes | `None` (see security note) |

## Affected Crates

| Crate | Changes |
|-------|---------|
| `reinhardt-di` | `Option<T>` blanket `Injectable` impl |
| `reinhardt-auth` | `AuthUser<U>` type + `Injectable` impl, `AuthInfo` type + `Injectable` impl, `CurrentUser<U>` deprecated, `AuthExtractors` middleware |
| `reinhardt-core/macros` | Auto-detect `#[inject]` without `use_inject = true`, update trybuild test |
| `reinhardt` (facade) | Re-export `AuthUser`, `AuthInfo`, `AuthExtractors` |

## Test Plan

### Unit tests (`reinhardt-auth`)

- `AuthInfo::inject()` with valid `AuthState` → `Ok(AuthInfo(state))`
- `AuthInfo::inject()` without `AuthState` → `Err` (not authenticated)
- `AuthInfo::inject()` with unauthenticated `AuthState` → `Err`
- `AuthUser::inject()` full success path → `Ok(AuthUser(user))`
- `AuthUser::inject()` with malformed `user_id` → `Err` (not nil UUID)
- `AuthUser::inject()` without `DatabaseConnection` → `Err`
- `AuthUser::inject()` user not found in DB → `Err`
- `Option<AuthInfo>::inject()` without `AuthState` → `Ok(None)`
- `Option<AuthUser<U>>::inject()` DB failure → `Ok(None)`
- `AuthExtractors::on_startup()` with/without DB → correct log output
- Deprecated `CurrentUser<U>` still compiles with deprecation warning

### Unit tests (`reinhardt-di`)

- `Option<T>::inject()` blanket impl: success → `Some`, failure → `None`
- No conflict with existing `Arc<T>` blanket impl

### Integration tests (`tests/integration`)

- End-to-end: auth middleware → `#[inject] AuthInfo` → 200
- End-to-end: auth middleware → `#[inject] AuthUser` → DB query → 200
- Unauthenticated request → `#[inject] AuthInfo` → 401
- Unauthenticated request → `#[inject] Option<AuthInfo>` → 200 with `None`
- Mixed `#[inject]` handler: auth extractor + DI service
- Auto-detection: `#[inject]` works without `use_inject = true`

### Macro tests (`reinhardt-core/macros`)

- trybuild: `#[inject]` auto-detection generates correct wrapper
- trybuild: destructuring `AuthUser(user): AuthUser<U>` compiles
- Update `inject_without_use_inject.rs` trybuild test (now expects success)

## RC Phase Considerations

This change adds new public APIs during the RC phase:
- `AuthUser<U>` — new type
- `AuthInfo` — new type
- `Option<T>` blanket `Injectable` impl — new impl
- `AuthExtractors` — new middleware
- `#[inject]` auto-detection — behavior change (but backward compatible)

Requires SP-6 approval:
- Labels: `enhancement` + `rc-addition`
- Maintainer approval required

`CurrentUser<U>` is deprecated but NOT removed. It continues to work
with a deprecation warning during the RC phase.

In `develop/0.2.0`:
- Remove the old `CurrentUser<U>` struct
- Add `pub type CurrentUser<U> = AuthUser<U>` for familiarity
