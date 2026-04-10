//! Runserver lifecycle hooks for extending server startup behavior.
//!
//! Hooks are auto-discovered via `inventory` and invoked by
//! [`RunServerCommand`](crate::RunServerCommand) at two lifecycle points:
//!
//! 1. **Validation** ([`RunserverHook::validate`]) — before DI setup; return `Err` to abort.
//! 2. **Startup** ([`RunserverHook::on_server_start`]) — after DI is ready, before listen.
//!
//! Register hooks with the `#[hook(on = runserver)]` attribute macro.

use std::error::Error;
use std::sync::Arc;

use async_trait::async_trait;
use reinhardt_di::InjectionContext;
use reinhardt_server::ShutdownCoordinator;

/// Context passed to [`RunserverHook::on_server_start`].
pub struct RunserverContext {
	/// Shutdown coordinator for subscribing to graceful shutdown signals.
	///
	/// Use `shutdown_coordinator.subscribe()` to receive shutdown notifications
	/// in spawned concurrent services.
	pub shutdown_coordinator: ShutdownCoordinator,

	/// DI context for resolving application services.
	pub di_context: Arc<InjectionContext>,
}

/// Hook for extending runserver behavior.
///
/// Implementations are auto-discovered via `inventory` when registered
/// with the `#[hook(on = runserver)]` attribute macro.
///
/// # Lifecycle
///
/// 1. `validate()` is called before DI context setup.
///    Return `Err` to abort server startup (fail-fast).
/// 2. `on_server_start()` is called after DI context is ready,
///    before the server starts listening.
///    Spawn concurrent services here and subscribe to shutdown via
///    `ctx.shutdown_coordinator.subscribe()`.
///
/// Both methods have default no-op implementations. Implement only
/// the phases your hook needs.
///
/// # Examples
///
/// ```rust,ignore
/// use reinhardt::commands::{RunserverHook, RunserverContext};
///
/// #[reinhardt::hook(on = runserver)]
/// struct MyHook;
///
/// #[async_trait]
/// impl RunserverHook for MyHook {
///     async fn validate(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
///         // Check required configuration
///         Ok(())
///     }
///
///     async fn on_server_start(
///         &self,
///         ctx: &RunserverContext,
///     ) -> Result<(), Box<dyn Error + Send + Sync>> {
///         // Spawn a concurrent service
///         let mut shutdown_rx = ctx.shutdown_coordinator.subscribe();
///         tokio::spawn(async move {
///             tokio::select! {
///                 _ = shutdown_rx.recv() => { /* cleanup */ }
///             }
///         });
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait RunserverHook: Send + Sync {
	/// Validation phase: return `Err` to prevent server startup.
	///
	/// Runs before `on_server_start` and before DI context setup.
	async fn validate(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
		Ok(())
	}

	/// Startup phase: spawn concurrent services, register shutdown handlers.
	///
	/// Receives [`RunserverContext`] with access to [`ShutdownCoordinator`]
	/// and [`InjectionContext`].
	async fn on_server_start(
		&self,
		ctx: &RunserverContext,
	) -> Result<(), Box<dyn Error + Send + Sync>> {
		let _ = ctx;
		Ok(())
	}
}

/// Compile-time registration entry for auto-discovered runserver hooks.
///
/// Submitted via `inventory::submit!` by the `#[hook(on = runserver)]` macro.
/// You typically do not create this struct directly.
pub struct RunserverHookRegistration {
	/// Factory function that produces a boxed [`RunserverHook`].
	pub create: fn() -> Box<dyn RunserverHook>,

	/// Type name of the hook struct (for diagnostics).
	pub type_name: &'static str,
}

impl RunserverHookRegistration {
	/// Internal constructor used by the `#[hook]` macro.
	#[doc(hidden)]
	pub const fn __macro_new(
		create: fn() -> Box<dyn RunserverHook>,
		type_name: &'static str,
	) -> Self {
		Self { create, type_name }
	}
}

inventory::collect!(RunserverHookRegistration);

/// A collected runserver hook paired with its registration metadata.
pub(crate) struct CollectedRunserverHook {
	/// Instantiated hook implementation.
	pub hook: Box<dyn RunserverHook>,

	/// Type name of the hook struct (for diagnostics).
	pub type_name: &'static str,
}

