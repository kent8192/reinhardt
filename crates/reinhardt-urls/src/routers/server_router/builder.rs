//! Builder methods for [`ServerRouter`].
//!
//! Holds the constructor, prefix/namespace/DI configuration, middleware
//! registration, and child router composition (`mount`, `group`).

use super::ServerRouter;
use super::types::MiddlewareInfo;
use crate::routers::UrlReverser;
use reinhardt_di::InjectionContext;
use reinhardt_http::ExcludeMiddleware;
use reinhardt_middleware::Middleware;
#[cfg(feature = "viewsets")]
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

impl ServerRouter {
	/// Validate that a prefix for `mount`/`include` follows Django URL conventions.
	///
	/// # Panics
	///
	/// Panics if the prefix doesn't end with "/".
	/// This matches Django's behavior where URL patterns must end with a trailing slash.
	/// Use "/" for root mounting instead of an empty string "".
	///
	/// # Examples
	///
	/// ```should_panic
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// // This will panic because "api" doesn't end with "/"
	/// let router = ServerRouter::new()
	///     .mount("api", ServerRouter::new());
	/// ```
	///
	/// ```should_panic
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// // This will panic because "" is not allowed, use "/" instead
	/// let router = ServerRouter::new()
	///     .mount("", ServerRouter::new());
	/// ```
	fn validate_prefix(prefix: &str) {
		// Prefix must not contain path parameter placeholders.
		// Mount prefixes are matched as literal strings, so a placeholder like
		// `{org}` would never match an actual path segment and all child routes
		// would silently 404. Fail early at construction time instead.
		if prefix.contains('{') || prefix.contains('}') {
			panic!(
				"`mount()` prefix `{prefix}` contains a path parameter placeholder (`{{...}}`); this is not supported.\nUse `route()` with the full path on the child router instead, or mount at a literal prefix."
			);
		}

		// Prefix must end with "/"
		if !prefix.ends_with('/') {
			if prefix.is_empty() {
				panic!(
					"URL route prefix cannot be an empty string. \
					 Use '/' instead of ''. \
					 This follows Django URL configuration conventions."
				);
			} else {
				panic!(
					"URL route '{}' must end with a trailing slash '/'. \
					 Use '{}/' instead of '{}'. \
					 This follows Django URL configuration conventions.",
					prefix, prefix, prefix,
				);
			}
		}
	}

	/// Create a new ServerRouter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// let router = ServerRouter::new();
	/// ```
	pub fn new() -> Self {
		Self {
			prefix: String::new(),
			namespace: None,
			routes: Vec::new(),
			#[cfg(feature = "viewsets")]
			viewsets: HashMap::new(),
			functions: Vec::new(),
			configuration_errors: Vec::new(),
			views: Vec::new(),
			children: Vec::new(),
			di_context: None,
			pending_middleware_di: reinhardt_di::DiRegistrationList::new(),
			middleware: Vec::new(),
			middleware_names: Vec::new(),
			middleware_exclusions: Vec::new(),
			reverser: UrlReverser::new(),
			compiled_routes: OnceLock::new(),
		}
	}

	#[doc(hidden)]
	pub fn with_configuration_error(mut self, error: impl Into<String>) -> Self {
		self.invalidate_compiled_routes();
		self.configuration_errors.push(error.into());
		self
	}

