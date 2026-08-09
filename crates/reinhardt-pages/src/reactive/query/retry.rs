use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::time::Duration;

pub(crate) type ObserverRegistrationId = u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RetryWaitClock {
	Visible { failed_at_ms: u64 },
	Paused { visible_elapsed_ms: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RetryCandidate {
	pub(crate) delay_ms: u64,
}

pub(crate) struct RetrySequence<E> {
	pub(crate) generation: u64,
	pub(crate) completion_generation: u64,
	pub(crate) attempts_started: u32,
	pub(crate) last_error: Option<E>,
	pub(crate) jitter_sample: Option<u64>,
	pub(crate) candidates: HashMap<ObserverRegistrationId, RetryCandidate>,
	pub(crate) wait_clock: Option<RetryWaitClock>,
	pub(crate) manual_observer_id: Option<ObserverRegistrationId>,
	pub(crate) had_success: bool,
	pub(crate) invalidation_generation: u64,
}

impl<E> RetrySequence<E> {
	pub(crate) fn new(
		generation: u64,
		completion_generation: u64,
		manual_observer_id: Option<ObserverRegistrationId>,
		had_success: bool,
		invalidation_generation: u64,
	) -> Self {
		Self {
			generation,
			completion_generation,
			attempts_started: 0,
			last_error: None,
			jitter_sample: None,
			candidates: HashMap::new(),
			wait_clock: None,
			manual_observer_id,
			had_success,
			invalidation_generation,
		}
	}

	pub(crate) fn record_attempt_started(&mut self) -> u32 {
		self.clear_failure();
		self.attempts_started = self.attempts_started.saturating_add(1);
		self.attempts_started
	}

	pub(crate) fn record_failure(
		&mut self,
		error: E,
		failed_at_ms: u64,
		jitter_sample: Option<u64>,
		candidates: HashMap<ObserverRegistrationId, RetryCandidate>,
	) {
		self.last_error = Some(error);
		self.jitter_sample = jitter_sample;
		self.candidates = candidates;
		self.wait_clock = Some(RetryWaitClock::Visible { failed_at_ms });
	}

	pub(crate) fn candidate_due_ms(
		&self,
		observer_id: ObserverRegistrationId,
		now_ms: u64,
	) -> Option<u64> {
		let delay_ms = self.candidates.get(&observer_id)?.delay_ms;
		match self.wait_clock? {
			RetryWaitClock::Visible { failed_at_ms } => Some(failed_at_ms.saturating_add(delay_ms)),
			RetryWaitClock::Paused { visible_elapsed_ms } => {
				Some(now_ms.saturating_add(delay_ms.saturating_sub(visible_elapsed_ms)))
			}
		}
	}

	pub(crate) fn earliest_due_ms(&self, now_ms: u64) -> Option<u64> {
		self.candidates
			.keys()
			.filter_map(|observer_id| self.candidate_due_ms(*observer_id, now_ms))
			.min()
	}

	// Browser visibility integration is assigned to a later task, but these
	// transitions are part of the stable retry-sequence interface.
	#[allow(dead_code)]
	pub(crate) fn pause(&mut self, now_ms: u64) {
		if let Some(RetryWaitClock::Visible { failed_at_ms }) = self.wait_clock {
			self.wait_clock = Some(RetryWaitClock::Paused {
				visible_elapsed_ms: now_ms.saturating_sub(failed_at_ms),
			});
		}
	}

	// Browser visibility integration is assigned to a later task, but these
	// transitions are part of the stable retry-sequence interface.
	#[allow(dead_code)]
	pub(crate) fn resume(&mut self, now_ms: u64) {
		if let Some(RetryWaitClock::Paused { visible_elapsed_ms }) = self.wait_clock {
			self.wait_clock = Some(RetryWaitClock::Visible {
				failed_at_ms: now_ms.saturating_sub(visible_elapsed_ms),
			});
		}
	}

	pub(crate) fn clear_failure(&mut self) {
		self.last_error = None;
		self.jitter_sample = None;
		self.candidates.clear();
		self.wait_clock = None;
	}
}

/// A marker that disables retry behavior for a query observer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NoRetry;

/// Configures exponential retry behavior for a query observer.
///
/// A policy belongs to an observer, while attempts and intermediate failures
/// are coordinated by the shared cache entry. The predicate is stored as a
/// typed `Fn(&E) -> bool`, so policies and [`QueryOptions`](super::QueryOptions)
/// that contain them implement `Clone` and `Debug`, but not equality.
pub struct RetryPolicy<E> {
	pub(crate) max_attempts: u32,
	pub(crate) base_delay: Duration,
	pub(crate) max_delay: Duration,
	pub(crate) jitter: bool,
	pub(crate) when: Rc<dyn Fn(&E) -> bool>,
}

impl<E> Clone for RetryPolicy<E> {
	fn clone(&self) -> Self {
		Self {
			max_attempts: self.max_attempts,
			base_delay: self.base_delay,
			max_delay: self.max_delay,
			jitter: self.jitter,
			when: Rc::clone(&self.when),
		}
	}
}

impl<E> RetryPolicy<E> {
	/// Creates a policy with standard exponential retry defaults.
	///
	/// The defaults allow three total attempts, including the initial request,
	/// start at 250 milliseconds, cap the nominal delay at five seconds, disable
	/// jitter, and accept every error. Zero-delay policies are also valid when
	/// configured explicitly.
	pub fn exponential() -> Self {
		Self {
			max_attempts: 3,
			base_delay: Duration::from_millis(250),
			max_delay: Duration::from_secs(5),
			jitter: false,
			when: Rc::new(|_| true),
		}
	}

	/// Sets the maximum number of total attempts, including the initial request.
	///
	/// A zero value is rejected when the policy is installed with
	/// [`QueryOptions::retry`](super::QueryOptions::retry).
	pub fn max_attempts(mut self, value: u32) -> Self {
		self.max_attempts = value;
		self
	}

	/// Sets the initial delay used by exponential backoff.
	///
	/// A zero duration is valid. The policy is rejected when it is installed if
	/// this delay is greater than the maximum delay. Positive delays below one
	/// millisecond are scaled before being rounded up for the scheduler.
	pub fn base_delay(mut self, value: Duration) -> Self {
		self.base_delay = value;
		self
	}

	/// Sets the maximum nominal delay used by exponential backoff.
	///
	/// The policy is rejected when it is installed if this delay is less than
	/// the base delay.
	pub fn max_delay(mut self, value: Duration) -> Self {
		self.max_delay = value;
		self
	}

	/// Enables or disables equal jitter for retry delays.
	///
	/// Equal jitter chooses a delay from half the nominal delay through the full
	/// nominal delay, so it never exceeds the configured exponential backoff.
	pub fn jitter(mut self, value: bool) -> Self {
		self.jitter = value;
		self
	}

	/// Sets the typed predicate that selects errors eligible for retry.
	///
	/// The predicate receives a shared reference to the attempt error and must
	/// be `'static`. Returning `false` makes that observer ineligible for another
	/// attempt.
	pub fn when(mut self, value: impl Fn(&E) -> bool + 'static) -> Self {
		self.when = Rc::new(value);
		self
	}

	pub(crate) fn validate(&self) {
		assert!(
			self.max_attempts > 0,
			"RetryPolicy.max_attempts must be greater than 0, got {}",
			self.max_attempts,
		);
		assert!(
			self.max_delay >= self.base_delay,
			"RetryPolicy.max_delay ({:?}) must be greater than or equal to base_delay ({:?})",
			self.max_delay,
			self.base_delay,
		);
	}

	pub(crate) fn delay_ms(&self, failed_attempt: u32, sample: u64) -> u64 {
		assert!(
			failed_attempt > 0,
			"query retry failed_attempt must be greater than 0"
		);
		let exponent = failed_attempt.saturating_sub(1).min(127);
		let multiplier = 1_u128.checked_shl(exponent).unwrap_or(u128::MAX);
		let nominal_nanos = self
			.base_delay
			.as_nanos()
			.saturating_mul(multiplier)
			.min(self.max_delay.as_nanos());
		let nominal = nominal_nanos
			.saturating_add(999_999)
			.checked_div(1_000_000)
			.unwrap_or_default()
			.min(u128::from(u64::MAX)) as u64;
		if !self.jitter {
			return nominal;
		}
		let floor = nominal / 2;
		let spread = nominal - floor;
		let range = u128::from(spread) + 1;
		let offset = ((u128::from(sample) * range) >> 64) as u64;
		let jittered = floor.saturating_add(offset).min(nominal);
		if nominal == 0 { 0 } else { jittered.max(1) }
	}
}

impl<E> fmt::Debug for RetryPolicy<E> {
	fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
		formatter
			.debug_struct("RetryPolicy")
			.field("max_attempts", &self.max_attempts)
			.field("base_delay", &self.base_delay)
			.field("max_delay", &self.max_delay)
			.field("jitter", &self.jitter)
			.field("when", &"<predicate>")
			.finish()
	}
}

mod private {
	pub trait Sealed {}
}

/// Internal retry-state adapter used to associate policy error types with query errors.
#[doc(hidden)]
pub trait QueryRetryConfig<E>: private::Sealed + Clone + 'static {
	/// Returns the installed retry policy, when retries are enabled.
	fn retry_policy(&self) -> Option<&RetryPolicy<E>>;
}

