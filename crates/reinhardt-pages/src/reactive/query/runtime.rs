#[cfg(all(test, not(wasm)))]
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
#[cfg(not(wasm))]
use std::time::{SystemTime, UNIX_EPOCH};

use reinhardt_core::reactive::{ScopeId, scope::enter_scope};

/// Polls cached query work in the scope that owns the query entry.
///
/// Cache entries outlive the component that first requested them, so their
/// fetchers cannot rely on that component's render scope remaining active.
pub(super) struct ScopedQueryFuture<Fut> {
	pub(super) scope: ScopeId,
	pub(super) future: Pin<Box<Fut>>,
}

impl<Fut> Future for ScopedQueryFuture<Fut>
where
	Fut: Future<Output = ()>,
{
	type Output = ();

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.get_mut();
		let poll = || this.future.as_mut().poll(cx);
		enter_scope(this.scope, poll).unwrap_or(Poll::Ready(()))
	}
}

#[cfg(all(test, not(wasm)))]
thread_local! {
	static INLINE_QUERY_TASK_DEPTH: Cell<usize> = const { Cell::new(0) };
	static DEFERRED_QUERY_TASKS: RefCell<std::collections::VecDeque<Pin<Box<dyn Future<Output = ()> + 'static>>>> =
		const { RefCell::new(std::collections::VecDeque::new()) };
}

#[cfg(all(test, not(wasm)))]
struct InlineQueryTaskGuard;

#[cfg(all(test, not(wasm)))]
impl InlineQueryTaskGuard {
	fn new() -> Option<Self> {
		INLINE_QUERY_TASK_DEPTH.with(|depth| {
			let current = depth.get();
			if current == 0 {
				depth.set(1);
				Some(Self)
			} else {
				None
			}
		})
	}
}

#[cfg(all(test, not(wasm)))]
impl Drop for InlineQueryTaskGuard {
	fn drop(&mut self) {
		INLINE_QUERY_TASK_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
	}
}

#[cfg(all(test, not(wasm)))]
pub(super) fn spawn_query_task<F>(fut: F)
where
	F: Future<Output = ()> + 'static,
{
	if crate::platform::has_task_sink() {
		schedule_query_task(fut);
	} else {
		let Some(_guard) = InlineQueryTaskGuard::new() else {
			DEFERRED_QUERY_TASKS.with(|tasks| tasks.borrow_mut().push_back(Box::pin(fut)));
			return;
		};
		tokio_test::block_on(async move {
			fut.await;
			loop {
				let task = DEFERRED_QUERY_TASKS.with(|tasks| tasks.borrow_mut().pop_front());
				let Some(task) = task else {
					break;
				};
				task.await;
			}
		});
	}
}

#[cfg(any(not(test), wasm))]
pub(super) fn spawn_query_task<F>(fut: F)
where
	F: Future<Output = ()> + 'static,
{
	schedule_query_task(fut);
}

fn schedule_query_task<F>(fut: F)
where
	F: Future<Output = ()> + 'static,
{
	crate::platform::spawn_task_unscoped(async move {
		crate::platform::defer_yield().await;
		fut.await;
	});
}

pub(super) fn duration_ms(duration: Duration) -> u64 {
	duration.as_millis().min(u128::from(u64::MAX)) as u64
}

#[cfg(not(wasm))]
pub(super) fn now_ms() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.map(duration_ms)
		.unwrap_or_default()
}

#[cfg(wasm)]
pub(super) fn now_ms() -> u64 {
	js_sys::Date::now() as u64
}