	/// Set the prefix for this router
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// let router = ServerRouter::new()
	///     .with_prefix("/api/v1");
	/// ```
	pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
		self.invalidate_compiled_routes();
		self.prefix = prefix.into();
		self
	}

	/// Set the namespace for this router
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// let router = ServerRouter::new()
	///     .with_namespace("v1");
	/// ```
	pub fn with_namespace(mut self, namespace: impl Into<String>) -> Self {
		self.invalidate_compiled_routes();
		self.namespace = Some(namespace.into());
		self
	}

	/// Set the DI context for this router
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// use reinhardt_di::{InjectionContext, SingletonScope};
	/// use std::sync::Arc;
	///
	/// let singleton_scope = Arc::new(SingletonScope::new());
	/// let di_ctx = Arc::new(InjectionContext::builder(singleton_scope).build());
	/// let router = ServerRouter::new()
	///     .with_di_context(di_ctx);
	/// ```
	pub fn with_di_context(mut self, ctx: Arc<InjectionContext>) -> Self {
		self.invalidate_compiled_routes();
		// Drain any middleware-contributed DI registrations that were harvested
		// before `with_di_context` was called, walking children that have not
		// already been bound to their own context (e.g., a child mounted
		// before `with_di_context` was attached). Without this, registrations
		// staged by an earlier `with_middleware` on this router or any
		// not-yet-bound child would never reach this context's
		// `SingletonScope` (startup's `take_di_registrations()` path is
		// skipped whenever a user-supplied context exists). See #4426.
		Self::adopt_di_context_recursive(&mut self, &ctx);
		self.di_context = Some(ctx);
		self
	}

	/// Propagate a newly attached `InjectionContext` into every descendant
	/// that has no context of its own, draining each router's pending
	/// middleware DI registrations into the context's `SingletonScope` along
	/// the way. Descendants that already own a different context are left
	/// untouched. See #4426.
	/// If `parent` owns an `InjectionContext` and `child` does not, recursively
	/// adopt the parent's context into the child's subtree (draining any
	/// pending middleware DI registrations along the way) and attach the
	/// context to the child itself. Used by `mount`, `mount_mut`, and `group`
	/// to keep the DI propagation behavior in one place. See #4426.
	fn inherit_context_from_parent_if_any(parent: &ServerRouter, child: &mut ServerRouter) {
		if child.di_context.is_none()
			&& let Some(parent_ctx) = parent.di_context.as_ref()
		{
			Self::adopt_di_context_recursive(child, parent_ctx);
			child.di_context = Some(Arc::clone(parent_ctx));
		}
	}

	fn adopt_di_context_recursive(router: &mut ServerRouter, ctx: &Arc<InjectionContext>) {
		if !router.pending_middleware_di.is_empty() {
			let pending = std::mem::take(&mut router.pending_middleware_di);
			pending.apply_to(ctx.singleton_scope());
		}
		for child in router.children.iter_mut() {
			if child.di_context.is_none() {
				Self::adopt_di_context_recursive(child, ctx);
				child.di_context = Some(Arc::clone(ctx));
			}
		}
	}

	/// Returns a reference to the DI context, if set.
	pub(crate) fn di_context(&self) -> Option<&Arc<InjectionContext>> {
		self.di_context.as_ref()
	}

	/// Add middleware to this router
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// use reinhardt_middleware::LoggingMiddleware;
	///
	/// let router = ServerRouter::new()
	///     .with_middleware(LoggingMiddleware::new());
	/// ```
	pub fn with_middleware<M: Middleware + 'static>(mut self, mw: M) -> Self {
		self.invalidate_compiled_routes();
		let full_type_name = std::any::type_name::<M>().to_string();
		let short_name = full_type_name
			.rsplit("::")
			.next()
			.unwrap_or(&full_type_name)
			.to_string();
		// Harvest middleware-contributed DI singleton registrations. Decision
		// is order-independent across `with_middleware` / `with_di_context`:
		//   - If a DI context is already attached, apply directly to its
		//     `SingletonScope` so handlers resolved through it see the value.
		//   - Otherwise, stage into `pending_middleware_di`. A later
		//     `with_di_context` will drain it into the new context; if no
		//     context is ever attached, `register_all_routes` flushes it to
		//     the global deferred list (the path startup consumes when no
		//     user-supplied context exists). This eliminates both the silent
		//     drop on `with_middleware` → `with_di_context` ordering and the
		//     global-list leak that would otherwise occur. See #4426.
		let di_entries = mw.di_registrations();
		if !di_entries.is_empty() {
			if let Some(ctx) = self.di_context.as_ref() {
				let scope = ctx.singleton_scope();
				for (type_id, value) in di_entries {
					scope.set_arc_any(type_id, value);
				}
			} else {
				for (type_id, value) in di_entries {
					self.pending_middleware_di.register_arc_any(type_id, value);
				}
			}
		}
		self.middleware_names.push(MiddlewareInfo {
			name: short_name,
			type_name: full_type_name,
		});
		self.middleware.push(Arc::new(mw));
		self.middleware_exclusions.push(Vec::new());
		self
	}

	/// Exclude a URL path from the most recently added middleware.
	///
	/// Paths ending with `/` are treated as prefix matches: any request
	/// path starting with the given prefix will skip the middleware.
	/// Paths without trailing `/` require an exact match.
	///
	/// This method operates on the **last middleware** added via
	/// [`with_middleware()`](Self::with_middleware). Multiple `.exclude()`
	/// calls accumulate exclusions on the same middleware.
	///
	/// # Panics
	///
	/// Panics if no middleware has been added yet.
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	/// use reinhardt_middleware::LoggingMiddleware;
	///
	/// let router = ServerRouter::new()
	///     .with_middleware(LoggingMiddleware::new())
	///         .exclude("/api/auth/")    // prefix: skips /api/auth/*
	///         .exclude("/health");      // exact: skips only /health
	/// ```
	pub fn exclude(mut self, pattern: &str) -> Self {
		self.invalidate_compiled_routes();
		assert!(
			!self.middleware_exclusions.is_empty(),
			"exclude() called with no middleware. Call with_middleware() first."
		);
		self.middleware_exclusions
			.last_mut()
			.unwrap()
			.push(pattern.to_string());
		self
	}

	/// Build middleware list, wrapping any with exclusions in `ExcludeMiddleware`.
	pub(crate) fn build_middleware_with_exclusions(&self) -> Vec<Arc<dyn Middleware>> {
		let mut result: Vec<Arc<dyn Middleware>> = Vec::with_capacity(self.middleware.len());

		for (mw, exclusions) in self
			.middleware
			.iter()
			.zip(self.middleware_exclusions.iter())
		{
			if exclusions.is_empty() {
				result.push(mw.clone());
			} else {
				let mut exclude_mw = ExcludeMiddleware::new(mw.clone());
				for pattern in exclusions {
					exclude_mw.add_exclusion_mut(pattern);
				}
				result.push(Arc::new(exclude_mw) as Arc<dyn Middleware>);
			}
		}

		result
	}

	/// Mount a child router at the given prefix
	///
	/// # Panics
	///
	/// Panics if the prefix is non-empty, not "/" and doesn't end with "/".
	/// This follows Django's URL configuration conventions.
	///
	/// Also panics if the prefix contains a path parameter placeholder
	/// (e.g. `/orgs/{org}/`). Mount prefixes are matched as literal strings,
	/// so placeholders would silently cause all child routes to return 404.
	/// Use `route()` with the full path on the child router instead, or
	/// mount at a literal prefix.
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// let users_router = ServerRouter::new()
	///     .with_namespace("users");
	///
	/// let router = ServerRouter::new()
	///     .with_prefix("/api")
	///     .mount("/users/", users_router);  // Note: trailing slash required
	///
	/// // Verify the router was created successfully
	/// assert_eq!(router.prefix(), "/api");
	/// ```
	///
	/// Using "/" for root mounting is also valid:
	///
	/// ```rust
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// let app_router = ServerRouter::new();
	/// let router = ServerRouter::new().mount("/", app_router);
	/// ```
	pub fn mount(mut self, prefix: &str, mut child: ServerRouter) -> Self {
		self.invalidate_compiled_routes();
		// Validate prefix follows Django URL conventions
		Self::validate_prefix(prefix);

		// Set prefix if not already set
		if child.prefix.is_empty() {
			child.prefix = prefix.to_string();
		}
		child.invalidate_compiled_routes();

		// Inherit DI context if child doesn't have one. When inheriting,
		// recursively drain pending middleware DI from the entire subtree
		// (the child itself AND any grandchildren that also lack a context),
		// mirroring `with_di_context`. A non-recursive drain would leave
		// nested grandchildren's staged registrations stranded; later
		// `register_all_routes` would push them to the global list, which
		// startup skips when the top router owns a context. See #4426.
		Self::inherit_context_from_parent_if_any(&self, &mut child);

		self.children.push(child);
		self
	}

	/// Mount a child router (mutable version)
	///
	/// # Examples
	///
	/// ```rust,no_run
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// let mut router = ServerRouter::new();
	/// let users_router = ServerRouter::new();
	///
	/// router.mount_mut("/users/", users_router);
	/// ```
	pub fn mount_mut(&mut self, prefix: &str, mut child: ServerRouter) {
		self.invalidate_compiled_routes();
		// Validate prefix follows Django URL conventions
		Self::validate_prefix(prefix);

		if child.prefix.is_empty() {
			child.prefix = prefix.to_string();
		}
		child.invalidate_compiled_routes();
		// See `mount` for the rationale on recursive subtree adoption.
		Self::inherit_context_from_parent_if_any(self, &mut child);
		self.children.push(child);
	}

	/// Add multiple child routers at once
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_urls::routers::ServerRouter;
	///
	/// let users = ServerRouter::new().with_prefix("/users");
	/// let posts = ServerRouter::new().with_prefix("/posts");
	///
	/// let router = ServerRouter::new()
	///     .group(vec![users, posts]);
	///
	/// // Verify the router was created successfully
	/// assert_eq!(router.prefix(), "");
	/// ```
	pub fn group(mut self, routers: Vec<ServerRouter>) -> Self {
		for mut router in routers {
			self.invalidate_compiled_routes();
			router.invalidate_compiled_routes();
			// Mirror `mount`: when this router owns a context, recursively
			// adopt the grouped subtree into it so any pending middleware DI
			// staged before grouping is drained. Otherwise the grouped
			// subtree's pending lists would later be pushed onto the global
			// deferred list, which startup skips when the parent owns a
			// context. See #4426.
			Self::inherit_context_from_parent_if_any(&self, &mut router);
			self.children.push(router);
		}
		self
	}
}

