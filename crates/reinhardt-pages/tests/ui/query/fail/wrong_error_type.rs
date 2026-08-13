use reinhardt_pages::reactive::{QueryFamily, QueryOptions, RetryPolicy, use_query};

#[derive(Clone, serde::Deserialize, serde::Serialize)]
enum AppError {}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
enum OtherError {}

fn main() {
	let family = QueryFamily::<(), String, AppError>::new("ui.query-retry.wrong-error");
	let descriptor = family.query((), || async { Ok("ready".to_owned()) });
	let policy = RetryPolicy::<OtherError>::exponential();
	let _ = use_query(descriptor, QueryOptions::new().retry(policy));
}
