use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use reinhardt_core::reactive::ReactiveScope;
use rstest::rstest;
use serde::Serializer;
use serde::ser::{Error as _, SerializeMap};
use serde::{Deserialize, Serialize};
use serial_test::serial;

use crate::reactive::entity::{
	Entity, EntityDependencies, EntityProjection, EntityReader, EntityValue, EntityWriter,
	ProjectionMaterialization, ProjectionRemoval, RemovedEntities,
};

use super::super::resource::ResourceState;
use super::client::{
	ObserverPolicy, QueryClient, QueryEntry, TestQueryRuntime, acquire_query_with_options,
	initial_query_state, query_entry,
};
use super::runtime::now_ms;
use super::*;

struct OrderedMapArgs(&'static [(&'static str, i64)]);

impl Serialize for OrderedMapArgs {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let mut map = serializer.serialize_map(Some(self.0.len()))?;
		for (key, value) in self.0 {
			map.serialize_entry(key, value)?;
		}
		map.end()
	}
}

struct OrderedLargeMapArgs(&'static [(&'static str, u128)]);

impl Serialize for OrderedLargeMapArgs {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		let mut map = serializer.serialize_map(Some(self.0.len()))?;
		for (key, value) in self.0 {
			map.serialize_entry(key, value)?;
		}
		map.end()
	}
}

struct FailingFingerprintArgs;

#[test]
fn query_retry_jitter_samples_are_consumed_in_configured_order() {
	let runtime = TestQueryRuntime::with_jitter_samples([7, 11]);
	let runtime_handle = runtime.handle();

	assert_eq!(runtime_handle.jitter_sample(), 7);
	assert_eq!(runtime_handle.jitter_sample(), 11);
}

fn isolated_query_client() -> QueryClientGuard {
	provide_query_client(QueryClient::new(QueryDefaults::default()))
}

impl Serialize for FailingFingerprintArgs {
	fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		Err(S::Error::custom("fingerprint failed"))
	}
}

type TestTask = Pin<Box<dyn Future<Output = ()> + 'static>>;

fn poll_one_task(tasks: &Rc<RefCell<VecDeque<TestTask>>>) -> Poll<()> {
	let mut task = tasks
		.borrow_mut()
		.pop_front()
		.expect("a query request should schedule one task");
	let mut context = Context::from_waker(Waker::noop());
	let result = task.as_mut().poll(&mut context);
	if result.is_pending() {
		tasks.borrow_mut().push_back(task);
	}
	result
}

struct TestGate {
	ready: Rc<Cell<bool>>,
	dropped: Rc<Cell<usize>>,
	result: Option<Result<String, String>>,
}

impl Future for TestGate {
	type Output = Result<String, String>;

	fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.get_mut();
		if this.ready.get() {
			Poll::Ready(
				this.result
					.take()
					.expect("test gate polled after completion"),
			)
		} else {
			Poll::Pending
		}
	}
}

impl Drop for TestGate {
	fn drop(&mut self) {
		self.dropped.set(self.dropped.get() + 1);
	}
}

#[test]
fn query_snapshot_distinguishes_disabled_pending_and_resolved_state() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<i64, String, String>::new("tests.snapshot-state");
	let disabled = client.observe(
		family.query(0, || async { Ok("disabled".to_string()) }),
		QueryOptions::new().enabled(false),
	);
	let initial = client.observe(
		family.query(1, || std::future::pending::<Result<String, String>>()),
		QueryOptions::default(),
	);
	let resolved = client.observe(
		family.query(2, || async { Ok("cached".to_string()) }),
		QueryOptions::default(),
	);

	assert_eq!(disabled.snapshot().status, QueryStatus::Idle);
	assert!(!disabled.snapshot().is_fetching);
	assert_eq!(initial.snapshot().status, QueryStatus::Pending);
	assert!(initial.snapshot().is_fetching);

	runtime.run_until_stalled();

	assert_eq!(resolved.data(), Some("cached".to_string()));
	assert_eq!(resolved.snapshot().status, QueryStatus::Success);
}

#[test]
fn disabled_observer_does_not_join_an_enabled_observers_initial_fetch() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.disabled-shared-pending");
	let descriptor = family.query((), || std::future::pending::<Result<String, String>>());
	let enabled = client.observe(descriptor.clone(), QueryOptions::default());
	let disabled = client.observe(descriptor, QueryOptions::new().enabled(false));

	assert_eq!(enabled.snapshot().status, QueryStatus::Pending);
	assert!(enabled.snapshot().is_fetching);
	assert_eq!(disabled.snapshot().status, QueryStatus::Idle);
	assert!(!disabled.snapshot().is_fetching);
}

#[test]
fn initial_failure_populates_query_error() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.initial-error");
	let query = client.observe(
		family.query((), || async { Err("offline".to_string()) }),
		QueryOptions::default(),
	);

	runtime.run_until_stalled();

	assert_eq!(
		query.snapshot(),
		QuerySnapshot {
			status: QueryStatus::Error,
			data: None,
			error: Some("offline".to_string()),
			refetch_error: None,
			is_fetching: false,
			is_stale: false,
		}
	);
}

#[test]
fn query_retry_single_observer_waits_for_exponential_deadlines_and_publishes_only_terminal_error() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-single-observer-deadlines");
	let fetch_count = Rc::new(Cell::new(0));
	let query = client.observe(
		family.query((), {
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				let attempt = fetch_count.get() + 1;
				fetch_count.set(attempt);
				async move { Err(format!("attempt-{attempt}")) }
			}
		}),
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(3)
				.base_delay(Duration::from_millis(100))
				.max_delay(Duration::from_secs(1)),
		),
	);

	assert_eq!(query.snapshot().status, QueryStatus::Pending);
	assert!(query.is_fetching());
	runtime.run_until_stalled();

	assert_eq!(fetch_count.get(), 1);
	assert_eq!(query.snapshot().status, QueryStatus::Pending);
	assert_eq!(query.error(), None);
	assert!(!query.is_fetching());
	assert!(query.entry.completed.borrow().is_none());

	runtime.advance(Duration::from_millis(99));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 1);

	runtime.advance(Duration::from_millis(1));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 2);
	assert_eq!(query.snapshot().status, QueryStatus::Pending);
	assert_eq!(query.error(), None);
	assert!(!query.is_fetching());
	assert!(query.entry.completed.borrow().is_none());

	runtime.advance(Duration::from_millis(199));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 2);

	runtime.advance(Duration::from_millis(1));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 3);
	assert_eq!(query.error(), Some("attempt-3".to_string()));
	assert!(!query.is_fetching());
	assert_eq!(query.entry.last_fetched_ms.get(), Some(300));
	assert!(matches!(
		query.entry.completed.borrow().as_ref(),
		Some((_, Err(error))) if error == "attempt-3"
	));
}

#[test]
fn query_retry_single_observer_honors_total_attempt_limits() {
	for (family_id, max_attempts) in [
		("tests.retry-total-one", 1),
		("tests.retry-total-two", 2),
		("tests.retry-total-three", 3),
	] {
		let runtime = TestQueryRuntime::new();
		let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
		let family = QueryFamily::<(), String, String>::new(family_id);
		let fetch_count = Rc::new(Cell::new(0));
		let query = client.observe(
			family.query((), {
				let fetch_count = Rc::clone(&fetch_count);
				move || {
					let attempt = fetch_count.get() + 1;
					fetch_count.set(attempt);
					async move { Err(format!("attempt-{attempt}")) }
				}
			}),
			QueryOptions::new().retry(
				RetryPolicy::exponential()
					.max_attempts(max_attempts)
					.base_delay(Duration::ZERO)
					.max_delay(Duration::ZERO),
			),
		);

		runtime.run_until_stalled();
		for _ in 1..max_attempts {
			runtime.run_due_maintenance();
		}

		assert_eq!(fetch_count.get(), max_attempts);
		assert_eq!(query.error(), Some(format!("attempt-{max_attempts}")));
	}
}

#[test]
fn query_retry_success_clears_the_sequence_without_publishing_the_initial_error() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-success");
	let fetch_count = Rc::new(Cell::new(0));
	let query = client.observe(
		family.query((), {
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				let attempt = fetch_count.get() + 1;
				fetch_count.set(attempt);
				async move {
					if attempt == 1 {
						Err("temporary".to_string())
					} else {
						Ok("recovered".to_string())
					}
				}
			}
		}),
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.base_delay(Duration::from_millis(100))
				.max_delay(Duration::from_millis(100)),
		),
	);

	runtime.run_until_stalled();
	assert_eq!(query.snapshot().status, QueryStatus::Pending);
	assert_eq!(query.error(), None);
	assert!(query.entry.completed.borrow().is_none());

	runtime.advance(Duration::from_millis(100));
	runtime.run_due_maintenance();

	assert_eq!(fetch_count.get(), 2);
	assert_eq!(query.data(), Some("recovered".to_string()));
	assert_eq!(query.error(), None);
	assert!(query.entry.retry.borrow().is_none());
	assert!(matches!(
		query.entry.completed.borrow().as_ref(),
		Some((_, Ok(value))) if value == "recovered"
	));
}

#[test]
fn native_runtime_drives_retry_without_browser_maintenance() {
	let runtime = TestQueryRuntime::without_external_maintenance();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-native-runtime");
	let fetch_count = Rc::new(Cell::new(0));
	let query = client.observe(
		family.query((), {
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				let attempt = fetch_count.get() + 1;
				fetch_count.set(attempt);
				async move {
					if attempt == 1 {
						Err("temporary".to_string())
					} else {
						Ok("recovered".to_string())
					}
				}
			}
		}),
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.base_delay(Duration::from_millis(10))
				.max_delay(Duration::from_millis(10)),
		),
	);

	runtime.run_until_stalled();
	runtime.advance(Duration::from_millis(10));
	runtime.run_until_stalled();

	assert_eq!(fetch_count.get(), 2);
	assert_eq!(query.data(), Some("recovered".to_string()));
}

#[test]
fn dropping_the_final_native_observer_aborts_its_retry_wait() {
	let runtime = TestQueryRuntime::without_external_maintenance();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-native-wait-abort");
	let query = client.observe(
		family.query((), || async { Err("temporary".to_string()) }),
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.base_delay(Duration::from_secs(60))
				.max_delay(Duration::from_secs(60)),
		),
	);
	runtime.run_until_stalled();
	assert_eq!(runtime.pending_task_count(), 1);

	drop(query);
	runtime.run_until_stalled();

	assert_eq!(runtime.pending_task_count(), 0);
}

#[test]
fn acquiring_during_background_retry_waits_for_shared_completion() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-backoff-acquisition");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let attempt = fetch_count.get() + 1;
			fetch_count.set(attempt);
			async move {
				match attempt {
					1 => Ok("cached".to_string()),
					2 => Err("temporary".to_string()),
					_ => Ok("fresh".to_string()),
				}
			}
		}
	});
	let key = descriptor.key().clone();
	let mounted = client.observe(
		descriptor.clone(),
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.base_delay(Duration::from_millis(10))
				.max_delay(Duration::from_millis(10)),
		),
	);
	runtime.run_until_stalled();
	client.invalidate(&key);
	runtime.run_until_stalled();

	let lease = client.acquire(
		descriptor,
		QueryAcquireOptions {
			consumer: QueryConsumer::Navigation(1),
			error_policy: QueryErrorPolicy::Discard,
		},
	);
	let mut result = Box::pin(lease.result());
	let mut context = Context::from_waker(Waker::noop());

	assert_eq!(mounted.data(), Some("cached".to_string()));
	assert_eq!(result.as_mut().poll(&mut context), Poll::Pending);
	runtime.advance(Duration::from_millis(10));
	runtime.run_due_maintenance();
	assert_eq!(
		result.as_mut().poll(&mut context),
		Poll::Ready(Ok("fresh".to_string()))
	);
}

#[test]
fn query_retry_single_observer_predicate_rejection_is_terminal() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-predicate-rejection");
	let fetch_count = Rc::new(Cell::new(0));
	let query = client.observe(
		family.query((), {
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				fetch_count.set(fetch_count.get() + 1);
				async { Err("fatal".to_string()) }
			}
		}),
		QueryOptions::new().retry(RetryPolicy::exponential().when(|error| error != "fatal")),
	);

	runtime.run_until_stalled();
	runtime.advance(Duration::from_secs(10));
	runtime.run_due_maintenance();

	assert_eq!(fetch_count.get(), 1);
	assert_eq!(query.error(), Some("fatal".to_string()));
}

#[test]
fn query_retry_predicate_can_invalidate_the_same_key() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-predicate-invalidation");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let attempt = fetch_count.get() + 1;
			fetch_count.set(attempt);
			async move {
				if attempt == 1 {
					Err("stale".to_string())
				} else {
					Ok("fresh".to_string())
				}
			}
		}
	});
	let key = descriptor.key().clone();
	let client_for_predicate = client.clone();
	let query = client.observe(
		descriptor,
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::ZERO)
				.max_delay(Duration::ZERO)
				.when(move |_| {
					client_for_predicate.invalidate(&key);
					true
				}),
		),
	);

	runtime.run_until_stalled();

	assert_eq!(fetch_count.get(), 2);
	assert_eq!(query.data(), Some("fresh".to_string()));
	assert_eq!(query.error(), None);
}

#[test]
fn query_retry_background_keeps_data_and_timestamp_until_terminal_failure() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-background");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let attempt = fetch_count.get() + 1;
			fetch_count.set(attempt);
			async move {
				if attempt == 1 {
					Ok("cached".to_string())
				} else {
					Err(format!("attempt-{attempt}"))
				}
			}
		}
	});
	let key = descriptor.key().clone();
	let query = client.observe(
		descriptor,
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_millis(100))
				.max_delay(Duration::from_millis(100)),
		),
	);
	runtime.run_until_stalled();
	assert_eq!(query.entry.last_fetched_ms.get(), Some(0));

	client.invalidate(&key);
	runtime.run_until_stalled();

	assert_eq!(fetch_count.get(), 2);
	assert_eq!(query.data(), Some("cached".to_string()));
	assert_eq!(query.refetch_error(), None);
	assert!(!query.is_fetching());
	assert_eq!(query.entry.last_fetched_ms.get(), Some(0));

	runtime.advance(Duration::from_millis(100));
	runtime.run_due_maintenance();

	assert_eq!(fetch_count.get(), 3);
	assert_eq!(query.data(), Some("cached".to_string()));
	assert_eq!(query.refetch_error(), Some("attempt-3".to_string()));
	assert_eq!(query.entry.last_fetched_ms.get(), Some(0));
}

#[test]
fn query_retry_default_policy_retries_every_error_three_total_times() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-default-all-errors");
	let fetch_count = Rc::new(Cell::new(0));
	let query = client.observe(
		family.query((), {
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				let attempt = fetch_count.get() + 1;
				fetch_count.set(attempt);
				async move { Err(format!("attempt-{attempt}")) }
			}
		}),
		QueryOptions::new().retry(RetryPolicy::exponential()),
	);

	runtime.run_until_stalled();
	runtime.advance(Duration::from_millis(250));
	runtime.run_due_maintenance();
	runtime.advance(Duration::from_millis(500));
	runtime.run_due_maintenance();

	assert_eq!(fetch_count.get(), 3);
	assert_eq!(query.error(), Some("attempt-3".to_string()));
}

#[test]
fn query_retry_earliest_observer_policy_selects_the_shortest_deadline() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-earliest-policy");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let attempt = fetch_count.get() + 1;
			fetch_count.set(attempt);
			async move { Err(format!("attempt-{attempt}")) }
		}
	});
	let faster = client.observe(
		descriptor.clone(),
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_millis(100))
				.max_delay(Duration::from_millis(100)),
		),
	);
	let slower = client.observe(
		descriptor,
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_millis(500))
				.max_delay(Duration::from_millis(500)),
		),
	);

	runtime.run_until_stalled();
	assert_eq!(fetch_count.get(), 1);
	assert_eq!(slower.snapshot().status, QueryStatus::Pending);
	assert_eq!(faster.error(), None);
	assert_eq!(runtime.pending_task_count(), 0);

	runtime.advance(Duration::from_millis(99));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 1);
	assert_eq!(runtime.pending_task_count(), 0);

	runtime.advance(Duration::from_millis(1));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 2);
	assert_eq!(slower.error(), Some("attempt-2".to_string()));
	assert_eq!(faster.error(), Some("attempt-2".to_string()));
	assert_eq!(runtime.pending_task_count(), 0);
}

#[test]
fn query_retry_observers_share_the_largest_total_attempt_limit() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-observer-attempt-limit");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let attempt = fetch_count.get() + 1;
			fetch_count.set(attempt);
			async move { Err(format!("attempt-{attempt}")) }
		}
	});
	let two_attempts = client.observe(
		descriptor.clone(),
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::ZERO)
				.max_delay(Duration::ZERO),
		),
	);
	let four_attempts = client.observe(
		descriptor,
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(4)
				.base_delay(Duration::ZERO)
				.max_delay(Duration::ZERO),
		),
	);

	runtime.run_until_stalled();
	assert_eq!(fetch_count.get(), 1);
	for expected_attempts in 2..=4 {
		runtime.run_due_maintenance();
		assert_eq!(fetch_count.get(), expected_attempts);
	}

	assert_eq!(two_attempts.error(), Some("attempt-4".to_string()));
	assert_eq!(four_attempts.error(), Some("attempt-4".to_string()));
	assert_eq!(runtime.pending_task_count(), 0);
}

#[test]
fn query_retry_observer_acceptance_overrides_another_observers_rejection() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-observer-predicates");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let attempt = fetch_count.get() + 1;
			fetch_count.set(attempt);
			async move { Err(format!("transient-{attempt}")) }
		}
	});
	let rejecting = client.observe(
		descriptor.clone(),
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::ZERO)
				.max_delay(Duration::ZERO)
				.when(|_| false),
		),
	);
	let accepting = client.observe(
		descriptor,
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::ZERO)
				.max_delay(Duration::ZERO)
				.when(|error: &String| error.starts_with("transient-")),
		),
	);

	runtime.run_until_stalled();
	assert_eq!(fetch_count.get(), 1);
	assert_eq!(rejecting.error(), None);
	assert_eq!(accepting.snapshot().status, QueryStatus::Pending);

	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 2);
	assert_eq!(rejecting.error(), Some("transient-2".to_string()));
	assert_eq!(accepting.error(), Some("transient-2".to_string()));
	assert_eq!(runtime.pending_task_count(), 0);
}

#[test]
fn query_retry_earliest_deadline_moves_earlier_when_a_shorter_policy_mounts() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-earlier-policy-mount");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let attempt = fetch_count.get() + 1;
			fetch_count.set(attempt);
			async move { Err(format!("attempt-{attempt}")) }
		}
	});
	let slower = client.observe(
		descriptor.clone(),
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_millis(500))
				.max_delay(Duration::from_millis(500)),
		),
	);
	runtime.run_until_stalled();
	runtime.advance(Duration::from_millis(100));

	let faster = client.observe(
		descriptor,
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_millis(200))
				.max_delay(Duration::from_millis(200)),
		),
	);
	assert_eq!(fetch_count.get(), 1);
	assert_eq!(slower.error(), None);
	assert_eq!(faster.snapshot().status, QueryStatus::Pending);
	assert_eq!(runtime.pending_task_count(), 0);

	runtime.advance(Duration::from_millis(99));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 1);

	runtime.advance(Duration::from_millis(1));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 2);
	assert_eq!(faster.error(), Some("attempt-2".to_string()));
	assert_eq!(runtime.pending_task_count(), 0);
}

#[test]
fn query_retry_earliest_deadline_moves_later_when_the_shorter_policy_drops() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-later-policy-drop");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let attempt = fetch_count.get() + 1;
			fetch_count.set(attempt);
			async move { Err(format!("attempt-{attempt}")) }
		}
	});
	let faster = client.observe(
		descriptor.clone(),
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_millis(100))
				.max_delay(Duration::from_millis(100)),
		),
	);
	let slower = client.observe(
		descriptor,
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_millis(500))
				.max_delay(Duration::from_millis(500)),
		),
	);
	runtime.run_until_stalled();
	runtime.advance(Duration::from_millis(50));
	let stale_deadline_generation = faster
		.entry
		.retry
		.borrow()
		.as_ref()
		.expect("the failed query must be waiting to retry")
		.generation;

	drop(faster);
	assert_ne!(
		slower
			.entry
			.retry
			.borrow()
			.as_ref()
			.expect("the slower retry policy must remain eligible")
			.generation,
		stale_deadline_generation,
		"removing the shortest policy must invalidate its queued deadline"
	);
	runtime.advance(Duration::from_millis(50));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 1);
	assert_eq!(slower.snapshot().status, QueryStatus::Pending);
	assert_eq!(runtime.pending_task_count(), 0);

	runtime.advance(Duration::from_millis(400));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 2);
	assert_eq!(slower.error(), Some("attempt-2".to_string()));
	assert_eq!(runtime.pending_task_count(), 0);
}

#[test]
fn query_retry_observer_drop_publishes_failure_when_a_non_retry_lease_remains() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-policy-drop-publishes");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			fetch_count.set(fetch_count.get() + 1);
			async { Err("offline".to_string()) }
		}
	});
	let retrying = client.observe(
		descriptor.clone(),
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_secs(1))
				.max_delay(Duration::from_secs(1)),
		),
	);
	let remaining = client.observe(descriptor, QueryOptions::default());
	runtime.run_until_stalled();
	assert_eq!(remaining.snapshot().status, QueryStatus::Pending);
	assert_eq!(remaining.error(), None);

	drop(retrying);

	assert_eq!(fetch_count.get(), 1);
	assert_eq!(remaining.snapshot().status, QueryStatus::Error);
	assert_eq!(remaining.error(), Some("offline".to_string()));
	assert_eq!(runtime.pending_task_count(), 0);
}

#[test]
fn query_retry_observer_final_drop_discards_the_waiting_failure() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-final-drop-discards");
	let fetch_count = Rc::new(Cell::new(0));
	let query = client.observe(
		family.query((), {
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				fetch_count.set(fetch_count.get() + 1);
				async { Err("offline".to_string()) }
			}
		}),
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_secs(1))
				.max_delay(Duration::from_secs(1)),
		),
	);
	runtime.run_until_stalled();
	let entry = Rc::clone(&query.entry);
	assert_eq!(query.snapshot().status, QueryStatus::Pending);
	assert!(entry.completed.borrow().is_none());

	drop(query);

	assert_eq!(fetch_count.get(), 1);
	assert_eq!(
		entry.state.with_untracked(|state| state.clone()),
		ResourceState::Loading
	);
	assert!(entry.completed.borrow().is_none());
	assert!(entry.retry.borrow().is_none());
	assert_eq!(runtime.pending_task_count(), 0);
}

#[test]
fn query_retry_disabled_observer_participates_only_in_its_manual_sequence() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-disabled-manual-only");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let attempt = fetch_count.get() + 1;
			fetch_count.set(attempt);
			async move { Err(format!("attempt-{attempt}")) }
		}
	});
	let enabled = client.observe(descriptor.clone(), QueryOptions::default());
	let disabled = client.observe(
		descriptor,
		QueryOptions::new().enabled(false).retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::ZERO)
				.max_delay(Duration::ZERO),
		),
	);

	runtime.run_until_stalled();
	assert_eq!(fetch_count.get(), 1);
	assert_eq!(enabled.error(), Some("attempt-1".to_string()));
	assert_eq!(disabled.snapshot().status, QueryStatus::Error);

	disabled.refetch();
	runtime.run_until_stalled();
	assert_eq!(fetch_count.get(), 2);
	assert_eq!(disabled.snapshot().status, QueryStatus::Pending);
	assert_eq!(disabled.error(), None);
	runtime.run_due_maintenance();

	assert_eq!(fetch_count.get(), 3);
	assert_eq!(enabled.error(), Some("attempt-3".to_string()));
	assert_eq!(disabled.error(), Some("attempt-3".to_string()));
	assert_eq!(runtime.pending_task_count(), 0);
}

#[test]
fn query_retry_observer_changes_reuse_the_stored_jitter_sample() {
	let runtime = TestQueryRuntime::with_jitter_samples([0]);
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-observer-stable-jitter");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let attempt = fetch_count.get() + 1;
			fetch_count.set(attempt);
			async move { Err(format!("attempt-{attempt}")) }
		}
	});
	let first = client.observe(
		descriptor.clone(),
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_millis(100))
				.max_delay(Duration::from_millis(100))
				.jitter(true),
		),
	);
	runtime.run_until_stalled();
	assert_eq!(
		first.entry.retry.borrow().as_ref().unwrap().jitter_sample,
		Some(0)
	);
	assert_eq!(
		first
			.entry
			.retry
			.borrow()
			.as_ref()
			.unwrap()
			.candidates
			.len(),
		1
	);

	let second = client.observe(
		descriptor,
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_millis(40))
				.max_delay(Duration::from_millis(40))
				.jitter(true),
		),
	);
	assert_eq!(
		first.entry.retry.borrow().as_ref().unwrap().jitter_sample,
		Some(0)
	);
	assert_eq!(
		first
			.entry
			.retry
			.borrow()
			.as_ref()
			.unwrap()
			.candidates
			.len(),
		2
	);

	drop(second);
	assert_eq!(
		first.entry.retry.borrow().as_ref().unwrap().jitter_sample,
		Some(0)
	);
	assert_eq!(
		first
			.entry
			.retry
			.borrow()
			.as_ref()
			.unwrap()
			.candidates
			.len(),
		1
	);
	runtime.advance(Duration::from_millis(49));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 1);
	runtime.advance(Duration::from_millis(1));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 2);
	assert_eq!(first.error(), Some("attempt-2".to_string()));
	assert_eq!(runtime.pending_task_count(), 0);
}