/// Collect all registered runserver hooks from inventory, preserving
/// diagnostic identity for error reporting.
pub(crate) fn collect_hooks() -> Vec<CollectedRunserverHook> {
	inventory::iter::<RunserverHookRegistration>
		.into_iter()
		.map(|reg| CollectedRunserverHook {
			hook: (reg.create)(),
			type_name: reg.type_name,
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	// -- Test hook implementations --

	struct PassingValidateHook;

	#[async_trait]
	impl RunserverHook for PassingValidateHook {
		async fn validate(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
			Ok(())
		}
	}

	struct FailingValidateHook {
		message: &'static str,
	}

	#[async_trait]
	impl RunserverHook for FailingValidateHook {
		async fn validate(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
			Err(self.message.into())
		}
	}

	struct StartHookWithSideEffect {
		flag: Arc<std::sync::atomic::AtomicBool>,
	}

	#[async_trait]
	impl RunserverHook for StartHookWithSideEffect {
		async fn on_server_start(
			&self,
			_ctx: &RunserverContext,
		) -> Result<(), Box<dyn Error + Send + Sync>> {
			self.flag.store(true, std::sync::atomic::Ordering::SeqCst);
			Ok(())
		}
	}

	struct FailingStartHook;

	#[async_trait]
	impl RunserverHook for FailingStartHook {
		async fn on_server_start(
			&self,
			_ctx: &RunserverContext,
		) -> Result<(), Box<dyn Error + Send + Sync>> {
			Err("startup failed".into())
		}
	}

	struct NoOpHook;

	#[async_trait]
	impl RunserverHook for NoOpHook {}

	struct ValidateAndStartHook {
		validate_flag: Arc<std::sync::atomic::AtomicBool>,
		start_flag: Arc<std::sync::atomic::AtomicBool>,
	}

	#[async_trait]
	impl RunserverHook for ValidateAndStartHook {
		async fn validate(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
			self.validate_flag
				.store(true, std::sync::atomic::Ordering::SeqCst);
			Ok(())
		}

		async fn on_server_start(
			&self,
			_ctx: &RunserverContext,
		) -> Result<(), Box<dyn Error + Send + Sync>> {
			self.start_flag
				.store(true, std::sync::atomic::Ordering::SeqCst);
			Ok(())
		}
	}

	// -- Helper --

	fn make_runserver_context() -> RunserverContext {
		RunserverContext {
			shutdown_coordinator: ShutdownCoordinator::new(std::time::Duration::from_secs(1)),
			di_context: Arc::new(
				reinhardt_di::InjectionContext::builder(Arc::new(
					reinhardt_di::SingletonScope::new(),
				))
				.build(),
			),
		}
	}

	// ========================================================================
	// Normal cases
	// ========================================================================

	#[rstest]
	#[tokio::test]
	async fn validate_succeeds_for_passing_hook() {
		let hook = PassingValidateHook;
		assert!(hook.validate().await.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn on_server_start_sets_side_effect_flag() {
		// Arrange
		let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
		let hook = StartHookWithSideEffect { flag: flag.clone() };
		let ctx = make_runserver_context();

		// Act
		let result = hook.on_server_start(&ctx).await;

		// Assert
		assert!(result.is_ok());
		assert!(flag.load(std::sync::atomic::Ordering::SeqCst));
	}

	#[rstest]
	#[tokio::test]
	async fn hook_implementing_both_phases_calls_both() {
		// Arrange
		let validate_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
		let start_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
		let hook = ValidateAndStartHook {
			validate_flag: validate_flag.clone(),
			start_flag: start_flag.clone(),
		};
		let ctx = make_runserver_context();

		// Act
		hook.validate().await.unwrap();
		hook.on_server_start(&ctx).await.unwrap();

		// Assert
		assert!(validate_flag.load(std::sync::atomic::Ordering::SeqCst));
		assert!(start_flag.load(std::sync::atomic::Ordering::SeqCst));
	}

	#[rstest]
	#[tokio::test]
	async fn context_provides_shutdown_coordinator() {
		// Arrange
		let ctx = make_runserver_context();

		// Act: subscribe to shutdown coordinator (should not panic)
		let _rx = ctx.shutdown_coordinator.subscribe();

		// Assert: subscription succeeded (no panic)
	}

	#[rstest]
	#[tokio::test]
	async fn context_provides_di_context() {
		// Arrange
		let ctx = make_runserver_context();

		// Assert: DI context is accessible
		let _ = ctx.di_context.clone();
	}

	// ========================================================================
	// Error / failure cases
	// ========================================================================

	#[rstest]
	#[tokio::test]
	async fn validate_returns_error_with_message() {
		// Arrange
		let hook = FailingValidateHook {
			message: "JWT_SECRET not configured",
		};

		// Act
		let result = hook.validate().await;

		// Assert
		assert!(result.is_err());
		assert_eq!(result.unwrap_err().to_string(), "JWT_SECRET not configured");
	}

	#[rstest]
	#[tokio::test]
	async fn on_server_start_returns_error() {
		// Arrange
		let hook = FailingStartHook;
		let ctx = make_runserver_context();

		// Act
		let result = hook.on_server_start(&ctx).await;

		// Assert
		assert!(result.is_err());
		assert_eq!(result.unwrap_err().to_string(), "startup failed");
	}

	// ========================================================================
	// Edge cases / default implementations
	// ========================================================================

	#[rstest]
	#[tokio::test]
	async fn default_validate_returns_ok() {
		let hook = NoOpHook;
		assert!(hook.validate().await.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn default_on_server_start_returns_ok() {
		// Arrange
		let hook = NoOpHook;
		let ctx = make_runserver_context();

		// Act & Assert
		assert!(hook.on_server_start(&ctx).await.is_ok());
	}

	#[rstest]
	fn collect_hooks_returns_vec_without_panic() {
		// Act: collect hooks from inventory
		let hooks = collect_hooks();

		// Assert: returns a Vec of CollectedRunserverHook
		for collected in &hooks {
			// type_name is preserved from registration
			assert!(!collected.type_name.is_empty());
		}
	}

	#[rstest]
	fn registration_macro_new_creates_valid_entry() {
		// Arrange & Act
		let reg = RunserverHookRegistration::__macro_new(|| Box::new(NoOpHook), "NoOpHook");

		// Assert
		assert_eq!(reg.type_name, "NoOpHook");
		let hook = (reg.create)();
		// Verify the created hook is usable (trait object is valid)
		let _ = hook;
	}

	#[rstest]
	#[tokio::test]
	async fn registration_factory_produces_working_hook() {
		// Arrange
		let reg = RunserverHookRegistration::__macro_new(
			|| Box::new(PassingValidateHook),
			"PassingValidateHook",
		);

		// Act
		let hook = (reg.create)();
		let result = hook.validate().await;

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	#[tokio::test]
	async fn multiple_hooks_sequential_validation() {
		// Arrange
		let hooks: Vec<Box<dyn RunserverHook>> = vec![
			Box::new(PassingValidateHook),
			Box::new(PassingValidateHook),
			Box::new(PassingValidateHook),
		];

		// Act & Assert: all validations pass
		for hook in &hooks {
			assert!(hook.validate().await.is_ok());
		}
	}

	#[rstest]
	#[tokio::test]
	async fn validation_stops_on_first_error() {
		// Arrange
		let hooks: Vec<Box<dyn RunserverHook>> = vec![
			Box::new(PassingValidateHook),
			Box::new(FailingValidateHook {
				message: "second hook fails",
			}),
			Box::new(PassingValidateHook),
		];

		// Act: simulate the run_server validation loop
		let mut first_error = None;
		for hook in &hooks {
			if let Err(e) = hook.validate().await {
				first_error = Some(e);
				break;
			}
		}

		// Assert: stopped at second hook
		assert!(first_error.is_some());
		assert_eq!(first_error.unwrap().to_string(), "second hook fails");
	}

	#[rstest]
	#[tokio::test]
	async fn on_server_start_not_called_when_validate_fails() {
		// Arrange
		let start_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
		let hook = FailingValidateHook {
			message: "config error",
		};
		let ctx = make_runserver_context();

		// Act: simulate run_server flow — validate first, skip on_server_start on error
		let validate_result = hook.validate().await;
		if validate_result.is_ok() {
			// This simulates the NoOpHook case for on_server_start
			let _ = NoOpHook.on_server_start(&ctx).await;
			start_flag.store(true, std::sync::atomic::Ordering::SeqCst);
		}

		// Assert: on_server_start was never called
		assert!(validate_result.is_err());
		assert!(!start_flag.load(std::sync::atomic::Ordering::SeqCst));
	}
}
