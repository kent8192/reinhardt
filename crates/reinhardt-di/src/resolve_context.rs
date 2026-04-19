//! Task-local resolve context for accessing `InjectionContext` within DI factories.
//!
//! This module provides [`get_di_context`] which retrieves the active
//! `InjectionContext` during `#[injectable_factory]` execution.
//! The context is stored in a [`tokio::task_local!`] and set automatically
//! by the macro-generated wrapper code.

use std::sync::Arc;

use crate::context::InjectionContext;

/// Selects which `InjectionContext` level to retrieve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextLevel {
	/// The root (application-level) context created at startup.
	Root,
	/// The currently active resolving context (may be a request-scoped fork).
	Current,
}

/// Internal storage for the task-local resolve context.
///
/// Holds both the root and current contexts so that factories can
/// access either level via [`get_di_context`].
pub struct ResolveContext {
	/// The root (application-level) context.
	pub root: Arc<InjectionContext>,
	/// The currently active context performing the resolution.
	pub current: Arc<InjectionContext>,
}

tokio::task_local! {
	/// Task-local storage for the resolve context.
	///
	/// Set by macro-generated wrapper code before calling the user's
	/// factory implementation. Automatically restored on scope exit.
	pub static RESOLVE_CTX: ResolveContext;
}

/// Retrieves the [`InjectionContext`] for the given [`ContextLevel`].
///
/// This function is intended to be called within `#[injectable_factory]`
/// function bodies to access the DI context without `#[inject]`.
///
/// # Panics
///
/// Panics if called outside of a DI resolution context (i.e., not
/// within an `#[injectable_factory]` or `#[injectable]` execution).
pub fn get_di_context(level: ContextLevel) -> Arc<InjectionContext> {
	RESOLVE_CTX.with(|ctx| match level {
		ContextLevel::Root => Arc::clone(&ctx.root),
		ContextLevel::Current => Arc::clone(&ctx.current),
	})
}

/// Non-panicking variant of [`get_di_context`].
///
/// Returns `None` if called outside of a DI resolution context.
pub fn try_get_di_context(level: ContextLevel) -> Option<Arc<InjectionContext>> {
	RESOLVE_CTX
		.try_with(|ctx| match level {
			ContextLevel::Root => Arc::clone(&ctx.root),
			ContextLevel::Current => Arc::clone(&ctx.current),
		})
		.ok()
}

#[cfg(test)]
#[path = "resolve_context/tests.rs"]
mod tests;