#[test]
fn query_retry_observer_predicate_panic_propagates_unchanged() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-observer-predicate-panic");
	let fetch_count = Rc::new(Cell::new(0));
	let _query = client.observe(
		family.query((), {
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				fetch_count.set(fetch_count.get() + 1);
				async { Err("offline".to_string()) }
			}
		}),
		QueryOptions::new()
			.retry(RetryPolicy::exponential().when(|_: &String| panic!("retry predicate panic"))),
	);

	let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
		runtime.run_until_stalled();
	}))
	.expect_err("the retry predicate panic must escape query completion");
	let message = panic
		.downcast_ref::<&str>()
		.copied()
		.or_else(|| panic.downcast_ref::<String>().map(String::as_str));

	assert_eq!(message, Some("retry predicate panic"));
	assert_eq!(fetch_count.get(), 1);
}

#[test]
fn query_retry_refetch_resets_a_waiting_sequence_immediately() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-refetch-reset");
	let fetch_count = Rc::new(Cell::new(0));
	let query = client.observe(
		family.query((), {
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				let attempt = fetch_count.get() + 1;
				fetch_count.set(attempt);
				async move { Err(format!("attempt-{attempt}")) }
			}
		}),
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_secs(1))
				.max_delay(Duration::from_secs(1)),
		),
	);
	runtime.run_until_stalled();

	query.refetch();
	runtime.run_until_stalled();
	assert_eq!(fetch_count.get(), 2);
	assert_eq!(query.error(), None);

	runtime.advance(Duration::from_secs(1));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 3);
	assert_eq!(query.error(), Some("attempt-3".to_string()));
}

#[test]
fn query_retry_invalidation_resets_a_waiting_sequence_immediately() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-invalidation-reset");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let attempt = fetch_count.get() + 1;
			fetch_count.set(attempt);
			async move { Err(format!("attempt-{attempt}")) }
		}
	});
	let key = descriptor.key().clone();
	let query = client.observe(
		descriptor,
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_secs(1))
				.max_delay(Duration::from_secs(1)),
		),
	);
	runtime.run_until_stalled();

	client.invalidate(&key);
	runtime.run_until_stalled();
	assert_eq!(fetch_count.get(), 2);
	assert!(query.is_stale());

	runtime.advance(Duration::from_secs(1));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 3);
	assert_eq!(query.error(), Some("attempt-3".to_string()));
}

#[test]
fn query_retry_refetch_coalesces_one_follow_up_for_an_in_flight_attempt() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-refetch-coalesce");
	let ready = Rc::new(Cell::new(false));
	let fetch_count = Rc::new(Cell::new(0));
	let query = client.observe(
		family.query((), {
			let ready = Rc::clone(&ready);
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				fetch_count.set(fetch_count.get() + 1);
				TestGate {
					ready: Rc::clone(&ready),
					dropped: Rc::new(Cell::new(0)),
					result: Some(Ok("fresh".to_string())),
				}
			}
		}),
		QueryOptions::default(),
	);
	runtime.run_until_stalled();

	query.refetch();
	query.refetch();
	query.refetch();
	ready.set(true);
	runtime.run_until_stalled();

	assert_eq!(fetch_count.get(), 2);
	assert_eq!(query.data(), Some("fresh".to_string()));
	assert!(!query.is_fetching());
}

#[test]
fn query_retry_superseded_failure_stays_unpublished_and_retargets_waiters() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-superseded-failure");
	let ready = Rc::new(Cell::new(false));
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let ready = Rc::clone(&ready);
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let attempt = fetch_count.get() + 1;
			fetch_count.set(attempt);
			TestGate {
				ready: Rc::clone(&ready),
				dropped: Rc::new(Cell::new(0)),
				result: Some(if attempt == 1 {
					Err("obsolete".to_string())
				} else {
					Ok("fresh".to_string())
				}),
			}
		}
	});
	let lease = client.acquire(
		descriptor,
		QueryAcquireOptions {
			consumer: QueryConsumer::Navigation(1),
			error_policy: QueryErrorPolicy::Discard,
		},
	);
	runtime.run_until_stalled();
	let mut result = Box::pin(lease.result());
	let mut context = Context::from_waker(Waker::noop());
	assert_eq!(result.as_mut().poll(&mut context), Poll::Pending);

	let entry = Rc::clone(&lease.inner.entry);
	entry.start_fetch(true);
	ready.set(true);
	runtime.run_until_stalled();

	assert_eq!(fetch_count.get(), 2);
	assert_eq!(
		result.as_mut().poll(&mut context),
		Poll::Ready(Ok("fresh".to_string()))
	);
	assert_eq!(entry.refetch_error.get(), None);
	assert!(matches!(
		entry.state.with_untracked(|state| state.clone()),
		ResourceState::Success(value) if value == "fresh"
	));
}

#[test]
fn dropping_a_retargeted_ssr_waiter_cancels_the_replacement_generation() {
	let client = QueryClient::new_ssr(QueryDefaults::default());
	let family = QueryFamily::<(), String, String>::new("tests.ssr-retargeted-waiter-drop");
	let first_ready = Rc::new(Cell::new(false));
	let second_ready = Rc::new(Cell::new(false));
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let first_ready = Rc::clone(&first_ready);
		let second_ready = Rc::clone(&second_ready);
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let attempt = fetch_count.get() + 1;
			fetch_count.set(attempt);
			TestGate {
				ready: if attempt == 1 {
					Rc::clone(&first_ready)
				} else {
					Rc::clone(&second_ready)
				},
				dropped: Rc::new(Cell::new(0)),
				result: Some(if attempt == 1 {
					Err("obsolete".to_string())
				} else {
					Ok("fresh".to_string())
				}),
			}
		}
	});
	let lease = client.acquire(
		descriptor,
		QueryAcquireOptions {
			consumer: QueryConsumer::Navigation(1),
			error_policy: QueryErrorPolicy::Discard,
		},
	);
	let entry = Rc::clone(&lease.inner.entry);
	let mut result = Box::pin(lease.result());
	let mut context = Context::from_waker(Waker::noop());
	assert_eq!(result.as_mut().poll(&mut context), Poll::Pending);
	entry.start_fetch(true);

	first_ready.set(true);
	assert_eq!(result.as_mut().poll(&mut context), Poll::Pending);
	assert_eq!(fetch_count.get(), 2);
	assert!(entry.has_request());

	drop(result);

	assert!(!entry.has_request());
	assert!(entry.retry.borrow().is_none());
	assert!(!entry.is_fetching.get());
}

#[test]
fn dropping_one_shared_ssr_waiter_keeps_the_sequence_alive() {
	let client = QueryClient::new_ssr(QueryDefaults::default());
	let family = QueryFamily::<(), String, String>::new("tests.ssr-shared-waiter-drop");
	let ready = Rc::new(Cell::new(false));
	let descriptor = family.query((), {
		let ready = Rc::clone(&ready);
		move || TestGate {
			ready: Rc::clone(&ready),
			dropped: Rc::new(Cell::new(0)),
			result: Some(Ok("shared".to_string())),
		}
	});
	let first = client.acquire(
		descriptor.clone(),
		QueryAcquireOptions {
			consumer: QueryConsumer::Prefetch,
			error_policy: QueryErrorPolicy::Discard,
		},
	);
	let second = client.acquire(
		descriptor,
		QueryAcquireOptions {
			consumer: QueryConsumer::Navigation(1),
			error_policy: QueryErrorPolicy::Discard,
		},
	);
	let entry = Rc::clone(&first.inner.entry);
	let mut first_result = Box::pin(first.result());
	let mut second_result = Box::pin(second.result());
	let mut context = Context::from_waker(Waker::noop());
	assert_eq!(first_result.as_mut().poll(&mut context), Poll::Pending);
	assert_eq!(second_result.as_mut().poll(&mut context), Poll::Pending);

	drop(first_result);

	assert!(entry.has_request());
	ready.set(true);
	assert_eq!(
		second_result.as_mut().poll(&mut context),
		Poll::Ready(Ok("shared".to_string()))
	);
}

#[test]
fn query_retry_dropped_manual_refetch_publishes_the_in_flight_failure() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-dropped-manual-refetch");
	let ready = Rc::new(Cell::new(false));
	let descriptor = family.query((), {
		let ready = Rc::clone(&ready);
		move || TestGate {
			ready: Rc::clone(&ready),
			dropped: Rc::new(Cell::new(0)),
			result: Some(Err("offline".to_string())),
		}
	});
	let remaining = client.observe(descriptor.clone(), QueryOptions::default());
	let requesting = client.observe(descriptor, QueryOptions::default());
	runtime.run_until_stalled();

	requesting.refetch();
	drop(requesting);
	ready.set(true);
	runtime.run_until_stalled();

	assert_eq!(remaining.error(), Some("offline".to_string()));
	assert!(!remaining.is_fetching());
}

#[test]
fn query_retry_polling_joins_a_waiting_sequence_without_resetting_it() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-polling-joins");
	let fetch_count = Rc::new(Cell::new(0));
	let _query = client.observe(
		family.query((), {
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				fetch_count.set(fetch_count.get() + 1);
				async { Err("offline".to_string()) }
			}
		}),
		QueryOptions::new()
			.refetch_interval(Duration::from_millis(500))
			.retry(
				RetryPolicy::exponential()
					.max_attempts(2)
					.base_delay(Duration::from_secs(1))
					.max_delay(Duration::from_secs(1)),
			),
	);
	runtime.run_until_stalled();

	runtime.advance(Duration::from_millis(500));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 1);

	runtime.advance(Duration::from_millis(500));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 2);
}

#[test]
fn query_retry_cancel_invalidates_a_waiting_deadline() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-cancel-wait");
	let fetch_count = Rc::new(Cell::new(0));
	let query = client.observe(
		family.query((), {
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				fetch_count.set(fetch_count.get() + 1);
				async { Err("offline".to_string()) }
			}
		}),
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_secs(1))
				.max_delay(Duration::from_secs(1)),
		),
	);
	runtime.run_until_stalled();
	let entry = Rc::clone(&query.entry);

	drop(query);
	runtime.advance(Duration::from_secs(1));
	runtime.run_due_maintenance();

	assert_eq!(fetch_count.get(), 1);
	assert!(entry.retry.borrow().is_none());
	assert!(!entry.is_fetching.get());
}

#[test]
fn query_retry_client_drop_invalidates_a_waiting_deadline() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-client-cancel-wait");
	let fetch_count = Rc::new(Cell::new(0));
	let query = client.observe(
		family.query((), {
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				fetch_count.set(fetch_count.get() + 1);
				async { Err("offline".to_string()) }
			}
		}),
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_secs(1))
				.max_delay(Duration::from_secs(1)),
		),
	);
	runtime.run_until_stalled();
	let entry = Rc::clone(&query.entry);

	drop(client);
	runtime.advance(Duration::from_secs(1));
	runtime.run_due_maintenance();

	assert_eq!(fetch_count.get(), 1);
	assert!(entry.retry.borrow().is_none());
	assert!(!entry.is_fetching.get());
}

#[test]
fn query_retry_client_drop_clears_disabled_manual_refetch_pending() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-client-cancel-disabled");
	let query = client.observe(
		family.query((), || async { Err("offline".to_string()) }),
		QueryOptions::new().enabled(false).retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_secs(1))
				.max_delay(Duration::from_secs(1)),
		),
	);

	query.refetch();
	runtime.run_until_stalled();
	assert_eq!(query.snapshot().status, QueryStatus::Pending);
	assert!(query.lease.inner.manual_refetch_pending.get());

	drop(client);

	assert_eq!(query.snapshot().status, QueryStatus::Idle);
	assert!(!query.lease.inner.manual_refetch_pending.get());
}

#[test]
fn query_retry_garbage_collection_invalidates_a_waiting_deadline() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.retry-gc-cancel-wait");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			fetch_count.set(fetch_count.get() + 1);
			async { Err("offline".to_string()) }
		}
	});
	let key = descriptor.key().clone();
	let query = client.observe(
		descriptor,
		QueryOptions::new().gc_time(Duration::from_secs(1)).retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_secs(1))
				.max_delay(Duration::from_secs(1)),
		),
	);
	runtime.run_until_stalled();
	let entry = Rc::clone(&query.entry);

	drop(query);
	runtime.advance(Duration::from_secs(1));
	runtime.run_due_maintenance();

	assert_eq!(fetch_count.get(), 1);
	assert!(!client.contains_for_test(&key));
	assert!(entry.retry.borrow().is_none());
}

#[test]
fn refetch_after_initial_error_transitions_through_pending() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.error-refetch-pending");
	let fetch_count = Rc::new(Cell::new(0));
	let ready = Rc::new(Cell::new(true));
	let query = client.observe(
		family.query((), {
			let fetch_count = Rc::clone(&fetch_count);
			let ready = Rc::clone(&ready);
			move || {
				let call = fetch_count.get() + 1;
				fetch_count.set(call);
				TestGate {
					ready: Rc::clone(&ready),
					dropped: Rc::new(Cell::new(0)),
					result: Some(if call == 1 {
						Err("offline".to_string())
					} else {
						Ok("recovered".to_string())
					}),
				}
			}
		}),
		QueryOptions::default(),
	);
	runtime.run_until_stalled();
	assert_eq!(query.snapshot().status, QueryStatus::Error);
	assert!(!query.snapshot().is_fetching);

	ready.set(false);
	query.refetch();

	assert_eq!(query.snapshot().status, QueryStatus::Pending);
	assert!(query.snapshot().is_fetching);
	assert_eq!(query.error(), None);

	ready.set(true);
	runtime.run_until_stalled();

	assert_eq!(fetch_count.get(), 2);
	assert_eq!(query.snapshot().status, QueryStatus::Success);
	assert_eq!(query.data(), Some("recovered".to_string()));
	assert!(!query.snapshot().is_fetching);
}

#[rstest]
fn stale_cached_error_refetches_when_observer_remounts() {
	// Arrange
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.stale-error-remount");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			fetch_count.set(fetch_count.get() + 1);
			async { Err::<String, _>("offline".to_string()) }
		}
	});
	let options = QueryOptions::new()
		.stale_time(Duration::from_secs(5))
		.gc_time(Duration::from_secs(30));
	let first = client.observe(descriptor.clone(), options.clone());
	runtime.run_until_stalled();
	drop(first);
	runtime.advance(Duration::from_secs(5));

	// Act
	let remounted = client.observe(descriptor, options);
	runtime.run_until_stalled();

	// Assert
	assert_eq!(fetch_count.get(), 2);
	assert_eq!(remounted.error(), Some("offline".to_string()));
}

#[test]
fn background_failure_preserves_data_and_clears_on_next_request() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.background-error");
	let fail_next_fetch = Rc::new(Cell::new(false));
	let descriptor = family.query((), {
		let fail_next_fetch = Rc::clone(&fail_next_fetch);
		move || {
			let fail = fail_next_fetch.get();
			async move {
				if fail {
					Err("offline".to_string())
				} else {
					Ok("cached".to_string())
				}
			}
		}
	});
	let key = descriptor.key().clone();
	let query = client.observe(descriptor, QueryOptions::default());
	runtime.run_until_stalled();

	fail_next_fetch.set(true);
	client.invalidate(&key);
	runtime.run_until_stalled();

	assert_eq!(
		query.snapshot(),
		QuerySnapshot {
			status: QueryStatus::Success,
			data: Some("cached".to_string()),
			error: None,
			refetch_error: Some("offline".to_string()),
			is_fetching: false,
			is_stale: true,
		}
	);

	fail_next_fetch.set(false);
	query.refetch();

	assert_eq!(query.refetch_error(), None);
	assert!(query.is_fetching());

	runtime.run_until_stalled();
	assert_eq!(query.data(), Some("cached".to_string()));
}

#[test]
fn disabled_observer_reads_cache_and_can_refetch_explicitly() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.disabled-cached-read");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let value = fetch_count.get() + 1;
			fetch_count.set(value);
			async move { Ok(format!("cached-{value}")) }
		}
	});
	let enabled = client.observe(descriptor.clone(), QueryOptions::default());
	runtime.run_until_stalled();
	assert_eq!(enabled.data(), Some("cached-1".to_string()));
	drop(enabled);

	let disabled = client.observe(descriptor, QueryOptions::new().enabled(false));

	assert_eq!(disabled.data(), Some("cached-1".to_string()));
	assert_eq!(fetch_count.get(), 1);

	disabled.refetch();
	runtime.run_until_stalled();

	assert_eq!(fetch_count.get(), 2);
	assert_eq!(disabled.data(), Some("cached-2".to_string()));
}

#[test]
fn invalidation_notifies_a_disabled_observer_of_staleness() {
	ReactiveScope::run(|| {
		let runtime = TestQueryRuntime::new();
		let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
		let family =
			QueryFamily::<(), String, String>::new("tests.disabled-invalidation-notification");
		let descriptor = family.query((), || async { Ok("cached".to_string()) });
		let key = descriptor.key().clone();
		let enabled = client.observe(descriptor.clone(), QueryOptions::default());
		runtime.run_until_stalled();
		drop(enabled);
		let disabled = client.observe(descriptor, QueryOptions::new().enabled(false));
		let observed_staleness = Rc::new(Cell::new(false));
		let observed_staleness_for_effect = Rc::clone(&observed_staleness);
		let query_for_effect = disabled.clone();
		let _effect = reinhardt_core::reactive::Effect::new(move || {
			observed_staleness_for_effect.set(query_for_effect.is_stale());
		});
		assert!(!observed_staleness.get());

		client.invalidate(&key);
		reinhardt_core::reactive::runtime::with_runtime(|runtime| runtime.flush_updates());

		assert!(observed_staleness.get());
		assert_eq!(disabled.data(), Some("cached".to_string()));
	});
}

#[test]
fn disabled_refetch_prefers_the_earliest_live_enabled_observer_fetcher() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.disabled-enabled-priority");
	let enabled_calls = Rc::new(Cell::new(0));
	let enabled = client.observe(
		family.query((), {
			let enabled_calls = Rc::clone(&enabled_calls);
			move || {
				let call = enabled_calls.get() + 1;
				enabled_calls.set(call);
				async move { Ok(format!("enabled-{call}")) }
			}
		}),
		QueryOptions::default(),
	);
	runtime.run_until_stalled();

	let disabled_calls = Rc::new(Cell::new(0));
	let disabled = client.observe(
		family.query((), {
			let disabled_calls = Rc::clone(&disabled_calls);
			move || {
				let call = disabled_calls.get() + 1;
				disabled_calls.set(call);
				async move { Ok(format!("disabled-{call}")) }
			}
		}),
		QueryOptions::new().enabled(false),
	);

	disabled.refetch();
	runtime.run_until_stalled();

	assert_eq!(enabled_calls.get(), 2);
	assert_eq!(disabled_calls.get(), 0);
	assert_eq!(enabled.data(), Some("enabled-2".to_string()));
	assert_eq!(disabled.data(), Some("enabled-2".to_string()));
}

#[test]
fn disabled_double_refetch_queued_behind_an_enabled_fetch_uses_its_fetcher() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.disabled-queued-refetch");
	let enabled_ready = Rc::new(Cell::new(false));
	let enabled_calls = Rc::new(Cell::new(0));
	let enabled = client.observe(
		family.query((), {
			let enabled_ready = Rc::clone(&enabled_ready);
			let enabled_calls = Rc::clone(&enabled_calls);
			move || {
				enabled_calls.set(enabled_calls.get() + 1);
				TestGate {
					ready: Rc::clone(&enabled_ready),
					dropped: Rc::new(Cell::new(0)),
					result: Some(Ok("enabled".to_string())),
				}
			}
		}),
		QueryOptions::default(),
	);
	runtime.run_until_stalled();

	let disabled_calls = Rc::new(Cell::new(0));
	let disabled = client.observe(
		family.query((), {
			let disabled_calls = Rc::clone(&disabled_calls);
			move || {
				let call = disabled_calls.get() + 1;
				disabled_calls.set(call);
				async move { Ok(format!("disabled-{call}")) }
			}
		}),
		QueryOptions::new().enabled(false),
	);

	disabled.refetch();
	disabled.refetch();
	assert_eq!(disabled.snapshot().status, QueryStatus::Pending);
	assert!(disabled.snapshot().is_fetching);
	drop(enabled);

	enabled_ready.set(true);
	runtime.run_until_stalled();

	assert_eq!(enabled_calls.get(), 1);
	assert_eq!(disabled_calls.get(), 1);
	assert_eq!(disabled.data(), Some("disabled-1".to_string()));
	assert_eq!(disabled.snapshot().status, QueryStatus::Success);
	assert!(!disabled.snapshot().is_fetching);
}

#[test]
fn observer_policies_do_not_overwrite_each_other() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(
		QueryDefaults::default()
			.stale_time(Duration::from_secs(90))
			.gc_time(Duration::from_secs(180)),
		runtime.handle(),
	);
	let family = QueryFamily::<(), String, String>::new("tests.observer-policies");
	let descriptor = family.query((), || async { Ok("cached".to_string()) });
	let first = client.observe(descriptor.clone(), QueryOptions::default());
	runtime.run_until_stalled();
	let second = client.observe(
		descriptor,
		QueryOptions::default()
			.stale_time(Duration::ZERO)
			.gc_time(Duration::from_secs(1)),
	);
	runtime.run_until_stalled();

	assert!(!first.is_stale());
	assert!(second.is_stale());
	assert_eq!(first.lease.inner.policy.gc_time, Duration::from_secs(180));
	assert_eq!(second.lease.inner.policy.gc_time, Duration::from_secs(1));
}

#[test]
fn polling_and_gc_follow_the_deterministic_runtime_clock() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.clock());
	let family = QueryFamily::<(), String, String>::new("tests.polling-gc-clock");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let call = fetch_count.get() + 1;
			fetch_count.set(call);
			async move { Ok(format!("value-{call}")) }
		}
	});
	let key = descriptor.key().clone();
	let query = client.observe(
		descriptor,
		QueryOptions::new()
			.refetch_interval(Duration::from_secs(5))
			.gc_time(Duration::from_secs(30)),
	);
	runtime.run_until_stalled();
	assert_eq!(fetch_count.get(), 1);

	runtime.advance(Duration::from_secs(5));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 2);

	drop(query);
	runtime.advance(Duration::from_secs(29));
	runtime.run_due_maintenance();
	assert!(client.contains_for_test(&key));

	runtime.advance(Duration::from_secs(1));
	runtime.run_due_maintenance();
	assert!(!client.contains_for_test(&key));
}

#[test]
fn earliest_polling_observer_starts_one_shared_request() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.clock());
	let family = QueryFamily::<(), String, String>::new("tests.earliest-polling-observer");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let call = fetch_count.get() + 1;
			fetch_count.set(call);
			async move { Ok(format!("value-{call}")) }
		}
	});
	let _slow = client.observe(
		descriptor.clone(),
		QueryOptions::new().refetch_interval(Duration::from_secs(10)),
	);
	let _fast = client.observe(
		descriptor,
		QueryOptions::new().refetch_interval(Duration::from_secs(5)),
	);
	runtime.run_until_stalled();
	assert_eq!(fetch_count.get(), 1);

	runtime.advance(Duration::from_secs(5));
	runtime.run_due_maintenance();

	assert_eq!(fetch_count.get(), 2);
}

#[test]
fn polling_interval_restarts_from_request_completion() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.clock());
	let family = QueryFamily::<(), String, String>::new("tests.polling-after-completion");
	let ready = Rc::new(Cell::new(false));
	let fetch_count = Rc::new(Cell::new(0));
	let _query = client.observe(
		family.query((), {
			let ready = Rc::clone(&ready);
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				let call = fetch_count.get() + 1;
				fetch_count.set(call);
				TestGate {
					ready: Rc::clone(&ready),
					dropped: Rc::new(Cell::new(0)),
					result: Some(Ok(format!("value-{call}"))),
				}
			}
		}),
		QueryOptions::new().refetch_interval(Duration::from_secs(5)),
	);
	runtime.run_until_stalled();
	assert_eq!(fetch_count.get(), 1);

	runtime.advance(Duration::from_secs(4));
	ready.set(true);
	runtime.run_until_stalled();
	runtime.advance(Duration::from_secs(1));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 1);

	runtime.advance(Duration::from_secs(4));
	runtime.run_due_maintenance();
	assert_eq!(fetch_count.get(), 2);
}

