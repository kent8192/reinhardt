use std::fmt;
use std::rc::Rc;
use std::time::Duration;

/// A marker that disables retry behavior for a query observer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NoRetry;

/// Configures exponential retry behavior for a query observer.
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
	pub fn exponential() -> Self {
		Self {
			max_attempts: 3,
			base_delay: Duration::from_millis(250),
			max_delay: Duration::from_secs(5),
			jitter: false,
			when: Rc::new(|_| true),
		}
	}

	/// Sets the maximum number of retry attempts.
	pub fn max_attempts(mut self, value: u32) -> Self {
		self.max_attempts = value;
		self
	}

	/// Sets the initial delay used by exponential backoff.
	pub fn base_delay(mut self, value: Duration) -> Self {
		self.base_delay = value;
		self
	}

	/// Sets the maximum delay used by exponential backoff.
	pub fn max_delay(mut self, value: Duration) -> Self {
		self.max_delay = value;
		self
	}

	/// Enables or disables random jitter for retry delays.
	pub fn jitter(mut self, value: bool) -> Self {
		self.jitter = value;
		self
	}

	/// Sets the predicate that selects errors eligible for retry.
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
}
