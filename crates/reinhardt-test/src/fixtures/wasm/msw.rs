//! rstest fixtures for `MockServiceWorker`.

use rstest::fixture;

use crate::msw::{MockServiceWorker, UnhandledPolicy};

/// Provides a `MockServiceWorker` with `UnhandledPolicy::Error` (default).
/// Auto-started. Automatically stops on drop.
#[fixture]
pub async fn msw_worker() -> MockServiceWorker {
	let worker = MockServiceWorker::new();
	worker.start().await;
	worker
}

/// Provides a `MockServiceWorker` with `UnhandledPolicy::Passthrough`.
/// Auto-started. Automatically stops on drop.
#[fixture]
pub async fn msw_worker_passthrough() -> MockServiceWorker {
	let worker = MockServiceWorker::with_policy(UnhandledPolicy::Passthrough);
	worker.start().await;
	worker
}