#[test]
fn disabled_observer_does_not_contribute_a_polling_deadline() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.clock());
	let family = QueryFamily::<(), String, String>::new("tests.disabled-polling");
	let fetch_count = Rc::new(Cell::new(0));
	let _query = client.observe(
		family.query((), {
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				fetch_count.set(fetch_count.get() + 1);
				async { Ok("unused".to_string()) }
			}
		}),
		QueryOptions::new()
			.enabled(false)
			.refetch_interval(Duration::from_secs(5)),
	);

	runtime.advance(Duration::from_secs(10));
	runtime.run_due_maintenance();

	assert_eq!(fetch_count.get(), 0);
}

#[test]
fn final_observer_drop_uses_the_epoch_maximum_gc_time() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.clock());
	let family = QueryFamily::<(), String, String>::new("tests.maximum-gc-time");
	let descriptor = family.query((), || async { Ok("cached".to_string()) });
	let key = descriptor.key().clone();
	let short = client.observe(
		descriptor.clone(),
		QueryOptions::new().gc_time(Duration::from_secs(10)),
	);
	let long = client.observe(
		descriptor,
		QueryOptions::new().gc_time(Duration::from_secs(30)),
	);
	runtime.run_until_stalled();

	drop(short);
	drop(long);
	runtime.advance(Duration::from_secs(29));
	runtime.run_due_maintenance();
	assert!(client.contains_for_test(&key));

	runtime.advance(Duration::from_secs(1));
	runtime.run_due_maintenance();
	assert!(!client.contains_for_test(&key));
}

#[test]
fn remount_invalidates_an_older_gc_deadline() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.clock());
	let family = QueryFamily::<(), String, String>::new("tests.remount-cancels-gc");
	let descriptor = family.query((), || async { Ok("cached".to_string()) });
	let key = descriptor.key().clone();
	let first = client.observe(
		descriptor.clone(),
		QueryOptions::new().gc_time(Duration::from_secs(5)),
	);
	runtime.run_until_stalled();
	drop(first);

	runtime.advance(Duration::from_secs(4));
	let second = client.observe(
		descriptor,
		QueryOptions::new().gc_time(Duration::from_secs(5)),
	);
	runtime.advance(Duration::from_secs(1));
	runtime.run_due_maintenance();

	assert!(client.contains_for_test(&key));

	drop(second);
	runtime.advance(Duration::from_secs(5));
	runtime.run_due_maintenance();
	assert!(!client.contains_for_test(&key));
}

#[test]
fn exact_invalidation_only_refetches_the_matching_active_query() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<i64, String, String>::new("tests.exact-invalidation");
	let first_count = Rc::new(Cell::new(0));
	let second_count = Rc::new(Cell::new(0));
	let first_descriptor = family.query(1, {
		let first_count = Rc::clone(&first_count);
		move || {
			first_count.set(first_count.get() + 1);
			async { Ok("first".to_string()) }
		}
	});
	let first_key = first_descriptor.key().clone();
	let _first = client.observe(first_descriptor, QueryOptions::default());
	let _second = client.observe(
		family.query(2, {
			let second_count = Rc::clone(&second_count);
			move || {
				second_count.set(second_count.get() + 1);
				async { Ok("second".to_string()) }
			}
		}),
		QueryOptions::default(),
	);
	runtime.run_until_stalled();

	client.invalidate(&first_key);
	runtime.run_until_stalled();

	assert_eq!(first_count.get(), 2);
	assert_eq!(second_count.get(), 1);
}

#[rstest]
fn exact_removal_clears_active_data_and_refetches_after_remount() {
	// Arrange
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.exact-removal");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let fetch_count = Rc::clone(&fetch_count);
			async move {
				let next = fetch_count.get() + 1;
				fetch_count.set(next);
				Ok(format!("value-{next}"))
			}
		}
	});
	let key = descriptor.key().clone();
	let active = client.observe(descriptor.clone(), QueryOptions::new());
	runtime.run_until_stalled();
	assert_eq!(active.data(), Some("value-1".to_string()));

	// Act
	client.remove(&key);

	// Assert
	let removed = active.snapshot();
	assert_eq!(removed.status, QueryStatus::Pending);
	assert_eq!(removed.data, None);
	assert_eq!(removed.error, None);
	assert_eq!(removed.refetch_error, None);
	assert!(!removed.is_fetching);
	assert!(!client.contains_for_test(&key));
	active.refetch();
	runtime.run_until_stalled();
	assert_eq!(fetch_count.get(), 1);

	let remounted = client.observe(descriptor, QueryOptions::new());
	runtime.run_until_stalled();
	assert_eq!(fetch_count.get(), 2);
	assert_eq!(remounted.data(), Some("value-2".to_string()));
}

#[rstest]
fn family_removal_evicts_only_matching_entries() {
	// Arrange
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<i64, String, String>::new("tests.family-removal");
	let other_family = QueryFamily::<i64, String, String>::new("tests.other-family-removal");
	let target_count = Rc::new(Cell::new(0));
	let target = |value, target_count: Rc<Cell<i32>>| {
		family.query(value, move || {
			target_count.set(target_count.get() + 1);
			async move { Ok(format!("target-{value}")) }
		})
	};
	let first_descriptor = target(1, Rc::clone(&target_count));
	let second_descriptor = target(2, Rc::clone(&target_count));
	let other_descriptor = other_family.query(1, || async { Ok("other".to_string()) });
	let first_key = first_descriptor.key().clone();
	let second_key = second_descriptor.key().clone();
	let other_key = other_descriptor.key().clone();
	let first = client.observe(first_descriptor.clone(), QueryOptions::default());
	let second = client.observe(second_descriptor, QueryOptions::default());
	let other = client.observe(other_descriptor, QueryOptions::default());
	runtime.run_until_stalled();

	// Act
	client.remove_family(family);

	// Assert
	assert_eq!(first.snapshot().status, QueryStatus::Pending);
	assert_eq!(first.data(), None);
	assert_eq!(second.snapshot().status, QueryStatus::Pending);
	assert_eq!(second.data(), None);
	assert_eq!(other.data(), Some("other".to_string()));
	assert!(!client.contains_for_test(&first_key));
	assert!(!client.contains_for_test(&second_key));
	assert!(client.contains_for_test(&other_key));

	let remounted = client.observe(first_descriptor, QueryOptions::default());
	runtime.run_until_stalled();
	assert_eq!(target_count.get(), 3);
	assert_eq!(remounted.data(), Some("target-1".to_string()));
}

#[rstest]
fn removal_cancels_in_flight_request() {
	// Arrange
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let cancellations = Rc::new(Cell::new(0));
	let family = QueryFamily::<(), String, String>::new("tests.removal-cancellation");
	let descriptor = family.query_with_cancellation((), {
		let cancellations = Rc::clone(&cancellations);
		move |cancellation| {
			let cancellations = Rc::clone(&cancellations);
			async move {
				let _registration = cancellation.on_cancel(move || {
					cancellations.set(cancellations.get() + 1);
				});
				std::future::pending::<Result<String, String>>().await
			}
		}
	});
	let key = descriptor.key().clone();
	let lease = client.acquire(
		descriptor.clone(),
		QueryAcquireOptions {
			consumer: QueryConsumer::Navigation(1),
			error_policy: QueryErrorPolicy::Discard,
		},
	);
	let mut result = Box::pin(lease.result());
	let mut context = Context::from_waker(Waker::noop());
	let query = client.observe(descriptor, QueryOptions::default());
	runtime.run_until_stalled();
	assert_eq!(result.as_mut().poll(&mut context), Poll::Pending);

	// Act
	client.remove(&key);
	runtime.run_until_stalled();

	// Assert
	assert_eq!(cancellations.get(), 1);
	assert_eq!(
		result.as_mut().poll(&mut context),
		Poll::Ready(Err(QueryResultError::Evicted))
	);
	assert_eq!(query.snapshot().status, QueryStatus::Pending);
	assert!(!query.snapshot().is_fetching);
}

#[rstest]
fn removal_clears_waiting_retry_without_a_follow_up_request() {
	// Arrange
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let fetch_count = Rc::new(Cell::new(0));
	let family = QueryFamily::<(), String, String>::new("tests.removal-retry");
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			fetch_count.set(fetch_count.get() + 1);
			async { Err("offline".to_string()) }
		}
	});
	let key = descriptor.key().clone();
	let query = client.observe(
		descriptor,
		QueryOptions::new().retry(
			RetryPolicy::exponential()
				.max_attempts(2)
				.base_delay(Duration::from_secs(1))
				.max_delay(Duration::from_secs(1)),
		),
	);
	runtime.run_until_stalled();
	assert_eq!(fetch_count.get(), 1);
	assert!(query.entry.retry.borrow().is_some());

	// Act
	client.remove(&key);
	runtime.advance(Duration::from_secs(1));
	runtime.run_due_maintenance();

	// Assert
	assert_eq!(fetch_count.get(), 1);
	assert!(query.entry.retry.borrow().is_none());
	assert_eq!(query.snapshot().status, QueryStatus::Pending);
}

#[test]
fn family_invalidation_marks_inactive_entries_stale_until_remount() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<i64, String, String>::new("tests.family-invalidation");
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query(1, {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let value = fetch_count.get() + 1;
			fetch_count.set(value);
			async move {
				let value = if value == 1 { "cached" } else { "refetched" };
				Ok(value.to_string())
			}
		}
	});
	let query = client.observe(descriptor.clone(), QueryOptions::default());
	runtime.run_until_stalled();
	assert_eq!(query.data(), Some("cached".to_string()));

	drop(query);
	client.invalidate_family(family);
	runtime.run_until_stalled();

	assert_eq!(fetch_count.get(), 1);

	let remounted = client.observe(descriptor, QueryOptions::default());
	runtime.run_until_stalled();

	assert_eq!(fetch_count.get(), 2);
	assert_eq!(remounted.data(), Some("refetched".to_string()));
}

#[test]
fn disabled_only_family_invalidation_does_not_fetch() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.disabled-invalidation");
	let fetch_count = Rc::new(Cell::new(0));
	let query = client.observe(
		family.query((), {
			let fetch_count = Rc::clone(&fetch_count);
			move || {
				fetch_count.set(fetch_count.get() + 1);
				async { Ok("cached".to_string()) }
			}
		}),
		QueryOptions::new().enabled(false),
	);

	client.invalidate_family(family);
	runtime.run_until_stalled();

	assert_eq!(fetch_count.get(), 0);
	assert_eq!(query.snapshot().status, QueryStatus::Idle);
	assert!(query.is_stale());
}

#[test]
fn typed_family_keys_share_family_and_distinguish_arguments() {
	let family = QueryFamily::<i64, String, String>::new("tests.project");

	let first = family.key(1);
	let same = family.key(1);
	let second = family.key(2);

	assert_eq!(first.identity(), same.identity());
	assert_ne!(first.identity(), second.identity());
	assert_eq!(first.family_id(), "tests.project");
}

#[test]
#[should_panic(expected = "query family arguments must serialize into a stable cache key")]
fn typed_family_rejects_arguments_without_a_stable_fingerprint() {
	let family = QueryFamily::<FailingFingerprintArgs, String, String>::new("tests.failure");

	let _ = family.key(FailingFingerprintArgs);
}

#[test]
fn identical_keys_share_only_within_one_client() {
	let family = QueryFamily::<i64, String, String>::new("tests.client-scope");
	let runtime = TestQueryRuntime::new();
	let first_client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let second_client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let calls = Rc::new(Cell::new(0));

	let counted_fetcher = |calls: Rc<Cell<usize>>, value: &'static str| {
		move || {
			calls.set(calls.get() + 1);
			async move { Ok::<_, String>(value.to_string()) }
		}
	};

	let first = first_client.observe(
		family.query(1, counted_fetcher(Rc::clone(&calls), "first")),
		QueryOptions::default(),
	);
	let same_client = first_client.observe(
		family.query(1, counted_fetcher(Rc::clone(&calls), "first")),
		QueryOptions::default(),
	);
	let other_client = second_client.observe(
		family.query(1, counted_fetcher(Rc::clone(&calls), "second")),
		QueryOptions::default(),
	);

	runtime.run_until_stalled();

	assert_eq!(first.data(), Some("first".to_string()));
	assert_eq!(same_client.data(), Some("first".to_string()));
	assert_eq!(other_client.data(), Some("second".to_string()));
	assert_eq!(calls.get(), 2);
}

#[test]
fn one_client_rejects_incompatible_query_family_types() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let first = QueryFamily::<i64, String, String>::new("tests.collision");
	let second = QueryFamily::<String, String, String>::new("tests.collision");

	let _first = client.observe(
		first.query(1, || async { Ok::<_, String>("first".to_string()) }),
		QueryOptions::default(),
	);
	let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
		let _second = client.observe(
			second.query("1".to_string(), || async {
				Ok::<_, String>("second".to_string())
			}),
			QueryOptions::default(),
		);
	}))
	.expect_err("incompatible query family types must panic");
	let message = panic
		.downcast_ref::<String>()
		.map(String::as_str)
		.or_else(|| panic.downcast_ref::<&str>().copied())
		.expect("query family type collision should panic with a string");

	assert!(message.contains("tests.collision"));
	assert!(message.contains("incompatible query family types"));
}

#[test]
fn exact_invalidation_rejects_an_incompatible_family_with_the_same_id() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let registered = QueryFamily::<i64, String, String>::new("tests.invalidate-collision");
	let incompatible = QueryFamily::<String, String, String>::new("tests.invalidate-collision");
	let _query = client.observe(
		registered.query(1, || async { Ok::<_, String>("cached".to_string()) }),
		QueryOptions::default(),
	);
	runtime.run_until_stalled();

	let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
		client.invalidate(&incompatible.key("1".to_string()));
	}))
	.expect_err("incompatible exact invalidation must panic");
	let message = panic
		.downcast_ref::<String>()
		.map(String::as_str)
		.or_else(|| panic.downcast_ref::<&str>().copied())
		.expect("query family type collision should panic with a string");

	assert_eq!(
		message,
		"incompatible query family types for `tests.invalidate-collision`: expected Args=`i64`, data=`alloc::string::String`, error=`alloc::string::String`; actual Args=`alloc::string::String`, data=`alloc::string::String`, error=`alloc::string::String`"
	);
}

#[test]
fn family_invalidation_rejects_an_incompatible_family_with_the_same_id() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let registered = QueryFamily::<i64, String, String>::new("tests.invalidate-family-collision");
	let incompatible = QueryFamily::<i64, u64, String>::new("tests.invalidate-family-collision");
	let _query = client.observe(
		registered.query(1, || async { Ok::<_, String>("cached".to_string()) }),
		QueryOptions::default(),
	);
	runtime.run_until_stalled();

	let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
		client.invalidate_family(incompatible);
	}))
	.expect_err("incompatible family invalidation must panic");
	let message = panic
		.downcast_ref::<String>()
		.map(String::as_str)
		.or_else(|| panic.downcast_ref::<&str>().copied())
		.expect("query family type collision should panic with a string");

	assert_eq!(
		message,
		"incompatible query family types for `tests.invalidate-family-collision`: expected Args=`i64`, data=`alloc::string::String`, error=`alloc::string::String`; actual Args=`i64`, data=`u64`, error=`alloc::string::String`"
	);
}

mod normalization_contract {
	use super::*;

	#[derive(Clone)]
	struct FirstProjection;

	impl EntityProjection<String> for FirstProjection {
		type Recipe = String;

		const SCHEMA: &'static str = "first-projection-v1";

		fn normalize(&self, value: String, _entities: &mut EntityWriter<'_>) -> Self::Recipe {
			value
		}

		fn dependencies(&self, _recipe: &Self::Recipe, _dependencies: &mut EntityDependencies) {}

		fn materialize(
			&self,
			recipe: &Self::Recipe,
			_entities: &EntityReader<'_>,
		) -> ProjectionMaterialization<String> {
			ProjectionMaterialization::Ready(recipe.clone())
		}

		fn apply_removals(
			&self,
			_recipe: &mut Self::Recipe,
			_removed: &RemovedEntities<'_>,
		) -> ProjectionRemoval {
			ProjectionRemoval::Unchanged
		}
	}

	#[derive(Clone)]
	struct SameSchemaProjection;

	impl EntityProjection<String> for SameSchemaProjection {
		type Recipe = String;

		const SCHEMA: &'static str = "first-projection-v1";

		fn normalize(&self, value: String, _entities: &mut EntityWriter<'_>) -> Self::Recipe {
			value
		}

		fn dependencies(&self, _recipe: &Self::Recipe, _dependencies: &mut EntityDependencies) {}

		fn materialize(
			&self,
			recipe: &Self::Recipe,
			_entities: &EntityReader<'_>,
		) -> ProjectionMaterialization<String> {
			ProjectionMaterialization::Ready(recipe.clone())
		}

		fn apply_removals(
			&self,
			_recipe: &mut Self::Recipe,
			_removed: &RemovedEntities<'_>,
		) -> ProjectionRemoval {
			ProjectionRemoval::Unchanged
		}
	}

	#[derive(Clone)]
	struct DifferentSchemaProjection;

	impl EntityProjection<String> for DifferentSchemaProjection {
		type Recipe = String;

		const SCHEMA: &'static str = "different-projection-v1";

		fn normalize(&self, value: String, _entities: &mut EntityWriter<'_>) -> Self::Recipe {
			value
		}

		fn dependencies(&self, _recipe: &Self::Recipe, _dependencies: &mut EntityDependencies) {}

		fn materialize(
			&self,
			recipe: &Self::Recipe,
			_entities: &EntityReader<'_>,
		) -> ProjectionMaterialization<String> {
			ProjectionMaterialization::Ready(recipe.clone())
		}

		fn apply_removals(
			&self,
			_recipe: &mut Self::Recipe,
			_removed: &RemovedEntities<'_>,
		) -> ProjectionRemoval {
			ProjectionRemoval::Unchanged
		}
	}

	#[derive(Clone)]
	struct EmptySchemaProjection;

	impl EntityProjection<String> for EmptySchemaProjection {
		type Recipe = String;

		const SCHEMA: &'static str = "";

		fn normalize(&self, value: String, _entities: &mut EntityWriter<'_>) -> Self::Recipe {
			value
		}

		fn dependencies(&self, _recipe: &Self::Recipe, _dependencies: &mut EntityDependencies) {}

		fn materialize(
			&self,
			recipe: &Self::Recipe,
			_entities: &EntityReader<'_>,
		) -> ProjectionMaterialization<String> {
			ProjectionMaterialization::Ready(recipe.clone())
		}

		fn apply_removals(
			&self,
			_recipe: &mut Self::Recipe,
			_removed: &RemovedEntities<'_>,
		) -> ProjectionRemoval {
			ProjectionRemoval::Unchanged
		}
	}

	#[derive(Clone)]
	#[allow(dead_code)] // This field deliberately makes the test adapter non-zero-sized.
	struct StatefulProjection(String);

	impl EntityProjection<String> for StatefulProjection {
		type Recipe = String;

		const SCHEMA: &'static str = "stateful-projection-v1";

		fn normalize(&self, value: String, _entities: &mut EntityWriter<'_>) -> Self::Recipe {
			value
		}

		fn dependencies(&self, _recipe: &Self::Recipe, _dependencies: &mut EntityDependencies) {}

		fn materialize(
			&self,
			recipe: &Self::Recipe,
			_entities: &EntityReader<'_>,
		) -> ProjectionMaterialization<String> {
			ProjectionMaterialization::Ready(recipe.clone())
		}

		fn apply_removals(
			&self,
			_recipe: &mut Self::Recipe,
			_removed: &RemovedEntities<'_>,
		) -> ProjectionRemoval {
			ProjectionRemoval::Unchanged
		}
	}

	fn panic_message(panic: Box<dyn std::any::Any + Send>) -> String {
		panic
			.downcast_ref::<String>()
			.cloned()
			.or_else(|| panic.downcast_ref::<&str>().map(ToString::to_string))
			.expect("normalization contract collision should panic with a string")
	}

	#[test]
	fn descriptor_retains_a_mode_neutral_key_for_exact_invalidation() {
		let runtime = TestQueryRuntime::new();
		let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
		let family = QueryFamily::<i64, String, String>::new("tests.normalized-key");
		let key = family.key(1);
		let descriptor = family
			.query(1, || async { Ok::<_, String>("cached".to_string()) })
			.with_entities(FirstProjection);

		assert_eq!(descriptor.key(), &key);
		let query = client.observe(descriptor, QueryOptions::new().enabled(false));
		client.invalidate(&key);

		assert!(query.is_stale());
	}

	#[test]
	fn rejects_a_plain_descriptor_after_a_normalized_family_registration() {
		let runtime = TestQueryRuntime::new();
		let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
		let family = QueryFamily::<i64, String, String>::new("tests.plain-normalized-collision");
		let _normalized = client.observe(
			family
				.query(1, || async { Ok::<_, String>("normalized".to_string()) })
				.with_entities(FirstProjection),
			QueryOptions::new().enabled(false),
		);

		let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			let _plain = client.observe(
				family.query(2, || async { Ok::<_, String>("plain".to_string()) }),
				QueryOptions::new().enabled(false),
			);
		}))
		.expect_err("plain and normalized descriptors must not share a family");
		let message = panic_message(panic);

		assert_eq!(
			message,
			format!(
				"incompatible query family normalization for `tests.plain-normalized-collision`: expected mode=normalized, adapter_type=`{}`, schema=`first-projection-v1`; actual mode=plain, adapter_type=none, schema=none",
				std::any::type_name::<FirstProjection>(),
			),
		);
	}

	#[test]
	fn rejects_normalized_descriptors_with_different_adapter_types() {
		let runtime = TestQueryRuntime::new();
		let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
		let family = QueryFamily::<i64, String, String>::new("tests.adapter-collision");
		let _first = client.observe(
			family
				.query(1, || async { Ok::<_, String>("first".to_string()) })
				.with_entities(FirstProjection),
			QueryOptions::new().enabled(false),
		);

		let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			let _second = client.observe(
				family
					.query(2, || async { Ok::<_, String>("second".to_string()) })
					.with_entities(SameSchemaProjection),
				QueryOptions::new().enabled(false),
			);
		}))
		.expect_err("different projection adapter types must not share a family");
		let message = panic_message(panic);

		assert_eq!(
			message,
			format!(
				"incompatible query family normalization for `tests.adapter-collision`: expected mode=normalized, adapter_type=`{}`, schema=`first-projection-v1`; actual mode=normalized, adapter_type=`{}`, schema=`first-projection-v1`",
				std::any::type_name::<FirstProjection>(),
				std::any::type_name::<SameSchemaProjection>(),
			),
		);
	}

	#[test]
	fn rejects_normalized_descriptors_with_different_schemas() {
		let runtime = TestQueryRuntime::new();
		let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
		let family = QueryFamily::<i64, String, String>::new("tests.schema-collision");
		let _first = client.observe(
			family
				.query(1, || async { Ok::<_, String>("first".to_string()) })
				.with_entities(FirstProjection),
			QueryOptions::new().enabled(false),
		);

		let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			let _second = client.observe(
				family
					.query(2, || async { Ok::<_, String>("second".to_string()) })
					.with_entities(DifferentSchemaProjection),
				QueryOptions::new().enabled(false),
			);
		}))
		.expect_err("different projection schemas must not share a family");
		let message = panic_message(panic);

		assert_eq!(
			message,
			format!(
				"incompatible query family normalization for `tests.schema-collision`: expected mode=normalized, adapter_type=`{}`, schema=`first-projection-v1`; actual mode=normalized, adapter_type=`{}`, schema=`different-projection-v1`",
				std::any::type_name::<FirstProjection>(),
				std::any::type_name::<DifferentSchemaProjection>(),
			),
		);
	}

	#[test]
	fn rejects_an_empty_projection_schema() {
		let family = QueryFamily::<(), String, String>::new("tests.empty-schema");

		let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			let _descriptor = family
				.query((), || async { Ok::<_, String>("value".to_string()) })
				.with_entities(EmptySchemaProjection);
		}))
		.expect_err("an empty projection schema must panic");
		let message = panic_message(panic);

		assert_eq!(
			message,
			format!(
				"entity projection adapter `{}` for query family `tests.empty-schema` with schema `` must define a non-empty schema",
				std::any::type_name::<EmptySchemaProjection>(),
			),
		);
	}

	#[test]
	fn rejects_a_non_zero_sized_projection_adapter() {
		let family = QueryFamily::<(), String, String>::new("tests.non-zst-projection");

		let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
			let _descriptor = family
				.query((), || async { Ok::<_, String>("value".to_string()) })
				.with_entities(StatefulProjection("state".to_string()));
		}))
		.expect_err("a stateful projection adapter must panic");
		let message = panic_message(panic);

		assert_eq!(
			message,
			format!(
				"entity projection adapter `{}` for query family `tests.non-zst-projection` with schema `stateful-projection-v1` must be zero-sized, but its size is {} bytes",
				std::any::type_name::<StatefulProjection>(),
				std::mem::size_of::<StatefulProjection>(),
			),
		);
	}
}

mod normalized {
	use super::*;
	use std::collections::HashMap;

	#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
	pub(super) struct Project {
		id: u64,
		name: String,
	}

