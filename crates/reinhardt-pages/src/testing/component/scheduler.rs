//! Native async scheduler for component tests.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use crate::platform;

type BoxedTask = Pin<Box<dyn Future<Output = ()> + 'static>>;
const SETTLE_BACKOFF_AFTER_YIELDS: usize = 4;
const SETTLE_BACKOFF: Duration = Duration::from_millis(1);

/// Error returned when the native component scheduler cannot settle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettleError {
	/// Pending work did not quiesce before the harness limit.
	DidNotQuiesce {
		/// Number of settle iterations attempted.
		iterations: usize,
		/// Number of tasks still pending after the last iteration.
		pending_tasks: usize,
		/// Pretty DOM output captured when settle failed.
		dom: String,
	},
}

impl std::fmt::Display for SettleError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::DidNotQuiesce {
				iterations,
				pending_tasks,
				dom,
			} => write!(
				f,
				"scheduled work did not quiesce after {iterations} iterations with {pending_tasks} pending tasks\n\n{dom}"
			),
		}
	}
}

impl std::error::Error for SettleError {}

#[derive(Default)]
pub(crate) struct TestScheduler {
	tasks: VecDeque<BoxedTask>,
}

pub(crate) struct SchedulerScope {
	scheduler: Rc<RefCell<TestScheduler>>,
}

impl SchedulerScope {
	pub(crate) fn new() -> Self {
		let scheduler = Rc::new(RefCell::new(TestScheduler::default()));
		Self { scheduler }
	}

	pub(crate) fn with_current<R>(&self, f: impl FnOnce() -> R) -> R {
		let scheduler = Rc::clone(&self.scheduler);
		let _guard = platform::install_task_sink(move |task| {
			scheduler.borrow_mut().tasks.push_back(task);
		});
		f()
	}

	pub(crate) async fn settle_with_context(
		&self,
		dom: impl Fn() -> String,
		mut with_context: impl FnMut(&mut dyn FnMut() -> usize) -> usize,
	) -> Result<(), SettleError> {
		let mut cx = Context::from_waker(Waker::noop());
		for iteration in 0..100 {
			let mut poll_tasks = || {
				self.with_current(|| {
					let mut pending = VecDeque::new();
					loop {
						let Some(mut task) = self.scheduler.borrow_mut().tasks.pop_front() else {
							break;
						};
						match task.as_mut().poll(&mut cx) {
							Poll::Ready(()) => {}
							Poll::Pending => pending.push_back(task),
						}
					}
					let mut scheduler = self.scheduler.borrow_mut();
					pending.extend(std::mem::take(&mut scheduler.tasks));
					let pending_tasks = pending.len();
					scheduler.tasks = pending;
					pending_tasks
				})
			};
			let pending_tasks = with_context(&mut poll_tasks);
			if pending_tasks == 0 {
				return Ok(());
			}
			if iteration < SETTLE_BACKOFF_AFTER_YIELDS {
				tokio::task::yield_now().await;
			} else {
				tokio::time::sleep(SETTLE_BACKOFF).await;
			}
			if iteration == 99 {
				return Err(SettleError::DidNotQuiesce {
					iterations: 100,
					pending_tasks,
					dom: dom(),
				});
			}
		}
		unreachable!("settle loop returns inside fixed iteration range")
	}

	pub(crate) fn pending_task_count(&self) -> usize {
		self.scheduler.borrow().tasks.len()
	}
}
