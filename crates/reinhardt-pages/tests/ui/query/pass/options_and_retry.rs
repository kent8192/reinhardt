use std::time::Duration;

use reinhardt_pages::reactive::{QueryFamily, QueryOptions, RetryPolicy, use_query};

#[derive(Clone, serde::Deserialize, serde::Serialize)]
enum AppError {
	Transient,
	Permanent,
}

fn main() {
	let family = QueryFamily::<(), String, AppError>::new("ui.query-retry");
	let descriptor = family.query((), || async { Ok("ready".to_owned()) });
	let _plain: QueryOptions = QueryOptions::new();
	let _default: QueryOptions = QueryOptions::default();
	let policy = RetryPolicy::exponential()
		.max_attempts(3)
		.base_delay(Duration::from_millis(250))
		.max_delay(Duration::from_secs(5))
		.jitter(true)
		.when(|error: &AppError| matches!(error, AppError::Transient));
	if false {
		let _explicit_default =
			use_query::<String, AppError>(descriptor.clone(), QueryOptions::default());
		let _explicit_retry = use_query::<String, AppError>(
			descriptor.clone(),
			QueryOptions::new().retry(policy.clone()),
		);
		let _before = use_query(
			descriptor.clone(),
			QueryOptions::new().retry(policy.clone()).enabled(true),
		);
		let _after = use_query(
			descriptor,
			QueryOptions::new().enabled(true).retry(policy),
		);
	}
}