	impl Entity for Project {
		type Id = u64;

		const TYPE: &'static str = "reactive.query.tests.normalized-project";

		fn entity_id(&self) -> Self::Id {
			self.id
		}
	}

	struct ProjectGate {
		ready: Rc<Cell<bool>>,
		dropped: Rc<Cell<usize>>,
		result: Option<Result<Project, String>>,
	}

	impl Future for ProjectGate {
		type Output = Result<Project, String>;

		fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
			let this = self.get_mut();
			if this.ready.get() {
				Poll::Ready(
					this.result
						.take()
						.expect("project gate polled after completion"),
				)
			} else {
				Poll::Pending
			}
		}
	}

	impl Drop for ProjectGate {
		fn drop(&mut self) {
			self.dropped.set(self.dropped.get() + 1);
		}
	}

	pub(super) fn project(id: u64, name: &str) -> Project {
		Project {
			id,
			name: name.to_string(),
		}
	}

	#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
	pub(super) struct ProjectList {
		label: String,
		projects: Vec<Project>,
	}

	struct ProjectListGate {
		ready: Rc<Cell<bool>>,
		result: Option<Result<ProjectList, String>>,
	}

	impl Future for ProjectListGate {
		type Output = Result<ProjectList, String>;

		fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
			let this = self.get_mut();
			if this.ready.get() {
				Poll::Ready(
					this.result
						.take()
						.expect("project-list gate polled after completion"),
				)
			} else {
				Poll::Pending
			}
		}
	}

	#[derive(Clone, Deserialize, Serialize)]
	pub(super) struct ProjectListRecipe {
		label: String,
		project_ids: Vec<u64>,
	}

	#[derive(Clone, Copy)]
	pub(super) struct ProjectListProjection;

	thread_local! {
		static PROJECTION_MATERIALIZATIONS: RefCell<HashMap<String, usize>> =
			RefCell::new(HashMap::new());
	}

	impl EntityProjection<ProjectList> for ProjectListProjection {
		type Recipe = ProjectListRecipe;

		const SCHEMA: &'static str = "project-list-v1";

		fn normalize(&self, value: ProjectList, entities: &mut EntityWriter<'_>) -> Self::Recipe {
			let project_ids = value
				.projects
				.into_iter()
				.map(|project| {
					let id = project.id;
					entities.upsert(project);
					id
				})
				.collect();
			ProjectListRecipe {
				label: value.label,
				project_ids,
			}
		}

		fn dependencies(&self, recipe: &Self::Recipe, dependencies: &mut EntityDependencies) {
			dependencies.extend::<Project>(recipe.project_ids.iter().copied());
		}

		fn materialize(
			&self,
			recipe: &Self::Recipe,
			entities: &EntityReader<'_>,
		) -> ProjectionMaterialization<ProjectList> {
			PROJECTION_MATERIALIZATIONS.with(|counts| {
				*counts.borrow_mut().entry(recipe.label.clone()).or_default() += 1;
			});
			match entities.required_vec::<Project>(&recipe.project_ids) {
				ProjectionMaterialization::Ready(projects) => {
					ProjectionMaterialization::Ready(ProjectList {
						label: recipe.label.clone(),
						projects,
					})
				}
				ProjectionMaterialization::MissingRequired => {
					ProjectionMaterialization::MissingRequired
				}
			}
		}

		fn apply_removals(
			&self,
			recipe: &mut Self::Recipe,
			removed: &RemovedEntities<'_>,
		) -> ProjectionRemoval {
			let previous_len = recipe.project_ids.len();
			recipe
				.project_ids
				.retain(|id| !removed.contains::<Project>(id));
			ProjectionRemoval::from_changed(previous_len != recipe.project_ids.len())
		}
	}

	pub(super) fn project_list(label: &str, projects: Vec<Project>) -> ProjectList {
		ProjectList {
			label: label.to_string(),
			projects,
		}
	}

	pub(super) fn reset_materializations() {
		PROJECTION_MATERIALIZATIONS.with(|counts| counts.borrow_mut().clear());
	}

	pub(super) fn materializations(label: &str) -> usize {
		PROJECTION_MATERIALIZATIONS
			.with(|counts| counts.borrow().get(label).copied().unwrap_or_default())
	}

	#[test]
	fn normalized_success_publishes_the_entity_and_materialized_query() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let query = client.observe(
				QueryFamily::<u64, Project, String>::new("tests.normalized-success")
					.query(1, || async { Ok(project(1, "normalized")) })
					.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);

			runtime.run_until_stalled();

			assert_eq!(
				client.entity::<Project>(1).get(),
				Some(project(1, "normalized"))
			);
			assert_eq!(query.data(), Some(project(1, "normalized")));
		});
	}

	#[rstest]
	fn removal_releases_normalized_dependency_and_reverse_index() {
		ReactiveScope::run(|| {
			// Arrange
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let descriptor = QueryFamily::<u64, Project, String>::new("tests.normalized-removal")
				.query(1, || async { Ok(project(1, "cached")) })
				.with_entities(EntityValue::new());
			let key = descriptor.key().clone();
			let query = client.observe(descriptor, QueryOptions::default());
			runtime.run_until_stalled();
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&1),
				1
			);
			assert_eq!(client.entity_dependency_index_len_for_test(), 1);

			// Act
			client.remove(&key);

			// Assert
			assert_eq!(query.data(), None);
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&1),
				0
			);
			assert_eq!(client.entity_dependency_index_len_for_test(), 0);
			client.upsert_entity(project(1, "new"));
			assert_eq!(query.data(), None);
		});
	}

	#[test]
	fn normalized_query_keys_share_one_entity_record() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let family = QueryFamily::<u64, Project, String>::new("tests.normalized-shared-entity");
			let first = client.observe(
				family
					.query(1, || async { Ok(project(7, "shared")) })
					.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);
			let second = client.observe(
				family
					.query(2, || async { Ok(project(7, "shared")) })
					.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);

			runtime.run_until_stalled();

			assert_eq!(first.data(), Some(project(7, "shared")));
			assert_eq!(second.data(), Some(project(7, "shared")));
			assert_eq!(
				client.entity::<Project>(7).get(),
				Some(project(7, "shared"))
			);
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&7),
				2
			);
		});
	}

	#[test]
	fn later_mutation_wins_over_an_older_in_flight_query() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let ready = Rc::new(Cell::new(false));
			let query = client.observe(
				QueryFamily::<u64, Project, String>::new("tests.normalized-ticket-race")
					.query(1, {
						let ready = Rc::clone(&ready);
						move || ProjectGate {
							ready: Rc::clone(&ready),
							dropped: Rc::new(Cell::new(0)),
							result: Some(Ok(project(1, "older-query"))),
						}
					})
					.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);

			client.upsert_entity(project(1, "mutation"));
			ready.set(true);
			runtime.run_until_stalled();

			assert_eq!(
				client.entity::<Project>(1).get(),
				Some(project(1, "mutation"))
			);
			assert_eq!(query.data(), Some(project(1, "mutation")));
		});
	}

	#[test]
	fn newer_tombstone_prevents_an_old_collection_query_from_restoring_membership() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let ready = Rc::new(Cell::new(false));
			let query = client.observe(
				QueryFamily::<(), ProjectList, String>::new(
					"tests.normalized-tombstone-membership-race",
				)
				.query((), {
					let ready = Rc::clone(&ready);
					move || ProjectListGate {
						ready: Rc::clone(&ready),
						result: Some(Ok(project_list(
							"older collection",
							vec![project(1, "older query")],
						))),
					}
				})
				.with_entities(ProjectListProjection),
				QueryOptions::new(),
			);

			client.remove_entity::<Project>(&1);
			ready.set(true);
			runtime.run_until_stalled();

			assert_eq!(query.data(), Some(project_list("older collection", vec![])));
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&1),
				0
			);

			reset_materializations();
			client.upsert_entity(project(1, "later mutation"));

			assert_eq!(query.data(), Some(project_list("older collection", vec![])));
			assert_eq!(materializations("older collection"), 0);
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&1),
				0
			);
		});
	}

	#[test]
	fn later_started_query_wins_when_it_completes_first() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let family = QueryFamily::<u64, Project, String>::new("tests.normalized-query-order");
			let first_ready = Rc::new(Cell::new(false));
			let second_ready = Rc::new(Cell::new(false));
			let first = client.observe(
				family
					.query(1, {
						let ready = Rc::clone(&first_ready);
						move || ProjectGate {
							ready: Rc::clone(&ready),
							dropped: Rc::new(Cell::new(0)),
							result: Some(Ok(project(1, "first"))),
						}
					})
					.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);
			let second = client.observe(
				family
					.query(2, {
						let ready = Rc::clone(&second_ready);
						move || ProjectGate {
							ready: Rc::clone(&ready),
							dropped: Rc::new(Cell::new(0)),
							result: Some(Ok(project(1, "second"))),
						}
					})
					.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);

			second_ready.set(true);
			runtime.run_until_stalled();
			first_ready.set(true);
			runtime.run_until_stalled();

			assert_eq!(
				client.entity::<Project>(1).get(),
				Some(project(1, "second"))
			);
			assert_eq!(first.data(), Some(project(1, "second")));
			assert_eq!(second.data(), Some(project(1, "second")));
		});
	}

	#[test]
	fn cancelling_a_normalized_request_releases_its_query_ticket() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let arena = client.entity_arena_for_test();
			let query = client.observe(
				QueryFamily::<u64, Project, String>::new("tests.normalized-ticket-cancel")
					.query(1, || ProjectGate {
						ready: Rc::new(Cell::new(false)),
						dropped: Rc::new(Cell::new(0)),
						result: Some(Ok(project(1, "pending"))),
					})
					.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);
			let ticket = query
				.entry
				.request
				.borrow()
				.as_ref()
				.expect("normalized request should own a ticket")
				.ticket
				.as_ref()
				.expect("normalized request should own an entity ticket")
				.ticket();

			assert_eq!(arena.active_query_ticket_count(ticket), 1);
			drop(query);

			assert_eq!(arena.active_query_ticket_count(ticket), 0);
		});
	}

	#[test]
	fn dropping_the_query_client_releases_normalized_request_tickets() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let arena = client.entity_arena_for_test();
			let query = client.observe(
				QueryFamily::<u64, Project, String>::new("tests.normalized-ticket-owner-drop")
					.query(1, || ProjectGate {
						ready: Rc::new(Cell::new(false)),
						dropped: Rc::new(Cell::new(0)),
						result: Some(Ok(project(1, "pending"))),
					})
					.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);
			let ticket = query
				.entry
				.request
				.borrow()
				.as_ref()
				.expect("normalized request should own a ticket")
				.ticket
				.as_ref()
				.expect("normalized request should own an entity ticket")
				.ticket();

			assert_eq!(arena.active_query_ticket_count(ticket), 1);
			drop(client);

			assert_eq!(arena.active_query_ticket_count(ticket), 0);
			drop(query);
		});
	}

	#[test]
	fn pending_plain_query_does_not_block_entity_garbage_collection() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(
				QueryDefaults::default().gc_time(Duration::ZERO),
				runtime.handle(),
			);
			let _plain = client.observe(
				QueryFamily::<(), String, String>::new("tests.plain-ticket-gc")
					.query((), || std::future::pending::<Result<String, String>>()),
				QueryOptions::new(),
			);
			let handle = client.entity::<Project>(1);
			client.upsert_entity(project(1, "temporary"));

			drop(handle);
			runtime.run_due_maintenance();

			assert!(client.entity::<Project>(1).get().is_none());
		});
	}
}

#[cfg(any(wasm, test))]
mod normalized_hydration {
	use super::normalized::{Project, project};
	use super::*;
	use crate::hydration::HydrationContext;
	use crate::reactive::entity::{
		ENTITY_TABLE_HYDRATION_ID, ENTITY_TABLE_VERSION, EntityHydrationEnvelope,
		EntityHydrationRow,
	};
	use crate::ssr::SsrState;
	use std::collections::BTreeMap;

	#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
	struct Task {
		id: u64,
		name: String,
	}

	impl Entity for Task {
		type Id = u64;

		const TYPE: &'static str = "reactive.query.tests.normalized-task";

		fn entity_id(&self) -> Self::Id {
			self.id
		}
	}

	#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
	struct WideProject {
		id: u64,
	}

	impl Entity for WideProject {
		type Id = u64;

		const TYPE: &'static str = "reactive.query.tests.wide-project";

		fn entity_id(&self) -> Self::Id {
			self.id
		}
	}

	fn table(value: Project) -> serde_json::Value {
		serde_json::to_value(EntityHydrationEnvelope {
			version: ENTITY_TABLE_VERSION,
			entities: BTreeMap::from([(
				Project::TYPE.to_string(),
				vec![EntityHydrationRow {
					id: serde_json::to_value(value.entity_id()).unwrap(),
					value: serde_json::to_value(value).unwrap(),
				}],
			)]),
		})
		.unwrap()
	}

	fn snapshot(id: u64) -> serde_json::Value {
		serde_json::json!({
			"version": 1,
			"kind": "success",
			"schema": "entity-value-v1",
			"state": { "Success": { "projection": id } },
			"refetch_error": null,
			"is_fetching": false,
			"is_stale": false,
		})
	}

	fn panic_message(panic: Box<dyn std::any::Any + Send>) -> String {
		panic
			.downcast_ref::<String>()
			.cloned()
			.or_else(|| panic.downcast_ref::<&str>().map(ToString::to_string))
			.expect("hydration validation should panic with a string")
	}

	#[test]
	fn normalized_hydration_reuses_entity_table_without_a_fetch() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let family = QueryFamily::<u64, Project, String>::new("tests.normalized-hydration");
			let descriptor = family
				.query(1, || async {
					panic!("hydrated normalized query must not fetch")
				})
				.with_entities(EntityValue::new());
			let mut state = SsrState::new();
			state.add_resource_state(ENTITY_TABLE_HYDRATION_ID, table(project(7, "hydrated")));
			state.add_resource_state(descriptor.key().hydration_id(), snapshot(7));
			let mut hydration = HydrationContext::from_state(state);

