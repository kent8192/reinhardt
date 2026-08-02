use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use reinhardt_core::reactive::ReactiveScope;
use rstest::rstest;
use serde::Serialize;
use serde::Serializer;
use serde::ser::{Error as _, SerializeMap};
use serial_test::serial;

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
		entry.complete_fetch(generation, Ok("obsolete".to_string()));
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
			Err("route failed".to_string())
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
			Err("route failed".to_string())
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
			Err("route failed".to_string())
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