impl private::Sealed for NoRetry {}

impl<E> QueryRetryConfig<E> for NoRetry {
	fn retry_policy(&self) -> Option<&RetryPolicy<E>> {
		None
	}
}

impl<E> private::Sealed for RetryPolicy<E> {}

impl<E: 'static> QueryRetryConfig<E> for RetryPolicy<E> {
	fn retry_policy(&self) -> Option<&RetryPolicy<E>> {
		Some(self)
	}
}

#[cfg(test)]
mod tests {
	use super::RetryPolicy;
	use crate::reactive::query::QueryOptions;
	use std::time::Duration;

	#[test]
	#[should_panic(expected = "RetryPolicy.max_attempts must be greater than 0, got 0")]
	fn retry_policy_rejects_zero_max_attempts() {
		let _ = QueryOptions::new().retry(RetryPolicy::<()>::exponential().max_attempts(0));
	}

	#[test]
	#[should_panic(
		expected = "RetryPolicy.max_delay (1s) must be greater than or equal to base_delay (2s)"
	)]
	fn retry_policy_rejects_max_delay_before_base_delay() {
		let _ = QueryOptions::new().retry(
			RetryPolicy::<()>::exponential()
				.base_delay(Duration::from_secs(2))
				.max_delay(Duration::from_secs(1)),
		);
	}

	#[test]
	fn retry_policy_accepts_zero_delays() {
		let _: QueryOptions<RetryPolicy<()>> = QueryOptions::new().retry(
			RetryPolicy::exponential()
				.base_delay(Duration::ZERO)
				.max_delay(Duration::ZERO),
		);
	}

	#[test]
	fn retry_policy_clone_retains_its_predicate() {
		let policy = RetryPolicy::<u8>::exponential().when(|value| *value == 7);

		assert!((policy.clone().when)(&7));
	}

	#[test]
	fn retry_policy_debug_redacts_its_predicate() {
		let policy = RetryPolicy::<()>::exponential();

		assert_eq!(
			format!("{policy:?}"),
			"RetryPolicy { max_attempts: 3, base_delay: 250ms, max_delay: 5s, jitter: false, when: \"<predicate>\" }",
		);
	}

	#[test]
	fn retry_policy_caps_exponential_delay() {
		let policy = RetryPolicy::<()>::exponential()
			.base_delay(Duration::from_millis(100))
			.max_delay(Duration::from_millis(500));

		for (attempt, expected_ms) in [(1, 100), (2, 200), (3, 400), (4, 500)] {
			assert_eq!(policy.delay_ms(attempt, 0), expected_ms);
		}
	}

	#[test]
	fn retry_policy_scales_sub_millisecond_delays_before_rounding_up() {
		let policy = RetryPolicy::<()>::exponential()
			.base_delay(Duration::from_micros(500))
			.max_delay(Duration::from_millis(2));

		for (attempt, expected_ms) in [(1, 1), (2, 1), (3, 2), (4, 2)] {
			assert_eq!(policy.delay_ms(attempt, 0), expected_ms);
		}
	}

	#[test]
	fn retry_policy_projects_equal_jitter_from_the_supplied_sample() {
		let policy = RetryPolicy::<()>::exponential()
			.base_delay(Duration::from_millis(100))
			.max_delay(Duration::from_millis(100))
			.jitter(true);

		for (sample, expected_ms) in [(0, 50), (u64::MAX / 2, 75), (u64::MAX, 100)] {
			assert_eq!(policy.delay_ms(1, sample), expected_ms);
		}
	}

	#[test]
	fn retry_policy_keeps_positive_jittered_delays_above_zero() {
		let policy = RetryPolicy::<()>::exponential()
			.base_delay(Duration::from_micros(500))
			.max_delay(Duration::from_micros(500))
			.jitter(true);

		assert_eq!(policy.delay_ms(1, 0), 1);
	}

	#[test]
	fn retry_policy_ignores_samples_when_jitter_is_disabled() {
		let policy = RetryPolicy::<()>::exponential()
			.base_delay(Duration::from_millis(100))
			.max_delay(Duration::from_millis(100));

		for sample in [0, u64::MAX / 2, u64::MAX] {
			assert_eq!(policy.delay_ms(1, sample), 100);
		}
	}

	#[test]
	fn retry_policy_saturates_duration_and_attempt_overflow() {
		let policy = RetryPolicy::<()>::exponential()
			.base_delay(Duration::MAX)
			.max_delay(Duration::MAX);

		assert_eq!(policy.delay_ms(u32::MAX, 0), u64::MAX);
	}
}