			hydration
				.seed_query_descriptor(&client, &descriptor)
				.expect("normalized recipe should hydrate");
			let query = client.observe(descriptor, QueryOptions::default());
			assert_eq!(query.data(), Some(project(7, "hydrated")));
			assert_eq!(runtime.pending_task_count(), 0);
		});
	}

	#[test]
	fn normalized_hydration_preserves_an_existing_live_entry() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let descriptor = QueryFamily::<u64, Project, String>::new(
				"tests.normalized-hydration-existing-entry",
			)
			.query(1, || std::future::pending::<Result<Project, String>>())
			.with_entities(EntityValue::new());
			let existing = client.observe(descriptor.clone(), QueryOptions::default());
			let mut state = SsrState::new();
			state.add_resource_state(ENTITY_TABLE_HYDRATION_ID, table(project(7, "server")));
			state.add_resource_state(descriptor.key().hydration_id(), snapshot(7));

			HydrationContext::from_state(state)
				.seed_query_descriptor(&client, &descriptor)
				.expect("the live normalized entry should be retained");
			let retained = client.observe(descriptor, QueryOptions::default());

			assert!(Rc::ptr_eq(&existing.entry, &retained.entry));
			assert!(existing.entry.request.borrow().is_some());
		});
	}

	#[test]
	fn late_hydration_cannot_overwrite_an_earlier_client_mutation() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			client.upsert_entity(project(7, "client"));

			client.install_entity_hydration_envelope(
				serde_json::from_value(table(project(7, "server"))).unwrap(),
			);

			assert_eq!(
				client.entity::<Project>(7).get(),
				Some(project(7, "client"))
			);
		});
	}

	#[test]
	fn normalized_hydration_rejects_missing_required_entity() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let family =
				QueryFamily::<u64, Project, String>::new("tests.normalized-hydration-missing");
			let descriptor = family
				.query(1, || async { Ok::<_, String>(project(7, "fetch")) })
				.with_entities(EntityValue::new());
			let mut state = SsrState::new();
			state.add_resource_state(descriptor.key().hydration_id(), snapshot(7));
			let mut hydration = HydrationContext::from_state(state);
			let error = hydration
				.seed_query_descriptor(&client, &descriptor)
				.expect_err("required entity omission must reject the recipe");
			assert!(error.to_string().contains("missing required entity"));
		});
	}

	#[test]
	fn normalized_initial_error_round_trips_without_a_fetch() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let family =
				QueryFamily::<u64, Project, String>::new("tests.normalized-hydration-error");
			let descriptor = family
				.query(1, || async {
					panic!("hydrated normalized errors must not fetch while disabled")
				})
				.with_entities(EntityValue::new());
			let mut state = SsrState::new();
			state.add_resource_state(
				descriptor.key().hydration_id(),
				serde_json::json!({
					"version": 1,
					"kind": "error",
					"schema": "entity-value-v1",
					"state": { "Error": "offline" },
					"refetch_error": null,
					"is_fetching": false,
					"is_stale": false,
				}),
			);
			let mut hydration = HydrationContext::from_state(state);
			hydration
				.seed_query_descriptor(&client, &descriptor)
				.expect("normalized initial errors should hydrate");

			let query = client.observe(descriptor, QueryOptions::new().enabled(false));
			assert_eq!(
				query.snapshot(),
				QuerySnapshot {
					status: QueryStatus::Error,
					data: None,
					error: Some("offline".to_string()),
					refetch_error: None,
					is_fetching: false,
					is_stale: false,
				}
			);
			assert_eq!(runtime.pending_task_count(), 0);
		});
	}

	#[test]
	fn normalized_snapshot_rejects_unsupported_version_and_schema() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let family =
				QueryFamily::<u64, Project, String>::new("tests.normalized-hydration-contract");
			let descriptor = family
				.query(1, || async { Ok::<_, String>(project(7, "unused")) })
				.with_entities(EntityValue::new());

			let mut wrong_version = snapshot(7);
			wrong_version["version"] = serde_json::json!(2);
			let mut version_state = SsrState::new();
			version_state.add_resource_state(descriptor.key().hydration_id(), wrong_version);
			let version_error = HydrationContext::from_state(version_state)
				.seed_query_descriptor(&client, &descriptor)
				.expect_err("unsupported normalized snapshot versions must be rejected");
			assert_eq!(
				version_error.to_string(),
				"normalized query hydration snapshot has an unsupported version"
			);

			let mut wrong_schema = snapshot(7);
			wrong_schema["schema"] = serde_json::json!("wrong-schema-v1");
			let mut schema_state = SsrState::new();
			schema_state.add_resource_state(descriptor.key().hydration_id(), wrong_schema);
			let schema_error = HydrationContext::from_state(schema_state)
				.seed_query_descriptor(&client, &descriptor)
				.expect_err("normalized snapshot schemas must match the descriptor");
			assert_eq!(
				schema_error.to_string(),
				"normalized query hydration snapshot schema does not match the query projection"
			);
		});
	}

	#[test]
	fn normalized_hydration_rejects_a_row_value_id_mismatch() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let family =
				QueryFamily::<u64, Project, String>::new("tests.normalized-hydration-row-id");
			let descriptor = family
				.query(1, || async {
					panic!("row-ID mismatch must fail before fetching")
				})
				.with_entities(EntityValue::new());
			client.install_entity_hydration_envelope(EntityHydrationEnvelope {
				version: ENTITY_TABLE_VERSION,
				entities: BTreeMap::from([(
					Project::TYPE.to_string(),
					vec![EntityHydrationRow {
						id: serde_json::json!(7),
						value: serde_json::to_value(project(8, "wrong-id")).unwrap(),
					}],
				)]),
			});
			let mut state = SsrState::new();
			state.add_resource_state(descriptor.key().hydration_id(), snapshot(7));
			let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
				HydrationContext::from_state(state)
					.seed_query_descriptor(&client, &descriptor)
					.expect("row-ID mismatch must panic during typed validation");
			}))
			.expect_err("a mismatched row ID must be rejected");
			assert_eq!(
				panic_message(panic),
				format!(
					"entity hydration loader for TYPE `{}` received an entity whose ID differs from its hydration record",
					Project::TYPE
				)
			);
		});
	}

	#[test]
	fn normalized_hydration_rejects_a_malformed_entity_value() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let family =
				QueryFamily::<u64, Project, String>::new("tests.normalized-hydration-entity-value");
			let descriptor = family
				.query(1, || async {
					panic!("malformed entity values must fail before fetching")
				})
				.with_entities(EntityValue::new());
			client.install_entity_hydration_envelope(EntityHydrationEnvelope {
				version: ENTITY_TABLE_VERSION,
				entities: BTreeMap::from([(
					Project::TYPE.to_string(),
					vec![EntityHydrationRow {
						id: serde_json::json!(7),
						value: serde_json::json!({"id": 7}),
					}],
				)]),
			});
			let mut state = SsrState::new();
			state.add_resource_state(descriptor.key().hydration_id(), snapshot(7));
			let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
				HydrationContext::from_state(state)
					.seed_query_descriptor(&client, &descriptor)
					.expect("malformed entity values must panic during typed validation");
			}))
			.expect_err("malformed entity values must be rejected");
			assert_eq!(
				panic_message(panic),
				format!(
					"entity hydration loader for TYPE `{}` failed to deserialize entity type `{}`: missing field `name`",
					Project::TYPE,
					std::any::type_name::<Project>()
				)
			);
		});
	}

	#[test]
	fn normalized_hydration_rejects_a_malformed_recipe() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let family =
				QueryFamily::<u64, Project, String>::new("tests.normalized-hydration-recipe");
			let descriptor = family
				.query(1, || async {
					panic!("malformed recipes must fail before fetching")
				})
				.with_entities(EntityValue::new());
			client.install_entity_hydration_envelope(
				serde_json::from_value(table(project(7, "recipe"))).unwrap(),
			);
			let mut malformed = snapshot(7);
			malformed["state"]["Success"]["projection"] = serde_json::json!("bad-recipe");
			let mut state = SsrState::new();
			state.add_resource_state(descriptor.key().hydration_id(), malformed);
			let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
				HydrationContext::from_state(state)
					.seed_query_descriptor(&client, &descriptor)
					.expect("malformed recipes must panic during recipe validation");
			}))
			.expect_err("malformed recipes must be rejected");
			assert_eq!(
				panic_message(panic),
				format!(
					"entity projection adapter `{}` for query family `{}` with schema `entity-value-v1` failed to deserialize its recipe: invalid type: string \"bad-recipe\", expected u64",
					std::any::type_name::<EntityValue<Project>>(),
					family.id()
				)
			);
		});
	}

	#[test]
	fn ssr_entity_hydration_is_isolated_per_query_client() {
		ReactiveScope::run(|| {
			let first = QueryClient::new_ssr(QueryDefaults::default());
			first.upsert_entity(project(1, "first-request"));
			assert_eq!(
				first.entity::<Project>(1).get(),
				Some(project(1, "first-request"))
			);

			let second = QueryClient::new_ssr(QueryDefaults::default());
			second.upsert_entity(project(2, "second-request"));
			assert_eq!(
				second.entity::<Project>(2).get(),
				Some(project(2, "second-request"))
			);

			assert_eq!(
				serde_json::to_value(first.reachable_entity_hydration_envelope()).unwrap(),
				serde_json::json!({
					"version": 1,
					"entities": {
						Project::TYPE: [{"id": 1, "value": {"id": 1, "name": "first-request"}}]
					}
				})
			);
			assert_eq!(
				serde_json::to_value(second.reachable_entity_hydration_envelope()).unwrap(),
				serde_json::json!({
					"version": 1,
					"entities": {
						Project::TYPE: [{"id": 2, "value": {"id": 2, "name": "second-request"}}]
					}
				})
			);
		});
	}

	#[test]
	fn ssr_entity_handle_reads_mark_only_present_reachable_rows() {
		ReactiveScope::run(|| {
			let client = QueryClient::new_ssr(QueryDefaults::default());
			client.upsert_entity(project(1, "reachable"));
			client.upsert_entity(project(2, "unread"));
			let first = client.entity::<Project>(1);
			let second = client.entity::<Project>(1);
			assert_eq!(first.get(), Some(project(1, "reachable")));
			assert_eq!(second.get(), Some(project(1, "reachable")));
			assert!(client.has_ssr_entity_reads());
			let envelope = client.reachable_entity_hydration_envelope();
			assert_eq!(envelope.entities[Project::TYPE].len(), 1);
			assert_eq!(envelope.entities[Project::TYPE][0].id, serde_json::json!(1));
		});
	}

	#[test]
	fn ssr_entity_table_keeps_raw_ids_separate_by_type() {
		ReactiveScope::run(|| {
			let client = QueryClient::new_ssr(QueryDefaults::default());
			client.upsert_entity(project(1, "project"));
			client.upsert_entity(Task {
				id: 1,
				name: "task".to_string(),
			});
			assert_eq!(
				client.entity::<Project>(1).get(),
				Some(project(1, "project"))
			);
			assert_eq!(
				client.entity::<Task>(1).get(),
				Some(Task {
					id: 1,
					name: "task".to_string()
				})
			);
			let envelope = client.reachable_entity_hydration_envelope();
			assert_eq!(envelope.entities[Project::TYPE].len(), 1);
			assert_eq!(envelope.entities[Task::TYPE].len(), 1);
		});
	}

	#[test]
	fn normalized_entity_type_group_is_consumed_once_and_reused() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let family =
				QueryFamily::<u64, Project, String>::new("tests.normalized-hydration-once");
			let first = family
				.query(1, || async {
					panic!("first hydrated query must not fetch")
				})
				.with_entities(EntityValue::new());
			let second = family
				.query(2, || async {
					panic!("second hydrated query must not fetch")
				})
				.with_entities(EntityValue::new());
			client.install_entity_hydration_envelope(
				serde_json::from_value(table(project(7, "shared"))).unwrap(),
			);
			let mut first_state = SsrState::new();
			first_state.add_resource_state(first.key().hydration_id(), snapshot(7));
			HydrationContext::from_state(first_state)
				.seed_query_descriptor(&client, &first)
				.expect("first recipe should consume the raw type group");
			let mut second_state = SsrState::new();
			second_state.add_resource_state(second.key().hydration_id(), snapshot(7));
			HydrationContext::from_state(second_state)
				.seed_query_descriptor(&client, &second)
				.expect("second recipe should reuse the typed bucket");
			assert_eq!(
				client.observe(first, QueryOptions::default()).data(),
				Some(project(7, "shared"))
			);
			assert_eq!(
				client.observe(second, QueryOptions::default()).data(),
				Some(project(7, "shared"))
			);
		});
	}

	#[test]
	fn normalized_entity_type_group_hydrates_distinct_ids_for_separate_recipes() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let family =
				QueryFamily::<u64, Project, String>::new("tests.normalized-hydration-distinct-ids");
			let first = family
				.query(1, || async {
					panic!("first hydrated query must not fetch")
				})
				.with_entities(EntityValue::new());
			let second = family
				.query(2, || async {
					panic!("second hydrated query must not fetch")
				})
				.with_entities(EntityValue::new());
			client.install_entity_hydration_envelope(EntityHydrationEnvelope {
				version: ENTITY_TABLE_VERSION,
				entities: BTreeMap::from([(
					Project::TYPE.to_string(),
					vec![
						EntityHydrationRow {
							id: serde_json::json!(7),
							value: serde_json::to_value(project(7, "first")).unwrap(),
						},
						EntityHydrationRow {
							id: serde_json::json!(8),
							value: serde_json::to_value(project(8, "second")).unwrap(),
						},
					],
				)]),
			});

			let mut first_state = SsrState::new();
			first_state.add_resource_state(first.key().hydration_id(), snapshot(7));
			HydrationContext::from_state(first_state)
				.seed_query_descriptor(&client, &first)
				.expect("first recipe should hydrate its declared row");
			let mut second_state = SsrState::new();
			second_state.add_resource_state(second.key().hydration_id(), snapshot(8));
			HydrationContext::from_state(second_state)
				.seed_query_descriptor(&client, &second)
				.expect("second recipe should reuse the fully hydrated TYPE group");

			assert_eq!(
				client.observe(first, QueryOptions::default()).data(),
				Some(project(7, "first"))
			);
			assert_eq!(
				client.observe(second, QueryOptions::default()).data(),
				Some(project(8, "second"))
			);
		});
	}

	#[test]
	fn hydration_installed_before_a_client_write_cannot_overwrite_that_write() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let descriptor =
				QueryFamily::<u64, Project, String>::new("tests.normalized-hydration-write-order")
					.query(1, || async { panic!("hydrated query must not fetch") })
					.with_entities(EntityValue::new());
			let _registered = client.entity::<Project>(7);
			client.install_entity_hydration_envelope(
				serde_json::from_value(table(project(7, "server"))).unwrap(),
			);
			client.upsert_entity(project(7, "client"));
			let mut state = SsrState::new();
			state.add_resource_state(descriptor.key().hydration_id(), snapshot(7));

			HydrationContext::from_state(state)
				.seed_query_descriptor(&client, &descriptor)
				.expect("the normalized recipe should seed from the newer client record");

			assert_eq!(
				client.observe(descriptor, QueryOptions::default()).data(),
				Some(project(7, "client"))
			);
		});
	}

	#[test]
	fn hydration_keeps_unaffected_rows_when_one_row_is_stale() {
		ReactiveScope::run(|| {
			let client = QueryClient::with_runtime(
				QueryDefaults::default(),
				TestQueryRuntime::new().handle(),
			);
			client.upsert_entity(project(7, "client"));
			client.install_entity_hydration_envelope(EntityHydrationEnvelope {
				version: ENTITY_TABLE_VERSION,
				entities: BTreeMap::from([(
					Project::TYPE.to_string(),
					vec![
						EntityHydrationRow {
							id: serde_json::json!(7),
							value: serde_json::to_value(project(7, "server")).unwrap(),
						},
						EntityHydrationRow {
							id: serde_json::json!(8),
							value: serde_json::to_value(project(8, "unaffected")).unwrap(),
						},
					],
				)]),
			});

			assert_eq!(
				client.entity::<Project>(7).get(),
				Some(project(7, "client"))
			);
			assert_eq!(
				client.entity::<Project>(8).get(),
				Some(project(8, "unaffected"))
			);
		});
	}

	#[test]
	fn installing_hydration_populates_an_already_registered_entity_bucket() {
		ReactiveScope::run(|| {
			let client = QueryClient::with_runtime(
				QueryDefaults::default(),
				TestQueryRuntime::new().handle(),
			);
			let entity = client.entity::<Project>(7);

			client.install_entity_hydration_envelope(
				serde_json::from_value(table(project(7, "hydrated"))).unwrap(),
			);

			assert_eq!(entity.get(), Some(project(7, "hydrated")));
		});
	}

	#[test]
	fn deferred_recipe_can_rehydrate_an_unclaimed_row_after_gc() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(
				QueryDefaults::default().gc_time(Duration::ZERO),
				runtime.handle(),
			);
			let family = QueryFamily::<u64, Project, String>::new(
				"tests.normalized-hydration-deferred-recipe",
			);
			let first = family
				.query(1, || async {
					panic!("first hydrated query must not fetch")
				})
				.with_entities(EntityValue::new());
			let second = family
				.query(2, || async {
					panic!("second hydrated query must not fetch")
				})
				.with_entities(EntityValue::new());
			client.install_entity_hydration_envelope(EntityHydrationEnvelope {
				version: ENTITY_TABLE_VERSION,
				entities: BTreeMap::from([(
					Project::TYPE.to_string(),
					vec![
						EntityHydrationRow {
							id: serde_json::json!(7),
							value: serde_json::to_value(project(7, "first")).unwrap(),
						},
						EntityHydrationRow {
							id: serde_json::json!(8),
							value: serde_json::to_value(project(8, "second")).unwrap(),
						},
					],
				)]),
			});
			let mut first_state = SsrState::new();
			first_state.add_resource_state(first.key().hydration_id(), snapshot(7));
			HydrationContext::from_state(first_state)
				.seed_query_descriptor(&client, &first)
				.expect("the first recipe should seed");
			runtime.run_due_maintenance();
			assert!(
				!client
					.entity_arena_for_test()
					.entity_record_exists_for_test::<Project>(&8)
			);
			let mut second_state = SsrState::new();
			second_state.add_resource_state(second.key().hydration_id(), snapshot(8));

			HydrationContext::from_state(second_state)
				.seed_query_descriptor(&client, &second)
				.expect("the deferred recipe should rehydrate its retained row");

			assert_eq!(
				client.observe(second, QueryOptions::default()).data(),
				Some(project(8, "second"))
			);
		});
	}

	#[test]
	#[serial(entity_gc)]
	fn client_tombstone_removes_a_retained_hydration_row_before_gc() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(
				QueryDefaults::default().gc_time(Duration::ZERO),
				runtime.handle(),
			);
			let family =
				QueryFamily::<u64, Project, String>::new("tests.normalized-hydration-tombstone-gc");
			let first = family
				.query(1, || async {
					panic!("first hydrated query must not fetch")
				})
				.with_entities(EntityValue::new());
			let second = family
				.query(2, || async {
					panic!("second hydrated query must not fetch")
				})
				.with_entities(EntityValue::new());
			client.install_entity_hydration_envelope(EntityHydrationEnvelope {
				version: ENTITY_TABLE_VERSION,
				entities: BTreeMap::from([(
					Project::TYPE.to_string(),
					vec![
						EntityHydrationRow {
							id: serde_json::json!(7),
							value: serde_json::to_value(project(7, "first")).unwrap(),
						},
						EntityHydrationRow {
							id: serde_json::json!(8),
							value: serde_json::to_value(project(8, "second")).unwrap(),
						},
					],
				)]),
			});

			let mut first_state = SsrState::new();
			first_state.add_resource_state(first.key().hydration_id(), snapshot(7));
			HydrationContext::from_state(first_state)
				.seed_query_descriptor(&client, &first)
				.expect("the first recipe should seed");

			client.remove_entity::<Project>(&8);
			runtime.run_due_maintenance();
			assert!(
				!client
					.entity_arena_for_test()
					.entity_record_exists_for_test::<Project>(&8)
			);

			let mut second_state = SsrState::new();
			second_state.add_resource_state(second.key().hydration_id(), snapshot(8));
			let error = HydrationContext::from_state(second_state)
				.seed_query_descriptor(&client, &second)
				.expect_err("a tombstoned hydration row must not be replayed");
			assert!(error.to_string().contains("missing required entity"));
		});
	}

	#[test]
	fn hydration_rows_preserve_wide_integer_values() {
		ReactiveScope::run(|| {
			let client = QueryClient::new_ssr(QueryDefaults::default());
			let value = WideProject { id: u64::MAX };
			client.upsert_entity(value.clone());
			assert_eq!(client.entity::<WideProject>(u64::MAX).get(), Some(value));

			let envelope = client.reachable_entity_hydration_envelope();
			let row = &envelope
				.entities
				.get(WideProject::TYPE)
				.expect("wide entity type should be serialized")[0];
			assert_eq!(row.id, serde_json::json!(u64::MAX));
			assert_eq!(row.value, serde_json::json!({ "id": u64::MAX }));
		});
	}

	#[test]
	fn malformed_entity_table_duplicate_identity_is_rejected() {
		ReactiveScope::run(|| {
			let client = QueryClient::with_runtime(
				QueryDefaults::default(),
				TestQueryRuntime::new().handle(),
			);
			let duplicate = EntityHydrationEnvelope {
				version: ENTITY_TABLE_VERSION,
				entities: BTreeMap::from([(
					Project::TYPE.to_string(),
					vec![
						EntityHydrationRow {
							id: serde_json::json!(1),
							value: serde_json::json!({"id": 1, "name": "first"}),
						},
						EntityHydrationRow {
							id: serde_json::json!(1),
							value: serde_json::json!({"id": 1, "name": "duplicate"}),
						},
					],
				)]),
			};
			let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
				client.install_entity_hydration_envelope(duplicate);
			}))
			.expect_err("duplicate hydration identities must panic");
			let message = panic
				.downcast_ref::<String>()
				.map(String::as_str)
				.or_else(|| panic.downcast_ref::<&str>().copied())
				.unwrap_or_default();
			assert!(message.contains("duplicate identity"));
		});
	}
}

mod entity_removal {
	use super::normalized::{Project, project};
	use super::*;
	use crate::reactive::entity::{EntityVec, OptionalEntity};

	thread_local! {
		static AUTHORITATIVE_REMOVAL_CALLED: Cell<bool> = const { Cell::new(false) };
	}

	#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
	struct Task {
		id: u64,
		name: String,
	}

	impl Entity for Task {
		type Id = u64;

		const TYPE: &'static str = "reactive.query.tests.entity-removal-task";

		fn entity_id(&self) -> Self::Id {
			self.id
		}
	}

	fn task(id: u64, name: &str) -> Task {
		Task {
			id,
			name: name.to_string(),
		}
	}

	#[derive(Clone, Copy)]
	struct RemoveUnprojectedEntity;

	#[derive(Clone, Debug, Deserialize, Serialize)]
	struct RemovalAuthoritativeRecipe {
		id: u64,
		missing: bool,
	}

	#[derive(Clone, Copy)]
	struct RemovalAuthoritativeProjection;

	impl EntityProjection<Project> for RemovalAuthoritativeProjection {
		type Recipe = RemovalAuthoritativeRecipe;

		const SCHEMA: &'static str = "removal-authoritative-v1";

		fn normalize(&self, value: Project, entities: &mut EntityWriter<'_>) -> Self::Recipe {
			let id = value.entity_id();
			entities.upsert(value);
			RemovalAuthoritativeRecipe { id, missing: false }
		}

		fn dependencies(&self, recipe: &Self::Recipe, dependencies: &mut EntityDependencies) {
			dependencies.extend::<Project>([recipe.id]);
		}

		fn materialize(
			&self,
			recipe: &Self::Recipe,
			_entities: &EntityReader<'_>,
		) -> ProjectionMaterialization<Project> {
			ProjectionMaterialization::Ready(project(
				recipe.id,
				if recipe.missing { "invalid" } else { "valid" },
			))
		}

		fn apply_removals(
			&self,
			recipe: &mut Self::Recipe,
			removed: &RemovedEntities<'_>,
		) -> ProjectionRemoval {
			if removed.contains::<Project>(&recipe.id) {
				AUTHORITATIVE_REMOVAL_CALLED.with(|called| called.set(true));
				recipe.missing = true;
				ProjectionRemoval::MissingRequired
			} else {
				ProjectionRemoval::Unchanged
			}
		}
	}

	impl EntityProjection<Project> for RemoveUnprojectedEntity {
		type Recipe = u64;

		const SCHEMA: &'static str = "remove-unprojected-entity-v1";

		fn normalize(&self, value: Project, entities: &mut EntityWriter<'_>) -> Self::Recipe {
			entities.remove::<Project>(&2);
			let id = value.entity_id();
			entities.upsert(value);
			id
		}

		fn dependencies(&self, recipe: &Self::Recipe, dependencies: &mut EntityDependencies) {
			dependencies.extend::<Project>([*recipe]);
		}

		fn materialize(
			&self,
			recipe: &Self::Recipe,
			entities: &EntityReader<'_>,
		) -> ProjectionMaterialization<Project> {
			entities.required::<Project>(recipe)
		}

		fn apply_removals(
			&self,
			recipe: &mut Self::Recipe,
			removed: &RemovedEntities<'_>,
		) -> ProjectionRemoval {
			if removed.contains::<Project>(recipe) {
				ProjectionRemoval::MissingRequired
			} else {
				ProjectionRemoval::Unchanged
			}
		}
	}

	#[test]
	#[serial(entity_removal)]
	fn normalization_propagates_a_tombstone_outside_its_own_recipe() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let optional = client.observe(
				QueryFamily::<(), Option<Project>, String>::new(
					"tests.normalization-unprojected-tombstone-dependent",
				)
				.query((), || async { Ok(Some(project(2, "remove me"))) })
				.with_entities(OptionalEntity::new()),
				QueryOptions::new(),
			);
			runtime.run_until_stalled();

			let _source = client.observe(
				QueryFamily::<(), Project, String>::new(
					"tests.normalization-unprojected-tombstone-source",
				)
				.query((), || async { Ok(project(1, "keep me")) })
				.with_entities(RemoveUnprojectedEntity),
				QueryOptions::new(),
			);
			runtime.run_until_stalled();

			assert_eq!(optional.data(), Some(None));
		});
	}

	#[test]
	#[serial(entity_removal)]
	fn missing_required_removal_does_not_publish_a_materialized_candidate() {
		ReactiveScope::run(|| {
			AUTHORITATIVE_REMOVAL_CALLED.with(|called| called.set(false));
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let query = client.observe(
				QueryFamily::<(), Project, String>::new("tests.removal-authoritative")
					.query((), || async { Ok(project(1, "fetched")) })
					.with_entities(RemovalAuthoritativeProjection),
				QueryOptions::new(),
			);
			runtime.run_until_stalled();
			assert_eq!(query.data(), Some(project(1, "valid")));

			client.remove_entity::<Project>(&1);

			assert!(AUTHORITATIVE_REMOVAL_CALLED.with(Cell::get));
			assert_eq!(query.data(), Some(project(1, "valid")));
		});
	}

	struct RemovalGate {
		ready: Rc<Cell<bool>>,
		reset_ready_on_completion: bool,
		result: Option<Result<Project, String>>,
	}

	impl Future for RemovalGate {
		type Output = Result<Project, String>;

		fn poll(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Self::Output> {
			let this = self.get_mut();
			if this.ready.get() {
				let result = this
					.result
					.take()
					.expect("removal gate polled after completion");
				if this.reset_ready_on_completion {
					this.ready.set(false);
				}
				Poll::Ready(result)
			} else {
				Poll::Pending
			}
		}
	}

	#[test]
	#[serial(entity_removal)]
	fn direct_entity_handle_publishes_none_after_removal() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(
				QueryDefaults::default().stale_time(Duration::from_secs(60)),
				runtime.handle(),
			);
			let entity = client.entity::<Project>(1);

			client.upsert_entity(project(1, "present"));
			assert_eq!(entity.get(), Some(project(1, "present")));

			client.remove_entity::<Project>(&1);
			assert_eq!(entity.get(), None);
		});
	}

	#[test]
	#[serial(entity_removal)]
	fn required_removal_retains_stale_value_and_starts_one_active_refetch() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(
				QueryDefaults::default().stale_time(Duration::from_secs(60)),
				runtime.handle(),
			);
			let calls = Rc::new(Cell::new(0));
			let query = client.observe(
				QueryFamily::<(), Project, String>::new("tests.entity-removal-required-active")
					.query((), {
						let calls = Rc::clone(&calls);
						move || {
							calls.set(calls.get() + 1);
							async { Ok(project(1, "refetched")) }
						}
					})
					.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);
			runtime.run_until_stalled();
			assert_eq!(query.data(), Some(project(1, "refetched")));
			assert_eq!(calls.get(), 1);

			client.remove_entity::<Project>(&1);

			assert_eq!(query.snapshot().status, QueryStatus::Success);
			assert_eq!(query.data(), Some(project(1, "refetched")));
			assert!(query.is_stale());
			assert!(query.is_fetching());
			assert_eq!(runtime.pending_task_count(), 1);

			runtime.run_until_stalled();
			assert_eq!(calls.get(), 2);
			assert_eq!(query.data(), Some(project(1, "refetched")));
			assert!(!query.is_stale());
		});
	}

	#[test]
	#[serial(entity_removal)]
	fn inactive_and_disabled_required_queries_wait_for_enabled_mount_or_manual_refetch() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let family = QueryFamily::<(), Project, String>::new("tests.entity-removal-disabled");
			let calls = Rc::new(Cell::new(0));
			let descriptor = family
				.query((), {
					let calls = Rc::clone(&calls);
					move || {
						calls.set(calls.get() + 1);
						async { Ok(project(1, "fresh")) }
					}
				})
				.with_entities(EntityValue::new());
			let active = client.observe(descriptor.clone(), QueryOptions::new());
			runtime.run_until_stalled();
			drop(active);

			client.remove_entity::<Project>(&1);
			assert_eq!(runtime.pending_task_count(), 0);

			let disabled = client.observe(descriptor.clone(), QueryOptions::new().enabled(false));
			assert_eq!(disabled.data(), Some(project(1, "fresh")));
			assert!(disabled.is_stale());
			assert_eq!(runtime.pending_task_count(), 0);

			disabled.refetch();
			assert_eq!(runtime.pending_task_count(), 1);
			runtime.run_until_stalled();
			assert_eq!(calls.get(), 2);

			client.remove_entity::<Project>(&1);
			assert_eq!(runtime.pending_task_count(), 0);
			let enabled = client.observe(descriptor, QueryOptions::new());
			assert_eq!(runtime.pending_task_count(), 1);
			runtime.run_until_stalled();
			assert_eq!(calls.get(), 3);
			assert_eq!(enabled.data(), Some(project(1, "fresh")));
		});
	}

	#[test]
	#[serial(entity_removal)]
	fn optional_and_vector_removals_permanently_update_recipes_and_dependencies() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let optional = client.observe(
				QueryFamily::<(), Option<Project>, String>::new("tests.entity-removal-optional")
					.query((), || async { Ok(Some(project(1, "optional"))) })
					.with_entities(OptionalEntity::new()),
				QueryOptions::new(),
			);
			let vector = client.observe(
				QueryFamily::<(), Vec<Project>, String>::new("tests.entity-removal-vector")
					.query((), || async {
						Ok(vec![
							project(1, "first"),
							project(2, "second"),
							project(1, "first"),
						])
					})
					.with_entities(EntityVec::new()),
				QueryOptions::new(),
			);
			runtime.run_until_stalled();

			client.remove_entity::<Project>(&1);
			assert!(
				client
					.normalized_recipe_refreshes()
					.into_iter()
					.any(|(_, refresh)| matches!(
						refresh,
						NormalizedRecipeRefresh::Success(recipe)
							if recipe == serde_json::json!([2])
					))
			);

			assert_eq!(optional.data(), Some(None));
			assert_eq!(vector.data(), Some(vec![project(2, "second")]));
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&1),
				0
			);
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&2),
				1
			);

			client.upsert_entity(project(1, "later"));
			assert_eq!(optional.data(), Some(None));
			assert_eq!(vector.data(), Some(vec![project(2, "second")]));
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&1),
				0
			);
		});
	}

	#[test]
	#[serial(entity_removal)]
	fn removing_a_project_preserves_related_tasks_until_staged_explicitly() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let project_handle = client.entity::<Project>(1);
			let task_handle = client.entity::<Task>(1);
			client.update_entities(|entities| {
				entities.upsert(project(1, "project"));
				entities.upsert(task(1, "task"));
			});

			client.remove_entity::<Project>(&1);
			assert_eq!(project_handle.get(), None);
			assert_eq!(task_handle.get(), Some(task(1, "task")));

			client.update_entities(|entities| {
				entities.remove::<Project>(&1);
				entities.remove::<Task>(&1);
			});
			assert_eq!(task_handle.get(), None);
		});
	}

	#[test]
	#[serial(entity_removal)]
	fn tombstoned_first_fetch_does_not_publish_a_rejected_value() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(
				QueryDefaults::default().stale_time(Duration::from_secs(60)),
				runtime.handle(),
			);
			let ready = Rc::new(Cell::new(false));
			let calls = Rc::new(Cell::new(0));
			let query = client.observe(
				QueryFamily::<(), Project, String>::new("tests.entity-removal-first-fetch-race")
					.query((), {
						let ready = Rc::clone(&ready);
						let calls = Rc::clone(&calls);
						move || {
							let call = calls.get();
							calls.set(call + 1);
							RemovalGate {
								ready: Rc::clone(&ready),
								reset_ready_on_completion: call == 0,
								result: Some(Ok(project(
									1,
									if call == 0 { "older" } else { "fresh" },
								))),
							}
						}
					})
					.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);

			client.remove_entity::<Project>(&1);
			ready.set(true);
			runtime.run_until_stalled();

			assert_eq!(query.snapshot().status, QueryStatus::Pending);
			assert_eq!(query.data(), None);
			assert!(query.is_fetching());
			assert!(query.is_stale());
			assert_eq!(client.entity::<Project>(1).get(), None);
			assert_eq!(runtime.pending_task_count(), 1);
			assert_eq!(calls.get(), 2);

			ready.set(true);
			runtime.run_until_stalled();
			assert_eq!(calls.get(), 2);
			assert_eq!(query.data(), Some(project(1, "fresh")));
			assert!(!query.is_stale());
		});
	}

	#[test]
	#[serial(entity_removal)]
	fn tombstoned_refetch_retains_an_existing_successful_value() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(
				QueryDefaults::default().stale_time(Duration::from_secs(60)),
				runtime.handle(),
			);
			let ready = Rc::new(Cell::new(true));
			let calls = Rc::new(Cell::new(0));
			let descriptor = QueryFamily::<(), Project, String>::new(
				"tests.entity-removal-existing-success-race",
			)
			.query((), {
				let ready = Rc::clone(&ready);
				let calls = Rc::clone(&calls);
				move || {
					let call = calls.get();
					calls.set(call + 1);
					RemovalGate {
						ready: Rc::clone(&ready),
						reset_ready_on_completion: call == 1,
						result: Some(Ok(project(
							1,
							if call == 0 { "initial" } else { "in-flight" },
						))),
					}
				}
			})
			.with_entities(EntityValue::new());
			let query = client.observe(descriptor.clone(), QueryOptions::new());
			runtime.run_until_stalled();
			assert_eq!(query.data(), Some(project(1, "initial")));

			ready.set(false);
			query.refetch();
			runtime.run_until_stalled();
			assert!(query.is_fetching());
			client.remove_entity::<Project>(&1);
			ready.set(true);
			runtime.run_until_stalled();

			assert_eq!(query.snapshot().status, QueryStatus::Success);
			assert_eq!(query.data(), Some(project(1, "initial")));
			assert!(query.is_stale());
			assert_eq!(calls.get(), 3);
			assert_eq!(runtime.pending_task_count(), 1);
		});
	}

	#[test]
	#[serial(entity_removal)]
	fn missing_completion_and_invalidation_start_only_one_follow_up_refetch() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(
				QueryDefaults::default().stale_time(Duration::from_secs(60)),
				runtime.handle(),
			);
			let ready = Rc::new(Cell::new(false));
			let calls = Rc::new(Cell::new(0));
			let family = QueryFamily::<(), Project, String>::new(
				"tests.entity-removal-missing-invalidation-race",
			);
			let descriptor = family
				.query((), {
					let ready = Rc::clone(&ready);
					let calls = Rc::clone(&calls);
					move || {
						let call = calls.get();
						calls.set(call + 1);
						RemovalGate {
							ready: Rc::clone(&ready),
							reset_ready_on_completion: call == 0,
							result: Some(Ok(project(1, if call == 0 { "older" } else { "fresh" }))),
						}
					}
				})
				.with_entities(EntityValue::new());
			let query = client.observe(descriptor.clone(), QueryOptions::new());

			client.remove_entity::<Project>(&1);
			client.invalidate(descriptor.key());
			ready.set(true);
			runtime.run_until_stalled();

			assert_eq!(calls.get(), 2);
			assert_eq!(runtime.pending_task_count(), 1);

			runtime.run_until_stalled();
			assert_eq!(calls.get(), 2);
			assert_eq!(runtime.pending_task_count(), 1);

			ready.set(true);
			runtime.run_until_stalled();
			assert_eq!(calls.get(), 2);
			assert_eq!(query.data(), Some(project(1, "fresh")));
			assert!(!query.is_stale());
		});
	}

	#[test]
	#[serial(entity_removal)]
	fn canceled_recovery_allows_remount_follow_up_after_newer_tombstone() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(
				QueryDefaults::default().stale_time(Duration::from_secs(60)),
				runtime.handle(),
			);
			let ready = Rc::new(Cell::new(true));
			let calls = Rc::new(Cell::new(0));
			let descriptor = QueryFamily::<(), Project, String>::new(
				"tests.entity-removal-canceled-recovery-remount",
			)
			.query((), {
				let ready = Rc::clone(&ready);
				let calls = Rc::clone(&calls);
				move || {
					let call = calls.get();
					calls.set(call + 1);
					RemovalGate {
						ready: Rc::clone(&ready),
						reset_ready_on_completion: call == 0 || call == 2,
						result: Some(Ok(project(
							1,
							match call {
								0 => "initial",
								1 => "canceled",
								2 => "stale",
								_ => "fresh",
							},
						))),
					}
				}
			})
			.with_entities(EntityValue::new());

			let active = client.observe(descriptor.clone(), QueryOptions::new());
			runtime.run_until_stalled();
			assert_eq!(calls.get(), 1);

			ready.set(false);
			client.remove_entity::<Project>(&1);
			assert!(active.is_stale());
			assert!(active.is_fetching());
			assert_eq!(runtime.pending_task_count(), 1);
			runtime.run_until_stalled();
			assert_eq!(calls.get(), 2);
			assert_eq!(runtime.pending_task_count(), 1);
			drop(active);
			runtime.run_until_stalled();
			assert_eq!(calls.get(), 2);

			let remounted = client.observe(descriptor, QueryOptions::new());
			assert!(remounted.is_stale());
			assert!(remounted.is_fetching());
			assert_eq!(runtime.pending_task_count(), 1);
			runtime.run_until_stalled();
			assert_eq!(calls.get(), 3);
			assert_eq!(runtime.pending_task_count(), 1);
			client.remove_entity::<Project>(&1);

			ready.set(true);
			runtime.run_until_stalled();
			assert_eq!(calls.get(), 4);
			assert_eq!(runtime.pending_task_count(), 1);

			ready.set(true);
			runtime.run_until_stalled();
			assert_eq!(calls.get(), 4);
			assert_eq!(remounted.data(), Some(project(1, "fresh")));
			assert!(!remounted.is_stale());
		});
	}

	#[test]
	#[serial(entity_removal)]
	fn successful_upsert_clears_normalization_missing_without_clearing_invalidation() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let descriptor =
				QueryFamily::<(), Project, String>::new("tests.entity-removal-missing-recovery")
					.query((), || async { Ok(project(1, "initial")) })
					.with_entities(EntityValue::new());
			let active = client.observe(descriptor.clone(), QueryOptions::new());
			runtime.run_until_stalled();
			drop(active);
			let disabled = client.observe(descriptor.clone(), QueryOptions::new().enabled(false));

			client.remove_entity::<Project>(&1);
			client.invalidate(descriptor.key());
			assert!(disabled.is_stale());
			assert_eq!(runtime.pending_task_count(), 0);

			client.upsert_entity(project(1, "recovered"));

			assert_eq!(disabled.snapshot().status, QueryStatus::Success);
			assert_eq!(disabled.data(), Some(project(1, "recovered")));
			assert!(disabled.is_stale());
		});
	}
}