#[cfg(test)]
mod middleware_di_tests {
	use super::*;
	use async_trait::async_trait;
	use reinhardt_core::exception::Result;
	use reinhardt_di::{InjectionContext, SingletonScope};
	use reinhardt_http::{Handler, Request, Response};
	use rstest::rstest;
	use std::any::TypeId;
	use std::sync::Arc;

	#[derive(Debug, PartialEq, Eq)]
	struct DummyState(&'static str);

	struct DummyMiddleware {
		state: Arc<DummyState>,
	}

	#[async_trait]
	impl Middleware for DummyMiddleware {
		async fn process(&self, request: Request, handler: Arc<dyn Handler>) -> Result<Response> {
			handler.handle(request).await
		}

		fn di_registrations(&self) -> Vec<reinhardt_http::MiddlewareDiRegistration> {
			vec![(
				TypeId::of::<DummyState>(),
				Arc::clone(&self.state) as Arc<dyn std::any::Any + Send + Sync>,
			)]
		}
	}

	fn make_mw(tag: &'static str) -> DummyMiddleware {
		DummyMiddleware {
			state: Arc::new(DummyState(tag)),
		}
	}

	#[rstest]
	#[serial_test::serial(global_di)]
	fn with_middleware_before_with_di_context_applies_to_context() {
		// Arrange: builder calls in the order that previously dropped the
		// registration (with_middleware first, with_di_context later).
		let scope = Arc::new(SingletonScope::new());
		let ctx = Arc::new(InjectionContext::builder(Arc::clone(&scope)).build());

		// Act
		let _router = ServerRouter::new()
			.with_middleware(make_mw("before-context"))
			.with_di_context(Arc::clone(&ctx));

		// Drain the global list to assert nothing leaked there.
		let leaked = crate::routers::take_di_registrations();

		// Assert: scope resolves the middleware-owned singleton, and the
		// global deferred list received nothing.
		let resolved = scope
			.get::<DummyState>()
			.expect("with_di_context must drain pending middleware DI into the new context");
		assert_eq!(resolved.0, "before-context");
		assert!(
			leaked.is_none(),
			"pending middleware DI must not leak into the global deferred list when a context is attached later"
		);
	}

	#[rstest]
	#[serial_test::serial(global_di)]
	fn with_middleware_after_with_di_context_applies_to_context() {
		// Arrange
		let scope = Arc::new(SingletonScope::new());
		let ctx = Arc::new(InjectionContext::builder(Arc::clone(&scope)).build());

		// Act: reverse order — context first, then middleware.
		let _router = ServerRouter::new()
			.with_di_context(Arc::clone(&ctx))
			.with_middleware(make_mw("after-context"));

		let leaked = crate::routers::take_di_registrations();

		// Assert
		let resolved = scope
			.get::<DummyState>()
			.expect("with_middleware after with_di_context must apply directly to context scope");
		assert_eq!(resolved.0, "after-context");
		assert!(leaked.is_none());
	}

	#[rstest]
	#[serial_test::serial(global_di)]
	fn with_middleware_without_context_flushes_to_global_on_register_all_routes() {
		// Arrange: no DI context ever attached. Pending must flush to global
		// on `register_all_routes`.
		let _ = crate::routers::take_di_registrations(); // clear any leftover

		// Act
		let mut router = ServerRouter::new().with_middleware(make_mw("no-context"));
		let _errors = router.register_all_routes();

		// Assert: global deferred list now contains the registration.
		let taken = crate::routers::take_di_registrations()
			.expect("register_all_routes must flush pending middleware DI when no context is set");
		let scope = SingletonScope::new();
		taken.apply_to(&scope);
		let resolved = scope.get::<DummyState>().expect(
			"flushed registration must resolve from the global deferred list after apply_to",
		);
		assert_eq!(resolved.0, "no-context");
	}

	#[rstest]
	#[serial_test::serial(global_di)]
	fn group_drains_grouped_router_pending_into_parent_context() {
		// Arrange: each grouped child stages its own middleware DI; one of
		// them also has a nested grandchild with pending DI to verify the
		// recursive walk through `group`.
		let scope = Arc::new(SingletonScope::new());
		let ctx = Arc::new(InjectionContext::builder(Arc::clone(&scope)).build());
		let users = ServerRouter::new()
			.with_prefix("/users")
			.with_middleware(make_mw("group-users"));
		let posts_grandchild =
			ServerRouter::new().with_middleware(make_mw("group-posts-grandchild"));
		let posts = ServerRouter::new()
			.with_prefix("/posts")
			.mount("/comments/", posts_grandchild);

		// Act: group both routers under a context-owning parent.
		let _parent = ServerRouter::new()
			.with_di_context(Arc::clone(&ctx))
			.group(vec![users, posts]);

		let leaked = crate::routers::take_di_registrations();

		// Assert: both staged values reach the parent's scope; the second
		// `set_arc_any` overwrites the first under the same `DummyState`
		// `TypeId`, so we only verify presence and absence of global leak.
		let resolved = scope.get::<DummyState>().expect(
			"group must recursively drain grouped routers' pending middleware DI into the parent context",
		);
		assert!(matches!(
			resolved.0,
			"group-users" | "group-posts-grandchild"
		));
		assert!(leaked.is_none());
	}

	#[rstest]
	#[serial_test::serial(global_di)]
	fn nested_grandchild_pending_drains_into_parent_context_on_mount() {
		// Arrange: build a grandchild with pending middleware DI, nest it
		// inside a child (neither has a context yet), then mount the whole
		// subtree under a parent that already owns a context. `mount` must
		// recursively drain the grandchild — not just the immediate child.
		let scope = Arc::new(SingletonScope::new());
		let ctx = Arc::new(InjectionContext::builder(Arc::clone(&scope)).build());
		let grandchild = ServerRouter::new().with_middleware(make_mw("nested-grandchild"));
		let child = ServerRouter::new().mount("/users/", grandchild);

		// Act
		let _parent = ServerRouter::new()
			.with_di_context(Arc::clone(&ctx))
			.mount("/api/", child);

		let leaked = crate::routers::take_di_registrations();

		// Assert
		let resolved = scope.get::<DummyState>().expect(
			"mount must recursively drain grandchildren's pending middleware DI into the parent's context",
		);
		assert_eq!(resolved.0, "nested-grandchild");
		assert!(leaked.is_none());
	}

	#[rstest]
	#[serial_test::serial(global_di)]
	fn child_pending_drains_when_context_attached_after_mount() {
		// Arrange: child is mounted BEFORE the parent has a DI context, so
		// `mount` cannot drain. Then `with_di_context` runs on the parent and
		// must propagate into the already-mounted child.
		let scope = Arc::new(SingletonScope::new());
		let ctx = Arc::new(InjectionContext::builder(Arc::clone(&scope)).build());
		let child = ServerRouter::new().with_middleware(make_mw("late-context-child"));

		// Act: mount first, then attach context.
		let _parent = ServerRouter::new()
			.mount("/api/", child)
			.with_di_context(Arc::clone(&ctx));

		let leaked = crate::routers::take_di_registrations();

		// Assert
		let resolved = scope.get::<DummyState>().expect(
			"attaching a context after mounting a child with pending middleware DI must propagate into the child",
		);
		assert_eq!(resolved.0, "late-context-child");
		assert!(leaked.is_none());
	}

	#[rstest]
	#[serial_test::serial(global_di)]
	fn child_pending_drains_into_parent_context_on_mount() {
		// Arrange: parent has a context; child staged a middleware DI before
		// being mounted under the parent.
		let scope = Arc::new(SingletonScope::new());
		let ctx = Arc::new(InjectionContext::builder(Arc::clone(&scope)).build());
		let child = ServerRouter::new().with_middleware(make_mw("mounted-child"));

		// Act
		let _parent = ServerRouter::new()
			.with_di_context(Arc::clone(&ctx))
			.mount("/api/", child);

		let leaked = crate::routers::take_di_registrations();

		// Assert
		let resolved = scope.get::<DummyState>().expect(
			"mounting a child with pending middleware DI into a context-bearing parent must drain into the parent's scope",
		);
		assert_eq!(resolved.0, "mounted-child");
		assert!(leaked.is_none());
	}
}
