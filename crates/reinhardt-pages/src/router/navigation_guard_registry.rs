//! Registry and ordered execution for navigation guards.

use super::navigation_guard::{NavigationContext, NavigationDecision, NavigationGuardError};
use reinhardt_urls::routers::client_router::NavigationGuardId;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

/// Erased future returned by a registered navigation guard.
pub type NavigationGuardFuture =
	Pin<Box<dyn Future<Output = Result<NavigationDecision, NavigationGuardError>> + 'static>>;

/// Erased navigation guard executor.
pub type NavigationGuardExecutor = fn(NavigationContext) -> NavigationGuardFuture;

/// Static registration record for one navigation guard.
pub struct NavigationGuardRegistration {
	/// Stable guard identifier.
	pub id: NavigationGuardId,
	/// Erased execution entry point.
	pub execute: NavigationGuardExecutor,
}

impl NavigationGuardRegistration {
	/// Creates a static registration record.
	pub const fn new(id: NavigationGuardId, execute: NavigationGuardExecutor) -> Self {
		Self { id, execute }
	}
}

inventory::collect!(NavigationGuardRegistration);

/// Read-only lookup table for erased navigation guard registrations.
pub struct NavigationGuardRegistry {
	entries: HashMap<NavigationGuardId, &'static NavigationGuardRegistration>,
}

impl NavigationGuardRegistry {
	/// Builds a registry and rejects duplicate IDs.
	pub fn from_entries<I>(entries: I) -> Result<Self, NavigationGuardError>
	where
		I: IntoIterator<Item = &'static NavigationGuardRegistration>,
	{
		let mut indexed = HashMap::new();
		for entry in entries {
			if indexed.insert(entry.id, entry).is_some() {
				return Err(NavigationGuardError::with_status(
					format!("duplicate navigation guard `{}`", entry.id.as_str()),
					500,
				));
			}
		}
		Ok(Self { entries: indexed })
	}

	/// Collects all inventory registrations for the current application.
	pub fn global() -> Result<Self, NavigationGuardError> {
		Self::from_entries(inventory::iter::<NavigationGuardRegistration>)
	}

	fn get(
		&self,
		id: NavigationGuardId,
	) -> Result<&'static NavigationGuardRegistration, NavigationGuardError> {
		self.entries.get(&id).copied().ok_or_else(|| {
			NavigationGuardError::with_status(
				format!("navigation guard `{}` is not registered", id.as_str()),
				500,
			)
		})
	}
}

/// Executes navigation guards in route-tree order.
pub async fn execute_navigation_guards(
	registry: &NavigationGuardRegistry,
	ids: &[NavigationGuardId],
	context: NavigationContext,
) -> Result<NavigationDecision, NavigationGuardError> {
	for id in ids {
		let decision = (registry.get(*id)?.execute)(context.clone()).await?;
		if decision != NavigationDecision::Allow {
			return Ok(decision);
		}
	}
	Ok(NavigationDecision::Allow)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
	use super::*;
	use crate::cancellation::CancellationSource;
	use crate::reactive::{QueryClient, QueryDefaults};
	use crate::router::navigation_guard::NavigationKind;
	use reinhardt_urls::routers::client_router::RouteContext;
	use std::cell::RefCell;
	use std::collections::HashMap;

	thread_local! {
		static EXECUTIONS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
	}

	fn context() -> NavigationContext {
		let source = CancellationSource::new();
		NavigationContext::new(
			"/projects/7/?tab=activity".to_string(),
			RouteContext::new(
				"/projects/7/".to_string(),
				HashMap::from([("project_id".to_string(), "7".to_string())]),
				"tab=activity".to_string(),
			),
			NavigationKind::Push,
			QueryClient::new(QueryDefaults::default()),
			source.handle(),
			crate::reactive::QueryConsumer::Navigation(1),
			#[cfg(native)]
			None,
		)
	}

	fn allow_first(_: NavigationContext) -> NavigationGuardFuture {
		Box::pin(async {
			EXECUTIONS.with(|executions| executions.borrow_mut().push("first"));
			Ok(NavigationDecision::Allow)
		})
	}

	fn redirect_second(_: NavigationContext) -> NavigationGuardFuture {
		Box::pin(async {
			EXECUTIONS.with(|executions| executions.borrow_mut().push("second"));
			Ok(NavigationDecision::Redirect {
				location: "/login/".to_string(),
				replace: true,
			})
		})
	}

	fn never_run(_: NavigationContext) -> NavigationGuardFuture {
		Box::pin(async {
			EXECUTIONS.with(|executions| executions.borrow_mut().push("third"));
			Ok(NavigationDecision::Allow)
		})
	}

	static DUPLICATE_FIRST: NavigationGuardRegistration =
		NavigationGuardRegistration::new(NavigationGuardId::new("duplicate"), allow_first);
	static DUPLICATE_SECOND: NavigationGuardRegistration =
		NavigationGuardRegistration::new(NavigationGuardId::new("duplicate"), allow_first);
	static FIRST: NavigationGuardRegistration =
		NavigationGuardRegistration::new(NavigationGuardId::new("first"), allow_first);
	static SECOND: NavigationGuardRegistration =
		NavigationGuardRegistration::new(NavigationGuardId::new("second"), redirect_second);
	static THIRD: NavigationGuardRegistration =
		NavigationGuardRegistration::new(NavigationGuardId::new("third"), never_run);

	#[test]
	fn duplicate_ids_are_safe_errors() {
		let error =
			match NavigationGuardRegistry::from_entries([&DUPLICATE_FIRST, &DUPLICATE_SECOND]) {
				Err(error) => error,
				Ok(_) => panic!("duplicate navigation guards must be rejected"),
			};
		assert_eq!(error.status(), Some(500));
		assert_eq!(
			error.public_message(),
			"duplicate navigation guard `duplicate`"
		);
	}

	#[test]
	fn execution_preserves_order_and_short_circuits() {
		EXECUTIONS.with(|executions| executions.borrow_mut().clear());
		let registry = NavigationGuardRegistry::from_entries([&FIRST, &SECOND, &THIRD]).unwrap();

		let decision = tokio_test::block_on(execute_navigation_guards(
			&registry,
			&[
				NavigationGuardId::new("first"),
				NavigationGuardId::new("second"),
				NavigationGuardId::new("third"),
			],
			context(),
		))
		.unwrap();

		assert_eq!(
			decision,
			NavigationDecision::Redirect {
				location: "/login/".to_string(),
				replace: true,
			}
		);
		EXECUTIONS.with(|executions| assert_eq!(*executions.borrow(), ["first", "second"]));
	}

	#[test]
	fn missing_ids_are_safe_errors() {
		let registry = NavigationGuardRegistry::from_entries(std::iter::empty::<
			&'static NavigationGuardRegistration,
		>())
		.unwrap();
		let error = tokio_test::block_on(execute_navigation_guards(
			&registry,
			&[NavigationGuardId::new("missing")],
			context(),
		))
		.unwrap_err();
		assert_eq!(error.status(), Some(500));
		assert_eq!(
			error.public_message(),
			"navigation guard `missing` is not registered"
		);
	}
}