mod entity_propagation {
	use super::normalized::{
		Project, ProjectList, ProjectListProjection, materializations, project, project_list,
		reset_materializations,
	};
	use super::*;

	#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
	struct RollbackCollection {
		label: String,
		projects: Vec<Project>,
	}

	#[derive(Clone, Deserialize, Serialize)]
	struct RollbackCollectionRecipe {
		label: String,
		project_ids: Vec<u64>,
	}

	#[derive(Clone, Copy)]
	struct RollbackCollectionProjection;

	#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
	struct RollbackDetail {
		label: String,
		project: Project,
	}

	#[derive(Clone, Deserialize, Serialize)]
	struct RollbackDetailRecipe {
		label: String,
		project_id: u64,
	}

	#[derive(Clone, Copy)]
	struct RollbackDetailProjection;

	thread_local! {
		static ROLLBACK_PREPARATION_TRACE: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
	static PANIC_AFTER_ROLLBACK_PREPARATIONS: Cell<Option<usize>> = const { Cell::new(None) };
	static SWITCH_ROLLBACK_DEPENDENCY_TO_PROJECT_TWO: Cell<bool> = const { Cell::new(false) };
	static ROLLBACK_COLLECTION_MATERIALIZATIONS: Cell<usize> = const { Cell::new(0) };
		static ROLLBACK_DETAIL_MATERIALIZATIONS: Cell<usize> = const { Cell::new(0) };
	}

	fn record_rollback_preparation(projection: &'static str) {
		let preparation_count = ROLLBACK_PREPARATION_TRACE.with(|trace| {
			let mut trace = trace.borrow_mut();
			trace.push(projection);
			trace.len()
		});
		if PANIC_AFTER_ROLLBACK_PREPARATIONS
			.with(Cell::get)
			.is_some_and(|allowed| preparation_count > allowed)
		{
			panic!("heterogeneous projection precommit panic");
		}
	}

	fn reset_rollback_projection_state() {
		ROLLBACK_PREPARATION_TRACE.with(|trace| trace.borrow_mut().clear());
		PANIC_AFTER_ROLLBACK_PREPARATIONS.with(|limit| limit.set(None));
		SWITCH_ROLLBACK_DEPENDENCY_TO_PROJECT_TWO.with(|switch| switch.set(false));
		ROLLBACK_COLLECTION_MATERIALIZATIONS.with(|count| count.set(0));
		ROLLBACK_DETAIL_MATERIALIZATIONS.with(|count| count.set(0));
	}

	fn set_rollback_preparation_panic_after(limit: Option<usize>) {
		PANIC_AFTER_ROLLBACK_PREPARATIONS.with(|current| current.set(limit));
	}

	impl EntityProjection<RollbackCollection> for RollbackCollectionProjection {
		type Recipe = RollbackCollectionRecipe;

		const SCHEMA: &'static str = "rollback-collection-v1";

		fn normalize(
			&self,
			value: RollbackCollection,
			entities: &mut EntityWriter<'_>,
		) -> Self::Recipe {
			let project_ids = value
				.projects
				.into_iter()
				.map(|project| {
					let id = project.entity_id();
					entities.upsert(project);
					id
				})
				.collect();
			RollbackCollectionRecipe {
				label: value.label,
				project_ids,
			}
		}

		fn dependencies(&self, recipe: &Self::Recipe, dependencies: &mut EntityDependencies) {
			dependencies.extend::<Project>(recipe.project_ids.iter().copied());
		}

		fn materialize(
			&self,
			recipe: &Self::Recipe,
			entities: &EntityReader<'_>,
		) -> ProjectionMaterialization<RollbackCollection> {
			record_rollback_preparation("collection");
			ROLLBACK_COLLECTION_MATERIALIZATIONS.with(|count| count.set(count.get() + 1));
			match entities.required_vec::<Project>(&recipe.project_ids) {
				ProjectionMaterialization::Ready(projects) => {
					ProjectionMaterialization::Ready(RollbackCollection {
						label: recipe.label.clone(),
						projects,
					})
				}
				ProjectionMaterialization::MissingRequired => {
					ProjectionMaterialization::MissingRequired
				}
			}
		}

		fn apply_removals(
			&self,
			recipe: &mut Self::Recipe,
			removed: &RemovedEntities<'_>,
		) -> ProjectionRemoval {
			if SWITCH_ROLLBACK_DEPENDENCY_TO_PROJECT_TWO.with(Cell::get)
				&& removed.contains::<Project>(&1)
			{
				recipe.project_ids = vec![2];
				return ProjectionRemoval::Updated;
			}
			let previous_len = recipe.project_ids.len();
			recipe
				.project_ids
				.retain(|id| !removed.contains::<Project>(id));
			ProjectionRemoval::from_changed(previous_len != recipe.project_ids.len())
		}
	}

	impl EntityProjection<RollbackDetail> for RollbackDetailProjection {
		type Recipe = RollbackDetailRecipe;

		const SCHEMA: &'static str = "rollback-detail-v1";

		fn normalize(
			&self,
			value: RollbackDetail,
			entities: &mut EntityWriter<'_>,
		) -> Self::Recipe {
			let project_id = value.project.entity_id();
			entities.upsert(value.project);
			RollbackDetailRecipe {
				label: value.label,
				project_id,
			}
		}

		fn dependencies(&self, recipe: &Self::Recipe, dependencies: &mut EntityDependencies) {
			dependencies.extend::<Project>([recipe.project_id]);
		}

		fn materialize(
			&self,
			recipe: &Self::Recipe,
			entities: &EntityReader<'_>,
		) -> ProjectionMaterialization<RollbackDetail> {
			record_rollback_preparation("detail");
			ROLLBACK_DETAIL_MATERIALIZATIONS.with(|count| count.set(count.get() + 1));
			match entities.required::<Project>(&recipe.project_id) {
				ProjectionMaterialization::Ready(project) => {
					ProjectionMaterialization::Ready(RollbackDetail {
						label: recipe.label.clone(),
						project,
					})
				}
				ProjectionMaterialization::MissingRequired => {
					ProjectionMaterialization::MissingRequired
				}
			}
		}

		fn apply_removals(
			&self,
			recipe: &mut Self::Recipe,
			removed: &RemovedEntities<'_>,
		) -> ProjectionRemoval {
			if SWITCH_ROLLBACK_DEPENDENCY_TO_PROJECT_TWO.with(Cell::get)
				&& removed.contains::<Project>(&1)
			{
				recipe.project_id = 2;
				return ProjectionRemoval::Updated;
			}
			if removed.contains::<Project>(&recipe.project_id) {
				ProjectionRemoval::MissingRequired
			} else {
				ProjectionRemoval::Unchanged
			}
		}
	}

	#[test]
	#[serial(entity_propagation)]
	fn one_transaction_updates_every_exact_query_and_handle_once() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(
				QueryDefaults::default().stale_time(Duration::from_secs(60)),
				runtime.handle(),
			);
			let family = QueryFamily::<u64, ProjectList, String>::new(
				"tests.entity-propagation-exact-queries",
			);
			let first = client.observe(
				family
					.query(1, || async {
						Ok(project_list(
							"first recipe",
							vec![project(1, "one"), project(2, "two")],
						))
					})
					.with_entities(ProjectListProjection),
				QueryOptions::new(),
			);
			let second = client.observe(
				family
					.query(2, || async {
						Ok(project_list("second recipe", vec![project(1, "one")]))
					})
					.with_entities(ProjectListProjection),
				QueryOptions::new(),
			);
			let entity = client.entity::<Project>(1);

			runtime.run_until_stalled();
			let first_fetched = first.entry.last_fetched_ms.get();
			let second_fetched = second.entry.last_fetched_ms.get();
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&1),
				2
			);
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&2),
				1
			);
			reset_materializations();
			let snapshots = Rc::new(RefCell::new(Vec::new()));
			let snapshots_for_effect = Rc::clone(&snapshots);
			let entity_for_effect = entity.clone();
			let first_for_effect = first.clone();
			let second_for_effect = second.clone();
			let _effect = reinhardt_core::reactive::Effect::new(move || {
				snapshots_for_effect.borrow_mut().push((
					entity_for_effect.get(),
					first_for_effect.data(),
					second_for_effect.data(),
				));
			});
			snapshots.borrow_mut().clear();
			runtime.advance(Duration::from_secs(10));

			client.update_entities(|entities| {
				entities.upsert(project(1, "updated"));
				entities.remove::<Project>(&2);
			});
			reinhardt_core::reactive::runtime::with_runtime(|runtime| {
				runtime.flush_updates();
			});

			let expected_first = project_list("first recipe", vec![project(1, "updated")]);
			let expected_second = project_list("second recipe", vec![project(1, "updated")]);
			assert_eq!(entity.get(), Some(project(1, "updated")));
			assert_eq!(first.data(), Some(expected_first.clone()));
			assert_eq!(second.data(), Some(expected_second.clone()));
			assert_eq!(materializations("first recipe"), 1);
			assert_eq!(materializations("second recipe"), 1);
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&1),
				2
			);
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&2),
				0
			);
			assert_eq!(first.entry.last_fetched_ms.get(), first_fetched);
			assert_eq!(second.entry.last_fetched_ms.get(), second_fetched);
			assert!(!first.is_stale());
			assert!(!second.is_stale());
			assert_eq!(
				snapshots.borrow().as_slice(),
				&[(
					Some(project(1, "updated")),
					Some(expected_first),
					Some(expected_second),
				)]
			);

			runtime.advance(Duration::from_secs(50));
			assert!(first.is_stale());
			assert!(second.is_stale());
		});
	}

	#[test]
	#[serial(entity_propagation)]
	fn unrelated_upsert_does_not_infer_collection_membership() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let query = client.observe(
				QueryFamily::<(), ProjectList, String>::new(
					"tests.entity-propagation-no-membership-inference",
				)
				.query((), || async {
					Ok(project_list("stable recipe", vec![project(1, "one")]))
				})
				.with_entities(ProjectListProjection),
				QueryOptions::new(),
			);
			runtime.run_until_stalled();
			reset_materializations();

			client.upsert_entity(project(2, "unrelated"));

			assert_eq!(
				query.data(),
				Some(project_list("stable recipe", vec![project(1, "one")]))
			);
			assert_eq!(materializations("stable recipe"), 0);
		});
	}

	#[test]
	#[serial(entity_propagation)]
	fn heterogeneous_query_projections_commit_in_one_transaction() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let collection = client.observe(
				QueryFamily::<(), ProjectList, String>::new(
					"tests.entity-propagation-heterogeneous-collection",
				)
				.query((), || async {
					Ok(project_list("heterogeneous", vec![project(1, "initial")]))
				})
				.with_entities(ProjectListProjection),
				QueryOptions::new(),
			);
			let detail = client.observe(
				QueryFamily::<(), Project, String>::new(
					"tests.entity-propagation-heterogeneous-detail",
				)
				.query((), || async { Ok(project(1, "initial")) })
				.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);
			runtime.run_until_stalled();
			reset_materializations();

			client.upsert_entity(project(1, "updated"));

			assert_eq!(
				collection.data(),
				Some(project_list("heterogeneous", vec![project(1, "updated")]))
			);
			assert_eq!(detail.data(), Some(project(1, "updated")));
			assert_eq!(materializations("heterogeneous"), 1);
		});
	}

	#[test]
	#[serial(entity_propagation)]
	fn normalized_query_completion_propagates_to_an_existing_exact_query() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
			let family = QueryFamily::<u64, Project, String>::new(
				"tests.entity-propagation-query-completion",
			);
			let first = client.observe(
				family
					.query(1, || async { Ok(project(1, "first completion")) })
					.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);
			runtime.run_until_stalled();
			assert_eq!(first.data(), Some(project(1, "first completion")));

			let second = client.observe(
				family
					.query(2, || async { Ok(project(1, "second completion")) })
					.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);
			runtime.run_until_stalled();

			assert_eq!(first.data(), Some(project(1, "second completion")));
			assert_eq!(second.data(), Some(project(1, "second completion")));
		});
	}

	#[test]
	#[serial(entity_propagation)]
	fn callback_and_adapter_panics_roll_back_the_whole_transaction() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(
				QueryDefaults::default().stale_time(Duration::from_secs(3_600)),
				runtime.handle(),
			);
			reset_rollback_projection_state();
			let collection_descriptor = QueryFamily::<(), RollbackCollection, String>::new(
				"tests.entity-propagation-panic-rollback-collection",
			)
			.query((), || async {
				Ok(RollbackCollection {
					label: "collection recipe".to_string(),
					projects: vec![project(1, "stable")],
				})
			})
			.with_entities(RollbackCollectionProjection);
			let detail_descriptor = QueryFamily::<(), RollbackDetail, String>::new(
				"tests.entity-propagation-panic-rollback-detail",
			)
			.query((), || async {
				Ok(RollbackDetail {
					label: "detail recipe".to_string(),
					project: project(1, "stable"),
				})
			})
			.with_entities(RollbackDetailProjection);
			let collection_key = collection_descriptor.key().clone();
			let detail_key = detail_descriptor.key().clone();
			let enabled_collection =
				client.observe(collection_descriptor.clone(), QueryOptions::new());
			let enabled_detail = client.observe(detail_descriptor.clone(), QueryOptions::new());
			let entity = client.entity::<Project>(1);
			runtime.run_until_stalled();
			drop(enabled_collection);
			drop(enabled_detail);
			let disabled_options = QueryOptions::new()
				.enabled(false)
				.stale_time(Duration::from_secs(3_600));
			let collection = client.observe(collection_descriptor, disabled_options);
			let detail = client.observe(
				detail_descriptor,
				QueryOptions::new()
					.enabled(false)
					.stale_time(Duration::from_secs(3_600)),
			);
			let collection_before = RollbackCollection {
				label: "collection recipe".to_string(),
				projects: vec![project(1, "stable")],
			};
			let detail_before = RollbackDetail {
				label: "detail recipe".to_string(),
				project: project(1, "stable"),
			};
			let collection_fetched = collection.entry.last_fetched_ms.get();
			let detail_fetched = detail.entry.last_fetched_ms.get();
			let arena = client.entity_arena_for_test();
			let ticket_before = arena.record_write_ticket::<Project>(&1);
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&1),
				2
			);
			assert!(!collection.is_stale());
			assert!(!detail.is_stale());
			client.invalidate(&collection_key);
			client.invalidate(&detail_key);
			assert!(collection.is_stale());
			assert!(detail.is_stale());
			let snapshots = Rc::new(RefCell::new(Vec::new()));
			let snapshots_for_effect = Rc::clone(&snapshots);
			let entity_for_effect = entity.clone();
			let collection_for_effect = collection.clone();
			let detail_for_effect = detail.clone();
			let _effect = reinhardt_core::reactive::Effect::new(move || {
				snapshots_for_effect.borrow_mut().push((
					entity_for_effect.get(),
					collection_for_effect.data(),
					detail_for_effect.data(),
				));
			});
			snapshots.borrow_mut().clear();

			let callback_panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
				client.update_entities(|entities| {
					entities.upsert(project(1, "callback"));
					panic!("entity callback panic");
				});
			}));
			assert!(callback_panic.is_err());
			assert_eq!(entity.get(), Some(project(1, "stable")));
			assert_eq!(collection.data(), Some(collection_before.clone()));
			assert_eq!(detail.data(), Some(detail_before.clone()));

			reset_rollback_projection_state();
			set_rollback_preparation_panic_after(Some(1));
			let adapter_panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
				client.upsert_entity(project(1, "adapter"));
			}));
			set_rollback_preparation_panic_after(None);
			reinhardt_core::reactive::runtime::with_runtime(|runtime| {
				runtime.flush_updates();
			});

			assert!(adapter_panic.is_err());
			let mut preparation_trace =
				ROLLBACK_PREPARATION_TRACE.with(|trace| trace.borrow().clone());
			assert_eq!(preparation_trace.len(), 2);
			preparation_trace.sort_unstable();
			assert_eq!(preparation_trace, vec!["collection", "detail"]);
			assert_eq!(
				ROLLBACK_COLLECTION_MATERIALIZATIONS.with(Cell::get)
					+ ROLLBACK_DETAIL_MATERIALIZATIONS.with(Cell::get),
				1
			);
			assert_eq!(entity.get(), Some(project(1, "stable")));
			assert_eq!(collection.data(), Some(collection_before));
			assert_eq!(detail.data(), Some(detail_before));
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&1),
				2
			);
			assert_eq!(arena.record_write_ticket::<Project>(&1), ticket_before);
			assert!(!arena.record_is_removed::<Project>(&1));
			assert_eq!(collection.entry.last_fetched_ms.get(), collection_fetched);
			assert_eq!(detail.entry.last_fetched_ms.get(), detail_fetched);
			assert!(collection.is_stale());
			assert!(detail.is_stale());
			assert!(snapshots.borrow().is_empty());

			reset_rollback_projection_state();
			client.upsert_entity(project(1, "committed"));
			reinhardt_core::reactive::runtime::with_runtime(|runtime| {
				runtime.flush_updates();
			});

			assert_eq!(entity.get(), Some(project(1, "committed")));
			assert_eq!(
				collection.data(),
				Some(RollbackCollection {
					label: "collection recipe".to_string(),
					projects: vec![project(1, "committed")],
				})
			);
			assert_eq!(
				detail.data(),
				Some(RollbackDetail {
					label: "detail recipe".to_string(),
					project: project(1, "committed"),
				})
			);
			assert_eq!(ROLLBACK_COLLECTION_MATERIALIZATIONS.with(Cell::get), 1);
			assert_eq!(ROLLBACK_DETAIL_MATERIALIZATIONS.with(Cell::get), 1);
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&1),
				2
			);
			assert_eq!(collection.entry.last_fetched_ms.get(), collection_fetched);
			assert_eq!(detail.entry.last_fetched_ms.get(), detail_fetched);
			assert!(collection.is_stale());
			assert!(detail.is_stale());
			assert_eq!(
				snapshots.borrow().as_slice(),
				&[(
					Some(project(1, "committed")),
					Some(RollbackCollection {
						label: "collection recipe".to_string(),
						projects: vec![project(1, "committed")],
					}),
					Some(RollbackDetail {
						label: "detail recipe".to_string(),
						project: project(1, "committed"),
					}),
				)]
			);
		});
	}

	#[test]
	#[serial(entity_propagation)]
	fn failed_heterogeneous_preparation_preserves_unleased_dependency_gc_deadline() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(
				QueryDefaults::default().gc_time(Duration::from_secs(300)),
				runtime.handle(),
			);
			reset_rollback_projection_state();
			let arena = client.entity_arena_for_test();

			// Project 2 is present but unleased, so its initial collection deadline is t=300.
			client.upsert_entity(project(2, "unleased"));
			assert_eq!(
				arena.entity_gc_due_ms_for_test::<Project>(&2),
				Some(300_000)
			);

			let collection = client.observe(
				QueryFamily::<(), RollbackCollection, String>::new(
					"tests.entity-propagation-rollback-gc-collection",
				)
				.query((), || async {
					Ok(RollbackCollection {
						label: "collection recipe".to_string(),
						projects: vec![project(1, "stable")],
					})
				})
				.with_entities(RollbackCollectionProjection),
				QueryOptions::new(),
			);
			let detail = client.observe(
				QueryFamily::<(), RollbackDetail, String>::new(
					"tests.entity-propagation-rollback-gc-detail",
				)
				.query((), || async {
					Ok(RollbackDetail {
						label: "detail recipe".to_string(),
						project: project(1, "stable"),
					})
				})
				.with_entities(RollbackDetailProjection),
				QueryOptions::new(),
			);
			runtime.run_until_stalled();
			runtime.advance(Duration::from_secs(100));

			SWITCH_ROLLBACK_DEPENDENCY_TO_PROJECT_TWO.with(|switch| switch.set(true));
			set_rollback_preparation_panic_after(Some(1));
			let preparation_panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
				client.remove_entity::<Project>(&1);
			}));
			set_rollback_preparation_panic_after(None);
			SWITCH_ROLLBACK_DEPENDENCY_TO_PROJECT_TWO.with(|switch| switch.set(false));

			assert!(preparation_panic.is_err());
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&2),
				0
			);
			assert_eq!(
				arena.entity_gc_due_ms_for_test::<Project>(&2),
				Some(300_000)
			);
			assert_eq!(
				collection.data(),
				Some(RollbackCollection {
					label: "collection recipe".to_string(),
					projects: vec![project(1, "stable")],
				})
			);
			assert_eq!(
				detail.data(),
				Some(RollbackDetail {
					label: "detail recipe".to_string(),
					project: project(1, "stable"),
				})
			);

			runtime.advance(Duration::from_secs(200));
			runtime.run_due_maintenance();
			assert!(!arena.entity_record_exists_for_test::<Project>(&2));
		});
	}

	#[test]
	#[serial(entity_propagation)]
	fn reverse_dependencies_are_local_to_the_owning_client() {
		ReactiveScope::run(|| {
			let first_runtime = TestQueryRuntime::new();
			let second_runtime = TestQueryRuntime::new();
			let first_client =
				QueryClient::with_runtime(QueryDefaults::default(), first_runtime.handle());
			let second_client =
				QueryClient::with_runtime(QueryDefaults::default(), second_runtime.handle());
			let family = QueryFamily::<(), Project, String>::new(
				"tests.entity-propagation-client-isolation",
			);
			let first = first_client.observe(
				family
					.query((), || async { Ok(project(1, "first")) })
					.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);
			let second = second_client.observe(
				family
					.query((), || async { Ok(project(1, "second")) })
					.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);
			first_runtime.run_until_stalled();
			second_runtime.run_until_stalled();

			first_client.upsert_entity(project(1, "first updated"));

			assert_eq!(first.data(), Some(project(1, "first updated")));
			assert_eq!(second.data(), Some(project(1, "second")));
		});
	}

	#[test]
	#[serial(entity_propagation)]
	fn reverse_index_does_not_retain_inactive_query_entries() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = QueryClient::with_runtime(
				QueryDefaults::default().gc_time(Duration::ZERO),
				runtime.handle(),
			);
			let query = client.observe(
				QueryFamily::<(), Project, String>::new(
					"tests.entity-propagation-weak-reverse-index",
				)
				.query((), || async { Ok(project(1, "cached")) })
				.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);
			runtime.run_until_stalled();
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&1),
				1
			);

			let handle = client.entity::<Project>(1);
			drop(query);
			runtime.run_due_maintenance();

			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&1),
				0
			);
			client.upsert_entity(project(1, "uncached"));
			assert_eq!(client.entity_dependency_index_len_for_test(), 0);
			assert_eq!(handle.get(), Some(project(1, "uncached")));
		});
	}
}

mod entity_gc {
	use super::normalized::{Project, project};
	use super::*;

	fn client(runtime: &TestQueryRuntime, gc_time: Duration) -> QueryClient {
		QueryClient::with_runtime(QueryDefaults::default().gc_time(gc_time), runtime.clock())
	}

	#[test]
	#[serial(entity_gc)]
	fn dependency_and_handle_leases_delay_entity_collection() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = client(&runtime, Duration::from_secs(5));
			let query = client.observe(
				QueryFamily::<(), Project, String>::new("tests.entity-gc-retention")
					.query((), || async { Ok(project(1, "cached")) })
					.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);
			runtime.run_until_stalled();
			let handle = client.entity::<Project>(1);
			drop(query);
			runtime.advance(Duration::from_secs(5));
			runtime.run_due_maintenance();
			assert_eq!(handle.get(), Some(project(1, "cached")));
			drop(handle);
			runtime.advance(Duration::from_secs(4));
			runtime.run_due_maintenance();
			let arena = client.entity_arena_for_test();
			assert!(arena.entity_record_exists_for_test::<Project>(&1));
			runtime.advance(Duration::from_secs(1));
			runtime.run_due_maintenance();
			assert!(!arena.entity_record_exists_for_test::<Project>(&1));
		});
	}

	#[test]
	#[serial(entity_gc)]
	fn query_gc_releases_dependency_leases_before_entity_gc() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = client(&runtime, Duration::from_secs(5));
			let descriptor =
				QueryFamily::<(), Project, String>::new("tests.entity-gc-query-release")
					.query((), || async { Ok(project(1, "cached")) })
					.with_entities(EntityValue::new());
			let query = client.observe(descriptor, QueryOptions::new());
			runtime.run_until_stalled();
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&1),
				1
			);
			drop(query);
			runtime.advance(Duration::from_secs(5));
			runtime.run_due_maintenance();
			assert_eq!(
				client.entity_dependency_lease_count_for_test::<Project>(&1),
				0
			);
			runtime.advance(Duration::from_secs(5));
			runtime.run_due_maintenance();
			assert!(
				!client
					.entity_arena_for_test()
					.entity_record_exists_for_test::<Project>(&1)
			);
		});
	}

	#[test]
	#[serial(entity_gc)]
	fn reacquire_invalidates_an_entity_gc_generation() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = client(&runtime, Duration::from_secs(5));
			client.upsert_entity(project(1, "cached"));
			let arena = client.entity_arena_for_test();
			let first = client.entity::<Project>(1);
			drop(first);
			runtime.advance(Duration::from_secs(4));
			let second = client.entity::<Project>(1);
			runtime.advance(Duration::from_secs(1));
			runtime.run_due_maintenance();
			assert_eq!(second.get(), Some(project(1, "cached")));
			drop(second);
			assert!(arena.entity_record_exists_for_test::<Project>(&1));
		});
	}

	#[test]
	#[serial(entity_gc)]
	fn unreferenced_present_and_tombstone_records_share_grace_period() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = client(&runtime, Duration::from_secs(5));
			let arena = client.entity_arena_for_test();
			client.upsert_entity(project(1, "present"));
			client.remove_entity::<Project>(&2);
			assert!(arena.entity_record_exists_for_test::<Project>(&1));
			assert!(arena.entity_record_exists_for_test::<Project>(&2));
			runtime.advance(Duration::from_secs(4));
			runtime.run_due_maintenance();
			assert!(arena.entity_record_exists_for_test::<Project>(&1));
			assert!(arena.entity_record_exists_for_test::<Project>(&2));
			runtime.advance(Duration::from_secs(1));
			runtime.run_due_maintenance();
			assert!(!arena.entity_record_exists_for_test::<Project>(&1));
			assert!(!arena.entity_record_exists_for_test::<Project>(&2));
		});
	}

	#[test]
	#[serial(entity_gc)]
	fn zero_duration_collection_waits_for_the_next_maintenance_pass() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = client(&runtime, Duration::ZERO);
			let arena = client.entity_arena_for_test();
			client.upsert_entity(project(1, "published"));
			assert!(arena.entity_record_exists_for_test::<Project>(&1));
			runtime.run_due_maintenance();
			assert!(!arena.entity_record_exists_for_test::<Project>(&1));
		});
	}

	#[test]
	#[serial(entity_gc)]
	fn older_query_ticket_blocks_collection_until_ticket_drop() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = client(&runtime, Duration::ZERO);
			let arena = client.entity_arena_for_test();
			let ticket = arena.acquire_query_ticket();
			client.upsert_entity(project(1, "ticketed"));
			runtime.run_due_maintenance();
			assert!(arena.entity_record_exists_for_test::<Project>(&1));
			drop(ticket);
			assert!(!arena.entity_record_exists_for_test::<Project>(&1));
		});
	}

	#[test]
	#[serial(entity_gc)]
	fn ticket_drop_prunes_dead_reverse_dependencies_after_entity_collection() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = client(&runtime, Duration::ZERO);
			let arena = client.entity_arena_for_test();
			let ticket = arena.acquire_query_ticket();
			let query = client.observe(
				QueryFamily::<(), Project, String>::new("tests.entity-gc-ticket-prunes-index")
					.query((), || async { Ok(project(1, "ticketed")) })
					.with_entities(EntityValue::new()),
				QueryOptions::new(),
			);
			runtime.run_until_stalled();
			assert_eq!(client.entity_dependency_index_len_for_test(), 1);

			drop(query);
			runtime.run_due_maintenance();
			assert!(arena.entity_record_exists_for_test::<Project>(&1));
			assert_eq!(client.entity_dependency_index_len_for_test(), 1);

			drop(ticket);
			assert!(!arena.entity_record_exists_for_test::<Project>(&1));
			assert_eq!(client.entity_dependency_index_len_for_test(), 0);
		});
	}

	#[test]
	#[serial(entity_gc)]
	fn final_client_drop_releases_browser_resources_and_entities() {
		ReactiveScope::run(|| {
			let runtime = TestQueryRuntime::new();
			let client = client(&runtime, Duration::from_secs(5));
			let probe = query_browser_resource_probe_for_test(&client);
			client.upsert_entity(project(1, "owned"));
			drop(client);
			assert_eq!(probe.counts(), (0, 0));
		});
	}
}

#[test]
fn query_client_guards_restore_the_previous_client() {
	let first = QueryClient::new(QueryDefaults::default());
	let second = QueryClient::new(QueryDefaults::default());
	let first_guard = provide_query_client(first.clone());
	let second_guard = provide_query_client(second.clone());
	let nested_first_guard = provide_query_client(first.clone());

	assert!(queries().same_instance(&first));
	drop(first_guard);
	assert!(queries().same_instance(&first));
	drop(nested_first_guard);
	assert!(queries().same_instance(&second));
	drop(second_guard);

	let panic = match std::panic::catch_unwind(queries) {
		Ok(_) => panic!("dropping every guard must remove the ambient query client"),
		Err(panic) => panic,
	};
	let message = panic
		.downcast_ref::<String>()
		.map(String::as_str)
		.or_else(|| panic.downcast_ref::<&str>().copied())
		.expect("missing query client should panic with a string");
	assert_eq!(message, "use_query requires an active QueryClient");
}

#[test]
fn pending_query_task_does_not_retain_the_final_client_owner() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let client_guard = provide_query_client(client.clone());
	let cancellations = Rc::new(Cell::new(0));
	let family = QueryFamily::<(), String, String>::new("tests.client-drop-cancellation");
	let query = client.observe(
		family.query_with_cancellation((), {
			let cancellations = Rc::clone(&cancellations);
			move |cancellation| {
				let cancellations = Rc::clone(&cancellations);
				async move {
					let _registration = cancellation.on_cancel(move || {
						cancellations.set(cancellations.get() + 1);
					});
					std::future::pending::<Result<String, String>>().await
				}
			}
		}),
		QueryOptions::default(),
	);
	runtime.run_until_stalled();

	assert_eq!(runtime.pending_task_count(), 1);
	assert_eq!(cancellations.get(), 0);

	drop(client_guard);
	drop(client);

	assert_eq!(cancellations.get(), 1);
	runtime.run_until_stalled();
	assert_eq!(runtime.pending_task_count(), 0);
	assert_eq!(query.snapshot().status, QueryStatus::Pending);

	drop(query);
	assert_eq!(cancellations.get(), 1);
}

#[test]
#[serial(query_cache)]
fn earliest_live_enabled_observer_supplies_the_fetcher() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		let family = QueryFamily::<(), String, String>::new("tests.observer-fetcher-order");
		let first_calls = Rc::new(Cell::new(0));
		let second_calls = Rc::new(Cell::new(0));
		let first = use_query(
			family.query((), {
				let first_calls = Rc::clone(&first_calls);
				move || {
					let call = first_calls.get() + 1;
					first_calls.set(call);
					async move { Ok::<_, String>(format!("first-{call}")) }
				}
			}),
			QueryOptions::default(),
		);
		let second = use_query(
			family.query((), {
				let second_calls = Rc::clone(&second_calls);
				move || {
					let call = second_calls.get() + 1;
					second_calls.set(call);
					async move { Ok::<_, String>(format!("second-{call}")) }
				}
			}),
			QueryOptions::default(),
		);

		// Act
		second.refetch();

		// Assert
		assert_eq!(first_calls.get(), 2);
		assert_eq!(second_calls.get(), 0);
		assert_eq!(second.data(), Some("first-2".to_string()));

		// Act
		drop(first);
		second.refetch();

		// Assert
		assert_eq!(first_calls.get(), 2);
		assert_eq!(second_calls.get(), 1);
		assert_eq!(second.data(), Some("second-1".to_string()));
	});
}

#[tokio::test]
#[serial(query_cache)]
async fn active_ssr_query_preserves_observer_time_options() {
	// Arrange
	let context = Rc::new(RefCell::new(
		crate::ssr::resource_context::SsrResourceContext::new(Duration::from_secs(1)),
	));
	let expected_stale_time = Duration::from_secs(17);
	let expected_gc_time = Duration::from_secs(41);

	// Act
	let client = QueryClient::new_ssr(QueryDefaults::default());
	let query = with_query_client_async(
		client,
		crate::ssr::resource_context::scope_context(Rc::clone(&context), async {
			ReactiveScope::run(|| {
				use_query(
					QueryFamily::<(), String, String>::new("tests.ssr-query-options")
						.query((), || async { Ok("value".to_string()) }),
					QueryOptions::default()
						.stale_time(expected_stale_time)
						.gc_time(expected_gc_time),
				)
			})
		}),
	)
	.await;

	// Assert
	assert_eq!(query.lease.inner.policy.stale_time, expected_stale_time);
	assert_eq!(query.lease.inner.policy.gc_time, expected_gc_time);
}

#[test]
#[serial(query_cache)]
fn imperative_acquisition_deduplicates_in_flight_work() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		let tasks = Rc::new(RefCell::new(VecDeque::new()));
		let tasks_for_sink = Rc::clone(&tasks);
		let _sink = crate::platform::install_task_sink(move |task| {
			tasks_for_sink.borrow_mut().push_back(task);
		});
		let calls = Rc::new(Cell::new(0));
		let key = QueryFamily::<(), _, _>::new("imperative-dedupe").query((), {
			let calls = Rc::clone(&calls);
			move || {
				calls.set(calls.get() + 1);
				async { Ok::<_, String>("value".to_string()) }
			}
		});

		// Act
		let first = acquire_query(
			key.clone(),
			QueryAcquireOptions {
				consumer: QueryConsumer::Navigation(1),
				error_policy: QueryErrorPolicy::Discard,
			},
		);
		let second = acquire_query(
			key,
			QueryAcquireOptions {
				consumer: QueryConsumer::Navigation(2),
				error_policy: QueryErrorPolicy::Discard,
			},
		);

		// Assert
		assert_eq!(calls.get(), 0, "acquisition must not run a second fetch");
		assert_eq!(tasks.borrow().len(), 1);
		assert_eq!(poll_one_task(&tasks), Poll::Ready(()));
		assert_eq!(calls.get(), 1);
		assert_eq!(
			tokio_test::block_on(first.result()),
			Ok("value".to_string())
		);
		assert_eq!(
			tokio_test::block_on(second.result()),
			Ok("value".to_string())
		);
	});
}

#[test]
#[serial(query_cache)]
fn dropping_one_of_two_leases_keeps_request_alive() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		let tasks = Rc::new(RefCell::new(VecDeque::new()));
		let tasks_for_sink = Rc::clone(&tasks);
		let _sink = crate::platform::install_task_sink(move |task| {
			tasks_for_sink.borrow_mut().push_back(task);
		});
		let ready = Rc::new(Cell::new(false));
		let dropped = Rc::new(Cell::new(0));
		let key = QueryFamily::<(), _, _>::new("two-leases").query((), {
			let ready = Rc::clone(&ready);
			let dropped = Rc::clone(&dropped);
			move || TestGate {
				ready: Rc::clone(&ready),
				dropped: Rc::clone(&dropped),
				result: Some(Ok("shared".to_string())),
			}
		});
		let entry = query_entry(key.clone());
		let first = acquire_query(
			key.clone(),
			QueryAcquireOptions {
				consumer: QueryConsumer::Navigation(1),
				error_policy: QueryErrorPolicy::Discard,
			},
		);
		let second = acquire_query(
			key,
			QueryAcquireOptions {
				consumer: QueryConsumer::MountedRoute(2),
				error_policy: QueryErrorPolicy::Discard,
			},
		);
		assert_eq!(poll_one_task(&tasks), Poll::Pending);

		// Act
		drop(first);
		assert_eq!(entry.lease_count.get(), 1);
		assert!(entry.has_request(), "the remaining lease keeps work alive");
		ready.set(true);
		let completion = poll_one_task(&tasks);

		// Assert
		assert_eq!(completion, Poll::Ready(()));
		assert_eq!(
			entry.state.with_untracked(|state| state.clone()),
			ResourceState::Success("shared".to_string())
		);
		assert_eq!(
			tokio_test::block_on(second.result()),
			Ok("shared".to_string())
		);
		assert_eq!(dropped.get(), 1);
	});
}

#[test]
#[serial(query_cache)]
fn shared_fetch_receives_the_query_request_cancellation_handle() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		let tasks = Rc::new(RefCell::new(VecDeque::new()));
		let tasks_for_sink = Rc::clone(&tasks);
		let _sink = crate::platform::install_task_sink(move |task| {
			tasks_for_sink.borrow_mut().push_back(task);
		});
		let ready = Rc::new(Cell::new(false));
		let dropped = Rc::new(Cell::new(0));
		let observed_cancellation = Rc::new(RefCell::new(None));
		let key = QueryFamily::<(), _, _>::new("shared-request-cancellation")
			.query_with_cancellation((), {
				let ready = Rc::clone(&ready);
				let dropped = Rc::clone(&dropped);
				let observed_cancellation = Rc::clone(&observed_cancellation);
				move |cancellation| {
					observed_cancellation.borrow_mut().replace(cancellation);
					TestGate {
						ready: Rc::clone(&ready),
						dropped: Rc::clone(&dropped),
						result: Some(Ok("shared".to_string())),
					}
				}
			});
		let first = acquire_query(
			key.clone(),
			QueryAcquireOptions {
				consumer: QueryConsumer::Prefetch,
				error_policy: QueryErrorPolicy::Discard,
			},
		);
		let second = acquire_query(
			key,
			QueryAcquireOptions {
				consumer: QueryConsumer::Navigation(2),
				error_policy: QueryErrorPolicy::Discard,
			},
		);
		assert_eq!(poll_one_task(&tasks), Poll::Pending);

		// Act
		drop(first);
		let cancellation = observed_cancellation
			.borrow()
			.as_ref()
			.expect("the shared fetch receives a cancellation handle")
			.clone();

		// Assert
		assert!(
			!cancellation.is_cancelled(),
			"the remaining lease keeps the shared request cancellation alive"
		);
		ready.set(true);
		assert_eq!(poll_one_task(&tasks), Poll::Ready(()));
		assert_eq!(
			tokio_test::block_on(second.result()),
			Ok("shared".to_string())
		);
		assert_eq!(dropped.get(), 1);
	});
}

#[test]
#[serial(query_cache)]
fn dropping_final_lease_cancels_request_once() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		let tasks = Rc::new(RefCell::new(VecDeque::new()));
		let tasks_for_sink = Rc::clone(&tasks);
		let _sink = crate::platform::install_task_sink(move |task| {
			tasks_for_sink.borrow_mut().push_back(task);
		});
		let ready = Rc::new(Cell::new(false));
		let dropped = Rc::new(Cell::new(0));
		let key = QueryFamily::<(), _, _>::new("final-lease-cancel").query((), {
			let ready = Rc::clone(&ready);
			let dropped = Rc::clone(&dropped);
			move || TestGate {
				ready: Rc::clone(&ready),
				dropped: Rc::clone(&dropped),
				result: Some(Ok("never-published".to_string())),
			}
		});
		let entry = query_entry(key.clone());
		let lease = acquire_query(
			key,
			QueryAcquireOptions {
				consumer: QueryConsumer::Navigation(3),
				error_policy: QueryErrorPolicy::Discard,
			},
		);
		assert_eq!(poll_one_task(&tasks), Poll::Pending);
		let cancelled = Rc::new(Cell::new(0));
		let cancelled_for_callback = Rc::clone(&cancelled);
		let registration = entry
			.request
			.borrow()
			.as_ref()
			.expect("the pending request must be owned by the entry")
			.source
			.register(move || cancelled_for_callback.set(cancelled_for_callback.get() + 1));

		// Act
		drop(lease);
		let completion = poll_one_task(&tasks);

		// Assert
		assert_eq!(cancelled.get(), 1, "the source must cancel exactly once");
		assert!(entry.request.borrow().is_none());
		assert!(!entry.is_fetching.get());
		assert_eq!(completion, Poll::Ready(()));
		assert_eq!(dropped.get(), 1, "the aborted fetch future must be dropped");
		drop(registration);
		let _ = ready;
	});
}

#[test]
#[serial(query_cache)]
fn queued_refetch_keeps_completed_generation_for_existing_lease() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		let tasks = Rc::new(RefCell::new(VecDeque::new()));
		let tasks_for_sink = Rc::clone(&tasks);
		let _sink = crate::platform::install_task_sink(move |task| {
			tasks_for_sink.borrow_mut().push_back(task);
		});
		let ready = Rc::new(Cell::new(false));
		let dropped = Rc::new(Cell::new(0));
		let key = QueryFamily::<(), _, _>::new("queued-generation").query((), {
			let ready = Rc::clone(&ready);
			let dropped = Rc::clone(&dropped);
			move || TestGate {
				ready: Rc::clone(&ready),
				dropped: Rc::clone(&dropped),
				result: Some(Ok("first".to_string())),
			}
		});
		let entry = query_entry(key.clone());
		let lease = acquire_query(
			key,
			QueryAcquireOptions {
				consumer: QueryConsumer::Navigation(11),
				error_policy: QueryErrorPolicy::Discard,
			},
		);
		assert_eq!(poll_one_task(&tasks), Poll::Pending);
		let _ = entry.start_fetch(true);

		// Act
		ready.set(true);
		assert_eq!(poll_one_task(&tasks), Poll::Ready(()));
		let mut result = Box::pin(lease.result());
		let mut context = Context::from_waker(Waker::noop());

		// Assert
		assert!(
			entry.has_request(),
			"the queued refetch must start after completion"
		);
		assert_eq!(
			result.as_mut().poll(&mut context),
			Poll::Ready(Ok("first".to_string()))
		);
		drop(result);
		drop(lease);
	});
}

#[test]
#[serial(query_cache)]
fn cancelling_request_discards_queued_refetch() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		let tasks = Rc::new(RefCell::new(VecDeque::new()));
		let tasks_for_sink = Rc::clone(&tasks);
		let _sink = crate::platform::install_task_sink(move |task| {
			tasks_for_sink.borrow_mut().push_back(task);
		});
		let ready = Rc::new(Cell::new(false));
		let dropped = Rc::new(Cell::new(0));
		let key = QueryFamily::<(), _, _>::new("cancel-queued-refetch").query((), {
			let ready = Rc::clone(&ready);
			let dropped = Rc::clone(&dropped);
			move || TestGate {
				ready: Rc::clone(&ready),
				dropped: Rc::clone(&dropped),
				result: Some(Ok("replacement".to_string())),
			}
		});
		let entry = query_entry(key.clone());
		let lease = acquire_query(
			key.clone(),
			QueryAcquireOptions {
				consumer: QueryConsumer::Navigation(12),
				error_policy: QueryErrorPolicy::Discard,
			},
		);
		assert_eq!(poll_one_task(&tasks), Poll::Pending);
		let _ = entry.start_fetch(true);
		assert!(entry.refetch_after_in_flight.get());

		// Act
		drop(lease);

		// Assert
		assert!(!entry.refetch_after_in_flight.get());
		assert_eq!(poll_one_task(&tasks), Poll::Ready(()));
		ready.set(true);
		let replacement = acquire_query(
			key,
			QueryAcquireOptions {
				consumer: QueryConsumer::Navigation(13),
				error_policy: QueryErrorPolicy::Discard,
			},
		);
		assert_eq!(poll_one_task(&tasks), Poll::Ready(()));
		assert!(
			!entry.has_request(),
			"a cancelled request must not schedule a stale follow-up fetch"
		);
		assert_eq!(
			tokio_test::block_on(replacement.result()),
			Ok("replacement".to_string())
		);
	});
}

#[test]
#[serial(query_cache)]
fn cancel_completion_race_does_not_publish_obsolete_value() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		let tasks = Rc::new(RefCell::new(VecDeque::new()));
		let tasks_for_sink = Rc::clone(&tasks);
		let _sink = crate::platform::install_task_sink(move |task| {
			tasks_for_sink.borrow_mut().push_back(task);
		});
		let ready = Rc::new(Cell::new(false));
		let dropped = Rc::new(Cell::new(0));
		let key = QueryFamily::<(), _, _>::new("cancel-race").query((), {
			let ready = Rc::clone(&ready);
			let dropped = Rc::clone(&dropped);
			move || TestGate {
				ready: Rc::clone(&ready),
				dropped: Rc::clone(&dropped),
				result: Some(Ok("obsolete".to_string())),
			}
		});
		let entry = query_entry(key.clone());
		let lease = acquire_query(
			key,
			QueryAcquireOptions {
				consumer: QueryConsumer::Navigation(4),
				error_policy: QueryErrorPolicy::Discard,
			},
		);
		assert_eq!(poll_one_task(&tasks), Poll::Pending);
		let generation = entry
			.request
			.borrow()
			.as_ref()
			.expect("the request generation must be visible")
			.generation;

		// Act
		drop(lease);
		entry.complete_attempt(generation, Ok("obsolete".to_string()));
		let completion = poll_one_task(&tasks);

		// Assert
		assert_eq!(completion, Poll::Ready(()));
		assert_eq!(
			entry.state.with_untracked(|state| state.clone()),
			ResourceState::Loading
		);
		assert!(entry.completed.borrow().is_none());
		assert_eq!(dropped.get(), 1);
		let _ = ready;
	});
}

#[test]
#[serial(query_cache)]
fn cancelled_revalidation_preserves_previous_success() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		let tasks = Rc::new(RefCell::new(VecDeque::new()));
		let tasks_for_sink = Rc::clone(&tasks);
		let _sink = crate::platform::install_task_sink(move |task| {
			tasks_for_sink.borrow_mut().push_back(task);
		});
		let ready = Rc::new(Cell::new(false));
		let dropped = Rc::new(Cell::new(0));
		let key = QueryFamily::<(), _, _>::new("cancel-revalidation").query((), {
			let ready = Rc::clone(&ready);
			let dropped = Rc::clone(&dropped);
			move || TestGate {
				ready: Rc::clone(&ready),
				dropped: Rc::clone(&dropped),
				result: Some(Ok("new".to_string())),
			}
		});
		let entry = query_entry(key.clone());
		entry.state.set(ResourceState::Success("old".to_string()));
		entry.last_fetched_ms.set(Some(now_ms()));
		let lease = acquire_query_with_options(
			key,
			QueryAcquireOptions {
				consumer: QueryConsumer::Navigation(5),
				error_policy: QueryErrorPolicy::Discard,
			},
			QueryOptions::default().stale_time(Duration::ZERO),
		);
		assert_eq!(poll_one_task(&tasks), Poll::Pending);

		// Act
		drop(lease);
		ready.set(true);
		let completion = poll_one_task(&tasks);

		// Assert
		assert_eq!(completion, Poll::Ready(()));
		assert_eq!(
			entry.state.with_untracked(|state| state.clone()),
			ResourceState::Success("old".to_string())
		);
		assert!(!entry.is_fetching.get());
		assert!(entry.last_fetched_ms.get().is_some());
		assert_eq!(dropped.get(), 1);
	});
}

#[test]
#[serial(query_cache)]
fn discarded_error_retries_on_next_acquisition() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		let calls = Rc::new(Cell::new(0));
		let key = QueryFamily::<(), _, _>::new("discarded-error").query((), {
			let calls = Rc::clone(&calls);
			move || {
				calls.set(calls.get() + 1);
				async { Err::<String, _>("route failed".to_string()) }
			}
		});

		// Act
		let first = acquire_query(
			key.clone(),
			QueryAcquireOptions {
				consumer: QueryConsumer::Navigation(6),
				error_policy: QueryErrorPolicy::Discard,
			},
		);
		assert_eq!(
			tokio_test::block_on(first.result()),
			Err(QueryResultError::Fetch("route failed".to_string()))
		);
		let second = acquire_query(
			key,
			QueryAcquireOptions {
				consumer: QueryConsumer::Navigation(7),
				error_policy: QueryErrorPolicy::Discard,
			},
		);

		// Assert
		assert_eq!(calls.get(), 2);
		assert_eq!(
			tokio_test::block_on(second.result()),
			Err(QueryResultError::Fetch("route failed".to_string()))
		);
	});
}

#[test]
#[serial(query_cache)]
fn discarded_error_retries_for_a_later_retaining_observer() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		let calls = Rc::new(Cell::new(0));
		let descriptor =
			QueryFamily::<(), _, _>::new("discarded-error-retain-observer").query((), {
				let calls = Rc::clone(&calls);
				move || {
					let call = calls.get() + 1;
					calls.set(call);
					async move {
						if call == 1 {
							Err("route failed".to_string())
						} else {
							Ok("hook recovered".to_string())
						}
					}
				}
			});
		let discarded = acquire_query(
			descriptor.clone(),
			QueryAcquireOptions {
				consumer: QueryConsumer::Navigation(8),
				error_policy: QueryErrorPolicy::Discard,
			},
		);
		assert_eq!(
			tokio_test::block_on(discarded.result()),
			Err(QueryResultError::Fetch("route failed".to_string()))
		);
		drop(discarded);

		// Act
		let retained = use_query(
			descriptor,
			QueryOptions::default().stale_time(Duration::from_secs(30)),
		);

		// Assert
		assert_eq!(calls.get(), 2);
		assert_eq!(retained.data(), Some("hook recovered".to_string()));
	});
}

#[test]
#[serial(query_cache)]
fn invalidation_without_live_observer_does_not_refetch() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		let client = queries();
		let tasks = Rc::new(RefCell::new(VecDeque::new()));
		let tasks_for_sink = Rc::clone(&tasks);
		let _sink = crate::platform::install_task_sink(move |task| {
			tasks_for_sink.borrow_mut().push_back(task);
		});
		let ready = Rc::new(Cell::new(false));
		let dropped = Rc::new(Cell::new(0));
		let calls = Rc::new(Cell::new(0));
		let key = QueryFamily::<(), _, _>::new("cancel-then-invalidate").query((), {
			let ready = Rc::clone(&ready);
			let dropped = Rc::clone(&dropped);
			let calls = Rc::clone(&calls);
			move || {
				calls.set(calls.get() + 1);
				TestGate {
					ready: Rc::clone(&ready),
					dropped: Rc::clone(&dropped),
					result: Some(Ok("refetched".to_string())),
				}
			}
		});
		let exact_key = key.key().clone();
		let lease = acquire_query(
			key.clone(),
			QueryAcquireOptions {
				consumer: QueryConsumer::Navigation(8),
				error_policy: QueryErrorPolicy::Discard,
			},
		);
		assert_eq!(poll_one_task(&tasks), Poll::Pending);

		// Act
		drop(lease);
		ready.set(true);
		client.invalidate(&exact_key);
		assert_eq!(
			tasks.borrow().len(),
			1,
			"an inactive cache entry must not retain a fetch closure"
		);
		assert_eq!(poll_one_task(&tasks), Poll::Ready(()));

		// Assert
		assert_eq!(calls.get(), 1);
		assert_eq!(dropped.get(), 1);
		let entry = query_entry(key);
		assert_eq!(
			entry.state.with_untracked(|state| state.clone()),
			ResourceState::Loading
		);
	});
}

#[rstest]
#[serial(query_cache)]
fn use_query_deduplicates_shared_key() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		let calls = Rc::new(Cell::new(0));

		// Act
		let first = use_query(
			QueryFamily::<(), _, _>::new("shared").query((), {
				let calls = Rc::clone(&calls);
				move || {
					calls.set(calls.get() + 1);
					async { Ok::<_, String>("value".to_string()) }
				}
			}),
			QueryOptions::default(),
		);
		let second = use_query(
			QueryFamily::<(), _, _>::new("shared").query((), {
				let calls = Rc::clone(&calls);
				move || {
					calls.set(calls.get() + 1);
					async { Ok::<_, String>("value".to_string()) }
				}
			}),
			QueryOptions::default(),
		);

		// Assert
		assert_eq!(calls.get(), 1);
		assert_eq!(first.data(), Some("value".to_string()));
		assert_eq!(second.data(), Some("value".to_string()));
	});
}

#[rstest]
#[serial(query_cache)]
fn cached_query_survives_the_scope_that_created_it() {
	// Arrange
	let _query_client = isolated_query_client();
	let key = QueryFamily::<(), _, _>::new("retained-cache-entry")
		.query((), || async { Ok::<_, String>("cached".to_string()) });
	let scope = ReactiveScope::new();
	let first = scope.enter(|| use_query(key.clone(), QueryOptions::default()));
	assert_eq!(first.data(), Some("cached".to_string()));
	drop(first);
	drop(scope);

	// Act
	let cached = ReactiveScope::run(|| use_query(key, QueryOptions::default()));

	// Assert
	assert_eq!(cached.data(), Some("cached".to_string()));
}

#[rstest]
#[serial(query_cache)]
fn refetch_runs_fetcher_again() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		let calls = Rc::new(Cell::new(0));
		let query = use_query(
			QueryFamily::<(), _, _>::new("manual-refetch").query((), {
				let calls = Rc::clone(&calls);
				move || {
					let value = calls.get() + 1;
					calls.set(value);
					async move { Ok::<_, String>(value) }
				}
			}),
			QueryOptions::default(),
		);

		// Act
		query.refetch();

		// Assert
		assert_eq!(calls.get(), 2);
		assert_eq!(query.data(), Some(2));
	});
}

#[rstest]
#[serial(query_cache)]
fn failed_query_respects_stale_time_before_retrying() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		let calls = Rc::new(Cell::new(0));
		let key = QueryFamily::<(), _, _>::new("failed-query").query((), {
			let calls = Rc::clone(&calls);
			move || {
				calls.set(calls.get() + 1);
				async { Err::<String, _>("not found".to_string()) }
			}
		});

		// Act
		let first = use_query(
			key.clone(),
			QueryOptions::default().stale_time(Duration::from_secs(30)),
		);
		let second = use_query(
			key,
			QueryOptions::default().stale_time(Duration::from_secs(30)),
		);

		// Assert
		assert_eq!(calls.get(), 1);
		assert_eq!(first.error(), Some("not found".to_string()));
		assert_eq!(second.error(), Some("not found".to_string()));
	});
}

#[rstest]
#[serial(query_cache)]
fn successful_query_is_not_pending_during_background_fetch() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		let descriptor = QueryFamily::<(), _, _>::new("background-refetch")
			.query((), || async { Ok::<_, String>("fresh".to_string()) });
		let fetcher = Rc::clone(&descriptor.fetcher);
		let entry = Rc::new(QueryEntry::new(descriptor));
		entry
			.state
			.set(ResourceState::Success("cached".to_string()));
		entry.is_fetching.set(true);
		let lease = entry.make_lease(
			None,
			QueryConsumer::MountedQuery,
			QueryErrorPolicy::Retain,
			fetcher,
			ObserverPolicy::resolve(&QueryOptions::default(), &QueryDefaults::default()),
			None,
		);
		let query = QueryHandle { entry, lease };

		// Act
		let data = query.data();
		let is_fetching = query.is_fetching();
		let status = query.snapshot().status;

		// Assert
		assert_eq!(data, Some("cached".to_string()));
		assert!(is_fetching);
		assert_eq!(status, QueryStatus::Success);
	});
}

#[rstest]
#[serial(query_cache)]
fn invalidation_during_in_flight_fetch_runs_after_completion() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		let client = queries();
		let calls = Rc::new(Cell::new(0));

		// Act
		let family = QueryFamily::<(), i32, String>::new("queued-invalidation");
		let exact_key = family.key(());
		let query = use_query(
			family.query((), {
				let calls = Rc::clone(&calls);
				let client = client.clone();
				move || {
					let calls = Rc::clone(&calls);
					let client = client.clone();
					let exact_key = exact_key.clone();
					async move {
						let value = calls.get() + 1;
						calls.set(value);
						if value == 1 {
							client.invalidate(&exact_key);
						}
						Ok::<_, String>(value)
					}
				}
			}),
			QueryOptions::default(),
		);

		// Assert
		assert_eq!(calls.get(), 2);
		assert_eq!(query.data(), Some(2));
	});
}

#[rstest]
fn promoted_navigation_lease_refetches_if_invalidated_while_loading() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), i32, String>::new("tests.promoted-navigation-invalidation");
	let key = family.key(());
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let client = client.clone();
		let key = key.clone();
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let client = client.clone();
			let key = key.clone();
			let call = fetch_count.get() + 1;
			fetch_count.set(call);
			async move {
				if call == 1 {
					client.invalidate(&key);
				}
				Ok::<_, String>(call)
			}
		}
	});
	let lease = client.acquire(
		descriptor,
		QueryAcquireOptions {
			consumer: QueryConsumer::Navigation(8),
			error_policy: QueryErrorPolicy::Discard,
		},
	);
	runtime.run_until_stalled();
	assert_eq!(fetch_count.get(), 1);

	lease.promote_to_mounted_route(8);
	runtime.run_until_stalled();

	assert_eq!(fetch_count.get(), 2);
}

#[test]
fn dropping_a_queued_manual_refetch_observer_does_not_start_a_fallback_fetch() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.dropped-manual-refetch");
	let ready = Rc::new(Cell::new(false));
	let calls = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let ready = Rc::clone(&ready);
		let calls = Rc::clone(&calls);
		move || {
			calls.set(calls.get() + 1);
			TestGate {
				ready: Rc::clone(&ready),
				dropped: Rc::new(Cell::new(0)),
				result: Some(Ok("initial".to_string())),
			}
		}
	});
	let queued_observer = client.observe(descriptor.clone(), QueryOptions::default());
	let remaining_observer = client.observe(descriptor, QueryOptions::default());
	queued_observer.refetch();
	drop(queued_observer);

	ready.set(true);
	runtime.run_until_stalled();

	assert_eq!(calls.get(), 1);
	assert_eq!(remaining_observer.data(), Some("initial".to_string()));
}

#[test]
fn ssr_prefetch_policy_can_only_disable_an_eligible_descriptor() {
	let family = QueryFamily::<(), String, String>::new("tests.ssr-prefetch-policy");

	let eligible = family.query((), || async { Ok("eligible".to_string()) });
	let disabled = eligible.clone().with_ssr_prefetch(false);
	let attempted_reenable = disabled.clone().with_ssr_prefetch(true);

	assert!(eligible.ssr_prefetch);
	assert!(!disabled.ssr_prefetch);
	assert!(!attempted_reenable.ssr_prefetch);
}

#[test]
#[serial(query_cache)]
fn typed_query_identity_does_not_reserve_resource_counter() {
	ReactiveScope::run(|| {
		// Arrange
		let _query_client = isolated_query_client();
		super::super::resource::set_client_resource_counter(0);

		// Act
		let _entry = query_entry(
			QueryFamily::<(), _, _>::new("rh-res-0")
				.query((), || async { Ok::<_, String>("query".to_string()) }),
		);

		// Assert
		assert_eq!(super::super::resource::current_client_resource_counter(), 0);
		super::super::resource::set_client_resource_counter(0);
	});
}

#[test]
#[serial(query_cache)]
fn hydrated_query_error_is_fresh_on_first_mount() {
	ReactiveScope::run(|| {
		// Arrange
		let (hydrated_state, last_fetched_ms) =
			initial_query_state(Some(ResourceState::Error("not found".to_string())));
		let entry = QueryEntry::new(
			QueryFamily::<(), _, _>::new("hydrated-query-error")
				.query((), || async { Err::<String, _>("not found".to_string()) }),
		);
		entry.state.set(hydrated_state);
		entry.last_fetched_ms.set(last_fetched_ms);

		// Assert
		assert!(
			!entry.should_fetch_on_mount(QueryDefaults::default().resolved_stale_time()),
			"a freshly hydrated error must remain visible for the initial mount"
		);
	});
}

#[test]
fn hydration_snapshot_is_consumed_once_after_entry_eviction() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(
		QueryDefaults::default().gc_time(Duration::ZERO),
		runtime.handle(),
	);
	let family = QueryFamily::<(), String, String>::new("tests.hydration-consumption");
	let key = family.key(());
	let snapshot = serde_json::json!({
		"state": { "Success": "server-value" },
		"refetch_error": null,
		"is_fetching": false,
		"is_stale": false
	});
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			fetch_count.set(fetch_count.get() + 1);
			async { Ok::<_, String>("client-value".to_string()) }
		}
	});

	client
		.seed_query_snapshot(key.clone(), &snapshot)
		.expect("initial hydration snapshot should seed");
	let hydrated = client.observe(descriptor.clone(), QueryOptions::default());
	assert_eq!(hydrated.data(), Some("server-value".to_string()));
	assert_eq!(fetch_count.get(), 0);
	drop(hydrated);
	runtime.run_due_maintenance();
	assert!(!client.contains_for_test(&key));

	client
		.seed_query_snapshot(key, &snapshot)
		.expect("a consumed hydration snapshot should be ignored");
	let remounted = client.observe(descriptor, QueryOptions::default());
	runtime.run_until_stalled();

	assert_eq!(fetch_count.get(), 1);
	assert_eq!(remounted.data(), Some("client-value".to_string()));
}

#[test]
fn exact_removal_consumes_an_unobserved_hydration_snapshot() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.hydration-exact-removal");
	let key = family.key(());
	let snapshot = serde_json::json!({
		"state": { "Success": "server-value" },
		"refetch_error": null,
		"is_fetching": false,
		"is_stale": false
	});
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			fetch_count.set(fetch_count.get() + 1);
			async { Ok::<_, String>("client-value".to_string()) }
		}
	});

	client.remove(&key);
	client
		.seed_query_snapshot(key, &snapshot)
		.expect("an auth-boundary removal should suppress the old snapshot");
	let query = client.observe(descriptor, QueryOptions::default());
	runtime.run_until_stalled();

	assert_eq!(fetch_count.get(), 1);
	assert_eq!(query.data(), Some("client-value".to_string()));
}

#[test]
fn family_removal_consumes_unobserved_hydration_snapshots() {
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.hydration-family-removal");
	let key = family.key(());
	let snapshot = serde_json::json!({
		"state": { "Success": "server-value" },
		"refetch_error": null,
		"is_fetching": false,
		"is_stale": false
	});
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			fetch_count.set(fetch_count.get() + 1);
			async { Ok::<_, String>("client-value".to_string()) }
		}
	});

	client.remove_family(family);
	client
		.seed_query_snapshot(key, &snapshot)
		.expect("a family removal should suppress old family snapshots");
	let query = client.observe(descriptor, QueryOptions::default());
	runtime.run_until_stalled();

	assert_eq!(fetch_count.get(), 1);
	assert_eq!(query.data(), Some("client-value".to_string()));
}

#[rstest]
fn successful_null_snapshot_preserves_present_data_during_hydration() {
	// Arrange
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), Option<String>, String>::new("tests.hydration-null-success");
	let key = family.key(());
	let snapshot = serde_json::json!({
		"state": { "Success": null },
		"refetch_error": null,
		"is_fetching": false,
		"is_stale": false
	});

	// Act
	client
		.seed_query_snapshot(key, &snapshot)
		.expect("a successful null value should hydrate");
	let hydrated = client.observe(
		family.query((), || async { Ok::<_, String>(Some("client".to_string())) }),
		QueryOptions::default(),
	);

	// Assert
	assert_eq!(hydrated.snapshot().status, QueryStatus::Success);
	assert_eq!(hydrated.data(), Some(None));
}

#[rstest]
fn promoted_navigation_lease_refetches_on_invalidation() {
	// Arrange
	let runtime = TestQueryRuntime::new();
	let client = QueryClient::with_runtime(QueryDefaults::default(), runtime.handle());
	let family = QueryFamily::<(), String, String>::new("tests.promoted-navigation-lease");
	let key = family.key(());
	let fetch_count = Rc::new(Cell::new(0));
	let descriptor = family.query((), {
		let fetch_count = Rc::clone(&fetch_count);
		move || {
			let call = fetch_count.get() + 1;
			fetch_count.set(call);
			async move { Ok::<_, String>(format!("value-{call}")) }
		}
	});
	let lease = client.acquire(
		descriptor,
		QueryAcquireOptions {
			consumer: QueryConsumer::Navigation(7),
			error_policy: QueryErrorPolicy::Discard,
		},
	);
	runtime.run_until_stalled();
	lease.promote_to_mounted_route(7);

	// Act
	client.invalidate(&key);
	runtime.run_until_stalled();

	// Assert
	assert_eq!(fetch_count.get(), 2);
}

#[tokio::test]
#[serial(query_cache)]
async fn ssr_replayed_query_error_is_fresh_for_stale_time() {
	// Arrange
	let context = Rc::new(RefCell::new(
		crate::ssr::resource_context::SsrResourceContext::new(Duration::from_secs(1)),
	));

	let client = QueryClient::new_ssr(QueryDefaults::default());
	let discovery_query = with_query_client_async(
		client.clone(),
		crate::ssr::resource_context::scope_context(Rc::clone(&context), async {
			ReactiveScope::run(|| {
				let query = use_query(
					QueryFamily::<(), _, _>::new("ssr-replayed-query-error")
						.query((), || async { Err::<String, _>("not found".to_string()) }),
					QueryOptions::default(),
				);
				let _ = query.snapshot();
				query
			})
		}),
	)
	.await;
	assert!(crate::ssr::resource_context::resolve_external_resources(&context).await);

	// Act
	let replayed_query = with_query_client_async(
		client,
		crate::ssr::resource_context::scope_context(Rc::clone(&context), async {
			ReactiveScope::run(|| {
				let query = use_query(
					QueryFamily::<(), _, _>::new("ssr-replayed-query-error").query((), || async {
						Err::<String, _>("must not refetch during replay".to_string())
					}),
					QueryOptions::default().stale_time(Duration::from_secs(30)),
				);

				// Assert
				assert_eq!(query.error(), Some("not found".to_string()));
				assert!(
					!query.is_stale(),
					"a replayed error must remain fresh when stale_time is applied"
				);
				query
			})
		}),
	)
	.await;
	drop(replayed_query);
	drop(discovery_query);
}

#[rstest]
#[serial(query_cache)]
fn server_fn_key_hashes_arguments_without_exposing_them() {
	// Arrange
	let _query_client = isolated_query_client();

	// Act
	let key: QueryKey<Vec<i64>, crate::server_fn::ServerFnError> =
		QueryFamily::new("server_fn:/api/server_fn/list_jobs:json").key((42_i64,));

	// Assert
	assert_eq!(
		key.id(),
		"server_fn:/api/server_fn/list_jobs:json:sha256:b86b1ea11b28136fe5224b9d1e3017b7efb68d4fae0b90c4940e0c0f89b3907a"
	);
	assert_eq!(
		key.hydration_id(),
		"query:server_fn:/api/server_fn/list_jobs:json:sha256:b86b1ea11b28136fe5224b9d1e3017b7efb68d4fae0b90c4940e0c0f89b3907a"
	);
	assert!(!key.id().contains("[42]"));
	assert!(!key.hydration_id().contains("[42]"));
}

#[rstest]
#[serial(query_cache)]
fn server_fn_key_preserves_large_integer_arguments() {
	// Arrange
	let _query_client = isolated_query_client();

	// Act
	let key: QueryKey<(), crate::server_fn::ServerFnError> =
		QueryFamily::new("server_fn:/api/server_fn/load_job:json").key((u128::MAX,));

	// Assert
	assert_eq!(
		key.id(),
		"server_fn:/api/server_fn/load_job:json:sha256:d80bcc323657a82faa939889d29892c9b53c3bb4f98ff3738140a27a3ac7b9df"
	);
	assert!(!key.id().contains(&u128::MAX.to_string()));
}

#[rstest]
#[serial(query_cache)]
fn server_fn_key_canonicalizes_object_arguments() {
	// Arrange
	let _query_client = isolated_query_client();

	// Act
	let first: QueryKey<(), crate::server_fn::ServerFnError> =
		QueryFamily::new("server_fn:/api/server_fn/filter_jobs:json")
			.key((OrderedMapArgs(&[("status", 1), ("owner", 2)]),));
	let second: QueryKey<(), crate::server_fn::ServerFnError> =
		QueryFamily::new("server_fn:/api/server_fn/filter_jobs:json")
			.key((OrderedMapArgs(&[("owner", 2), ("status", 1)]),));

	// Assert
	assert_eq!(first.id(), second.id());
	assert_eq!(
		first.id(),
		"server_fn:/api/server_fn/filter_jobs:json:sha256:b2b2c11c6c2d2aacfabe8dba6102508d46a7690b66d0662adc332e4802f078d2"
	);
}

#[rstest]
#[serial(query_cache)]
fn server_fn_key_canonicalizes_large_integer_object_arguments() {
	// Arrange
	let _query_client = isolated_query_client();

	// Act
	let first: QueryKey<(), crate::server_fn::ServerFnError> =
		QueryFamily::new("server_fn:/api/server_fn/filter_large_jobs:json")
			.key((OrderedLargeMapArgs(&[("status", u128::MAX), ("owner", 2)]),));
	let second: QueryKey<(), crate::server_fn::ServerFnError> =
		QueryFamily::new("server_fn:/api/server_fn/filter_large_jobs:json")
			.key((OrderedLargeMapArgs(&[("owner", 2), ("status", u128::MAX)]),));

	// Assert
	assert_eq!(first.id(), second.id());
}

#[rstest]
#[serial(query_cache)]
fn server_fn_key_does_not_expose_sensitive_arguments() {
	// Arrange
	let _query_client = isolated_query_client();

	let email = "sensitive@example.com";

	// Act
	let key: QueryKey<(), crate::server_fn::ServerFnError> =
		QueryFamily::new("server_fn:/api/server_fn/load_user:json").key((email,));

	// Assert
	assert_eq!(
		key.id(),
		"server_fn:/api/server_fn/load_user:json:sha256:5cb828e12cdd77b9af33cfac3c965b44acc673692df8ffb22bc6794506ea59bc"
	);
	assert!(!key.id().contains(email));
}
